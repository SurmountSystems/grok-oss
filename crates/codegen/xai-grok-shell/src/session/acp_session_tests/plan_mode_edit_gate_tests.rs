//! Plan-mode edit gate through the real `prepare_tool_call` path: plan mode
//! is read-only except the plan file in EVERY permission mode. The fixture's
//! `PermissionHandle::allow_all()` is the always-approve worst case — before
//! the gate, it silently approved any edit in plan mode (the "yolo edits in
//! plan mode" bug); these tests pin that the gate rejects
//! BEFORE the permission layer can auto-approve.
use super::support::*;
use super::*;
/// Build an actor whose toolset parses grok `search_replace` plus the plan
/// tools (so `${{ tools.by_kind.exit_plan }}` resolves in the rejection
/// message), with a gateway drain answering session notifications.
async fn build_gate_actor() -> SessionActor {
    use xai_grok_tools::implementations::grok_build::ask_user_question::AskUserQuestionTool;
    use xai_grok_tools::implementations::grok_build::enter_plan_mode::EnterPlanModeTool;
    use xai_grok_tools::implementations::grok_build::exit_plan_mode::ExitPlanModeTool;
    use xai_grok_tools::registry::types::ToolConfig;
    let (gateway_tx, mut gateway_rx) =
        tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
    let (persistence_tx, _persistence_rx) =
        tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
    let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
    *actor.agent.borrow_mut() = test_agent_with_tools(vec![
        // search_replace's requirements demand a Read tool in the same toolset.
        ToolConfig::from_id("GrokBuild:read_file"),
        ToolConfig::from_id("GrokBuild:search_replace"),
        ToolConfig::for_tool::<EnterPlanModeTool>(),
        ToolConfig::for_tool::<ExitPlanModeTool>(),
        // Keep ask_user_question registered so prepare can parse a call even
        // when plan mode would have stripped it from the advertised list —
        // the hard reject path must still fire.
        ToolConfig::for_tool::<AskUserQuestionTool>(),
    ])
    .await;
    tokio::task::spawn_local(async move {
        while let Some(msg) = gateway_rx.recv().await {
            if let xai_acp_lib::AcpClientMessage::SessionNotification(args) = msg {
                let _ = args.response_tx.send(Ok(()));
            }
        }
    });
    actor
}
/// Flip the fixture's tracker to Active (plan file: `/tmp/test-session/plan.md`).
fn activate_plan_mode(actor: &SessionActor) {
    let mut tracker = actor.plan_mode.lock();
    assert!(tracker.enter_pending());
    assert!(tracker.activate());
}
fn search_replace_call(id: &str, path: &str) -> ToolCallResponse {
    ToolCallResponse {
        id: id.to_string(),
        kind: "function".to_string(),
        function: crate::sampling::types::ToolCallFunction::new(
            "search_replace",
            format!(r#"{{"file_path":"{path}","old_string":"a","new_string":"b"}}"#),
        ),
    }
}
async fn prepare(
    actor: &SessionActor,
    call: ToolCallResponse,
) -> Result<PreparedToolCall, ToolLoop> {
    let mut deferred = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        actor.prepare_tool_call(call, &mut deferred),
    )
    .await
    .expect("prepare_tool_call must not hang (a hang means a permission prompt was issued)")
    .expect("prepare_tool_call must not error")
}
/// Last tool_result pushed for `call_id`, or panic.
async fn tool_result_text(actor: &SessionActor, call_id: &str) -> String {
    let conv = actor.chat_state_handle.get_conversation().await;
    conv.iter()
        .rev()
        .find_map(|item| match item {
            xai_grok_sampling_types::ConversationItem::ToolResult(tr)
                if tr.tool_call_id == call_id =>
            {
                Some(tr.content.to_string())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no tool_result for {call_id} in {conv:?}"))
}
/// The headline: plan mode Active + allow-all permissions (the always-approve
/// worst case) still rejects a grok edit outside the plan file, without ever
/// reaching the permission layer, and steers the model to `exit_plan_mode`.
#[tokio::test(flavor = "current_thread")]
async fn plan_mode_rejects_grok_edit_outside_plan_file_despite_allow_all_permissions() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let actor = build_gate_actor().await;
            activate_plan_mode(&actor);
            let result =
                prepare(&actor, search_replace_call("call_gate", "/tmp/src/main.rs")).await;
            assert!(
                matches!(result, Err(ToolLoop::Continue)),
                "gate must reject with Continue (tool not executed); got {result:?}"
            );
            let text = tool_result_text(&actor, "call_gate").await;
            assert!(
                text.contains("Rejected: file edits are not allowed in plan mode"),
                "rejection text: {text}"
            );
            assert!(
                text.contains("/tmp/test-session/plan.md"),
                "must name the plan file so the model knows the one editable path: {text}"
            );
            assert!(
                !text.contains("exit_plan_mode"),
                "rejection should stay short (no exit-tool steering): {text}"
            );
        })
        .await;
}
/// The carve-out: the plan file itself prepares cleanly (the gate defers to
/// `should_auto_approve_edit`, the same predicate as the permission bypass).
#[tokio::test(flavor = "current_thread")]
async fn plan_mode_allows_plan_file_edit() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let actor = build_gate_actor().await;
            activate_plan_mode(&actor);
            let result = prepare(
                &actor,
                search_replace_call("call_plan_file", "/tmp/test-session/plan.md"),
            )
            .await;
            assert!(
                result.is_ok(),
                "plan-file edit must pass the gate and prepare; got {:?}",
                result.err()
            );
        })
        .await;
}
/// Control: with plan mode inactive the same edit prepares cleanly — the gate
/// is plan-scoped, not a general edit block.
#[tokio::test(flavor = "current_thread")]
async fn inactive_plan_mode_does_not_gate_edits() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let actor = build_gate_actor().await;
            let result = prepare(
                &actor,
                search_replace_call("call_no_plan", "/tmp/src/main.rs"),
            )
            .await;
            assert!(
                result.is_ok(),
                "edit outside plan mode must prepare; got {:?}",
                result.err()
            );
        })
        .await;
}

