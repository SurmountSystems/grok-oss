//! Full-pipeline mouse tests for the per-task action icons: `[↗]` (view) and
//! `[✗]` (Tasks pane kill) / `[stop]` (dock kill).
//!
//! Unlike `dock_input_tests`, nothing forges `pane_areas`: every test paints a
//! real frame with `draw`, locates the icon cells the frame actually painted,
//! and drives hover and click through `handle_mouse` at those exact cells. This
//! is the paint-vs-hit-test agreement the user exercises.
use super::test_fixtures::make_agent;
use super::{AgentView, BannerSlotParams};
use crate::actions::ActionRegistry;
use crate::app::actions::{Action, Effect};
use crate::app::app_view::InputOutcome;
use crate::scrollback::render::ScratchBuffer;
use crate::views::tasks_pane::TaskEntryId;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
fn cell_symbol(buf: &Buffer, x: u16, y: u16) -> String {
    buf.cell((x, y))
        .map(|c| c.symbol().to_string())
        .unwrap_or_else(|| panic!("missing cell at ({x},{y})"))
}
fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}
fn draw_frame(agent: &mut AgentView, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    let mut scratch = ScratchBuffer::new();
    let _ = agent.draw(
        area,
        &mut buf,
        &ActionRegistry::defaults(),
        &mut scratch,
        None,
        false,
        BannerSlotParams::none(),
        false,
        false,
        &mut Vec::new(),
        super::AppRenderParams::default(),
    );
    buf
}
/// All cells whose symbol matches `sym` exactly, in reading order.
fn find_symbol(buf: &Buffer, sym: &str) -> Vec<(u16, u16)> {
    let area = *buf.area();
    let mut hits = Vec::new();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if buf.cell((x, y)).is_some_and(|c| c.symbol() == sym) {
                hits.push((x, y));
            }
        }
    }
    hits
}
fn insert_running_task(agent: &mut AgentView, task_id: &str) {
    let entry_id = agent
        .scrollback
        .push_block(crate::scrollback::block::RenderBlock::BgTask(
            crate::scrollback::blocks::BgTaskBlock::started("sleep 5", task_id),
        ));
    insert_running_task_with_entry(agent, task_id, Some(entry_id));
}
fn insert_running_task_with_entry(
    agent: &mut AgentView,
    task_id: &str,
    scrollback_entry_id: Option<crate::scrollback::entry::EntryId>,
) {
    agent.session.bg_tasks.insert(
        task_id.into(),
        crate::app::agent::BgTaskState {
            task_id: task_id.into(),
            tool_call_id: format!("call-{task_id}"),
            command: "sleep 5".into(),
            description: None,
            cwd: "/tmp".into(),
            output_file: "/tmp/out".into(),
            status: crate::app::agent::BgTaskStatus::Running,
            start_time: std::time::SystemTime::now(),
            end_time: None,
            exit_code: None,
            signal: None,
            stdout: String::new(),
            stdout_line_count: 0,
            truncated: false,
            pending_kill: false,
            kill_requested_at: None,
            scrollback_entry_id,
            is_monitor: false,
            restored_from_replay: false,
        },
    );
}
/// Tasks pane (dock off): the frame paints `[↗]` and `[✗]` on a running task
/// row; hovering each icon activates it and clicking fires its action.
#[test]
fn tasks_pane_icons_hover_and_click_where_painted() {
    let mut agent = make_agent();
    insert_running_task(&mut agent, "bg-1");
    let area = Rect::new(0, 0, 80, 30);
    let _ = draw_frame(&mut agent, area);
    let buf = draw_frame(&mut agent, area);
    let kill = *find_symbol(&buf, "\u{2717}")
        .first()
        .expect("kill icon [✗] painted for a running task");
    let view = *find_symbol(&buf, "\u{2197}")
        .first()
        .expect("view icon [↗] painted for a running task");
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, kill.0, kill.1));
    assert_eq!(
        agent.tasks.hovered_kill,
        Some(TaskEntryId::BgTask("bg-1".into())),
        "hovering the painted [✗] must activate it"
    );
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        kill.0,
        kill.1,
    ));
    assert!(
        matches!(outcome, InputOutcome::Action(Action::KillBgTask(ref id)) if id == "bg-1"),
        "clicking the painted [✗] must kill the task, got {outcome:?}"
    );
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, view.0, view.1));
    assert_eq!(
        agent.tasks.hovered_view,
        Some(TaskEntryId::BgTask("bg-1".into())),
        "hovering the painted [↗] must activate it"
    );
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        view.0,
        view.1,
    ));
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking the painted [↗] must open the viewer, got {outcome:?}"
    );
    assert!(
        agent.block_viewer.is_some(),
        "the [↗] click must open the bg task viewer"
    );
}
/// A task whose scrollback entry never materialized (completed-early race) or
/// dangles after a scrollback swap must still open its viewer from `[↗]`: the
/// task state carries the stdout, and the dock path already opens it.
#[test]
fn tasks_pane_view_click_opens_viewer_without_scrollback_entry() {
    let mut agent = make_agent();
    insert_running_task_with_entry(&mut agent, "bg-1", None);
    let area = Rect::new(0, 0, 80, 30);
    let _ = draw_frame(&mut agent, area);
    let buf = draw_frame(&mut agent, area);
    let view = *find_symbol(&buf, "\u{2197}")
        .first()
        .expect("view icon [↗] painted for a running task");
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, view.0, view.1));
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        view.0,
        view.1,
    ));
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking the painted [↗] must open the viewer, got {outcome:?}"
    );
    assert!(
        agent.block_viewer.is_some(),
        "the [↗] click must open the bg task viewer even without a scrollback entry"
    );
    let buf = draw_frame(&mut agent, area);
    assert!(
        agent.block_viewer.is_some(),
        "the viewer must stay open across a repaint without a scrollback entry"
    );
    let painted: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| cell_symbol(&buf, x, y))
                .collect::<Vec<_>>()
                .concat()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        painted.contains("sleep 5"),
        "the viewer header must show the task command: {painted}"
    );
}
/// A left click on the task row itself opens that task. The second of two
/// rows is the one that opens. The row stays, and the click does not kill.
#[test]
fn clicking_a_task_row_opens_that_task() {
    let mut agent = make_agent();
    insert_running_task(&mut agent, "bg-review");
    insert_running_task(&mut agent, "bg-sampler");
    let older = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1);
    let newer = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2);
    if let Some(task) = agent.session.bg_tasks.get_mut("bg-review") {
        task.description = Some("Review implementation".into());
        task.start_time = older;
    }
    if let Some(task) = agent.session.bg_tasks.get_mut("bg-sampler") {
        task.description = Some("Fix sampler filter_map clippy deny".into());
        task.start_time = newer;
    }
    let area = Rect::new(0, 0, 80, 30);
    let _ = draw_frame(&mut agent, area);
    let _ = draw_frame(&mut agent, area);
    let mut rows: Vec<(TaskEntryId, Rect)> = agent
        .tasks
        .row_hit_rects
        .iter()
        .filter(|(id, _)| matches!(id, TaskEntryId::BgTask(_)))
        .cloned()
        .collect();
    rows.sort_by_key(|(_, rect)| rect.y);
    assert!(
        rows.len() >= 2,
        "two task rows must be hit targets, got {}",
        rows.len()
    );
    let (second_id, second_rect) = rows[1].clone();
    let TaskEntryId::BgTask(opened_id) = second_id else {
        panic!("second row must be a background task");
    };
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        second_rect.x,
        second_rect.y,
    ));
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking the second task row must open it, got {outcome:?}"
    );
    assert!(
        !matches!(
            outcome,
            InputOutcome::Action(Action::KillBgTask(_))
                | InputOutcome::Action(Action::KillSubagent(_))
        ),
        "a row click must not kill, got {outcome:?}"
    );
    let shown = agent
        .block_viewer
        .as_ref()
        .and_then(|viewer| viewer.bg_task_id.clone());
    assert_eq!(
        shown.as_deref(),
        Some(opened_id.as_str()),
        "the opened viewer must be the second row"
    );
    assert!(
        agent.session.bg_tasks.contains_key("bg-review"),
        "the review row must stay"
    );
    assert!(
        agent.session.bg_tasks.contains_key("bg-sampler"),
        "the sampler row must stay"
    );
}

