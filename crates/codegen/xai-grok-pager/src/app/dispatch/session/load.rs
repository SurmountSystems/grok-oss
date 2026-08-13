//! Session loading, session pickers, and deep-search dispatchers.
use super::foreign::{dispatch_fetch_session_list, invalidate_foreign_picker};
use super::fork::build_child_fork_marker;
use super::lifecycle::{
    clear_startup_actions, dispatch_new_session_inner, dispatch_new_worktree_session,
    refuse_chat_mode_build_agent,
};
use crate::acp::tracker::AcpUpdateTracker;
use crate::app::actions::{Action, Effect};
use crate::app::agent::{AgentCommand, AgentId, AgentSession, AgentState};
use crate::app::agent_view::AgentView;
use crate::app::app_view::AppView;
use crate::app::dispatch::ctx::{
    SwitchCause, get_active_agent, get_active_agent_mut, switch_to_agent, with_active_agent,
};
use crate::app::dispatch::modes::inherit_auto_mode;
use crate::app::dispatch::prompt::{defer_to_open_reload_window, supersede_open_reload_window};
use crate::app::dispatch::queue::{
    force_drain_queue_past_background, maybe_drain_queue, note_peek_page_flip,
};
use crate::app::dispatch::router::dispatch;
use crate::app::dispatch::status::notify_session_ready;
use crate::app::dispatch::transcript::extensions_modal_tab_fetches;
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::SessionEvent;
use crate::scrollback::state::ScrollbackState;
use agent_client_protocol as acp;
/// Create a placeholder agent and load an existing session by ID.
///
/// `session_cwd` overrides the CWD in the `LoadSessionRequest`. This is needed
/// when resuming a session that was created in a different CWD (e.g., a worktree).
pub(in crate::app::dispatch) fn dispatch_load_session(
    app: &mut AppView,
    session_id: String,
    session_cwd: Option<std::path::PathBuf>,
    chat_kind: bool,
) -> Vec<Effect> {
    if !app.session_startup_allowed() {
        app.deferred_startup.session =
            Some(crate::app::session_startup::DeferredSessionStartup::Load {
                session_id,
                session_cwd,
                chat_kind,
            });
        return vec![];
    }
    dispatch_load_session_ungated(app, session_id, session_cwd, chat_kind)
}
/// Clear `session_id` from any existing agent that already owns the given
/// session, then return a freshly constructed [`acp::SessionId`].
///
/// Without this, `find_session_match` finds the stale agent first (IndexMap
/// insertion order) and routes all ACP notifications to it instead of the
/// new agent.
pub(in crate::app::dispatch) fn clear_stale_session_id(
    app: &mut AppView,
    session_id: &str,
) -> acp::SessionId {
    let sid = acp::SessionId::new(session_id);
    for agent in app.agents.values_mut() {
        if agent.session.session_id.as_ref() == Some(&sid) {
            agent.unbind_session_id();
        }
    }
    sid
}
/// If a local agent already owns this id **and** matches kind, focus it.
///
/// - Kind: compare against the stamped form `chat_kind || app.chat_mode` (agents
///   store that; the LoadSession arg is conversation-entry only). Conversation
///   vs Build still differs when sticky `--chat` is off.
/// - Eager `session_id` + leftover load placeholder after `SessionLoadFailed`
///   is not "open" — reissue load instead of focusing.
/// - Overlay: retarget when on the dashboard list, already in overlay (attached
///   matches visible), or attached already points at the agent we will show
///   (so switch activates overlay with the correct `focus_row`).
pub(in crate::app::dispatch) fn focus_if_session_already_open(
    app: &mut AppView,
    session_id: &str,
    chat_kind: bool,
) -> Option<AgentId> {
    use crate::app::app_view::ActiveView;
    use crate::views::dashboard::DashboardRowId;
    let expected_kind = chat_kind || app.chat_mode;
    let existing_id = app.agents.iter().find_map(|(id, a)| {
        let sid_ok = a
            .session
            .session_id
            .as_ref()
            .is_some_and(|sid| &*sid.0 == session_id);
        if !sid_ok || a.chat_kind != expected_kind {
            return None;
        }
        if a.loading_placeholder_id.is_some() && !a.session.loading_replay {
            return None;
        }
        Some(*id)
    })?;
    if let Some(agent) = app.agents.get_mut(&existing_id) {
        agent.active_subagent = None;
    }
    let retarget_overlay = match app.active_view {
        ActiveView::AgentDashboard => true,
        ActiveView::Agent(visible) => app.dashboard.as_ref().is_some_and(|d| {
            d.attached_agent == Some(visible) || d.attached_agent == Some(existing_id)
        }),
        _ => false,
    };
    if retarget_overlay && let Some(d) = app.dashboard.as_mut() {
        d.focus_row(DashboardRowId::TopLevel(existing_id));
        d.attached_agent = Some(existing_id);
    }
    switch_to_agent(app, existing_id, SwitchCause::Load);
    Some(existing_id)
}
fn dispatch_load_session_ungated(
    app: &mut AppView,
    session_id: String,
    session_cwd: Option<std::path::PathBuf>,
    chat_kind: bool,
) -> Vec<Effect> {
    if crate::app::session_startup::chat_mode_refuses_local_build_load(
        app.chat_mode,
        chat_kind,
        &session_id,
        &app.cwd,
    ) {
        app.show_toast(crate::app::session_startup::CHAT_MODE_LOCAL_BUILD_REFUSAL);
        return vec![];
    }
    invalidate_picker_fetch_on_dismiss(app);
    if let Some(existing_id) = focus_if_session_already_open(app, &session_id, chat_kind) {
        // Already in this process: no SessionLoaded. Still auto-resume when
        // the last primary turn ended in error (403 / Internal error).
        return try_auto_resume_error_idle_on_reopen(app, existing_id);
    }
    let acp_session_id = clear_stale_session_id(app, &session_id);
    let agent_id = AgentId(app.next_agent_id);
    app.next_agent_id += 1;
    let mut scrollback = ScrollbackState::new();
    scrollback.set_appearance(app.appearance.clone());
    let loading_msg = if matches!(app.restore_code, Some(true)) {
        format!("Restoring code for session {}...", &session_id)
    } else {
        format!("Loading session {}...", &session_id)
    };
    let loading_placeholder_id = scrollback.push_block(RenderBlock::system(loading_msg));
    let agent = AgentView::new(
        AgentSession {
            id: agent_id,
            acp_tx: app.acp_tx.clone(),
            session_id: Some(acp_session_id),
            models: app.models.clone(),
            state: AgentState::Idle,
            tracker: AcpUpdateTracker::new(),
            cwd: session_cwd.clone().unwrap_or_else(|| app.cwd.clone()),
            is_worktree: false,
            forked_from: None,
            pending_prompts: std::collections::VecDeque::new(),
            next_queue_id: 0,
            yolo_mode: app.default_yolo,
            auto_mode: inherit_auto_mode(app),
            prompt_history: Vec::new(),
            prompt_history_loading: true,
            loading_replay: true,
            restore_degree: None,
            rate_limited: false,
            model_incompatible: false,
            credit_limit_blocked: false,
            free_usage_blocked: false,
            available_commands: app.bootstrap_acp_commands.clone(),
            available_commands_generation: 1,
            available_tools: None,
            model_switch_pending: false,
            user_model_preference: None,
            deferred_model_switch: app.deferred_model_switch_from_cli(),
            bg_tasks: std::collections::BTreeMap::new(),
            bg_tool_call_to_task: std::collections::HashMap::new(),
            scheduled_tasks: std::collections::HashMap::new(),
            in_flight_prompt: None,
            cancel_resume_prompt_text: None,
            compact_held_prompt: None,
            current_prompt_id: None,
            created_via_new: false,
            session_notes: crate::app::agent::SessionNotes::default(),
        },
        scrollback,
    );
    app.agents.insert(agent_id, agent);
    let agent_mut = app.agents.get_mut(&agent_id).unwrap();
    agent_mut.attached_as_viewer = true;
    agent_mut.begin_replay_window();
    agent_mut.loading_placeholder_id = Some(loading_placeholder_id);
    agent_mut.prompt.set_compact(app.appearance.prompt.compact);
    agent_mut.prompt.adopt_slash_mru(app.slash_mru.clone());
    agent_mut
        .prompt
        .adopt_command_tags(app.command_tags.clone());
    agent_mut
        .prompt
        .set_contextual_hints(app.contextual_hints.undo, app.contextual_hints.plan_mode);
    agent_mut.set_session_recap_available(app.session_recap_available);
    agent_mut.set_voice_mode_available(app.voice_mode_enabled);
    agent_mut.scrollback.begin_batch();
    if matches!(app.restore_code, Some(true)) {
        agent_mut.session.start_command(AgentCommand::RestoreCode);
        agent_mut.turn_started_at = Some(std::time::Instant::now());
    }
    agent_mut.apply_app_scoped_gates(
        app.sharing_enabled,
        app.usage_visible,
        app.chat_mode,
        app.screen_mode,
        &app.active_announcements,
        &app.tier_restricted_commands,
    );
    agent_mut.chat_kind = chat_kind || app.chat_mode;
    agent_mut.apply_credit_balance(
        app.credit_balance.clone(),
        app.auto_topup.clone(),
        app.openrouter_credit_balance,
    );
    agent_mut
        .prompt
        .slash_controller
        .registry_mut()
        .set_plugins_visible(!app.appearance.disable_plugins);
    app.mark_project_picker_done();
    switch_to_agent(app, agent_id, SwitchCause::Load);
    vec![Effect::LoadSession {
        agent_id,
        session_id,
        session_cwd,
        // Conversation-entry bit; effects OR SessionFlags.chat_mode for meta.
        chat_kind,
    }]
}
/// Load the session selected in the session picker.
pub(in crate::app::dispatch) fn dispatch_pick_session(
    app: &mut AppView,
    index: usize,
) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    if session_picker_external_filter_active(app) {
        let source = get_active_agent(app)
            .and_then(|agent| match agent.active_modal.as_ref() {
                Some(ActiveModal::SessionPicker {
                    entries: Some(entries),
                    ..
                }) => entries.get(index),
                _ => None,
            })
            .or_else(|| {
                app.session_picker_entries
                    .as_ref()
                    .and_then(|entries| entries.get(index))
            })
            .map(|entry| entry.source.as_str());
        if !source.is_some_and(crate::app::foreign_sessions::is_foreign_picker_source) {
            return vec![];
        }
    }
    let mut picker_dismissed = false;
    let entry_data = if let Some(agent) = get_active_agent_mut(app) {
        if let Some(ActiveModal::SessionPicker { entries, .. }) = agent.active_modal.as_mut() {
            let data = entries
                .as_ref()
                .and_then(|s| s.get(index))
                .map(|e| (e.id.clone(), e.source.clone(), e.cwd.clone()));
            agent.active_modal = None;
            picker_dismissed = true;
            data
        } else {
            None
        }
    } else {
        None
    };
    if picker_dismissed {
        invalidate_picker_fetch_on_dismiss(app);
    }
    let (session_id, source, cwd) = match entry_data {
        Some(d) => d,
        None => {
            let sessions = match app.session_picker_entries.take() {
                Some(s) => s,
                None => return vec![],
            };
            if !picker_dismissed {
                invalidate_picker_fetch_on_dismiss(app);
            }
            let entry = match sessions.get(index) {
                Some(e) => e,
                None => return vec![],
            };
            let d = (entry.id.clone(), entry.source.clone(), entry.cwd.clone());
            app.session_picker_loading = false;
            app.session_picker_state.set_query("");
            app.session_picker_state.search_active = false;
            app.session_picker_state.expanded.clear();
            app.session_picker_content_results = None;
            app.session_picker_content_loading = false;
            d
        }
    };
    if let Some(foreign_source) =
        crate::app::foreign_sessions::ForeignPickerSource::from_picker_source(&source)
    {
        let prompt = foreign_source.resume_prompt(&session_id);
        clear_startup_actions(app);
        if !app.session_startup_allowed() {
            app.deferred_startup.session = Some(
                crate::app::session_startup::DeferredSessionStartup::ForeignResume {
                    tool: foreign_source.tool(),
                    native_id: session_id,
                },
            );
            return vec![];
        }
        let mut effects = dispatch_new_session_inner(app, None);
        effects.extend(dispatch(Action::SendPrompt(prompt), app));
        return effects;
    }
    let chat_kind = source == "conversation";
    if chat_kind {
        return dispatch_load_session(app, session_id, None, true);
    }
    let local_cwd = app.cwd.to_string_lossy().to_string();
    if xai_grok_shell::session::resolve_local_session(&session_id, &local_cwd).is_some() {
        return dispatch_load_session(app, session_id, None, false);
    }
    if let Some(original_cwd) = xai_grok_shell::session::resolve_local_session_any_cwd(&session_id)
    {
        return dispatch_load_session(
            app,
            session_id,
            Some(std::path::PathBuf::from(original_cwd)),
            false,
        );
    }
    if source == "remote" || source == "both" {
        if let Some(existing_id) = focus_if_session_already_open(app, &session_id, false) {
            return try_auto_resume_error_idle_on_reopen(app, existing_id);
        }
        app.show_toast("Restoring session from remote...");
        dispatch_load_session_with_restore(app, session_id, cwd)
    } else {
        app.show_toast("Session not found locally");
        vec![]
    }
}
/// Pick a session from the picker and resume it in a new git worktree.
pub(in crate::app::dispatch) fn dispatch_pick_session_in_worktree(
    app: &mut AppView,
    index: usize,
) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    if session_picker_external_filter_active(app) {
        return vec![];
    }
    let is_foreign = get_active_agent(app)
        .and_then(|agent| match agent.active_modal.as_ref() {
            Some(ActiveModal::SessionPicker {
                entries: Some(entries),
                ..
            }) => entries.get(index),
            _ => None,
        })
        .or_else(|| {
            app.session_picker_entries
                .as_ref()
                .and_then(|entries| entries.get(index))
        })
        .is_some_and(|entry| crate::app::foreign_sessions::is_foreign_picker_source(&entry.source));
    if is_foreign {
        app.show_toast("External sessions can't be resumed in a worktree");
        return vec![];
    }
    let mut picker_dismissed = false;
    let entry_data = if let Some(agent) = get_active_agent_mut(app) {
        if let Some(ActiveModal::SessionPicker { entries, .. }) = agent.active_modal.as_mut() {
            let data = entries
                .as_ref()
                .and_then(|s| s.get(index))
                .map(|e| (e.id.clone(), e.source.clone()));
            agent.active_modal = None;
            picker_dismissed = true;
            data
        } else {
            None
        }
    } else {
        None
    };
    if picker_dismissed {
        invalidate_picker_fetch_on_dismiss(app);
    }
    let (session_id, source) = match entry_data {
        Some(d) => d,
        None => {
            let sessions = match app.session_picker_entries.take() {
                Some(s) => s,
                None => return vec![],
            };
            if !picker_dismissed {
                invalidate_picker_fetch_on_dismiss(app);
            }
            let entry = match sessions.get(index) {
                Some(e) => e,
                None => return vec![],
            };
            let d = (entry.id.clone(), entry.source.clone());
            app.session_picker_loading = false;
            app.session_picker_state.set_query("");
            app.session_picker_state.search_active = false;
            app.session_picker_state.expanded.clear();
            d
        }
    };
    if source == "conversation" {
        app.show_toast("Chat conversations can't be resumed in a worktree");
        return vec![];
    }
    dispatch_new_worktree_session(app, Some(session_id), None, None, None, None, None)
}
/// Remove a deleted session identity from the modal session picker and the
/// welcome-screen picker, then re-anchor the selection on a real row.
///
/// Called after [`crate::app::actions::TaskResult::DeleteSessionComplete`] so
/// the just-deleted entry vanishes from the open list without a full refetch.
pub(in crate::app::dispatch) fn remove_session_from_pickers(
    app: &mut AppView,
    source: &str,
    session_id: &str,
) {
    use crate::views::modal::ActiveModal;
    use crate::views::session_picker::build_entry_map;
    app.session_picker_detail_generation += 1;
    if let Some(agent) = get_active_agent_mut(app)
        && let Some(ActiveModal::SessionPicker {
            entries,
            content_results,
            state,
            source_filter,
            content_loading,
            entries_query,
            pending_delete,
            ..
        }) = agent.active_modal.as_mut()
    {
        if pending_delete
            .as_ref()
            .is_some_and(|(pending_source, pending_id, _)| {
                pending_source == source && pending_id == session_id
            })
        {
            *pending_delete = None;
        }
        if let Some(list) = entries.as_mut() {
            list.retain(|entry| entry.source != source || entry.id != session_id);
        }
        if let Some(hits) = content_results.as_mut() {
            hits.retain(|h| h.session_id != session_id);
        }
        let current_repo =
            crate::views::session_picker::repo_name_from_cwd(&agent.session.cwd.to_string_lossy());
        let map = build_entry_map(
            entries.as_deref(),
            content_results.as_deref(),
            crate::views::session_picker::effective_filter_query(
                state.query(),
                entries_query.as_deref(),
            ),
            true,
            *content_loading,
            *source_filter,
            Some(current_repo.as_str()),
        );
        reanchor_grouped_selection(state, &map);
    }
    if let Some(list) = app.session_picker_entries.as_mut() {
        list.retain(|entry| entry.source != source || entry.id != session_id);
    }
    if let Some(hits) = app.session_picker_content_results.as_mut() {
        hits.retain(|h| h.session_id != session_id);
    }
    let welcome_current_repo =
        crate::views::session_picker::repo_name_from_cwd(&app.cwd.to_string_lossy());
    let welcome_map = build_entry_map(
        app.session_picker_entries.as_deref(),
        app.session_picker_content_results.as_deref(),
        crate::views::session_picker::effective_filter_query(
            app.session_picker_state.query(),
            app.session_picker_entries_query.as_deref(),
        ),
        app.session_picker_grouped,
        app.session_picker_content_loading,
        app.session_picker_source_filter,
        Some(welcome_current_repo.as_str()),
    );
    reanchor_grouped_selection(&mut app.session_picker_state, &welcome_map);
}
/// Clamp `state.selected` to a selectable slot in a grouped picker `map`
/// (`Some` = selectable row, `None` = non-selectable header).
pub(in crate::app::dispatch) fn reanchor_grouped_selection<T>(
    state: &mut crate::views::picker::PickerState,
    map: &[Option<T>],
) {
    state.scroll_offset = None;
    if map.is_empty() {
        state.selected = 0;
        return;
    }
    let mut sel = state.selected.min(map.len() - 1);
    while sel > 0 && map[sel].is_none() {
        sel -= 1;
    }
    if map[sel].is_none() {
        sel = map.iter().position(|e| e.is_some()).unwrap_or(0);
    }
    state.selected = sel;
}
/// Trigger a deep content search when the session picker query changes.
///
/// Any query of 2+ chars searches content — title matches never suppress
/// it. Forced (Ctrl+/) searches fire immediately; keystrokes otherwise
/// coalesce through [`Effect::DebounceSessionSearch`], whose expiry runs
/// the search only if its seq is still current. Shorter queries clear the
/// content results.
///
/// Checks the active agent's modal first; if no modal session picker
/// exists, falls back to the welcome-screen picker state.
pub(in crate::app::dispatch) fn dispatch_cycle_session_source_filter(
    app: &mut AppView,
) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    app.session_picker_detail_generation += 1;
    if let Some(agent) = get_active_agent_mut(app)
        && let Some(ActiveModal::SessionPicker {
            state,
            content_results,
            content_loading,
            deep_search_seq,
            source_filter,
            pending_delete,
            ..
        }) = agent.active_modal.as_mut()
    {
        *source_filter = source_filter.next();
        state.selected = 0;
        state.scroll_offset = None;
        if *source_filter == crate::views::session_picker::SourceFilter::External {
            *content_results = None;
            *content_loading = false;
            *deep_search_seq += 1;
            state.expanded.clear();
            *pending_delete = None;
        }
        return vec![];
    }
    app.session_picker_source_filter = app.session_picker_source_filter.next();
    app.session_picker_state.selected = 0;
    app.session_picker_state.scroll_offset = None;
    if app.session_picker_source_filter == crate::views::session_picker::SourceFilter::External {
        app.session_picker_content_results = None;
        app.session_picker_content_loading = false;
        app.session_picker_deep_search_seq += 1;
        app.session_picker_state.expanded.clear();
    }
    vec![]
}
pub(in crate::app::dispatch) fn dispatch_trigger_deep_search(
    app: &mut AppView,
    force: bool,
) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    if app.chat_mode {
        return dispatch_chat_search_refetch(app, force);
    }
    if let Some(agent) = get_active_agent_mut(app)
        && let Some(ActiveModal::SessionPicker {
            state,
            content_results,
            content_loading,
            deep_search_seq,
            source_filter,
            ..
        }) = agent.active_modal.as_mut()
    {
        if *source_filter == crate::views::session_picker::SourceFilter::External {
            *deep_search_seq += 1;
            *content_results = None;
            *content_loading = false;
            state.expanded.clear();
            return vec![];
        }
        let query = state.query().trim().to_string();
        *deep_search_seq += 1;
        let seq = *deep_search_seq;
        if query.len() < 2 {
            *content_results = None;
            *content_loading = false;
            return vec![];
        }
        *content_loading = true;
        if force {
            return vec![Effect::DeepSearchSessions { query, seq }];
        }
        return vec![Effect::DebounceSessionSearch { query, seq }];
    }
    if app.session_picker_source_filter == crate::views::session_picker::SourceFilter::External {
        app.session_picker_deep_search_seq += 1;
        app.session_picker_content_results = None;
        app.session_picker_content_loading = false;
        app.session_picker_state.expanded.clear();
        return vec![];
    }
    let query = app.session_picker_state.query().trim().to_string();
    app.session_picker_deep_search_seq += 1;
    let seq = app.session_picker_deep_search_seq;
    if query.len() < 2 {
        app.session_picker_content_results = None;
        app.session_picker_content_loading = false;
        return vec![];
    }
    app.session_picker_content_loading = true;
    if force {
        vec![Effect::DeepSearchSessions { query, seq }]
    } else {
        vec![Effect::DebounceSessionSearch { query, seq }]
    }
}
/// Chat-mode replacement for local deep search: refetch the session list
/// with the picker query pushed down as `x.ai/session/list` `query`.
/// Keystrokes are coalesced through [`Effect::DebounceSessionSearch`]; a
/// forced search (Ctrl+/) or a cleared query fetches immediately. Every
/// trigger bumps `session_picker_list_seq`, so stale in-flight debounces
/// and fetches are dropped when they complete.
fn dispatch_chat_search_refetch(app: &mut AppView, force: bool) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    let query = if let Some(agent) = get_active_agent(app)
        && let Some(ActiveModal::SessionPicker { state, .. }) = agent.active_modal.as_ref()
    {
        state.query().trim().to_string()
    } else {
        app.session_picker_state.query().trim().to_string()
    };
    app.session_picker_list_seq += 1;
    let seq = app.session_picker_list_seq;
    if query.is_empty() {
        set_chat_search_loading(app, false);
        return vec![Effect::FetchSessionList { query: None, seq }];
    }
    set_chat_search_loading(app, true);
    if force {
        vec![Effect::FetchSessionList {
            query: Some(query),
            seq,
        }]
    } else {
        vec![Effect::DebounceSessionSearch { query, seq }]
    }
}
/// Flip the search in-flight flag on the active picker surface (modal first,
/// welcome fallback — same order as `dispatch_chat_search_refetch`'s query
/// read).
fn set_chat_search_loading(app: &mut AppView, loading: bool) {
    use crate::views::modal::ActiveModal;
    if let Some(agent) = get_active_agent_mut(app)
        && let Some(ActiveModal::SessionPicker {
            content_loading, ..
        }) = agent.active_modal.as_mut()
    {
        *content_loading = loading;
        return;
    }
    app.session_picker_content_loading = loading;
}
fn session_picker_entry_source<'a>(app: &'a AppView, session_id: &str) -> Option<&'a str> {
    use crate::views::modal::ActiveModal;
    if let Some(agent) = get_active_agent(app)
        && let Some(ActiveModal::SessionPicker {
            entries: Some(entries),
            ..
        }) = agent.active_modal.as_ref()
        && let Some(e) = entries.iter().find(|e| e.id == session_id)
    {
        return Some(e.source.as_str());
    }
    app.session_picker_entries
        .as_ref()
        .and_then(|entries| entries.iter().find(|e| e.id == session_id))
        .map(|entry| entry.source.as_str())
}
pub(in crate::app::dispatch) fn session_picker_external_filter_active(app: &AppView) -> bool {
    use crate::views::modal::ActiveModal;
    if let Some(agent) = get_active_agent(app)
        && let Some(ActiveModal::SessionPicker { source_filter, .. }) = agent.active_modal.as_ref()
    {
        return *source_filter == crate::views::session_picker::SourceFilter::External;
    }
    app.session_picker_source_filter == crate::views::session_picker::SourceFilter::External
}
/// Whether the picker row with `session_id` is a backend conversation.
pub(in crate::app::dispatch) fn session_picker_entry_is_conversation(
    app: &AppView,
    session_id: &str,
) -> bool {
    session_picker_entry_source(app, session_id) == Some("conversation")
}
pub(in crate::app::dispatch) fn session_picker_entry_matches(
    app: &AppView,
    source: &str,
    session_id: &str,
) -> bool {
    use crate::views::modal::ActiveModal;
    if let Some(agent) = get_active_agent(app)
        && let Some(ActiveModal::SessionPicker {
            entries,
            content_results,
            ..
        }) = agent.active_modal.as_ref()
    {
        return entries.as_ref().is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.source == source && entry.id == session_id)
        }) || (source == "local"
            && content_results
                .as_ref()
                .is_some_and(|results| results.iter().any(|hit| hit.session_id == session_id)));
    }
    app.session_picker_entries.as_ref().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry.source == source && entry.id == session_id)
    }) || (source == "local"
        && app
            .session_picker_content_results
            .as_ref()
            .is_some_and(|results| results.iter().any(|hit| hit.session_id == session_id)))
}
/// Pick a session from deep content search results.
pub(in crate::app::dispatch) fn dispatch_pick_content_session(
    app: &mut AppView,
    session_id: String,
    cwd: String,
) -> Vec<Effect> {
    if session_picker_external_filter_active(app) {
        return vec![];
    }
    let chat_kind = session_picker_entry_is_conversation(app, &session_id);
    app.session_picker_entries = None;
    app.session_picker_loading = false;
    app.session_picker_state.reset();
    app.session_picker_content_results = None;
    app.session_picker_content_loading = false;
    invalidate_picker_fetch_on_dismiss(app);
    if chat_kind {
        return dispatch_load_session(app, session_id, None, true);
    }
    let local_cwd = app.cwd.to_string_lossy().to_string();
    if xai_grok_shell::session::resolve_local_session(&session_id, &local_cwd).is_some() {
        return dispatch_load_session(app, session_id, None, false);
    }
    if let Some(original_cwd) = xai_grok_shell::session::resolve_local_session_any_cwd(&session_id)
    {
        return dispatch_load_session(
            app,
            session_id,
            Some(std::path::PathBuf::from(original_cwd)),
            false,
        );
    }
    if let Some(existing_id) = focus_if_session_already_open(app, &session_id, false) {
        return try_auto_resume_error_idle_on_reopen(app, existing_id);
    }
    app.show_toast("Restoring session from remote...");
    dispatch_load_session_with_restore(app, session_id, cwd)
}
/// Create a placeholder agent and restore a remote session before loading.
/// Build rows only — conversation rows never reach the restore path.
pub(in crate::app::dispatch) fn dispatch_load_session_with_restore(
    app: &mut AppView,
    session_id: String,
    session_cwd: String,
) -> Vec<Effect> {
    if crate::app::session_startup::chat_mode_refuses_local_build_load(
        app.chat_mode,
        false,
        &session_id,
        &app.cwd,
    ) {
        app.show_toast(crate::app::session_startup::CHAT_MODE_LOCAL_BUILD_REFUSAL);
        return vec![];
    }
    if let Some(existing_id) = focus_if_session_already_open(app, &session_id, false) {
        return try_auto_resume_error_idle_on_reopen(app, existing_id);
    }
    let agent_id = AgentId(app.next_agent_id);
    app.next_agent_id += 1;
    let mut scrollback = ScrollbackState::new();
    scrollback.set_appearance(app.appearance.clone());
    scrollback.push_block(RenderBlock::system(format!(
        "Restoring session {session_id} from remote..."
    )));
    let agent = AgentView::new(
        AgentSession {
            id: agent_id,
            acp_tx: app.acp_tx.clone(),
            session_id: None,
            models: app.models.clone(),
            state: AgentState::Idle,
            tracker: AcpUpdateTracker::new(),
            cwd: app.cwd.clone(),
            is_worktree: false,
            forked_from: None,
            pending_prompts: std::collections::VecDeque::new(),
            next_queue_id: 0,
            yolo_mode: app.default_yolo,
            auto_mode: inherit_auto_mode(app),
            prompt_history: Vec::new(),
            prompt_history_loading: true,
            loading_replay: true,
            restore_degree: None,
            rate_limited: false,
            model_incompatible: false,
            credit_limit_blocked: false,
            free_usage_blocked: false,
            available_commands: app.bootstrap_acp_commands.clone(),
            available_commands_generation: 1,
            available_tools: None,
            model_switch_pending: false,
            user_model_preference: None,
            deferred_model_switch: app.deferred_model_switch_from_cli(),
            bg_tasks: std::collections::BTreeMap::new(),
            bg_tool_call_to_task: std::collections::HashMap::new(),
            scheduled_tasks: std::collections::HashMap::new(),
            in_flight_prompt: None,
            cancel_resume_prompt_text: None,
            compact_held_prompt: None,
            current_prompt_id: None,
            created_via_new: false,
            session_notes: crate::app::agent::SessionNotes::default(),
        },
        scrollback,
    );
    app.agents.insert(agent_id, agent);
    {
        let agent = app.agents.get_mut(&agent_id).unwrap();
        agent.attached_as_viewer = true;
        agent.begin_replay_window();
        agent.prompt.set_compact(app.appearance.prompt.compact);
        agent.prompt.adopt_slash_mru(app.slash_mru.clone());
        agent.prompt.adopt_command_tags(app.command_tags.clone());
        agent
            .prompt
            .set_contextual_hints(app.contextual_hints.undo, app.contextual_hints.plan_mode);
        agent.set_session_recap_available(app.session_recap_available);
        agent.set_voice_mode_available(app.voice_mode_enabled);
        agent.apply_app_scoped_gates(
            app.sharing_enabled,
            app.usage_visible,
            app.chat_mode,
            app.screen_mode,
            &app.active_announcements,
            &app.tier_restricted_commands,
        );
        agent.chat_kind = app.chat_mode;
        agent.apply_credit_balance(
            app.credit_balance.clone(),
            app.auto_topup.clone(),
            app.openrouter_credit_balance,
        );
        agent
            .prompt
            .slash_controller
            .registry_mut()
            .set_plugins_visible(!app.appearance.disable_plugins);
    }
    switch_to_agent(app, agent_id, SwitchCause::Load);
    vec![Effect::RestoreAndLoadSession {
        agent_id,
        session_id,
        session_cwd,
    }]
}
#[allow(clippy::too_many_arguments)]
/// Snapshot mid-work death evidence **before** `finish_turn` / zombie finalize
/// clear running tools, blocking waits, and unfinished subagent rows.
///
/// Counts as interrupted when:
/// - unfinished subagent records (even if the primary already completed —
///   parent success with live children / killall mid-child), **or**
/// - primary did **not** complete in this load's replay **and** any of:
///   - parent/child scrollback still running
///   - tracker mid-turn activity (suppressed wait tools never hit scrollback)
///   - open turn: agent work after last user prompt with no turn-terminal event
///
/// **Replay residue after a completed primary:** during `session/load`, durable
/// `TurnCompleted` sets [`AgentView::last_primary_user_turn_completed_in_replay`]
/// but does **not** call `finish_turn`. Running scrollback entries and tracker
/// `current_agent_msg` / thinking / pending tools therefore often remain until
/// `handle_session_loaded` calls `finish_turn` **after** this snapshot. That
/// residue is **not** mid-work. Treating it as interrupted made every clean
/// completed session look mid-work, so the stale-marker gate never dropped and
/// reopen / `/rebuild` re-fired leftover `canceled_turn_resume.json` (dogfood
/// 2026-08-08 session `019faf9d…`: immediate `prompt.drain` len 14 for
/// `??? [Image #1]` on grok-oss after a completed primary).
///
/// Iso dogfood shape still interrupts: parent parked on suppressed
/// `get_command_or_subagent_output`, all children finished, **no** durable
/// primary terminal (`last_primary_user_turn_completed_in_replay == false`).
pub(crate) fn session_looks_interrupted_mid_work(agent: &AgentView) -> bool {
    // Live children always win — parent may already have a completed terminal.
    if agent.subagent_sessions.values().any(|s| !s.finished) {
        return true;
    }
    // Completed primary in this load: ignore parent stream residue until
    // finish_turn. Only unfinished children (above) keep resume alive.
    if agent.last_primary_user_turn_completed_in_replay {
        return false;
    }
    agent.scrollback.has_running_entries()
        || agent
            .subagent_views
            .values()
            .any(|child| child.scrollback.has_running_entries())
        || agent.session.tracker.has_in_flight_mid_turn_activity()
        || scrollback_has_open_turn_without_terminal(agent)
}

