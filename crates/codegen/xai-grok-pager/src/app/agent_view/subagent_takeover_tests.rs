use crate::actions::ActionRegistry;
use crate::app::actions::Action;
use crate::app::agent_view::test_fixtures::{
    add_running_execute, ctrl, key, make_agent, parent_with_child, running_subagent_info,
};
use crate::app::agent_view::{AgentPane, AgentView, InputMode, ViewSurface};
use crate::app::app_view::InputOutcome;
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::SubagentBlock;
use crate::scrollback::blocks::tool::{
    SentMessageDelivery, SentMessageInput, SentMessagePresentation, SentMessageTarget,
    SentMessageToolCallBlock, ToolCallBlock,
};
use crate::scrollback::render::ScratchBuffer;
use crate::scrollback::types::DisplayMode;
use crate::views::shortcuts_bar::PendingHint;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::sync::Arc;
use std::time::{Duration, Instant};
const CHILD_SID: &str = "child-sid";
fn subagent_row() -> RenderBlock {
    RenderBlock::Subagent(SubagentBlock::started(
        "child task",
        CHILD_SID,
        "general-purpose",
        None,
        None,
        None,
        false,
    ))
}
fn message_row(target: SentMessageTarget) -> RenderBlock {
    RenderBlock::ToolCall(ToolCallBlock::SentMessage(SentMessageToolCallBlock::new(
        SentMessagePresentation::Sent,
        Some(SentMessageInput {
            target,
            delivery: Some(SentMessageDelivery::Steer),
            text: "follow up".to_owned(),
        }),
    )))
}
fn named_child() -> SentMessageTarget {
    SentMessageTarget::Named {
        label: Arc::from("Subagent \u{201c}sleeper\u{201d}"),
        child_session_id: Arc::from(CHILD_SID),
    }
}
/// A parent with `block` as its only, selected scrollback entry; the child view exists only when `owns_child`.
fn parent_selecting(block: RenderBlock, owns_child: bool) -> AgentView {
    let mut parent = if owns_child {
        parent_with_child(CHILD_SID)
    } else {
        make_agent()
    };
    parent.scrollback.push_block(block);
    parent.scrollback.prepare_layout(80, 40);
    parent.scrollback.set_selected(Some(0));
    if parent.scrollback.toggle_group_expansion() {
        parent.scrollback.prepare_layout(80, 40);
    }
    parent
}
#[test]
fn viewer_keys_open_the_linked_child_from_subagent_and_message_rows() {
    let registry = ActionRegistry::defaults();
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let ctrl_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
    let unresolved = SentMessageTarget::Unresolved {
        subagent_id: CHILD_SID.to_owned(),
    };
    for (case, block, owns_child, opens) in [
        ("subagent row", subagent_row(), true, true),
        ("named message row", message_row(named_child()), true, true),
        (
            "named row, child view gone",
            message_row(named_child()),
            false,
            false,
        ),
        (
            "parent alias",
            message_row(SentMessageTarget::Parent),
            true,
            false,
        ),
        ("unresolved id", message_row(unresolved), true, false),
    ] {
        for chord in [&enter, &ctrl_f] {
            let mut parent = parent_selecting(block.clone(), owns_child);
            let outcome = parent.handle_scrollback_key(chord, &registry);
            assert_eq!(
                opens.then_some(CHILD_SID),
                parent.active_subagent.as_deref(),
                "{case}: {chord:?}"
            );
            assert!(
                matches!(
                    (opens, &outcome),
                    (true, InputOutcome::Changed)
                        | (false, InputOutcome::Action(Action::OpenBlockViewer))
                ),
                "{case}: {chord:?} gave {outcome:?}"
            );
        }
    }
}
#[test]
fn double_click_opens_the_linked_child_or_folds_the_message_row() {
    let now = Instant::now();
    let again = now + Duration::from_millis(10);
    for (case, block, owns_child, expected_child, expected_mode) in [
        (
            "message row",
            message_row(named_child()),
            true,
            Some(CHILD_SID),
            DisplayMode::Collapsed,
        ),
        (
            "message row, child view gone",
            message_row(named_child()),
            false,
            None,
            DisplayMode::Expanded,
        ),
        (
            "subagent row",
            subagent_row(),
            true,
            Some(CHILD_SID),
            DisplayMode::Collapsed,
        ),
        (
            "subagent row, child view gone",
            subagent_row(),
            false,
            None,
            DisplayMode::Collapsed,
        ),
    ] {
        let mut parent = parent_selecting(block, owns_child);
        (parent.last_click, _) = parent.handle_scrollback_click(now, 0, false);
        let _ = parent.handle_scrollback_click(again, 0, false);
        let mode = parent.scrollback.entry(0).map(|entry| entry.display_mode);
        assert_eq!(
            (expected_child, Some(expected_mode)),
            (parent.active_subagent.as_deref(), mode),
            "{case}"
        );
    }
}
#[test]
fn fullscreen_child_ctrl_b_never_demotes_child_or_parent() {
    let registry = ActionRegistry::defaults();
    let child_sid = "child-sid".to_string();
    let mut parent = make_agent();
    add_running_execute(&mut parent);
    assert!(
        parent
            .session
            .tracker
            .running_execute_tool_call_id()
            .is_some()
    );
    let mut child = make_agent();
    add_running_execute(&mut child);
    assert!(
        child
            .session
            .tracker
            .running_execute_tool_call_id()
            .is_some()
    );
    child.set_active_pane(AgentPane::Scrollback, true);
    parent.insert_test_child(child_sid.clone(), Box::new(child));
    assert_eq!(
        ViewSurface::ChildTakeover,
        parent
            .subagent_views
            .get(&child_sid)
            .unwrap_or_else(|| panic!("missing map entry"))
            .surface()
    );
    parent.open_subagent_fullscreen(child_sid.clone());
    let outcome = parent.handle_input(&ctrl('b'), &registry);
    assert!(matches!(outcome, InputOutcome::Changed));
    assert!(!matches!(
        outcome,
        InputOutcome::Action(Action::DemoteToBackground)
    ));
    assert_eq!(parent.active_subagent.as_deref(), Some(child_sid.as_str()));
    assert!(
        parent
            .session
            .tracker
            .running_execute_tool_call_id()
            .is_some()
    );
    assert!(
        parent
            .subagent_views
            .get(&child_sid)
            .unwrap_or_else(|| panic!("missing map entry"))
            .session
            .tracker
            .running_execute_tool_call_id()
            .is_some()
    );
    let child = &parent
        .subagent_views
        .get(&child_sid)
        .unwrap_or_else(|| panic!("missing map entry"));
    assert_eq!(ViewSurface::ChildTakeover, child.surface());
    assert!(
        !child
            .current_shortcut_hints(&registry)
            .iter()
            .any(|hint| hint.label == "send to bg")
    );
    assert!(child.hit_bg_button.rect.is_none());
}
/// Every glyph in `area`, row by row.
fn buffer_text(buf: &Buffer, area: Rect) -> String {
    (area.y..area.y + area.height)
        .map(|y| {
            (area.x..area.x + area.width)
                .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_owned()))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// The parent paints none of its own chrome under a takeover, so a pending hint can only reach the buffer by being
/// forwarded into the child's shortcuts bar; the child's hidden composer yields no cursor to forward.
#[test]
fn takeover_draw_forwards_pending_hint_and_child_cursor() {
    let registry = ActionRegistry::defaults();
    let mut parent = parent_with_child("child");
    parent.open_subagent_fullscreen("child".to_owned());
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    let mut scratch = ScratchBuffer::new();
    let (cursor, _) = parent.draw(
        area,
        &mut buf,
        &registry,
        &mut scratch,
        Some(PendingHint {
            shortcut: crate::key!(Esc),
            label: "clear pending hint",
        }),
        false,
        crate::app::agent_view::BannerSlotParams::none(),
        false,
        false,
        &mut Vec::new(),
        crate::app::agent_view::AppRenderParams::default(),
    );
    assert_eq!(None, cursor);
    let text = buffer_text(&buf, area);
    assert!(text.contains("press again to clear pending hint"), "{text}");
    assert_eq!(
        0,
        parent
            .subagent_view("child")
            .expect("child view")
            .pane_areas
            .prompt
            .height
    );
}
/// A child view paints no `[L0]` button, even when its own registry could dispatch `/dashboard`.
/// Inside the dashboard overlay the button stays as the way back.
#[test]
fn takeover_shows_dashboard_button_only_inside_the_overlay() {
    let registry = ActionRegistry::defaults();
    for (in_overlay, expect_button) in [(false, false), (true, true)] {
        let mut parent = parent_with_child("child");
        parent
            .subagent_view_mut("child")
            .expect("child view")
            .set_dashboard_visible(true);
        parent.open_subagent_fullscreen("child".to_owned());
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        let _ = parent.draw(
            area,
            &mut buf,
            &registry,
            &mut scratch,
            None,
            false,
            crate::app::agent_view::BannerSlotParams::none(),
            in_overlay,
            false,
            &mut Vec::new(),
            crate::app::agent_view::AppRenderParams::default(),
        );
        let text = buffer_text(&buf, area);
        assert_eq!(
            expect_button,
            text.contains("[L0]"),
            "in_overlay={in_overlay}: {text}"
        );
        let child = parent.subagent_view("child").expect("child view");
        assert_eq!(
            expect_button,
            child.hit_dashboard.rect.is_some(),
            "in_overlay={in_overlay}"
        );
    }
}
/// A parent whose child, with a transcript, sits on bare scrollback under an open takeover. `vim_mode` is pinned on
/// both views: `AgentView::new` reads it from the user's config, which CI does not have.
fn open_takeover(child_sid: &str, vim_mode: bool) -> AgentView {
    let mut parent = parent_with_child(child_sid);
    let child = parent.subagent_view_mut(child_sid).expect("child view");
    add_running_execute(child);
    child.set_input_mode(InputMode::Vim);
    parent.set_vim_mode_recursive(vim_mode);
    parent.open_subagent_fullscreen(child_sid.to_owned());
    parent
}
/// Root-only chords never open a modal on the child under either binding set; the takeover stays up and `q` still
/// closes it. `?` is the palette's alt key and dies in the funnel on both; with vim bindings it is query text once the
/// child's search is open. `Shift+/` matches no chord, so without vim bindings it falls to type-to-focus, whose
/// `FocusPrompt` the composer guard refuses.
#[test]
fn child_root_only_chords_are_swallowed() {
    let registry = ActionRegistry::defaults();
    for vim_mode in [true, false] {
        let mut parent = open_takeover("child", vim_mode);
        for chord in [
            ctrl('p'),
            ctrl('m'),
            ctrl('r'),
            ctrl('o'),
            ctrl('l'),
            key(KeyCode::F(2)),
        ] {
            let outcome = parent.handle_input(&chord, &registry);
            assert!(
                matches!(outcome, InputOutcome::Changed | InputOutcome::Unchanged),
                "vim={vim_mode} {chord:?}: {outcome:?}"
            );
            let child = parent.subagent_view("child").expect("child view");
            assert!(
                child.active_modal.is_none(),
                "vim={vim_mode} {chord:?} opened a modal"
            );
            assert!(
                child.is_bare_scrollback(),
                "vim={vim_mode} {chord:?} left bare scrollback"
            );
        }
        let question = key(KeyCode::Char('?'));
        let shift_slash = Event::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::SHIFT));
        for printable in [&question, &shift_slash] {
            let outcome = parent.handle_input(printable, &registry);
            let expected = if vim_mode || printable == &question {
                matches!(outcome, InputOutcome::Changed | InputOutcome::Unchanged)
            } else {
                matches!(
                    outcome,
                    InputOutcome::ActionThenForward(Action::FocusPrompt)
                )
            };
            assert!(expected, "vim={vim_mode} {printable:?}: {outcome:?}");
            let child = parent.subagent_view("child").expect("child view");
            assert!(
                child.active_modal.is_none(),
                "vim={vim_mode} {printable:?} opened a modal"
            );
            assert!(
                child.is_bare_scrollback(),
                "vim={vim_mode} {printable:?} left bare scrollback"
            );
            assert_eq!(AgentPane::Scrollback, child.active_pane);
        }
        assert_eq!(Some("child"), parent.active_subagent.as_deref());
        if vim_mode {
            parent.handle_input(&key(KeyCode::Char('/')), &registry);
            parent.handle_input(&key(KeyCode::Char('?')), &registry);
            let child = parent.subagent_view("child").expect("child view");
            assert!(child.active_modal.is_none());
            assert_eq!(
                Some("?"),
                child.scrollback_search.as_ref().map(|s| s.query())
            );
            parent.handle_input(&key(KeyCode::Esc), &registry);
            assert!(
                parent
                    .subagent_view("child")
                    .expect("child view")
                    .scrollback_search
                    .is_none()
            );
            assert_eq!(Some("child"), parent.active_subagent.as_deref());
        }
        parent.handle_input(&key(KeyCode::Char('q')), &registry);
        assert_eq!(None, parent.active_subagent, "vim={vim_mode}");
    }
}
/// The child resolves a key pane-first exactly as a root does: with the mouse-capture toggle enabled, Ctrl+R on
/// scrollback is `ToggleMouseCapture` (allowed) rather than the `OpenSessions` chord it maps to under `AgentScreen`.
#[test]
fn child_ctrl_r_keeps_scrollback_precedence() {
    let mut parent = open_takeover("child", true);
    let outcome = parent.handle_input(&ctrl('r'), &ActionRegistry::defaults_with_config(true));
    assert!(
        matches!(outcome, InputOutcome::Action(Action::ToggleMouseCapture)),
        "{outcome:?}"
    );
    let outcome = parent.handle_input(&ctrl('r'), &ActionRegistry::defaults());
    assert!(matches!(outcome, InputOutcome::Changed), "{outcome:?}");
    let child = parent.subagent_view("child").expect("child view");
    assert!(child.active_modal.is_none());
    assert!(child.is_bare_scrollback());
}