/// A left click on the painted Tasks row hit rect opens that task. A click
/// that does nothing fails. The click does not send KillSubagent. Hit the
/// painted rect. Do not use a keyboard stand-in.
#[test]
fn clicking_a_painted_tasks_row_opens_that_task() {
    let owed = "a left click on the painted Tasks row hit rect opens that task. A click that does nothing fails. The click does not send KillSubagent. Hit the painted rect. Do not use a keyboard stand-in.";
    let mut agent = make_agent();
    insert_running_task(&mut agent, "bg-review");
    insert_running_task(&mut agent, "bg-sampler");
    let older = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1);
    let newer = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2);
    if let Some(task) = agent.session.bg_tasks.get_mut("bg-review") {
        task.description = Some("Review implementation".into());
        task.start_time = older;
    }
    if let Some(task) = agent.session.bg_tasks.get_mut("bg-sampler") {
        task.description = Some("Fix sampler filter_map clippy deny".into());
        task.start_time = newer;
    }
    let area = Rect::new(0, 0, 80, 30);
    let buf = draw_frame(&mut agent, area);
    let sampler_rect = agent
        .tasks
        .row_hit_rects
        .iter()
        .find(|(id, _)| matches!(id, TaskEntryId::BgTask(tid) if tid == "bg-sampler"))
        .map(|(_, rect)| *rect)
        .expect(owed);
    let phrase = "Fix sampler filter_map clippy deny";
    let (column, row) = find_phrase_inside(&buf, phrase, sampler_rect).expect(owed);
    assert!(
        agent
            .pane_areas
            .tasks
            .contains(ratatui::layout::Position::new(column, row)),
        "{owed}"
    );
    assert!(
        sampler_rect.contains(ratatui::layout::Position::new(column, row)),
        "{owed}"
    );
    assert_ne!(cell_symbol(&buf, column, row), "\u{2717}");
    assert_ne!(cell_symbol(&buf, column, row), "\u{2197}");
    let outcome = click_left(&mut agent, column, row);
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "{owed} got {outcome:?}"
    );
    assert!(
        !matches!(
            outcome,
            InputOutcome::Action(Action::KillBgTask(_))
                | InputOutcome::Action(Action::KillSubagent(_))
        ),
        "{owed} got {outcome:?}"
    );
    assert!(
        !agent
            .pending_effects
            .iter()
            .any(|effect| matches!(effect, Effect::KillSubagent { .. })),
        "{owed}"
    );
    let shown = agent
        .block_viewer
        .as_ref()
        .and_then(|viewer| viewer.bg_task_id.clone());
    assert_eq!(shown.as_deref(), Some("bg-sampler"), "{owed}");
    assert!(agent.session.bg_tasks.contains_key("bg-review"), "{owed}");
    assert!(agent.session.bg_tasks.contains_key("bg-sampler"), "{owed}");
}

