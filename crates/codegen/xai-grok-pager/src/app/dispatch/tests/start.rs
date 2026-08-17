//! `/start` starts paused or interrupted work in the current session.
//!
//! Not `/resume`: that command only opens the session picker.

use super::*;

#[test]
fn start_while_globally_paused_continues_interrupted_turn_once() {
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

    let start_effects = dispatch(Action::SendPrompt("/start".into()), &mut app);
    assert!(
        !app.global_work_pause.is_active(),
        "/start must unpause; must not leave global pause engaged"
    );
    let requeued = app.agents[&id]
        .session
        .pending_prompts
        .iter()
        .any(|p| p.text == "work item")
        || start_effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "work item"));
    assert!(
        requeued,
        "/start must continue the interrupted prompt once; effects={start_effects:?} queue={:?}",
        app.agents[&id]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        !start_effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "/start")),
        "/start must not send itself as a prompt: {start_effects:?}"
    );
    assert!(
        !app.session_picker_loading,
        "/start must not open the session picker"
    );
}

#[test]
fn start_on_idle_clean_session_does_not_invent_a_turn() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    assert!(app.agents[&id].session.state.is_idle());
    assert!(!app.global_work_pause.is_active());
    assert!(app.agents[&id].session.pending_prompts.is_empty());

    let effects = dispatch(Action::SendPrompt("/start".into()), &mut app);

    assert!(
        !app.global_work_pause.is_active(),
        "/start on an idle session must not engage global pause"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::CancelTurn { .. })),
        "idle /start must not invent a turn: {effects:?}"
    );
    assert!(
        app.agents[&id].session.pending_prompts.is_empty(),
        "idle /start must not enqueue a fake prompt; queue={:?}",
        app.agents[&id]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        app.agents[&id].session.state.is_idle(),
        "idle /start must leave the session idle; state={:?}",
        app.agents[&id].session.state
    );
    assert!(
        !app.session_picker_loading,
        "/start must not open the session picker"
    );
    let toast = app.agents[&id]
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("no paused or interrupted work"),
        "idle /start must tell the operator nothing is held; toast={toast:?}"
    );
}

#[test]
fn start_with_cancel_resume_marker_continues_interrupted_turn() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "start-cmd-resume-sess";
    let cwd = std::env::temp_dir().join("start-cmd-resume-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    // Operator typed /start: apply even if the restart setting is off.
    app.current_ui.resume_canceled_turn_on_restart = Some(false);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.pending_prompts.clear();
    }
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        "finish the held turn after Esc",
        Some("pid-start-cmd"),
        "2026-08-16T12:00:00Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write marker");

    let effects = dispatch(Action::SendPrompt("/start".into()), &mut app);

    let agent = app.agents.get(&id).unwrap();
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("Continuing interrupted turn"),
        "/start with a cancel-resume marker must toast continue-interrupted-turn; got {toast:?}"
    );
    let started = effects.iter().any(|e| {
        matches!(
            e,
            Effect::SendPrompt { text, .. } if text == "finish the held turn after Esc"
        )
    });
    assert!(
        started,
        "/start with a cancel-resume marker must emit SendPrompt of the held work, got {effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text == "/start")),
        "/start must not send itself as a prompt: {effects:?}"
    );
    assert!(
        !app.session_picker_loading,
        "/start must not open the session picker"
    );

    let remaining =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load after start");
    assert!(
        remaining.is_none(),
        "/start must clear the cancel-resume marker after continuing"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}
