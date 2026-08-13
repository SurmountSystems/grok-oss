//! Tests for turn cancellation, subagent kills, and cancel preferences.

use super::*;

#[test]
fn demote_dispatch_keeps_turn_session_and_execute_guards() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);

    assert!(dispatch(Action::DemoteToBackground, &mut app).is_empty());

    crate::app::agent_view::test_fixtures::add_running_execute(app.agents.get_mut(&id).unwrap());
    let effects = dispatch(Action::DemoteToBackground, &mut app);
    assert!(matches!(
        effects.as_slice(),
        [Effect::DemoteToBackground {
            session_id,
            tool_call_id,
        }] if session_id.0.as_ref() == "test-session" && tool_call_id == "exec-1"
    ));

    app.agents.get_mut(&id).unwrap().session.state = AgentState::Idle;
    assert!(dispatch(Action::DemoteToBackground, &mut app).is_empty());
}

/// Regression (leader mode): a queued prompt's parked `session/prompt` RPC
/// can resolve as an *error* — e.g. its `respond_to` is dropped on the
/// leader when the prompt is removed from the shared queue, surfacing as
/// `Internal error: "session failed to respond"`. An `acp::Error` carries
/// no `promptId`, so before the Err-arm gate this error was misattributed
/// to the running turn and rendered as a spurious "Turn failed", detonating
/// an unrelated in-flight turn. The handler now gates the Err arm on the
/// `prompt_id` the pager minted for that RPC: an error whose id is NOT the
/// running turn is discarded; the running turn is left untouched.
#[test]
fn queued_prompt_rpc_error_does_not_kill_running_turn() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);

    // First prompt drains immediately → Running. Capture its prompt_id.
    let effects = dispatch(Action::SendPrompt("running".into()), &mut app);
    let running_pid = match &effects[0] {
        Effect::SendPrompt { prompt_id, .. } => prompt_id.clone(),
        other => panic!("expected SendPrompt, got {other:?}"),
    };
    assert!(app.agents[&id].session.state.is_turn_running());
    assert_eq!(
        app.agents[&id].session.current_prompt_id.as_deref(),
        Some(running_pid.as_str())
    );

    // Second prompt typed while running → immediate server-authoritative
    // send (queued at the leader). Capture its prompt_id.
    let effects = dispatch(Action::SendPrompt("queued".into()), &mut app);
    let queued_pid = match &effects[0] {
        Effect::SendPrompt { prompt_id, .. } => prompt_id.clone(),
        other => panic!("expected immediate SendPrompt, got {other:?}"),
    };
    assert_ne!(running_pid, queued_pid);

    let scrollback_before = app.agents[&id].scrollback.len();

    // The queued prompt is removed; its parked RPC resolves Err.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Internal error: session failed to respond".to_string()),
            http_status: None,
            prompt_id: Some(queued_pid.clone()),
        }),
        &mut app,
    );

    // Discarded: no effects, running turn untouched, no "Turn failed" block.
    assert!(
        effects.is_empty(),
        "a queued prompt's RPC error must be discarded, got {effects:?}"
    );
    assert!(
        app.agents[&id].session.state.is_turn_running(),
        "the running turn must survive a queued prompt's RPC error"
    );
    assert_eq!(
        app.agents[&id].session.current_prompt_id.as_deref(),
        Some(running_pid.as_str()),
        "current_prompt_id must still point at the running turn"
    );
    assert_eq!(
        app.agents[&id].scrollback.len(),
        scrollback_before,
        "no TurnFailed block may be pushed for a non-running prompt's error"
    );

    // Sanity: an error for the ACTUAL running prompt is NOT discarded — it
    // ends the turn and renders the failure.
    let _ = dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("upstream boom".to_string()),
            http_status: None,
            prompt_id: Some(running_pid.clone()),
        }),
        &mut app,
    );
    assert!(
        !app.agents[&id].session.state.is_turn_running(),
        "the running turn's own error must end the turn"
    );
    assert!(
        app.agents[&id].scrollback.len() > scrollback_before,
        "the running turn's own error must render a failure block"
    );
}

#[test]
fn cta_install_done_skills_only_settles_installed_without_fetch() {
    use crate::app::agent_view::CtaPhase;
    use xai_hooks_plugins_types::OutcomeStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::Installing {
            plugin_relative_path: "plugins/figma".into(),
            name: "figma".into(),
        };
        cta.expects_mcp = false;
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::CtaPluginInstallDone {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(cta_outcome(OutcomeStatus::Success, "installed")),
        }),
        &mut app,
    );
    // No MCP fetch, no "Setting up…" flash: straight to Installed.
    assert_eq!(
        app.agents[&id].plugin_cta.phase,
        CtaPhase::Installed {
            name: "figma".into()
        }
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaMcps { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::DismissCtaInstalled { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaCatalog { .. }))
    );
}

#[test]
fn cta_reload_done_skills_only_settles_installed_without_fetch() {
    use crate::app::agent_view::CtaPhase;
    use xai_hooks_plugins_types::OutcomeStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::AwaitingReload {
            name: "figma".into(),
        };
        cta.expects_mcp = false;
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::CtaPluginReloadDone {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(cta_outcome(OutcomeStatus::Success, "reloaded")),
        }),
        &mut app,
    );
    assert_eq!(
        app.agents[&id].plugin_cta.phase,
        CtaPhase::Installed {
            name: "figma".into()
        }
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaMcps { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::DismissCtaInstalled { .. }))
    );
}

#[test]
fn cancel_turn_without_subagents_cancels_immediately() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            cancel_subagents: true,
            ..
        }
    ));
    assert!(app.agents[&id].session.state.is_cancelling());
}

/// Cancel inside a subagent drill-in view kills the focused running subagent
/// instead of resolving the root turn. The root is idle here, so only the kill
/// path reaches the coordinator-run child.
#[test]
fn cancel_turn_in_subagent_view_kills_focused_subagent() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::Idle;
        agent
            .subagent_sessions
            .insert("child-1".to_string(), make_test_subagent("child-1", "sa-1"));
        agent.active_subagent = Some("child-1".into());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::KillSubagent { subagent_id, .. }] if subagent_id == "sa-1"
        ),
        "stop in a subagent view must kill the focused subagent, got {effects:?}"
    );
    assert!(app.agents[&id].subagent_sessions["child-1"].pending_kill);
}

/// The kill routing keys off the focused running subagent, not root idleness:
/// with the root turn running, cancel still kills the child and leaves the root
/// turn running (never cancelling).
#[test]
fn cancel_turn_in_subagent_view_kills_child_even_with_running_root() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent
            .subagent_sessions
            .insert("child-1".to_string(), make_test_subagent("child-1", "sa-1"));
        agent.active_subagent = Some("child-1".into());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::KillSubagent { subagent_id, .. }] if subagent_id == "sa-1"
        ),
        "a running focused subagent must be killed even while the root turn runs, got {effects:?}"
    );
    assert!(
        app.agents[&id].session.state.is_turn_running(),
        "the root turn must keep running"
    );
    assert!(!app.agents[&id].session.state.is_cancelling());
}

/// A finished focused subagent must NOT swallow the cancel into a kill: the
/// stop falls through to normal root-turn cancellation.
#[test]
fn cancel_turn_in_finished_subagent_view_falls_through_to_root() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        let mut info = make_test_subagent("child-1", "sa-1");
        info.finished = true;
        agent.subagent_sessions.insert("child-1".to_string(), info);
        agent.active_subagent = Some("child-1".into());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        matches!(effects.as_slice(), [Effect::CancelTurn { .. }]),
        "a finished subagent must not intercept cancel, got {effects:?}"
    );
}

#[test]
fn cancel_turn_forwards_trigger_hint_to_effect() {
    // The key/mouse producer sets `cancel_trigger_hint` (here ESC) before
    // dispatching CancelTurn; `do_cancel_turn` must forward it onto
    // `Effect::CancelTurn.trigger` (→ `_meta.cancelTrigger`) and consume it.
    // This is the same plumbing the Ctrl+C end-to-end test exercises; only
    // the `CancelTrigger` value differs across producers (esc/ctrl_c/mouse).
    use crate::app::actions::CancelTrigger;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.cancel_trigger_hint = Some(CancelTrigger::Esc);
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            trigger: Some(CancelTrigger::Esc),
            ..
        }
    ));
    // One-shot: consumed when the cancel is built.
    assert_eq!(app.agents[&id].cancel_trigger_hint, None);
}

#[test]
fn cancel_turn_without_trigger_hint_sends_none() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(matches!(
        &effects[0],
        Effect::CancelTurn { trigger: None, .. }
    ));
}

#[test]
fn lost_cancel_is_resent_while_still_cancelling() {
    use crate::app::actions::CancelTrigger;
    use crate::app::dispatch::CANCEL_RESEND_GRACE;
    use crate::app::dispatch::reconcile_overdue_cancels;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.cancel_trigger_hint = Some(CancelTrigger::Mouse);
    }
    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(matches!(effects.as_slice(), [Effect::CancelTurn { .. }]));
    assert!(app.agents[&id].session.state.is_cancelling());

    // Inside the grace: nothing fires.
    assert!(reconcile_overdue_cancels(&mut app).is_none());

    // The cancel is lost in transit (no response ever arrives); age it out.
    app.agents
        .get_mut(&id)
        .unwrap()
        .pending_cancel_resend
        .as_mut()
        .unwrap()
        .sent_at = std::time::Instant::now() - CANCEL_RESEND_GRACE;
    let resent = reconcile_overdue_cancels(&mut app).expect("overdue cancel must re-send");
    assert!(
        matches!(
            resent.as_slice(),
            [Effect::CancelTurn {
                trigger: Some(CancelTrigger::Mouse),
                rewind_if_no_output: false,
                ..
            }]
        ),
        "the resend replays the gesture trigger, got {resent:?}"
    );
    assert_eq!(
        app.agents[&id]
            .pending_cancel_resend
            .as_ref()
            .unwrap()
            .attempts,
        2
    );

    // A received `prompt_complete` broadcast proves the cancel landed: the
    // resend stops even though the pane is still cancelling, so it can
    // never race the turn-end reconcile and cancel a promoted queued prompt.
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.pending_cancel_resend.as_mut().unwrap().sent_at =
            std::time::Instant::now() - CANCEL_RESEND_GRACE;
        agent.pending_turn_end_reconcile = Some(crate::app::agent_view::PendingTurnEnd {
            prompt_id: "p1".into(),
            stop_reason: Some("cancelled".into()),
            agent_result: None,
            cancel_trigger: None,
            received_at: std::time::Instant::now(),
        });
    }
    assert!(reconcile_overdue_cancels(&mut app).is_none());
    // The record survives, confirmed: the auto-resend is dead, but a manual
    // retry can still read the recorded subagent choice.
    assert!(
        app.agents[&id]
            .pending_cancel_resend
            .as_ref()
            .unwrap()
            .confirmed
    );
    app.agents.get_mut(&id).unwrap().pending_turn_end_reconcile = None;
    assert!(
        reconcile_overdue_cancels(&mut app).is_none(),
        "a confirmed record keeps the auto-resend off after the window closes"
    );

    // Turn resolved: the marker clears and nothing more fires.
    app.agents.get_mut(&id).unwrap().session.state = AgentState::Idle;
    assert!(reconcile_overdue_cancels(&mut app).is_none());
    assert!(app.agents[&id].pending_cancel_resend.is_none());
}

