//! Dispatch integration tests for fearless global pause / resume.

use super::*;
use crate::app::global_work_pause::{GlobalWorkPause, PausedSessionSnapshot};
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

/// Named contract (idle over-window compact-fail unstick): mouse-host idle
/// `[pause]` after AUTO compact failed for spending-limit, with used tokens
/// over the sampling window, must not be a no-op empty pause. It must retry
/// manual `/compact` (AUTO suppress does not apply) and continue the last
/// user prompt (`/implement` here) so the operator is not stuck at 507K/500K.
#[test]
fn idle_pause_on_over_window_compact_fail_retries_compact_and_continues_last_prompt() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::Idle;
        agent.session.in_flight_prompt = None;
        agent.session.compact_held_prompt = None;
        agent.session_sampling_window = Some(500_000);
        agent.context_state = Some(xai_grok_shell::session::ContextInfo::from_notification(
            507_000, 500_000,
        ));
        agent
            .scrollback
            .push_block(RenderBlock::UserPrompt(UserPromptBlock::new(
                "/implement --effort 3",
            )));
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::CompactionFailed {
                error: "Compaction sampler got a spending-limit response. \
                     The live compact chip already shows console team prepaid remaining."
                    .into(),
            }));
    }
    assert!(app.agents[&id].session.state.is_idle());
    assert!(!app.global_work_pause.is_active());

    let effects = dispatch(Action::ToggleGlobalPause, &mut app);

    assert!(
        !app.global_work_pause.is_active(),
        "idle over-window compact-fail pause must unstick, not engage empty global pause"
    );
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Compact { .. })),
        "pause must retry compact so the session can leave 507K/500K, got {effects:?}"
    );
    let continued = app.agents[&id]
        .session
        .pending_prompts
        .iter()
        .any(|p| p.text == "/implement --effort 3")
        || effects.iter().any(
            |e| matches!(e, Effect::SendPrompt { text, .. } if text == "/implement --effort 3"),
        );
    assert!(
        continued,
        "pause must continue the interrupted /implement after compact; \
         effects={effects:?} queue={:?}",
        app.agents[&id]
            .session
            .pending_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
    );
    let toast = app.agents[&id]
        .toast
        .as_ref()
        .map(|(m, _)| m.as_str())
        .unwrap_or("");
    assert!(
        !toast.to_ascii_lowercase().contains("nothing pending"),
        "must not toast Resumed · nothing pending on this stuck state: {toast}"
    );
    assert!(
        !toast.to_ascii_lowercase().contains("add credits"),
        "must not tell Add credits while console remaining can be painted: {toast}"
    );
    assert!(
        toast.to_ascii_lowercase().contains("supergrok"),
        "compact recovery must name Stay on SuperGrok if that is the live identity: {toast}"
    );
}

/// 12:17 iso: compact fail is not the last scrollback block. `finish_turn`
/// pushes TurnFailed, Stay-on-SuperGrok is the compact banner, then a leftover
/// `/implement` is loaded as a cancel-resume / compact-held user line. Idle
/// `[pause]` must still retry compact and continue the real prior user work,
/// not empty-pause and not start a second `/implement` turn.
fn setup_idle_over_window_compact_fail_with_stale_implement(app: &mut AppView) {
    let id = AgentId(0);
    let agent = app.agents.get_mut(&id).unwrap();
    agent.session.state = AgentState::Idle;
    agent.session.in_flight_prompt = None;
    agent.session.compact_held_prompt = None;
    agent.session.pending_prompts.clear();
    agent.session_sampling_window = Some(500_000);
    agent.context_state = Some(xai_grok_shell::session::ContextInfo::from_notification(
        507_000, 500_000,
    ));
    agent
        .scrollback
        .push_block(RenderBlock::UserPrompt(UserPromptBlock::new(
            "keep going on the compiler",
        )));
    agent
        .scrollback
        .push_block(RenderBlock::session_event(SessionEvent::CompactionFailed {
            error: "Compaction sampler got a spending-limit response. \
                 grok-oss remaining is a client printout, not xAI billing truth. \
                 Stay on SuperGrok if that is the live identity. \
                 Check the product Usage view or console.x.ai Billing."
                .into(),
        }));
    agent
        .scrollback
        .push_block(RenderBlock::session_event(SessionEvent::TurnFailed {
            error: "context is over the sampling window".into(),
            elapsed: None,
        }));
    agent
        .scrollback
        .push_block(RenderBlock::UserPrompt(UserPromptBlock::new(
            "/implement --effort 3 --from plan.md",
        )));
}

