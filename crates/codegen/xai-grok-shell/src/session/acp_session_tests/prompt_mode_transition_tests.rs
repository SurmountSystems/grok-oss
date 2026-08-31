use super::support::*;
use super::*;
use crate::session::plan_mode::PlanModeState;
/// An actor plus the `SessionEvent` rail its mode updates ride. Plan-mode
/// changes deliberately queue behind the turn's streaming deltas rather than
/// emitting straight to the client, so the assertions have to read that rail
/// and not the gateway.
async fn actor_with_events() -> (
    SessionActor,
    tokio::sync::mpsc::UnboundedReceiver<SessionEvent>,
) {
    let (gateway_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel();
    create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await
}
/// Every mode id the actor has queued for the client so far, in order.
fn mode_updates(rx: &mut tokio::sync::mpsc::UnboundedReceiver<SessionEvent>) -> Vec<String> {
    let mut seen = Vec::new();
    while let Ok(SessionEvent::Notification(notification)) = rx.try_recv() {
        let SessionNotification::Acp(notification) = notification else {
            continue;
        };
        if let acp::SessionUpdate::CurrentModeUpdate(update) = &notification.update {
            seen.push(update.current_mode_id.0.to_string());
        }
    }
    seen
}
#[test]
fn prompt_mode_from_session_mode_id_uses_acp_session_mode() {
    assert_eq!(
        PromptMode::Ask,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("ask"))
    );
    assert_eq!(
        PromptMode::Plan,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("plan"))
    );
    assert_eq!(
        PromptMode::Agent,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("default"))
    );
    assert_eq!(
        PromptMode::Agent,
        prompt_mode_from_session_mode_id(&acp::SessionModeId::new("browser_use"))
    );
}
fn fn_def(name: &str) -> ToolDefinition {
    ToolDefinition::function(name, None::<&str>, serde_json::json!({"type": "object"}))
}
fn names(defs: &[ToolDefinition]) -> Vec<&str> {
    defs.iter().map(|d| d.function.name.as_str()).collect()
}
#[test]
fn cursor_filter_in_plan_mode_keeps_writes_and_shows_create_plan() {
    let defs = vec![
        fn_def("Read"),
        fn_def("Grep"),
        fn_def("Write"),
        fn_def("StrReplace"),
        fn_def("CreatePlan"),
        fn_def("SwitchMode"),
        fn_def("AskQuestion"),
    ];
    let filtered = filter_cursor_tools_by_plan_mode(defs, true);
    let kept = names(&filtered);
    assert!(kept.contains(&"Read"));
    assert!(kept.contains(&"Grep"));
    assert!(kept.contains(&"CreatePlan"));
    assert!(kept.contains(&"SwitchMode"));
    // Cursor AskQuestion is a different surface; only the grok questionnaire
    // names are stripped (see is_plan_mode_blocked_ask_user_tool_name).
    assert!(kept.contains(&"AskQuestion"));
    assert!(kept.contains(&"Write"));
    assert!(kept.contains(&"StrReplace"));
}
/// Plan mode must hard-strip `ask_user_question` from the advertised tool list.
/// Soft prompt bans alone left the tool available and models still opened
/// multi-choice plan questionnaires.
#[test]
fn plan_mode_tool_list_omits_ask_user_question() {
    let defs = vec![
        fn_def("read_file"),
        fn_def("search_replace"),
        fn_def("write"),
        fn_def("ask_user_question"),
        fn_def("AskUserQuestion"),
        fn_def("AskUser"),
        fn_def("enter_plan_mode"),
        fn_def("exit_plan_mode"),
    ];
    let in_plan = filter_cursor_tools_by_plan_mode(defs.clone(), true);
    let out_of_plan = filter_cursor_tools_by_plan_mode(defs.clone(), false);
    let in_names = names(&in_plan);
    assert!(
        !in_names.contains(&"ask_user_question"),
        "plan mode must not advertise ask_user_question: {in_names:?}"
    );
    assert!(!in_names.contains(&"AskUserQuestion"));
    assert!(!in_names.contains(&"AskUser"));
    assert!(in_names.contains(&"read_file"));
    assert!(in_names.contains(&"exit_plan_mode"));
    assert!(in_names.contains(&"search_replace"));
    // Outside plan mode the questionnaire stays available (non-plan use).
    let out_names = names(&out_of_plan);
    assert_eq!(out_names.len(), defs.len());
    assert!(out_names.contains(&"ask_user_question"));
}
#[test]
fn plan_mode_blocked_ask_user_name_matcher() {
    assert!(is_plan_mode_blocked_ask_user_tool_name("ask_user_question"));
    assert!(is_plan_mode_blocked_ask_user_tool_name("AskUserQuestion"));
    assert!(is_plan_mode_blocked_ask_user_tool_name("AskUser"));
    assert!(!is_plan_mode_blocked_ask_user_tool_name("exit_plan_mode"));
    assert!(!is_plan_mode_blocked_ask_user_tool_name("AskQuestion"));
    assert!(!is_plan_mode_blocked_ask_user_tool_name("read_file"));
}