/// Context-only refuse is on the live tool loop: a search_replace call does
/// not execute (file contents stay put) and the result is Continue + refusal.
#[tokio::test(flavor = "current_thread")]
async fn context_only_tool_loop_refuses_without_executing() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("marker.rs");
            std::fs::write(&path, "a").expect("write marker");
            let path_str = path.to_string_lossy().into_owned();
            let actor = build_gate_actor().await;
            actor
                .context_only
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let loop_result = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                actor.execute_tool_calls(vec![search_replace_call("call_co", &path_str)]),
            )
            .await
            .expect("execute_tool_calls must not hang")
            .expect("execute_tool_calls must not error");
            assert!(
                matches!(loop_result, ToolLoop::Continue),
                "context-only refuse must Continue without executing; got {loop_result:?}"
            );
            let text = tool_result_text(&actor, "call_co").await;
            assert!(text.contains("context-only"), "refusal text: {text}");
            assert!(text.contains("no tools"), "refusal text: {text}");
            let after = std::fs::read_to_string(&path).expect("read marker");
            assert_eq!(
                after, "a",
                "search_replace must not execute in context-only (invoke count 0)"
            );
        })
        .await;
}

fn ask_user_question_call(id: &str) -> ToolCallResponse {
    ToolCallResponse {
        id: id.to_string(),
        kind: "function".to_string(),
        function: crate::sampling::types::ToolCallFunction::new(
            "ask_user_question",
            r#"{"questions":[{"question":"Which follow-ups?","options":[{"label":"A","description":"option a"}]}]}"#,
        ),
    }
}

/// Hard block: plan mode Active rejects ask_user_question before the client
/// questionnaire UI opens (even if the tool is still registered/callable).
#[tokio::test(flavor = "current_thread")]
async fn plan_mode_rejects_ask_user_question_before_ui() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let actor = build_gate_actor().await;
            activate_plan_mode(&actor);
            let result = prepare(&actor, ask_user_question_call("call_ask")).await;
            assert!(
                matches!(result, Err(ToolLoop::Continue)),
                "ask_user_question must be rejected in plan mode; got {result:?}"
            );
            let text = tool_result_text(&actor, "call_ask").await;
            assert!(
                text.contains("ask_user_question") && text.contains("plan mode"),
                "rejection must name the tool and plan mode: {text}"
            );
            assert!(
                text.contains("plan file") || text.contains("exit_plan_mode"),
                "rejection must steer to plan.md / exit_plan_mode: {text}"
            );
        })
        .await;
}

/// Outside plan mode, ask_user_question still prepares (non-plan interactive Q&A).
/// The plan gate must not reject; tool body runs later at dispatch.
#[tokio::test(flavor = "current_thread")]
async fn inactive_plan_mode_allows_ask_user_question_prepare() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let actor = build_gate_actor().await;
            let result = prepare(&actor, ask_user_question_call("call_ask_ok")).await;
            assert!(
                result.is_ok(),
                "ask_user_question outside plan mode must prepare; got {:?}",
                result.err()
            );
        })
        .await;
}
