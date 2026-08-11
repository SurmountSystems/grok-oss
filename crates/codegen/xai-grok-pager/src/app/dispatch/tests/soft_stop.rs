//! Dispatch integration tests for soft stop (finish turn, then hold queue).

use super::*;
use crate::app::soft_stop::SoftStopPhase;

fn enqueue_two(app: &mut AppView, id: AgentId) {
    enqueue_local(app, id, "first");
    enqueue_local(app, id, "second");
}

#[test]
fn soft_stop_with_non_empty_queue_does_not_drain_next() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    enqueue_two(&mut app, id);
    // Start the first item.
    let effects = maybe_drain_queue_and_note_peek(&mut app, id);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "first")),
        "first should start: {effects:?}"
    );
    assert_eq!(app.agents[&id].session.pending_prompts.len(), 1);

    // Arm soft stop mid-turn (does not cancel).
    let _ = dispatch(Action::ToggleSoftStop, &mut app);
    assert!(app.soft_stop.is_armed());
    assert!(!app.soft_stop.blocks_drain());

    // Finish the turn: soft stop takes effect; second stays queued.
    app.agents.get_mut(&id).unwrap().session.state = AgentState::Idle;
    app.agents.get_mut(&id).unwrap().session.current_prompt_id = None;
    let toast = app
        .soft_stop
        .on_top_level_turn_finished()
        .expect("take effect");
    assert!(toast.contains("queue held"), "{toast}");
    assert!(app.soft_stop.is_holding());

    let blocked = maybe_drain_queue_and_note_peek(&mut app, id);
    assert!(
        blocked.is_empty(),
        "holding must not drain second: {blocked:?}"
    );
    assert_eq!(app.agents[&id].session.pending_prompts.len(), 1);
    assert_eq!(
        app.agents[&id]
            .session
            .pending_prompts
            .front()
            .unwrap()
            .text,
        "second"
    );
}

#[test]
fn unarmed_continues_queue_drain() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    enqueue_two(&mut app, id);
    assert!(app.soft_stop.is_off());
    let effects = maybe_drain_queue_and_note_peek(&mut app, id);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. })),
        "unarmed drain starts work: {effects:?}"
    );
}

#[test]
fn soft_stop_distinct_from_global_pause_mid_turn() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;
    app.agents.get_mut(&id).unwrap().session.in_flight_prompt =
        Some(crate::app::agent::InFlightPrompt {
            text: "running".into(),
            images: vec![],
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: vec![],
            chip_elements: vec![],
        });

    // Soft stop does not cancel.
    let soft_effects = dispatch(Action::ToggleSoftStop, &mut app);
    assert!(
        !soft_effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "soft stop must not cancel mid-turn: {soft_effects:?}"
    );
    assert_eq!(app.soft_stop.phase(), SoftStopPhase::Armed);
    assert!(app.agents[&id].session.state.is_turn_running());

    // Global pause still cancels.
    let pause_effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(
        pause_effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "global pause must cancel: {pause_effects:?}"
    );
}

#[test]
fn disarm_before_turn_ends_allows_drain() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    enqueue_local(&mut app, id, "after disarm");
    let _ = dispatch(Action::ToggleSoftStop, &mut app);
    assert!(app.soft_stop.is_armed());
    let _ = dispatch(Action::ToggleSoftStop, &mut app);
    assert!(app.soft_stop.is_off());
    // Turn finished while unarmed: no hold.
    assert!(app.soft_stop.on_top_level_turn_finished().is_none());
    let effects = maybe_drain_queue_and_note_peek(&mut app, id);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "after disarm")),
        "disarmed must drain: {effects:?}"
    );
}

#[test]
fn release_holding_resumes_drain() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    enqueue_local(&mut app, id, "held then released");
    let _ = dispatch(Action::ToggleSoftStop, &mut app);
    let _ = app.soft_stop.on_top_level_turn_finished();
    assert!(app.soft_stop.is_holding());
    assert!(maybe_drain_queue_and_note_peek(&mut app, id).is_empty());

    let effects = dispatch(Action::ToggleSoftStop, &mut app);
    assert!(app.soft_stop.is_off());
    let drained = effects
        .iter()
        .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "held then released"))
        || app.agents[&id].session.state.is_turn_running()
        || maybe_drain_queue_and_note_peek(&mut app, id)
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "held then released"));
    assert!(
        drained,
        "release must allow queue to continue; effects={effects:?}"
    );
}