/// Context-only must advertise no tools (the redteam point: the model works
/// from context and instructions only). Plan and normal still keep tools.
#[test]
fn context_only_advertises_no_tools() {
    let defs = vec![
        fn_def("read_file"),
        fn_def("run_terminal_command"),
        fn_def("search_replace"),
        fn_def("spawn_subagent"),
        fn_def("exit_plan_mode"),
    ];
    let empty = advertise_tools_for_turn(defs.clone(), false, true);
    assert!(
        empty.is_empty(),
        "context-only must send an empty tool list, got {empty:?}"
    );
    let empty_in_plan = advertise_tools_for_turn(defs.clone(), true, true);
    assert!(
        empty_in_plan.is_empty(),
        "context-only wins over plan-mode filtering"
    );
    let normal = advertise_tools_for_turn(defs.clone(), false, false);
    assert_eq!(names(&normal).len(), defs.len());
    let in_plan = advertise_tools_for_turn(
        vec![fn_def("read_file"), fn_def("ask_user_question")],
        true,
        false,
    );
    let in_names = names(&in_plan);
    assert!(in_names.contains(&"read_file"));
    assert!(!in_names.contains(&"ask_user_question"));
}

#[test]
fn context_only_strips_hosted_tools() {
    let hosted = vec!["web_search", "code_execution"];
    assert!(advertise_hosted_tools_for_turn(hosted.clone(), true).is_empty());
    assert_eq!(advertise_hosted_tools_for_turn(hosted, false).len(), 2);
}

#[test]
fn context_only_refuses_tool_calls_without_executing() {
    assert!(should_refuse_tool_in_context_only(true));
    assert!(!should_refuse_tool_in_context_only(false));
    let msg = context_only_tool_refusal_message();
    assert!(msg.contains("context-only"));
    assert!(msg.contains("no tools"));
}