/// A left click on a painted in_progress Tasks row opens that task. Many
/// running rows stay on screen. The click uses the painted cells, including
/// the accent column the stored inner rect may omit. The click does not
/// send KillSubagent.
#[test]
fn clicking_a_painted_in_progress_tasks_row_opens_that_task() {
    let owed = "a left click on a painted in_progress Tasks row opens that task. Many running rows stay on screen. The click uses the painted cells, including the accent column the stored inner rect may omit. The click does not send KillSubagent.";
    let mut agent = make_agent();
    let names = [
        ("l2-alpha", "in-progress-alpha"),
        ("l2-bravo", "in-progress-bravo"),
        ("l2-charlie", "in-progress-charlie"),
        ("l2-sampler", "in-progress-sampler"),
        ("l2-echo", "in-progress-echo"),
        ("l2-foxtrot", "in-progress-foxtrot"),
    ];
    for (id, description) in names {
        insert_in_progress_l2(&mut agent, id, description);
    }
    let area = Rect::new(0, 0, 120, 40);
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains("in-progress-sampler"),
        "{owed} painted:\n{painted}"
    );
    let (column, row) = find_phrase(&buf, "in-progress-sampler").expect(owed);
    assert!(
        agent
            .pane_areas
            .tasks
            .contains(ratatui::layout::Position::new(column, row)),
        "{owed}"
    );
    assert_ne!(cell_symbol(&buf, column, row), "\u{2717}");
    assert_ne!(cell_symbol(&buf, column, row), "\u{2197}");
    let outcome = click_left(&mut agent, column, row);
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "{owed} got {outcome:?}"
    );
    assert!(
        !matches!(
            outcome,
            InputOutcome::Action(Action::KillBgTask(_))
                | InputOutcome::Action(Action::KillSubagent(_))
        ),
        "{owed} got {outcome:?}"
    );
    assert!(
        !agent
            .pending_effects
            .iter()
            .any(|effect| matches!(effect, Effect::KillSubagent { .. })),
        "{owed}"
    );
    assert_eq!(
        agent.active_subagent.as_deref(),
        Some("l2-sampler"),
        "{owed}"
    );
    agent.close_subagent_fullscreen();
    assert!(agent.active_subagent.is_none(), "{owed}");
    let buf = draw_frame(&mut agent, area);
    let (column, row) = find_phrase(&buf, "in-progress-sampler").expect(owed);
    let pad_x = agent.pane_areas.tasks.x;
    assert_ne!(pad_x, column, "{owed}");
    let outcome = click_left(&mut agent, pad_x, row);
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "{owed} accent-column click got {outcome:?}"
    );
    assert!(
        !matches!(outcome, InputOutcome::Action(Action::KillSubagent(_))),
        "{owed} accent-column click got {outcome:?}"
    );
    assert_eq!(
        agent.active_subagent.as_deref(),
        Some("l2-sampler"),
        "{owed}"
    );
}