fn assert_over_window_compact_unstick(app: &AppView, effects: &[Effect], toast: &str) {
    let id = AgentId(0);
    assert!(
        !app.global_work_pause.is_active(),
        "over-window compact-fail pause/resume must unstick, not leave or engage empty pause"
    );
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Compact { .. })),
        "must retry compact so the session can leave 507K/500K, got {effects:?}"
    );
    let queue: Vec<&str> = app.agents[&id]
        .session
        .pending_prompts
        .iter()
        .map(|p| p.text.as_str())
        .collect();
    let continued_real = queue.iter().any(|t| *t == "keep going on the compiler")
        || effects.iter().any(
            |e| matches!(e, Effect::SendPrompt { text, .. } if text == "keep going on the compiler"),
        );
    let invented_implement = queue.iter().any(|t| t.starts_with("/implement"))
        || effects.iter().any(
            |e| matches!(e, Effect::SendPrompt { text, .. } if text.starts_with("/implement")),
        );
    assert!(
        continued_real,
        "must continue the real prior user work after compact; effects={effects:?} queue={queue:?}"
    );
    assert!(
        !invented_implement,
        "must not start a second /implement turn from a stale resume line; \
         effects={effects:?} queue={queue:?}"
    );
    assert!(
        !toast.to_ascii_lowercase().contains("nothing pending"),
        "must not toast Resumed · nothing pending on this stuck state: {toast}"
    );
    assert!(
        !toast.to_ascii_lowercase().contains("add credits"),
        "must not tell Add credits from a client remaining printout: {toast}"
    );
    assert!(
        toast.to_ascii_lowercase().contains("supergrok"),
        "compact recovery must name Stay on SuperGrok if that is the live identity: {toast}"
    );
}

/// Named contract (12:17 idle `[pause]` after compact fail): trailing
/// TurnFailed + leftover `/implement` user line must not hide the compact-fail
/// unstick. Do not treat that slash replay as the work to continue.
#[test]
fn idle_pause_after_compact_fail_skips_stale_implement_and_retries_compact() {
    let mut app = test_app_with_agent();
    setup_idle_over_window_compact_fail_with_stale_implement(&mut app);
    assert!(app.agents[&AgentId(0)].session.state.is_idle());
    assert!(!app.global_work_pause.is_active());

    let effects = dispatch(Action::ToggleGlobalPause, &mut app);
    let toast = app.agents[&AgentId(0)]
        .toast
        .as_ref()
        .map(|(m, _)| m.as_str())
        .unwrap_or("");
    assert_over_window_compact_unstick(&app, &effects, toast);
}

/// Named contract (12:17 `[pause]` while already empty-paused): first click
/// missed unstick (trailing `/implement`), so resume toasts
/// `Resumed · nothing pending` with the cursor still on `[pause]`. Resume
/// must unstick the same way: retry compact, continue the real prior work.
#[test]
fn resume_after_empty_pause_on_compact_fail_does_not_toast_nothing_pending() {
    let mut app = test_app_with_agent();
    setup_idle_over_window_compact_fail_with_stale_implement(&mut app);
    app.global_work_pause.engage(
        Instant::now(),
        vec![PausedSessionSnapshot::capture(
            AgentId(0),
            Some("sess-0".into()),
            false,
            0,
            None,
        )],
    );
    crate::app::active_session_heartbeat::set_global_work_paused(true);
    assert!(app.global_work_pause.is_active());
    assert_eq!(app.global_work_pause.sessions_held_count(), 0);

    let effects = dispatch(Action::ToggleGlobalPause, &mut app);
    let toast = app.agents[&AgentId(0)]
        .toast
        .as_ref()
        .map(|(m, _)| m.as_str())
        .unwrap_or("");
    assert_over_window_compact_unstick(&app, &effects, toast);
}

/// Named contract: `[pause]` while chrome is `Waiting for the model…`
/// (first-token wait, including after Retrying cleared the rewind stash)
/// must cancel that sampler wait. The session must not stay TurnRunning
/// with Waiting(Model) after pause.
#[test]
fn pause_during_waiting_for_the_model_cancels_the_sampler_wait() {
    use crate::acp::tracker::{TurnActivity, WaitingReason};

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("first-token-wait".into());
        // Retrying / StreamResumed clears this stash; pause must still cancel.
        agent.session.in_flight_prompt = None;
        agent.active_pane = ActivePane::Scrollback;
    }
    assert!(
        matches!(
            app.agents[&id].resolve_turn_activity(),
            Some(TurnActivity::Waiting(WaitingReason::Model))
        ),
        "fixture is the live first-token wait, got {:?}",
        app.agents[&id].resolve_turn_activity()
    );

    let pause_effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(app.global_work_pause.is_active());
    assert!(
        pause_effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "pause during Waiting for the model must cancel the turn: {pause_effects:?}"
    );
    assert!(
        !app.agents[&id].session.state.is_turn_running(),
        "pause must unstick the model wait, state={:?}",
        app.agents[&id].session.state
    );
    assert!(
        !matches!(
            app.agents[&id].resolve_turn_activity(),
            Some(TurnActivity::Waiting(WaitingReason::Model))
        ),
        "must not stay Waiting for the model after pause, got {:?}",
        app.agents[&id].resolve_turn_activity()
    );
}

