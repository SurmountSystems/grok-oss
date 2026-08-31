use super::*;

/// Handle `x.ai/ask_user_question` ext-method.
///
/// Parses the typed request, creates a `QuestionViewState` with the
/// `response_tx` stashed, and opens the question overlay. The pager does
/// NOT respond immediately — the response is sent later when the user
/// submits, cancels, or is replaced by another question.
///
/// If a question is already active, the old one is cancelled first
/// (`Cancelled` is sent on its stashed `response_tx`).
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

    // Route by the request's session id (like `session/update`), so a question
    // raised by a BACKGROUND session lands on its own view even when the user is
    // on the dashboard or another session — rather than failing because the
    // user hasn't entered the session yet.
    let Some(id) = interaction_target_agent(app, &ext_req.session_id) else {
        // No local view for this session. Do NOT send an error — that would FAIL
        // the tool (rendered red). Leave the reverse-request unanswered: the
        // agent keeps awaiting and the leader replays it when a client attaches
        // via `session/load`.
        tracing::info!(
            session_id = %ext_req.session_id,
            "ask_user_question for a session with no local view; parked for leader replay-on-attach"
        );
        drop(ext.response_tx);
        return false;
    };
    let is_active = is_matched_agent_active(app, id);
    let Some(agent) = app.agents.get_mut(&id) else {
        // `interaction_target_agent` only returns ids that exist; defensive.
        tracing::warn!("ask_user_question: agent {id:?} not found");
        drop(ext.response_tx);
        return false;
    };

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

        // Local question displaced by an ACP ask, so surface why it vanished.
        // Any directive it carried is dropped; the user re-issues the command after answering.
        if let Some(ref kind) = old_qv.local_kind {
            use crate::views::question_view::LocalQuestionKind;
            let cmd = match kind {
                LocalQuestionKind::Fork { .. } => "/fork",
                LocalQuestionKind::NewSession => "/new",
                LocalQuestionKind::CreditLimitUpsell { .. } => "credit-limit upsell",
                LocalQuestionKind::FreeUsageUpsell { .. } => "SuperGrok upsell",
                LocalQuestionKind::AgentTypeMismatch { .. } => "model switch",
                LocalQuestionKind::DoctorFix { .. } => "/doctor fix",
                LocalQuestionKind::DeleteCurrentSession => "/delete",
                LocalQuestionKind::Feedback => "/feedback",
            };
            let message = if matches!(kind, LocalQuestionKind::DoctorFix { .. }) {
                "/doctor fix was cancelled because another question opened.".to_owned()
            } else {
                format!("{cmd} cancelled because another question opened.")
            };
            agent.scrollback.push_block(RenderBlock::system(message));
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

    // Clear prompt for question interaction.
    agent.prompt.set_text("");

    // Stamp the "last activity" anchor so the
    // dashboard's NeedsInput row reflects "time since this question
    // arrived" rather than the previous turn's end time.
    agent.last_active_at = Some(std::time::Instant::now());

    tracing::info!(
        mode = ?ext_req.mode,
        question_count = agent.question_view.as_ref().map(|q| q.questions.len()).unwrap_or(0),
        target_active = is_active,
        "Opened question view from ext_method"
    );

    // Only the currently-displayed view needs an immediate redraw; a question
    // parked on a background agent surfaces via the roster `NeedsInput` delta
    // and renders when the user switches to that session.
    is_active
}

