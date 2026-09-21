//! Mouse-routing tests for the line viewer's plan preview: the scrollbar
//! must own a click+drag gesture end-to-end. A press on the track was
//! previously also treated as a comment-gutter anchor (row-only hit test),
//! so dragging the thumb selected plan lines for a comment instead of
//! scrolling (GB-4579: "can't click and drag scrollbar to view plan").

use std::path::Path;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::actions::ActionRegistry;
use crate::app::actions::Action;
use crate::app::agent::AgentState;
use crate::app::agent_view::AgentView;
use crate::app::agent_view::test_fixtures::make_agent;
use crate::app::app_view::InputOutcome;
use crate::views::file_search::line_viewer::{LineViewerKind, LineViewerState};
use crate::views::list_pane::InputBarMode;
use crate::views::plan_approval_view::{
    PLAN_APPROVED_REVIEW_COMMENTS_LEAD, PlanApprovalFocus, PlanPromptIntent,
};

const POPUP: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 10,
};
/// Scrollbar track column as split off by the list pane render
/// (`maybe_split_for_scrollbar`): last column of the popup area.
const TRACK_X: u16 = 79;

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    })
}

/// Agent showing a plan-approval preview whose plan overflows the
/// viewport, with the render-time areas planted so mouse dispatch works.
fn agent_with_scrollable_plan() -> AgentView {
    let mut agent = make_agent();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    let plan: String = (1..=60).fold(String::new(), |mut acc, i| {
        acc.push_str(&format!("step {i}\n"));
        acc
    });
    let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
        session_id: "test-session".into(),
        tool_call_id: "call-1".into(),
        plan_content: Some(plan),
    };
    agent.plan_approval_view = Some(
        crate::views::plan_approval_view::PlanApprovalViewState::new(
            request,
            crate::views::prompt_widget::StashedPrompt {
                text: String::new(),
                cursor: 0,
                images: Vec::new(),
                chip_elements: Vec::new(),
                image_counter: 0,
                image_undo_stash: Vec::new(),
            },
            tx,
        ),
    );
    agent.show_plan_preview();

    let viewer = agent
        .line_viewer
        .as_mut()
        .expect("plan preview opens the line viewer");
    viewer.prepare_layout(POPUP.width, POPUP.height);
    viewer.last_popup_area = Some(POPUP);
    viewer.last_modal_area = Some(Rect::new(0, 0, 80, 12));
    viewer
        .list_state
        .set_scrollbar_area(Some(Rect::new(TRACK_X, POPUP.y, 1, POPUP.height)));
    assert!(
        viewer.list_state.total_height() > POPUP.height as usize,
        "plan must overflow the viewport so the scrollbar is live"
    );
    agent
}

/// Presses on the modal border column next to the track (users read the
/// thumb + border as one two-column scrollbar) used to fall into the
/// click-outside-modal path instead of grabbing the thumb.
#[test]
fn border_column_press_grabs_scrollbar() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X + 1, 5),
        &registry,
    );

    let viewer = agent.line_viewer.as_ref().expect("viewer stays open");
    assert!(
        viewer.list_state.is_scrollbar_dragging(),
        "press one column right of the track (modal border) must grab the thumb"
    );
    assert!(
        viewer.list_state.scroll_offset() > 0,
        "the press must scroll toward the clicked track position"
    );
    assert!(
        viewer
            .plan_ref()
            .and_then(|p| p.gutter_drag_start)
            .is_none(),
        "a border-column press must not anchor a comment-gutter drag"
    );
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_eq!(pav.focus, PlanApprovalFocus::Preview);

    let offset_after_press = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .scroll_offset();
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Drag(MouseButton::Left), TRACK_X + 1, 9),
        &registry,
    );
    let viewer = agent.line_viewer.as_ref().unwrap();
    assert!(
        viewer.list_state.scroll_offset() > offset_after_press,
        "dragging on the border column must keep scrolling (offset {} -> {})",
        offset_after_press,
        viewer.list_state.scroll_offset()
    );
}

#[test]
fn gap_column_press_grabs_scrollbar() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X - 1, 5),
        &registry,
    );

    let viewer = agent.line_viewer.as_ref().unwrap();
    assert!(
        viewer.list_state.is_scrollbar_dragging(),
        "press on the gap column must grab the thumb"
    );
    assert!(
        viewer
            .plan_ref()
            .and_then(|p| p.gutter_drag_start)
            .is_none(),
        "a gap-column press must not anchor a comment-gutter drag"
    );
}

#[test]
fn border_column_press_does_not_close_casual_preview() {
    let mut agent = agent_with_scrollable_plan();
    agent.plan_approval_view = None;
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X + 1, 5),
        &registry,
    );

    let viewer = agent
        .line_viewer
        .as_ref()
        .expect("a border-column press must not close the casual preview");
    assert!(viewer.list_state.is_scrollbar_dragging());
}

#[test]
fn press_beyond_grab_zone_still_closes_casual_preview() {
    let mut agent = agent_with_scrollable_plan();
    agent.plan_approval_view = None;
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X + 2, 5),
        &registry,
    );

    assert!(
        agent.line_viewer.is_none(),
        "a click two columns right of the track is outside the modal and must close it"
    );
}

#[test]
fn scrollbar_press_does_not_enter_commenting() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X, 5),
        &registry,
    );

    let viewer = agent.line_viewer.as_ref().unwrap();
    assert!(
        viewer.list_state.is_scrollbar_dragging(),
        "press on the track must latch a scrollbar drag"
    );
    assert!(
        viewer
            .plan_ref()
            .and_then(|p| p.gutter_drag_start)
            .is_none(),
        "press on the track must not anchor a comment-gutter drag"
    );
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_eq!(
        pav.focus,
        PlanApprovalFocus::Preview,
        "press on the track must not enter commenting"
    );
}

#[test]
fn scrollbar_drag_scrolls_plan_instead_of_selecting_lines() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X, 2),
        &registry,
    );
    let offset_after_press = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .scroll_offset();

    // Drag the thumb to the bottom of the track.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Drag(MouseButton::Left), TRACK_X, 9),
        &registry,
    );

    let viewer = agent.line_viewer.as_ref().unwrap();
    assert!(
        viewer.list_state.scroll_offset() > offset_after_press,
        "dragging the thumb down must scroll the plan (offset {} -> {})",
        offset_after_press,
        viewer.list_state.scroll_offset()
    );
    assert!(
        viewer.plan_ref().and_then(|p| p.gutter_drag_end).is_none(),
        "thumb drag must not extend a comment line selection"
    );

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Up(MouseButton::Left), TRACK_X, 9),
        &registry,
    );
    let viewer = agent.line_viewer.as_ref().unwrap();
    assert!(
        !viewer.list_state.is_scrollbar_dragging(),
        "release must end the scrollbar drag"
    );
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_eq!(
        pav.commenting_range, None,
        "releasing the thumb must not open a comment on the dragged lines"
    );
    assert_eq!(pav.focus, PlanApprovalFocus::Preview);
}

/// The thumb must keep following the pointer when a drag drifts off the
/// popup rect (standard scrollbar behavior in every toolkit).
#[test]
fn scrollbar_drag_outside_popup_keeps_scrolling() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X, 8),
        &registry,
    );
    let offset_after_press = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .scroll_offset();
    assert!(offset_after_press > 0, "press near the bottom scrolls down");

    // Pointer drifts left of the track and above the popup while dragging.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Drag(MouseButton::Left), 40, 0),
        &registry,
    );

    let viewer = agent.line_viewer.as_ref().unwrap();
    assert!(
        viewer.list_state.scroll_offset() < offset_after_press,
        "drag toward the top of the track must scroll back up (offset {} -> {})",
        offset_after_press,
        viewer.list_state.scroll_offset()
    );
    assert!(
        viewer.plan_ref().and_then(|p| p.gutter_drag_end).is_none(),
        "scrollbar drag must never turn into a comment line selection"
    );
}

/// A gutter line-selection whose Up was lost must not survive a later
/// scrollbar gesture: the track press drops the stale anchor, so a stray
/// release afterwards cannot commit the leftover lines as a comment.
#[test]
fn scrollbar_gesture_drops_stale_gutter_anchor() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    // Anchor + extend a comment line selection, then lose the Up.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 10, 4),
        &registry,
    );
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Drag(MouseButton::Left), 10, 6),
        &registry,
    );
    {
        let viewer = agent.line_viewer.as_ref().unwrap();
        let start = viewer.plan_ref().and_then(|p| p.gutter_drag_start);
        let end = viewer.plan_ref().and_then(|p| p.gutter_drag_end);
        assert!(
            start.is_some() && end.is_some() && start != end,
            "precondition: a multi-line gutter drag is live (start {start:?}, end {end:?})"
        );
    }
    // Scrollbar click + release: the track press must drop the stale anchor.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X, 5),
        &registry,
    );
    {
        let viewer = agent.line_viewer.as_ref().unwrap();
        assert!(viewer.list_state.is_scrollbar_dragging());
        assert!(
            viewer
                .plan_ref()
                .and_then(|p| p.gutter_drag_start)
                .is_none()
                && viewer.plan_ref().and_then(|p| p.gutter_drag_end).is_none(),
            "track press must drop a stale comment-gutter anchor"
        );
    }
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Up(MouseButton::Left), TRACK_X, 5),
        &registry,
    );

    // The track press also discarded the in-progress comment draft
    // (same rule as clicking back into the modal).
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_eq!(pav.commenting_range, None);
    assert_eq!(pav.focus, PlanApprovalFocus::Preview);

    // A stray release on content must not commit the leftover lines.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Up(MouseButton::Left), 10, 6),
        &registry,
    );
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_eq!(
        pav.commenting_range, None,
        "stale gutter lines must not be committed as a comment range"
    );
    assert_eq!(
        pav.focus,
        PlanApprovalFocus::Preview,
        "a stray release must not re-enter commenting"
    );
}

