//! Dispatch integration tests for fearless global pause / resume.

use super::*;
use crate::app::global_work_pause::GlobalWorkPause;
use std::time::Instant;

fn add_second_agent(app: &mut AppView) -> AgentId {
    let id = AgentId(app.next_agent_id);
    app.next_agent_id += 1;
    let session = make_test_agent_session(app, id, &format!("sess-{}", id.0));
    let mut agent = AgentView::new(session, ScrollbackState::new());
    agent.active_pane = ActivePane::Scrollback;
    app.agents.insert(id, agent);
    id
}

#[test]
fn pause_mid_turn_then_resume_continues_once() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let effects = dispatch(Action::SendPrompt("work item".into()), &mut app);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. })),
        "expected SendPrompt effect, got {effects:?}"
    );
    assert!(app.agents[&id].session.state.is_turn_running());
    if app.agents[&id].session.in_flight_prompt.is_none() {
        app.agents.get_mut(&id).unwrap().session.in_flight_prompt =
            Some(crate::app::agent::InFlightPrompt {
                text: "work item".into(),
                images: vec![],
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: vec![],
                chip_elements: vec![],
            });
    }

    let pause_effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(app.global_work_pause.is_active());
    assert!(
        pause_effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "pause must cancel the running turn: {pause_effects:?}"
    );
    app.agents.get_mut(&id).unwrap().session.state = AgentState::Idle;
    app.agents.get_mut(&id).unwrap().session.current_prompt_id = None;

    let resume_effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(!app.global_work_pause.is_active());
    let requeued = app.agents[&id]
        .session
        .pending_prompts
        .iter()
        .any(|p| p.text == "work item")
        || resume_effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "work item"));
    assert!(
        requeued,
        "resume must re-queue interrupted prompt once; effects={resume_effects:?} queue={:?}",
        app.agents[&id]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );

    // Clear residual work; second pause/resume with nothing pending invents nothing.
    app.agents
        .get_mut(&id)
        .unwrap()
        .session
        .pending_prompts
        .clear();
    app.agents.get_mut(&id).unwrap().session.state = AgentState::Idle;
    app.agents.get_mut(&id).unwrap().session.in_flight_prompt = None;
    let _ = dispatch(Action::ToggleGlobalPause, &mut app);
    let effects2 = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(
        !effects2
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. })),
        "resume with nothing pending must not start work: {effects2:?}"
    );
}

#[test]
fn resume_with_nothing_pending_does_nothing() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    assert!(app.agents[&id].session.state.is_idle());
    let _ = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(app.global_work_pause.is_active());
    assert_eq!(app.global_work_pause.sessions_held_count(), 0);
    let effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(!app.global_work_pause.is_active());
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::CancelTurn { .. })),
        "idle resume must not invent work: {effects:?}"
    );
}

#[test]
fn finished_agent_not_re_spawned() {
    let mut app = test_app_with_agent();
    let a = AgentId(0);
    let b = add_second_agent(&mut app);
    enqueue_local(&mut app, b, "still waiting");
    switch_to_agent(&mut app, a, SwitchCause::New);

    let before_count = app.agents.len();
    let _ = dispatch(Action::ToggleGlobalPause, &mut app);
    assert_eq!(app.global_work_pause.sessions_held_count(), 1);
    // Finished session a must not grow a resume stash.
    assert!(
        app.global_work_pause
            .snapshots()
            .get(&a)
            .is_none_or(|s| !s.needs_resume_requeue()),
        "finished agent must not be scheduled for re-spawn"
    );
    let effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert_eq!(
        app.agents.len(),
        before_count,
        "resume must not create agents"
    );
    assert!(app.agents.contains_key(&a) && app.agents.contains_key(&b));
    // No inventing prompts for finished agent a.
    assert!(
        app.agents[&a].session.pending_prompts.is_empty(),
        "finished agent must not receive invented queue rows"
    );
    let b_continued = app.agents[&b].session.state.is_turn_running()
        || app.agents[&b]
            .session
            .pending_prompts
            .iter()
            .any(|p| p.text == "still waiting")
        || effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "still waiting"));
    assert!(
        b_continued,
        "waiting session must keep/continue its real pending work; effects={effects:?}"
    );
}

#[test]
fn pause_holds_all_sessions_and_tracks_count_duration() {
    let mut app = test_app_with_agent();
    let a = AgentId(0);
    let b = add_second_agent(&mut app);
    app.agents.get_mut(&a).unwrap().session.state = AgentState::TurnRunning;
    app.agents.get_mut(&a).unwrap().session.in_flight_prompt =
        Some(crate::app::agent::InFlightPrompt {
            text: "a turn".into(),
            images: vec![],
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: vec![],
            chip_elements: vec![],
        });
    app.agents.get_mut(&b).unwrap().session.state = AgentState::TurnRunning;
    app.agents.get_mut(&b).unwrap().session.in_flight_prompt =
        Some(crate::app::agent::InFlightPrompt {
            text: "b turn".into(),
            images: vec![],
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: vec![],
            chip_elements: vec![],
        });
    let effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(app.global_work_pause.is_active());
    assert_eq!(app.global_work_pause.sessions_held_count(), 2);
    let cancel_count = effects
        .iter()
        .filter(|e| matches!(e, Effect::CancelTurn { .. }))
        .count();
    assert_eq!(cancel_count, 2, "both sessions cancelled: {effects:?}");
    let label = app
        .global_work_pause
        .status_label(Instant::now())
        .expect("paused label");
    assert!(label.contains("2 sessions"), "{label}");
    assert!(label.contains("Paused"), "{label}");
    assert!(
        GlobalWorkPause::disengage_toast(2, true).contains("interrupted turns"),
        "toast naming"
    );
}

#[test]
fn drain_blocked_while_paused() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    enqueue_local(&mut app, id, "held while paused");
    let _ = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(app.global_work_pause.is_active());
    let effects = maybe_drain_queue_and_note_peek(&mut app, id);
    assert!(
        effects.is_empty(),
        "queue must not drain while paused: {effects:?}"
    );
    assert_eq!(app.agents[&id].session.pending_prompts.len(), 1);
}