/// After the last resumable user prompt, agent work started but no
/// `TurnCompleted` / `TurnCancelled` / `TurnFailed` was recorded — typical
/// killall / process death mid-turn (PromptResponse never landed).
///
/// **Replay caveat:** during `session/load`, durable primary-user
/// `TurnCompleted` updates are recorded on
/// [`AgentView::last_primary_user_turn_completed_in_replay`] and are **not**
/// pushed as scrollback `SessionEvent` terminals. Without that flag, every
/// clean completed session looks "open" and false-fires auto-resume of the
/// last user prompt (dogfood: re-sent "Still nothing!!! [Image #1]" on reopen).
fn scrollback_has_open_turn_without_terminal(agent: &AgentView) -> bool {
    // Durable terminal for the last primary user turn arrived in this load's
    // replay — not an open/interrupted turn.
    if agent.last_primary_user_turn_completed_in_replay {
        return false;
    }
    let len = agent.scrollback.len();
    let mut last_user_idx: Option<usize> = None;
    for idx in 0..len {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::UserPrompt(b) = &entry.block {
            if b.is_bash || b.is_cron || b.is_interjection {
                continue;
            }
            if b.text.trim().is_empty() {
                continue;
            }
            last_user_idx = Some(idx);
        }
    }
    let Some(user_idx) = last_user_idx else {
        return false;
    };
    let mut saw_agent_work = false;
    let mut saw_terminal = false;
    for idx in (user_idx + 1)..len {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        match &entry.block {
            RenderBlock::SessionEvent(b) if b.event.is_turn_terminal() => {
                saw_terminal = true;
            }
            RenderBlock::AgentMessage(_)
            | RenderBlock::Thinking(_)
            | RenderBlock::ToolCall(_)
            | RenderBlock::Subagent(_)
            | RenderBlock::BgTask(_) => {
                saw_agent_work = true;
            }
            _ => {}
        }
    }
    saw_agent_work && !saw_terminal
}

