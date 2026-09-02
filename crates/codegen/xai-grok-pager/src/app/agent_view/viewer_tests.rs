//! Mouse-routing tests for the line viewer's plan preview: the scrollbar
//! must own a click+drag gesture end-to-end. A press on the track was
//! previously also treated as a comment-gutter anchor (row-only hit test),
//! so dragging the thumb selected plan lines for a comment instead of
//! scrolling (GB-4579: "can't click and drag scrollbar to view plan").

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::actions::ActionRegistry;
use crate::app::agent_view::AgentView;
use crate::app::agent_view::test_fixtures::make_agent;
use crate::views::plan_approval_view::{PlanApprovalFocus, PlanPromptIntent};

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

/// Empty-prompt `c` is still the line-comment gesture. A mid-type `c` is not.
#[test]
fn empty_preview_c_enters_line_commenting() {
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
        PlanApprovalFocus::Commenting,
        "empty-prompt `c` remains the explicit line-comment gesture"
    );
    assert!(
        pav.commenting_range.is_some(),
        "empty-prompt `c` must arm a line range"
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
