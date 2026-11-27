//! A click on `show 5 more` in an L2 subagents list must reveal the hidden rows.

use std::sync::Arc;

use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::actions::ActionRegistry;
use crate::app::agent_view::test_fixtures::{make_agent, running_subagent_info};
use crate::scrollback::render::ScratchBuffer;

const L2: &str = "l2-coord";

fn specialist_id(index: u32) -> String {
    format!("01a21791-c631-75fb-8941-{index:04}")
}

fn buffer_text(buf: &Buffer, area: Rect) -> String {
    (area.y..area.y + area.height)
        .map(|y| {
            (area.x..area.x + area.width)
                .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_owned()))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn phrase_cell(buf: &Buffer, area: Rect, phrase: &str) -> Option<(u16, u16)> {
    let glyphs: Vec<String> = phrase.chars().map(|ch| ch.to_string()).collect();
    if glyphs.is_empty() {
        return None;
    }
    for y in area.y..area.y + area.height {
        let symbols: Vec<String> = (area.x..area.x + area.width)
            .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_owned()))
            .collect();
        if symbols.len() < glyphs.len() {
            continue;
        }
        for start in 0..=symbols.len() - glyphs.len() {
            if symbols[start..start + glyphs.len()] == glyphs[..] {
                return Some((area.x + start as u16, y));
            }
        }
    }
    None
}

fn paint(l1: &mut crate::app::agent_view::AgentView, area: Rect) -> (Buffer, String) {
    let registry = ActionRegistry::defaults();
    let mut buf = Buffer::empty(area);
    let mut scratch = ScratchBuffer::new();
    let _ = l1.draw(
        area,
        &mut buf,
        &registry,
        &mut scratch,
        None,
        false,
        crate::app::agent_view::BannerSlotParams::none(),
        true,
        false,
        &mut Vec::new(),
        crate::app::agent_view::AppRenderParams::default(),
    );
    let painted = buffer_text(&buf, area);
    (buf, painted)
}

fn visible_ids(painted: &str, ids: &[String]) -> usize {
    ids.iter()
        .filter(|id| painted.contains(id.as_str()))
        .count()
}

/// Eleven live L3 rows on an L2 dock paint six rows and `show 5 more`.
/// A click on that line must show all eleven. Leaving six rows fails.
#[test]
fn l2_click_on_show_5_more_reveals_all_11_subagent_rows() {
    crate::views::dock::set_enabled_for_test(true);
    let registry = ActionRegistry::defaults();
    let mut l1 = make_agent();
    let l2 = make_agent();
    let mut l2_info = running_subagent_info(L2);
    l2_info.description = Arc::from("Coordinate the overlay");
    l2_info.attempt.depth = Some(1);
    l1.subagent_sessions.insert(L2.to_owned(), l2_info);
    l1.subagent_views.insert(L2.to_owned(), Box::new(l2));

    let ids: Vec<String> = (1..=11).map(specialist_id).collect();
    for id in &ids {
        let mut info = running_subagent_info(id);
        info.description = Arc::from(format!("specialist {id}"));
        info.attempt.depth = Some(2);
        info.attempt.parent_session_id = Some(Arc::from(L2));
        l1.subagent_sessions.insert(id.clone(), info);
        l1.insert_test_child(id.clone(), Box::new(make_agent()));
    }
    l1.active_subagent = Some(L2.to_owned());

    let area = Rect::new(0, 0, 120, 60);
    let (buf, painted) = paint(&mut l1, area);
    assert!(
        painted.contains("show 5 more"),
        "collapsed L2 dock must paint `show 5 more`:\n{painted}"
    );
    assert_eq!(
        visible_ids(&painted, &ids),
        6,
        "collapsed L2 dock must paint 6 of 11 rows:\n{painted}"
    );
    let (column, row) = phrase_cell(&buf, area, "show 5 more")
        .unwrap_or_else(|| panic!("`show 5 more` must be a click target:\n{painted}"));

    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
    let _ = l1.handle_input(&click, &registry);

    let (_buf, revealed) = paint(&mut l1, area);
    assert_eq!(
        visible_ids(&revealed, &ids),
        11,
        "a click on `show 5 more` must show all 11 rows, not leave the list at 6:\n{revealed}"
    );
}