/// Same unstick when chrome is Retrying (attempt N / first token timed out).
#[test]
fn pause_during_retrying_cancels_the_sampler_wait() {
    use crate::acp::tracker::TurnActivity;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("retry-wait".into());
        agent.session.in_flight_prompt = None;
        agent
            .session
            .set_retry_activity(Some(TurnActivity::Retrying {
                attempt: 1,
                max_retries: 3,
                reason: "waiting for first token".into(),
            }));
    }
    assert!(
        matches!(
            app.agents[&id].resolve_turn_activity(),
            Some(TurnActivity::Retrying { attempt: 1, .. })
        ),
        "fixture is Retrying chrome, got {:?}",
        app.agents[&id].resolve_turn_activity()
    );

    let pause_effects = dispatch(Action::ToggleGlobalPause, &mut app);
    assert!(
        pause_effects
            .iter()
            .any(|e| matches!(e, Effect::CancelTurn { .. })),
        "pause during Retrying must cancel the sampler wait: {pause_effects:?}"
    );
    assert!(
        !app.agents[&id].session.state.is_turn_running(),
        "pause must unstick Retrying, state={:?}",
        app.agents[&id].session.state
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

/// Named contract: fearless global pause that cancels a running primary
/// turn must write `canceled_turn_resume.json` the same way `/rebuild`
/// mid-turn does, so last-session on start and `/start` can continue the
/// interrupted prompt after this process is gone. The in-memory pause
/// gate is still RAM-only; this is only the interrupted-prompt marker.
#[test]
fn pause_mid_turn_writes_cancel_resume_marker_for_restart() {
    let proj = tempfile::tempdir().unwrap();
    let cwd = proj.path().to_path_buf();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let sid = "pause-cancel-resume-mid-turn";
    let prompt = "keep going after pause if this process dies";

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(sid.into());
        agent.session.cwd = cwd.clone();
        agent.session.state = AgentState::TurnRunning;
        agent.session.current_prompt_id = Some("pid-pause-resume".into());
        agent.session.in_flight_prompt = Some(crate::app::agent::InFlightPrompt {
            text: prompt.into(),
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

    // Process state dropped: RAM pause snapshots are gone. Disk marker must
    // still exist so reopen / `/start` can continue the interrupted prompt.
    drop(app);

    let marker =
        xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
            .expect("load marker")
            .expect(
                "pause that cancels a running turn must write canceled_turn_resume.json \
                 so reopen continues the turn",
            );
    assert_eq!(marker.prompt_text, prompt);
    assert!(
        xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
            true,
            Some(&marker)
        )
    );

    let _ =
        xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(&cwd_str, sid);
    xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
}

/// Named contract: leftover `/implement` after HTTP 502 is not the compact-fail
/// skip. Over-window idle `[pause]` still retries compact, then continues that
/// leftover `/implement` so looping can run when the session is able to sample.
/// Compact-fail skip without a later 502 stays in
/// `idle_pause_after_compact_fail_skips_stale_implement_and_retries_compact`.
#[test]
fn idle_pause_after_http_502_does_not_skip_leftover_implement() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::Idle;
        agent.session.in_flight_prompt = None;
        agent.session.compact_held_prompt = None;
        agent.session.pending_prompts.clear();
        agent.session_sampling_window = Some(500_000);
        agent.context_state = Some(xai_grok_shell::session::ContextInfo::from_notification(
            507_000, 500_000,
        ));
        agent
            .scrollback
            .push_block(RenderBlock::UserPrompt(UserPromptBlock::new(
                "keep going on the compiler",
            )));
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::CompactionFailed {
                error: "Compaction sampler got a spending-limit response. \
                     Stay on SuperGrok if that is the live identity."
                    .into(),
            }));
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::TurnFailed {
                error: "context is over the sampling window".into(),
                elapsed: None,
            }));
        agent
            .scrollback
            .push_block(RenderBlock::UserPrompt(UserPromptBlock::new(
                "/implement --effort 3 leftover after 502",
            )));
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::RetryFailed {
                error: "API error (status 502 Bad Gateway): temporarily unavailable".into(),
                error_type: None,
            }));
    }

    let effects = dispatch(Action::ToggleGlobalPause, &mut app);
    let queue: Vec<&str> = app.agents[&id]
        .session
        .pending_prompts
        .iter()
        .map(|p| p.text.as_str())
        .collect();
    let continued_implement = queue
        .iter()
        .any(|t| t.contains("/implement") && t.contains("leftover after 502"))
        || effects.iter().any(|e| {
            matches!(
                e,
                Effect::SendPrompt { text, .. }
                    if text.contains("/implement") && text.contains("leftover after 502")
            )
        });
    let continued_prior = queue.iter().any(|t| *t == "keep going on the compiler")
        || effects.iter().any(
            |e| matches!(e, Effect::SendPrompt { text, .. } if text == "keep going on the compiler"),
        );

    assert!(
        !app.global_work_pause.is_active(),
        "over-window compact-fail plus 502 must unstick, not empty-pause"
    );
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Compact { .. })),
        "must still retry compact so the session can leave 507K/500K, got {effects:?}"
    );
    assert!(
        continued_implement,
        "HTTP 502 must not use the compact-fail leftover /implement skip; \
         continue leftover /implement after compact; effects={effects:?} queue={queue:?}"
    );
    assert!(
        !continued_prior,
        "do not skip leftover /implement in favor of older prior work when 502 is later; \
         effects={effects:?} queue={queue:?}"
    );
}
