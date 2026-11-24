//! Planning must insert through the L1 composer (`AgentView.prompt: PromptWidget`).
//!
//! Revise submit is `send_plan_feedback` in `plan.rs`. That function is
//! `pub(crate)` and calls `send_cancelled`, not the approved-plan sentence.
//! This file does not invent a failing assert for that path.

use xai_grok_pager::views::plan_approval_view::PlanApprovalViewState;
use xai_grok_pager::views::prompt_widget::PromptWidget;

#[test]
fn planning_prompt_uses_the_same_input_as_the_l1_composer() {
    // L1 Operator box: `AgentView.prompt: PromptWidget`.
    // Planning keeps `feedback_draft: Option<String>` and
    // `stashed_feedback_prompt: Option<StashedPrompt>`.
    let state = PlanApprovalViewState::for_idle_decision(Some("# plan\n".to_string()));
    let composer = std::any::type_name::<PromptWidget>();
    let planning_draft = std::any::type_name_of_val(&state.feedback_draft);
    let planning_stash = std::any::type_name_of_val(&state.stashed_feedback_prompt);

    assert_eq!(
        planning_draft, composer,
        "planning feedback_draft must be the L1 composer PromptWidget, not a separate buffer. A typed sentence must land once, in order, and the letter a must stay the letter a"
    );
    assert_eq!(
        planning_stash, composer,
        "planning stashed_feedback_prompt must be the L1 composer PromptWidget, not a second buffer"
    );
}
