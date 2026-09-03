//! Surmount / grok-oss fork (ours). Named tests in this file are
//! **contracts**, not optional chrome. Do not delete or weaken them on
//! recon / onto / import.
//!
//! Clickable Approve on an isolated present must not lose a Human-box
//! prompt. Unit tests that only call `AgentView::approve_plan` or
//! `handle_plan_feedback_key` do not cover that path: the operator
//! clicks the footer Approve hit rect, `AppView::handle_input` routes
//! the mouse, and `dispatch` paints Interject. Letter `a` / `A` types;
//! it is not Approve.
//!
//! Hunter's razor: an older unit test may require Approve to clear the
//! composer after consume. That wipe is not this contract. If the box
//! is empty after click, scrollback / Interject must still carry the
//! typed string. Leftover composer text without an implement payload is
//! also a miss (typed after an empty present, stash does not match).
//!
//! Docs leftover (do not edit those files from this slice):
//! `FORK.md` Land checklist and `doc/dev/upstream-regression-filters.md`
//! must catalog these function names.

use super::*;
use crate::app::actions::{Action, Effect};
use crate::app::app_view::InputOutcome;
use crate::scrollback::block::RenderBlock;
use crate::views::plan_approval_view::{
    PLAN_APPROVED_IMPLEMENT_MESSAGE, PLAN_APPROVED_REVIEW_COMMENTS_LEAD, PlanApprovalFocus,
    PlanPromptIntent,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

const HUMAN_BOX_PROMPT: &str = "keep this review comment after click approve";
const APPROVE_HIT: Rect = Rect {
    x: 10,
    y: 20,
    width: 8,
    height: 1,
};
const COMMENT_HIT: Rect = Rect {
    x: 30,
    y: 20,
    width: 8,
    height: 1,
};
const MODAL_AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

struct AfterClickApprove {
    interject_text: Option<String>,
    effects: Vec<Effect>,
}

fn isolated_present(
    app: &mut AppView,
    tool_call_id: &str,
    plan: &str,
) -> tokio::sync::oneshot::Receiver<xai_acp_lib::AcpResult<acp::ExtResponse>> {
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        seed_pending_tool(agent, tool_call_id, "CreatePlan");
        agent.pane_areas.prompt = Rect::new(0, 22, 80, 3);
    }
    let (ext, rx) = make_exit_plan_ext_with_tool_call_id(tool_call_id, Some(plan));
    assert!(
        handle_exit_plan_mode(ext, app),
        "isolated present must park the live waiter and dock the pane"
    );
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(agent.plan_approval_view.is_some());
    assert!(
        agent.line_viewer.is_some(),
        "isolated present must auto-open the plan side panel"
    );
    rx
}

fn arm_approve_hit_rect(app: &mut AppView) {
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    let viewer = agent.line_viewer.as_mut().expect("plan pane open");
    viewer.plan_mut().approve_button_area = Some(APPROVE_HIT);
    viewer.last_modal_area = Some(MODAL_AREA);
}

fn arm_comment_and_approve_hit_rects(app: &mut AppView) {
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    let viewer = agent.line_viewer.as_mut().expect("plan pane open");
    viewer.plan_mut().approve_button_area = Some(APPROVE_HIT);
    viewer.plan_mut().comment_button_area = Some(COMMENT_HIT);
    viewer.last_modal_area = Some(MODAL_AREA);
}