/// A single click on a plan body row focuses or scrolls. It must not
/// enter Commenting or wipe the composer.
#[test]
fn plan_row_click_does_not_enter_commenting() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("keep typing");
    agent.prompt.set_cursor(agent.prompt.text().len());
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 10, 4),
        &registry,
    );

    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_ne!(
        pav.focus,
        PlanApprovalFocus::Commenting,
        "clicking a plan row must not steal the composer into Commenting"
    );
    assert_eq!(
        agent.prompt.text(),
        "keep typing",
        "clicking a plan row must leave the composer typeable"
    );

    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        &registry,
    );
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_ne!(
        pav.focus,
        PlanApprovalFocus::Commenting,
        "a live Preview draft must type `c`, not stash-and-wipe into Commenting"
    );
    assert_eq!(
        agent.prompt.text(),
        "keep typingc",
        "typed `c` must stay in the Human box, got {:?}",
        agent.prompt.text()
    );
}

/// Idle or cancelling plan present must not steal `x`/`e`/`j`/`k` into list
/// capture. Empty Enter never Approves. Clickable CTAs stay.
#[test]
fn plan_present_xejk_type_in_human_box_even_while_cancelling() {
    use crate::app::agent::AgentState;
    use crate::app::app_view::InputOutcome;
    use crate::app::queue_edit::PromptMode;

    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    agent.session.state = AgentState::TurnCancelling;
    agent.prompt_mode = PromptMode::EditingQueued {
        id: 1,
        original: "queued #1".into(),
        server_id: None,
        kind: crate::app::agent::QueueEntryKind::Prompt,
    };
    let registry = ActionRegistry::defaults();
    let pane_before = plan_pane_nav(&agent);

    for ch in ['x', 'e', 'j', 'k'] {
        let _ = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
            &registry,
        );
    }
    assert_eq!(
        agent.prompt.text(),
        "xejk",
        "idle/cancelling plan present must type x/e/j/k in the Human box, got {:?}",
        agent.prompt.text()
    );
    assert_eq!(
        plan_pane_nav(&agent),
        pane_before,
        "those letters must not walk the plan list"
    );
    assert!(
        agent.plan_approval_view.is_some(),
        "clickable plan CTAs must stay"
    );

    agent.prompt.set_text("");
    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &registry,
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "empty Enter must never Approve"
    );

    let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    let outcome = agent.handle_input(&ctrl_c, &registry);
    assert!(
        matches!(
            outcome,
            InputOutcome::Action(crate::app::actions::Action::CancelTurn)
        ),
        "queue #1 plus plan row editor plus Cancelling must still stop, got {outcome:?}"
    );
}

/// Empty Enter on the default parked Preview stays on Preview.
/// Commenting is explicit `c` only.
#[test]
fn empty_enter_on_soft_park_preview_does_not_enter_commenting() {
    let mut agent = agent_with_scrollable_plan();
    let viewer = agent.line_viewer.as_ref().expect("preview is open");
    assert!(
        viewer.selected_line_range().is_some(),
        "fixture must have a selected line so Enter would enter Commenting if routed there"
    );
    assert!(agent.prompt.text().trim().is_empty());
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_eq!(pav.focus, PlanApprovalFocus::Preview);

    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );

    let pav = agent
        .plan_approval_view
        .as_ref()
        .expect("empty Enter must leave the parked plan open");
    assert_eq!(
        pav.focus,
        PlanApprovalFocus::Preview,
        "empty Enter on Preview must not enter Commenting"
    );
    assert!(
        !agent.plan_decision_resolved,
        "empty Enter must never Approve a parked plan"
    );
}

/// A lost mouse-up after a track press must not make the next plan-line
/// click skip gutter / click-to-comment (sticky `is_scrollbar_dragging`).
#[test]
fn lost_scrollbar_up_does_not_block_next_line_click() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X, 5),
        &registry,
    );
    assert!(
        agent
            .line_viewer
            .as_ref()
            .unwrap()
            .list_state
            .is_scrollbar_dragging(),
        "precondition: track press latched a thumb drag"
    );

    // No Up — simulate a dropped release, then click a plan line.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 10, 4),
        &registry,
    );

    let viewer = agent.line_viewer.as_ref().unwrap();
    assert!(
        !viewer.list_state.is_scrollbar_dragging(),
        "content Down must clear the stale scrollbar latch"
    );
    assert!(
        viewer
            .plan_ref()
            .and_then(|p| p.gutter_drag_start)
            .is_some(),
        "content Down must still anchor a comment-gutter drag"
    );
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_ne!(
        pav.focus,
        PlanApprovalFocus::Commenting,
        "content Down must not steal the composer into Commenting"
    );
}

#[test]
fn wheel_on_border_column_scrolls_plan() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), TRACK_X + 1, 9),
        &registry,
    );
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Up(MouseButton::Left), TRACK_X + 1, 9),
        &registry,
    );
    let off = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .scroll_offset();
    assert!(off > 0, "border click near track bottom scrolls down");

    agent.handle_scroll(-3, TRACK_X + 1, 5);
    let off_after = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .scroll_offset();
    assert!(
        off_after < off,
        "wheel-up on the border column must scroll up ({off} -> {off_after})"
    );
}

/// Overlay router is skipped: empty-composer Ctrl+C in the line viewer must
/// abandon plan approval, not return Changed and swallow the chord.
#[test]
fn line_viewer_empty_ctrl_c_abandons_plan_approval() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let outcome = agent.handle_line_viewer_key(&ctrl_c);
    assert!(
        matches!(
            outcome,
            crate::app::app_view::InputOutcome::Changed
                | crate::app::app_view::InputOutcome::Action(_)
        ),
        "empty Ctrl+C must be consumed as plan quit; got {outcome:?}"
    );
    assert!(
        agent.plan_approval_view.is_none(),
        "line-viewer empty Ctrl+C must abandon, not swallow as Changed"
    );
    assert!(
        agent.plan_decision_resolved,
        "line-viewer Ctrl+C abandon must set the same sticky as q / Quit"
    );
}

/// Non-empty composer: line-viewer Ctrl+C clears the draft first. Second
/// empty press then abandons.
#[test]
fn line_viewer_ctrl_c_clears_draft_then_second_abandons() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("draft notes");
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let first = agent.handle_line_viewer_key(&ctrl_c);
    assert!(
        matches!(first, crate::app::app_view::InputOutcome::Changed),
        "first Ctrl+C with draft must clear; got {first:?}"
    );
    assert!(
        agent.plan_approval_view.is_some(),
        "first Ctrl+C must not abandon while draft existed"
    );
    assert!(
        agent.prompt.text().is_empty(),
        "first Ctrl+C must clear composer draft"
    );

    let second = agent.handle_line_viewer_key(&ctrl_c);
    assert!(
        agent.plan_approval_view.is_none(),
        "second empty Ctrl+C must abandon; got {second:?}"
    );
    assert!(agent.plan_decision_resolved);
}

/// Isolated plan.md viewer: a mid-compose draft means `a` is text, not Approve.
#[test]
fn plan_md_preview_mid_compose_a_types_does_not_approve() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("oh you interrupted my typing");
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let a = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    let _ = agent.handle_input(&a, &ActionRegistry::defaults());
    assert!(
        agent.plan_approval_view.is_some(),
        "plan.md Preview must not Approve while the composer has a draft"
    );
    assert!(
        agent.prompt.text().contains("oh you interrupted my typing"),
        "draft must stay in the composer, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.prompt.text().contains('a'),
        "typed `a` must land in the composer, got {:?}",
        agent.prompt.text()
    );
}

/// Empty-prompt `a` on the isolated plan.md Preview path types.
#[test]
fn plan_md_preview_empty_a_still_approves() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let a = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    let _ = agent.handle_input(&a, &ActionRegistry::defaults());
    assert!(
        agent.plan_approval_view.is_some(),
        "empty-prompt `a` on plan.md Preview must type, not Approve"
    );
    assert_eq!(agent.prompt.text(), "a");
}

/// Isolated plan.md Preview is non-capturing: a non-accelerator letter types.
#[test]
fn plan_md_preview_empty_printable_goes_to_composer() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let h = Event::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
    let _ = agent.handle_input(&h, &ActionRegistry::defaults());
    assert!(
        agent.plan_approval_view.is_some(),
        "a non-accelerator letter must not decide the plan"
    );
    assert_eq!(
        agent.prompt.text(),
        "h",
        "printable keys go to the composer while plan.md is open, got {:?}",
        agent.prompt.text()
    );
}

/// Ctrl+Backspace deletes the previous word in the plan composer even
/// while Preview owns Tab/?/y.
#[test]
fn plan_md_preview_ctrl_backspace_deletes_word_in_composer() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("hello world");
    agent.prompt.set_cursor(agent.prompt.text().len());
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let chord = Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL));
    let _ = agent.handle_input(&chord, &ActionRegistry::defaults());
    assert_eq!(
        agent.prompt.text(),
        "hello ",
        "Ctrl+Backspace must word-delete in the plan composer, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.plan_approval_view.is_some(),
        "Ctrl+Backspace must not dismiss plan.md"
    );
}

fn plan_pane_nav(agent: &AgentView) -> (Option<usize>, usize) {
    let viewer = agent
        .line_viewer
        .as_ref()
        .expect("isolated present keeps plan.md open");
    (
        viewer.list_state.selected_index(),
        viewer.list_state.scroll_offset(),
    )
}

