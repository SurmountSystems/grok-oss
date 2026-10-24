//! Login, logout, account switching, and auth-code submission dispatchers.

use super::ctx::{refuse_withheld, restore_auth_return_view, show_welcome};
use super::queue::{maybe_drain_queue, note_peek_page_flip};
use super::router::dispatch;
use super::session::lifecycle::{clear_startup_actions, drain_startup_actions};
use crate::app::actions::{Action, Effect};
use crate::app::agent::AgentId;
use crate::app::agent_view::AgentView;
use crate::app::app_view::{ActiveView, AppView, AuthMode, AuthState};
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::SessionEvent;
use agent_client_protocol as acp;
use xai_grok_config::Capability;

/// `/logout`: ask the shell to clear auth, then return to the login screen.
pub(super) fn dispatch_logout(app: &mut AppView) -> Vec<Effect> {
    if refuse_withheld(app, Capability::AccountLogin) {
        return vec![];
    }
    app.logout_pending = true;
    vec![Effect::Logout]
}

/// Ensure `login_method_id` is populated from stored auth methods.
/// On the eager-auth path (cached token) `login_method_id` is never set, because the user skipped the login screen.
/// Does **not** invent `grok.com` when no interactive method is advertised (`preferred_method=api_key` with no key leaves `auth_methods` empty).
pub(super) fn ensure_login_method(app: &mut AppView) {
    if app.login_method_id.is_some() {
        return;
    }
    let (label, method_id, start_mode) =
        crate::acp::find_interactive_login_method(&app.auth_methods);
    if let Some(id) = method_id {
        app.login_label = label;
        app.login_method_id = Some(id);
        app.auth_start_mode = match start_mode {
            crate::acp::AuthStartMode::Pending => AuthMode::Pending,
            crate::acp::AuthStartMode::Command => AuthMode::Command,
        };
    }
    // No interactive method: leave login_method_id unset (fail-closed).
}

/// Error when no interactive login method is available (empty auth_methods, e.g. `preferred_method=api_key` with no credentials).
/// When the list is empty, prefer the shell's `PREFERRED_API_KEY_UNAVAILABLE` copy.
fn no_login_method_error(app: &AppView) -> String {
    if app.auth_methods.is_empty() {
        xai_grok_shell::agent::auth_method::PREFERRED_API_KEY_UNAVAILABLE.to_string()
    } else {
        "No login method available".to_string()
    }
}

/// Abort any in-flight Authenticate/SwitchAccount task *and* its URL poll (single-flight).
/// A new login must not stack device-code mints or let a stale poll steal the successor's URL.
/// No-op when not authenticating or when the abort handles have not been installed yet.
fn abort_prior_auth(app: &mut AppView) {
    if let AuthState::Authenticating {
        handle,
        request_seq,
        ..
    } = &mut app.auth_state
        && let Some(h) = handle.take()
    {
        tracing::debug!(
            request_seq,
            "aborting prior in-flight auth task for single-flight"
        );
        h.abort();
    }
    if let Some((seq, h)) = app.auth_url_poll_handle.take() {
        tracing::debug!(
            request_seq = seq,
            "aborting prior auth URL poll for single-flight"
        );
        h.abort();
    }
}

/// Log out, then start a new login flow in a single sequential task.
pub(super) fn dispatch_switch_account(app: &mut AppView) -> Vec<Effect> {
    if refuse_withheld(app, Capability::AccountLogin) {
        return vec![];
    }
    ensure_login_method(app);

    let Some(method_id) = app.login_method_id.clone() else {
        app.auth_state = AuthState::Pending {
            error: Some(no_login_method_error(app)),
        };
        return vec![];
    };

    abort_prior_auth(app);

    let request_seq = app.next_auth_request_seq;
    app.next_auth_request_seq += 1;
    app.auth_code_input.reset();
    app.auth_state = AuthState::Authenticating {
        request_seq,
        handle: None,
        auth_url: None,
        mode: app.auth_start_mode,
    };

    vec![
        Effect::SwitchAccount {
            request_seq,
            method_id,
            use_oauth: app.auth_use_oauth,
        },
        Effect::PollAuthUrl { request_seq },
    ]
}

/// Scan the trailing run of session-event / system blocks for a [`SessionEvent::ReAuthRequired`] prompt.
/// Used by the `PromptResponse` handler to suppress the redundant "Turn failed" block after a 401.
/// The re-auth prompt is pushed by the `RetryState` handler, which runs first.
pub(super) fn scrollback_has_recent_reauth_prompt(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    trailing_session_events(scrollback).any(|(_, ev)| matches!(ev, SessionEvent::ReAuthRequired))
}