#[test]
fn cancel_retry_reuses_recorded_subagent_choice() {
    use crate::app::actions::CancelTrigger;
    use crate::app::dispatch::reconcile_overdue_cancels;
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.cancel_trigger_hint = Some(CancelTrigger::CtrlC);
    }

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::ContinueToRun),
        &mut app,
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::CancelTurn {
            cancel_subagents: false,
            ..
        }]
    ));

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                cancel_subagents: false,
                ..
            }]
        ),
        "the retry must not escalate past the one-shot choice, got {effects:?}"
    );

    // The turn-end broadcast stands the auto-resend down; a retry after it
    // must still reuse the recorded choice instead of escalating.
    app.agents.get_mut(&id).unwrap().pending_turn_end_reconcile =
        Some(crate::app::agent_view::PendingTurnEnd {
            prompt_id: "p1".into(),
            stop_reason: Some("cancelled".into()),
            agent_result: None,
            cancel_trigger: None,
            received_at: std::time::Instant::now(),
        });
    assert!(reconcile_overdue_cancels(&mut app).is_none());
    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                cancel_subagents: false,
                ..
            }]
        ),
        "a confirmed cancel must not discard the recorded choice, got {effects:?}"
    );
}

#[test]
fn confirmed_stop_retry_does_not_rearm_auto_resend() {
    use crate::app::actions::CancelTrigger;
    use crate::app::dispatch::CANCEL_RESEND_GRACE;
    use crate::app::dispatch::reconcile_overdue_cancels;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.cancel_trigger_hint = Some(CancelTrigger::Mouse);
    }
    assert!(matches!(
        dispatch(Action::CancelTurn, &mut app).as_slice(),
        [Effect::CancelTurn {
            trigger: Some(CancelTrigger::Mouse),
            ..
        }]
    ));

    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.pending_turn_end_reconcile = Some(crate::app::agent_view::PendingTurnEnd {
            prompt_id: "p1".into(),
            stop_reason: Some("cancelled".into()),
            agent_result: None,
            cancel_trigger: None,
            received_at: std::time::Instant::now(),
        });
    }
    assert!(reconcile_overdue_cancels(&mut app).is_none());
    assert!(
        app.agents[&id]
            .pending_cancel_resend
            .as_ref()
            .is_some_and(|p| p.confirmed)
    );

    // Gesture retry (hint set, as `[stop]` / Esc do).
    app.agents.get_mut(&id).unwrap().cancel_trigger_hint = Some(CancelTrigger::Mouse);
    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                trigger: Some(CancelTrigger::Mouse),
                ..
            }]
        ),
        "a manual retry still re-sends, got {effects:?}"
    );
    let pending = app.agents[&id]
        .pending_cancel_resend
        .as_ref()
        .expect("resend record must survive");
    assert!(
        pending.confirmed,
        "a confirmed record must stay confirmed across a gesture retry"
    );

    app.agents
        .get_mut(&id)
        .unwrap()
        .pending_cancel_resend
        .as_mut()
        .unwrap()
        .sent_at = std::time::Instant::now() - CANCEL_RESEND_GRACE;
    assert!(
        reconcile_overdue_cancels(&mut app).is_none(),
        "auto-resend must stay off after a confirmed gesture retry"
    );
}

#[test]
fn hintless_retry_replays_recorded_trigger() {
    use crate::app::actions::CancelTrigger;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.cancel_trigger_hint = Some(CancelTrigger::Esc);
    }
    assert!(matches!(
        dispatch(Action::CancelTurn, &mut app).as_slice(),
        [Effect::CancelTurn {
            trigger: Some(CancelTrigger::Esc),
            ..
        }]
    ));

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                trigger: Some(CancelTrigger::Esc),
                ..
            }]
        ),
        "a hint-less retry must replay the recorded trigger, got {effects:?}"
    );
}

#[test]
fn cancel_turn_stops_compact_even_with_stale_wake_marker() {
    use crate::app::actions::CancelTrigger;
    use crate::app::agent::AgentCommand;
    use crate::app::agent_view::RunningWakeTurn;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.start_command(AgentCommand::Compact);
        agent.running_wake_turn = Some(RunningWakeTurn {
            prompt_id: "task-completed-bg1".into(),
            cancel_sent: false,
        });
        agent.cancel_trigger_hint = Some(CancelTrigger::Esc);
    }

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(effects.as_slice(), [Effect::CancelTurn { .. }]),
        "compact cancel must emit, got {effects:?}"
    );
    let agent = &app.agents[&id];
    assert!(
        matches!(
            agent.session.state,
            AgentState::CommandCancelling {
                command: AgentCommand::Compact,
            }
        ),
        "Esc during /compact must cancel compact, not only the stale wake, got {:?}",
        agent.session.state
    );
}

#[test]
fn cancel_after_local_send_during_wake_does_not_arm_resend() {
    use crate::app::actions::CancelTrigger;
    use crate::app::agent_view::RunningWakeTurn;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.running_wake_turn = Some(RunningWakeTurn {
            prompt_id: "task-completed-bg1".into(),
            cancel_sent: false,
        });
        agent.start_turn_boundary(Some("user-1"));
        agent.session.current_prompt_id = Some("user-1".into());
        agent.cancel_trigger_hint = Some(CancelTrigger::Esc);
    }

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                trigger: Some(CancelTrigger::Esc),
                rewind_if_no_output: false,
                ..
            }]
        ),
        "must still cancel the shell-front wake, got {effects:?}"
    );
    let agent = &app.agents[&id];
    assert!(
        agent.session.state.is_turn_running(),
        "the local user turn is queued on the shell, not cancelled"
    );
    assert!(
        agent.pending_cancel_resend.is_none(),
        "auto-resend would cancel the promoted user turn"
    );
    assert!(
        agent
            .running_wake_turn
            .as_ref()
            .is_some_and(|w| w.cancel_sent),
        "the wake marker must record the cancel"
    );
}

#[test]
fn stale_cancel_resend_clears_once_pane_is_idle() {
    use crate::app::actions::CancelTrigger;
    use crate::app::dispatch::reconcile_overdue_cancels;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::Idle;
        agent.pending_cancel_resend = Some(crate::app::agent_view::PendingCancelResend {
            prompt_id: None,
            sent_at: std::time::Instant::now(),
            attempts: 3,
            confirmed: true,
            cancel_subagents: false,
            trigger: CancelTrigger::Esc,
        });
    }
    assert!(reconcile_overdue_cancels(&mut app).is_none());
    assert!(
        app.agents[&id].pending_cancel_resend.is_none(),
        "reconcile must drop a stale record once nothing is cancelling"
    );
}

#[test]
fn do_cancel_turn_cancels_running_wake_turn() {
    use crate::app::agent_view::RunningWakeTurn;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.running_wake_turn = Some(RunningWakeTurn {
            prompt_id: "task-completed-bg1".into(),
            cancel_sent: false,
        });
    }

    let effects = super::super::turn::do_cancel_turn(&mut app, true);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                cancel_subagents: true,
                rewind_if_no_output: false,
                trigger: None,
                ..
            }]
        ),
        "programmatic cancel must stop a wake turn, got {effects:?}"
    );
    let agent = &app.agents[&id];
    assert!(agent.session.state.is_idle());
    assert!(agent.wake_turn_cancelling());
}

#[test]
fn stop_click_cancels_running_wake_turn() {
    use crate::app::actions::CancelTrigger;
    use crate::app::agent_view::RunningWakeTurn;
    use crate::app::dispatch::CANCEL_RESEND_GRACE;
    use crate::app::dispatch::reconcile_overdue_cancels;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.running_wake_turn = Some(RunningWakeTurn {
            prompt_id: "task-completed-bg1".into(),
            cancel_sent: false,
        });
        assert!(
            matches!(agent.wake_display_state(), Some(AgentState::TurnRunning)),
            "a streaming wake turn must offer the running chrome (and [stop])"
        );
        // The mouse handler sets the hint before dispatching CancelTurn.
        agent.cancel_trigger_hint = Some(CancelTrigger::Mouse);
    }

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                trigger: Some(CancelTrigger::Mouse),
                rewind_if_no_output: false,
                ..
            }]
        ),
        "the wake cancel must ride the normal cancel wire, got {effects:?}"
    );
    let agent = &app.agents[&id];
    assert!(
        agent.session.state.is_idle(),
        "a wake cancel must not fabricate a local turn"
    );
    assert!(matches!(
        agent.wake_display_state(),
        Some(AgentState::TurnCancelling)
    ));

    // The fire-and-forget cancel is loss-prone: the resend reconcile must
    // stay armed even though the pane never left Idle.
    app.agents
        .get_mut(&id)
        .unwrap()
        .pending_cancel_resend
        .as_mut()
        .unwrap()
        .sent_at = std::time::Instant::now() - CANCEL_RESEND_GRACE;
    assert!(reconcile_overdue_cancels(&mut app).is_some());
}

#[test]
fn cancel_turn_leaves_shared_queue_for_agent_to_drain() {
    use crate::app::prompt_queue::QueueEntryWire;
    // Prompts typed while a turn runs live on the server-authoritative
    // shared queue (broadcast to all attached clients). The agent owns the
    // drain: on cancel the FRONT queued prompt runs next (promoted
    // server-side), so the pager must NOT pull it back into the input or
    // mutate the queue locally — the `x.ai/queue/changed` rebroadcast is the
    // source of truth.
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.shared_queue = vec![
            QueueEntryWire {
                id: "q1".into(),
                version: 3,
                owner: None,
                last_editor: None,
                kind: "prompt".into(),
                text: "first queued".into(),
                position: 0,
                combined_texts: None,
            },
            QueueEntryWire {
                id: "q2".into(),
                version: 4,
                owner: None,
                last_editor: None,
                kind: "prompt".into(),
                text: "second queued".into(),
                position: 1,
                combined_texts: None,
            },
        ];
        assert!(agent.prompt.text().is_empty());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    // The input box is left untouched — the front queued prompt is NOT
    // pulled back into it (it runs next on the agent instead).
    assert!(
        app.agents[&id].prompt.text().is_empty(),
        "cancel must not restore a queued prompt into the input"
    );
    // The local mirror is left intact; the agent's rebroadcast drives the
    // queue, so the pager must not predict the post-cancel order.
    let q = &app.agents[&id].shared_queue;
    assert_eq!(
        q.len(),
        2,
        "cancel must not mutate the shared queue locally"
    );
    assert_eq!(q[0].id, "q1");
    assert_eq!(q[1].id, "q2");
    // A plain CancelTurn is emitted (no queued-prompt id threaded, no
    // separate QueueRemove) — the agent tears down the running turn and
    // promotes q1 as the next turn.
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "must emit CancelTurn, got {effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::QueueRemove { .. })),
        "must NOT emit a separate QueueRemove on cancel"
    );
}

#[test]
fn cancel_turn_with_running_subagents_shows_panel() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;
    app.agents
        .get_mut(&id)
        .unwrap()
        .subagent_sessions
        .insert("child-1".into(), make_test_subagent("child-1", "sa-1"));

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(effects.is_empty());
    assert!(app.agents[&id].cancel_turn_view.is_some());
    assert_eq!(
        app.agents[&id]
            .cancel_turn_view
            .as_ref()
            .unwrap()
            .running_count,
        1
    );
    assert!(app.agents[&id].session.state.is_turn_running());
}

#[test]
fn cancel_turn_choice_stop_running_sends_cancel_true() {
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::StopRunning),
        &mut app,
    );

    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            cancel_subagents: true,
            ..
        }
    ));
    assert!(app.agents[&id].session.state.is_cancelling());
}

#[test]
fn cancel_turn_choice_continue_to_run_sends_cancel_false() {
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::ContinueToRun),
        &mut app,
    );

    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            cancel_subagents: false,
            ..
        }
    ));
    assert!(app.agents[&id].session.state.is_cancelling());
}

#[test]
fn cancel_turn_choice_after_turn_finished_is_noop() {
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    assert!(app.agents[&id].session.state.is_idle());

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::StopRunning),
        &mut app,
    );

    assert!(effects.is_empty());
    assert!(app.agents[&id].session.state.is_idle());
}