fn press_plan_key(agent: &mut AgentView, code: KeyCode, modifiers: KeyModifiers) {
    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(code, modifiers)),
        &ActionRegistry::defaults(),
    );
}

fn assert_plan_prompt_cursor_keys_stay_in_composer(
    agent: &AgentView,
    draft: &str,
    pane_before: (Option<usize>, usize),
    intent_before: PlanPromptIntent,
) {
    assert_eq!(
        agent.prompt.text(),
        draft,
        "cursor keys must not rewrite the Human box, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.plan_approval_view.is_some(),
        "cursor keys must not Approve or Exit the parked plan"
    );
    assert!(
        !agent.plan_decision_resolved,
        "cursor keys must not decide the plan"
    );
    let pav = agent
        .plan_approval_view
        .as_ref()
        .expect("plan review stays parked");
    assert_eq!(
        pav.prompt_intent, intent_before,
        "cursor keys must not arm Clarify or switch the box intent"
    );
    assert!(
        agent.active_modal.is_none(),
        "cursor keys must not open help or the command palette"
    );
    assert_eq!(
        plan_pane_nav(agent),
        pane_before,
        "cursor keys must not scroll or retarget the plan pane"
    );
}

/// Isolated plan.md Preview with a live Human-box draft: Left/Right move
/// the composer cursor, not the plan pane.
#[test]
fn plan_prompt_cursor_keys_preview_arrows() {
    const DRAFT: &str = "hello world";
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text(DRAFT);
    agent.prompt.set_cursor(DRAFT.len());
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    let pane_before = plan_pane_nav(&agent);
    let intent_before = agent.plan_approval_view.as_ref().unwrap().prompt_intent;
    let end = agent.prompt.cursor();

    press_plan_key(&mut agent, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(
        agent.prompt.cursor(),
        end.saturating_sub(1),
        "Left must move the Human box caret, got {}",
        agent.prompt.cursor()
    );
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);

    press_plan_key(&mut agent, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        agent.prompt.cursor(),
        end,
        "Right must move the Human box caret back, got {}",
        agent.prompt.cursor()
    );
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);
}

/// Same isolated Preview Human box: Ctrl-Left / Ctrl-Right move by word,
/// matching Ctrl+Backspace staying on that composer.
#[test]
fn plan_prompt_cursor_keys_preview_ctrl_arrows() {
    const DRAFT: &str = "hello world";
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text(DRAFT);
    agent.prompt.set_cursor(DRAFT.len());
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    let pane_before = plan_pane_nav(&agent);
    let intent_before = agent.plan_approval_view.as_ref().unwrap().prompt_intent;
    let end = agent.prompt.cursor();

    press_plan_key(&mut agent, KeyCode::Left, KeyModifiers::CONTROL);
    let after_word_left = agent.prompt.cursor();
    assert!(
        after_word_left < end,
        "Ctrl-Left must jump left by a word, cursor stayed at {after_word_left}"
    );
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);

    press_plan_key(&mut agent, KeyCode::Right, KeyModifiers::CONTROL);
    assert_eq!(
        agent.prompt.cursor(),
        end,
        "Ctrl-Right must jump right by a word, got {}",
        agent.prompt.cursor()
    );
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);
}

/// Same isolated Preview Human box: Ctrl-A / Ctrl-E are line start / end
/// in the composer, not help, Clarify, or plan-pane nav.
#[test]
fn plan_prompt_cursor_keys_preview_ctrl_a_e() {
    const DRAFT: &str = "hello world";
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text(DRAFT);
    agent.prompt.set_cursor(DRAFT.len());
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    let pane_before = plan_pane_nav(&agent);
    let intent_before = agent.plan_approval_view.as_ref().unwrap().prompt_intent;

    press_plan_key(&mut agent, KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(
        agent.prompt.cursor(),
        0,
        "Ctrl-A must go to the start of the Human box line, got {}",
        agent.prompt.cursor()
    );
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);

    press_plan_key(&mut agent, KeyCode::Char('e'), KeyModifiers::CONTROL);
    assert_eq!(
        agent.prompt.cursor(),
        DRAFT.len(),
        "Ctrl-E must go to the end of the Human box line, got {}",
        agent.prompt.cursor()
    );
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);
}

/// Tab has focused the plan prompt: the same cursor keys edit the box,
/// including when the isolated present Preview path is not the owner.
#[test]
fn plan_prompt_cursor_keys_tab_focus() {
    const DRAFT: &str = "hello world";
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text(DRAFT);
    agent.prompt.set_cursor(DRAFT.len());
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Prompt;
        pav.prompt_intent = PlanPromptIntent::Comment;
    }
    let pane_before = plan_pane_nav(&agent);
    let intent_before = agent.plan_approval_view.as_ref().unwrap().prompt_intent;
    let end = agent.prompt.cursor();

    press_plan_key(&mut agent, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(agent.prompt.cursor(), end.saturating_sub(1));
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Prompt,
        "arrows must not steal Tab focus back to the plan pane"
    );
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);

    press_plan_key(&mut agent, KeyCode::Left, KeyModifiers::CONTROL);
    assert!(agent.prompt.cursor() < end.saturating_sub(1));
    press_plan_key(&mut agent, KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(agent.prompt.cursor(), 0);
    press_plan_key(&mut agent, KeyCode::Char('e'), KeyModifiers::CONTROL);
    assert_eq!(agent.prompt.cursor(), DRAFT.len());
    press_plan_key(&mut agent, KeyCode::Right, KeyModifiers::CONTROL);
    assert_eq!(agent.prompt.cursor(), DRAFT.len());
    assert_plan_prompt_cursor_keys_stay_in_composer(&agent, DRAFT, pane_before, intent_before);
}

fn type_plan_chars(agent: &mut AgentView, text: &str) {
    let registry = ActionRegistry::defaults();
    for ch in text.chars() {
        let modifiers = if ch.is_uppercase() {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        };
        let _ = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Char(ch), modifiers)),
            &registry,
        );
    }
}

fn composer_undos_until_empty(agent: &mut AgentView) -> usize {
    let mut n = 0;
    while !agent.prompt.text().is_empty() && agent.prompt.textarea.can_undo() {
        assert!(
            agent.prompt.textarea.undo(),
            "can_undo was true but undo returned false"
        );
        n += 1;
        if n > 50 {
            break;
        }
    }
    n
}

fn composer_redo_n(agent: &mut AgentView, n: usize) {
    for _ in 0..n {
        assert!(
            agent.prompt.textarea.redo(),
            "redo must restore the draft we just undid"
        );
    }
}

/// Isolated Preview Human box types `c` unless Comment was clicked.
/// Empty-prompt `c` must not steal the first printable of a Human send.
#[test]
fn empty_preview_c_types_in_human_box() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    let pav = agent.plan_approval_view.as_ref().unwrap();
    assert_eq!(
        pav.focus,
        PlanApprovalFocus::Preview,
        "Isolated Preview must not arm Comment when Comment was not clicked"
    );
    assert!(
        pav.commenting_range.is_none(),
        "empty-prompt `c` must not arm a line range; Comment CTA still does"
    );
    assert_eq!(
        agent.prompt.text(),
        "c",
        "typed Isolated Preview `c` must land in the Human box, got {:?}",
        agent.prompt.text()
    );
}

/// Typed Preview/Prompt text must survive the letter `c`, a panel reopen, and
/// must not grow a wipe-to-empty undo frame (one accidental wipe used to need
/// several Ctrl-Z).
#[test]
fn plan_preview_typed_text_survives_c_reopen_without_wipe_undo() {
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("");
    agent.prompt.clear_history();

    type_plan_chars(&mut agent, "because");
    assert_eq!(
        agent.prompt.text(),
        "because",
        "Preview must type a word that contains `c`, not stash-and-wipe, got {:?}",
        agent.prompt.text()
    );
    assert_ne!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Commenting,
        "typing `c` inside a live draft must not enter Commenting"
    );

    let undos_before = composer_undos_until_empty(&mut agent);
    composer_redo_n(&mut agent, undos_before);
    assert_eq!(agent.prompt.text(), "because");

    agent.reopen_plan_approval();
    assert_eq!(
        agent.prompt.text(),
        "because",
        "reopen must not replace the live draft, got {:?}",
        agent.prompt.text()
    );

    let undos_after = composer_undos_until_empty(&mut agent);
    assert_eq!(
        undos_after, undos_before,
        "reopen must not push extra undo frames (a wipe-to-empty used to stack Ctrl-Z)"
    );
    composer_redo_n(&mut agent, undos_after);
    assert_eq!(agent.prompt.text(), "because");
}