/// True if this scrollback already has the `/login` expiry sentence, including when a later nested-run summary sits after it.
/// Trailing-only matching would stack a copy on every failed nested run in the same expired session.
pub(crate) fn scrollback_has_reauth_prompt(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    (0..scrollback.len()).any(|idx| {
        matches!(
            scrollback.entry(idx).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev)) if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    })
}

/// True if the trailing run of session/system blocks has a terminal context-overflow block ([`SessionEvent::ContextTooLarge`] or `CompactionFailed`).
/// Lets `PromptResponse` suppress the redundant `TurnFailed`, mirroring reauth.
pub(super) fn scrollback_has_recent_context_too_large(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    trailing_session_events(scrollback).any(|(_, ev)| {
        matches!(
            ev,
            SessionEvent::ContextTooLarge { .. } | SessionEvent::CompactionFailed { .. }
        )
    })
}

pub(crate) fn scrollback_has_recent_disk_full(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    trailing_session_events(scrollback).any(|(_, ev)| matches!(ev, SessionEvent::DiskFull))
}

/// True if the trailing run already has a dedicated terminal error banner that replaces `TurnFailed`.
/// `CompactionFailed` is deliberately excluded: it can appear mid-turn.
/// On the reconcile/viewer paths a stale one must not swallow the only banner an unrelated error gets.
pub(in crate::app) fn scrollback_has_recent_error_banner(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    trailing_session_events(scrollback).any(|(_, ev)| {
        matches!(
            ev,
            SessionEvent::ReAuthRequired
                | SessionEvent::ContextTooLarge { .. }
                | SessionEvent::DiskFull
                | SessionEvent::RequestFailed { .. }
        )
    })
}

/// True if the trailing run already has a formatted [`SessionEvent::RequestFailed`] banner.
/// Lets `PromptResponse` skip the redundant `TurnFailed`.
/// Deliberately does not match `RetryFailed`.
pub(super) fn scrollback_has_recent_request_failed(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    trailing_session_events(scrollback)
        .any(|(_, ev)| matches!(ev, SessionEvent::RequestFailed { .. }))
}