const OWED_OVERLAY_CLICK: &str =
    "A left click on the specialist status in the L2 overlay opens that L3 takeover.";

/// First cell of `phrase` on one row. Each ASCII glyph is one cell.
fn specialist_status_cell(buf: &Buffer, area: Rect, phrase: &str) -> Option<(u16, u16)> {
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

/// One cell per row that paints `phrase`, top to bottom. The origin is the first glyph.
fn phrase_row_cells(buf: &Buffer, area: Rect, phrase: &str) -> Vec<(u16, u16)> {
    let glyphs: Vec<String> = phrase.chars().map(|ch| ch.to_string()).collect();
    if glyphs.is_empty() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for y in area.y..area.y + area.height {
        let symbols: Vec<String> = (area.x..area.x + area.width)
            .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_owned()))
            .collect();
        if symbols.len() < glyphs.len() {
            continue;
        }
        for start in 0..=symbols.len() - glyphs.len() {
            if symbols[start..start + glyphs.len()] == glyphs[..] {
                rows.push((area.x + start as u16, y));
                break;
            }
        }
    }
    rows
}

/// A left click on the specialist status in the L2 overlay opens that L3 takeover.
/// The status is on screen. `hit_overlay_nested_status` stays empty because nothing paints it, and the click is never read.
#[test]
fn overlay_nested_status_click_opens_l3_session_view() {
    const L2: &str = "l2-coord";
    const L3: &str = "l3-open";
    const SPECIALIST: &str = "Open the specialist view";
    let registry = ActionRegistry::defaults();
    let mut l1 = make_agent();
    let mut l2 = make_agent();
    l2.session.state = crate::app::agent::AgentState::TurnRunning;
    let mut l2_info = running_subagent_info(L2);
    l2_info.description = Arc::from("Coordinate the overlay");
    l2_info.attempt.depth = Some(1);
    l1.subagent_sessions.insert(L2.to_owned(), l2_info);
    l1.subagent_views.insert(L2.to_owned(), Box::new(l2));
    let mut specialist = running_subagent_info(L3);
    specialist.description = Arc::from(SPECIALIST);
    specialist.attempt.is_background = true;
    specialist.attempt.depth = Some(2);
    specialist.attempt.parent_session_id = Some(Arc::from(L2));
    l1.subagent_sessions.insert(L3.to_owned(), specialist);
    l1.insert_test_child(L3.to_owned(), Box::new(make_agent()));
    l1.active_subagent = Some(L2.to_owned());

    let area = Rect::new(0, 0, 120, 30);
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
    let status = specialist_status_cell(&buf, area, SPECIALIST).unwrap_or_else(|| {
        panic!("{OWED_OVERLAY_CLICK} The specialist status must be on screen so the click has a target:\n{painted}")
    });
    assert!(
        l1.hit_overlay_nested_status.contains(status.0, status.1),
        "{OWED_OVERLAY_CLICK} The specialist status is at ({}, {}) and hit_overlay_nested_status is never painted: {:?}",
        status.0,
        status.1,
        l1.hit_overlay_nested_status.rect
    );
    assert_eq!(
        l1.overlay_nested_status_child_sid.as_deref(),
        Some(L3),
        "{OWED_OVERLAY_CLICK}"
    );
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: status.0,
        row: status.1,
        modifiers: KeyModifiers::NONE,
    });
    let _ = l1.handle_input(&click, &registry);
    assert_eq!(
        l1.active_subagent.as_deref(),
        Some(L3),
        "{OWED_OVERLAY_CLICK} The click is never read."
    );
}