/// Opening an L3 shows that specialist's own context. The L1 list stays L2
/// rows plus a live L3 count. The click does not send KillSubagent. Do not
/// paint the L1 window as the L3 context. Do not invent a context integer.
/// When the figure is absent, do not print 0.
#[test]
fn an_l3_context_is_visible_when_that_l3_is_opened() {
    use agent_client_protocol as acp;
    use std::sync::Arc;
    const L2: &str = "l2-session";
    const L3: &str = "l3-session";
    const LINE: &str = "L3_TRANSCRIPT_OWN_CONTEXT";
    let owed = "opening an L3 shows that specialist's own context. The L1 list stays L2 rows plus a live L3 count. The click does not send KillSubagent. Do not paint the L1 window as the L3 context. Do not invent a context integer. When the figure is absent, do not print 0.";
    crate::views::dock::set_enabled_for_test(true);
    let registry = ActionRegistry::defaults();
    let mut agent = make_agent();
    agent.session.session_id = Some(acp::SessionId::new("l1-session"));
    agent.session.models.override_context_window(500_000);
    agent.session_sampling_window = Some(500_000);
    agent.context_state = Some(xai_grok_shell::session::ContextInfo {
        used: 152_000,
        ..xai_grok_shell::session::ContextInfo::default()
    });
    let mut l2_info = super::test_fixtures::running_subagent_info(L2);
    l2_info.description = Arc::from("coordinator-row-stays");
    l2_info.attempt.depth = Some(1);
    l2_info.attempt.parent_session_id = Some(Arc::from("l1-session"));
    agent.subagent_sessions.insert(L2.to_owned(), l2_info);
    agent.insert_subagent_view(
        L2.to_owned(),
        Box::new(super::test_fixtures::make_agent()),
        super::ChildLink::l2_coordinator(acp::SessionId::new("l1-session")),
    );
    let mut l3 = super::test_fixtures::make_agent();
    l3.session.session_id = Some(acp::SessionId::new(L3));
    l3.scrollback
        .push_block(crate::scrollback::block::RenderBlock::user_prompt(LINE));
    let mut l3_info = super::test_fixtures::running_subagent_info(L3);
    l3_info.attempt.depth = Some(2);
    l3_info.attempt.parent_session_id = Some(Arc::from(L2));
    l3_info.attempt.tokens_used = None;
    l3_info.attempt.context_window_tokens = None;
    l3_info.description = Arc::from("specialist-name-must-not-be-the-row");
    agent.subagent_sessions.insert(L3.to_owned(), l3_info);
    agent.insert_test_child(L3.to_owned(), Box::new(l3));
    let area = Rect::new(0, 0, 120, 40);
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains("coordinator-row-stays"),
        "{owed} {painted}"
    );
    assert!(painted.contains("1 specialist"), "{owed} {painted}");
    assert!(
        !painted.contains("specialist-name-must-not-be-the-row"),
        "{owed} {painted}"
    );
    assert!(painted.contains("152K / 500K"), "{owed} {painted}");
    let (column, row) = find_phrase(&buf, "1 specialist").expect(owed);
    assert!(
        agent
            .pane_areas
            .dock
            .contains(ratatui::layout::Position::new(column + 2, row)),
        "{owed}"
    );
    let outcome = click_left(&mut agent, column + 2, row);
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "{owed} got {outcome:?}"
    );
    assert!(
        !matches!(outcome, InputOutcome::Action(Action::KillSubagent(_))),
        "{owed} got {outcome:?}"
    );
    assert!(
        !agent
            .pending_effects
            .iter()
            .any(|effect| matches!(effect, Effect::KillSubagent { .. })),
        "{owed}"
    );
    assert_eq!(agent.active_subagent.as_deref(), Some(L3), "{owed}");
    assert_eq!(agent.overlay_return_sid.as_deref(), Some(L2), "{owed}");
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(painted.contains(LINE), "{owed} {painted}");
    assert!(!painted.contains("152K / 500K"), "{owed} {painted}");
    assert!(!painted.contains("500K"), "{owed} {painted}");
    assert!(!painted.contains("0 /"), "{owed} {painted}");
    assert!(!painted.contains("0K"), "{owed} {painted}");
    let l3_info = agent
        .subagent_sessions
        .get_mut(L3)
        .expect("the opened specialist stays in the registry");
    l3_info.attempt.tokens_used = Some(48_000);
    l3_info.attempt.context_window_tokens = Some(200_000);
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(painted.contains("48K / 200K"), "{owed} {painted}");
    assert!(painted.contains(LINE), "{owed} {painted}");
    assert!(!painted.contains("152K / 500K"), "{owed} {painted}");
    assert!(!painted.contains("0 /"), "{owed} {painted}");
    assert_eq!(
        crate::app::subagent::live_l3_count(agent.subagent_sessions.values(), L2),
        1,
        "{owed}"
    );
    let back = agent.handle_input(&super::test_fixtures::key(KeyCode::Esc), &registry);
    assert!(
        !matches!(back, InputOutcome::Action(Action::KillSubagent(_))),
        "{owed} got {back:?}"
    );
    assert_eq!(agent.active_subagent.as_deref(), Some(L2), "{owed}");
    let back = agent.handle_input(&super::test_fixtures::key(KeyCode::Esc), &registry);
    assert!(agent.active_subagent.is_none(), "{owed} {back:?}");
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains("coordinator-row-stays"),
        "{owed} {painted}"
    );
    assert!(painted.contains("1 specialist"), "{owed} {painted}");
    assert!(
        !painted.contains("specialist-name-must-not-be-the-row"),
        "{owed} {painted}"
    );
    crate::views::dock::set_enabled_for_test(false);
}

fn insert_running_subagent(agent: &mut AgentView, child_session_id: &str) {
    let info = super::test_fixtures::running_subagent_info(child_session_id);
    agent
        .subagent_sessions
        .insert(child_session_id.to_string(), info);
    agent.insert_test_child(
        child_session_id.to_string(),
        Box::new(super::test_fixtures::make_agent()),
    );
}