/// Handle an `x.ai/exit_plan_mode` ext_method request.
///
/// Creates a `PlanApprovalViewState` overlay for interactive approval.
///
/// Follows the `handle_ask_user_question` pattern: parse → guard → cancel old
/// → stash prompt → create state → clear prompt → return true.
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

    // 2. Route by the request's session id (like `session/update`), so a
    // plan-approval raised by a BACKGROUND session lands on its own view even
    // when the user isn't currently focused on it — rather than failing.
    let is_restore = params.tool_call_id.starts_with("exit-plan-mode-resume-");
    // Restore must not use the unbound race-window fallback. That agent is
    // not a local view for this session until SessionLoaded binds it.
    let Some(id) = (if is_restore {
        interaction_target_bound_agent(app, &params.session_id)
    } else {
        interaction_target_agent(app, &params.session_id)
    }) else {
        // No local view yet. Restore must not drop the reverse-request: the
        // pager binds the session on SessionLoaded after session/load returns.
        // Live mid-turn still leaves unanswered for leader replay-on-attach.
        if is_restore {
            tracing::info!(
                session_id = %params.session_id,
                "exit_plan_mode restore for a session with no local view; held until bind"
            );
            app.pending_exit_plan_mode = Some(ext);
            return false;
        }
        tracing::info!(
            session_id = %params.session_id,
            "exit_plan_mode for a session with no local view; parked for leader replay-on-attach"
        );
        drop(ext.response_tx);
        return false;
    };
    let is_active = is_matched_agent_active(app, id);
    let Some(agent) = app.agents.get_mut(&id) else {
        // `interaction_target_agent` only returns ids that exist; defensive.
        tracing::warn!("exit_plan_mode: agent {id:?} not found");
        drop(ext.response_tx);
        return false;
    };

    if is_restore && agent.plan_decision_resolved {
        tracing::info!(
            "exit_plan_mode restore skipped: plan already decided; do not re-arm Plan ready"
        );
        drop(ext.response_tx);
        return is_active;
    }

    // Resume must not auto-dock. `/view-plan` can race this reverse-request:
    // the pane may already be open, the slash may have run with no pane yet,
    // or Enter may still be in the composer as `/view-plan`. Dock only then.
    if agent.composer_holds_view_plan_slash() {
        agent.view_plan_requested = true;
        agent.prompt.set_text("");
    }
    let restore_open_pane = is_restore
        && (agent.is_plan_viewer() || agent.line_viewer.is_some() || agent.view_plan_requested);

    let mut carried_comments = Vec::new();
    let mut carried_next_comment_id = 0;
    let mut carried_feedback_draft = None;
    if let Some(mut old) = agent.plan_approval_view.take() {
        tracing::warn!(
            old_tool_call_id = %old.tool_call_id,
            new_tool_call_id = %params.tool_call_id,
            "Replacing active plan approval — dismissing previous"
        );
        if is_restore {
            carried_comments = std::mem::take(&mut old.comments);
            carried_next_comment_id = old.next_comment_id;
            carried_feedback_draft = old.feedback_draft.take();
        }
        old.send_stale_cancel();
        agent.plan_next_comment_id = if is_restore {
            carried_next_comment_id
        } else {
            old.next_comment_id
        };
        if agent.prompt.text().trim().is_empty() {
            agent.prompt.restore(old.stashed_prompt);
        }
        if !restore_open_pane {
            agent.line_viewer = None;
        }
    }

    // Dismiss competing overlays so plan approval owns the screen.
    // - active_modal: draw returns before line_viewer (plan never paints);
    //   keys still route to the invisible plan viewer.
    // - block_viewer: draw returns on line_viewer (plan visible) but
    //   handle_scroll prefers block_viewer, so wheel hits the hidden Edit pane.
    agent.active_modal = None;
    agent.block_viewer = None;

    let source = plan_review_source_for_tool(&params.tool_call_id, agent);

    // If the user was mid-casual-comment when this new plan-approval
    // request arrived, restore the pre-comment prompt first so the
    // upcoming `stash()` captures the user's original text rather
    // than the in-progress comment draft. Also clears the now-stale
    // `casual_stashed_prompt` so it doesn't dangle into the next
    // casual entry.
    if let Some(stashed) = agent.casual_stashed_prompt.take() {
        agent.prompt.restore(stashed);
    }
    if agent.composer_holds_view_plan_slash() {
        agent.view_plan_requested = true;
        agent.prompt.set_text("");
    }

    let keep_draft = !agent.prompt.text().trim().is_empty();
    let live_cursor = agent.prompt.cursor();
    // Restore must not snapshot the Revise / Comment box as keep-draft
    // review notes. Isolated present still Approves from Preview.
    let stashed = if is_restore {
        crate::views::prompt_widget::StashedPrompt::default()
    } else {
        agent.prompt.stash()
    };
    // Live mid-turn present auto-opens. Resume / restore re-park keeps the
    // waiter and does not dock the side panel.
    let state = PlanApprovalViewState::with_source(params, source, stashed, ext.response_tx);

    agent.plan_comments.clear();
    agent.plan_next_comment_id = 0;

    if state.source == PlanReviewSource::Inline {
        agent.latest_inline_plan_content = state.plan_content.clone();
    } else {
        agent.latest_inline_plan_content = None;
    }
    // Live present re-arms decision CTAs after a prior Approve/Quit and
    // clears Revise/Clarify in-flight so CTAs arm once. Restore must not
    // clear sticky resolved (leftover/approved plan.md stays decided).
    if !is_restore {
        agent.clear_plan_loop_flags_for_new_present();
    }
    agent.plan_approval_view = Some(state);
    if is_restore {
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.comments = carried_comments;
            pav.next_comment_id = carried_next_comment_id;
            pav.feedback_draft = carried_feedback_draft;
        }
        agent.plan_next_comment_id = carried_next_comment_id;
        agent.restore_plan_feedback_draft_if_composer_lost();
        if !agent.prompt.text().trim().is_empty() && !agent.composer_holds_view_plan_slash() {
            agent.snapshot_or_clear_plan_feedback_draft();
        }
    }
    // Keep a mid-compose draft visible. stash() copies text and does not
    // clear it; only wipe when the composer was already empty so empty-prompt
    // `a` / `s` / `q` stay accelerators.
    if keep_draft {
        agent.prompt.set_cursor(live_cursor);
    }

    agent.casual_commenting_range = None;
    agent.casual_editing_comment_id = None;

    crate::appearance::cache::set_plan_approval_force_modal(
        app.current_ui.plan_approval_force_modal(),
    );
    if !is_restore || restore_open_pane {
        agent.show_plan_preview_if_available();
    }
    if restore_open_pane {
        agent.bind_restore_plan_preview_approve();
    } else if agent.line_viewer.is_some()
        && let Some(ref mut viewer) = agent.line_viewer
    {
        // Isolated present is visual. Leave Preview so the composer stays
        // the agent prompt. Click Revise / Clarify / Comment to arm feedback.
        viewer.plan_mut().feedback_active = true;
    }
    agent.restore_plan_feedback_draft_if_composer_lost();
    agent.persist_unsent_composer_draft();

    tracing::info!(
        target_active = is_active,
        "Opened plan approval view from ext_method"
    );

    // Background-parked approval renders when the user switches to the session;
    // only the active view needs an immediate redraw.
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
