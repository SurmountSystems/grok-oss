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
        agent.turn_paused_duration += old_qv.opened_at.elapsed();
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
        // Restore the old stashed prompt before stashing the new one.
        agent.prompt.restore(old_qv.stashed_prompt);
        // Inverse-collision: the displaced question was a
        // local one (e.g. /fork, /new) -- surface a system-block marker so
        // the user understands why their modal vanished. The directive
        // payload (if any) is dropped; the user can re-issue the command
        // after answering the model's question.
        if let Some(ref kind) = old_qv.local_kind {
            use crate::views::question_view::LocalQuestionKind;
            let cmd = match kind {
                LocalQuestionKind::Fork { .. } => "/fork",
                LocalQuestionKind::NewSession => "/new",
                LocalQuestionKind::CreditLimitUpsell { .. } => "credit-limit upsell",
                LocalQuestionKind::FreeUsageUpsell { .. } => "SuperGrok upsell",
                LocalQuestionKind::AgentTypeMismatch { .. } => "model switch",
                LocalQuestionKind::ProjectSelect { .. } => "project select",
                LocalQuestionKind::DoctorFix { .. } => "/doctor fix",
            };
            let message = if matches!(kind, LocalQuestionKind::DoctorFix { .. }) {
                "/doctor fix was cancelled because another question opened.".to_owned()
            } else {
                format!("{cmd} cancelled because another question opened.")
            };
            agent.scrollback.push_block(RenderBlock::system(message));
        }
    }

    // Stash the current prompt and create the question view.
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
/// Soft-parks a durable `PlanApprovalViewState` (option A): status chrome +
/// toast, **no** hard modal takeover. The full approval surface opens on
/// demand via `/view-plan`, status click, or `ShowPlan` / `reopen_plan_approval`.
///
/// Parse → cancel old (if any) → park state → toast → return whether the
/// active view needs a redraw.
pub(super) fn handle_exit_plan_mode(
    ext: xai_acp_lib::AcpArgs<acp::ExtRequest>,
    app: &mut AppView,
) -> bool {
    use crate::views::plan_approval_view::{
        ExitPlanModeExtRequest, PLAN_PARKED_TOAST, PlanApprovalViewState,
    };
    use crate::views::prompt_widget::StashedPrompt;

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
    let Some(id) = interaction_target_agent(app, &params.session_id) else {
        // No local view for this session. Do NOT error (that fails the tool):
        // leave the reverse-request unanswered and rely on the leader's
        // replay-on-attach.
        tracing::info!(
            session_id = %params.session_id,
            "exit_plan_mode for a session with no local view; parked for leader replay-on-attach"
        );
        drop(ext.response_tx);
        return false;
    };
    let is_active = is_matched_agent_active(app, id);
    // Read UI config before borrowing the agent (option D force-modal).
    let force_modal = app.current_ui.plan_approval_force_modal();
    let Some(agent) = app.agents.get_mut(&id) else {
        // `interaction_target_agent` only returns ids that exist; defensive.
        tracing::warn!("exit_plan_mode: agent {id:?} not found");
        drop(ext.response_tx);
        return false;
    };

    if let Some(mut old) = agent.plan_approval_view.take() {
        tracing::warn!(
            old_tool_call_id = %old.tool_call_id,
            new_tool_call_id = %params.tool_call_id,
            "Replacing active plan approval — dismissing previous"
        );
        old.send_stale_cancel();
        agent.plan_next_comment_id = old.next_comment_id;
        // Soft-park uses an empty stash until the user opens the modal
        // (`reopen_plan_approval` stashes then). Restoring an empty soft-park
        // stash would wipe in-progress prompt text — only restore when the
        // previous engagement captured a real stash.
        let had_real_stash = !old.stashed_prompt.text.is_empty()
            || !old.stashed_prompt.images.is_empty()
            || !old.stashed_prompt.chip_elements.is_empty();
        if had_real_stash {
            agent.prompt.restore(old.stashed_prompt);
        }
        agent.line_viewer = None;
    }

    // Soft park does **not** dismiss competing overlays or open the line
    // viewer — that hard takeover is what jars when plan mode was unexpected.
    // `reopen_plan_approval` dismisses overlays when the user engages.

    let source = plan_review_source_for_tool(&params.tool_call_id, agent);

    // If the user was mid-casual-comment when this new plan-approval
    // request arrived, restore the pre-comment prompt so soft-park leaves
    // them where they were (and clears the now-stale casual draft fields).
    if let Some(stashed) = agent.casual_stashed_prompt.take() {
        agent.prompt.restore(stashed);
    }

    // Soft park: do not stash/clear the live prompt. Empty stash is filled
    // when the user opens the approval surface via reopen.
    let state = PlanApprovalViewState::with_source(
        params,
        source,
        StashedPrompt::default(),
        ext.response_tx,
    );

    agent.plan_comments.clear();
    agent.plan_next_comment_id = 0;

    if state.source == PlanReviewSource::Inline {
        agent.latest_inline_plan_content = state.plan_content.clone();
    } else {
        agent.latest_inline_plan_content = None;
    }
    agent.plan_approval_view = Some(state);

    agent.casual_commenting_range = None;
    agent.casual_editing_comment_id = None;

    // Option D: force-modal opens the line-viewer immediately (fullscreen);
    // default soft park is toast + status + inline transcript card (option A/C),
    // with side-panel review on demand (option B via /view-plan / status).
    if force_modal {
        agent.reopen_plan_approval();
        // reopen opens the side panel by default; force-modal upgrades to
        // full takeover so the historical modal setting still hard-opens.
        if let Some(ref mut viewer) = agent.line_viewer {
            viewer.fullscreen = true;
            viewer.side_panel = false;
        }
        tracing::info!(
            target_active = is_active,
            "Force-modal plan approval from ext_method ([ui] plan_approval_park=modal)"
        );
    } else {
        // Non-blocking status chrome: toast + status label + transcript card.
        // Focus the Prompt **pane** and plan-approval **Prompt** focus so the
        // composer paints focused (caret, no "Build anything" dead look) and
        // typing lands immediately. Soft-park is **non-capturing** for Char
        // (all printable → composer); CTAs are mouse footer / status /
        // `/view-plan` panel only (L1 modal-free, 2026-07-29).
        agent.set_active_pane(crate::views::agent::ActivePane::Prompt, false);
        if let Some(ref mut pav) = agent.plan_approval_view {
            pav.focus = crate::views::plan_approval_view::PlanApprovalFocus::Prompt;
        }
        agent.commit_parked_plan_card();
        // Flush unsent draft so a hard kill after soft-park still recovers text.
        agent.persist_unsent_prompt_draft();
        agent.show_toast(PLAN_PARKED_TOAST);
        tracing::info!(
            target_active = is_active,
            "Soft-parked plan approval from ext_method (side panel on demand)"
        );
    }

    // Background-parked approval renders when the user switches to the session;
    // only the active view needs an immediate redraw.
    is_active
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
