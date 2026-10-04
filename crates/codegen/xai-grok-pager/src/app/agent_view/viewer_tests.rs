//! Mouse-routing tests for the line viewer's plan preview: the scrollbar must own a click-and-drag gesture end-to-end.
//! A press on the track was previously also treated as a comment-gutter anchor (the hit test was row-only).
//! Dragging the thumb then selected plan lines for a comment instead of scrolling.

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
/// Scrollbar track column as split off by the list pane render (`maybe_split_for_scrollbar`): last column of the popup area.
const TRACK_X: u16 = 79;

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    })
}

/// Agent showing a plan-approval preview whose plan overflows the viewport, with the render-time areas planted so mouse dispatch works.
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

/// Presses on the modal border column next to the track used to fall into the click-outside-modal path instead of grabbing the thumb.
/// Users read the thumb and the border as one two-column scrollbar.
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

/// The thumb must keep following the pointer when a drag drifts off the popup rect (standard scrollbar behavior in every toolkit).
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

/// A gutter line-selection whose Up was lost must not survive a later scrollbar gesture.
/// The track press drops the stale anchor, so a stray release afterwards cannot commit the leftover lines as a comment.
#[test]
fn scrollbar_gesture_drops_stale_gutter_anchor() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    // Anchor and extend a comment line selection, then lose the Up
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
    // Scrollbar click and release: the track press must drop the stale anchor
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

    // The track press also discarded the in-progress comment draft (same rule as clicking back into the modal)
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

/// A second multi-line gutter drag while already Commenting must not replace the frozen freeform stash with the unsaved comment draft.
#[test]
fn gutter_drag_while_commenting_does_not_clobber_freeform_stash() {
    let mut agent = agent_with_scrollable_plan();
    let registry = ActionRegistry::defaults();

    agent.prompt.set_text("keep my freeform notes");
    // First multi-line drag: enter commenting and freeze freeform.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 10, 4),
        &registry,
    );
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Drag(MouseButton::Left), 10, 6),
        &registry,
    );
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Up(MouseButton::Left), 10, 6),
        &registry,
    );
    {
        let pav = agent.plan_approval_view.as_ref().unwrap();
        assert_eq!(pav.focus, PlanApprovalFocus::Commenting);
        assert_eq!(
            pav.stashed_feedback_prompt
                .as_ref()
                .map(|s| s.text.as_str()),
            Some("keep my freeform notes")
        );
    }
    agent.prompt.set_text("unsaved comment draft");

    // Second multi-line drag: new range, must keep original freeform stash.
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), 10, 5),
        &registry,
    );
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Drag(MouseButton::Left), 10, 7),
        &registry,
    );
    let _ = agent.handle_input(
        &mouse(MouseEventKind::Up(MouseButton::Left), 10, 7),
        &registry,
    );
    {
        let pav = agent.plan_approval_view.as_ref().unwrap();
        assert_eq!(pav.focus, PlanApprovalFocus::Commenting);
        assert_eq!(
            pav.stashed_feedback_prompt
                .as_ref()
                .map(|s| s.text.as_str()),
            Some("keep my freeform notes"),
            "second gutter drag must not replace freeform with comment draft"
        );
    }
    // Cancel commenting: freeform must restore, not the abandoned draft.
    agent.prompt.set_text("another draft");
    let esc = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    );
    let _ = agent.handle_plan_feedback_key(&esc);
    assert_eq!(agent.prompt.text(), "keep my freeform notes");
}