/// Last non-empty user prompt text suitable for cancel-resume re-queue.
///
/// Skips bash, cron, and mid-turn interjections. Full block text (not first
/// line only) so `/implement …` skill lines re-enter the same send path.
pub(crate) fn last_resumable_user_prompt_text(agent: &AgentView) -> Option<String> {
    let len = agent.scrollback.len();
    for idx in (0..len).rev() {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::UserPrompt(b) = &entry.block {
            if b.is_bash || b.is_cron || b.is_interjection {
                continue;
            }
            let text = b.text.trim();
            if text.is_empty() {
                continue;
            }
            return Some(text.to_string());
        }
    }
    None
}

/// Last primary user turn ended as an **error-class** failure (not clean
/// success, not user cancel).
///
/// Evidence (either):
/// - Durable load replay: [`AgentView::last_primary_user_turn_failed_in_replay`]
///   (`stop_reason == "error"` on primary-user turn_completed), **or**
/// - Scrollback: after the last resumable user prompt, the last turn-terminal
///   `SessionEvent` is [`SessionEvent::TurnFailed`] (live push path; tests).
///
/// Used so reopen / `/rebuild` relaunch auto-resumes work that died on API
/// Internal error / 403 / failed sampling instead of leaving the session idle
/// with only the yellow error lines. Clean `TurnCompleted` success and user
/// `TurnCancelled` without a marker stay non-auto.
pub(crate) fn session_last_turn_ended_in_error(agent: &AgentView) -> bool {
    if agent.last_primary_user_turn_failed_in_replay {
        return true;
    }
    let len = agent.scrollback.len();
    let mut last_user_idx: Option<usize> = None;
    for idx in 0..len {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::UserPrompt(b) = &entry.block {
            if b.is_bash || b.is_cron || b.is_interjection {
                continue;
            }
            if b.text.trim().is_empty() {
                continue;
            }
            last_user_idx = Some(idx);
        }
    }
    let Some(user_idx) = last_user_idx else {
        return false;
    };
    let mut last_terminal: Option<&SessionEvent> = None;
    for idx in (user_idx + 1)..len {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::SessionEvent(b) = &entry.block
            && b.event.is_turn_terminal()
        {
            last_terminal = Some(&b.event);
        }
    }
    matches!(last_terminal, Some(SessionEvent::TurnFailed { .. }))
}