/// Isolated Preview (footer `Tab:prompt`): Ctrl+Z must restore a wiped Human
/// box. The chord used to stay with the plan list, so undo never ran.
#[test]
fn plan_preview_ctrl_z_restores_wiped_human_box() {
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("");
    agent.prompt.clear_history();
    type_plan_chars(&mut agent, "please keep this prompt");
    assert_eq!(agent.prompt.text(), "please keep this prompt");

    press_plan_key(&mut agent, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(
        agent.prompt.text().is_empty(),
        "Ctrl+C must wipe the Human box first, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.plan_approval_view.is_some(),
        "first Ctrl+C is wipe, not Exit"
    );

    press_plan_key(&mut agent, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(
        agent.prompt.text(),
        "please keep this prompt",
        "Ctrl+Z while Preview is focused must restore the wiped Human box, got {:?}",
        agent.prompt.text()
    );
}

/// Tab-focused Prompt box: same Ctrl+Z restore after a wipe.
#[test]
fn plan_prompt_ctrl_z_restores_wiped_human_box() {
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Prompt;
        pav.prompt_intent = PlanPromptIntent::Comment;
    }
    agent.prompt.set_text("");
    agent.prompt.clear_history();
    type_plan_chars(&mut agent, "revise notes that vanished");
    press_plan_key(&mut agent, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(agent.prompt.text().is_empty());
    press_plan_key(&mut agent, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(
        agent.prompt.text(),
        "revise notes that vanished",
        "Ctrl+Z on the Prompt-focused Human box must restore the wipe, got {:?}",
        agent.prompt.text()
    );
}

/// Ctrl/Cmd+Z is composer undo even while the plan list owns Preview.
#[test]
fn plan_preview_key_treats_ctrl_z_as_composer_text() {
    let undo = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert!(
        super::plan_preview_key_is_composer_text(&undo),
        "Ctrl+Z must reach the Human box, not the plan list"
    );
    let redo = KeyEvent::new(
        KeyCode::Char('Z'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert!(
        super::plan_preview_key_is_composer_text(&redo),
        "Ctrl+Shift+Z redo must reach the Human box"
    );
    let fullscreen = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert!(
        !super::plan_preview_key_is_composer_text(&fullscreen),
        "Ctrl+F stays with the plan viewer"
    );
}

/// Operator: Shift+Enter in plan Preview must reach the Human box.
/// Overlay copy / clarify / approve must not steal it.
#[test]
fn plan_preview_key_treats_shift_enter_as_composer_text() {
    let shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
    assert!(
        super::plan_preview_key_is_composer_text(&shift_enter),
        "Shift+Enter must reach the Human box, not y:copy / ?:clarify / Approve"
    );
    let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
    assert!(
        super::plan_preview_key_is_composer_text(&alt_enter),
        "Alt+Enter must reach the Human box the same way Shift+Enter does"
    );
    let bare_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        super::plan_preview_key_is_composer_text(&bare_enter),
        "bare Enter must reach the Human box"
    );
}

/// Default composer: Shift+Enter inserts a newline in Preview, matching
/// the main Human box. The parked plan must stay; copy/clarify must not fire.
#[test]
fn plan_preview_shift_enter_inserts_newline_when_composer_multiline_on() {
    crate::appearance::cache::set_composer_multiline(true);
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.multiline_mode = false;
    agent.prompt.set_text("hello");
    agent.prompt.set_cursor(5);
    let pane_before = plan_pane_nav(&agent);
    let intent_before = agent.plan_approval_view.as_ref().unwrap().prompt_intent;
    press_plan_key(&mut agent, KeyCode::Enter, KeyModifiers::SHIFT);
    assert!(
        agent.prompt.text().contains('\n'),
        "Preview Shift+Enter must insert a newline when composer multiline is on, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.prompt.text().contains("hello"),
        "Preview Shift+Enter must keep the draft, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.plan_approval_view.is_some(),
        "Preview Shift+Enter must not Approve or Exit"
    );
    assert!(
        !agent.plan_decision_resolved,
        "Preview Shift+Enter must not decide the plan"
    );
    let pav = agent
        .plan_approval_view
        .as_ref()
        .expect("plan review stays parked");
    assert_eq!(
        pav.prompt_intent, intent_before,
        "Preview Shift+Enter must not arm Clarify"
    );
    assert!(
        agent.active_modal.is_none(),
        "Preview Shift+Enter must not open help or the command palette"
    );
    assert_eq!(
        plan_pane_nav(&agent),
        pane_before,
        "Preview Shift+Enter must not scroll or retarget the plan pane"
    );
    crate::appearance::cache::set_composer_multiline(true);
}

/// `[ui] composer_multiline = false`: Shift+Enter must not open a second
/// line in Preview. It sends like the main composer.
#[test]
fn plan_preview_shift_enter_sends_when_composer_multiline_off() {
    crate::appearance::cache::set_composer_multiline(false);
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.multiline_mode = false;
    agent.prompt.set_text("hello");
    agent.prompt.set_cursor(5);
    let outcome = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
        &ActionRegistry::defaults(),
    );
    assert!(
        !agent.prompt.text().contains('\n'),
        "Preview Shift+Enter must not insert a newline when composer multiline is off, got {:?}",
        agent.prompt.text()
    );
    match outcome {
        crate::app::app_view::InputOutcome::Action(crate::app::actions::Action::SendPrompt(
            text,
        )) => {
            assert_eq!(text, "hello", "Preview Shift+Enter must send the draft");
        }
        other => panic!("Preview Shift+Enter with composer multiline off must send, got {other:?}"),
    }
    crate::appearance::cache::set_composer_multiline(true);
}

/// Session Multiline on Isolated Preview idle: Enter Approves with notes.
/// Shift+Enter still inserts a newline. Empty Enter never Approves.
#[test]
fn plan_preview_session_multiline_shift_enter_sends() {
    crate::appearance::cache::set_composer_multiline(true);
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.multiline_mode = true;
    agent.prompt.set_text("hello");
    agent.prompt.set_cursor(5);
    let enter = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match enter {
        crate::app::app_view::InputOutcome::Action(crate::app::actions::Action::Interject {
            text,
            ..
        })
        | crate::app::app_view::InputOutcome::ActionThenForward(
            crate::app::actions::Action::Interject { text, .. },
        ) => {
            assert!(
                text.contains("hello"),
                "Isolated Preview idle plus notes plus Enter must Approve with those notes, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview idle plus a non-empty Operator box plus Enter must Approve with those notes; got {other:?}"
        ),
    }

    let mut shift = agent_with_scrollable_plan();
    {
        let pav = shift.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    shift.multiline_mode = true;
    shift.prompt.set_text("hello");
    shift.prompt.set_cursor(5);
    let outcome = shift.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
        &ActionRegistry::defaults(),
    );
    match outcome {
        crate::app::app_view::InputOutcome::Action(crate::app::actions::Action::SendPrompt(
            text,
        )) => {
            assert_eq!(
                text, "hello",
                "Preview Shift+Enter in session Multiline must send"
            );
        }
        other => panic!("Preview Shift+Enter in session Multiline must send, got {other:?}"),
    }
    crate::appearance::cache::set_composer_multiline(true);
}

/// Operator: typing, cancel, and interject stay responsive. A keystroke
/// burst must not fsync WAL N times and must not rewrite pending_prompts.
#[test]
fn plan_human_box_keystroke_burst_does_not_append_prompt_wal() {
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("");
    agent.prompt_wal_append_count.set(0);
    agent.pending_prompts_persist_count.set(0);
    type_plan_chars(&mut agent, "twelve chars!");
    assert_eq!(
        agent.prompt_wal_append_count.get(),
        0,
        "keystroke burst must not fsync WAL N times, got {} WAL appends",
        agent.prompt_wal_append_count.get()
    );
    assert_eq!(
        agent.pending_prompts_persist_count.get(),
        0,
        "keystroke burst must not rewrite pending_prompts.json, got {} snapshots",
        agent.pending_prompts_persist_count.get()
    );
}

/// Main composer shares the no-WAL-on-letters contract.
#[test]
fn main_composer_keystroke_burst_does_not_append_prompt_wal() {
    let mut agent = make_agent();
    agent.prompt.set_text("");
    agent.prompt_wal_append_count.set(0);
    agent.pending_prompts_persist_count.set(0);
    type_plan_chars(&mut agent, "hello world");
    assert_eq!(
        agent.prompt_wal_append_count.get(),
        0,
        "main prompt typing must not append prompt_wal.jsonl, got {} WAL appends",
        agent.prompt_wal_append_count.get()
    );
    assert_eq!(
        agent.pending_prompts_persist_count.get(),
        0,
        "main prompt typing must not rewrite pending_prompts.json, got {} snapshots",
        agent.pending_prompts_persist_count.get()
    );
}

/// A burst of Human-box keystrokes must not flush the unsent draft on every
/// character (that path used to `sync_all` per key).
#[test]
fn plan_human_box_keystroke_burst_does_not_flush_unsent_draft_every_char() {
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("");
    agent.unsent_draft_persist_flush_count.set(0);
    agent.unsent_draft_persist_skip_count.set(0);
    agent.last_unsent_draft_persist.set(None);

    type_plan_chars(&mut agent, "twelve chars!");
    let flushes = agent.unsent_draft_persist_flush_count.get();
    let skips = agent.unsent_draft_persist_skip_count.get();
    assert_eq!(
        flushes, 1,
        "a burst must write the unsent draft once, got {flushes} flushes and {skips} skips"
    );
    assert!(
        skips >= 12,
        "remaining keystrokes must coalesce, got {skips} skips and {flushes} flushes"
    );
}

/// Main composer (no plan pane) shares the coalesced persist path.
#[test]
fn main_composer_keystroke_burst_does_not_flush_unsent_draft_every_char() {
    let mut agent = make_agent();
    agent.prompt.set_text("");
    agent.unsent_draft_persist_flush_count.set(0);
    agent.unsent_draft_persist_skip_count.set(0);
    agent.last_unsent_draft_persist.set(None);
    type_plan_chars(&mut agent, "hello world");
    let flushes = agent.unsent_draft_persist_flush_count.get();
    let skips = agent.unsent_draft_persist_skip_count.get();
    assert_eq!(
        flushes, 1,
        "main prompt typing must not persist every character, got {flushes} flushes and {skips} skips"
    );
    assert!(
        skips >= 10,
        "burst after the first key must skip, got {skips} skips"
    );
}

/// Tab leaving Commenting must restore the pre-comment Human-box draft, not
/// leave an empty wipe that takes several Ctrl-Z to undo.
#[test]
fn tab_leave_commenting_restores_stashed_composer() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("live draft");
    agent.prompt.set_cursor(10);
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }

    let _ = agent.enter_plan_commenting();
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Commenting
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "entering Commenting clears the box for the line note, got {:?}",
        agent.prompt.text()
    );
    type_plan_chars(&mut agent, "nit");
    assert_eq!(agent.prompt.text(), "nit");

    let tab = Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let _ = agent.handle_input(&tab, &ActionRegistry::defaults());
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Preview
    );
    assert_eq!(
        agent.prompt.text(),
        "live draft",
        "leaving Commenting without save must restore the stashed Human box, got {:?}",
        agent.prompt.text()
    );
}