/// Work B: idle primary + live subagents still opens the cancel panel (ask pref).
#[test]
fn cancel_turn_idle_with_running_subagents_shows_panel() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    assert!(app.agents[&id].session.state.is_idle());
    app.agents
        .get_mut(&id)
        .unwrap()
        .subagent_sessions
        .insert("child-1".into(), make_test_subagent("child-1", "sa-1"));

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        effects.is_empty(),
        "idle stop path opens panel first, got {effects:?}"
    );
    assert!(app.agents[&id].cancel_turn_view.is_some());
    assert_eq!(
        app.agents[&id]
            .cancel_turn_view
            .as_ref()
            .unwrap()
            .running_count,
        1
    );
    assert!(
        app.agents[&id].session.state.is_idle(),
        "must not invent a parent cancel"
    );
}

/// Work B: idle + subagents + StopRunning choice kills children, no CancelTurn.
#[test]
fn cancel_turn_choice_idle_with_subagents_kills_without_parent_cancel() {
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents
        .get_mut(&id)
        .unwrap()
        .subagent_sessions
        .insert("child-1".into(), make_test_subagent("child-1", "sa-1"));

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::StopRunning),
        &mut app,
    );

    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::KillSubagent {
                subagent_id,
                ..
            } if subagent_id == "sa-1"
        )),
        "must kill live subagent, got {effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "idle primary must not emit parent CancelTurn, got {effects:?}"
    );
    assert!(app.agents[&id].session.state.is_idle());
}

/// Work B: idle + no subagents CancelTurn remains a no-op.
#[test]
fn cancel_turn_idle_without_subagents_is_noop() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    assert!(app.agents[&id].session.state.is_idle());
    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(effects.is_empty());
    assert!(app.agents[&id].cancel_turn_view.is_none());
}

#[test]
fn cancel_turn_choice_after_subagents_finished_still_cancels() {
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let mut info = make_test_subagent("child-1", "sa-1");
    info.finished = true;
    app.agents
        .get_mut(&id)
        .unwrap()
        .subagent_sessions
        .insert("child-1".into(), info);

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::StopRunning),
        &mut app,
    );

    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            cancel_subagents: true,
            ..
        }
    ));
    assert!(app.agents[&id].session.state.is_cancelling());
}

#[test]
fn cancel_turn_double_dispatch_falls_through_when_panel_open() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;
    app.agents
        .get_mut(&id)
        .unwrap()
        .subagent_sessions
        .insert("child-1".into(), make_test_subagent("child-1", "sa-1"));

    // First CancelTurn shows the panel.
    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(effects.is_empty());
    assert!(app.agents[&id].cancel_turn_view.is_some());

    // Second CancelTurn falls through (panel already open) and cancels.
    let effects = dispatch(Action::CancelTurn, &mut app);
    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            cancel_subagents: true,
            ..
        }
    ));
    assert!(app.agents[&id].session.state.is_cancelling());
}

#[test]
fn cancel_turn_when_idle_does_nothing() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    assert!(app.agents[&id].session.state.is_idle());

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(effects.is_empty());
    assert!(app.agents[&id].session.state.is_idle());
}

/// An Idle parent with a TurnRunning overlay child must cancel the child session.
#[test]
fn cancel_turn_in_subagent_overlay_cancels_child_while_parent_idle() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-overlay-idle-parent";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let child = AgentView::new(child_session, ScrollbackState::new());
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = Some(child_sid.to_string());
        assert!(parent.session.state.is_idle());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                session_id,
                cancel_subagents: true,
                rewind_if_no_output: false,
                ..
            }] if session_id.0.as_ref() == child_sid
        ),
        "overlay stop must emit CancelTurn for the child session, got {effects:?}"
    );
    let parent = app.agents.get(&parent_id).unwrap();
    assert!(
        parent.session.state.is_idle(),
        "parent stays Idle; overlay stop is not a parent cancel"
    );
    assert!(parent.cancel_turn_view.is_none());
    let child = parent.subagent_views.get(child_sid).unwrap();
    assert!(
        child.session.state.is_cancelling(),
        "child overlay must show Cancelling"
    );
}

/// Overlay stop cancels the running child and must not open the parent ask panel.
#[test]
fn cancel_turn_in_subagent_overlay_does_not_open_parent_ask_panel() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-overlay-running-parent";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let child = AgentView::new(child_session, ScrollbackState::new());
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent.session.state = AgentState::TurnRunning;
        parent
            .subagent_sessions
            .insert(child_sid.into(), make_test_subagent(child_sid, "sa-1"));
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = Some(child_sid.to_string());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                session_id,
                cancel_subagents: true,
                ..
            }] if session_id.0.as_ref() == child_sid
        ),
        "overlay stop must target the child session, got {effects:?}"
    );
    let parent = app.agents.get(&parent_id).unwrap();
    assert!(
        parent.cancel_turn_view.is_none(),
        "ask panel on the parent is unreachable under the overlay"
    );
    assert!(
        parent.session.state.is_turn_running(),
        "parent turn is not the cancel target"
    );
    assert!(
        parent
            .subagent_views
            .get(child_sid)
            .unwrap()
            .session
            .state
            .is_cancelling()
    );
}

/// No overlay: a running subagent still opens the parent ask panel.
#[test]
fn cancel_turn_without_overlay_still_shows_subagent_ask_panel() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-not-focused";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let child = AgentView::new(child_session, ScrollbackState::new());
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent.session.state = AgentState::TurnRunning;
        parent
            .subagent_sessions
            .insert(child_sid.into(), make_test_subagent(child_sid, "sa-1"));
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = None;
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(effects.is_empty());
    let parent = app.agents.get(&parent_id).unwrap();
    assert!(parent.cancel_turn_view.is_some());
    assert!(parent.session.state.is_turn_running());
    assert!(
        parent
            .subagent_views
            .get(child_sid)
            .unwrap()
            .session
            .state
            .is_turn_running(),
        "unfocused child must not be cancelled"
    );
}

/// Second `[stop]` while the overlay child is already Cancelling must re-send.
#[test]
fn cancel_turn_in_subagent_overlay_retries_when_child_already_cancelling() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-overlay-retry";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let child = AgentView::new(child_session, ScrollbackState::new());
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = Some(child_sid.to_string());
    }

    let first = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            first.as_slice(),
            [Effect::CancelTurn { session_id, .. }] if session_id.0.as_ref() == child_sid
        ),
        "first overlay stop must cancel the child, got {first:?}"
    );

    let retry = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(
            retry.as_slice(),
            [Effect::CancelTurn {
                session_id,
                cancel_subagents: true,
                rewind_if_no_output: false,
                ..
            }] if session_id.0.as_ref() == child_sid
        ),
        "second overlay stop must re-send child CancelTurn, got {retry:?}"
    );
    let parent = app.agents.get(&parent_id).unwrap();
    assert!(parent.session.state.is_idle());
    assert!(
        parent
            .subagent_views
            .get(child_sid)
            .unwrap()
            .session
            .state
            .is_cancelling()
    );
}

/// An Idle parent with a cancelling overlay child must keep Fast ticks for resend.
#[test]
fn tick_demand_fast_for_idle_parent_with_cancelling_overlay_child() {
    use crate::app::app_view::TickDemand;

    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-overlay-tick-demand";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let mut child = AgentView::new(child_session, ScrollbackState::new());
    child.cancel_trigger_hint = Some(crate::app::actions::CancelTrigger::Mouse);
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = Some(child_sid.to_string());
        assert!(parent.session.state.is_idle());
    }
    assert_eq!(app.tick_demand(), TickDemand::None, "idle overlay parks");

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(effects.as_slice(), [Effect::CancelTurn { .. }]),
        "overlay stop must cancel the child, got {effects:?}"
    );
    assert_eq!(
        app.tick_demand(),
        TickDemand::Fast,
        "idle parent with a cancelling child must not park before resend grace"
    );
}

/// Overlay stop sends cancel_subagents true even when always_continue is set.
#[test]
fn cancel_turn_in_subagent_overlay_ignores_always_continue_pref() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-overlay-always-continue";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let mut child = AgentView::new(child_session, ScrollbackState::new());
    child.cancel_subagents_preference = Some(false);
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent.cancel_subagents_preference = Some(false);
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = Some(child_sid.to_string());
    }
    app.current_ui.cancel_subagents_on_turn_cancel = Some("always_continue".into());

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                session_id,
                cancel_subagents: true,
                ..
            }] if session_id.0.as_ref() == child_sid
        ),
        "overlay stop must ignore always_continue, got {effects:?}"
    );
}

/// Dangling active_subagent is not an overlay; parent ask-panel still opens.
#[test]
fn cancel_turn_with_stale_active_subagent_still_shows_ask_panel() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent.session.state = AgentState::TurnRunning;
        parent
            .subagent_sessions
            .insert("child-1".into(), make_test_subagent("child-1", "sa-1"));
        parent.active_subagent = Some("stale-sid".into());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(effects.is_empty());
    let parent = app.agents.get(&parent_id).unwrap();
    assert!(parent.cancel_turn_view.is_some());
    assert!(parent.session.state.is_turn_running());
}

/// Overlay child with no session_id: no wire cancel and no local Cancelling.
#[test]
fn cancel_turn_in_subagent_overlay_without_session_id_is_noop() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-overlay-no-sid";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    child_session.session_id = None;
    let child = AgentView::new(child_session, ScrollbackState::new());
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = Some(child_sid.to_string());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(effects.is_empty());
    let parent = app.agents.get(&parent_id).unwrap();
    assert!(
        parent
            .subagent_views
            .get(child_sid)
            .unwrap()
            .session
            .state
            .is_turn_running(),
        "must not flip to Cancelling when there is no session to cancel"
    );
}

#[test]
fn reconcile_overdue_cancels_resends_for_overlay_child() {
    use crate::app::actions::CancelTrigger;
    use crate::app::dispatch::CANCEL_RESEND_GRACE;
    use crate::app::dispatch::reconcile_overdue_cancels;

    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-overlay-resend";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let mut child = AgentView::new(child_session, ScrollbackState::new());
    child.cancel_trigger_hint = Some(CancelTrigger::Mouse);
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = Some(child_sid.to_string());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(matches!(effects.as_slice(), [Effect::CancelTurn { .. }]));
    assert!(reconcile_overdue_cancels(&mut app).is_none());

    app.agents
        .get_mut(&parent_id)
        .unwrap()
        .subagent_views
        .get_mut(child_sid)
        .unwrap()
        .pending_cancel_resend
        .as_mut()
        .unwrap()
        .sent_at = std::time::Instant::now() - CANCEL_RESEND_GRACE;

    let resent = reconcile_overdue_cancels(&mut app).expect("child overdue cancel must re-send");
    assert!(
        matches!(
            resent.as_slice(),
            [Effect::CancelTurn {
                session_id,
                trigger: Some(CancelTrigger::Mouse),
                rewind_if_no_output: false,
                ..
            }] if session_id.0.as_ref() == child_sid
        ),
        "auto-resend must target the child session, got {resent:?}"
    );
}

/// Idle parent with no overlay must not cancel a background running child view.
#[test]
fn cancel_turn_without_overlay_while_idle_is_noop_even_with_running_child() {
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-background-running";
    let mut child_session = make_test_agent_session(&app, AgentId(1), child_sid);
    child_session.state = AgentState::TurnRunning;
    let child = AgentView::new(child_session, ScrollbackState::new());
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent
            .subagent_views
            .insert(child_sid.to_string(), Box::new(child));
        parent.active_subagent = None;
        assert!(parent.session.state.is_idle());
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(effects.is_empty());
    let parent = app.agents.get(&parent_id).unwrap();
    assert!(parent.session.state.is_idle());
    assert!(
        parent
            .subagent_views
            .get(child_sid)
            .unwrap()
            .session
            .state
            .is_turn_running()
    );
}