/// When there is **no** `canceled_turn_resume.json` but the loaded session
/// should auto-continue, recover a one-shot resume from history.
///
/// Returns prompt text to re-queue when a last user prompt exists and either:
/// - mid-work interruption evidence (unfinished children / open turn / …), or
/// - the last primary turn ended in **error** (failed sampling / Internal
///   error / 403 as turn failure).
///
/// Does **not** invent work for clean completed turns or for user cancel
/// without a marker.
pub(crate) fn recover_interrupted_turn_from_session(agent: &AgentView) -> Option<String> {
    if !session_looks_interrupted_mid_work(agent) && !session_last_turn_ended_in_error(agent) {
        return None;
    }
    last_resumable_user_prompt_text(agent)
}

/// Auto-resume when the operator "reopens" a session that is **already open**
/// in this process and idle after an **error-class** turn (dogfood 2026-08-09
/// evening: bitmagi / iso / surmount-server sat yellow-403 idle while markers
/// stayed on disk).
///
/// Cold `SessionLoaded` already resumes error terminals (marker path + history
/// path). `focus_if_session_already_open` used to only switch the visible agent
/// and return no effects, so multi-session / picker reopen never re-entered
/// the resume path. That left product-theme processes idle after 403 even when
/// the load-path fix was in the binary.
///
/// Scope: **error terminals only** (not cancel markers, not clean success).
/// Cancel resume stays restart / cold-load only. Does not re-fire a busy turn.
pub(in crate::app::dispatch) fn try_auto_resume_error_idle_on_reopen(
    app: &mut AppView,
    agent_id: AgentId,
) -> Vec<Effect> {
    let resume_enabled = app.current_ui.resume_canceled_turn_on_restart_enabled();
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    if agent.session.loading_replay || agent.session.state.is_busy() {
        return vec![];
    }
    if !resume_enabled {
        return vec![];
    }
    if !session_last_turn_ended_in_error(agent) {
        return vec![];
    }

    let cwd = agent.session.cwd.to_string_lossy().into_owned();
    let sid = agent
        .session
        .session_id
        .as_ref()
        .map(|s| s.0.to_string())
        .unwrap_or_default();
    if sid.is_empty() {
        return vec![];
    }

    let marker =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd, &sid)
            .ok()
            .flatten();
    let prompt_text = marker
        .as_ref()
        .filter(|m| {
            xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
                true,
                Some(m),
            )
        })
        .map(|m| m.prompt_text.clone())
        .or_else(|| last_resumable_user_prompt_text(agent));
    let Some(prompt_text) = prompt_text.filter(|t| !t.trim().is_empty()) else {
        tracing::info!(
            session = %sid,
            "canceled_turn_resume: already-open error idle but no user prompt to re-queue"
        );
        agent.show_toast(
            &xai_grok_shell::session::canceled_turn_resume::interrupted_resume_failed_toast(
                "no user prompt to re-queue",
            ),
        );
        return vec![];
    };
    let prompt_id = marker.as_ref().and_then(|m| m.prompt_id.clone());

    tracing::info!(
        session = %sid,
        prompt_len = prompt_text.len(),
        had_marker = marker.is_some(),
        "canceled_turn_resume: applying error-idle auto-resume on already-open reopen"
    );
    agent.session.enqueue_prompt_front(prompt_text.clone());
    let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd, &sid);

    let drain = force_drain_queue_past_background(agent);
    let turn_started = drain.effects.iter().any(|e| {
        matches!(
            e,
            Effect::SendPrompt { .. }
                | Effect::SendPromptBlocks { .. }
                | Effect::SetModeThenPrompt { .. }
        )
    });
    if turn_started {
        agent.show_toast(xai_grok_shell::session::canceled_turn_resume::auto_resume_toast());
    } else {
        agent.show_toast(
            &xai_grok_shell::session::canceled_turn_resume::interrupted_resume_failed_toast(
                "queue drain did not start a turn",
            ),
        );
        if let Some(built) = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
            &prompt_text,
            prompt_id.as_deref(),
            chrono::Utc::now().to_rfc3339(),
        ) {
            let _ = xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
                &cwd, &sid, &built,
            );
        }
    }
    drain.effects
}