fn mouse_down(column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn dispatch_outcome(app: &mut AppView, outcome: InputOutcome) -> Vec<Effect> {
    match outcome {
        InputOutcome::Action(action) | InputOutcome::ActionThenForward(action) => {
            crate::app::dispatch::dispatch(action, app)
        }
        InputOutcome::ActionPair(first, second) => {
            let mut effects = crate::app::dispatch::dispatch(first, app);
            effects.extend(crate::app::dispatch::dispatch(second, app));
            effects
        }
        _ => Vec::new(),
    }
}

fn click_approve_via_app(app: &mut AppView) -> AfterClickApprove {
    arm_approve_hit_rect(app);
    let outcome = app.handle_input(&mouse_down(12, 20));
    let interject_text = match &outcome {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => Some(text.clone()),
        InputOutcome::ActionPair(Action::Interject { text, .. }, _)
        | InputOutcome::ActionPair(_, Action::Interject { text, .. }) => Some(text.clone()),
        _ => None,
    };
    let effects = dispatch_outcome(app, outcome);
    AfterClickApprove {
        interject_text,
        effects,
    }
}

fn type_into_human_box(app: &mut AppView, text: &str) {
    for ch in text.chars() {
        let ev = Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        let outcome = app.handle_input(&ev);
        assert!(
            !matches!(
                outcome,
                InputOutcome::Action(_)
                    | InputOutcome::ActionThenForward(_)
                    | InputOutcome::ActionPair(_, _)
            ),
            "typing in the Human box must not Approve or send, got {outcome:?}"
        );
    }
    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        agent.prompt.text().contains(text),
        "typed Human-box text must land in the composer, got {:?}",
        agent.prompt.text()
    );
}

fn user_prompt_texts(app: &AppView) -> Vec<String> {
    let agent = app.agents.get(&AgentId(0)).unwrap();
    (0..agent.scrollback.len())
        .filter_map(|i| match agent.scrollback.get(i).map(|e| &e.block) {
            Some(RenderBlock::UserPrompt(b)) => Some(b.text.clone()),
            _ => None,
        })
        .collect()
}

fn prompt_in_scrollback_or_interject(
    app: &AppView,
    interject_text: Option<&str>,
    needle: &str,
) -> bool {
    interject_text.is_some_and(|t| t.contains(needle))
        || user_prompt_texts(app).iter().any(|t| t.contains(needle))
        || last_interjection_text(&app.agents.get(&AgentId(0)).unwrap().scrollback)
            .is_some_and(|t| t.contains(needle))
}

fn wrapped_review_has_prompt(app: &AppView, interject_text: Option<&str>, needle: &str) -> bool {
    let lead = PLAN_APPROVED_REVIEW_COMMENTS_LEAD;
    let hit = |t: &str| t.contains(lead) && t.contains(needle);
    interject_text.is_some_and(hit)
        || user_prompt_texts(app).iter().any(|t| hit(t))
        || last_interjection_text(&app.agents.get(&AgentId(0)).unwrap().scrollback)
            .as_deref()
            .is_some_and(hit)
}

/// Composer OR Interject/scrollback must still hold the typed string.
/// Empty composer plus a bare `"approved"` ACP result plus the implement
/// sentence only is a fail: the prompt was lost.
fn assert_human_box_prompt_not_lost(app: &AppView, after: &AfterClickApprove, needle: &str) {
    let agent = app.agents.get(&AgentId(0)).unwrap();
    let composer = agent.prompt.text().to_string();
    let in_composer = composer.contains(needle);
    let in_payload =
        prompt_in_scrollback_or_interject(app, after.interject_text.as_deref(), needle);
    assert!(
        in_composer || in_payload,
        "click Approve must not drop the Human-box prompt; composer={composer:?} interject={:?} scrollback={:?}",
        after.interject_text,
        user_prompt_texts(app)
    );
    if composer.trim().is_empty() {
        assert!(
            in_payload,
            "composer was cleared; Interject or UserPrompt scrollback must carry {needle:?}; interject={:?} scrollback={:?}",
            after.interject_text,
            user_prompt_texts(app)
        );
        let implement_only =
            after.interject_text.as_deref().is_some_and(|t| {
                t.contains(PLAN_APPROVED_IMPLEMENT_MESSAGE) && !t.contains(needle)
            }) || user_prompt_texts(app)
                .iter()
                .any(|t| t.contains(PLAN_APPROVED_IMPLEMENT_MESSAGE) && !t.contains(needle));
        assert!(
            !implement_only,
            "empty composer + implement sentence without the typed prompt is a lost prompt; interject={:?} scrollback={:?}",
            after.interject_text,
            user_prompt_texts(app)
        );
    }
    let persist = agent.unsent_composer_draft_to_persist();
    assert!(
        in_composer || in_payload || !persist.contains(needle),
        "feedback_draft / unsent persist must not be the only home after the pane is taken down; persist={persist:?}"
    );
}