fn insert_in_progress_l2(agent: &mut AgentView, child_session_id: &str, description: &str) {
    let mut info = super::test_fixtures::running_subagent_info(child_session_id);
    info.description = std::sync::Arc::from(description);
    info.attempt.depth = Some(1);
    info.attempt.activity_label = Some("in_progress".into());
    agent
        .subagent_sessions
        .insert(child_session_id.to_string(), info);
    agent.insert_test_child(
        child_session_id.to_string(),
        Box::new(super::test_fixtures::make_agent()),
    );
}
/// Dock subagent row: hovering reveals `[↗][stop]`; the painted `[stop]` must
/// kill the subagent and the painted `[↗]` must open it fullscreen.
#[test]
fn dock_subagent_icons_hover_and_click_where_painted() {
    crate::views::dock::set_enabled_for_test(true);
    let mut agent = make_agent();
    insert_running_subagent(&mut agent, "child-1");
    let area = Rect::new(0, 0, 80, 30);
    let buf = draw_frame(&mut agent, area);
    assert!(agent.dock_on, "dock must be on for this test");
    let dock = agent.pane_areas.dock;
    assert!(dock.height >= 2, "dock painted: {dock:?}");
    let row_y = dock.y + 1;
    let row: String = (0..area.width)
        .map(|x| cell_symbol(&buf, x, row_y))
        .collect::<Vec<_>>()
        .concat();
    assert!(
        row.contains("Subagent test"),
        "subagent row painted: {row:?}"
    );
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, dock.x + 5, row_y));
    let buf = draw_frame(&mut agent, area);
    let row: String = (0..area.width)
        .map(|x| cell_symbol(&buf, x, row_y))
        .collect::<Vec<_>>()
        .concat();
    assert!(
        row.contains('\u{2197}') && row.contains("[stop]"),
        "hovered dock subagent row must show [↗][stop]: {row:?}"
    );
    let view_x = (0..area.width)
        .find(|x| cell_symbol(&buf, *x, row_y) == "\u{2197}")
        .expect("[↗] painted on hovered subagent row");
    let kill_x = (0..area.width)
        .rev()
        .find(|x| cell_symbol(&buf, *x, row_y) == "s")
        .expect("[stop] painted on hovered subagent row");
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, kill_x, row_y));
    let _ = draw_frame(&mut agent, area);
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        kill_x,
        row_y,
    ));
    assert!(
        matches!(outcome, InputOutcome::Action(Action::KillSubagent(ref id)) if id == "sa-child-1"),
        "clicking the painted [stop] must kill the subagent, got {outcome:?}"
    );
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, view_x, row_y));
    let _ = draw_frame(&mut agent, area);
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        view_x,
        row_y,
    ));
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking the painted [↗] must open the subagent, got {outcome:?}"
    );
    assert_eq!(
        agent.active_subagent.as_deref(),
        Some("child-1"),
        "the [↗] click must open the subagent fullscreen"
    );
}
/// Dock (remote `dock_enabled`): hovering a task row reveals `[↗][stop]`;
/// hovering and clicking the painted icons must act on that row.
#[test]
fn dock_icons_hover_and_click_where_painted() {
    crate::views::dock::set_enabled_for_test(true);
    let mut agent = make_agent();
    insert_running_task(&mut agent, "bg-1");
    let area = Rect::new(0, 0, 80, 30);
    let buf = draw_frame(&mut agent, area);
    assert!(agent.dock_on, "dock must be on for this test");
    let dock = agent.pane_areas.dock;
    let row_y = (dock.y..dock.bottom())
        .find(|y| {
            let text: String = (0..area.width)
                .map(|x| cell_symbol(&buf, x, *y))
                .collect::<Vec<_>>()
                .concat();
            text.contains("sleep 5")
        })
        .expect("dock task row painted");
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, area.x + 5, row_y));
    let buf = draw_frame(&mut agent, area);
    let row: String = (0..area.width)
        .map(|x| cell_symbol(&buf, x, row_y))
        .collect::<Vec<_>>()
        .concat();
    assert!(
        row.contains('\u{2197}') && row.contains("[stop]"),
        "hovered dock row must show [↗][stop]: {row:?}"
    );
    let view_x = (0..area.width)
        .find(|x| cell_symbol(&buf, *x, row_y) == "\u{2197}")
        .expect("[↗] cell");
    let stop_x = (0..area.width)
        .rev()
        .find(|x| cell_symbol(&buf, *x, row_y) == "s")
        .expect("[stop] cell");
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, stop_x, row_y));
    let _ = draw_frame(&mut agent, area);
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        stop_x,
        row_y,
    ));
    assert!(
        matches!(outcome, InputOutcome::Action(Action::KillBgTask(ref id)) if id == "bg-1"),
        "clicking the painted [stop] must kill the task, got {outcome:?}"
    );
    let _ = agent.handle_mouse(&mouse(MouseEventKind::Moved, view_x, row_y));
    let _ = draw_frame(&mut agent, area);
    let outcome = agent.handle_mouse(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        view_x,
        row_y,
    ));
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking the painted [↗] must open the task viewer, got {outcome:?}"
    );
    assert!(
        agent.block_viewer.is_some(),
        "the [↗] click must open the bg task viewer"
    );
}
fn dock_row_has_tasks(buf: &Buffer, dock: Rect) -> bool {
    if dock.height == 0 {
        return false;
    }
    (dock.y..dock.bottom()).any(|y| {
        let text: String = (0..buf.area().width)
            .map(|x| buf[(x, y)].symbol())
            .collect();
        text.contains("Tasks")
    })
}
#[test]
fn ctrl_g_hides_and_shows_painted_dock() {
    crate::views::dock::set_enabled_for_test(true);
    let mut agent = make_agent();
    insert_running_task(&mut agent, "bg-1");
    let area = Rect::new(0, 0, 80, 30);
    let registry = ActionRegistry::defaults();
    let ctrl_g = Event::Key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('g'),
        KeyModifiers::CONTROL,
    ));
    let buf = draw_frame(&mut agent, area);
    assert!(agent.dock_shown);
    assert!(dock_row_has_tasks(&buf, agent.pane_areas.dock));
    assert!(matches!(
        agent.handle_input(&ctrl_g, &registry),
        InputOutcome::Changed
    ));
    let buf = draw_frame(&mut agent, area);
    assert!(agent.dock_on && agent.dock_hidden);
    assert!(!agent.dock_shown);
    assert_eq!(agent.pane_areas.dock.height, 0);
    assert!(!dock_row_has_tasks(&buf, agent.pane_areas.dock));
    assert!(matches!(
        agent.handle_input(&ctrl_g, &registry),
        InputOutcome::Changed
    ));
    let buf = draw_frame(&mut agent, area);
    assert!(agent.dock_shown && !agent.dock_hidden);
    assert!(dock_row_has_tasks(&buf, agent.pane_areas.dock));
}

fn buffer_text_rows(buf: &Buffer) -> Vec<String> {
    let area = *buf.area();
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| cell_symbol(buf, x, y))
                .collect()
        })
        .collect()
}

fn insert_described_subagent(
    agent: &mut AgentView,
    child_session_id: &str,
    description: &str,
    completed: bool,
) {
    let mut info = super::test_fixtures::running_subagent_info(child_session_id);
    info.description = std::sync::Arc::from(description);
    if completed {
        info.finished = true;
        info.set_finished_for_test(true);
        info.status = Some(std::sync::Arc::from("completed"));
        info.attempt.status = Some(std::sync::Arc::from("completed"));
    }
    agent
        .subagent_sessions
        .insert(child_session_id.to_string(), info);
    agent.insert_test_child(
        child_session_id.to_string(),
        Box::new(super::test_fixtures::make_agent()),
    );
}