/// The trailing run of session events, newest first: yields `(index, event)` for each session-event block at the tail of the scrollback.
/// It skips interleaved system messages and stops at the first substantive block.
/// Banners for the finishing turn live in this run; they were pushed just before its `PromptResponse` arrived.
pub(super) fn trailing_session_events(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> impl Iterator<Item = (usize, &SessionEvent)> {
    use crate::scrollback::block::RenderBlock;
    (0..scrollback.len())
        .rev()
        .map(|idx| (idx, scrollback.entry(idx).map(|e| &e.block)))
        .take_while(|(_, block)| {
            matches!(
                block,
                Some(RenderBlock::SessionEvent(_) | RenderBlock::System(_))
            )
        })
        .filter_map(|(idx, block)| match block {
            Some(RenderBlock::SessionEvent(ev)) => Some((idx, &ev.event)),
            _ => None,
        })
}

/// Strip the trailing run of auth-error blocks (the `ReAuthRequired` prompt plus any stale `RetryFailed` / `TurnFailed`) from an agent's scrollback.
/// Also removes every `ReAuthRequired` banner that is no longer trailing, so a later real expiry can show after `/login`.
/// Called after a successful mid-session re-auth so the prompt disappears once the user returns to the session.
/// Mirrors how the credit-limit upsell strips its stale blocks.
pub(super) fn strip_trailing_auth_error_blocks(agent: &mut AgentView) {
    let to_remove: Vec<usize> = trailing_session_events(&agent.scrollback)
        .filter(|(_, ev)| {
            matches!(
                ev,
                SessionEvent::ReAuthRequired
                    | SessionEvent::RequestFailed { .. }
                    | SessionEvent::RetryFailed { .. }
                    | SessionEvent::TurnFailed { .. }
            )
        })
        .map(|(idx, _)| idx)
        .collect();
    for idx in to_remove {
        agent.scrollback.remove_from(idx);
    }
    strip_reauth_required_blocks(agent);
    strip_login_advice_system_blocks(agent);
}

/// Remove every [`SessionEvent::ReAuthRequired`] banner, including those that are no longer trailing because a later nested-run summary was painted after them.
/// Login and cancel-login use this so a later real expiry can show again.
fn strip_reauth_required_blocks(agent: &mut AgentView) {
    let ids: Vec<_> = (0..agent.scrollback.len())
        .filter_map(|idx| {
            let entry = agent.scrollback.entry(idx)?;
            match &entry.block {
                RenderBlock::SessionEvent(ev)
                    if matches!(ev.event, SessionEvent::ReAuthRequired) =>
                {
                    Some(entry.id)
                }
                _ => None,
            }
        })
        .collect();
    for id in ids {
        agent.scrollback.remove_entry(id);
    }
}

/// After the Operator authenticates, never leave "Please run grok-oss /login"
/// as the only thing that happened. Agents never tell the Operator to /login.
fn strip_login_advice_system_blocks(agent: &mut AgentView) {
    let ids: Vec<_> = (0..agent.scrollback.len())
        .filter_map(|idx| {
            let entry = agent.scrollback.entry(idx)?;
            match &entry.block {
                RenderBlock::System(block)
                    if block.text.contains("grok-oss /login")
                        || block.text.contains("Please run grok-oss /login")
                        || block.text.contains("Run /login to re-authenticate") =>
                {
                    Some(entry.id)
                }
                _ => None,
            }
        })
        .collect();
    for id in ids {
        agent.scrollback.remove_entry(id);
    }
}

fn take_prompt_to_retry_after_auth(
    agent: &mut AgentView,
) -> Option<crate::app::agent::InFlightPrompt> {
    agent
        .reauth_stashed_prompt
        .take()
        .or_else(|| agent.session.compact_held_prompt.take())
}

fn recovered_parent_jwt(agent: &AgentView) -> Option<String> {
    agent
        .parent_sampler_jwt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `/goal resume` is the shell builtin that calls existing `SessionActor::resume_goal`.
/// AuthComplete must send this instead of the original `/goal <objective>` so login
/// does not Goal created, drop tracker progress, or leave the interrupted goal paused.
const GOAL_RESUME_AFTER_LOGIN_AUTH: &str = "/goal resume";

/// In-flight or error-interrupted goals resume after login auth. User pause,
/// back-off, blocked, complete, budget-limited, and failed stay put.
fn goal_should_resume_after_login_auth(agent: &AgentView) -> bool {
    use crate::app::agent::GoalDisplayStatus;
    agent.goal_state.as_ref().is_some_and(|goal| {
        matches!(
            goal.status,
            GoalDisplayStatus::Active
                | GoalDisplayStatus::InfraPaused
                | GoalDisplayStatus::Interrupted
        )
    })
}

fn synthetic_goal_resume_prompt() -> crate::app::agent::InFlightPrompt {
    crate::app::agent::InFlightPrompt {
        text: GOAL_RESUME_AFTER_LOGIN_AUTH.to_string(),
        images: Vec::new(),
        scrollback_entry: crate::scrollback::EntryId::new(0),
        combined_scrollback_entries: Vec::new(),
        chip_elements: Vec::new(),
    }
}

/// Waiting for the model after login: `maybe_drain_queue` is blocked while
/// TurnRunning. UnstickResendPrompt POSTs immediately with recoveredBearer.
/// An interrupted `/goal` POSTs `/goal resume` so the shell calls `resume_goal`.
fn unstick_waiting_parent_after_auth(
    agent: &mut AgentView,
    prompt: crate::app::agent::InFlightPrompt,
) -> Option<Effect> {
    let session_id = agent.session.session_id.clone()?;
    let prompt_id = uuid::Uuid::new_v4().to_string();
    agent.session.current_prompt_id = Some(prompt_id.clone());
    let text = if goal_should_resume_after_login_auth(agent) {
        GOAL_RESUME_AFTER_LOGIN_AUTH.to_string()
    } else {
        prompt.text
    };
    Some(Effect::UnstickResendPrompt {
        agent_id: agent.session.id,
        session_id,
        text,
        prompt_id,
        images: Vec::new(),
        images_dir: None,
        recovered_bearer: true,
        recovered_bearer_jwt: recovered_parent_jwt(agent),
    })
}

/// After `/login` succeeds, resubmit the prompt that 401'd. Do this even when
/// the login did not stash a return view (dashboard or another-process login).
/// Parent already TurnRunning (Waiting for the model) must still POST immediately.
/// Interrupted `/goal` (Active / InfraPaused / Interrupted) always UnstickResendPrompt
/// `/goal resume` with the live login JWT so `resume_goal` runs.
fn enqueue_prompts_after_auth(app: &mut AppView) -> Vec<Effect> {
    let mut retry_effects = Vec::new();
    let mut page_flips = Vec::new();
    for agent in app.agents.values_mut() {
        agent.nested_auth_resume_retries = 0;
        strip_trailing_auth_error_blocks(agent);
        let was_idle = agent.session.state.is_idle();
        let resume_goal = goal_should_resume_after_login_auth(agent);
        let mut parent_retry = take_prompt_to_retry_after_auth(agent);
        if resume_goal && parent_retry.is_none() {
            parent_retry = agent
                .session
                .in_flight_prompt
                .clone()
                .or_else(|| Some(synthetic_goal_resume_prompt()));
        }
        let had_parent_retry = parent_retry.is_some();
        if let Some(prompt) = parent_retry {
            agent.scrollback.push_block(RenderBlock::system(
                "Re-authenticated. Retrying\u{2026}".to_string(),
            ));
            if resume_goal {
                if let Some(effect) = unstick_waiting_parent_after_auth(agent, prompt) {
                    retry_effects.push(effect);
                }
            } else if was_idle {
                agent.session.enqueue_in_flight_prompt_front(prompt);
                let drain = maybe_drain_queue(agent, &mut app.pending_image_notices);
                retry_effects.extend(drain.effects);
                page_flips.push((agent.session.id, drain.page_flip_entry));
            } else if let Some(effect) = unstick_waiting_parent_after_auth(agent, prompt) {
                retry_effects.push(effect);
            }
        }
        let nested = enqueue_nested_resume_after_auth(agent);
        // Waiting for the model after login: a nested 401 row must not skip the
        // already-typed parent in_flight POST. Prior land only unstuck that
        // prompt when nested resume was empty, so chrome kept ticking while
        // nested recoveredBearer still sent a stale Bearer.
        if !had_parent_retry
            && !was_idle
            && let Some(prompt) = agent.session.in_flight_prompt.clone()
            && let Some(effect) = unstick_waiting_parent_after_auth(agent, prompt)
        {
            retry_effects.push(effect);
        }
        retry_effects.extend(nested);
    }
    for (id, page_flip_entry) in page_flips {
        note_peek_page_flip(app, id, page_flip_entry);
    }
    retry_effects
}

/// Nested L2s that 401'd stay stuck unless AuthComplete resumes them.
/// Completed and cancelled rows stay put. L3s stay with their L2.
/// Resume the L2 that 401'd (`attempt.error` missing-auth), finished or still
/// running. Do not resume every finished depth-1 row.
fn nested_l2_needs_auth_resume(info: &crate::app::subagent::SubagentInfo) -> bool {
    if info.attempt.depth.is_some_and(|depth| depth != 1) {
        return false;
    }
    let status = info.status.as_deref().or(info.attempt.status.as_deref());
    if matches!(status, Some("completed") | Some("cancelled")) {
        return false;
    }
    info.attempt
        .error
        .as_deref()
        .is_some_and(nested_inherit_auth_error)
}

/// Nested inherit miss, or the AuthTransient SelfHealing copy stamped on
/// Retrying before the Failed body carries `reason=no auth context`.
fn nested_inherit_auth_error(error: &str) -> bool {
    xai_grok_shell::extensions::notification::is_nested_missing_auth_context(error)
        || error
            .to_ascii_lowercase()
            .contains("authentication is temporarily unavailable")
}

/// Last nested prompt, or the spawn description when meta.json prompt still lags.
fn nested_resume_text(info: &crate::app::subagent::SubagentInfo) -> String {
    info.prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| info.description.trim().to_string())
}

/// Operator 2026-10-17: this sentence is true only after a nested POST with the
/// live parent sampler JWT is in flight or succeeded. Parent `/login` is not that.
pub(crate) const RESUMING_NESTED_WORK_COPY: &str = "Re-authenticated. Resuming nested work";

pub(crate) fn is_resuming_nested_work_copy(text: &str) -> bool {
    let rest = text.trim().strip_prefix(RESUMING_NESTED_WORK_COPY);
    matches!(rest, Some("") | Some("\u{2026}") | Some("..."))
}

pub(crate) fn strip_resuming_nested_work_copy(
    scrollback: &mut crate::scrollback::state::ScrollbackState,
) {
    let ids: Vec<_> = (0..scrollback.len())
        .filter_map(|idx| {
            let entry = scrollback.entry(idx)?;
            match &entry.block {
                RenderBlock::System(block) if is_resuming_nested_work_copy(&block.text) => {
                    Some(entry.id)
                }
                _ => None,
            }
        })
        .collect();
    for id in ids {
        scrollback.remove_entry(id);
    }
}

fn paint_resuming_nested_work_copy(scrollback: &mut crate::scrollback::state::ScrollbackState) {
    let already = (0..scrollback.len()).any(|idx| {
        matches!(
            scrollback.entry(idx).map(|e| &e.block),
            Some(RenderBlock::System(block)) if is_resuming_nested_work_copy(&block.text)
        )
    });
    if already {
        return;
    }
    scrollback.push_block(RenderBlock::system(format!(
        "{RESUMING_NESTED_WORK_COPY}\u{2026}"
    )));
}

/// Paint the nested-resume copy only after that UnstickResendPrompt resolved ok.
/// A nested 401 with `auth_kind=bearer` plus `reason=no auth context` must not
/// leave the sentence up as if nested work re-authenticated and resumed.
pub(crate) fn apply_nested_resume_copy_outcome(
    agent: &mut AgentView,
    prompt_id: Option<&str>,
    response_pid: Option<&str>,
    succeeded: bool,
    error: Option<&str>,
) {
    let matched = [prompt_id, response_pid]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            agent
                .nested_resume_prompt_ids
                .iter()
                .position(|id| id == candidate)
                .map(|index| agent.nested_resume_prompt_ids.remove(index))
        })
        .is_some();
    let nested_missing_auth = error.is_some_and(|message| {
        xai_grok_shell::extensions::notification::is_nested_missing_auth_context(message)
    });
    if nested_missing_auth || (matched && !succeeded) {
        strip_resuming_nested_work_copy(&mut agent.scrollback);
        return;
    }
    if matched && succeeded {
        paint_resuming_nested_work_copy(&mut agent.scrollback);
    }
}