#[test]
fn cancel_turn_when_already_cancelling_resends_cancel() {
    // A cancel that was sent but never resolved (lost notification or
    // lost turn-end response) used to make every further
    // Esc a silent no-op, permanently stranding the pane on
    // "Cancelling…". Cancelling again must RE-SEND the (idempotent)
    // cancel instead.
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnCancelling;

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelTurn {
                cancel_subagents: true,
                ..
            }]
        ),
        "cancel while cancelling must re-send the cancel, got {effects:?}"
    );
    assert!(app.agents[&id].session.state.is_cancelling());
}

#[test]
fn cancel_turn_retry_honors_subagent_preference() {
    // The retry skips the subagent panel (the choice was already made on
    // the first cancel) but must reuse the remembered preference.
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnCancelling;
        agent.cancel_subagents_preference = Some(false);
    }

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert!(matches!(
        effects.as_slice(),
        [Effect::CancelTurn {
            cancel_subagents: false,
            ..
        }]
    ));
}

/// The latched-cancel deadlock: cancel sent → state
/// `TurnCancelling` → the turn's PromptResponse RPC is lost → nothing can
/// ever exit the state. The armed broadcast marker must finish the turn
/// after the grace window.
#[test]
fn reconcile_finishes_cancelling_turn_after_grace() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnCancelling;
        agent.session.current_prompt_id = Some("pid-stuck".into());
    }
    arm_reconcile(
        &mut app,
        id,
        "pid-stuck",
        "cancelled",
        TURN_END_RECONCILE_GRACE + std::time::Duration::from_secs(1),
    );

    let fired = reconcile_overdue_turn_ends(&mut app);

    assert!(fired.is_some(), "an overdue marker must fire the reconcile");
    let agent = &app.agents[&id];
    assert!(
        agent.session.state.is_idle(),
        "reconcile must exit TurnCancelling"
    );
    assert!(agent.session.current_prompt_id.is_none());
    assert!(agent.pending_turn_end_reconcile.is_none());
    let has_cancelled_marker = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(ev.event, SessionEvent::TurnCancelled { .. })
        )
    });
    assert!(
        has_cancelled_marker,
        "reconcile must surface the 'Turn cancelled' marker"
    );
}

/// A lost-RPC reconcile for a send-now cancel (`_meta.cancelTrigger: "send_now"`) pushes no marker.
#[test]
fn reconcile_suppresses_send_now_cancel_marker() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("pid-stuck".into());
    }
    arm_reconcile_with_trigger(
        &mut app,
        id,
        "pid-stuck",
        "cancelled",
        Some("send_now"),
        TURN_END_RECONCILE_GRACE + std::time::Duration::from_secs(1),
    );

    let fired = reconcile_overdue_turn_ends(&mut app);

    assert!(fired.is_some(), "the overdue reconcile must still fire");
    let agent = &app.agents[&id];
    assert!(agent.session.state.is_idle(), "the turn still finishes");
    let has_marker = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(
                    ev.event,
                    SessionEvent::TurnCancelled { .. } | SessionEvent::TurnCompleted { .. }
                )
        )
    });
    assert!(
        !has_marker,
        "a send-now cancel reconcile must not push a cancelled (or substitute \
         completed) marker"
    );
}

/// Older-shell fallback on the reconcile rail: no wire trigger, armed expectation.
#[test]
fn reconcile_suppresses_expected_send_now_cancel_without_wire_trigger() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("pid-stuck".into());
        agent.expect_send_now_cancel = Some("p-next".into());
    }
    arm_reconcile(
        &mut app,
        id,
        "pid-stuck",
        "cancelled",
        TURN_END_RECONCILE_GRACE + std::time::Duration::from_secs(1),
    );

    let fired = reconcile_overdue_turn_ends(&mut app);

    assert!(fired.is_some());
    let agent = &app.agents[&id];
    let has_cancelled = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(ev.event, SessionEvent::TurnCancelled { .. })
        )
    });
    assert!(!has_cancelled, "expected send-now cancel renders no marker");
    assert!(
        agent.expect_send_now_cancel.is_none(),
        "the expectation is consumed by the reconcile"
    );
}

#[test]
fn reconcile_waits_for_grace_window() {
    // A freshly-armed marker means the RPC response may still be in
    // flight (healthy path: it lands milliseconds after the broadcast) —
    // do not touch the turn yet.
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnCancelling;
        agent.session.current_prompt_id = Some("pid-stuck".into());
    }
    arm_reconcile(
        &mut app,
        id,
        "pid-stuck",
        "cancelled",
        std::time::Duration::ZERO,
    );

    let fired = reconcile_overdue_turn_ends(&mut app);

    assert!(fired.is_none());
    let agent = &app.agents[&id];
    assert!(agent.session.state.is_cancelling());
    assert!(
        agent.pending_turn_end_reconcile.is_some(),
        "marker must stay armed until grace expires"
    );
}

#[test]
fn reconcile_drops_stale_marker_when_turn_already_resolved() {
    // The normal path won the race (PromptResponse finished the turn, or
    // a new turn was adopted): the marker is stale and must be dropped
    // without touching state or pushing a marker.
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let scrollback_before = app.agents[&id].scrollback.len();
    arm_reconcile(
        &mut app,
        id,
        "pid-old",
        "end_turn",
        TURN_END_RECONCILE_GRACE + std::time::Duration::from_secs(1),
    );

    let fired = reconcile_overdue_turn_ends(&mut app);

    assert!(fired.is_none(), "stale marker must not fire");
    let agent = &app.agents[&id];
    assert!(agent.session.state.is_idle());
    assert!(agent.pending_turn_end_reconcile.is_none());
    assert_eq!(agent.scrollback.len(), scrollback_before);
}

#[test]
fn reconcile_applies_stashed_running_adoption() {
    // The failing sequence: queued prompt promoted server-side while the
    // cancelled turn's response was lost. The reconcile must hand the pane
    // to the promoted prompt (turn-start shim), not strand it Idle.
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnCancelling;
        agent.session.current_prompt_id = Some("pid-stuck".into());
    }
    // The leader's running_prompt_id broadcast arrived mid-teardown and
    // was stashed (same as the PromptResponse path).
    app.pending_running_adoptions.insert(
        id,
        crate::app::acp_handler::PendingRunningAdoption {
            prompt_id: "pid-next".into(),
            text: Some("queued prompt".into()),
            combined_texts: None,
            kind: "prompt".into(),
            turn_ended: false,
        },
    );
    arm_reconcile(
        &mut app,
        id,
        "pid-stuck",
        "cancelled",
        TURN_END_RECONCILE_GRACE + std::time::Duration::from_secs(1),
    );

    let fired = reconcile_overdue_turn_ends(&mut app);

    assert!(fired.is_some());
    let agent = &app.agents[&id];
    assert_eq!(
        agent.session.current_prompt_id.as_deref(),
        Some("pid-next"),
        "the stashed adoption must be applied after the reconcile"
    );
    assert!(
        matches!(agent.session.state, AgentState::TurnRunning),
        "the promoted prompt is the new running turn"
    );
    assert!(!app.pending_running_adoptions.contains_key(&id));
}

/// The reconcile rail's `stop_reason == "error"` arm: formats the raw
/// agent_result and skips the marker when a dedicated banner already
/// explains the failure.
#[test]
fn reconcile_error_formats_marker_and_defers_to_banner() {
    fn run(with_banner: bool) -> Option<String> {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.state = AgentState::TurnRunning;
            agent.session.current_prompt_id = Some("pid-stuck".into());
            if with_banner {
                agent.scrollback.push_block(RenderBlock::session_event(
                    SessionEvent::RequestFailed {
                        status: Some(500),
                        headline: "Server error (500)".into(),
                        detail: String::new(),
                    },
                ));
            }
            agent.pending_turn_end_reconcile = Some(crate::app::agent_view::PendingTurnEnd {
                prompt_id: "pid-stuck".into(),
                stop_reason: Some("error".into()),
                agent_result: Some("boom".into()),
                cancel_trigger: None,
                received_at: std::time::Instant::now()
                    - (TURN_END_RECONCILE_GRACE + std::time::Duration::from_secs(1)),
            });
        }
        let fired = reconcile_overdue_turn_ends(&mut app);
        assert!(
            fired.is_some(),
            "the overdue reconcile must finish the turn"
        );
        let agent = &app.agents[&id];
        (0..agent.scrollback.len()).find_map(|i| {
            match agent.scrollback.entry(i).map(|e| &e.block) {
                Some(RenderBlock::SessionEvent(ev)) => match &ev.event {
                    SessionEvent::TurnFailed { error, .. } => Some(error.clone()),
                    _ => None,
                },
                _ => None,
            }
        })
    }

    assert_eq!(
        run(false).as_deref(),
        Some("Request failed \u{2014} boom. Try sending again."),
        "the raw agent_result must render as a formatted marker"
    );
    assert_eq!(
        run(true),
        None,
        "a dedicated banner must suppress the reconcile's TurnFailed marker"
    );
}

#[test]
fn always_stop_preference_skips_panel() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;
    app.agents.get_mut(&id).unwrap().cancel_subagents_preference = Some(true);
    app.agents
        .get_mut(&id)
        .unwrap()
        .subagent_sessions
        .insert("child-1".into(), make_test_subagent("child-1", "sa-1"));

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            cancel_subagents: true,
            ..
        }
    ));
    assert!(app.agents[&id].cancel_turn_view.is_none());
}

#[test]
fn always_continue_preference_skips_panel() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;
    app.agents.get_mut(&id).unwrap().cancel_subagents_preference = Some(false);
    app.agents
        .get_mut(&id)
        .unwrap()
        .subagent_sessions
        .insert("child-1".into(), make_test_subagent("child-1", "sa-1"));

    let effects = dispatch(Action::CancelTurn, &mut app);

    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::CancelTurn {
            cancel_subagents: false,
            ..
        }
    ));
    assert!(app.agents[&id].cancel_turn_view.is_none());
}

#[test]
fn always_stop_choice_sets_preference() {
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::AlwaysStop),
        &mut app,
    );

    assert_eq!(app.agents[&id].cancel_subagents_preference, Some(true));
    assert_eq!(
        app.current_ui.cancel_subagents_on_turn_cancel.as_deref(),
        Some("always_stop")
    );
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::PersistSetting {
            key: "cancel_subagents_on_turn_cancel",
            value: crate::settings::SettingValue::Enum("always_stop"),
            ..
        }
    )));
}

#[test]
fn always_continue_choice_sets_preference() {
    use crate::views::modal::CancelTurnChoice;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(
        Action::CancelTurnChoice(CancelTurnChoice::AlwaysContinue),
        &mut app,
    );

    assert_eq!(app.agents[&id].cancel_subagents_preference, Some(false));
    assert_eq!(
        app.current_ui.cancel_subagents_on_turn_cancel.as_deref(),
        Some("always_continue")
    );
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::PersistSetting {
            key: "cancel_subagents_on_turn_cancel",
            value: crate::settings::SettingValue::Enum("always_continue"),
            ..
        }
    )));
}

#[test]
fn prompt_response_clears_cancel_turn_panel() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;
    app.agents.get_mut(&id).unwrap().turn_started_at = Some(std::time::Instant::now());
    app.agents.get_mut(&id).unwrap().cancel_turn_view =
        Some(crate::views::modal::CancelTurnViewState {
            active_idx: 0,
            running_count: 2,
        });

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Ok(acp::PromptResponse::new(acp::StopReason::EndTurn)),
            http_status: None,
            prompt_id: None,
        }),
        &mut app,
    );

    assert!(app.agents[&id].cancel_turn_view.is_none());
    assert!(app.agents[&id].session.state.is_idle());
}