const WAITING_ONE_TASK: &str = "Waiting on 1 task...";
const OWED_WAITING_ROW: &str =
    "A left click on Waiting on 1 task... in the nested view opens that waited task.";

/// The nested view's bottom `Waiting on 1 task...` row is the overlay status control.
/// The takeover title also paints that phrase. The click is the lowest row, not the title.
/// The specialist description is empty, so the row does not depend on that phrase.
#[test]
fn waiting_task_row_opens_the_waited_task() {
    use agent_client_protocol as acp;
    const L2: &str = "l2-coord";
    const L3: &str = "l3-open";
    let registry = ActionRegistry::defaults();
    let mut l1 = make_agent();
    let mut l2 = make_agent();
    l2.session.state = crate::app::agent::AgentState::TurnRunning;
    let meta = crate::acp::meta::NotificationMeta::default();
    l2.session.handle_update(
        acp::SessionUpdate::ToolCall(
            acp::ToolCall::new(
                acp::ToolCallId::new(Arc::from("wait-one")),
                "get_command_or_subagent_output",
            )
            .kind(acp::ToolKind::Other)
            .status(acp::ToolCallStatus::Pending)
            .content(vec![])
            .raw_input(Some(serde_json::json!({
                "task_ids": [L3],
                "timeout_ms": 30_000,
            })))
            .locations(vec![]),
        ),
        &meta,
        &mut l2.scrollback,
    );
    let mut l2_info = running_subagent_info(L2);
    l2_info.description = Arc::from("Coordinate the overlay");
    l2_info.attempt.depth = Some(1);
    l1.subagent_sessions.insert(L2.to_owned(), l2_info);
    l1.subagent_views.insert(L2.to_owned(), Box::new(l2));
    let mut specialist = running_subagent_info(L3);
    specialist.description = Arc::from("");
    specialist.attempt.is_background = true;
    specialist.attempt.depth = Some(2);
    specialist.attempt.parent_session_id = Some(Arc::from(L2));
    l1.subagent_sessions.insert(L3.to_owned(), specialist);
    l1.insert_test_child(L3.to_owned(), Box::new(make_agent()));
    l1.active_subagent = Some(L2.to_owned());

    let area = Rect::new(0, 0, 120, 30);
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
    assert!(
        painted.contains(WAITING_ONE_TASK),
        "{OWED_WAITING_ROW} The nested view must paint `{WAITING_ONE_TASK}`:\n{painted}"
    );
    let phrase_rows = phrase_row_cells(&buf, area, WAITING_ONE_TASK);
    let title = phrase_rows.first().copied().unwrap_or_else(|| {
        panic!("{OWED_WAITING_ROW} The takeover title must paint `{WAITING_ONE_TASK}`:\n{painted}")
    });
    let status = phrase_rows.last().copied().unwrap_or(title);
    let lowest_y = phrase_rows
        .iter()
        .map(|cell| cell.1)
        .max()
        .unwrap_or(title.1);
    assert_eq!(
        status.1, lowest_y,
        "{OWED_WAITING_ROW} The clicked cell must be the lowest row that contains `{WAITING_ONE_TASK}`."
    );
    assert!(
        status.1 > title.1,
        "{OWED_WAITING_ROW} The clicked y must be strictly below the takeover title row. The title also paints `{WAITING_ONE_TASK}`. Title y is {}, clicked y is {}. The bottom status row does not contain the phrase when these match. hit_overlay_nested_status is {:?}.\n{painted}",
        title.1,
        status.1,
        l1.hit_overlay_nested_status.rect
    );
    assert!(
        l1.hit_overlay_nested_status.contains(status.0, status.1),
        "{OWED_WAITING_ROW} `{WAITING_ONE_TASK}` lowest row is at ({}, {}), below the takeover title at y {}. hit_overlay_nested_status is the title row: {:?}.",
        status.0,
        status.1,
        title.1,
        l1.hit_overlay_nested_status.rect
    );
    assert_eq!(
        l1.overlay_nested_status_child_sid.as_deref(),
        Some(L3),
        "{OWED_WAITING_ROW}"
    );
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: status.0,
        row: status.1,
        modifiers: KeyModifiers::NONE,
    });
    let _ = l1.handle_input(&click, &registry);
    assert_eq!(
        l1.active_subagent.as_deref(),
        Some(L3),
        "{OWED_WAITING_ROW} The click did not open the waited task."
    );
}