/// After re-auth, resend the nested L2's last prompt the same way a dropped
/// network does. The parent session stays bound. `LoadSession` of the nested
/// id would replace the parent chat.
///
/// AuthComplete may un-finish those L2s and emit `UnstickResendPrompt`. The
/// "Re-authenticated. Resuming nested work" copy waits until that resend's
/// PromptResponse succeeds. Painting it here after parent `/login` is a lie
/// when nested still 401s.
fn enqueue_nested_resume_after_auth(agent: &mut AgentView) -> Vec<Effect> {
    strip_resuming_nested_work_copy(&mut agent.scrollback);
    let agent_id = agent.session.id;
    let mut resumes = Vec::new();
    for info in agent.subagent_sessions.values_mut() {
        if !nested_l2_needs_auth_resume(info) {
            continue;
        }
        resumes.push((info.child_session_id.to_string(), nested_resume_text(info)));
        info.finished = false;
        info.status = None;
        info.attempt.status = None;
    }
    let recovered_bearer_jwt = recovered_parent_jwt(agent);
    let mut effects = Vec::new();
    for (child_sid, text) in resumes {
        if text.is_empty() {
            continue;
        }
        let prompt_id = uuid::Uuid::new_v4().to_string();
        agent.nested_resume_prompt_ids.push(prompt_id.clone());
        effects.push(Effect::UnstickResendPrompt {
            agent_id,
            session_id: acp::SessionId::new(child_sid),
            text,
            prompt_id,
            images: Vec::new(),
            images_dir: None,
            recovered_bearer: true,
            recovered_bearer_jwt: recovered_bearer_jwt.clone(),
        });
    }
    if !effects.is_empty() && agent.session.state.is_idle() {
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
    }
    effects
}