#[test]
fn cancel_after_first_activity_does_not_restore() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);

    dispatch(Action::SendPrompt("keep me".into()), &mut app);
    assert!(app.agents[&id].session.in_flight_prompt.is_some());
    // Simulate that the server emitted activity (the acp_handler
    // clear-on-first-activity hook would have cleared this).
    app.agents.get_mut(&id).unwrap().session.in_flight_prompt = None;

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::CancelTurn { .. }));

    // Prompt was NOT restored; user-prompt block stays; state is
    // the normal TurnCancelling (not the rewind-Idle).
    assert!(app.agents[&id].prompt.text().is_empty());
    assert_eq!(app.agents[&id].scrollback.len(), 1);
    assert!(app.agents[&id].session.state.is_cancelling());

    // PromptResponse arrives — TurnCancelled banner is pushed.
    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Ok(acp::PromptResponse::new(acp::StopReason::Cancelled)),
            http_status: None,
            prompt_id: None,
        }),
        &mut app,
    );
    // user_prompt + TurnCancelled banner.
    assert_eq!(app.agents[&id].scrollback.len(), 2);
}

/// Ctrl+C rewind of a locally-drained combined turn must remove *every*
/// per-segment user bubble (not just the last) and restore the joined text.
#[test]
fn cancel_rewind_removes_all_combined_segment_blocks() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let (first_id, last_id) = {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("p-combo".into());
        // One bubble per original follow-up, as a combined drain paints them.
        let first_id = agent
            .scrollback
            .push_block(RenderBlock::user_prompt("first"));
        let last_id = agent
            .scrollback
            .push_block(RenderBlock::user_prompt("second"));
        agent.session.in_flight_prompt = Some(crate::app::agent::InFlightPrompt {
            text: "first\n\nsecond".into(),
            images: Vec::new(),
            scrollback_entry: last_id,
            combined_scrollback_entries: vec![first_id],
            chip_elements: Vec::new(),
        });
        (first_id, last_id)
    };

    let _ = dispatch(Action::CancelTurn, &mut app);

    let agent = &app.agents[&id];
    assert!(
        agent.scrollback.index_of_id(first_id).is_none(),
        "the earlier segment bubble must also be removed on rewind"
    );
    assert!(
        agent.scrollback.index_of_id(last_id).is_none(),
        "the primary segment bubble must be removed on rewind"
    );
    assert_eq!(
        agent.prompt.text(),
        "first\n\nsecond",
        "the joined combined text is restored into the composer"
    );
}

/// A cancel landing before first server activity must NOT rewind the stashed
/// in-flight prompt over a NEWER composer draft. Esc (and the mouse stop /
/// palette cancel) fire with the draft intact — unlike keyboard Ctrl+C,
/// which only cancels on an empty prompt — so the no-output rewind falls back
/// to the standard cancel and the draft survives.
#[test]
fn cancel_with_newer_draft_skips_no_output_rewind_and_keeps_draft() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sent_id = {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("p-sent".into());
        let sent_id = agent
            .scrollback
            .push_block(RenderBlock::user_prompt("sent prompt"));
        agent.session.in_flight_prompt = Some(crate::app::agent::InFlightPrompt {
            text: "sent prompt".into(),
            images: Vec::new(),
            scrollback_entry: sent_id,
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        // Typed WHILE the turn was starting — newer than the stash.
        agent.prompt.set_text("newer draft");
        sent_id
    };

    let effects = dispatch(Action::CancelTurn, &mut app);
    assert!(
        matches!(effects.as_slice(), [Effect::CancelTurn { .. }]),
        "cancel still flies to the server, got {effects:?}"
    );

    let agent = &app.agents[&id];
    assert_eq!(
        agent.prompt.text(),
        "newer draft",
        "the composer draft must survive the cancel (no rewind clobber)"
    );
    assert!(
        agent.scrollback.index_of_id(sent_id).is_some(),
        "standard cancel keeps the sent prompt's block (no rewind removal)"
    );
    assert!(
        agent.session.state.is_cancelling(),
        "standard cancel path (TurnCancelling), not the rewind-Idle"
    );
}

#[test]
fn entry_title_strips_skill_xml_from_generated_title() {
    use crate::views::session_title::entry_title;
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    agent.generated_session_title = Some(
        "<command-name>implement</command-name>\n\
             <command-message>/implement</command-message>\n\
             <command-args>fix the rendering bug</command-args>"
            .into(),
    );
    let title = entry_title(&app.agents[&AgentId(0)]);
    assert_eq!(title, "/implement fix the rendering bug");
}

#[test]
fn entry_title_strips_skill_xml_from_first_prompt() {
    use crate::scrollback::block::RenderBlock;
    use crate::views::session_title::entry_title;
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    agent.scrollback.push_block(RenderBlock::user_prompt(
        "<command-name>deploy</command-name>\n\
             <command-message>/deploy</command-message>",
    ));
    let title = entry_title(&app.agents[&AgentId(0)]);
    assert_eq!(title, "/deploy");
}

#[test]
fn prompt_history_loaded_sanitizes_skill_xml() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);

    let prompts = vec![
        "<command-name>review</command-name>\n\
             <command-message>/review</command-message>\n\
             <command-args>198653</command-args>"
            .into(),
        "plain prompt".into(),
        "<command-name>deploy</command-name>".into(),
    ];

    dispatch(
        Action::TaskComplete(TaskResult::PromptHistoryLoaded {
            agent_id: id,
            prompts,
        }),
        &mut app,
    );

    let history = &app.agents[&id].session.prompt_history;
    assert_eq!(history[0], "/review 198653");
    assert_eq!(history[1], "plain prompt");
    assert_eq!(history[2], "/deploy");
}

#[test]
fn bg_task_killed_already_exited_clears_pending_kill_on_inactive_agent() {
    let mut app = two_agent_app_with_bg_task();
    // Agent 1 has task-B-1 with pending_kill=true, active view is agent 0

    dispatch(
        Action::TaskComplete(TaskResult::BgTaskKilled {
            session_id: "sess-B".into(),
            task_id: "task-B-1".into(),
            outcome: Some(xai_grok_tools::types::KillOutcome::AlreadyExited),
        }),
        &mut app,
    );

    let task = &app.agents[&AgentId(1)].session.bg_tasks["task-B-1"];
    assert!(!task.pending_kill);
    assert!(task.kill_requested_at.is_none());
}

#[test]
fn bg_task_killed_not_found_removes_task_from_inactive_agent() {
    let mut app = two_agent_app_with_bg_task();

    dispatch(
        Action::TaskComplete(TaskResult::BgTaskKilled {
            session_id: "sess-B".into(),
            task_id: "task-B-1".into(),
            outcome: Some(xai_grok_tools::types::KillOutcome::NotFound),
        }),
        &mut app,
    );

    assert!(
        !app.agents[&AgentId(1)]
            .session
            .bg_tasks
            .contains_key("task-B-1")
    );
}

/// Resume regression: a stale row restored by replay keeps a
/// running "Task started" scrollback entry. When the ✗ kill resolves
/// `not_found`, the entry must be finished alongside the row removal so
/// the started block doesn't keep its running accent forever.
#[test]
fn bg_task_killed_not_found_finishes_scrollback_entry() {
    let mut app = two_agent_app_with_bg_task();
    {
        let agent1 = app.agents.get_mut(&AgentId(1)).unwrap();
        let eid = agent1.scrollback.push_block(RenderBlock::BgTask(
            crate::scrollback::blocks::BgTaskBlock::started("sleep 99", "task-B-1"),
        ));
        agent1.scrollback.set_last_running(true);
        agent1
            .session
            .bg_tasks
            .get_mut("task-B-1")
            .unwrap()
            .scrollback_entry_id = Some(eid);
        assert!(agent1.scrollback.needs_animation());
    }

    dispatch(
        Action::TaskComplete(TaskResult::BgTaskKilled {
            session_id: "sess-B".into(),
            task_id: "task-B-1".into(),
            outcome: Some(xai_grok_tools::types::KillOutcome::NotFound),
        }),
        &mut app,
    );

    let agent1 = &app.agents[&AgentId(1)];
    assert!(!agent1.session.bg_tasks.contains_key("task-B-1"));
    assert!(
        !agent1.scrollback.needs_animation(),
        "started entry must be finished when the stale row is removed"
    );
}

/// `outcome: None` (error envelope / unparseable payload) clears the
/// pending state so the user can retry, and keeps the row.
#[test]
fn bg_task_killed_missing_outcome_clears_pending_kill() {
    let mut app = two_agent_app_with_bg_task();

    dispatch(
        Action::TaskComplete(TaskResult::BgTaskKilled {
            session_id: "sess-B".into(),
            task_id: "task-B-1".into(),
            outcome: None,
        }),
        &mut app,
    );

    let task = &app.agents[&AgentId(1)].session.bg_tasks["task-B-1"];
    assert!(!task.pending_kill);
    assert!(task.kill_requested_at.is_none());
}

#[test]
fn bg_task_killed_keeps_pending_kill_on_killed_outcome() {
    let mut app = two_agent_app_with_bg_task();

    dispatch(
        Action::TaskComplete(TaskResult::BgTaskKilled {
            session_id: "sess-B".into(),
            task_id: "task-B-1".into(),
            outcome: Some(xai_grok_tools::types::KillOutcome::Killed),
        }),
        &mut app,
    );

    // "killed" means signal sent, wait for task_completed — pending_kill stays
    let task = &app.agents[&AgentId(1)].session.bg_tasks["task-B-1"];
    assert!(task.pending_kill);
}

#[test]
fn kill_bg_task_action_emits_client_ui_source() {
    use xai_grok_shell::extensions::task::TaskKillSource;

    let mut app = test_app_with_agent();
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent
            .session
            .bg_tasks
            .insert("pane-x".into(), super::make_bg_task("pane-x"));
    }

    let effects = dispatch(Action::KillBgTask("pane-x".into()), &mut app);
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::KillBgTask {
                task_id,
                source: TaskKillSource::ClientUi,
                ..
            }] if task_id == "pane-x"
        ),
        "single-task [×] must stay ClientUi, got {effects:?}"
    );
}

#[test]
fn bg_task_kill_failed_clears_pending_kill_on_inactive_agent() {
    let mut app = two_agent_app_with_bg_task();

    dispatch(
        Action::TaskComplete(TaskResult::BgTaskKillFailed {
            session_id: "sess-B".into(),
            task_id: "task-B-1".into(),
            error: "connection lost".into(),
        }),
        &mut app,
    );

    let task = &app.agents[&AgentId(1)].session.bg_tasks["task-B-1"];
    assert!(!task.pending_kill);
    assert!(task.kill_requested_at.is_none());
}

/// build_rows handles many subagents (placeholder).
#[test]
fn build_rows_collapses_many_subagents() {
    use crate::views::dashboard::build_rows;
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    // Insert 9 subagents.
    for i in 0..9 {
        let info = make_test_subagent(&format!("c{i}"), &format!("sa{i}"));
        agent
            .subagent_sessions
            .insert(info.child_session_id.to_string(), info);
    }
    let rows = build_rows(
        &app.agents,
        &std::collections::BTreeSet::new(),
        &[],
        None,
        crate::views::dashboard::Grouping::State,
        &crate::views::dashboard::Filter::None,
        None,
    );
    // 1 parent + 8 subagents + 1 placeholder = 10.
    assert_eq!(rows.len(), 10);
    assert!(rows.last().unwrap().is_more_placeholder);
    assert_eq!(rows.last().unwrap().more_count, 1);
}