/// Operator: `y` copies the plan while the comment overlay is open.
#[test]
fn y_copies_the_plan_while_the_comment_overlay_is_open() {
    let mut agent = agent_with_scrollable_plan();
    let _ = agent.enter_plan_commenting();
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Commenting
    );
    type_plan_chars(&mut agent, "line note");
    agent.toast = None;
    press_plan_key(&mut agent, KeyCode::Char('y'), KeyModifiers::NONE);
    assert_eq!(
        agent.prompt.text(),
        "line note",
        "y while commenting copies the plan; it must not type y into the line comment, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.toast.is_some(),
        "y while commenting must copy the plan (clipboard toast)"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "y while commenting must not Approve or Exit"
    );
}

/// Operator: clickable copy control copies the plan.
#[test]
fn plan_approval_copy_button_click_copies_the_plan() {
    let mut agent = agent_with_scrollable_plan();
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().copy_button_area = Some(Rect::new(2, 11, 6, 1));
    }
    agent.toast = None;
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 3, 11),
        &ActionRegistry::defaults(),
    );
    assert!(
        agent.toast.is_some(),
        "clicking copy must copy the plan (clipboard toast)"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "copy click must not Approve or Exit"
    );
}

/// Operator: Enter while composing a comment still saves the comment.
/// Footer is `Enter:save comment`. Session Multiline must not turn that
/// Enter into a newline.
#[test]
fn enter_while_composing_a_comment_still_saves_the_comment() {
    crate::appearance::cache::set_composer_multiline(true);
    let mut agent = agent_with_scrollable_plan();
    agent.multiline_mode = true;
    let _ = agent.enter_plan_commenting();
    type_plan_chars(&mut agent, "keep this line note");
    press_plan_key(&mut agent, KeyCode::Enter, KeyModifiers::NONE);
    let pav = agent.plan_approval_view.as_ref().expect("plan stays");
    assert_eq!(pav.focus, PlanApprovalFocus::Preview);
    assert!(
        pav.comments
            .iter()
            .any(|c| c.text.contains("keep this line note")),
        "Enter while commenting must save the line comment; got {:?}",
        pav.comments
    );
    assert!(
        !agent.plan_decision_resolved,
        "saving a line comment must not Approve"
    );
    crate::appearance::cache::set_composer_multiline(true);
}

/// Operator: empty Enter never Approves, even when Approve is marked.
#[test]
fn empty_enter_never_approves_even_when_approve_is_marked() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Approve);
    }
    press_plan_key(&mut agent, KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "empty Enter must never Approve a parked plan"
    );
}

/// Operator: Enter submits the marked idle CTA.
#[test]
fn enter_submits_the_marked_idle_cta() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Exit);
    }
    press_plan_key(&mut agent, KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        agent.plan_approval_view.is_none(),
        "Enter on marked Exit must abandon the parked plan"
    );
}

/// Operator: click marks a CTA and runs it; first click on Approve still
/// Approves. Comment focuses the composer. Exit abandons.
#[test]
fn click_selects_a_cta_and_first_click_approve_still_submits() {
    let mut agent = agent_with_scrollable_plan();
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().comment_button_area = Some(Rect::new(20, 11, 8, 1));
        viewer.plan_mut().approve_button_area = Some(Rect::new(10, 11, 8, 1));
        viewer.last_modal_area = Some(Rect::new(0, 0, 80, 12));
    }
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 22, 11),
        &ActionRegistry::defaults(),
    );
    let selected = agent
        .line_viewer
        .as_ref()
        .and_then(|v| v.plan_ref())
        .and_then(|p| p.selected_cta);
    assert_eq!(
        selected,
        Some(crate::views::file_search::line_viewer::SelectedPlanCta::Comment),
        "clicking Comment must mark it selected"
    );
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Prompt,
        "first Comment click focuses the composer"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "first Comment click must not Approve or Exit"
    );
    press_plan_key(&mut agent, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Prompt,
        "Enter on marked Comment keeps the composer focused"
    );

    let mut agent = agent_with_scrollable_plan();
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().abandon_button_area = Some(Rect::new(40, 11, 6, 1));
        viewer.last_modal_area = Some(Rect::new(0, 0, 80, 12));
    }
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 42, 11),
        &ActionRegistry::defaults(),
    );
    assert!(
        agent.plan_approval_view.is_none(),
        "first Exit click must abandon the parked plan"
    );

    let mut agent = agent_with_scrollable_plan();
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().approve_button_area = Some(Rect::new(10, 11, 8, 1));
        viewer.last_modal_area = Some(Rect::new(0, 0, 80, 12));
    }
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 12, 11),
        &ActionRegistry::defaults(),
    );
    assert!(
        agent.plan_decision_resolved || agent.plan_approval_view.is_none(),
        "first click on Approve must still Approve"
    );
}

/// Operator: second click on an already-selected CTA still submits.
#[test]
fn second_click_on_already_selected_cta_still_submits() {
    let mut agent = agent_with_scrollable_plan();
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().abandon_button_area = Some(Rect::new(40, 11, 6, 1));
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Exit);
        viewer.last_modal_area = Some(Rect::new(0, 0, 80, 12));
    }
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 42, 11),
        &ActionRegistry::defaults(),
    );
    assert!(
        agent.plan_approval_view.is_none(),
        "second click on already-selected Exit must still submit Exit"
    );
}

/// Operator: letter keys type; they are not the only submit.
#[test]
fn letter_key_types_and_is_not_the_only_submit() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    press_plan_key(&mut agent, KeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(
        agent.prompt.text(),
        "a",
        "letter a types; it must not Approve, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "letters are not the only submit"
    );
}

/// Isolated Preview idle plus a non-empty Operator paste plus Enter
/// Approves with those notes. It does not Plan-Exit and leave the paste.
/// Empty Enter never Approves.
#[test]
fn isolated_preview_idle_non_empty_operator_paste_enter_approves_with_notes_not_plan_exit() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;
    use crate::views::prompt_widget::KIND_PASTE;

    let paste: String = (1..=15)
        .map(|i| format!("line {i} of the review notes"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("");
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Exit);
    }
    let _ = agent.handle_input(&Event::Paste(paste.clone()), &ActionRegistry::defaults());
    assert!(
        agent.prompt.text().contains("line 1 of the review notes"),
        "15-line paste must land in the Operator box, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent
            .prompt
            .textarea
            .elements()
            .iter()
            .any(|e| e.kind == KIND_PASTE),
        "15-line paste must fold into a paste chip"
    );
    agent.prompt.set_cursor(0);
    assert!(
        agent.prompt.paste_element_at_cursor().is_some(),
        "caret on the paste chip must still Approve with those notes"
    );

    let send = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match send {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains("line 1 of the review notes")
                    && text.contains("line 15 of the review notes"),
                "Enter must Approve with the pasted notes, got {text:?}"
            );
            assert!(
                text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "Approve with comment must wrap the paste as review notes, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview idle plus a non-empty Operator paste plus Enter must Approve with those notes; got {other:?}"
        ),
    }
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Enter with pasted notes must Approve, not Plan-Exit"
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer clears only after Approve lands, got {:?}",
        agent.prompt.text()
    );

    let mut empty = agent_with_scrollable_plan();
    empty.prompt.set_text("");
    let empty_enter = empty.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    assert!(
        !matches!(
            empty_enter,
            InputOutcome::Action(Action::SendPrompt(_))
                | InputOutcome::Action(Action::SendPromptNow { .. })
                | InputOutcome::Action(Action::Interject { .. })
        ),
        "empty Enter never Approves and must not send; got {empty_enter:?}"
    );
    assert!(
        empty.plan_approval_view.is_some() && !empty.plan_decision_resolved,
        "empty Enter never Approves"
    );
}

/// Isolated Preview present plus `[Pasted: 13 lines]` whose first line is
/// `/implement --effort 3` plus Enter is Approve-with-comment. It is not
/// Plan Exit and must not leave the chip sitting. Empty Enter never
/// Approves. Typed `/implement` without a paste chip is still a slash.
fn pasted_13_lines_implement_chip_body() -> String {
    let mut lines = vec!["/implement --effort 3".to_string()];
    lines.extend((2..=13).map(|i| format!("line {i} of the pasted review")));
    lines.join("\n")
}

fn pasted_13_lines_chip_label(agent: &AgentView) -> String {
    use crate::views::prompt_widget::KIND_PASTE;
    agent
        .prompt
        .textarea
        .elements()
        .iter()
        .find(|e| e.kind == KIND_PASTE)
        .and_then(|e| e.display.as_ref())
        .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
        .unwrap_or_default()
}

fn arm_isolated_preview_exit_marked(agent: &mut AgentView) {
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("");
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Exit);
    }
}