/// `[clear]` on the Subagents list removes completed rows and leaves a running row.
#[test]
fn clear_button_on_the_subagent_list_removes_completed_rows_and_leaves_a_running_row() {
    let mut agent = make_agent();
    agent.tasks.toggle_show_done();
    insert_described_subagent(&mut agent, "running-job", "running-job", false);
    insert_described_subagent(&mut agent, "completed-job", "completed-job", true);
    let area = Rect::new(0, 0, 80, 30);
    let buf = draw_frame(&mut agent, area);
    let rows = buffer_text_rows(&buf);
    let painted = rows.join("\n");
    assert!(
        painted.contains("running-job"),
        "the list must show the running row: {painted}"
    );
    assert!(
        painted.contains("completed-job"),
        "the list must show the completed row: {painted}"
    );
    let (row, column) = rows
        .iter()
        .enumerate()
        .find_map(|(y, text)| text.find("[clear]").map(|x| (y as u16, x as u16)))
        .expect("[clear] painted on the Subagents list");
    let theme = crate::theme::Theme::current();
    let clear_fg = buf.cell((column, row)).expect("[clear] cell").fg;
    assert_eq!(clear_fg, theme.gray, "[clear] uses quiet secondary gray");
    assert_ne!(clear_fg, theme.accent_user);
    assert_ne!(clear_fg, theme.accent_running);
    let outcome = agent.handle_mouse(&mouse(MouseEventKind::Down(MouseButton::Left), column, row));
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking [clear] removes completed rows, got {outcome:?}"
    );
    assert!(
        !matches!(outcome, InputOutcome::Action(Action::KillSubagent(_))),
        "clicking [clear] must not send KillSubagent"
    );
    assert!(
        !agent.pending_effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::KillSubagent { subagent_id, .. } if subagent_id == "sa-running-job"
            )
        }),
        "clear must not kill the running subagent"
    );
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        !painted.contains("completed-job"),
        "the completed row must leave the list: {painted}"
    );
    assert!(
        painted.contains("running-job"),
        "the running row must stay on the list: {painted}"
    );
    assert!(agent.subagent_sessions.contains_key("running-job"));
    assert!(agent.subagent_sessions.contains_key("completed-job"));

    crate::views::dock::set_enabled_for_test(true);
    insert_described_subagent(&mut agent, "dock-completed-job", "dock-completed-job", true);
    let buf = draw_frame(&mut agent, area);
    let rows = buffer_text_rows(&buf);
    let painted = rows.join("\n");
    assert!(
        painted.contains("dock-completed-job"),
        "the dock must show the completed row: {painted}"
    );
    assert!(
        painted.contains("running-job"),
        "the dock must keep the running row: {painted}"
    );
    let (row, column) = rows
        .iter()
        .enumerate()
        .find_map(|(y, text)| text.find("[clear]").map(|x| (y as u16, x as u16)))
        .expect("dock [clear] painted");
    assert!(
        agent
            .pane_areas
            .dock
            .contains(ratatui::layout::Position::new(column, row)),
        "the painted [clear] cell is on the dock"
    );
    let outcome = agent.handle_input(
        &Event::Mouse(mouse(MouseEventKind::Down(MouseButton::Left), column, row)),
        &ActionRegistry::defaults(),
    );
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking the dock [clear] removes completed rows, got {outcome:?}"
    );
    assert!(
        !matches!(outcome, InputOutcome::Action(Action::KillSubagent(_))),
        "clicking the dock [clear] must not send KillSubagent"
    );
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        !painted.contains("dock-completed-job"),
        "the dock completed row must leave the list: {painted}"
    );
    assert!(
        painted.contains("running-job"),
        "the running row must stay on the dock: {painted}"
    );
    assert!(agent.subagent_sessions.contains_key("dock-completed-job"));
    assert!(agent.subagent_sessions.contains_key("running-job"));
    crate::views::dock::set_enabled_for_test(false);
}

fn find_phrase_inside(buf: &Buffer, phrase: &str, rect: Rect) -> Option<(u16, u16)> {
    let chars: Vec<char> = phrase.chars().collect();
    if chars.is_empty() || rect.width == 0 || rect.height == 0 {
        return None;
    }
    let phrase_len = u16::try_from(chars.len()).unwrap_or(u16::MAX);
    for y in rect.y..rect.bottom() {
        if rect.width < phrase_len {
            continue;
        }
        let mut col = rect.x;
        let last = rect.right().saturating_sub(phrase_len);
        while col <= last {
            let matched = chars
                .iter()
                .enumerate()
                .all(|(index, ch)| cell_symbol(buf, col + index as u16, y) == ch.to_string());
            if matched {
                return Some((col, y));
            }
            col += 1;
        }
    }
    None
}

fn find_phrase(buf: &Buffer, phrase: &str) -> Option<(u16, u16)> {
    let chars: Vec<char> = phrase.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let area = *buf.area();
    for y in area.y..area.bottom() {
        let mut col = area.x;
        while col as usize + chars.len() <= area.right() as usize {
            let matched = chars
                .iter()
                .enumerate()
                .all(|(index, ch)| cell_symbol(buf, col + index as u16, y) == ch.to_string());
            if matched {
                return Some((col, y));
            }
            col += 1;
        }
    }
    None
}

fn click_left(agent: &mut AgentView, column: u16, row: u16) -> InputOutcome {
    agent.handle_input(
        &Event::Mouse(mouse(MouseEventKind::Down(MouseButton::Left), column, row)),
        &ActionRegistry::defaults(),
    )
}