/// Pin the threshold neighbour just BELOW the
/// `MAX_VISIBLE_SUBAGENTS = 8` cap. 7 subagents fit without a
/// placeholder.
#[test]
fn build_rows_seven_subagents_no_placeholder() {
    use crate::views::dashboard::build_rows;
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    for i in 0..7 {
        let info = make_test_subagent(&format!("c{i}"), &format!("sa{i}"));
        agent
            .subagent_sessions
            .insert(info.child_session_id.to_string(), info);
    }
    let rows = build_rows(
        &app.agents,
        &std::collections::BTreeSet::new(),
        &[],
        None,
        crate::views::dashboard::Grouping::State,
        &crate::views::dashboard::Filter::None,
        None,
    );
    // 1 parent + 7 subagents + 0 placeholder = 8.
    assert_eq!(rows.len(), 8);
    assert!(!rows.last().unwrap().is_more_placeholder);
}

/// At the threshold (exactly 8), no placeholder.
#[test]
fn build_rows_eight_subagents_no_placeholder() {
    use crate::views::dashboard::build_rows;
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    for i in 0..8 {
        let info = make_test_subagent(&format!("c{i}"), &format!("sa{i}"));
        agent
            .subagent_sessions
            .insert(info.child_session_id.to_string(), info);
    }
    let rows = build_rows(
        &app.agents,
        &std::collections::BTreeSet::new(),
        &[],
        None,
        crate::views::dashboard::Grouping::State,
        &crate::views::dashboard::Filter::None,
        None,
    );
    // 1 parent + 8 subagents + 0 placeholder = 9.
    assert_eq!(rows.len(), 9);
    assert!(!rows.last().unwrap().is_more_placeholder);
}

/// Well over the threshold (16), placeholder counts
/// the trailing 8 hidden rows.
#[test]
fn build_rows_sixteen_subagents_placeholder_counts_remainder() {
    use crate::views::dashboard::build_rows;
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    for i in 0..16 {
        let info = make_test_subagent(&format!("c{i}"), &format!("sa{i}"));
        agent
            .subagent_sessions
            .insert(info.child_session_id.to_string(), info);
    }
    let rows = build_rows(
        &app.agents,
        &std::collections::BTreeSet::new(),
        &[],
        None,
        crate::views::dashboard::Grouping::State,
        &crate::views::dashboard::Filter::None,
        None,
    );
    // 1 parent + 8 subagents + 1 placeholder = 10.
    assert_eq!(rows.len(), 10);
    assert!(rows.last().unwrap().is_more_placeholder);
    // 16 total - 8 shown = 8 hidden.
    assert_eq!(rows.last().unwrap().more_count, 8);
}

/// The live dashboard builder (`build_rows_with_roster`, used by both
/// rendering and keyboard navigation) hides subagents: only the parent
/// row is listed. The full-tree `build_rows` still emits them.
#[test]
fn build_rows_with_roster_hides_subagent_rows() {
    use crate::views::dashboard::{build_rows, build_rows_with_roster};
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    for i in 0..3 {
        let info = make_test_subagent(&format!("c{i}"), &format!("sa{i}"));
        agent
            .subagent_sessions
            .insert(info.child_session_id.to_string(), info);
    }
    let live = build_rows_with_roster(
        &app.agents,
        &std::collections::BTreeSet::new(),
        &[],
        None,
        crate::views::dashboard::Grouping::State,
        &crate::views::dashboard::Filter::None,
        None,
        &[],
    );
    assert_eq!(live.len(), 1, "only the parent row shows in the dashboard");
    assert!(
        live.iter().all(|r| r.indent == 0),
        "no nested subagent rows in the live dashboard"
    );
    // `build_rows` keeps the full tree (1 parent + 3 subagents).
    let full = build_rows(
        &app.agents,
        &std::collections::BTreeSet::new(),
        &[],
        None,
        crate::views::dashboard::Grouping::State,
        &crate::views::dashboard::Filter::None,
        None,
    );
    assert_eq!(full.len(), 4, "build_rows still emits subagent rows");
    assert!(full.iter().any(|r| r.indent == 1));
}

/// Subagent labels also sanitise ANSI escapes
/// out of the persona.
#[test]
fn subagent_label_strips_control_characters() {
    use crate::views::dashboard::build_rows;
    let mut app = test_app_with_agent();
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    let mut info = make_test_subagent("child-evil", "sa-evil");
    // Inject an ANSI escape into the persona — this is what flows
    // through `format_subagent_label` → row builder sanitisation.
    info.persona = Some(Arc::from("a\x1b[31mevil\x1b[0m"));
    agent
        .subagent_sessions
        .insert(info.child_session_id.to_string(), info);
    let rows = build_rows(
        &app.agents,
        &std::collections::BTreeSet::new(),
        &[],
        None,
        crate::views::dashboard::Grouping::State,
        &crate::views::dashboard::Filter::None,
        None,
    );
    let sub = rows
        .iter()
        .find(|r| r.indent > 0)
        .expect("subagent row expected");
    assert!(
        !sub.label.contains('\x1b'),
        "subagent label must not retain \\x1b: {:?}",
        sub.label
    );
    // Visible characters survive.
    assert!(
        sub.label.contains("evil"),
        "sanitised label should preserve printable characters, got {:?}",
        sub.label
    );
}

/// Sticky must land on parent + subagent and remain on parent after leaving
/// the subagent view (Esc clears `active_subagent` only).
#[serial_test::serial(MOUSE_CAPTURE_ENABLED)]
#[test]
fn mouse_reporting_toggle_sticky_survives_subagent_esc_to_parent() {
    reset_mouse_capture_enabled(true);
    assert!(mouse_capture_is_enabled());
    let mut app = test_app_with_agent();
    let parent_id = AgentId(0);
    let child_sid = "child-mouse-toggle".to_string();

    let child_session = make_test_agent_session(&app, AgentId(1), &child_sid);
    let child = AgentView::new(child_session, ScrollbackState::new());
    {
        let parent = app.agents.get_mut(&parent_id).unwrap();
        parent
            .subagent_views
            .insert(child_sid.clone(), Box::new(child));
        parent.active_subagent = Some(child_sid.clone());
    }
    app.registry = crate::actions::ActionRegistry::defaults_with_config(true);

    // Toggle while subagent is "focused" (active_subagent set).
    let _ = dispatch(Action::ToggleMouseCapture, &mut app);

    let parent = app.agents.get(&parent_id).unwrap();
    assert_eq!(parent.sticky_toast.as_deref(), Some(MOUSE_OFF_STICKY));
    let child = parent.subagent_views.get(&child_sid).unwrap();
    assert_eq!(
        child.sticky_toast.as_deref(),
        Some(MOUSE_OFF_STICKY),
        "child gets sticky recursively even if toast path targeted active view only"
    );

    // Simulate Esc: leave subagent, return to parent agent view.
    app.agents.get_mut(&parent_id).unwrap().active_subagent = None;

    let parent = app.agents.get(&parent_id).unwrap();
    assert_eq!(
        parent.sticky_toast.as_deref(),
        Some(MOUSE_OFF_STICKY),
        "parent keeps sticky after leaving subagent fullscreen"
    );
    assert!(parent.toast.is_none() || parent.sticky_toast.is_some());

    reset_mouse_capture_enabled(true);
}

/// Named contract: mid-turn graceful Quit (SIGTERM / first signal / /exit)
/// writes `canceled_turn_resume.json` so session load can re-queue once.
/// Does not invent finished work for idle sessions.
#[test]
fn quit_mid_turn_writes_canceled_turn_resume_marker() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "quit-resume-sess";
    let cwd = std::path::PathBuf::from("/tmp/quit-resume-cwd");
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.current_prompt_id = Some("pid-quit-1".into());
        agent.session.in_flight_prompt = Some(crate::app::agent::InFlightPrompt {
            text: "finish the multi-track guard".into(),
            images: vec![],
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: vec![],
            chip_elements: vec![],
        });
        agent
            .session
            .note_cancel_resume_prompt_text("finish the multi-track guard");
    }

    let effects = dispatch(Action::Quit, &mut app);
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Quit)),
        "Quit must still emit Effect::Quit; got {effects:?}"
    );

    let cwd_str = cwd.to_string_lossy();
    let loaded =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load marker")
            .expect("marker must exist after mid-turn quit");
    assert_eq!(loaded.prompt_text, "finish the multi-track guard");
    assert_eq!(loaded.prompt_id.as_deref(), Some("pid-quit-1"));
    assert!(
        xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
            true,
            Some(&loaded),
        ),
        "marker must be auto-resume eligible when setting on"
    );

    // Cleanup so later tests / process home stay clean.
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
}

#[test]
fn quit_idle_does_not_write_canceled_turn_resume_marker() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "quit-idle-sess";
    let cwd = std::path::PathBuf::from("/tmp/quit-idle-cwd");
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::Idle;
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.in_flight_prompt = None;
        agent.session.cancel_resume_prompt_text = None;
    }
    // Clear any leftover from a prior run of this test name.
    let cwd_str = cwd.to_string_lossy();
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);

    let _ = dispatch(Action::Quit, &mut app);
    let loaded =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load");
    assert!(
        loaded.is_none(),
        "idle quit must not invent a cancel-resume marker"
    );
}

/// Named contract: after first server activity, `in_flight_prompt` is cleared
/// (pristine composer rewind only). Mid-implement / mid-subagent SIGTERM and
/// `killall` still must write `canceled_turn_resume.json` from the whole-turn
/// cancel-resume text so reopen re-queues once.
#[test]
fn quit_mid_turn_after_first_activity_writes_cancel_resume_marker() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "quit-after-activity-sess";
    let cwd = std::path::PathBuf::from("/tmp/quit-after-activity-cwd");
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.current_prompt_id = Some("pid-after-activity".into());
        // Production clears in_flight_prompt once the server emits any activity
        // (tools, chunks, subagent). That is the killall dogfood state.
        agent.session.in_flight_prompt = None;
        agent
            .session
            .note_cancel_resume_prompt_text("finish the multi-track guard after tools started");
    }

    let effects = dispatch(Action::Quit, &mut app);
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Quit)),
        "Quit must still emit Effect::Quit; got {effects:?}"
    );

    let cwd_str = cwd.to_string_lossy();
    let loaded =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load marker")
            .expect(
                "marker must exist after mid-turn quit even when in_flight_prompt was cleared \
                 by first activity (killall / SIGTERM dogfood)",
            );
    assert_eq!(
        loaded.prompt_text,
        "finish the multi-track guard after tools started"
    );
    assert_eq!(loaded.prompt_id.as_deref(), Some("pid-after-activity"));
    assert!(
        xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
            true,
            Some(&loaded),
        )
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
}