/// A lost mouse-up after a track press must not leave `is_scrollbar_dragging` sticky.
/// The next plan-line click must still anchor the gutter and enter click-to-comment.
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

    // No Up: simulate a dropped release, then click a plan line
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
    assert_eq!(
        pav.focus,
        PlanApprovalFocus::Commenting,
        "content Down must still enter click-to-comment"
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

fn enter_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

fn agent_with_markdown_viewer(text: &str) -> AgentView {
    let mut agent = make_agent();
    let id = agent
        .scrollback
        .push_block(crate::scrollback::block::RenderBlock::agent_message(text));
    let mut viewer = {
        let entry = agent.scrollback.get_by_id(id).expect("just pushed");
        crate::views::block_viewer::BlockViewerPane::for_markdown(id, entry)
            .expect("markdown viewer")
    };
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    agent.block_viewer = Some(viewer);
    agent
}

fn agent_with_running_markdown_viewer(text: &str) -> AgentView {
    let mut agent = make_agent();
    let id = agent
        .scrollback
        .push_block(crate::scrollback::block::RenderBlock::agent_message(text));
    agent
        .scrollback
        .get_by_id_mut(id)
        .expect("just pushed")
        .is_running = true;
    let mut viewer = {
        let entry = agent.scrollback.get_by_id(id).expect("just pushed");
        crate::views::block_viewer::BlockViewerPane::for_markdown(id, entry)
            .expect("markdown viewer")
    };
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    agent.block_viewer = Some(viewer);
    agent
}

#[test]
fn block_viewer_enter_quotes_current_line_and_closes() {
    let mut agent = agent_with_markdown_viewer("hello world");
    let outcome = agent.handle_block_viewer_key(&enter_key());
    assert!(matches!(
        outcome,
        crate::app::app_view::InputOutcome::Changed
    ));
    assert!(agent.block_viewer.is_none());
    assert_eq!(agent.active_pane, crate::app::agent_view::AgentPane::Prompt);
    let text = agent.prompt.text();
    assert!(
        text.contains("> hello world"),
        "quoted line missing from prompt: {text:?}"
    );
    assert!(
        text.ends_with("\n\n"),
        "quote should end with a blank line, got {text:?}"
    );
    assert!(agent.block_viewer_resume.is_some());
}

#[test]
fn block_viewer_enter_starts_quote_on_its_own_line() {
    let mut agent = agent_with_markdown_viewer("hello world");
    agent.prompt.set_text("draft");
    agent.prompt.set_cursor(agent.prompt.text().len());
    agent.handle_block_viewer_key(&enter_key());
    assert_eq!(agent.prompt.text(), "draft\n> hello world\n\n");
}

#[test]
fn block_viewer_enter_delimit_uses_selection_start() {
    let mut agent = agent_with_markdown_viewer("hello world");
    agent.prompt.set_text("prefix\nmore");
    agent.prompt.textarea.set_selection(3, 7);
    agent.handle_block_viewer_key(&enter_key());
    assert_eq!(agent.prompt.text(), "pre\n> hello world\n\nmore");
}

#[test]
fn block_viewer_enter_quotes_last_line_while_following() {
    let mut agent = agent_with_running_markdown_viewer("hello\n\nworld");
    assert!(agent.block_viewer.as_ref().unwrap().list_state.follow_mode);
    assert_eq!(
        agent
            .block_viewer
            .as_ref()
            .unwrap()
            .list_state
            .selected_index(),
        None
    );
    let outcome = agent.handle_block_viewer_key(&enter_key());
    assert!(matches!(
        outcome,
        crate::app::app_view::InputOutcome::Changed
    ));
    assert!(agent.block_viewer.is_none());
    let text = agent.prompt.text();
    assert!(
        text.contains("> world"),
        "follow-mode Enter should quote the last line, got {text:?}"
    );
}

#[test]
fn block_viewer_enter_pastes_chip_for_four_lines() {
    use crate::views::block_viewer::{TextDrag, TextEndpoint};
    use crate::views::prompt_widget::KIND_PASTE;

    let mut agent = make_agent();
    let mut viewer =
        crate::views::block_viewer::BlockViewerPane::for_plain_text("t", "one\ntwo\nthree\nfour");
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    viewer.text_drag = Some(TextDrag {
        anchor: TextEndpoint {
            item_idx: 2,
            col: 0,
        },
        head: TextEndpoint {
            item_idx: 5,
            col: 3,
        },
        active: false,
    });
    agent.block_viewer = Some(viewer);

    agent.handle_block_viewer_key(&enter_key());
    assert!(agent.block_viewer.is_none());
    assert!(
        agent
            .prompt
            .textarea
            .elements()
            .iter()
            .any(|e| e.kind == KIND_PASTE),
        "4-line quote should become a paste chip"
    );
    let text = agent.prompt.text();
    assert!(text.contains("> one"));
    assert!(text.contains("> four"));
    assert!(
        text.ends_with("\n\n"),
        "quote should end with a blank line, got {text:?}"
    );
}

#[test]
fn block_viewer_search_enter_does_not_quote() {
    let mut agent = agent_with_markdown_viewer("hello world");
    agent.handle_block_viewer_key(&KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert!(
        agent
            .block_viewer
            .as_ref()
            .unwrap()
            .list_state
            .input_mode()
            .is_some()
    );
    agent.handle_block_viewer_key(&KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
    agent.handle_block_viewer_key(&enter_key());
    assert!(
        agent.block_viewer.is_some(),
        "search-bar Enter must keep the viewer open"
    );
    assert!(
        agent.prompt.text().is_empty(),
        "search-bar Enter must not quote into the prompt"
    );
}

#[test]
fn block_viewer_enter_on_empty_selection_keeps_viewer_open() {
    let mut agent = make_agent();
    let mut viewer =
        crate::views::block_viewer::BlockViewerPane::for_plain_text("t", "hello\n\nworld");
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    viewer.select_body_line_for_test(3);
    agent.block_viewer = Some(viewer);

    agent.handle_block_viewer_key(&enter_key());
    assert!(
        agent.block_viewer.is_some(),
        "Enter with nothing to quote must keep the viewer open"
    );
    assert!(
        agent.prompt.text().is_empty(),
        "Enter with nothing to quote must not insert into the prompt"
    );
}

#[test]
fn block_viewer_esc_clears_sticky_then_closes() {
    use crate::views::block_viewer::{TextDrag, TextEndpoint};

    let mut agent = agent_with_markdown_viewer("hello world");
    {
        let viewer = agent.block_viewer.as_mut().unwrap();
        viewer.text_drag = Some(TextDrag {
            anchor: TextEndpoint {
                item_idx: 0,
                col: 0,
            },
            head: TextEndpoint {
                item_idx: 0,
                col: 5,
            },
            active: false,
        });
    }
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    agent.handle_block_viewer_key(&esc);
    assert!(
        agent.block_viewer.is_some(),
        "first Esc should clear the highlight, not close"
    );
    assert!(agent.block_viewer.as_ref().unwrap().text_drag.is_none());
    agent.handle_block_viewer_key(&esc);
    assert!(agent.block_viewer.is_none());
    assert!(agent.block_viewer_resume.is_some());
}

#[test]
fn block_viewer_enter_from_fullscreen_child_quotes_into_parent() {
    let mut child = agent_with_markdown_viewer("hello world");
    child.prompt.set_text("child-draft");
    let mut parent = make_agent();
    parent.prompt.set_text("parent-draft");
    parent.prompt.set_cursor(parent.prompt.text().len());
    parent.insert_test_child("child-sid".into(), Box::new(child));
    parent.open_subagent_fullscreen("child-sid".into());
    let registry = ActionRegistry::defaults();
    let outcome = parent.handle_input(&Event::Key(enter_key()), &registry);
    assert!(matches!(
        outcome,
        crate::app::app_view::InputOutcome::Changed
    ));
    assert!(parent.active_subagent.is_none());
    assert_eq!(parent.prompt.text(), "parent-draft\n> hello world\n\n");
    assert_eq!(
        parent.active_pane,
        crate::app::agent_view::AgentPane::Prompt
    );
    if let Some(child) = parent.subagent_views.get("child-sid") {
        assert!(child.block_viewer.is_none());
        assert_eq!(child.prompt.text(), "child-draft");
    }
}

#[test]
fn install_block_viewer_ignores_missing_resume_id() {
    use crate::app::agent_view::BlockViewerResume;
    use crate::scrollback::block::RenderBlock;
    use crate::views::block_viewer::BlockViewerPane;

    let mut agent = make_agent();
    let id = agent
        .scrollback
        .push_block(RenderBlock::agent_message("hello\n\nworld"));
    agent.block_viewer_resume = Some(BlockViewerResume {
        entry_id: id,
        kind: crate::views::block_viewer::ViewerKind::Markdown,
        selected_id: Some(99_999),
        scroll_offset: 0,
        follow_mode: false,
    });
    let pane = {
        let entry = agent.scrollback.get_by_id(id).expect("entry");
        BlockViewerPane::for_markdown(id, entry).expect("markdown")
    };
    agent.install_block_viewer(pane);
    let viewer = agent.block_viewer.as_mut().expect("installed");
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    assert_eq!(viewer.selected_plain_text(), "hello");
}

#[test]
fn block_viewer_enter_quotes_preamble_line() {
    use crate::scrollback::block::RenderBlock;
    use crate::views::block_viewer::BlockViewerPane;
    use ratatui::text::Line;

    let mut agent = make_agent();
    let id = agent
        .scrollback
        .push_block(RenderBlock::agent_message("hello"));
    let mut viewer = {
        let entry = agent.scrollback.get_by_id(id).expect("entry");
        BlockViewerPane::for_markdown(id, entry).expect("markdown")
    };
    viewer.install_prepend_lines(&[Line::from("Read src/main.rs")]);
    viewer.list_state.select_by_id(u64::MAX);
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    agent.block_viewer = Some(viewer);
    agent.handle_block_viewer_key(&enter_key());
    assert!(agent.block_viewer.is_none());
    assert!(
        agent.prompt.text().contains("> Read src/main.rs"),
        "header line should quote, got {:?}",
        agent.prompt.text()
    );
}

#[test]
fn insert_quoted_reply_clears_bash_mode() {
    let mut agent = agent_with_markdown_viewer("hello world");
    agent.prompt_input_mode = crate::app::agent_view::PromptInputMode::Bash;
    agent.prompt.set_text("draft");
    agent.handle_block_viewer_key(&enter_key());
    assert_eq!(
        agent.prompt_input_mode,
        crate::app::agent_view::PromptInputMode::Normal
    );
    assert!(agent.prompt.text().contains("> hello world"));
}

#[test]
fn insert_quoted_reply_is_one_undo() {
    let mut agent = agent_with_markdown_viewer("hello world");
    agent.prompt.set_text("draft");
    agent.prompt.set_cursor(5);
    agent.handle_block_viewer_key(&enter_key());
    assert!(agent.prompt.textarea.undo());
    assert_eq!(agent.prompt.text(), "draft");

    let mut agent = agent_with_markdown_viewer("hello world");
    agent.prompt.set_text("pre\nmore");
    agent.prompt.textarea.set_selection(0, 4);
    agent.handle_block_viewer_key(&enter_key());
    assert!(agent.prompt.textarea.undo());
    assert_eq!(agent.prompt.text(), "pre\nmore");
}

#[test]
fn install_block_viewer_ignores_mismatched_kind() {
    use crate::app::agent_view::BlockViewerResume;
    use crate::scrollback::block::RenderBlock;
    use crate::views::block_viewer::{BlockViewerPane, ViewerKind};

    let mut agent = make_agent();
    let id = agent
        .scrollback
        .push_block(RenderBlock::agent_message("hello\n\nworld"));
    agent.block_viewer_resume = Some(BlockViewerResume {
        entry_id: id,
        kind: ViewerKind::PlainText,
        selected_id: Some(0),
        scroll_offset: 99,
        follow_mode: false,
    });
    let pane = {
        let entry = agent.scrollback.get_by_id(id).expect("entry");
        BlockViewerPane::for_markdown(id, entry).expect("markdown")
    };
    agent.install_block_viewer(pane);
    let viewer = agent.block_viewer.as_mut().expect("installed");
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    assert_eq!(viewer.selected_plain_text(), "hello");
    assert_ne!(viewer.list_state.scroll_offset(), 99);
}

#[test]
fn install_block_viewer_restores_live_preamble_id() {
    use crate::app::agent_view::BlockViewerResume;
    use crate::scrollback::block::RenderBlock;
    use crate::views::block_viewer::BlockViewerPane;
    use ratatui::text::Line;

    let mut agent = make_agent();
    let id = agent
        .scrollback
        .push_block(RenderBlock::agent_message("hello\n\nworld"));
    agent.block_viewer_resume = Some(BlockViewerResume {
        entry_id: id,
        kind: crate::views::block_viewer::ViewerKind::Markdown,
        selected_id: Some(u64::MAX),
        scroll_offset: 0,
        follow_mode: false,
    });
    let pane = {
        let entry = agent.scrollback.get_by_id(id).expect("entry");
        BlockViewerPane::for_markdown(id, entry).expect("markdown")
    };
    agent.install_block_viewer(pane);
    let viewer = agent.block_viewer.as_mut().expect("installed");
    viewer.install_prepend_lines(&[Line::from("header")]);
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    assert_eq!(viewer.list_state.selected_id(), Some(u64::MAX));
    assert_eq!(viewer.selected_plain_text(), "header");
}

fn test_bg_task(
    task_id: &str,
    stdout: &str,
    scrollback_entry_id: Option<crate::scrollback::entry::EntryId>,
) -> crate::app::agent::BgTaskState {
    let mut task = crate::app::agent::BgTaskState {
        task_id: task_id.into(),
        tool_call_id: format!("call-{task_id}"),
        command: "echo".into(),
        description: None,
        cwd: "/tmp".into(),
        output_file: "/tmp/out".into(),
        status: crate::app::agent::BgTaskStatus::Done,
        start_time: std::time::SystemTime::now(),
        end_time: None,
        exit_code: Some(0),
        signal: None,
        stdout: String::new(),
        stdout_line_count: 0,
        truncated: false,
        pending_kill: false,
        kill_requested_at: None,
        scrollback_entry_id,
        is_monitor: false,
        restored_from_replay: false,
    };
    task.set_stdout(stdout.to_string());
    task
}

/// A task with no scrollback anchor (completed-early race, scrollback swap)
/// still opens its viewer from the task's own stdout, on the sentinel anchor.
#[test]
fn show_bg_task_viewer_opens_unattached() {
    let mut agent = make_agent();
    agent
        .session
        .bg_tasks
        .insert("orphan".into(), test_bg_task("orphan", "out", None));
    assert!(agent.show_bg_task_viewer("orphan"));
    let viewer = agent.block_viewer.as_ref().expect("opened");
    assert_eq!(viewer.entry_id, crate::scrollback::entry::EntryId::new(0));
    assert_eq!(viewer.bg_task_id.as_deref(), Some("orphan"));
}

#[test]
fn show_bg_task_viewer_restores_resume() {
    use crate::scrollback::block::RenderBlock;

    let mut agent = make_agent();
    let id = agent
        .scrollback
        .push_block(RenderBlock::agent_message("task body"));
    agent
        .session
        .bg_tasks
        .insert("t1".into(), test_bg_task("t1", "one\ntwo\nthree", Some(id)));
    assert!(agent.show_bg_task_viewer("t1"));
    let selected_id = {
        let viewer = agent.block_viewer.as_mut().expect("opened");
        assert_eq!(viewer.entry_id, id);
        assert_ne!(viewer.entry_id, crate::scrollback::entry::EntryId::new(0));
        viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
        viewer.select_body_line_for_test(1);
        viewer.list_state.selected_id()
    };
    agent.dismiss_block_viewer();
    assert!(agent.show_bg_task_viewer("t1"));
    let viewer = agent.block_viewer.as_mut().expect("reopened");
    viewer.prepare_for_test(Rect::new(0, 0, 80, 24));
    assert_eq!(viewer.list_state.selected_id(), selected_id);
    assert_eq!(viewer.selected_plain_text(), "two");
}

#[test]
fn hovering_a_comment_row_reveals_its_close_button() {
    let mut agent = agent_with_casual_commented_plan();
    let registry = ActionRegistry::defaults();

    assert!(
        close_button_areas(&agent).is_empty(),
        "no `[✗]` while the comment is neither hovered nor selected"
    );

    hover_comment_row(&mut agent, &registry);

    let buf = render_plan_viewer(&mut agent);
    let [(id, rect)] = close_button_areas(&agent)[..] else {
        panic!("hover must cache exactly one `[✗]` rect");
    };
    assert_eq!((id, rect.y), (0, comment_screen_row(&agent)));

    let drawn: String = (rect.x..rect.x + rect.width)
        .map(|x| {
            buf.cell((x, rect.y))
                .map(|c| c.symbol().to_owned())
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(drawn, crate::glyphs::ballot_x_button());
}

#[test]
fn clicking_the_close_button_deletes_without_starting_an_edit() {
    let mut agent = agent_with_casual_commented_plan();
    let registry = ActionRegistry::defaults();

    hover_comment_row(&mut agent, &registry);
    render_plan_viewer(&mut agent);
    let (_, rect) = *close_button_areas(&agent)
        .first()
        .expect("cached `[✗]` rect");

    let _ = agent.handle_input(
        &mouse(MouseEventKind::Down(MouseButton::Left), rect.x + 1, rect.y),
        &registry,
    );

    assert!(
        agent.plan_comments.is_empty(),
        "clicking `[✗]` must delete the comment"
    );
    assert!(
        agent.casual_commenting_range.is_none(),
        "the `[✗]` click must not fall through to click-to-edit"
    );
}

#[test]
fn deleting_the_comment_being_edited_cancels_the_edit() {
    let mut agent = agent_with_casual_commented_plan();
    agent.casual_editing_comment_id = Some(0);
    agent.casual_commenting_range = Some(2..3);

    let _ = agent.delete_plan_comment_by_id(0);

    assert_eq!(agent.casual_editing_comment_id, None);
    assert!(agent.casual_commenting_range.is_none());
}

#[test]
fn deleting_the_approval_comment_being_edited_cancels_the_edit_and_restores_the_prompt() {
    let mut agent = agent_with_scrollable_plan();
    {
        let pav = agent.plan_approval_view.as_mut().expect("approval mounted");
        pav.comments
            .push(crate::views::plan_approval_view::PlanComment {
                id: 7,
                line_range: 2..3,
                text: "tighten this".into(),
            });
        pav.editing_comment_id = Some(7);
        pav.commenting_range = Some(2..3);
        pav.focus = PlanApprovalFocus::Commenting;
        pav.stashed_feedback_prompt = Some(agent.prompt.stash());
    }
    agent.prompt.set_text("edited draft");

    let _ = agent.delete_plan_comment_by_id(7);

    let pav = agent.plan_approval_view.as_ref().expect("approval stays");
    assert!(pav.comments.is_empty(), "the comment must be deleted");
    assert_eq!(pav.focus, PlanApprovalFocus::Preview);
    assert_eq!(
        agent.prompt.text(),
        "",
        "the pre-edit prompt must be restored, not the abandoned draft"
    );
}

/// A casual plan preview with one comment.
/// The final render caches the mouse hit-test rects.
fn agent_with_casual_commented_plan() -> AgentView {
    let mut agent = make_agent();
    let mut viewer =
        crate::views::file_search::line_viewer::LineViewerState::open_markdown_content(
            "plan.md",
            "alpha\nbravo\ncharlie\ndelta\n".to_owned(),
            None,
        )
        .expect("plan content opens the viewer");
    viewer.kind = crate::views::file_search::line_viewer::LineViewerKind::PlanPreview;
    viewer.fullscreen = true;

    agent
        .plan_comments
        .push(crate::views::plan_approval_view::PlanComment {
            id: 0,
            line_range: 2..3,
            text: "tighten this".into(),
        });
    viewer.prepare_layout(POPUP.width, POPUP.height);
    viewer.rebuild_with_comments(&agent.plan_comments);
    agent.line_viewer = Some(viewer);

    render_plan_viewer(&mut agent);
    agent
}

fn render_plan_viewer(agent: &mut AgentView) -> ratatui::buffer::Buffer {
    let area = Rect::new(0, 0, 80, 16);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let comment_count = agent.plan_comments.len();

    crate::views::file_search::line_viewer::render_line_viewer(
        &mut buf,
        area,
        agent.line_viewer.as_mut().expect("viewer open"),
        std::path::Path::new("/tmp"),
        &crate::theme::Theme::current(),
        comment_count,
    );
    buf
}

fn close_button_areas(agent: &AgentView) -> Vec<(u64, Rect)> {
    agent
        .line_viewer
        .as_ref()
        .and_then(|v| v.plan_ref())
        .map(|p| p.comment_close_areas.clone())
        .unwrap_or_default()
}

fn hover_comment_row(agent: &mut AgentView, registry: &ActionRegistry) {
    let row = comment_screen_row(agent);
    let _ = agent.handle_input(&mouse(MouseEventKind::Moved, 10, row), registry);
}

fn comment_screen_row(agent: &AgentView) -> u16 {
    let viewer = agent.line_viewer.as_ref().expect("viewer open");
    let area = viewer.last_popup_area.expect("render caches popup area");
    (area.y..area.y + area.height)
        .find(|&row| viewer.comment_id_at_screen_row(row, area) == Some(0))
        .expect("comment row is visible")
}

fn press_plan_key(agent: &mut AgentView, code: KeyCode, modifiers: KeyModifiers) {
    let _ = agent.handle_input(
        &Event::Key(KeyEvent::new(code, modifiers)),
        &ActionRegistry::defaults(),
    );
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

/// Operator: a focused plan comment composer inserts y. Copy must not
/// steal that letter. The Operator typed `for you too` and saw `for ou too`
/// because the footer advertised `y:copy` during text entry.
#[test]
fn focused_plan_comment_box_inserts_y_instead_of_copy() {
    let mut agent = agent_with_scrollable_plan();
    let _ = agent.enter_plan_commenting();
    assert_eq!(
        agent.plan_approval_view.as_ref().unwrap().focus,
        PlanApprovalFocus::Commenting,
        "the plan comment composer must be focused before y"
    );
    type_plan_chars(&mut agent, "for you too");
    assert_eq!(
        agent.prompt.text(),
        "for you too",
        "focused plan comment composer must insert y; got {:?} (copy stole y when this is `for ou too`)",
        agent.prompt.text()
    );
    assert!(
        agent.toast.is_none(),
        "bare y must not copy the plan while the plan comment composer is focused"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "inserting y must not Approve or Exit"
    );
    let labels: Vec<String> = agent
        .current_shortcut_hints(&ActionRegistry::defaults(), false)
        .iter()
        .map(|hint| hint.label.to_string())
        .collect();
    assert!(
        labels.iter().any(|label| label == "save comment"),
        "Enter still saves; footer must keep save comment, got {labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == "cancel"),
        "Esc still cancels; footer must keep cancel, got {labels:?}"
    );
    assert!(
        !labels.iter().any(|label| label == "copy"),
        "do not advertise y:copy while the plan comment composer is focused, got {labels:?}"
    );
}

/// Catalog filter name stays enrolled. The Operator contract replaced
/// copy-on-y: a focused plan comment composer inserts y and does not copy.
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
        "line notey",
        "y while the plan comment composer is focused must insert y, got {:?}",
        agent.prompt.text()
    );
    assert!(
        agent.toast.is_none(),
        "bare y must not copy the plan while the plan comment composer is focused"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "inserting y must not Approve or Exit"
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
    agent.plan_mode_active = true;
    agent.plan_mode_pending = None;
    agent.prompt.set_text("");
    {
        let viewer = agent.line_viewer.as_mut().expect("plan pane");
        viewer.plan_mut().selected_cta =
            Some(crate::views::file_search::line_viewer::SelectedPlanCta::Approve);
    }
    let outcome = agent.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &ActionRegistry::defaults(),
    );
    assert!(
        !matches!(
            &outcome,
            InputOutcome::Action(
                Action::SendPrompt(_) | Action::SendPromptNow { .. } | Action::Interject { .. }
            ) | InputOutcome::ActionThenForward(
                Action::SendPrompt(_) | Action::SendPromptNow { .. } | Action::Interject { .. }
            )
        ),
        "empty Enter never Approves; got {outcome:?}"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "empty Enter must never Approve a parked plan"
    );
    assert!(
        agent.line_viewer.is_some(),
        "empty Enter never Approves and must leave the plan panel open"
    );
    assert_ne!(
        agent.plan_mode_pending,
        Some(false),
        "empty Enter never Approves and must not exit plan mode"
    );
    assert!(
        agent.prompt.text().trim().is_empty(),
        "empty Enter never Approves and must not invent `Love it! Execute now.` or any other composer text, got {:?}",
        agent.prompt.text()
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

/// Empty Enter on the default parked Preview stays on Preview.
/// Commenting is explicit `c` only. Empty Enter never Approves.
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

/// Empty-prompt `a` on the isolated plan.md Preview path types, not Approves.
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

/// Isolated Preview open plus a typed Operator sentence: Enter is the
/// human turn. It does not Approve. It is not an interjection. The
/// sentence stays in the composer for a later Approve click, and it is
/// not only plan comment 1. Empty Enter never Approves.
#[test]
fn isolated_preview_human_text_enter_is_human_turn_not_only_plan_comment() {
    use crate::app::actions::Action;
    use crate::app::app_view::InputOutcome;
    use crate::scrollback::block::RenderBlock;

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
    assert!(
        !matches!(
            &send,
            InputOutcome::Action(Action::Interject { .. })
                | InputOutcome::ActionThenForward(Action::Interject { .. })
        ),
        "Isolated Preview open plus a typed sentence plus Enter must not Interject; got {send:?}"
    );
    assert!(
        matches!(&send, InputOutcome::Changed),
        "Isolated Preview open plus a typed sentence plus Enter is a human turn, not Approve and not SendPrompt; got {send:?}"
    );
    assert!(
        agent.plan_approval_view.is_some() && !agent.plan_decision_resolved,
        "Enter must leave the plan waiting"
    );
    assert!(
        agent.prompt.text().contains(HUMAN),
        "Enter must leave the sentence in the composer so click Approve can send it as notes, got {:?}",
        agent.prompt.text()
    );
    let comments = agent
        .plan_approval_view
        .as_ref()
        .map(|pav| {
            pav.comments
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let in_comments = comments.iter().any(|c| c.contains(HUMAN));
    let in_scrollback = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.get(i).map(|e| &e.block),
            Some(RenderBlock::UserPrompt(b)) if b.text.contains(HUMAN)
        )
    });
    assert!(
        !in_comments || in_scrollback,
        "Human sentence must not remain only as plan comment 1; comments={comments:?}"
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
        agent.line_viewer.is_none(),
        "After the Operator submits revisions on an exclusive /plan present, the plan view goes away (or is not left up as the idle plan pane) while the revise turn runs. Do not leave plan.md docked after revision submit."
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