fn assert_prompt_sent_on_implement_turn(app: &AppView, after: &AfterClickApprove, needle: &str) {
    assert_human_box_prompt_not_lost(app, after, needle);
    assert!(
        after.effects.iter().any(|effect| matches!(
            effect,
            Effect::SendInterject { text, .. } if text.contains(needle)
        )),
        "dispatch must emit SendInterject carrying the typed prompt; effects={:?}",
        after.effects
    );
    assert!(
        prompt_in_scrollback_or_interject(app, after.interject_text.as_deref(), needle),
        "typed Human-box prompt must ride the implement turn (Interject / UserPrompt), not only leftover composer; composer={:?} interject={:?} scrollback={:?}",
        app.agents.get(&AgentId(0)).unwrap().prompt.text(),
        after.interject_text,
        user_prompt_texts(app)
    );
    assert!(
        wrapped_review_has_prompt(app, after.interject_text.as_deref(), needle),
        "implement payload must wrap review comments with {PLAN_APPROVED_REVIEW_COMMENTS_LEAD:?} plus the typed string; interject={:?} scrollback={:?}",
        after.interject_text,
        user_prompt_texts(app)
    );
}

fn assert_acp_approved_notes_not_in_feedback(
    mut rx: tokio::sync::oneshot::Receiver<xai_acp_lib::AcpResult<acp::ExtResponse>>,
) {
    let response = rx
        .try_recv()
        .expect("click Approve must complete the live exit_plan_mode waiter");
    let raw = response.expect("waiter response Ok");
    let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
    assert_eq!(
        parsed["outcome"], "approved",
        "ACP waiter must be approved; got {parsed:?}"
    );
    let feedback = parsed.get("feedback");
    assert!(
        feedback.is_none() || feedback == Some(&serde_json::Value::Null),
        "review notes ride Interject, not the ACP feedback field; got {parsed:?}"
    );
}

/// Surmount / grok-oss fork (ours). Named tests are contracts.
/// Case A: text already in the Human box at isolated present (stash match).
/// Click Approve via App handle_input, then dispatch. The prompt must not
/// vanish into an empty composer with no Interject copy.
#[test]
fn isolated_present_preview_click_approve_does_not_drop_human_box_prompt() {
    let mut app = make_app_with_agent("sess-1");
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text(HUMAN_BOX_PROMPT);
    }
    let rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nCase A\n",
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.prompt.text().contains(HUMAN_BOX_PROMPT),
            "present must keep the live composer draft"
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Preview),
            "isolated present starts Preview"
        );
    }

    let after = click_approve_via_app(&mut app);
    assert!(
        app.agents
            .get(&AgentId(0))
            .unwrap()
            .plan_approval_view
            .is_none(),
        "click Approve must decide the parked plan"
    );
    assert_human_box_prompt_not_lost(&app, &after, HUMAN_BOX_PROMPT);
    assert_prompt_sent_on_implement_turn(&app, &after, HUMAN_BOX_PROMPT);
    assert_acp_approved_notes_not_in_feedback(rx);
}