const OWED_WAITING_ARROW: &str = "The L2 overlay paints an open-arrow control on the waited L3 waiting row. Activating that control opens that L3.";

/// The L2 overlay waiting row is an open control again: it paints `[↗]` for the
/// waited L3, and a left click on that arrow opens that L3.
#[test]
fn l2_overlay_waiting_row_paints_open_arrow_that_opens_the_waited_l3() {
    use agent_client_protocol as acp;
    const L2: &str = "l2-coord";
    const L3: &str = "l3-open";
    let registry = ActionRegistry::defaults();
    let mut l1 = make_agent();
    let mut l2 = make_agent();
    l2.session.state = crate::app::agent::AgentState::TurnRunning;
    let meta = crate::acp::meta::NotificationMeta::default();
    l2.session.handle_update(
        acp::SessionUpdate::ToolCall(
            acp::ToolCall::new(
                acp::ToolCallId::new(Arc::from("wait-one")),
                "get_command_or_subagent_output",
            )
            .kind(acp::ToolKind::Other)
            .status(acp::ToolCallStatus::Pending)
            .content(vec![])
            .raw_input(Some(serde_json::json!({
                "task_ids": [L3],
                "timeout_ms": 30_000,
            })))
            .locations(vec![]),
        ),
        &meta,
        &mut l2.scrollback,
    );
    let mut l2_info = running_subagent_info(L2);
    l2_info.description = Arc::from("Coordinate the overlay");
    l2_info.attempt.depth = Some(1);
    l1.subagent_sessions.insert(L2.to_owned(), l2_info);
    l1.subagent_views.insert(L2.to_owned(), Box::new(l2));
    let mut specialist = running_subagent_info(L3);
    specialist.description = Arc::from("");
    specialist.attempt.is_background = true;
    specialist.attempt.depth = Some(2);
    specialist.attempt.parent_session_id = Some(Arc::from(L2));
    l1.subagent_sessions.insert(L3.to_owned(), specialist);
    l1.insert_test_child(L3.to_owned(), Box::new(make_agent()));
    l1.active_subagent = Some(L2.to_owned());

    let area = Rect::new(0, 0, 120, 30);
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
    let open_arrow = crate::glyphs::enlarge_button();
    assert!(
        painted.contains(WAITING_ONE_TASK),
        "{OWED_WAITING_ARROW} The nested view must paint `{WAITING_ONE_TASK}`:\n{painted}"
    );
    assert!(
        painted.contains(open_arrow),
        "{OWED_WAITING_ARROW} The nested view must paint the open-arrow control `{open_arrow}` for the waited L3:\n{painted}"
    );
    let phrase_rows = phrase_row_cells(&buf, area, WAITING_ONE_TASK);
    let title = phrase_rows.first().copied().unwrap_or_else(|| {
        panic!(
            "{OWED_WAITING_ARROW} The takeover title must paint `{WAITING_ONE_TASK}`:\n{painted}"
        )
    });
    let waiting = phrase_rows.last().copied().unwrap_or(title);
    let lowest_y = phrase_rows
        .iter()
        .map(|cell| cell.1)
        .max()
        .unwrap_or(title.1);
    assert_eq!(
        waiting.1, lowest_y,
        "{OWED_WAITING_ARROW} The open arrow belongs on the lowest `{WAITING_ONE_TASK}` row."
    );
    assert!(
        waiting.1 > title.1,
        "{OWED_WAITING_ARROW} The open arrow must sit on the waiting row below the takeover title. Title y is {}, waiting y is {}.\n{painted}",
        title.1,
        waiting.1
    );
    let waiting_row = Rect {
        x: area.x,
        y: waiting.1,
        width: area.width,
        height: 1,
    };
    let arrow = specialist_status_cell(&buf, waiting_row, open_arrow).unwrap_or_else(|| {
        panic!(
            "{OWED_WAITING_ARROW} `{open_arrow}` must paint on the `{WAITING_ONE_TASK}` row at y {}. hit_overlay_nested_status is {:?}.\n{painted}",
            waiting.1,
            l1.hit_overlay_nested_status.rect
        )
    });
    assert_eq!(
        arrow.1, waiting.1,
        "{OWED_WAITING_ARROW} The open arrow is not on the waiting row."
    );
    assert!(
        l1.hit_overlay_nested_status.contains(arrow.0, arrow.1),
        "{OWED_WAITING_ARROW} `{open_arrow}` at ({}, {}) is not a hit target. hit_overlay_nested_status is {:?}.",
        arrow.0,
        arrow.1,
        l1.hit_overlay_nested_status.rect
    );
    assert_eq!(
        l1.overlay_nested_status_child_sid.as_deref(),
        Some(L3),
        "{OWED_WAITING_ARROW}"
    );
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: arrow.0,
        row: arrow.1,
        modifiers: KeyModifiers::NONE,
    });
    let _ = l1.handle_input(&click, &registry);
    assert_eq!(
        l1.active_subagent.as_deref(),
        Some(L3),
        "{OWED_WAITING_ARROW} Activating the open arrow did not open the waited L3."
    );
}