/// Nested bearer + xai-grok-cli + no auth context 401. Resend the interrupted
/// nested prompt with recovered Bearer once. Do not sit idle. Do not mark
/// other live L2s failed.
pub(crate) fn retry_nested_resume_after_missing_auth(agent: &mut AgentView) -> Vec<Effect> {
    if agent.nested_auth_resume_retries >= 1 {
        return vec![];
    }
    let effects = enqueue_nested_resume_after_auth(agent);
    if !effects.is_empty() {
        agent.nested_auth_resume_retries = agent.nested_auth_resume_retries.saturating_add(1);
    }
    effects
}

/// Start an interactive login flow. Triggered by pressing 'l' on the welcome screen or by the `/login` slash command.
/// Only the welcome view renders the auth UI (the external auth provider's sign-in URL and status).
/// A mid-session invocation therefore stashes the caller's view in `auth_return_view` and switches to `Welcome` so the flow is visible.
/// A build without account logins refuses it from every surface: a slash command, a menu, startup.
pub(super) fn dispatch_login(app: &mut AppView) -> Vec<Effect> {
    if refuse_withheld(app, Capability::AccountLogin) {
        return vec![];
    }
    ensure_login_method(app);
    let Some(method_id) = app.login_method_id.clone() else {
        app.auth_state = AuthState::Pending {
            error: Some(no_login_method_error(app)),
        };
        return vec![];
    };

    // Show the auth UI when triggered from inside a session
    // `show_welcome` resets ephemeral state here, covering the AuthComplete / cancel-login fallbacks too (`auth_return_view` is only ever set here)
    if !matches!(app.active_view, ActiveView::Welcome) {
        app.auth_return_view = Some(app.active_view);
        show_welcome(app);
    }

    abort_prior_auth(app);

    let request_seq = app.next_auth_request_seq;
    app.next_auth_request_seq += 1;
    app.auth_code_input.reset();
    app.auth_state = AuthState::Authenticating {
        request_seq,
        handle: None,
        auth_url: None,
        mode: app.auth_start_mode,
    };

    vec![
        Effect::Authenticate {
            request_seq,
            method_id,
            use_oauth: app.auth_use_oauth,
            force_interactive: true,
        },
        Effect::PollAuthUrl { request_seq },
    ]
}