/// A left click on the painted dock `[clear]` removes completed rows and leaves the running row.
#[test]
fn clicking_clear_removes_completed_subagent_rows_and_leaves_the_running_row() {
    crate::views::dock::set_enabled_for_test(true);
    let mut agent = make_agent();
    agent.tasks.toggle_show_done();
    insert_described_subagent(&mut agent, "running-job", "running-job", false);
    insert_described_subagent(&mut agent, "completed-job", "completed-job", true);
    let area = Rect::new(0, 0, 80, 30);
    let buf = draw_frame(&mut agent, area);
    let (column, row) = find_phrase(&buf, "[clear]").expect("painted [clear]");
    assert!(
        agent
            .pane_areas
            .dock
            .contains(ratatui::layout::Position::new(column, row)),
        "the click is the painted dock [clear], not a helper"
    );
    let outcome = click_left(&mut agent, column, row);
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking [clear] removes completed rows, got {outcome:?}"
    );
    assert!(
        !matches!(outcome, InputOutcome::Action(Action::KillSubagent(_))),
        "clicking [clear] must not send KillSubagent, got {outcome:?}"
    );
    assert!(
        !agent
            .pending_effects
            .iter()
            .any(|effect| { matches!(effect, Effect::KillSubagent { .. }) }),
        "clear must not kill"
    );
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        !painted.contains("completed-job"),
        "the completed row must leave the list: {painted}"
    );
    assert!(
        painted.contains("running-job"),
        "the running row must stay on the list: {painted}"
    );
    assert!(agent.subagent_sessions.contains_key("running-job"));
    assert!(agent.subagent_sessions.contains_key("completed-job"));
    crate::views::dock::set_enabled_for_test(false);
}

/// `[clear]` rests yellow and turns white on hover. It does not become Operator green or magenta.
#[test]
fn clear_button_turns_from_yellow_to_white_on_hover() {
    let _pin = crate::theme::cache::pin_theme();
    crate::theme::cache::set(crate::theme::ThemeKind::Doge);
    crate::views::dock::set_enabled_for_test(true);
    let mut agent = make_agent();
    agent.tasks.toggle_show_done();
    insert_described_subagent(&mut agent, "running-job", "running-job", false);
    insert_described_subagent(&mut agent, "completed-job", "completed-job", true);
    let area = Rect::new(0, 0, 80, 30);
    let buf = draw_frame(&mut agent, area);
    let (column, row) = find_phrase(&buf, "[clear]").expect("painted [clear]");
    let theme = crate::theme::Theme::current();
    let rest = buf.cell((column, row)).expect("[clear] cell").fg;
    assert_eq!(rest, theme.gray);
    assert_eq!(rest, ratatui::style::Color::Rgb(255, 255, 0));
    assert_ne!(rest, theme.accent_user);
    assert_ne!(rest, theme.accent_running);
    let _ = agent.handle_input(
        &Event::Mouse(mouse(MouseEventKind::Moved, column, row)),
        &ActionRegistry::defaults(),
    );
    let buf = draw_frame(&mut agent, area);
    let hovered = buf.cell((column, row)).expect("[clear] cell").fg;
    assert_eq!(hovered, ratatui::style::Color::Rgb(255, 255, 255));
    assert_eq!(hovered, theme.text_primary);
    assert_ne!(hovered, theme.accent_user);
    assert_ne!(hovered, theme.accent_running);
    let _ = agent.handle_input(
        &Event::Mouse(mouse(MouseEventKind::Moved, 0, 0)),
        &ActionRegistry::defaults(),
    );
    let buf = draw_frame(&mut agent, area);
    let (column, row) = find_phrase(&buf, "[clear]").expect("painted [clear] after hover");
    let rest = buf.cell((column, row)).expect("[clear] cell").fg;
    assert_eq!(rest, ratatui::style::Color::Rgb(255, 255, 0));
    crate::theme::cache::set(crate::theme::ThemeKind::GrokNight);
    crate::views::dock::set_enabled_for_test(false);
}