/// Named contract: `/implement` skill-inject drain (wire_blocks + display text)
/// must eagerly write `canceled_turn_resume.json` the same way freeform chat
/// does. Dogfood killall mid-implement requires the display prompt on disk.
#[test]
fn skill_inject_drain_eagerly_writes_cancel_resume_marker() {
    use crate::app::agent::{QueueEntryKind, QueuedPrompt};
    use crate::app::dispatch::queue::maybe_drain_queue;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "skill-inject-eager-sess";
    let cwd = std::path::PathBuf::from("/tmp/skill-inject-eager-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::Idle;
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.loading_replay = false;
        agent.session.pending_prompts.clear();
        // Mirrors CommandResult::InjectSkill enqueue for /implement.
        let qid = agent.session.next_queue_id;
        agent.session.next_queue_id += 1;
        let wire = vec![acp::ContentBlock::Text(acp::TextContent::new(
            "<skill>implement body for the model</skill>",
        ))];
        agent.session.pending_prompts.push_back(QueuedPrompt {
            wire_blocks: Some(wire),
            display_as_skill: true,
            ..QueuedPrompt::plain(
                qid,
                "/implement --effort 2 finish residual",
                QueueEntryKind::Prompt,
            )
        });
        let drain = maybe_drain_queue(agent);
        assert!(
            drain
                .effects
                .iter()
                .any(|e| matches!(e, crate::app::actions::Effect::SendPromptBlocks { .. })),
            "skill inject must drain as SendPromptBlocks; got {:?}",
            drain.effects
        );
        assert!(
            agent.session.state.is_turn_running(),
            "skill inject drain must start a turn"
        );
        assert_eq!(
            agent.session.cancel_resume_prompt_text.as_deref(),
            Some("/implement --effort 2 finish residual"),
            "display text (not raw skill XML) is the cancel-resume identity"
        );
    }
    let loaded =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load")
            .expect("skill inject drain must eagerly write canceled_turn_resume.json");
    assert_eq!(
        loaded.prompt_text, "/implement --effort 2 finish residual",
        "marker must use skill display text for resume"
    );
    assert!(
        xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
            true,
            Some(&loaded),
        )
    );
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: parent turn success while background subagents are still
/// live must **keep** the cancel-resume marker (re-arm parent implement text).
/// Clearing on PromptResponse was the killall-mid-child dogfood hole.
#[test]
fn successful_turn_with_live_subagents_keeps_cancel_resume_marker() {
    use crate::app::agent_view::test_fixtures::running_subagent_info;
    use crate::app::dispatch::turn::finalize_cancel_resume_after_successful_turn;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "keep-marker-live-child-sess";
    let cwd = std::path::PathBuf::from("/tmp/keep-marker-live-child-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        // Parent finished; child still running (background implementer).
        let mut info = running_subagent_info("live-implementer");
        info.is_background = true;
        info.finished = false;
        agent
            .subagent_sessions
            .insert("live-implementer".into(), info);
        assert!(
            agent.holds_queue_for_background(),
            "precondition: live child holds background queue"
        );
        finalize_cancel_resume_after_successful_turn(
            agent,
            Some("/implement finish residual after parent PromptResponse"),
            Some("pid-parent-impl"),
        );
    }
    let loaded =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load")
            .expect("live subagents must keep cancel-resume marker after parent success");
    assert_eq!(
        loaded.prompt_text,
        "/implement finish residual after parent PromptResponse"
    );
    assert_eq!(loaded.prompt_id.as_deref(), Some("pid-parent-impl"));
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: clean success with no live children still clears the marker
/// (do not invent canceled work for finished turns).
#[test]
fn successful_turn_without_live_subagents_clears_cancel_resume_marker() {
    use crate::app::dispatch::turn::finalize_cancel_resume_after_successful_turn;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "clear-marker-clean-success-sess";
    let cwd = std::path::PathBuf::from("/tmp/clear-marker-clean-success-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        "finished work",
        Some("pid-done"),
        "2026-08-08T12:00:00Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("seed marker");
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.subagent_sessions.clear();
        assert!(!agent.holds_queue_for_background());
        finalize_cancel_resume_after_successful_turn(
            agent,
            Some("finished work"),
            Some("pid-done"),
        );
    }
    let loaded =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load");
    assert!(
        loaded.is_none(),
        "clean success with no live children must clear cancel-resume marker"
    );
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: noting cancel-resume text at turn start **eagerly** writes
/// `canceled_turn_resume.json` without Action::Quit / SIGTERM. Tight killall
/// races that never run the async signal task still leave a resumeable marker.
#[test]
fn note_cancel_resume_eagerly_writes_durable_marker_without_quit() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "eager-note-sess";
    let cwd = std::path::PathBuf::from("/tmp/eager-note-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.current_prompt_id = Some("pid-eager-note".into());
        // Mid-implement dogfood: first activity already cleared rewind stash.
        agent.session.in_flight_prompt = None;
        agent
            .session
            .note_cancel_resume_prompt_text("resume me after killall without Quit");
    }
    // No dispatch(Quit): marker must already be on disk from note alone.
    let loaded =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load")
            .expect("eager note must write canceled_turn_resume.json before any signal");
    assert_eq!(loaded.prompt_text, "resume me after killall without Quit");
    assert_eq!(loaded.prompt_id.as_deref(), Some("pid-eager-note"));
    assert!(
        xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
            true,
            Some(&loaded),
        )
    );
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: session load with a cancel-resume marker **starts a live
/// turn** (SendPrompt / equivalent), shows "Continuing interrupted turn...", and
/// does not leave the interrupted text idle in the queue or composer only.
/// Mid-implement eligibility must not drop a non-empty UserCancel marker.
#[test]
fn session_loaded_applies_cancel_resume_marker_and_toasts() {
    use crate::app::actions::TaskResult;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-resume-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-resume-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    // Ensure auto-resume is on (default; pin explicitly for the contract).
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
    }
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        "finish the multi-track guard after killall",
        Some("pid-load-resume"),
        "2026-08-08T12:00:00Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write marker");

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    // Toast must name the resume UX the operator looks for after killall reopen.
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("Continuing interrupted turn"),
        "session load must show cancel-resume toast; got {toast:?}"
    );
    // Hard contract: auto-restart = Send-equivalent effect, not queue-only.
    let started = effects.iter().any(|e| {
        matches!(
            e,
            Effect::SendPrompt {
                text,
                ..
            } if text == "finish the multi-track guard after killall"
        ) || matches!(
            e,
            Effect::SendPromptBlocks { .. } | Effect::SetModeThenPrompt { .. }
        )
    });
    assert!(
        started,
        "session load with cancel-resume marker must emit SendPrompt (auto-run), got {effects:?}"
    );
    assert!(
        agent.session.state.is_turn_running(),
        "resumed turn must be running; state={:?}",
        agent.session.state
    );
    assert!(
        agent.session.pending_prompts.is_empty(),
        "resumed prompt must be drained out of the queue, not left for Enter"
    );
    // Load clears the one-shot marker, then drain eagerly re-writes for the
    // new active turn (killall-safe second interrupt).
    let after =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load after apply");
    if let Some(m) = after {
        assert_eq!(
            m.prompt_text, "finish the multi-track guard after killall",
            "post-drain eager marker must match the resumed prompt"
        );
    }

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract (killall mid-subagent dogfood): cold session load after
/// process death can still have unfinished subagent rows from replay. Those
/// zombies must not block cancel-resume auto-run — the interrupted prompt
/// must Send and the toast must appear.
#[test]
fn session_loaded_cancel_resume_starts_turn_despite_zombie_subagents() {
    use crate::app::actions::TaskResult;
    use crate::app::agent_view::test_fixtures::running_subagent_info;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-resume-zombie-sub-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-resume-zombie-sub-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        // Killall mid-implement: SubagentFinished never landed; row still
        // unfinished after history replay.
        let mut info = running_subagent_info("zombie-child");
        info.is_background = true;
        info.finished = false;
        agent.subagent_sessions.insert("zombie-child".into(), info);
        assert!(
            agent.holds_queue_for_background(),
            "precondition: unfinished subagent holds the normal queue drain"
        );
    }
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        "restart implement after killall mid-subagent",
        Some("pid-zombie-resume"),
        "2026-08-08T12:30:00Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write marker");

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("Continuing interrupted turn"),
        "zombie subagents must not skip the cancel-resume toast; got {toast:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. }
                if text == "restart implement after killall mid-subagent"
        )),
        "must auto-start the interrupted turn despite zombie subagents; effects={effects:?}"
    );
    assert!(
        agent.session.state.is_turn_running(),
        "turn must be running after cancel-resume with zombie children"
    );
    assert!(
        !agent.holds_queue_for_background(),
        "cold load must finalize zombie subagents so background hold is cleared"
    );
    assert!(
        agent
            .subagent_sessions
            .get("zombie-child")
            .is_some_and(|s| s.finished),
        "zombie child must be marked finished on cold load"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: cold load with **no** cancel-resume marker, unfinished
/// subagent from killall mid-wave, and last user prompt in scrollback must
/// still auto-start that prompt (history recovery). Toast names interrupted.
#[test]
fn session_loaded_recovers_interrupted_turn_without_marker() {
    use crate::app::actions::TaskResult;
    use crate::app::agent_view::test_fixtures::running_subagent_info;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-history-resume-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-history-resume-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("implement foo"));
        let mut info = running_subagent_info("zombie-history-child");
        info.is_background = true;
        info.finished = false;
        agent
            .subagent_sessions
            .insert("zombie-history-child".into(), info);
        assert!(
            crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: unfinished subagent is interruption evidence"
        );
        assert!(
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .unwrap()
                .is_none(),
            "precondition: no marker on disk"
        );
    }

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("Continuing interrupted turn"),
        "history recovery toast must name interrupted; got {toast:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. } if text == "implement foo"
        )),
        "no-marker interrupted load must SendPrompt last user text; effects={effects:?}"
    );
    assert!(
        agent.session.state.is_turn_running(),
        "history-recovered turn must be running; state={:?}",
        agent.session.state
    );
    assert!(
        agent
            .subagent_sessions
            .get("zombie-history-child")
            .is_some_and(|s| s.finished),
        "zombie child finalized on cold load before force-drain"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: clean completed turn (no marker, no unfinished children,
/// no running scrollback) must **not** invent an auto SendPrompt on load.
#[test]
fn session_loaded_clean_completed_does_not_auto_resume_without_marker() {
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-clean-no-resume-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-clean-no-resume-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        // Prior implement exists in history but turn completed cleanly
        // (terminal session event present — open-turn recovery must not fire).
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("implement foo"));
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("all done"));
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::TurnCompleted {
                elapsed: None,
            }));
        assert!(
            !crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: clean session is not interrupted"
        );
    }

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. }
                | Effect::SendPromptBlocks { .. }
                | Effect::SetModeThenPrompt { .. }
        )),
        "clean completed load must not auto SendPrompt; effects={effects:?}"
    );
    assert!(
        !agent.session.state.is_turn_running(),
        "clean load must stay idle; state={:?}",
        agent.session.state
    );
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        !toast.contains("Resuming"),
        "clean load must not toast resume; got {toast:?}"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract (false-positive dogfood): after load replay, clean completed
/// turns have **no** SessionEvent terminal in scrollback (durable
/// `TurnCompleted` only sets `last_primary_user_turn_completed_in_replay`).
/// Must **not** re-fire last user text (e.g. "Still nothing!!! [Image #1]").
#[test]
fn session_loaded_replay_completed_without_session_event_does_not_auto_resume() {
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-replay-completed-no-se-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-replay-completed-no-se-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    let last_user = "Still nothing!!! [Image #1]";
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        // Real load shape: user + agent work, no SessionEvent terminal block.
        // Replay already saw primary-user TurnCompleted for this turn.
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(last_user));
        agent.scrollback.push_block(RenderBlock::agent_message(
            "root cause was evidence + old process",
        ));
        agent
            .replayed_terminal_prompts
            .insert("5828c1b2-done".into());
        agent.last_primary_user_turn_completed_in_replay = true;
        assert!(
            !crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: replay-completed primary turn is not interrupted"
        );
        assert!(
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .unwrap()
                .is_none(),
            "precondition: no marker"
        );
    }

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. }
                | Effect::SendPromptBlocks { .. }
                | Effect::SetModeThenPrompt { .. }
        )),
        "replay-completed load must not re-fire last prompt; effects={effects:?}"
    );
    assert!(
        !agent.session.state.is_turn_running(),
        "must stay idle; state={:?}",
        agent.session.state
    );
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        !toast.contains("Resuming"),
        "must not toast resume after clean completed replay; got {toast:?}"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: user-cancelled terminal (TurnCancelled SessionEvent) must