#[test]
fn l2_nested_view_prompt_enter_sends_to_that_l2_and_does_not_start_an_l1_turn() {
    use agent_client_protocol as acp;
    let registry = ActionRegistry::defaults();
    let mut l1 = make_agent();
    l1.session.session_id = Some(acp::SessionId::new("l1-session"));
    let mut l2 = make_agent();
    l2.session.session_id = Some(acp::SessionId::new("l2-session"));
    l2.session.state = crate::app::agent::AgentState::TurnRunning;
    l1.insert_subagent_view(
        "l2-session".to_owned(),
        Box::new(l2),
        crate::app::agent_view::ChildLink::l2_coordinator(acp::SessionId::new("l1-session")),
    );
    l1.open_subagent_fullscreen("l2-session".to_owned());
    {
        let child = l1.subagent_view("l2-session").expect("l2");
        assert_eq!(
            child.composer_route(),
            crate::app::agent_view::ComposerRoute::L2Floor
        );
        assert_eq!(child.active_pane, AgentPane::Prompt);
    }
    l1.subagent_view_mut("l2-session")
        .expect("l2")
        .prompt
        .set_text("steer this coordinator");
    let area = Rect::new(0, 0, 120, 40);
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
    assert!(
        painted.contains("steer this coordinator"),
        "the L2 floor must paint the draft:\n{painted}"
    );
    let outcome = l1.handle_input(&key(KeyCode::Enter), &registry);
    assert!(matches!(outcome, InputOutcome::Changed), "{outcome:?}");
    let sent = l1.pending_effects.iter().any(|effect| {
        matches!(
            effect,
            crate::app::actions::Effect::SendInterject { session_id, text, .. }
                if session_id.0.as_ref() == "l2-session" && text == "steer this coordinator"
        )
    });
    assert!(
        sent,
        "Enter must SendInterject to the L2, got {:?}",
        l1.pending_effects
    );
    assert!(
        !l1.pending_effects
            .iter()
            .any(|effect| { matches!(effect, crate::app::actions::Effect::SendPrompt { .. }) }),
        "the L2 floor must not start an L1 SendPrompt"
    );
    assert!(
        l1.subagent_view("l2-session")
            .expect("l2")
            .prompt
            .text()
            .is_empty()
    );
    assert_eq!(
        l1.session.session_id.as_ref().map(|sid| sid.0.as_ref()),
        Some("l1-session")
    );

    let mut hidden = make_agent();
    hidden.insert_test_child("hidden-child".to_owned(), Box::new(make_agent()));
    hidden.open_subagent_fullscreen("hidden-child".to_owned());
    hidden
        .subagent_view_mut("hidden-child")
        .expect("child")
        .prompt
        .set_text("must not send");
    let _ = hidden.handle_input(&key(KeyCode::Enter), &registry);
    assert!(!hidden.pending_effects.iter().any(|effect| {
        matches!(
            effect,
            crate::app::actions::Effect::SendInterject { .. }
                | crate::app::actions::Effect::SendPrompt { .. }
        )
    }));
}