fn paste_isolated_preview_13_line_implement_chip(agent: &mut AgentView) -> String {
    use crate::views::prompt_widget::KIND_PASTE;
    let paste = pasted_13_lines_implement_chip_body();
    let _ = agent.handle_input(&Event::Paste(paste.clone()), &ActionRegistry::defaults());
    assert!(
        agent.prompt.text().starts_with("/implement --effort 3"),
        "13-line paste must land in the Operator box, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent
            .prompt
            .textarea
            .elements()
            .iter()
            .any(|e| e.kind == KIND_PASTE),
        "13-line paste must fold into a [Pasted: 13 lines] chip"
    );
    assert_eq!(
        pasted_13_lines_chip_label(agent),
        "[Pasted: 13 lines]",
        "folded chip must paint [Pasted: 13 lines]"
    );
    agent.prompt.set_cursor(0);
    assert!(
        agent.prompt.paste_element_at_cursor().is_some(),
        "caret on the [Pasted: 13 lines] chip must still Approve with those notes"
    );
    paste
}

#[test]
fn isolated_preview_pasted_13_lines_implement_chip_enter_approves_with_comment_not_plan_exit() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;

    let mut typed_slash = agent_with_scrollable_plan();
    arm_isolated_preview_exit_marked(&mut typed_slash);
    typed_slash.prompt.set_text("/implement --effort 3");
    assert!(
        !typed_slash.isolated_preview_idle_enter_approves_with_notes(),
        "typed /implement without a [Pasted: 13 lines] chip is still a slash, not Approve-with-comment"
    );

    let mut agent = agent_with_scrollable_plan();
    arm_isolated_preview_exit_marked(&mut agent);
    let paste = paste_isolated_preview_13_line_implement_chip(&mut agent);
    assert!(
        agent.isolated_preview_idle_enter_approves_with_notes(),
        "Isolated Preview plus [Pasted: 13 lines] must Approve-with-comment even when the body starts with /implement"
    );

    let send = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match send {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains("/implement --effort 3")
                    && text.contains("line 13 of the pasted review"),
                "Enter on [Pasted: 13 lines] must Approve with the paste body, got {text:?}"
            );
            assert!(
                text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "Isolated Preview [Pasted: 13 lines] Enter is Approve-with-comment, not Plan Exit, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview plus [Pasted: 13 lines] plus Enter must Approve-with-comment, not Plan Exit; got {other:?}"
        ),
    }
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Enter on [Pasted: 13 lines] must Approve, not Plan Exit"
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer clears only after Approve-with-comment lands, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.prompt.textarea.elements().is_empty(),
        "[Pasted: 13 lines] must leave because Approve landed, not because Enter expanded the chip"
    );
    let _ = paste;

    let mut empty = agent_with_scrollable_plan();
    empty.prompt.set_text("");
    let empty_enter = empty.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    assert!(
        !matches!(
            empty_enter,
            InputOutcome::Action(Action::SendPrompt(_))
                | InputOutcome::Action(Action::SendPromptNow { .. })
                | InputOutcome::Action(Action::Interject { .. })
        ),
        "empty Enter never Approves and must not send; got {empty_enter:?}"
    );
    assert!(
        empty.plan_approval_view.is_some() && !empty.plan_decision_resolved,
        "empty Enter never Approves"
    );
}

/// Isolated Preview present plus `[Pasted: 13 lines]` plus click Approve
/// is Approve-with-comment. It must not Approve empty and leave the chip,
/// and it must not Plan Exit.
#[test]
fn isolated_preview_pasted_13_lines_implement_chip_click_approve_is_approve_with_comment_not_plan_exit()
 {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;

    let mut agent = agent_with_scrollable_plan();
    arm_isolated_preview_exit_marked(&mut agent);
    let paste = paste_isolated_preview_13_line_implement_chip(&mut agent);
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().approve_button_area = Some(Rect::new(10, 11, 8, 1));
        viewer.last_modal_area = Some(Rect::new(0, 0, 80, 12));
    }
    let click = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 12, 11),
        &ActionRegistry::defaults(),
    );
    match click {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains("/implement --effort 3")
                    && text.contains("line 13 of the pasted review"),
                "click Approve on [Pasted: 13 lines] must carry the paste body, got {text:?}"
            );
            assert!(
                text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "click Approve on [Pasted: 13 lines] is Approve-with-comment, not Plan Exit, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview plus [Pasted: 13 lines] plus click Approve must Approve-with-comment; got {other:?}"
        ),
    }
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "click Approve on [Pasted: 13 lines] must Approve, not Plan Exit"
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer clears only after Approve-with-comment lands, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.prompt.textarea.elements().is_empty(),
        "[Pasted: 13 lines] must not sit after click Approve"
    );
    let _ = paste;
}

/// Expand on `[Pasted: 13 lines]` stays paste-again or double-click.
/// Enter on the chip is Isolated Preview Approve-with-comment, not expand.
#[test]
fn isolated_preview_pasted_13_lines_expand_stays_paste_again_or_double_click_enter_approves() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;
    use crate::views::prompt_widget::{KIND_PASTE, PromptStyle};
    use ratatui::buffer::Buffer;

    let paste = pasted_13_lines_implement_chip_body();

    let mut paste_again = agent_with_scrollable_plan();
    arm_isolated_preview_exit_marked(&mut paste_again);
    let _ = paste_isolated_preview_13_line_implement_chip(&mut paste_again);
    let again = paste_again.handle_input(&Event::Paste(paste.clone()), &ActionRegistry::defaults());
    assert!(
        !matches!(
            again,
            InputOutcome::Action(_)
                | InputOutcome::ActionThenForward(_)
                | InputOutcome::ActionPair(_, _)
        ),
        "paste-again on [Pasted: 13 lines] must expand, not Approve or Plan Exit; got {again:?}"
    );
    assert!(
        paste_again
            .prompt
            .textarea
            .elements()
            .iter()
            .all(|e| e.kind != KIND_PASTE),
        "paste-again must expand [Pasted: 13 lines] into plain text"
    );
    assert!(
        paste_again
            .prompt
            .text()
            .starts_with("/implement --effort 3")
            && paste_again
                .prompt
                .text()
                .contains("line 13 of the pasted review"),
        "paste-again must keep the 13-line body, got {:?}",
        paste_again.prompt.text()
    );
    assert!(
        paste_again.plan_approval_view.is_some() && !paste_again.plan_decision_resolved,
        "paste-again must not Plan Exit Isolated Preview"
    );

    let mut double = agent_with_scrollable_plan();
    arm_isolated_preview_exit_marked(&mut double);
    let _ = paste_isolated_preview_13_line_implement_chip(&mut double);
    let area = Rect::new(0, 22, 40, 4);
    double.pane_areas.prompt = area;
    let mut buf = Buffer::empty(area);
    let style = PromptStyle {
        chrome: false,
        vpad_top: 0,
        ..Default::default()
    };
    double.prompt.draw(&mut buf, area, None, &style, None, None);
    let ta = double.prompt.textarea_area();
    let click = crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: ta.x + 1,
        row: ta.y,
        modifiers: KeyModifiers::empty(),
    };
    double.prompt.handle_mouse(&click);
    assert!(
        double
            .prompt
            .textarea
            .elements()
            .iter()
            .any(|e| e.kind == KIND_PASTE),
        "single click must not expand [Pasted: 13 lines]"
    );
    double.prompt.handle_mouse(&click);
    assert!(
        double
            .prompt
            .textarea
            .elements()
            .iter()
            .all(|e| e.kind != KIND_PASTE),
        "double-click must expand [Pasted: 13 lines]"
    );
    assert!(
        double.prompt.text().starts_with("/implement --effort 3"),
        "double-click expand must keep the paste body, got {:?}",
        double.prompt.text()
    );

    let mut enter = agent_with_scrollable_plan();
    arm_isolated_preview_exit_marked(&mut enter);
    let _ = paste_isolated_preview_13_line_implement_chip(&mut enter);
    let send = enter.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match send {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD)
                    && text.contains("/implement --effort 3"),
                "Enter on [Pasted: 13 lines] is Approve-with-comment, not expand, got {text:?}"
            );
        }
        other => panic!(
            "Enter on [Pasted: 13 lines] must Approve-with-comment, not only expand; got {other:?}"
        ),
    }
    assert!(
        enter.prompt.text().trim().is_empty(),
        "Enter must not only expand [Pasted: 13 lines]; composer after Approve={:?}",
        enter.prompt.text()
    );
}

/// Keep-draft after closing Isolated Preview and typing a later prompt is
/// the next SendPrompt. It is not `[Pasted: 13 lines]` review notes.
#[test]
fn isolated_preview_keep_draft_after_close_later_prompt_is_send_prompt_not_pasted_13_lines_notes() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;

    const LATER: &str = "later operator prompt after Isolated Preview closed";
    let mut agent = agent_with_scrollable_plan();
    arm_isolated_preview_exit_marked(&mut agent);
    agent.cancel_line_viewer();
    assert!(
        agent.line_viewer.is_none(),
        "fixture: Isolated Preview pane closed"
    );
    agent.prompt.set_text(LATER);
    agent.reopen_plan_approval();
    assert!(
        agent
            .plan_approval_view
            .as_ref()
            .is_some_and(|pav| pav.keep_draft_is_next_operator_turn),
        "text typed while Isolated Preview was closed is keep-draft for the next Operator turn"
    );
    let outcome = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match outcome {
        InputOutcome::Action(Action::SendPrompt(text)) => {
            assert!(
                text.contains(LATER),
                "keep-draft after close must SendPrompt the later prompt, got {text:?}"
            );
            assert!(
                !text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "keep-draft after close is not [Pasted: 13 lines] review notes, got {text:?}"
            );
        }
        other => panic!(
            "keep-draft after closing Isolated Preview and typing a later prompt must SendPrompt, not Approve-with-comment; got {other:?}"
        ),
    }
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "keep-draft Enter must not Approve or Plan Exit"
    );
}