/// Only meaningful when `auth_return_view` is set (a mid-session `/login` or 401 re-auth prompt).
/// Aborts the in-flight auth task and tells the shell to cancel its device/loopback flow so a retry does not race a still-polling prior mint.
/// Bump the seq so a fresh login does not collide with a late `AuthComplete`/`AuthFailed`.
pub(super) fn dispatch_cancel_login(app: &mut AppView) -> Vec<Effect> {
    let Some(return_view) = app.auth_return_view.take() else {
        return vec![];
    };
    // Capture the attempt's request_seq before abort clears Authenticating, so the shell cancel is scoped to this attempt only
    // A delayed RPC must not cancel a fast re-login
    let cancel_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => Some(*request_seq),
        _ => None,
    };
    abort_prior_auth(app);
    app.next_auth_request_seq += 1;
    app.auth_state = AuthState::Done;
    app.auth_show_raw_url = false;
    app.auth_code_input.reset();
    restore_auth_return_view(app, return_view);
    // This runs on all agents because the login may have been started from the dashboard
    // Clearing the stash alone is not enough
    // A leftover `ReAuthRequired` block would let a later `PromptResponse` re-detect it via `scrollback_has_recent_reauth_prompt`
    for agent in app.agents.values_mut() {
        agent.reauth_stashed_prompt = None;
        strip_trailing_auth_error_blocks(agent);
    }
    // Ask the shell to cancel its in-flight interactive auth (device poll / loopback wait)
    // Fire-and-forget: UI state is already restored
    match cancel_seq {
        Some(request_seq) => vec![Effect::CancelAuth { request_seq }],
        None => vec![],
    }
}

/// Connecting... splash wait ended: show a real auth error. Last-session restore stays deferred.
pub(super) fn dispatch_connecting_splash_timeout(app: &mut AppView) -> Vec<Effect> {
    if !app.auth_state.is_connecting_splash() {
        return vec![];
    }
    let cancel_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => Some(*request_seq),
        _ => None,
    };
    abort_prior_auth(app);
    app.next_auth_request_seq += 1;
    app.auth_state = AuthState::Pending {
        error: Some(crate::app::app_view::CONNECTING_SPLASH_TIMEOUT_ERROR.to_string()),
    };
    app.auth_show_raw_url = false;
    app.auth_code_input.reset();
    match cancel_seq {
        Some(request_seq) => vec![Effect::CancelAuth { request_seq }],
        None => vec![],
    }
}

/// User submitted a manually-pasted auth token in loopback mode.
pub(super) fn dispatch_submit_auth_code(app: &mut AppView, code: String) -> Vec<Effect> {
    let request_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        _ => return vec![],
    };

    vec![Effect::SubmitAuthCode { request_seq, code }]
}

// TaskResult handlers.

/// Live parent sampler JWTs from AuthenticateResponse `_meta`.
/// Nested recoveredBearer uses these, not AuthManager.
/// Login meta always replaces a stale pager JWT. Per-session `parentSamplerJwts`
/// wins over the generic recoveredBearerJwt / parentSamplerJwt.
fn apply_recovered_parent_sampler_jwts(app: &mut AppView, meta: Option<&serde_json::Value>) {
    let Some(obj) = meta.and_then(|v| v.as_object()) else {
        return;
    };
    for key in ["recoveredBearerJwt", "parentSamplerJwt"] {
        if let Some(jwt) = obj
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            for agent in app.agents.values_mut() {
                agent.parent_sampler_jwt = Some(jwt.to_string());
            }
            break;
        }
    }
    if let Some(map) = obj.get("parentSamplerJwts").and_then(|v| v.as_object()) {
        for agent in app.agents.values_mut() {
            if let Some(sid) = agent.session.session_id.as_ref()
                && let Some(jwt) = map
                    .get(sid.0.as_ref())
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            {
                agent.parent_sampler_jwt = Some(jwt.to_string());
            }
        }
    }
}

