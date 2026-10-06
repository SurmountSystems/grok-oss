use super::*;

/// Resolve a superseded elicitation's reverse-request with `Cancel` so the awaiting MCP server is released before a replacement takes the slot.
fn cancel_elicitation_request(
    response_tx: tokio::sync::oneshot::Sender<xai_acp_lib::AcpResult<acp::ExtResponse>>,
) {
    let cancelled = xai_grok_tools::mcp_elicitation::McpElicitExtResponse::Cancel;
    if let Ok(raw) = serde_json::value::to_raw_value(&cancelled) {
        response_tx.send(Ok(acp::ExtResponse::new(raw.into()))).ok();
    }
}

pub(crate) fn handle_mcp_elicit(
    ext: xai_acp_lib::AcpArgs<acp::ExtRequest>,
    app: &mut AppView,
) -> bool {
    use crate::views::elicitation_view::ElicitationViewState;
    use xai_grok_tools::mcp_elicitation::McpElicitExtRequest;

    let ext_req: McpElicitExtRequest = match serde_json::from_str(ext.request.params.get()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "Failed to parse McpElicitExtRequest");
            ext.response_tx
                .send(Err(acp::Error::new(-32602, format!("Invalid params: {e}"))))
                .ok();
            return false;
        }
    };

    let Some(id) = interaction_target_agent(app, &ext_req.session_id) else {
        tracing::info!(
            session_id = %ext_req.session_id,
            "mcp elicit for a session with no local view; parked for leader replay-on-attach"
        );
        drop(ext.response_tx);
        return false;
    };
    let is_active = is_matched_agent_active(app, id);
    let Some(agent) = app.agents.get_mut(&id) else {
        drop(ext.response_tx);
        return false;
    };

    let waiting = agent
        .elicitation_view
        .as_ref()
        .is_some_and(|ev| ev.is_url_waiting());
    if waiting {
        if let Some((_, old_tx)) = agent.pending_elicitation.take() {
            cancel_elicitation_request(old_tx);
        }
        agent.pending_elicitation = Some((ext_req, ext.response_tx));
        return is_active;
    }

    if let Some((_, old_tx)) = agent.pending_elicitation.take() {
        cancel_elicitation_request(old_tx);
    }

    // Mandatory ingress wins: evict an open feedback modal before this elicitation installs and stashes its own state.
    agent.displace_feedback_modal(
        crate::views::feedback_modal::FeedbackModalDisplacement::McpElicitation,
    );

    if let Some(mut old) =
        agent.take_unanswered_elicitation(crate::app::agent_view::UnansweredElicitation::Superseded)
    {
        if let Some(old_tx) = old.take_response_tx() {
            cancel_elicitation_request(old_tx);
        }
        agent.restore_elicitation_prompt(old.stashed_prompt);
    }

    let stashed = agent.stash_prompt_for_elicitation();
    agent.elicitation_view = Some(ElicitationViewState::from_request(
        ext_req,
        stashed,
        Some(ext.response_tx),
    ));
    agent.last_active_at = Some(std::time::Instant::now());

    tracing::info!(
        target_active = is_active,
        "Opened MCP elicitation view from ext_method"
    );
    is_active
}