/// A left click on `1 specialist` opens that running L3. Esc or q returns to the L2.
#[test]
fn clicking_the_specialist_count_opens_that_l3() {
    use agent_client_protocol as acp;
    use std::sync::Arc;
    const L2: &str = "l2-session";
    const L3: &str = "l3-session";
    const LINE: &str = "L3_TRANSCRIPT_FROM_SPECIALIST_COUNT";
    crate::views::dock::set_enabled_for_test(true);
    let registry = ActionRegistry::defaults();
    let mut agent = make_agent();
    agent.session.session_id = Some(acp::SessionId::new("l1-session"));
    let mut l2_info = super::test_fixtures::running_subagent_info(L2);
    l2_info.description = Arc::from("coordinator-row-stays");
    l2_info.attempt.depth = Some(1);
    l2_info.attempt.parent_session_id = Some(Arc::from("l1-session"));
    agent.subagent_sessions.insert(L2.to_owned(), l2_info);
    agent.insert_subagent_view(
        L2.to_owned(),
        Box::new(super::test_fixtures::make_agent()),
        super::ChildLink::l2_coordinator(acp::SessionId::new("l1-session")),
    );
    let mut l3 = super::test_fixtures::make_agent();
    l3.session.session_id = Some(acp::SessionId::new(L3));
    l3.scrollback
        .push_block(crate::scrollback::block::RenderBlock::user_prompt(LINE));
    let mut l3_info = super::test_fixtures::running_subagent_info(L3);
    l3_info.attempt.depth = Some(2);
    l3_info.attempt.parent_session_id = Some(Arc::from(L2));
    l3_info.description = Arc::from("specialist-name-must-not-be-the-row");
    agent.subagent_sessions.insert(L3.to_owned(), l3_info);
    agent.insert_test_child(L3.to_owned(), Box::new(l3));
    let area = Rect::new(0, 0, 120, 40);
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains("coordinator-row-stays"),
        "the L2 row is on the list: {painted}"
    );
    assert!(
        painted.contains("1 specialist"),
        "the live count is on the L2 row: {painted}"
    );
    let (column, row) = find_phrase(&buf, "1 specialist").expect("painted 1 specialist");
    assert!(
        agent
            .pane_areas
            .dock
            .contains(ratatui::layout::Position::new(column + 2, row)),
        "the count click is on the dock"
    );
    let outcome = click_left(&mut agent, column + 2, row);
    assert!(
        matches!(outcome, InputOutcome::Changed),
        "clicking the count opens the L3, got {outcome:?}"
    );
    assert!(
        !matches!(outcome, InputOutcome::Action(Action::KillSubagent(_))),
        "the count click must not send KillSubagent, got {outcome:?}"
    );
    assert_eq!(agent.active_subagent.as_deref(), Some(L3));
    assert_eq!(agent.overlay_return_sid.as_deref(), Some(L2));
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains(LINE),
        "the open L3 view must show its transcript: {painted}"
    );
    assert_eq!(
        crate::app::subagent::live_l3_count(agent.subagent_sessions.values(), L2),
        1
    );
    assert_eq!(
        crate::app::subagent::format_live_l3_count(1).as_deref(),
        Some("1 specialist")
    );
    assert!(agent.subagent_sessions.contains_key(L2));
    let back = agent.handle_input(&super::test_fixtures::key(KeyCode::Char('q')), &registry);
    assert!(
        !matches!(back, InputOutcome::Action(Action::KillSubagent(_))),
        "q from the L3 must not send KillSubagent, got {back:?}"
    );
    assert_eq!(agent.active_subagent.as_deref(), Some(L2));
    let back = agent.handle_input(&super::test_fixtures::key(KeyCode::Esc), &registry);
    assert!(
        !matches!(back, InputOutcome::Action(Action::KillSubagent(_))),
        "{back:?}"
    );
    assert!(agent.active_subagent.is_none());
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains("coordinator-row-stays"),
        "the L2 row stays: {painted}"
    );
    assert!(
        painted.contains("1 specialist"),
        "the count stays: {painted}"
    );
    let (column, row) = find_phrase(&buf, "1 specialist").expect("painted 1 specialist");
    let outcome = click_left(&mut agent, column + 2, row);
    assert_eq!(agent.active_subagent.as_deref(), Some(L3));
    assert!(
        !matches!(outcome, InputOutcome::Action(Action::KillSubagent(_))),
        "{outcome:?}"
    );
    let back = agent.handle_input(&super::test_fixtures::key(KeyCode::Esc), &registry);
    assert!(
        !matches!(back, InputOutcome::Action(Action::KillSubagent(_))),
        "Esc from the L3 must not send KillSubagent, got {back:?}"
    );
    assert_eq!(agent.active_subagent.as_deref(), Some(L2));
    crate::views::dock::set_enabled_for_test(false);
}

/// The Subagents list paints a scrollbar when the rows are taller than the panel.
/// Scrolling reaches every row, including the last one.
#[test]
fn subagent_list_scrolls_when_rows_overflow_the_panel() {
    let mut agent = make_agent();
    let started = std::time::Instant::now();
    for index in 0..16 {
        let id = format!("scroll-row-{index:02}");
        insert_described_subagent(&mut agent, &id, &id, false);
        let info = agent
            .subagent_sessions
            .get_mut(&id)
            .expect("inserted subagent");
        info.attempt.started_at = started + std::time::Duration::from_secs(index);
    }
    let area = Rect::new(0, 0, 80, 30);
    let buf = draw_frame(&mut agent, area);
    let tasks = agent.pane_areas.tasks;
    assert!(tasks.height > 0, "the Subagents list must be on screen");
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains("scroll-row-00"),
        "the first row must be on screen before scrolling: {painted}"
    );
    assert!(
        !painted.contains("scroll-row-15"),
        "the last row starts below the panel: {painted}"
    );
    let thumb = (tasks.x..tasks.right())
        .flat_map(|x| (tasks.y..tasks.bottom()).map(move |y| (x, y)))
        .find(|(x, y)| {
            buf.cell((*x, *y))
                .is_some_and(|cell| cell.symbol() == "\u{2588}")
        });
    let (thumb_x, thumb_y) = thumb.expect("a scrollbar thumb on the Subagents list");
    let theme = crate::theme::Theme::current();
    let thumb_fg = buf.cell((thumb_x, thumb_y)).expect("thumb cell").fg;
    assert_ne!(thumb_fg, theme.accent_user);
    assert_ne!(thumb_fg, theme.accent_running);
    let scroll_at = (tasks.x + 1, tasks.y);
    for _ in 0..24 {
        agent.handle_scroll(1, scroll_at.0, scroll_at.1);
    }
    let buf = draw_frame(&mut agent, area);
    let painted = buffer_text_rows(&buf).join("\n");
    assert!(
        painted.contains("scroll-row-15"),
        "scrolling must reach the last row: {painted}"
    );
    assert!(
        !painted.contains("scroll-row-00"),
        "scrolling past the top leaves the first row above the panel: {painted}"
    );
}