pub(super) fn handle_auth_complete(
    app: &mut AppView,
    request_seq: u64,
    meta: Option<serde_json::Value>,
) -> Vec<Effect> {
    if let AuthState::Authenticating {
        request_seq: current_seq,
        ..
    } = &app.auth_state
        && *current_seq == request_seq
    {
        if let Some(meta_val) = meta.as_ref()
            && let Ok(auth_meta) =
                serde_json::from_value::<xai_grok_login::AuthMeta>(meta_val.clone())
        {
            app.apply_auth_meta(&auth_meta);
        }
        apply_recovered_parent_sampler_jwts(app, meta.as_ref());

        app.auth_state = AuthState::Done;
        app.auth_show_raw_url = false;
        app.welcome_prompt_focused = !app.is_access_blocked();
        app.auth_code_input.reset();

        // Mid-session re-auth (`/login` or a 401 prompt): restore the view the user was on instead of running the startup load-session flow
        // The session state lives in `app.agents`, independent of `active_view`, so it is preserved across the auth detour
        if let Some(return_view) = app.auth_return_view.take() {
            restore_auth_return_view(app, return_view);
            // Mid-session re-auth returns to the existing session, not the startup flow
            // Discard any deferred startup stash rather than leaving it to fire later
            // One example: an incidental `Ctrl+N` pressed during /login that the chokepoint deferred
            clear_startup_actions(app);
            let retry_effects = enqueue_prompts_after_auth(app);
            let mut effects = dispatch(Action::RequestBundleStatus, app);
            if app.usage_visible {
                effects.push(Effect::FetchAppBilling { nonce: 0 });
            }
            effects.extend(retry_effects);
            return effects;
        }

        let retry_effects = enqueue_prompts_after_auth(app);
        // Request bundle status only; the shell auto-syncs after auth
        let mut effects = dispatch(Action::RequestBundleStatus, app);
        effects.extend(retry_effects);

        // Start auto-checking subscription if gated.
        // Check immediately (don't wait 5s) then schedule the timer.
        if !app.has_access() {
            app.paywall_check_started = Some(std::time::Instant::now());
            effects.push(Effect::CheckSubscription { verify: None });
            effects.push(Effect::SchedulePaywallCheck);
        }
        // Fetch billing so the welcome screen can show a credit warning.
        if app.usage_visible {
            effects.push(Effect::FetchAppBilling { nonce: 0 });
        }
        // Fetch changelog (mirrors startup path for interactive login).
        effects.push(Effect::FetchChangelog);

        // ZDR-blocked users stay on the welcome screen; discard any deferred startup (they cannot start a session)
        if app.is_zdr_blocked() {
            clear_startup_actions(app);
            return effects;
        }

        // Replay deferred session startup once both gates are open
        // If trust is still Pending its question renders next and its answer drains instead
        // The trust handlers use the same predicate, so the deferred startup runs exactly once after whichever gate resolves last
        if app.session_startup_allowed() {
            effects.extend(drain_startup_actions(app));
        }
        return effects;
    }
    // AuthComplete while already authenticated (`AuthState::Done`): IPC / sibling
    // window / login that already flipped Done. Dropping this left the credentials
    // toast plus Waiting for the model as the only thing that happened.
    // Do not drain startup: this is a mid-session recovered login, not welcome.
    if matches!(app.auth_state, AuthState::Done) {
        if let Some(meta_val) = meta.as_ref()
            && let Ok(auth_meta) =
                serde_json::from_value::<xai_grok_login::AuthMeta>(meta_val.clone())
        {
            app.apply_auth_meta(&auth_meta);
        }
        apply_recovered_parent_sampler_jwts(app, meta.as_ref());
        let retry_effects = enqueue_prompts_after_auth(app);
        let mut effects = dispatch(Action::RequestBundleStatus, app);
        if app.usage_visible {
            effects.push(Effect::FetchAppBilling { nonce: 0 });
        }
        effects.extend(retry_effects);
        return effects;
    }
    vec![]
}