/// Operator: "Also approve with comment STILL does not work on the latest
/// commit and build you gave me yesterday." Isolated Preview idle: leftover
/// slash-palette `/` plus Operator notes plus Enter Approves with those
/// notes. Click Approve with notes and no keystroke `feedback_draft` also
/// rides those notes. Empty Enter never Approves.
#[test]
fn isolated_preview_idle_leftover_slash_plus_notes_enter_approves_with_comment() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;

    const NOTES: &str = "keep this review comment after leftover slash";
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
        pav.prompt_intent = PlanPromptIntent::Revise;
        pav.feedback_draft = None;
    }
    agent.prompt.set_text("/");
    agent.prompt.refresh_slash(&agent.session.models);
    agent.prompt.set_text(NOTES);
    if let Some(pav) = agent.plan_approval_view.as_mut() {
        pav.feedback_draft = None;
    }
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Exit);
    }
    let send = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match send {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains(NOTES),
                "leftover slash plus notes plus Enter must Approve with those notes, got {text:?}"
            );
            assert!(
                text.contains(PLAN_APPROVED_REVIEW_COMMENTS_LEAD),
                "Approve with comment must wrap the notes, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview idle leftover slash plus Operator notes plus Enter must Approve with those notes; got {other:?}"
        ),
    }
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Enter with notes must Approve, not Plan-Exit"
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "composer clears only after Approve lands, got {:?}",
        agent.prompt.text()
    );
}

/// Isolated Preview idle plus typed Operator notes plus Enter Approves
/// with those notes. Empty Enter never Approves.
#[test]
fn isolated_preview_human_text_enter_is_human_turn_not_only_plan_comment() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;

    const HUMAN: &str = "keep the join order from the archive index";
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("");
    type_plan_chars(&mut agent, HUMAN);
    let send = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match send {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains(HUMAN),
                "Isolated Preview idle plus notes plus Enter must Approve with those notes, got {text:?}"
            );
        }
        other => panic!(
            "Isolated Preview idle plus a non-empty Operator box plus Enter must Approve with those notes; got {other:?}"
        ),
    }
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Isolated Preview idle Enter with notes must Approve"
    );

    let mut empty = agent_with_scrollable_plan();
    empty.prompt.set_text("");
    let empty_enter = empty.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    assert!(
        !matches!(
            empty_enter,
            InputOutcome::Action(Action::SendPrompt(_))
                | InputOutcome::Action(Action::SendPromptNow { .. })
                | InputOutcome::Action(Action::Interject { .. })
        ),
        "empty Enter never Approves and must not send; got {empty_enter:?}"
    );
    assert!(
        empty.plan_approval_view.is_some() && !empty.plan_decision_resolved,
        "empty Enter never Approves"
    );
}

/// Comment CTA still focuses the Operator box. Non-empty Enter then
/// Approves with those notes. Empty Enter never Approves.
#[test]
fn isolated_preview_non_empty_enter_sends_while_ride_approve_chrome_visible() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;

    const HUMAN: &str = "keep the join order from the archive index";
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Prompt;
        pav.prompt_intent = PlanPromptIntent::Comment;
    }
    agent.prompt.set_text("");
    type_plan_chars(&mut agent, HUMAN);
    let first = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match first {
        InputOutcome::Action(Action::Interject { text, .. })
        | InputOutcome::ActionThenForward(Action::Interject { text, .. }) => {
            assert!(
                text.contains(HUMAN),
                "Comment notes plus Enter must Approve with those notes, got {text:?}"
            );
        }
        other => {
            panic!("Comment CTA then notes then Enter must Approve with those notes; got {other:?}")
        }
    }
    assert!(
        agent.plan_approval_view.is_none() || agent.plan_decision_resolved,
        "Comment notes plus Enter must Approve, not Plan-Exit"
    );
}

/// Keep-draft from before live present still SendPrompt. Isolated Preview
/// Human text typed after park is also SendPrompt (see
/// `isolated_preview_human_text_enter_is_human_turn_not_only_plan_comment`).
/// Empty Enter never Approves.
#[test]
fn isolated_preview_keep_draft_from_before_present_enter_sends_prompt() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;

    const DRAFT: &str = "oh you interrupted my typing";
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
        pav.prompt_intent = PlanPromptIntent::Revise;
        pav.stashed_prompt.text = DRAFT.to_string();
    }
    agent.prompt.set_text(DRAFT);
    let outcome = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match outcome {
        InputOutcome::Action(Action::SendPrompt(text)) => {
            assert!(
                text.contains(DRAFT),
                "keep-draft from before present must SendPrompt, got {text:?}"
            );
        }
        other => panic!("keep-draft from before present must SendPrompt, got {other:?}"),
    }
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "keep-draft Enter must not Approve"
    );
    assert_ne!(
        agent
            .plan_approval_view
            .as_ref()
            .and_then(|p| p.feedback_draft.as_deref()),
        Some(DRAFT),
        "keep-draft Enter must not stash the pre-present composer as review comments"
    );
}

fn arm_exclusive_covering_revise_and_exit(agent: &mut AgentView) {
    let viewer = agent.line_viewer.as_mut().expect("plan pane");
    viewer.fullscreen = true;
    viewer.plan_mut().send_button_area = Some(Rect::new(40, 20, 8, 1));
    viewer.plan_mut().abandon_button_area = Some(Rect::new(50, 20, 8, 1));
    viewer.last_modal_area = Some(Rect::new(0, 0, 80, 24));
}

/// Operator: "I can't even revise plans now... exit won't work too. it's
/// fucking stuck!!" Exclusive covering clickable Revise rewrites and
/// re-presents. Letter keys type; they do not steal Approve.
#[test]
fn exclusive_covering_revise_cta_rewrites_and_represents_cannot_revise_plans() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;
    use crate::views::plan_approval_view::PlanFeedbackInFlight;

    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("cannot revise plans: add auth");
    arm_exclusive_covering_revise_and_exit(&mut agent);
    let outcome = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 41, 20),
        &ActionRegistry::defaults(),
    );
    assert!(
        matches!(
            outcome,
            InputOutcome::Changed | InputOutcome::Action(Action::Interject { .. })
        ),
        "cannot revise plans: clickable Revise must run; got {outcome:?}"
    );
    assert!(
        agent.plan_approval_view.is_none(),
        "cannot revise plans: clickable Revise must send the rewrite"
    );
    assert_eq!(
        agent.plan_feedback_in_flight,
        Some(PlanFeedbackInFlight::Revising)
    );
    assert!(
        !agent.plan_decision_resolved,
        "cannot revise plans: Revise is not Approve and not Exit"
    );
    assert!(
        agent.line_viewer.as_ref().is_some_and(|v| v.fullscreen),
        "cannot revise plans: Isolated Preview stays until Esc, Exit, or Approve"
    );
    assert!(
        agent.line_viewer.as_ref().is_some_and(|v| v
            .plan_ref()
            .is_some_and(|p| !p.show_action_buttons && !p.feedback_active)),
        "cannot revise plans: rewrite-wait must not arm idle Approve on leftover body"
    );
}

/// Operator: "exit won't work too. it's fucking stuck!!" Clickable Exit
/// leaves exclusive covering. Empty Enter never Approves.
#[test]
fn exclusive_covering_exit_cta_leaves_plan_exit_will_not_work_stuck() {
    use crate::app::agent_view::KeyOwner;
    use crate::app::app_view::InputOutcome;

    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("");
    arm_exclusive_covering_revise_and_exit(&mut agent);
    let empty = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    assert!(
        !matches!(
            empty,
            InputOutcome::Action(_)
                | InputOutcome::ActionThenForward(_)
                | InputOutcome::ActionPair(_, _)
        ),
        "GitHub #122: empty Enter never Approves exclusive covering; got {empty:?}"
    );
    let outcome = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 51, 20),
        &ActionRegistry::defaults(),
    );
    assert!(
        matches!(outcome, InputOutcome::Changed | InputOutcome::Action(_)),
        "Exit will not work / stuck: clickable Exit must run; got {outcome:?}"
    );
    assert!(
        agent.plan_approval_view.is_none() && agent.plan_decision_resolved,
        "Exit will not work / stuck: clickable Exit must leave plan"
    );
    assert!(
        agent.line_viewer.is_none(),
        "Exit will not work / stuck: Exit must not keep Isolated Preview covering after abandon"
    );
    assert!(
        !matches!(agent.key_owner(), KeyOwner::LineViewer),
        "Exit will not work / stuck: leftover exclusive covering must not own keys"
    );
}

fn ctrl_c() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
}

fn seed_grok_models(agent: &mut AgentView) {
    use agent_client_protocol as acp;
    use std::sync::Arc;
    let insert = |agent: &mut AgentView, id: &str, name: &str| {
        let mid = acp::ModelId::new(Arc::from(id));
        agent.session.models.available.insert(
            mid.clone(),
            acp::ModelInfo::new(mid, name.to_string()).meta(
                serde_json::json!({ "supportsReasoningEffort": true })
                    .as_object()
                    .cloned(),
            ),
        );
    };
    insert(agent, "grok-4.6", "Grok 4.6");
    insert(agent, "grok-4.5", "Grok 4.5");
}

fn park_leftover_isolated_preview(agent: &mut AgentView) {
    let mut viewer = LineViewerState::open_markdown_content(
        "plan.md",
        "# Isolated Preview\nleftover after Plan Exit\n".to_string(),
        None,
    )
    .expect("leftover Isolated Preview fixture must open");
    viewer.kind = LineViewerKind::PlanPreview;
    agent.line_viewer = Some(viewer);
    agent.plan_approval_view = None;
    assert!(
        agent.is_plan_viewer(),
        "fixture must be leftover Isolated Preview"
    );
}