/// not history-resume without a marker. Marker path still wins separately.
#[test]
fn session_loaded_user_cancelled_terminal_does_not_history_resume() {
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-user-cancelled-no-hist-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-user-cancelled-no-hist-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("do a thing"));
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("working on it"));
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::TurnCancelled {
                elapsed: std::time::Duration::from_secs(1),
            }));
        assert!(
            !crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: cancelled terminal is not open mid-work"
        );
    }

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. }
                | Effect::SendPromptBlocks { .. }
                | Effect::SetModeThenPrompt { .. }
        )),
        "cancelled terminal without marker must not history-resume; effects={effects:?}"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract (iso dogfood shape): no marker, **no** unfinished subagent,
/// last user is `/implement …`, agent work in scrollback without a turn
/// terminal (parent parked on suppressed wait / killall mid-turn). Must still
/// SendPrompt the implement text with interrupted toast.
#[test]
fn session_loaded_recovers_open_implement_turn_without_unfinished_subagent() {
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-iso-open-implement-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-iso-open-implement-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    let implement = "/implement --effort 2 all remaining residual tasks in priority order";
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        // Iso shape: finished children only (none unfinished), open turn
        // after last user implement with agent work and no TurnCompleted.
        // Prior primary turns may have completed in replay; the open implement
        // must leave last_primary_user_turn_completed_in_replay == false.
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(implement));
        agent
            .scrollback
            .push_block(RenderBlock::thinking("planning implement wave"));
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("Spawning implementer..."));
        agent.last_primary_user_turn_completed_in_replay = false;
        // Finished subagent row (not unfinished) — must not be required.
        assert!(
            agent.subagent_sessions.values().all(|s| s.finished)
                || agent.subagent_sessions.is_empty(),
            "precondition: no unfinished subagent rows"
        );
        assert!(
            !agent.scrollback.has_running_entries(),
            "precondition: no running scrollback (wait tools are suppressed)"
        );
        assert!(
            crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: open implement turn without terminal is interruption evidence"
        );
        assert!(
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .unwrap()
                .is_none(),
            "precondition: no marker on disk"
        );
    }

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("Continuing interrupted turn"),
        "iso open-turn recovery toast must name interrupted; got {toast:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. } if text == implement
        )),
        "iso open implement turn must SendPrompt last /implement text; effects={effects:?}"
    );
    assert!(
        agent.session.state.is_turn_running(),
        "iso history-recovered turn must be running; state={:?}",
        agent.session.state
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: open-turn evidence but no resumable user prompt must toast
/// failure loudly (not silent idle).
#[test]
fn session_loaded_interrupted_without_prompt_toasts_failure() {
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-interrupted-no-prompt-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-interrupted-no-prompt-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        // Agent work without any user prompt — interrupted evidence only via
        // running scrollback entry, no text to re-queue.
        let entry_id = agent
            .scrollback
            .push_block(RenderBlock::thinking("orphan mid-turn thought"));
        agent.scrollback.set_entry_running(entry_id, true);
        assert!(
            crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: running thought is interruption evidence"
        );
    }

    let _effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("Interrupted work found but could not continue"),
        "must toast loud failure when interrupted without prompt; got {toast:?}"
    );
    assert!(
        !agent.session.state.is_turn_running(),
        "must stay idle without a prompt to re-queue; state={:?}",
        agent.session.state
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: when a marker is present, it wins over history recovery
/// (marker prompt text is re-queued, not a different scrollback prompt).
#[test]
fn session_loaded_marker_wins_over_history_recovery() {
    use crate::app::actions::TaskResult;
    use crate::app::agent_view::test_fixtures::running_subagent_info;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-marker-wins-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-marker-wins-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        // Scrollback has a different last user text than the marker.
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt("implement from scrollback only"));
        let mut info = running_subagent_info("marker-wins-child");
        info.finished = false;
        agent
            .subagent_sessions
            .insert("marker-wins-child".into(), info);
    }
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        "finish the multi-track guard from marker",
        Some("pid-marker-wins"),
        "2026-08-08T14:00:00Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write marker");

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    // Marker and history recovery share the same operator-facing toast
    // ("Continuing interrupted turn..."). Distinction is which prompt text
    // re-queues: marker wins over last scrollback user prompt.
    assert!(
        toast.contains("Continuing interrupted turn"),
        "continue-interrupted-turn toast; got {toast:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. }
                if text == "finish the multi-track guard from marker"
        )),
        "marker prompt must win over scrollback text; effects={effects:?}"
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. } if text == "implement from scrollback only"
        )),
        "must not send scrollback prompt when marker is present"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract (rebuild / reopen dogfood): stale `canceled_turn_resume.json`
/// after a **clean completed** primary turn must **not** auto SendPrompt.
/// Shape: marker left from eager turn-start or missed clear + replay flag
/// `last_primary_user_turn_completed_in_replay` + no mid-work evidence.
/// Operator: `/rebuild` while idle re-fired "??? [Image #1]".
#[test]
fn session_loaded_stale_marker_after_completed_primary_does_not_resume() {
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-stale-marker-completed-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-stale-marker-completed-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    let last_user = "??? [Image #1]";
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(last_user));
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("all done after image"));
        // Real load: durable TurnCompleted is NOT a SessionEvent; only the
        // replay flag records a finished primary user turn.
        agent.last_primary_user_turn_completed_in_replay = true;
        assert!(
            !crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: completed primary is not mid-work"
        );
    }
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        last_user,
        Some("882cb6c3-stale"),
        "2026-08-08T11:36:52Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write stale marker");

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    let agent = app.agents.get(&id).unwrap();
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. }
                | Effect::SendPromptBlocks { .. }
                | Effect::SetModeThenPrompt { .. }
        )),
        "stale marker after completed primary must not auto SendPrompt; effects={effects:?}"
    );
    assert!(
        !agent.session.state.is_turn_running(),
        "must stay idle after rebuild-style reload; state={:?}",
        agent.session.state
    );
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        !toast.contains("Resuming"),
        "must not toast resume for stale completed-turn marker; got {toast:?}"
    );
    let after =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load after");
    assert!(
        after.is_none(),
        "stale marker must be cleared on load so next reopen stays clean"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Live dogfood (session `019faf9d…`, 2026-08-08T12:07Z on grok-oss):
/// durable primary `TurnCompleted` sets the completed flag but does **not**
/// `finish_turn` during load replay, so tracker/scrollback still show a
/// running agent stream. That residue must **not** count as mid-work, or the
/// stale-marker gate always fails and reopen re-fires `??? [Image #1]`.
#[test]
fn session_loaded_stale_marker_ignores_replay_running_residue_after_completed_primary() {
    use crate::acp::meta::NotificationMeta;
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-stale-marker-replay-residue-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-stale-marker-replay-residue-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    let stale_prompt = "??? [Image #1]";
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        // Last real user turn was a later completed prompt; marker text is older.
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(stale_prompt));
        agent.scrollback.push_block(RenderBlock::user_prompt(
            "Seems to be very broken right now",
        ));
        // Replay left the agent stream open (TurnCompleted does not finish_turn
        // while loading_replay). This is the live false mid-work signal.
        let meta = NotificationMeta {
            is_replay: true,
            ..NotificationMeta::default()
        };
        agent.session.tracker.handle_update(
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("all done after forensic turn"),
            ))),
            &meta,
            &mut agent.scrollback,
        );
        agent.last_primary_user_turn_completed_in_replay = true;
        assert!(
            agent.scrollback.has_running_entries()
                || agent.session.tracker.has_in_flight_mid_turn_activity(),
            "precondition: replay residue must leave running/tracker mid-turn state"
        );
        assert!(
            !crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "completed primary + only replay residue must not count as mid-work"
        );
    }
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        stale_prompt,
        Some("ca5862b5-live-stale"),
        "2026-08-08T11:56:26Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write stale marker");

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. }
                | Effect::SendPromptBlocks { .. }
                | Effect::SetModeThenPrompt { .. }
        )),
        "stale marker + completed primary + replay residue must not SendPrompt; effects={effects:?}"
    );
    let agent = app.agents.get(&id).unwrap();
    assert!(
        !agent.session.state.is_turn_running(),
        "must stay idle; state={:?}",
        agent.session.state
    );
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        !toast.contains("Resuming"),
        "must not toast resume; got {toast:?}"
    );
    let after =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load after");
    assert!(
        after.is_none(),
        "stale marker must be cleared so next reopen stays clean"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: Esc/user-cancel marker still auto-resumes when the primary
/// turn did **not** complete successfully (replay flag stays false for
/// `cancelled` stop_reason). Mid-work evidence is optional.
#[test]
fn session_loaded_cancel_marker_without_completed_primary_still_resumes() {
    use crate::app::actions::TaskResult;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-cancel-marker-no-completed-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-cancel-marker-no-completed-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        agent.scrollback.push_block(RenderBlock::user_prompt(
            "finish the multi-track guard after Esc",
        ));
        agent
            .scrollback
            .push_block(RenderBlock::agent_message("working…"));
        // Cancelled primary: flag stays false (stop_reason cancelled does not
        // set last_primary_user_turn_completed_in_replay).
        agent.last_primary_user_turn_completed_in_replay = false;
    }
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        "finish the multi-track guard after Esc",
        Some("pid-esc-cancel"),
        "2026-08-08T15:00:00Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write marker");

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. }
                if text == "finish the multi-track guard after Esc"
        )),
        "Esc-cancel marker must still auto-resume; effects={effects:?}"
    );
    let agent = app.agents.get(&id).unwrap();
    let toast = agent
        .toast
        .as_ref()
        .map(|(msg, _)| msg.as_str())
        .unwrap_or("");
    assert!(
        toast.contains("Continuing interrupted turn"),
        "cancel marker toast; got {toast:?}"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: mid-work + completed-primary flag still honors marker
/// (parent finished, live children kept marker, killall mid-child).
#[test]
fn session_loaded_marker_with_unfinished_child_resumes_despite_completed_primary_flag() {
    use crate::app::actions::TaskResult;
    use crate::app::agent_view::test_fixtures::running_subagent_info;
    use crate::scrollback::block::RenderBlock;
    use agent_client_protocol as acp;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sid = "load-marker-live-child-sess";
    let cwd = std::path::PathBuf::from("/tmp/load-marker-live-child-cwd");
    let cwd_str = cwd.to_string_lossy().into_owned();
    app.current_ui.resume_canceled_turn_on_restart = Some(true);
    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::Idle;
        agent.session.loading_replay = true;
        agent.session.pending_prompts.clear();
        agent.scrollback.push_block(RenderBlock::user_prompt(
            "implement keep marker while children live",
        ));
        agent.scrollback.push_block(RenderBlock::agent_message(
            "parent done, child still running",
        ));
        // Parent primary completed in replay, but unfinished child = mid-work.
        agent.last_primary_user_turn_completed_in_replay = true;
        let mut info = running_subagent_info("live-child-after-parent");
        info.finished = false;
        agent
            .subagent_sessions
            .insert("live-child-after-parent".into(), info);
        assert!(
            crate::app::dispatch::session::load::session_looks_interrupted_mid_work(agent),
            "precondition: unfinished child is mid-work"
        );
    }
    let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
        "implement keep marker while children live",
        Some("pid-keep-child"),
        "2026-08-08T16:00:00Z",
    )
    .expect("marker");
    xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
        &cwd_str, sid, &marker,
    )
    .expect("write marker");

    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionLoaded {
            agent_id: id,
            session_id: acp::SessionId::new(sid),
            models: None,
            code_restored: false,
            restore_summary: None,
            restore_degree: None,
            running_prompt_id: None,
        }),
        &mut app,
    );

    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { text, .. }
                if text == "implement keep marker while children live"
        )),
        "live-child mid-work must still apply marker; effects={effects:?}"
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}
