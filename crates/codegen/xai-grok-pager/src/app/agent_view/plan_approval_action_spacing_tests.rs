//! Idle plan approval actions are a little bigger and spaced out better.
//!
//! Operator: "plan search works very well, but the buttons are mushed too
//! close together. Make them a little bigger and spaced out better."
//!
//! The wide row is ` approve  |  comment  |  revise  |  exit `.
//! Each action is wider than the bare word. Two spaces sit on each side
//! of each pipe. The four actions stay approve, comment, revise, exit.
//!
//! Empty Enter never Approves. That contract stays in
//! `empty_enter_never_approves_even_when_approve_is_marked` and
//! `empty_enter_never_approves_exclusive_covering_present_github_122`.
//! This test does not change those asserts.

use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::views::file_search::line_viewer::{
    LineViewerKind, LineViewerState, plan_approval_spaced_action_row, render_line_viewer,
};

const OPERATOR_CONTRACT: &str = concat!(
    "plan search works very well, but the buttons are mushed too close together. ",
    "Make them a little bigger and spaced out better."
);

const SPACED_IDLE_ROW: &str = " approve  |  comment  |  revise  |  exit ";
/// Pre-edit wide paint: bare words, two spaces around each pipe, no pad
/// cell on the actions. That row is still mushed.
const MUSHED_BARE_ROW: &str = "approve  |  comment  |  revise  |  exit";

fn rect_text(buf: &Buffer, area: Rect) -> String {
    let mut text = String::new();
    for x in area.x..area.x + area.width {
        text.push_str(buf[(x, area.y)].symbol());
    }
    text
}

fn span_text(buf: &Buffer, y: u16, x0: u16, x1: u16) -> String {
    let mut text = String::new();
    for x in x0..x1 {
        text.push_str(buf[(x, y)].symbol());
    }
    text
}

#[test]
fn plan_approval_actions_are_a_little_bigger_and_spaced_out_better() {
    assert!(
        OPERATOR_CONTRACT.contains("the buttons are mushed too close together"),
        "the test must quote the Operator"
    );
    assert!(
        OPERATOR_CONTRACT.contains("a little bigger and spaced out better"),
        "the test must quote the Operator"
    );

    let row = plan_approval_spaced_action_row(["approve", "comment", "revise", "exit"]);
    assert_eq!(row, SPACED_IDLE_ROW);
    assert_ne!(
        row, MUSHED_BARE_ROW,
        "bare words with only the pipe gap are still mushed too close together"
    );
    assert!(
        !row.contains("approve|"),
        "reject approve jammed against the pipe: {row:?}"
    );
    assert!(
        !row.contains("approve | "),
        "reject a single space before the pipe: {row:?}"
    );
    assert_eq!(row.matches('|').count(), 3, "three pipes, four actions");
    for (index, ch) in row.char_indices() {
        if ch != '|' {
            continue;
        }
        assert!(index >= 2, "pipe needs two spaces before it");
        assert_eq!(&row[index - 2..index], "  ", "two spaces before pipe");
        assert_eq!(&row[index + 1..index + 3], "  ", "two spaces after pipe");
    }
    let words = ["approve", "comment", "revise", "exit"];
    let segments: Vec<&str> = row.split('|').collect();
    assert_eq!(segments.len(), 4, "still exactly four actions");
    for (segment, word) in segments.iter().zip(words) {
        assert_eq!(
            segment.trim(),
            word,
            "no extra decision word in {segment:?}"
        );
        assert!(
            segment.chars().count() > word.chars().count(),
            "action segment {segment:?} must be longer than the bare word {word}"
        );
    }
    let clarify = plan_approval_spaced_action_row(["approve", "clarify", "revise", "exit"]);
    assert_eq!(
        clarify, " approve  |  clarify  |  revise  |  exit ",
        "comment flow keeps the same spacing and does not add actions"
    );

    let mut viewer = LineViewerState::open_markdown_content(
        "plan.md",
        "# Plan\n\nDo the thing\n".to_owned(),
        None,
    )
    .expect("open plan");
    viewer.kind = LineViewerKind::PlanPreview;
    viewer.fullscreen = true;
    viewer.plan_mut().feedback_active = true;
    viewer.plan_mut().show_action_buttons = false;

    let full = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(full);
    let theme = crate::theme::Theme::current();
    render_line_viewer(&mut buf, full, &mut viewer, Path::new("/tmp"), &theme, 0);

    let plan = viewer.plan_ref().expect("plan extras");
    let approve = plan.approve_button_area.expect("approve hit box");
    let comment = plan.comment_button_area.expect("comment hit box");
    let revise = plan.send_button_area.expect("revise hit box");
    let exit = plan.abandon_button_area.expect("exit hit box");
    assert!(
        plan.questions_button_area.is_none(),
        "idle row is not clarify"
    );
    assert!(
        plan.approve_notes_button_area.is_none(),
        "notes is not an action"
    );

    let painted = span_text(&buf, approve.y, approve.x, exit.x + exit.width);
    assert_eq!(
        painted, row,
        "paint must use the spaced row, not the mushed bare words"
    );
    assert_eq!(painted, SPACED_IDLE_ROW);

    let boxes = [approve, comment, revise, exit];
    for (area, word) in boxes.iter().zip(words) {
        let text = rect_text(&buf, *area);
        assert_eq!(text.trim(), word, "hit box still names {word}");
        assert!(
            (text.chars().count() as u16) > word.chars().count() as u16,
            "hit box {text:?} must be a little bigger than the bare word {word}"
        );
        assert!(
            !text.contains('|'),
            "the pipe stays between actions, not inside {word}"
        );
    }
    assert!(approve.x < comment.x && comment.x < revise.x && revise.x < exit.x);
    assert!(approve.x + approve.width <= comment.x);
    assert!(comment.x + comment.width <= revise.x);
    assert!(revise.x + revise.width <= exit.x);
    assert_eq!(rect_text(&buf, approve).trim(), "approve");
    assert_eq!(rect_text(&buf, comment).trim(), "comment");
    assert_eq!(rect_text(&buf, revise).trim(), "revise");
    assert_eq!(rect_text(&buf, exit).trim(), "exit");

    let search = plan
        .search_button_area
        .expect("plan search magnifying glass stays a hit target");
    assert_ne!(
        search.y, approve.y,
        "plan search stays on the title bar, not the action row"
    );
    assert_eq!(
        buf[(search.x, search.y)].symbol(),
        crate::glyphs::search_icon(),
        "magnifying glass glyph is unchanged"
    );
}