/// Handle `x.ai/ask_user_question` ext-method.
/// Parses the typed request, creates a `QuestionViewState` with the `response_tx` stashed, and opens the question overlay.
/// The pager does NOT respond immediately; the response is sent later when the user submits, cancels, or is replaced by another question.
pub(crate) fn handle_ask_user_question(
    ext: xai_acp_lib::AcpArgs<acp::ExtRequest>,
    app: &mut AppView,
) -> bool {
    use crate::views::question_view::QuestionViewState;
    use xai_grok_tools::implementations::grok_build::ask_user_question::{
        AskUserQuestionExtRequest, AskUserQuestionExtResponse,
    };

    // Parse the typed request from the ext-method params.
    let ext_req: AskUserQuestionExtRequest = match serde_json::from_str(ext.request.params.get()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "Failed to parse AskUserQuestionExtRequest");
            ext.response_tx
                .send(Err(acp::Error::new(-32602, format!("Invalid params: {e}"))))
                .ok();
            return false;
        }
    };

    // Route by the request's session id (like `session/update`), so a question raised by a BACKGROUND session lands on its own view
    // The user may be on the dashboard or another session and never have entered this one; the question must not fail for that
    let Some(id) = interaction_target_agent(app, &ext_req.session_id) else {
        // No local view for this session. Do NOT send an error: that would FAIL the tool (rendered red).
        // Leave the reverse-request unanswered; the agent keeps awaiting and the leader replays it when a client attaches via `session/load`
        tracing::info!(
            session_id = %ext_req.session_id,
            "ask_user_question for a session with no local view; parked for leader replay-on-attach"
        );
        drop(ext.response_tx);
        return false;
    };
    let is_active = is_matched_agent_active(app, id);
    let Some(agent) = app.agents.get_mut(&id) else {
        // `interaction_target_agent` only returns ids that exist; this arm is defensive
        tracing::warn!("ask_user_question: agent {id:?} not found");
        drop(ext.response_tx);
        return false;
    };

    // Mandatory ingress wins: evict an open feedback modal before this question installs and stashes its own state.
    agent.displace_feedback_modal(
        crate::views::feedback_modal::FeedbackModalDisplacement::AcpQuestion,
    );

    // If a question is already active, cancel it before replacing.
    if let Some(mut old_qv) = agent.question_view.take() {
        agent.record_question_pause(&old_qv);
        tracing::warn!(
            old_tool_call_id = %old_qv.tool_call_id,
            new_tool_call_id = %ext_req.tool_call_id,
            "Replacing active question - cancelling previous"
        );
        if let Some(old_tx) = old_qv.response_tx.take() {
            let cancelled = AskUserQuestionExtResponse::Cancelled;
            let raw = serde_json::value::to_raw_value(&cancelled)
                .expect("Cancelled serialization should not fail");
            old_tx.send(Ok(acp::ExtResponse::new(raw.into()))).ok();
        }
        agent.restore_card_prompt(old_qv.stashed_prompt);

        // An ACP ask displaced this local question, so tell the user why it vanished
        // Any directive it carried is dropped; the user re-issues the command after answering.
        if let Some(kind) = old_qv.local_kind.take() {
            use crate::views::question_view::LocalQuestionKind;
            match kind {
                LocalQuestionKind::Feedback => {}
                LocalQuestionKind::DoctorFix { .. } => {
                    agent.scrollback.push_block(RenderBlock::system(
                        "/doctor fix was cancelled because another question opened.".to_owned(),
                    ));
                }
                // The hold and the requeued front row survive the displaced card, so the queue stays parked; only the card is lost
                LocalQuestionKind::PromptBlocked { .. } => {
                    agent.scrollback.push_block(RenderBlock::system(
                        "The blocked-prompt card was replaced by another question. Your prompt is still held at the front of the queue.".to_owned(),
                    ));
                }
                kind => {
                    // The doctor-fix arm above owns its variant; its label here is a graceful fallback
                    let cmd = match kind {
                        LocalQuestionKind::Fork { .. } => "/fork",
                        LocalQuestionKind::NewSession => "/new",
                        LocalQuestionKind::CreditLimitUpsell { .. } => "credit-limit upsell",
                        LocalQuestionKind::FreeUsageUpsell { .. } => "SuperGrok upsell",
                        LocalQuestionKind::AgentTypeMismatch { .. } => "model switch",
                        LocalQuestionKind::DeleteCurrentSession => "/delete",
                        LocalQuestionKind::DoctorFix { .. } => "/doctor fix",
                        LocalQuestionKind::Feedback => "feedback",
                        // The dedicated arm above owns this variant; the label is kept for exhaustiveness
                        LocalQuestionKind::PromptBlocked { .. } => "blocked prompt",
                    };
                    agent.scrollback.push_block(RenderBlock::system(format!(
                        "{cmd} cancelled because another question opened."
                    )));
                }
            }
        }
    }

    // Stash the composer so it comes back when this question closes.
    agent.question_view = Some(QuestionViewState::with_response_tx(
        ext_req.tool_call_id,
        ext_req.questions,
        agent.prompt.stash(),
        Some(ext.response_tx),
        ext_req.mode,
    ));

    agent.prompt.set_text("");

    // Stamp the "last activity" anchor so the dashboard's NeedsInput row shows time since this question arrived, not the previous turn's end
    agent.last_active_at = Some(std::time::Instant::now());

    tracing::info!(
        mode = ?ext_req.mode,
        question_count = agent.question_view.as_ref().map(|q| q.questions.len()).unwrap_or(0),
        target_active = is_active,
        "Opened question view from ext_method"
    );

    // Only the currently-displayed view needs an immediate redraw
    // A question parked on a background agent shows up via the roster `NeedsInput` delta and renders when the user switches to that session
    is_active
}