#[test]
fn enter_on_l2_opens_the_l3_view_and_the_l1_list_stays_l2_rows_plus_a_live_l3_count() {
    use agent_client_protocol as acp;
    use std::sync::Arc;
    const L2: &str = "l2-session";
    const L3: &str = "l3-session";
    const LINE: &str = "L3_TRANSCRIPT_LINE_FLOOR_OPEN";
    let registry = ActionRegistry::defaults();
    let mut l1 = make_agent();
    l1.session.session_id = Some(acp::SessionId::new("l1-session"));
    let mut l2 = make_agent();
    l2.session.session_id = Some(acp::SessionId::new(L2));
    l2.session.state = crate::app::agent::AgentState::TurnRunning;
    let mut l2_info = running_subagent_info(L2);
    l2_info.attempt.depth = Some(1);
    l2_info.attempt.parent_session_id = Some(Arc::from("l1-session"));
    l1.subagent_sessions.insert(L2.to_owned(), l2_info);
    l1.insert_subagent_view(
        L2.to_owned(),
        Box::new(l2),
        crate::app::agent_view::ChildLink::l2_coordinator(acp::SessionId::new("l1-session")),
    );
    let mut l3 = make_agent();
    l3.session.session_id = Some(acp::SessionId::new(L3));
    l3.scrollback
        .push_block(crate::scrollback::block::RenderBlock::user_prompt(LINE));
    let mut l3_info = running_subagent_info(L3);
    l3_info.attempt.depth = Some(2);
    l3_info.attempt.parent_session_id = Some(Arc::from(L2));
    l3_info.description = Arc::from("specialist-name-must-not-be-the-row");
    l1.subagent_sessions.insert(L3.to_owned(), l3_info);
    l1.insert_test_child(L3.to_owned(), Box::new(l3));
    l1.open_subagent_fullscreen(L2.to_owned());
    assert!(l1.subagent_view(L2).expect("l2").prompt.text().is_empty());
    let outcome = l1.handle_input(&key(KeyCode::Enter), &registry);
    assert!(matches!(outcome, InputOutcome::Changed), "{outcome:?}");
    assert_eq!(l1.active_subagent.as_deref(), Some(L3));
    assert_eq!(l1.overlay_return_sid.as_deref(), Some(L2));
    let area = Rect::new(0, 0, 120, 40);
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
    assert!(
        painted.contains(LINE),
        "the open L3 view must show its transcript:\n{painted}"
    );
    let child_ids: std::collections::HashSet<&str> = l1
        .subagent_sessions
        .values()
        .map(|row| row.child_session_id.as_ref())
        .collect();
    let l2_row = l1.subagent_sessions.get(L2).expect("l2 row");
    let l3_row = l1.subagent_sessions.get(L3).expect("l3 row");
    assert!(crate::app::subagent::is_l2_list_row(l2_row, &child_ids));
    assert!(!crate::app::subagent::is_l2_list_row(l3_row, &child_ids));
    assert_eq!(
        crate::app::subagent::live_l3_count(l1.subagent_sessions.values(), L2),
        1
    );
    assert_eq!(
        crate::app::subagent::format_live_l3_count(1).as_deref(),
        Some("1 specialist")
    );
    let back = l1.handle_input(&key(KeyCode::Esc), &registry);
    assert!(
        !matches!(back, InputOutcome::Action(Action::KillSubagent(_))),
        "{back:?}"
    );
    assert_eq!(l1.active_subagent.as_deref(), Some(L2));
    assert!(l1.overlay_return_sid.is_none());
    assert!(l1.subagent_sessions.get(L2).expect("l2").is_running());
}