fn render_isolated_preview_title_bar(agent: &mut AgentView) -> Buffer {
    let full = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(full);
    let theme = crate::theme::Theme::current();
    let viewer = agent
        .line_viewer
        .as_mut()
        .expect("Isolated Preview must be open to paint the title bar");
    crate::views::file_search::line_viewer::render_line_viewer(
        &mut buf,
        full,
        viewer,
        Path::new("/tmp"),
        &theme,
        0,
    );
    buf
}

/// Operator: "ctrl-c should have cleared this prompt but instead it exited
/// the plan." Isolated Preview `handle_input` first Ctrl+C with text
/// clears. Isolated Preview stays.
#[test]
fn isolated_preview_handle_input_ctrl_c_with_text_clears_and_stays() {
    let mut agent = agent_with_scrollable_plan();
    agent
        .prompt
        .set_text("ctrl-c should have cleared this prompt");
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    let first = agent.handle_input(&ctrl_c(), &ActionRegistry::defaults());
    assert!(
        matches!(first, InputOutcome::Changed),
        "Operator: first Ctrl+C with a draft must clear, not Exit; got {first:?}"
    );
    assert!(
        agent.plan_approval_view.is_some() && agent.line_viewer.is_some(),
        "first Ctrl+C must not Exit Isolated Preview"
    );
    assert!(
        agent.prompt.text().is_empty(),
        "first Ctrl+C must clear the Isolated Preview composer; got {:?}",
        agent.prompt.text()
    );
    assert!(
        !matches!(first, InputOutcome::Action(Action::CancelTurn)),
        "first Ctrl+C with a draft must not CancelTurn"
    );
}

/// Operator: "ctrl-c in every prompt input always clears first, then
/// exits only when ctrl-c is issued again." Second empty Ctrl+C via
/// `handle_input` then exits Isolated Preview / abandons the plan.
#[test]
fn isolated_preview_handle_input_second_empty_ctrl_c_exits() {
    let mut agent = agent_with_scrollable_plan();
    agent.prompt.set_text("draft");
    let _ = agent.handle_input(&ctrl_c(), &ActionRegistry::defaults());
    assert!(agent.prompt.text().is_empty());
    assert!(agent.plan_approval_view.is_some());
    let second = agent.handle_input(&ctrl_c(), &ActionRegistry::defaults());
    assert!(
        agent.plan_approval_view.is_none(),
        "second empty Ctrl+C must Exit Isolated Preview / abandon; got {second:?}"
    );
    assert!(
        agent.line_viewer.is_none(),
        "second empty Ctrl+C must not keep Isolated Preview covering"
    );
}

/// Isolated Preview plus a running turn plus a draft: first Ctrl+C
/// clears and must not CancelTurn.
#[test]
fn isolated_preview_handle_input_running_turn_draft_ctrl_c_does_not_cancel_turn() {
    let mut agent = agent_with_scrollable_plan();
    agent.session.state = AgentState::TurnRunning;
    agent.prompt.set_text("do not cancel this turn");
    let first = agent.handle_input(&ctrl_c(), &ActionRegistry::defaults());
    assert!(
        !matches!(first, InputOutcome::Action(Action::CancelTurn)),
        "Isolated Preview + running turn + draft must not CancelTurn; got {first:?}"
    );
    assert!(
        agent.prompt.text().is_empty(),
        "first Ctrl+C must still clear; got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.plan_approval_view.is_some() && agent.line_viewer.is_some(),
        "first Ctrl+C must not Exit Isolated Preview while the turn is running"
    );
    assert_eq!(agent.session.state, AgentState::TurnRunning);
}

/// Leftover Isolated Preview after Plan Exit: first Ctrl+C with text
/// clears and stays. Second empty Ctrl+C leaves the parked viewer.
#[test]
fn leftover_isolated_preview_handle_input_ctrl_c_clears_then_exits() {
    let mut agent = make_agent();
    park_leftover_isolated_preview(&mut agent);
    agent.prompt.set_text("leftover draft");
    let first = agent.handle_input(&ctrl_c(), &ActionRegistry::defaults());
    assert!(
        matches!(first, InputOutcome::Changed),
        "leftover first Ctrl+C with draft must clear; got {first:?}"
    );
    assert!(
        agent.line_viewer.is_some() && agent.is_plan_viewer(),
        "leftover first Ctrl+C must not close Isolated Preview"
    );
    assert!(agent.prompt.text().is_empty());
    let second = agent.handle_input(&ctrl_c(), &ActionRegistry::defaults());
    assert!(
        agent.line_viewer.is_none(),
        "leftover second empty Ctrl+C must leave Isolated Preview; got {second:?}"
    );
}

/// Operator: "The last tab when only the single model is highlighted
/// should switch it, but it doesn't." Isolated Preview unique `/model`
/// Tab must SwitchModel now and must not RowWalk focus.
#[test]
fn isolated_preview_unique_model_tab_switches_now_does_not_rowwalk() {
    let mut agent = agent_with_scrollable_plan();
    seed_grok_models(&mut agent);
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    agent.prompt.set_text("/model Grok 4.6");
    agent.prompt.set_cursor("/model Grok 4.6".len());
    agent.prompt.refresh_slash(&agent.session.models);
    assert!(
        agent.prompt.slash_open(),
        "Isolated Preview unique /model dropdown must be open"
    );
    assert_eq!(agent.prompt.slash_snapshot().matches.len(), 1);
    let outcome = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    match outcome {
        InputOutcome::Action(Action::SwitchModel { model_id, effort }) => {
            use agent_client_protocol as acp;
            use std::sync::Arc;
            assert_eq!(model_id, acp::ModelId::new(Arc::from("grok-4.6")));
            assert_eq!(effort, None);
        }
        other => panic!(
            "Operator: Isolated Preview unique /model Tab must SwitchModel now, not {other:?}; prompt={:?}",
            agent.prompt.text()
        ),
    }
    assert!(
        agent.prompt.text().is_empty(),
        "composer must clear; got {:?}",
        agent.prompt.text()
    );
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Preview,
        "unique /model Tab must not RowWalk Isolated Preview focus"
    );
    assert!(
        agent.line_viewer.is_some() && agent.plan_approval_view.is_some(),
        "SwitchModel must not Exit Isolated Preview"
    );
}

/// Magnifying glass is clickable next to copy and the fullscreen arrow.
#[test]
fn isolated_preview_search_glass_clickable_next_to_copy_and_expand() {
    let mut agent = agent_with_scrollable_plan();
    let _buf = render_isolated_preview_title_bar(&mut agent);
    let search = agent
        .line_viewer
        .as_ref()
        .and_then(|v| v.plan_ref())
        .and_then(|p| p.search_button_area)
        .expect("glass must be a clickable hit target next to copy");
    let copy = agent
        .line_viewer
        .as_ref()
        .and_then(|v| v.plan_ref())
        .and_then(|p| p.copy_button_area)
        .expect("copy stays next to enlarge");
    assert_eq!(
        search.x + search.width,
        copy.x,
        "glass must sit immediately left of copy"
    );
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), search.x, search.y),
        &ActionRegistry::defaults(),
    );
    assert_eq!(
        agent.line_viewer.as_ref().unwrap().list_state.input_mode(),
        Some(InputBarMode::Search),
        "glass click must open LineViewerState search, not a second engine"
    );
}

/// Isolated Preview composer `/` stays slash. Glass opens search.
#[test]
fn isolated_preview_composer_slash_stays_slash_not_line_search() {
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().unwrap();
        pav.focus = PlanApprovalFocus::Preview;
    }
    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    assert_eq!(
        agent.prompt.text(),
        "/",
        "Isolated Preview composer `/` stays slash; got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent
            .line_viewer
            .as_ref()
            .is_some_and(|v| v.list_state.input_mode().is_none()),
        "composer `/` must not open line-viewer search"
    );
}

/// After glass search is accepted, Isolated Preview n/N jump hits and
/// must not type into the Operator box.
#[test]
fn isolated_preview_handle_input_n_jumps_hits_after_search() {
    let mut agent = agent_with_scrollable_plan();
    {
        let viewer = agent.line_viewer.as_mut().unwrap();
        viewer.list_state.open_search(&viewer.lines);
        for ch in ['s', 't', 'e', 'p'] {
            viewer.list_state.handle_key_event(
                &KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &viewer.lines,
            );
        }
        viewer.list_state.handle_key_event(
            &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &viewer.lines,
        );
        assert!(viewer.list_state.input_mode().is_none());
        assert!(
            viewer.list_state.match_count() >= 2,
            "fixture steps must match query step; got {}",
            viewer.list_state.match_count()
        );
    }
    let first = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .matcher()
        .and_then(|m| m.current_match);
    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    let second = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .matcher()
        .and_then(|m| m.current_match);
    assert_ne!(first, second, "n must jump to the next search hit");
    assert!(
        agent.prompt.text().is_empty() && agent.prompt.images.is_empty(),
        "n/N jump hits, not typing into the Operator box; got {:?}",
        agent.prompt.text()
    );
    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT)),
        &ActionRegistry::defaults(),
    );
    let back = agent
        .line_viewer
        .as_ref()
        .unwrap()
        .list_state
        .matcher()
        .and_then(|m| m.current_match);
    assert_eq!(back, first, "N must jump to the previous search hit");
    assert!(
        agent.prompt.text().is_empty() && agent.prompt.images.is_empty(),
        "n/N jump hits, not typing into the Operator box; got {:?}",
        agent.prompt.text()
    );
}