pub(in crate::app::dispatch) fn handle_session_loaded(
    app: &mut AppView,
    agent_id: AgentId,
    session_id: acp::SessionId,
    new_models: Option<acp::SessionModelState>,
    code_restored: bool,
    restore_summary: Option<String>,
    restore_degree: Option<xai_grok_workspace::session::git::RestoreDegree>,
    running_prompt_id: Option<String>,
) -> Vec<Effect> {
    tracing::info!(
        "Session loaded for agent {:?} session {:?}",
        agent_id,
        session_id,
    );
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        if defer_to_open_reload_window(agent, agent_id, "SessionLoaded") {
            return vec![];
        }
        let hydrate_sid = session_id.clone();
        agent.bind_session_id(session_id);
        agent.scrollback.end_batch();
        agent.session.loading_replay = false;
        agent.session.restore_degree = restore_degree;
        // Capture resume evidence **before** finish_turn clears running
        // tools/agent messages and before zombie finalize marks subagents done.
        // Marker path does not need this; history recovery does.
        // Covers mid-work killall **and** last turn ended in error (403 /
        // Internal error) so reopen does not leave the session idle.
        let history_resume_prompt = recover_interrupted_turn_from_session(agent);
        let interrupted_for_log = session_looks_interrupted_mid_work(agent);
        let error_terminal_for_log = session_last_turn_ended_in_error(agent);
        agent.session.finish_turn(&mut agent.scrollback);
        agent.mark_turn_finished();
        if let Some(placeholder_id) = agent.loading_placeholder_id.take() {
            agent.scrollback.remove_entry(placeholder_id);
        }
        if let Some(m) = new_models {
            app.models = Some(m).into();
            agent.session.models = app.models.clone();
        }
        let deferred = crate::app::dispatch::session::lifecycle::apply_deferred_model_switch(
            agent,
            app.cli_effort_token.as_deref(),
        );
        match (code_restored, restore_summary.as_deref()) {
            (true, Some(s)) => {
                agent
                    .scrollback
                    .push_block(RenderBlock::system(format!("\u{2713} Code restored: {s}")));
            }
            (false, Some(s)) => {
                agent.scrollback.push_block(RenderBlock::system(format!(
                    "\u{26A0} Code restore failed: {s}"
                )));
            }
            _ => {}
        }
        if let Some(info) = agent.pending_fork_banner.take() {
            let sid = agent
                .session
                .session_id
                .as_ref()
                .map(|s| s.0.as_ref())
                .unwrap_or("???");
            let banner = build_child_fork_marker(
                sid,
                &info.parent_sid,
                info.worktree,
                crate::views::dashboard::session_switch_hint_command(app.screen_mode.is_minimal()),
            );
            agent.scrollback.push_block(RenderBlock::system(banner));
        }
        let adopting = running_prompt_id
            .as_deref()
            .is_some_and(|pid| agent.should_adopt_running_prompt(pid));
        let preserve = running_prompt_id.as_deref().filter(|_| adopting);
        agent.reset_follow_ups_for_reload_preserving(preserve);
        if adopting && let Some(running_pid) = running_prompt_id {
            agent.adopt_running_prompt(running_pid);
        } else {
            agent.scrollback.finish_all_running();
            for child in agent.subagent_views.values_mut() {
                child.scrollback.finish_all_running();
            }
            // Cold load after killall / process death: subagent rows may still
            // show unfinished=false from replay (SubagentFinished never landed).
            // Those children are dead with this process. Finalize so the local
            // queue is not held forever and cancel-resume can auto-start.
            for info in agent.subagent_sessions.values_mut() {
                if !info.finished {
                    info.finished = true;
                    info.pending_kill = false;
                    info.kill_requested_at = None;
                    info.activity_label = None;
                }
            }
        }
        let mut effects = Vec::new();
        if let Some(directive) = agent.pending_first_prompt.take() {
            agent.session.enqueue_prompt_front(directive);
        }
        // Auto-resume an interrupted/canceled turn on session open (default on).
        // Covers Esc cancel, SIGTERM/killall (eager or signal-path marker), and
        // graceful Quit. Only when not already adopting a live running prompt
        // from the leader. Path must match write side: cwd + session id string
        // (same as `session_id.0`), not a Display that could drift.
        //
        // Product contract: toast + **automatically start** the interrupted
        // prompt as a live turn (SendPrompt path), not composer-only. Clear
        // the one-shot marker when we enqueue, then force-drain so the turn
        // actually runs. Successful drain re-eager-writes for the new active
        // turn (second killall still works). If drain fails, re-persist the
        // marker so a later reopen can retry instead of leaving the session idle.
        let mut resume_toast: Option<String> = None;
        let mut resume_applied = false;
        let mut resume_rewarm: Option<(String, String, String, Option<String>)> = None;
        if !adopting {
            let resume_enabled = app.current_ui.resume_canceled_turn_on_restart_enabled();
            let cwd = agent.session.cwd.to_string_lossy().into_owned();
            // Prefer bound session id; hydrate_sid is the load request id and
            // matches after `bind_session_id` above.
            let sid = agent
                .session
                .session_id
                .as_ref()
                .map(|s| s.0.to_string())
                .unwrap_or_else(|| hydrate_sid.0.to_string());
            let marker = xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(
                &cwd, &sid,
            )
            .ok()
            .flatten();
            match marker.as_ref() {
                // A) Marker path when present (Esc / SIGTERM / eager turn /
                // mid-`/rebuild` cancel / kept after error). Refuse **stale**
                // markers after a primary user turn finished **successfully**
                // in this load's replay (no mid-work, no error terminal):
                // eager write + missed clear left `canceled_turn_resume.json`
                // that re-fired clean prompts on every reopen/`/rebuild`
                // (dogfood: "??? [Image #1]" and "Still nothing!!!" loops).
                // Error-class terminals keep the marker and auto-resume
                // (operator: last thing was an error → continue that work).
                Some(marker)
                    if xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
                        resume_enabled,
                        Some(marker),
                    ) =>
                {
                    let stale_after_successful_complete = agent
                        .last_primary_user_turn_completed_in_replay
                        && !interrupted_for_log
                        && !error_terminal_for_log;
                    if stale_after_successful_complete {
                        tracing::info!(
                            session = %sid,
                            prompt_len = marker.prompt_text.len(),
                            "canceled_turn_resume: dropping stale marker after successful primary turn (no mid-work, no error)"
                        );
                        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
                            &cwd, &sid,
                        );
                    } else {
                        let prompt_text = marker.prompt_text.clone();
                        let prompt_id = marker.prompt_id.clone();
                        tracing::info!(
                            session = %sid,
                            prompt_len = prompt_text.len(),
                            error_terminal = error_terminal_for_log,
                            "canceled_turn_resume: applying marker on session load (auto-start)"
                        );
                        agent.session.enqueue_prompt_front(prompt_text.clone());
                        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
                            &cwd, &sid,
                        );
                        resume_toast = Some(
                            xai_grok_shell::session::canceled_turn_resume::auto_resume_toast()
                                .to_string(),
                        );
                        resume_applied = true;
                        resume_rewarm = Some((cwd, sid, prompt_text, prompt_id));
                    }
                }
                Some(_) if !resume_enabled => {
                    tracing::info!(
                        session = %sid,
                        "canceled_turn_resume: marker present but resume_canceled_turn_on_restart is off"
                    );
                }
                Some(marker) => {
                    tracing::info!(
                        session = %sid,
                        prompt_len = marker.prompt_text.len(),
                        "canceled_turn_resume: marker present but not auto-resume eligible"
                    );
                }
                // B) No marker: recover from unfinished children / running
                // scrollback / suppressed wait tools / open turn / **error
                // terminal** + last user prompt (killall without marker file,
                // or turn failed after marker was cleared).
                None if resume_enabled => {
                    if let Some(prompt_text) = history_resume_prompt {
                        tracing::info!(
                            session = %sid,
                            prompt_len = prompt_text.len(),
                            interrupted = interrupted_for_log,
                            error_terminal = error_terminal_for_log,
                            "canceled_turn_resume: recovering turn from session history (no marker; mid-work or error)"
                        );
                        // Write a marker as we re-queue so a second SIGTERM mid-drain
                        // still has durable resume text (same shape as Esc/killall).
                        if let Some(built) =
                            xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
                                &prompt_text,
                                None,
                                chrono::Utc::now().to_rfc3339(),
                            )
                        {
                            let _ = xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
                                &cwd, &sid, &built,
                            );
                        }
                        agent.session.enqueue_prompt_front(prompt_text.clone());
                        let _ =
                            xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
                                &cwd, &sid,
                            );
                        resume_toast = Some(
                            xai_grok_shell::session::canceled_turn_resume::auto_resume_interrupted_toast(
                            )
                            .to_string(),
                        );
                        resume_applied = true;
                        resume_rewarm = Some((cwd, sid, prompt_text, None));
                    } else if interrupted_for_log || error_terminal_for_log {
                        tracing::info!(
                            session = %sid,
                            interrupted = interrupted_for_log,
                            error_terminal = error_terminal_for_log,
                            "canceled_turn_resume: resume evidence on load but no user prompt to re-queue"
                        );
                        resume_toast = Some(
                            xai_grok_shell::session::canceled_turn_resume::interrupted_resume_failed_toast(
                                "no user prompt to re-queue",
                            ),
                        );
                    }
                }
                None if interrupted_for_log || error_terminal_for_log => {
                    tracing::info!(
                        session = %sid,
                        interrupted = interrupted_for_log,
                        error_terminal = error_terminal_for_log,
                        "canceled_turn_resume: resume evidence on load but resume_canceled_turn_on_restart is off"
                    );
                    resume_toast = Some(
                        xai_grok_shell::session::canceled_turn_resume::interrupted_resume_failed_toast(
                            "resume on restart is off in settings",
                        ),
                    );
                }
                None => {}
            }
        }
        // Cancel-resume must start a turn even if residual background holds
        // remain (defense in depth after zombie finalize above). Normal load
        // keeps the standard drain so live-children hold still applies when
        // we did not just re-queue a killall interrupt.
        let drain = if resume_applied {
            force_drain_queue_past_background(agent)
        } else {
            maybe_drain_queue(agent)
        };
        let page_flip_entry = drain.page_flip_entry;
        let turn_started = drain.effects.iter().any(|e| {
            matches!(
                e,
                Effect::SendPrompt { .. }
                    | Effect::SendPromptBlocks { .. }
                    | Effect::SetModeThenPrompt { .. }
            )
        });
        if resume_applied {
            let resume_sid = agent
                .session
                .session_id
                .as_ref()
                .map(|s| s.0.to_string())
                .unwrap_or_else(|| hydrate_sid.0.to_string());
            if turn_started {
                tracing::info!(
                    session = %resume_sid,
                    "canceled_turn_resume: session load drain started SendPrompt (marker or history)"
                );
            } else {
                tracing::info!(
                    session = %resume_sid,
                    "canceled_turn_resume: session load drain blocked (will re-warm marker)"
                );
                // Loud dogfood: enqueue happened but turn did not start.
                resume_toast = Some(
                    xai_grok_shell::session::canceled_turn_resume::interrupted_resume_failed_toast(
                        "queue drain did not start a turn",
                    ),
                );
            }
        }
        effects.extend(drain.effects);
        if let Some((cwd, sid, prompt_text, prompt_id)) = resume_rewarm
            && !turn_started
        {
            tracing::warn!(
                session = %sid,
                "cancel-resume enqueued on session load but drain did not start a turn; \
                 re-writing canceled_turn_resume.json for a later reopen"
            );
            if let Some(marker) =
                xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
                    &prompt_text,
                    prompt_id.as_deref(),
                    chrono::Utc::now().to_rfc3339(),
                )
            {
                let _ = xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
                    &cwd, &sid, &marker,
                );
            }
        }
        if let Some(toast) = resume_toast {
            agent.show_toast(&toast);
        }
        let cwd = agent.session.cwd.clone();
        effects.push(Effect::HydrateSessionTitleFromDisk {
            agent_id,
            session_id: hydrate_sid.clone(),
            cwd: cwd.clone(),
        });
        agent.session.prompt_history_loading = true;
        effects.push(Effect::FetchPromptHistory {
            agent_id,
            cwd,
            session_id: hydrate_sid.to_string(),
        });
        effects.push(Effect::FetchSessionAgentName {
            agent_id,
            session_id: hydrate_sid.clone(),
        });
        if app.plugin_cta_enabled {
            effects.push(Effect::FetchPluginCtaCatalog {
                agent_id,
                session_id: hydrate_sid.clone(),
            });
        }
        effects.push(Effect::FetchBilling {
            agent_id,
            silent: true,
        });
        if let Some((model_id, effort)) = deferred {
            agent.session.model_switch_pending = true;
            effects.push(Effect::SwitchModel {
                agent_id,
                session_id: hydrate_sid.clone(),
                model_id,
                effort,
                prev_model_id: None,
            });
        }
        if std::mem::take(&mut agent.pending_extensions_fetch)
            && let Some(modal) = agent.extensions_modal.as_mut()
        {
            effects.extend(extensions_modal_tab_fetches(
                modal,
                agent_id,
                hydrate_sid.clone(),
            ));
        }
        effects.push(Effect::RegisterActiveSession {
            session_id: hydrate_sid,
            cwd: agent.session.cwd.display().to_string(),
        });
        notify_session_ready(&app.notification_service, agent);
        crate::memory_release::release_retained_memory_with("session-load-replay");
        note_peek_page_flip(app, agent_id, page_flip_entry);
        return effects;
    }
    vec![]
}
pub(in crate::app::dispatch) fn handle_session_load_failed(
    app: &mut AppView,
    agent_id: AgentId,
    session_id: acp::SessionId,
    error: String,
) -> Vec<Effect> {
    tracing::error!(agent = ?agent_id, session = ?session_id, error = %error, "Session load failed");
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        if defer_to_open_reload_window(agent, agent_id, "SessionLoadFailed") {
            return vec![];
        }
        agent.pending_extensions_fetch = false;
        agent.session.prompt_history_loading = false;
        agent.session.finish_command();
        agent.mark_turn_finished();
        agent.scrollback.end_batch();
        agent.session.loading_replay = false;
        agent.pending_first_prompt = None;
        agent.pending_fork_banner = None;
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::TurnFailed {
                error: format!("Couldn't load session: {error}"),
                elapsed: None,
            }));
    }
    vec![]
}
pub(in crate::app::dispatch) fn handle_session_search_debounce_expired(
    app: &mut AppView,
    query: String,
    seq: u64,
) -> Vec<Effect> {
    if app.chat_mode {
        if seq != app.session_picker_list_seq {
            return vec![];
        }
        return vec![Effect::FetchSessionList {
            query: (!query.is_empty()).then_some(query),
            seq,
        }];
    }
    if live_deep_search_seq(app) != Some(seq) {
        return vec![];
    }
    vec![Effect::DeepSearchSessions { query, seq }]
}
/// The deep-search seq of the surface that can still consume results: an
/// open modal SessionPicker (its own counter), else the welcome-screen
/// picker only while the welcome view is showing. `None` when neither
/// surface is live — dismissing a modal bumps the WELCOME counter, which
/// can collide with (not invalidate) a modal-armed seq, so those expiries
/// are dropped by liveness rather than counter arithmetic.
fn live_deep_search_seq(app: &AppView) -> Option<u64> {
    use crate::views::modal::ActiveModal;
    if let Some(agent) = get_active_agent(app)
        && let Some(ActiveModal::SessionPicker {
            deep_search_seq, ..
        }) = agent.active_modal.as_ref()
    {
        return Some(*deep_search_seq);
    }
    matches!(app.active_view, crate::app::app_view::ActiveView::Welcome)
        .then_some(app.session_picker_deep_search_seq)
}
pub(in crate::app::dispatch) fn handle_card_detail_loaded(
    app: &mut AppView,
    source: String,
    session_id: String,
    generation: u64,
    detail: crate::app::app_view::CardDetail,
) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    if generation != app.session_picker_detail_generation
        || crate::app::foreign_sessions::is_foreign_picker_source(&source)
    {
        return vec![];
    }
    if let Some(agent) = get_active_agent_mut(app)
        && let Some(ActiveModal::SessionPicker { entries, .. }) = agent.active_modal.as_mut()
    {
        if let Some(entry) = entries.as_mut().and_then(|sessions| {
            sessions.iter_mut().find(|entry| {
                entry.source == source
                    && entry.id == session_id
                    && !crate::app::foreign_sessions::is_foreign_picker_source(&entry.source)
            })
        }) {
            entry.card_detail = Some(detail);
        }
        return vec![];
    }
    if let Some(ref mut sessions) = app.session_picker_entries
        && let Some(entry) = sessions.iter_mut().find(|entry| {
            entry.source == source
                && entry.id == session_id
                && !crate::app::foreign_sessions::is_foreign_picker_source(&entry.source)
        })
    {
        entry.card_detail = Some(detail);
    }
    vec![]
}
pub(in crate::app::dispatch) fn handle_session_restored(
    app: &mut AppView,
    agent_id: AgentId,
    local_session_id: String,
) -> Vec<Effect> {
    if crate::app::session_startup::chat_mode_refuses_local_build_load(
        app.chat_mode,
        false,
        &local_session_id,
        &app.cwd,
    ) {
        refuse_chat_mode_build_agent(app, agent_id);
        return vec![];
    }
    let sid = clear_stale_session_id(app, &local_session_id);
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        supersede_open_reload_window(agent, agent_id, "SessionRestored");
        agent.bind_session_id(sid);
        agent.chat_kind = app.chat_mode;
        agent.apply_credit_balance(
            app.credit_balance.clone(),
            app.auto_topup.clone(),
            app.openrouter_credit_balance,
        );
        agent.scrollback.push_block(RenderBlock::system(format!(
            "Session restored. Loading {local_session_id}..."
        )));
    }
    let cwd = app.cwd.clone();
    vec![Effect::LoadSession {
        agent_id,
        session_id: local_session_id,
        session_cwd: Some(cwd),
        // Never a conversation entry (effects OR SessionFlags.chat_mode).
        chat_kind: false,
    }]
}
pub(in crate::app::dispatch) fn handle_session_restore_failed(
    app: &mut AppView,
    agent_id: AgentId,
    error: String,
) -> Vec<Effect> {
    tracing::error!(agent = ?agent_id, error = %error, "Session restore failed");
    if let Some(agent) = app.agents.get_mut(&agent_id) {
        if defer_to_open_reload_window(agent, agent_id, "SessionRestoreFailed") {
            return vec![];
        }
        agent.pending_extensions_fetch = false;
        agent.session.loading_replay = false;
        agent.session.prompt_history_loading = false;
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::TurnFailed {
                error: format!("Couldn't restore session: {error}"),
                elapsed: None,
            }));
    }
    vec![]
}
pub(in crate::app::dispatch) fn handle_deep_search_results(
    app: &mut AppView,
    results: Vec<xai_grok_shell::extensions::session_search::SearchSessionHit>,
    seq: u64,
) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    if let Some(agent) = get_active_agent_mut(app)
        && let Some(ActiveModal::SessionPicker {
            content_results,
            content_loading,
            deep_search_seq,
            source_filter,
            ..
        }) = agent.active_modal.as_mut()
    {
        if seq == *deep_search_seq
            && *source_filter != crate::views::session_picker::SourceFilter::External
        {
            *content_results = Some(results);
            *content_loading = false;
        }
        return vec![];
    }
    if seq == app.session_picker_deep_search_seq
        && app.session_picker_source_filter != crate::views::session_picker::SourceFilter::External
    {
        app.session_picker_content_results = Some(results);
        app.session_picker_content_loading = false;
    }
    vec![]
}
pub(in crate::app::dispatch) fn dispatch_show_session_picker(app: &mut AppView) -> Vec<Effect> {
    use crate::views::modal::ActiveModal;
    with_active_agent(app, |agent| {
        agent.active_modal = Some(ActiveModal::SessionPicker {
            state: crate::views::picker::PickerState::default(),
            entries: None,
            loading: true,
            lanes: Default::default(),
            previous_palette: None,
            window: crate::views::modal_window::ModalWindowState::new(),
            content_results: None,
            content_loading: false,
            deep_search_seq: 0,
            entries_query: None,
            source_filter: crate::views::session_picker::SourceFilter::default(),
            pending_delete: None,
        });
    });
    dispatch_fetch_session_list(app)
}
/// The picker (modal `/resume` or welcome screen) was dismissed without a
/// pick. Its own fields die with it, but a still-current in-flight
/// list/search fetch would fall through to the welcome picker fields in
/// `handle_session_list_loaded`, stamping them with a query the welcome
/// search box never had — or repopulating a picker the user just closed.
/// Invalidate it (same seq idiom as `dispatch_fetch_session_list`).
pub(in crate::app::dispatch) fn dispatch_session_picker_closed(app: &mut AppView) -> Vec<Effect> {
    invalidate_picker_fetch_on_dismiss(app);
    vec![]
}
/// Fetch invalidation shared by EVERY picker-dismissal path:
/// modal Esc/mouse close, modal and welcome picks (all variants), and the
/// welcome-screen Esc. Only chat mode can have a query-stamped search in
/// flight; a Build-mode MODAL close must NOT bump — only the plain list
/// fetch exists there and its response lands on the hidden welcome fields
/// (pre-existing last-write-wins behavior). A WELCOME dismissal must bump
/// and drop the loading flag: the welcome view survives the close, so a
/// still-loading flag holds `show_picker` in a spinner limbo that ignores
/// input until the late response lands and resurrects the picker.
fn invalidate_picker_fetch_on_dismiss(app: &mut AppView) {
    invalidate_foreign_picker(app);
    let welcome_dismissal = matches!(app.active_view, crate::app::app_view::ActiveView::Welcome);
    if app.chat_mode || welcome_dismissal {
        app.session_picker_list_seq += 1;
    }
    if welcome_dismissal {
        app.session_picker_loading = false;
    }
    app.session_picker_deep_search_seq += 1;
    app.session_picker_content_loading = false;
}
pub(in crate::app::dispatch) fn dispatch_pick_content_session_in_worktree(
    app: &mut AppView,
    session_id: String,
    _: String,
) -> Vec<Effect> {
    if session_picker_external_filter_active(app) {
        return vec![];
    }
    if session_picker_entry_is_conversation(app, &session_id) {
        app.show_toast("Chat conversations can't be resumed in a worktree");
        return vec![];
    }
    app.session_picker_entries = None;
    app.session_picker_loading = false;
    app.session_picker_state.reset();
    app.session_picker_content_results = None;
    app.session_picker_content_loading = false;
    if let Some(agent) = get_active_agent_mut(app) {
        agent.active_modal = None;
    }
    invalidate_picker_fetch_on_dismiss(app);
    dispatch_new_worktree_session(app, Some(session_id), None, None, None, None, None)
}