/// Context-only plus a JSON schema still advertises no tools (no StructuredOutput).
#[test]
fn context_only_structured_output_turn_advertises_no_tools() {
    let base = vec![ToolSpec {
        name: "read_file".into(),
        description: None,
        parameters: serde_json::json!({"type": "object"}),
    }];
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "ok": { "type": "boolean" } }
    });
    let tools = effective_tools_for_turn(base.clone(), true, Some(schema.clone()));
    assert!(
        tools.is_empty(),
        "context-only + JSON schema must keep an empty tool list, got {tools:?}"
    );
    assert!(
        !tools.iter().any(|t| t.name == STRUCTURED_OUTPUT_TOOL),
        "must not push StructuredOutput in context-only"
    );
    let with_schema = effective_tools_for_turn(base, false, Some(schema));
    assert!(
        with_schema.iter().any(|t| t.name == STRUCTURED_OUTPUT_TOOL),
        "without context-only, structured-output is advertised"
    );
}
/// Pins the `reconcile_plan_mode_with_prompt` transitions:
/// Plan → Pending, idempotent, non-plan modes exit cleanly.
#[test]
fn prompt_mode_plan_drives_tracker_into_pending_when_inactive() {
    use crate::session::plan_mode::PlanModeTracker;
    use std::path::PathBuf;
    fn reconcile(tracker: &mut PlanModeTracker, mode: PromptMode) {
        match mode {
            PromptMode::Plan => {
                tracker.enter_pending();
            }
            PromptMode::Agent | PromptMode::Ask => {
                if tracker.state() != PlanModeState::Inactive {
                    tracker.user_exit(false);
                }
            }
        }
    }
    let mut tracker = PlanModeTracker::new(PathBuf::from("/tmp/test"));
    assert_eq!(tracker.state(), PlanModeState::Inactive);
    reconcile(&mut tracker, PromptMode::Plan);
    assert_eq!(tracker.state(), PlanModeState::Pending);
    reconcile(&mut tracker, PromptMode::Plan);
    assert_eq!(tracker.state(), PlanModeState::Pending);
    reconcile(&mut tracker, PromptMode::Agent);
    assert_eq!(tracker.state(), PlanModeState::Inactive);
    reconcile(&mut tracker, PromptMode::Plan);
    assert_eq!(tracker.state(), PlanModeState::Pending);
    reconcile(&mut tracker, PromptMode::Ask);
    assert_eq!(tracker.state(), PlanModeState::Inactive);
}
#[test]
fn session_mode_id_from_prompt_mode_inverts_the_parse() {
    for id in ["plan", "ask", "default"] {
        let mode_id = acp::SessionModeId::new(id);
        let round_tripped =
            session_mode_id_from_prompt_mode(prompt_mode_from_session_mode_id(&mode_id));
        assert_eq!(round_tripped.0.as_ref(), id);
    }
}
/// A prompt that declares `_meta.mode` is the client changing mode, and the
/// client has to be told it took effect. Both arms used to persist the
/// transition and inject the model's reminder but emit nothing — so a client
/// that carries its mode on the prompt could enter or leave plan mode with no
/// signal at all, and `updates.jsonl` carried no mode line for replay either.
#[tokio::test]
async fn a_declared_mode_change_is_published_to_the_client() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut event_rx) = actor_with_events().await;
            actor.reconcile_plan_mode_with_prompt(PromptMode::Plan);
            actor.reconcile_plan_mode_with_prompt(PromptMode::Agent);
            assert_eq!(actor.plan_mode.lock().state(), PlanModeState::Inactive);
            assert_eq!(
                mode_updates(&mut event_rx),
                vec!["plan".to_string(), "default".to_string()],
            );
        })
        .await;
}
/// `ask` is its own client-facing mode, so leaving plan for it must not report
/// `default`.
#[tokio::test]
async fn leaving_plan_for_ask_reports_ask() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut event_rx) = actor_with_events().await;
            actor.reconcile_plan_mode_with_prompt(PromptMode::Plan);
            actor.reconcile_plan_mode_with_prompt(PromptMode::Ask);
            assert_eq!(
                mode_updates(&mut event_rx),
                vec!["plan".to_string(), "ask".to_string()],
            );
        })
        .await;
}
/// Re-declaring the mode already in effect is not a mode change. A client that
/// mirrors the session's mode back onto every prompt would otherwise emit one
/// `CurrentModeUpdate` per turn.
#[tokio::test]
async fn redeclaring_the_mode_already_in_effect_publishes_nothing() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut event_rx) = actor_with_events().await;
            actor.reconcile_plan_mode_with_prompt(PromptMode::Plan);
            assert_eq!(mode_updates(&mut event_rx), vec!["plan".to_string()]);
            actor.reconcile_plan_mode_with_prompt(PromptMode::Plan);
            actor.reconcile_plan_mode_with_prompt(PromptMode::Plan);
            assert!(mode_updates(&mut event_rx).is_empty());
            actor.reconcile_plan_mode_with_prompt(PromptMode::Agent);
            assert_eq!(mode_updates(&mut event_rx), vec!["default".to_string()]);
            actor.reconcile_plan_mode_with_prompt(PromptMode::Agent);
            assert!(mode_updates(&mut event_rx).is_empty());
        })
        .await;
}
/// A synthetic turn — a background task wake, a goal summary, a notification
/// drain — declares no mode; it is constructed with a placeholder `Agent`.
/// Treating that placeholder as a declaration ended plan mode just by waking
/// the session, and silently: nothing was emitted, so the indicator stayed lit
/// for the rest of the session while the agent was back in agent mode.
#[tokio::test]
async fn a_synthetic_turn_inherits_plan_mode_instead_of_ending_it() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut event_rx) = actor_with_events().await;
            actor.reconcile_plan_mode_with_prompt(PromptMode::Plan);
            actor.plan_mode.lock().activate();
            let _ = mode_updates(&mut event_rx);
            for prompt_id in [
                "task-completed-abc",
                "subagent-completed-abc",
                "workflow-completed-abc",
                "notifications-1",
                "goal-summary-1",
                "goal-classifier-nudge-1",
                "scheduler-fired-1",
                "plan-resume-1",
            ] {
                let origin = crate::session::PromptOrigin::from_prompt_id(prompt_id);
                let resolved = actor.resolve_turn_prompt_mode(&origin, PromptMode::Agent);
                assert_eq!(
                    actor.plan_mode.lock().state(),
                    PlanModeState::Active,
                    "{prompt_id} must not end plan mode"
                );
                assert_eq!(
                    resolved,
                    PromptMode::Plan,
                    "{prompt_id} runs under the session's mode, so it is recorded under it too"
                );
                assert!(
                    mode_updates(&mut event_rx).is_empty(),
                    "{prompt_id} changed no mode, so it must announce none"
                );
            }
        })
        .await;
}
/// The other half of the same rule: a real user turn still applies what it
/// declared, and the resolved mode is what it asked for.
#[tokio::test]
async fn a_user_turn_still_applies_its_declared_mode() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut event_rx) = actor_with_events().await;
            let origin = crate::session::PromptOrigin::from_prompt_id("prompt-1");
            assert!(!origin.is_synthetic(), "precondition");
            let resolved = actor.resolve_turn_prompt_mode(&origin, PromptMode::Plan);
            assert_eq!(resolved, PromptMode::Plan);
            assert_eq!(actor.plan_mode.lock().state(), PlanModeState::Pending);
            assert_eq!(mode_updates(&mut event_rx), vec!["plan".to_string()]);
        })
        .await;
}