/// Surmount / grok-oss fork (ours). Named tests are contracts.
/// Case B: composer empty at present, then type in isolated Preview, then
/// click Approve. Stash does not match. The typed string must still go on
/// the implement turn. Leftover composer without Interject is a miss.
#[test]
fn isolated_present_preview_typed_after_present_click_approve_sends_human_box_prompt() {
    let mut app = make_app_with_agent("sess-1");
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
    }
    let rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nCase B\n",
    );
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.prompt.text().trim().is_empty(),
            "fixture: present with an empty Human box"
        );
        assert_eq!(
            agent.plan_approval_view.as_ref().map(|p| p.focus),
            Some(PlanApprovalFocus::Preview)
        );
    }
    type_into_human_box(&mut app, HUMAN_BOX_PROMPT);

    let after = click_approve_via_app(&mut app);
    assert!(
        app.agents
            .get(&AgentId(0))
            .unwrap()
            .plan_approval_view
            .is_none(),
        "click Approve must decide the parked plan"
    );
    assert_prompt_sent_on_implement_turn(&app, &after, HUMAN_BOX_PROMPT);
    assert_acp_approved_notes_not_in_feedback(rx);
}

/// Surmount / grok-oss fork (ours). Named tests are contracts.
/// Prompt-focused: Comment CTA (same as Tab to Prompt), type the same
/// Human-box string, click Approve through App handle_input + dispatch.
#[test]
fn isolated_present_prompt_focus_click_approve_does_not_drop_human_box_prompt() {
    let mut app = make_app_with_agent("sess-1");
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text("");
    }
    let rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nPrompt focus\n",
    );

    arm_comment_and_approve_hit_rects(&mut app);
    let comment_outcome = app.handle_input(&mouse_down(32, 20));
    assert!(
        !matches!(
            comment_outcome,
            InputOutcome::Action(Action::Interject { .. })
        ),
        "Comment CTA arms Prompt; it must not Approve, got {comment_outcome:?}"
    );
    let _ = dispatch_outcome(&mut app, comment_outcome);
    {
        let agent = app.agents.get(&AgentId(0)).unwrap();
        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("Comment CTA must leave the plan parked");
        assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
        assert_eq!(pav.prompt_intent, PlanPromptIntent::Comment);
    }

    type_into_human_box(&mut app, HUMAN_BOX_PROMPT);
    let after = click_approve_via_app(&mut app);
    assert!(
        app.agents
            .get(&AgentId(0))
            .unwrap()
            .plan_approval_view
            .is_none(),
        "click Approve must decide the parked plan"
    );
    assert_prompt_sent_on_implement_turn(&app, &after, HUMAN_BOX_PROMPT);
    assert_acp_approved_notes_not_in_feedback(rx);
}

/// Surmount / grok-oss fork (ours). Named tests are contracts.
/// The live event loop is handle_input then dispatch. Interject must
/// carry the typed prompt and paint UserPrompt / interjection_prompt
/// scrollback. ACP stays `"approved"` with no feedback field.
#[test]
fn isolated_present_click_approve_dispatches_interject_with_prompt_text() {
    let mut app = make_app_with_agent("sess-1");
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.prompt.set_text(HUMAN_BOX_PROMPT);
    }
    let rx = isolated_present(
        &mut app,
        "create-plan-call",
        "# Isolated plan.md\n\nDispatch Interject\n",
    );

    arm_approve_hit_rect(&mut app);
    let outcome = app.handle_input(&mouse_down(12, 20));
    let interject_text = match &outcome {
        InputOutcome::Action(Action::Interject { text, .. }) => {
            assert!(
                text.contains(HUMAN_BOX_PROMPT),
                "Interject action must carry the Human-box prompt, got {text:?}"
            );
            assert!(
                text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "Interject action must wrap review comments, got {text:?}"
            );
            text.clone()
        }
        other => panic!(
            "click Approve with a Human-box prompt must return Action::Interject; got {other:?}"
        ),
    };
    let effects = dispatch_outcome(&mut app, outcome);
    assert!(
        effects.iter().any(|effect| matches!(
            effect,
            Effect::SendInterject { text, .. } if text.contains(HUMAN_BOX_PROMPT)
        )),
        "dispatch must emit SendInterject carrying the typed prompt; got {effects:?}"
    );
    let after = AfterClickApprove {
        interject_text: Some(interject_text),
        effects,
    };
    assert_prompt_sent_on_implement_turn(&app, &after, HUMAN_BOX_PROMPT);
    assert_acp_approved_notes_not_in_feedback(rx);
}