pub(super) fn handle_auth_url_ready(
    app: &mut AppView,
    request_seq: u64,
    auth_url: Option<String>,
    external: bool,
    mode: Option<String>,
) -> Vec<Effect> {
    let splash_miss = matches!(
        &app.auth_state,
        AuthState::Authenticating {
            request_seq: current_seq,
            mode: AuthMode::Pending,
            auth_url: None,
            ..
        } if *current_seq == request_seq
    ) && auth_url.is_none();
    if splash_miss {
        return dispatch_connecting_splash_timeout(app);
    }
    if let AuthState::Authenticating {
        request_seq: current_seq,
        auth_url: current_url,
        mode: current_mode,
        ..
    } = &mut app.auth_state
        && *current_seq == request_seq
    {
        *current_url = auth_url;
        // Prefer `mode`; fall back to `external` for older agents
        // An old-agent device login lands on Loopback (harmless paste box; the background poll still completes)
        *current_mode = match mode.as_deref() {
            Some("device") => AuthMode::Device,
            Some("command") => AuthMode::Command,
            Some("loopback") => AuthMode::Loopback,
            _ if external => AuthMode::Command,
            _ => AuthMode::Loopback,
        };
    }
    vec![]
}

pub(super) fn handle_mcp_auth_trigger_done(
    app: &mut AppView,
    agent_id: AgentId,
    server_name: String,
    result: Result<crate::app::actions::McpAuthTriggerOutcome, String>,
) -> Vec<Effect> {
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    if let Some(ref mut modal) = agent.extensions_modal {
        modal.pending_action = None;
        modal.pending_entry_index = None;
        match result {
            Ok(crate::app::actions::McpAuthTriggerOutcome::Authenticated) => {}
            Ok(crate::app::actions::McpAuthTriggerOutcome::SetupRequired(setup)) => {
                let setup_values = match &modal.mcps_data {
                    crate::views::extensions_modal::TabDataState::Loaded(servers) => servers
                        .iter()
                        .find(|server| server.name == server_name)
                        .map(|server| server.setup_values.clone())
                        .unwrap_or_default(),
                    _ => std::collections::HashMap::new(),
                };
                if let Some(form) = crate::views::extensions_modal::McpSetupFormState::from_setup(
                    server_name.clone(),
                    setup,
                    setup_values,
                ) {
                    modal.mcp_setup = Some(form);
                } else {
                    modal.modal_message =
                        Some(crate::views::extensions_modal::ModalMessage::Error(
                            format!("{server_name}: setup schema is not supported in this UI"),
                        ));
                }
                return vec![];
            }
            Err(e) => {
                let msg = if e.starts_with("To authenticate") {
                    format!("{server_name}: {e}")
                } else if e.contains(&server_name) {
                    format!("Auth failed: {e}")
                } else {
                    format!("{server_name} auth failed: {e}")
                };
                modal.modal_message =
                    Some(crate::views::extensions_modal::ModalMessage::Error(msg));
                if let Some(session_id) = agent.session.session_id.clone() {
                    return vec![Effect::FetchMcpsList {
                        agent_id,
                        session_id,
                        cache: false,
                    }];
                }
                return vec![];
            }
        }
    }
    // No toast on success: the row transition from the FetchMcpsList refresh below is the confirmation
    let Some(session_id) = agent.session.session_id.clone() else {
        return vec![];
    };
    vec![Effect::FetchMcpsList {
        agent_id,
        session_id,
        cache: false,
    }]
}

pub(super) fn handle_mcp_setup_submit_done(
    app: &mut AppView,
    agent_id: AgentId,
    server_name: String,
    result: Result<(), String>,
) -> Vec<Effect> {
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    if let Some(ref mut modal) = agent.extensions_modal {
        if let Err(e) = result {
            modal.pending_action = None;
            modal.pending_entry_index = None;
            modal.modal_message = Some(crate::views::extensions_modal::ModalMessage::Error(
                format!("{server_name} setup failed: {e}"),
            ));
            return vec![];
        }
        modal.pending_action = Some(format!("Authenticating {server_name}..."));
        modal.pending_entry_index = None;
    }
    let Some(session_id) = agent.session.session_id.clone() else {
        if let Some(ref mut modal) = agent.extensions_modal {
            modal.pending_action = None;
            modal.modal_message = Some(crate::views::extensions_modal::ModalMessage::Error(
                format!("{server_name}: no active session for authentication"),
            ));
        }
        return vec![];
    };
    vec![Effect::McpAuthTrigger {
        agent_id,
        session_id,
        server_name,
    }]
}