/// Handle an `x.ai/exit_plan_mode` ext_method request.
/// Creates a `PlanApprovalViewState` overlay for interactive approval.
/// Freeform is prefilled only when safe (not under an open permission).
pub(super) fn handle_exit_plan_mode(
    ext: xai_acp_lib::AcpArgs<acp::ExtRequest>,
    app: &mut AppView,
) -> bool {
    use crate::views::plan_approval_view::{ExitPlanModeExtRequest, PlanApprovalViewState};

    // 1. Parse typed request from raw JSON params.
    let params: ExitPlanModeExtRequest = match serde_json::from_str(ext.request.params.get()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to parse ExitPlanModeExtRequest: {e}");
            ext.response_tx
                .send(Err(acp::Error::new(
                    -32602,
                    format!("Invalid exit_plan_mode params: {e}"),
                )))
                .ok();
            return false;
        }
    };

    // 2. Route by the request's session id (like `session/update`), so a plan-approval raised by a BACKGROUND session lands on its own view.
    // The user may not be focused on that session; the approval must not fail for that
    let Some(id) = interaction_target_agent(app, &params.session_id) else {
        // No local view for this session. Do NOT error (that fails the tool).
        // Leave the reverse-request unanswered and rely on the leader's replay-on-attach
        tracing::info!(
            session_id = %params.session_id,
            "exit_plan_mode for a session with no local view; parked for leader replay-on-attach"
        );
        drop(ext.response_tx);
        return false;
    };
    // A restore that arrives before bind has no session to park on.
    // Hold the reverse-request until SessionLoaded binds, then flush.
    if app
        .agents
        .get(&id)
        .is_some_and(|agent| agent.session.session_id.is_none())
    {
        app.pending_exit_plan_mode = Some(ext);
        return false;
    }
    let is_active = is_matched_agent_active(app, id);
    let Some(agent) = app.agents.get_mut(&id) else {
        // `interaction_target_agent` only returns ids that exist; this arm is defensive
        tracing::warn!("exit_plan_mode: agent {id:?} not found");
        drop(ext.response_tx);
        return false;
    };

    // Approve or Quit already decided this plan. A later resume must not
    // re-park, must not dock, and must leave the mid-type draft alone.
    // A new live present still re-arms the decision.
    if agent.plan_decision_resolved && params.tool_call_id.starts_with("exit-plan-mode-resume") {
        drop(ext.response_tx);
        return is_active;
    }

    // Mandatory ingress wins: evict an open feedback modal before the approval captures the session draft.
    agent.displace_feedback_modal(
        crate::views::feedback_modal::FeedbackModalDisplacement::PlanApproval,
    );

    // Replay of a stored present keeps comments. A live present starts clean.
    let is_restore = agent.session.loading_replay;
    let resume_park = params.tool_call_id.starts_with("exit-plan-mode-resume");
    // unmount_plan_review closes the pane. Remember an already-open
    // pane so a resume re-park can dock that pane again. A closed pane
    // stays closed. A mid-compose draft must not open it.
    let pane_already_open = agent.line_viewer.is_some();
    let mut carried_comments = Vec::new();
    let mut carried_next_comment_id = 0u64;
    let mut carried_feedback_draft: Option<String> = None;
    // `unmount_plan_review` restores the previous session stash and would
    // wipe Revise-box text typed after that park. A resume replace keeps it.
    let composer_before_replace = (resume_park || is_restore)
        .then(|| agent.prompt.text().to_string())
        .filter(|text| {
            let trimmed = text.trim();
            !trimmed.is_empty() && !trimmed.starts_with('/')
        });

    if let Some(mut old) = agent.unmount_plan_review() {
        tracing::warn!(
            old_tool_call_id = %old.tool_call_id,
            new_tool_call_id = %params.tool_call_id,
            "Replacing active plan approval — dismissing previous"
        );
        if is_restore {
            carried_comments = std::mem::take(&mut old.comments);
            carried_next_comment_id = old.next_comment_id;
        }
        if is_restore || resume_park {
            carried_feedback_draft = old.feedback_draft.take();
        }
        old.send_stale_cancel();
    }
    if let Some(text) = composer_before_replace {
        agent.prompt.set_text(&text);
    }

    // Dismiss competing overlays so plan approval owns the screen.
    // - active_modal: draw returns before line_viewer (plan never paints); keys still route to the invisible plan viewer
    // - block_viewer: draw returns on line_viewer (plan visible) but handle_scroll prefers block_viewer, so wheel hits the hidden Edit pane
    agent.active_modal = None;
    agent.dismiss_block_viewer();

    let source = plan_review_source_for_tool(&params.tool_call_id, agent);

    // If the user was writing a casual comment when this new plan-approval request arrived, restore the prompt from before the comment
    // The upcoming `stash()` then captures the user's original text rather than the in-progress comment draft
    // Taking `casual_stashed_prompt` also clears it, so the stale draft cannot dangle into the next casual entry
    if let Some(stashed) = agent.casual_stashed_prompt.take() {
        agent.prompt.restore(stashed);
    }
    if agent.composer_holds_view_plan_slash() {
        agent.view_plan_requested = true;
        agent.prompt.set_text("");
    }

    // While a permission is open, the session draft is `permission_stashed_prompt` and the live composer holds the followup
    // Otherwise the live composer is the session draft
    let permission_still_open = !agent.permission_queue.is_empty();
    let session_draft = if let Some(perm_draft) = agent.permission_stashed_prompt.take() {
        let _permission_followup = agent.prompt.stash();
        perm_draft
    } else {
        agent.prompt.stash()
    };

    let had_session_draft = !session_draft.is_effectively_empty();
    // Never prefill freeform while permission owns the keyboard: the followup would type into (and could send) the private session draft
    // Set the deferred prefill flag instead so `restore_permission_stashes` applies it only when the queue actually drains
    if had_session_draft && !permission_still_open {
        agent.plan_freeform_prefill_deferred = false;
        agent.prompt.restore(session_draft.clone_for_live_prefill());
    } else {
        agent.plan_freeform_prefill_deferred = permission_still_open;
        agent.prompt.set_text("");
    }

    let state = PlanApprovalViewState::with_source(params, source, session_draft, ext.response_tx);

    agent.plan_comments.clear();
    agent.plan_next_comment_id = 0;

    if state.source == PlanReviewSource::Inline {
        if let Some(body) = state
            .plan_content
            .clone()
            .and_then(crate::app::agent_view::capped_kept_plan_body)
        {
            agent.kept_plan = crate::app::agent_view::KeptPlan::kept(Some(body), None);
        } else {
            // Empty or oversized inline must not leave the prior keep.
            agent.clear_kept_plan();
        }
    } else {
        agent.clear_kept_plan();
    }
    if let Some(ref body) = state.plan_content {
        if agent.isolated_preview_shows_secondary_plan && source == PlanReviewSource::Inline {
            agent.persist_session_plan_body_for(
                xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY,
                body,
            );
        } else {
            if source == PlanReviewSource::FileBacked {
                agent.isolated_preview_shows_secondary_plan = false;
            }
            agent.persist_session_plan_body(body);
        }
    }
    // Live present re-arms decision CTAs after a prior Approve/Quit and
    // clears Revise/Clarify in-flight so CTAs arm once. Restore must not
    // clear sticky resolved (leftover/approved plan.md stays decided).
    if !is_restore {
        agent.clear_plan_loop_flags_for_new_present();
    }
    agent.plan_approval_view = Some(state);
    if let Some(draft) = carried_feedback_draft.clone() {
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.feedback_draft = Some(draft.clone());
            if agent.prompt.text().trim() == draft.trim() && !draft.trim().is_empty() {
                // Carried Revise-box text is the next Operator turn. Approve
                // must not wrap it as review comments.
                pav.keep_draft_is_next_operator_turn = true;
            }
        }
    }
    if is_restore {
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.comments = carried_comments;
            pav.next_comment_id = carried_next_comment_id;
        }
        agent.plan_next_comment_id = carried_next_comment_id;
        agent.restore_plan_feedback_draft_if_composer_lost();
        if !agent.prompt.text().trim().is_empty() && !agent.composer_holds_view_plan_slash() {
            agent.snapshot_or_clear_plan_feedback_draft();
        }
    }
    // Keep a mid-compose draft visible. stash() copies text and does not
    // clear it; only wipe when the composer was already empty so empty-prompt
    // `a` / `s` / `q` stay accelerators. Focus stays Preview. Comment and
    // Revise clicks are what move focus to Prompt.
    let keep_draft =
        !agent.prompt.text().trim().is_empty() && !agent.composer_holds_view_plan_slash();
    let live_cursor = agent.prompt.cursor();
    if keep_draft {
        agent.prompt.set_cursor(live_cursor);
        if is_restore && let Some(ref mut pav) = agent.plan_approval_view {
            pav.stashed_prompt.text = agent.prompt.text().to_string();
            pav.stashed_prompt.cursor = live_cursor;
            pav.keep_draft_is_next_operator_turn = true;
        }
    }

    agent.casual_commenting_range = None;
    agent.casual_editing_comment_id = None;

    crate::appearance::cache::set_plan_approval_force_modal(
        app.current_ui.plan_approval_force_modal(),
    );
    // A live present docks Isolated Preview. A resume re-park stays shut
    // unless the pane was already open, `/view-plan` asked for it, or disk
    // still says this session is awaiting plan approval. That last case is
    // quit then `--continue`: the shell re-issues `exit_plan_mode`, and the
    // side panel must paint Plan ready so approval chrome returns. A
    // mid-compose draft must not open the pane. A resume with no awaiting
    // flag and no open pane stays shut.
    let composer_blocks_resume_dock = resume_park
        && !agent.prompt.text().trim().is_empty()
        && !agent.composer_holds_view_plan_slash();
    let awaiting_on_disk = resume_park
        && agent.session.session_id.as_ref().is_some_and(|sid| {
            let cwd = agent.session.cwd.to_string_lossy();
            xai_grok_shell::session::plan_mode::load_awaiting_plan_approval(&cwd, sid.0.as_ref())
        });
    let dock = if resume_park {
        !composer_blocks_resume_dock
            && (pane_already_open
                || agent.view_plan_requested
                || agent.composer_holds_view_plan_slash()
                || awaiting_on_disk)
    } else {
        true
    };
    if dock {
        agent.show_plan_preview_if_available();
        if let Some(ref mut viewer) = agent.line_viewer {
            viewer.plan_mut().feedback_active = true;
        }
    }
    agent.restore_plan_feedback_draft_if_composer_lost();
    agent.persist_unsent_composer_draft_now();

    // Soft plan present: a body that is only the Operator prompt, or only
    // a Job/State/Operator status recap, is not the feature document.
    if !is_restore && agent.isolated_preview_shows_secondary_plan {
        let presented = agent
            .plan_approval_view
            .as_ref()
            .and_then(|pav| pav.plan_content.clone())
            .unwrap_or_default();
        if crate::slash::commands::plan::soft_present_should_keep_feature_plan(&presented) {
            agent.restore_soft_feature_plan_over_prompt_or_status();
        }
    }

    // Soft plan present: a body that is only the Operator prompt, or only
    // a Job/State/Operator status recap, is not the feature document.
    if !is_restore && agent.isolated_preview_shows_secondary_plan {
        let presented = agent
            .plan_approval_view
            .as_ref()
            .and_then(|pav| pav.plan_content.clone())
            .unwrap_or_default();
        if crate::slash::commands::plan::soft_present_should_keep_feature_plan(&presented) {
            agent.restore_soft_feature_plan_over_prompt_or_status();
        }
    }

    tracing::info!(
        target_active = is_active,
        "Opened plan approval view from ext_method"
    );

    // An approval parked on a background session renders when the user switches to it; only the active view needs an immediate redraw
    is_active
}

/// Apply a restore `exit_plan_mode` that arrived before the session was bound.
pub(crate) fn flush_pending_exit_plan_mode(app: &mut AppView) -> bool {
    let Some(ext) = app.pending_exit_plan_mode.take() else {
        return false;
    };
    handle_exit_plan_mode(ext, app)
}

pub(super) fn plan_review_source_for_tool(
    tool_call_id: &str,
    agent: &AgentView,
) -> PlanReviewSource {
    agent
        .session
        .tracker
        .tool_title(tool_call_id)
        .filter(|title| *title == "CreatePlan" || *title == "Plan: Submit for approval")
        .map_or(PlanReviewSource::FileBacked, |_| PlanReviewSource::Inline)
}
