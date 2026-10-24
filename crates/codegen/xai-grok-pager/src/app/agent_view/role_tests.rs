use super::{ComposerRoute, ViewSurface};
use crate::actions::ActionRegistry;
use crate::app::actions::Effect;
use crate::app::agent::AgentState;
use crate::app::agent_view::test_fixtures::{
    add_running_bg_task, add_running_execute, ctrl, make_agent, parent_with_child,
    running_subagent_info, simulate_subagent_wait, simulate_task_output_wait,
};
use crate::app::agent_view::{AgentPane, AgentView};
use crate::scrollback::render::ScratchBuffer;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
/// A root view derives every legacy answer: it accepts prompt focus and keeps the demote affordance.
#[test]
fn root_surface_reproduces_legacy_sites() {
    let registry = ActionRegistry::defaults();
    let mut agent = make_agent();
    assert_eq!(ViewSurface::Root, agent.surface());
    assert_eq!(ComposerRoute::RootSession, agent.composer_route());
    assert!(agent.child_link().is_none());
    assert!(agent.set_active_pane(AgentPane::Prompt, false));
    assert_eq!(AgentPane::Prompt, agent.active_pane);
    add_running_execute(&mut agent);
    assert!(
        agent
            .current_shortcut_hints(&registry)
            .iter()
            .all(|hint| hint.label != "send to bg"),
        "GitHub #232: send to bg is not an Operator hint on L1"
    );
}
/// The composer is unreachable on a child: the constructor moves a prompt-focused view off the prompt, every later
/// switch is refused (forced or not), and the queue pane's Down off its last row leaves the queue focused and intact.
/// (`FocusPrompt` via dispatch is pinned in the prompt router tests; the zero-row composer in the takeover draw test.)
#[test]
fn child_unaddressable_hides_composer_and_refuses_prompt_pane() {
    let registry = ActionRegistry::defaults();
    let mut parent = make_agent();
    let mut child = make_agent();
    assert!(child.set_active_pane(AgentPane::Prompt, true));
    parent.insert_test_child(String::from("child"), Box::new(child));
    let child = parent.subagent_view_mut("child").expect("child view");
    assert_eq!(ViewSurface::ChildTakeover, child.surface());
    assert_eq!(ComposerRoute::Hidden, child.composer_route());
    assert_eq!(AgentPane::Scrollback, child.active_pane);
    assert!(!child.set_active_pane(AgentPane::Prompt, true));
    assert!(!child.set_active_pane(AgentPane::Prompt, false));
    assert_eq!(AgentPane::Scrollback, child.active_pane);
    child.shared_queue = vec![crate::app::prompt_queue::QueueEntryWire {
        id: String::from("p1"),
        version: 1,
        owner: None,
        last_editor: None,
        kind: String::from("prompt"),
        text: String::from("queued"),
        combined_texts: None,
        position: 0,
    }];
    child.sync_queue_pane();
    child.queue.overlay.visible = true;
    assert!(child.set_active_pane(AgentPane::Queue, false));
    child.queue.overlay.focused = true;
    let last = *child.queue.entry_ids().last().expect("one row");
    child.queue.list_state.select_by_id(last);
    child.handle_input(
        &Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        &registry,
    );
    assert_eq!(AgentPane::Queue, child.active_pane);
    assert!(child.queue.overlay.focused);
    assert!(child.prompt.text().is_empty());
    let parent = parent_with_child("child");
    assert_eq!(
        AgentPane::Scrollback,
        parent
            .subagent_view("child")
            .expect("child view")
            .active_pane
    );
}
/// A child's bg-task kill would be sent on the root session, so the child surface never advertises `x kill`; the root keeps it.
#[test]
fn child_surface_never_advertises_kill() {
    let registry = ActionRegistry::defaults();
    let mut parent = parent_with_child("child");
    let advertises_kill = |agent: &mut AgentView| {
        add_running_bg_task(agent);
        agent.handle_input(&ctrl('g'), &registry);
        assert_eq!(AgentPane::Tasks, agent.active_pane);
        let area = Rect::new(0, 0, 80, 30);
        agent.draw(
            area,
            &mut Buffer::empty(area),
            &registry,
            &mut ScratchBuffer::new(),
            None,
            false,
            crate::app::agent_view::BannerSlotParams::none(),
            false,
            false,
            &mut Vec::new(),
            crate::app::agent_view::AppRenderParams::default(),
        );
        agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            &registry,
        );
        assert!(agent.tasks.selected_task_id().is_some());
        agent
            .current_shortcut_hints(&registry)
            .iter()
            .any(|hint| hint.label == "kill")
    };
    let child_advertises_kill =
        advertises_kill(parent.subagent_view_mut("child").expect("child view"));
    assert!(!child_advertises_kill);
    assert!(advertises_kill(&mut parent));
}

/// Operator 2026-10-23: "also it looks like you just got rid of the capability
/// to send blocking subagents to the background instead of automatically
/// backgrounding agents that are blocking the L1 context"
///
/// GitHub #243: keep send-to-background for a blocking nested agent, and
/// auto-background that same wait so L1 is not stuck. Do not delete one to
/// stand in for the other.
#[test]
fn send_blocking_subagent_to_background_is_kept_and_l1_auto_backgrounds() {
    let mut agent = make_agent();
    agent.session.session_id = Some(agent_client_protocol::SessionId::new("test-session"));
    agent.session.state = AgentState::TurnRunning;
    simulate_subagent_wait(&mut agent);
    agent
        .subagent_sessions
        .insert("child-sid".into(), running_subagent_info("child-sid"));
    assert!(
        agent.can_send_to_background(),
        "send-to-background for a blocking subagent must still exist"
    );
    assert!(
        agent
            .l1_blocking_subagent_auto_background_effect()
            .is_some(),
        "L1 must auto-background a blocking nested agent so the Operator can type"
    );

    let effect = agent.take_send_blocking_subagent_to_background().expect(
        "Ctrl+B / send-to-background must still send a blocking subagent to the background",
    );
    assert!(
        matches!(
            effect,
            Effect::DemoteToBackground { tool_call_id, .. }
                if tool_call_id == "task-tc-1" || tool_call_id == "sa-child-sid"
        ),
        "send-to-background must still emit DemoteToBackground, got {effect:?}"
    );
    assert!(
        !agent.has_running_foreground_subagent(),
        "send-to-background must move the blocking subagent to the background"
    );
}

/// Operator 2026-10-23: a blocking `get_command_or_subagent_output` wait is
/// an agent blocking the L1 context. Keep send-to-background. Also
/// auto-background so the Operator box is not stuck.
#[test]
fn send_blocking_get_command_or_subagent_output_wait_to_background_is_kept() {
    let mut agent = make_agent();
    agent.session.session_id = Some(agent_client_protocol::SessionId::new("test-session"));
    agent.session.state = AgentState::TurnRunning;
    simulate_task_output_wait(&mut agent, "sa-child-sid");
    assert!(
        agent.can_send_to_background(),
        "send-to-background for a blocking get_command_or_subagent_output wait must still exist"
    );
    let auto = agent
        .take_l1_blocking_auto_background()
        .expect("L1 must auto-background a blocking nested wait");
    assert!(
        matches!(
            auto,
            Effect::DemoteToBackground { tool_call_id, .. } if tool_call_id == "wait-1"
        ),
        "auto-background must not be deleted as a stand-in for send-to-background, got {auto:?}"
    );
    assert!(
        agent
            .session
            .tracker
            .blocking_subagent_tool_call_id()
            .is_none(),
        "auto-background must clear the blocking get_command_or_subagent_output wait"
    );
}
