//! Resume re-park of the `exit_plan_mode` approval + the mid-turn
//! disconnect handling.
//!
//! On resume the shell re-issues the `x.ai/exit_plan_mode` reverse-request when
//! `awaiting_plan_approval` was persisted, recreating a real live waiter so the
//! pager's existing approve/revise/abandon path works unchanged. These tests
//! pin the reverse-request shape, the awaiting-bit lifecycle, and the mid-turn
//! disconnect path — a graceful client disconnect must NOT auto-approve.

use super::support::*;
use super::*;

/// Build the typed approval response the pager would send back.
fn ext_response(outcome: &str) -> Arc<serde_json::value::RawValue> {
    serde_json::value::to_raw_value(&serde_json::json!({ "outcome": outcome }))
        .unwrap()
        .into()
}

/// Actor with both gateway and persistence receivers retained (the shared
/// `build_actor` drops persistence).
async fn actor_with_channels() -> (
    std::sync::Arc<SessionActor>,
    tokio::sync::mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>,
    tokio::sync::mpsc::UnboundedReceiver<PersistenceMsg>,
) {
    let (gateway_tx, gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, persistence_rx) = tokio::sync::mpsc::unbounded_channel();
    let (actor, _ev) = create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
    (std::sync::Arc::new(actor), gateway_rx, persistence_rx)
}

/// Latest persisted `awaiting_plan_approval` value, or `None` if plan-mode state
/// was never persisted.
fn last_persisted_awaiting(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<PersistenceMsg>,
) -> Option<bool> {
    let mut last = None;
    while let Ok(msg) = rx.try_recv() {
        if let PersistenceMsg::PlanModeState(snapshot) = msg {
            last = Some(snapshot.awaiting_plan_approval);
        }
    }
    last
}

#[tokio::test(flavor = "current_thread")]
async fn request_plan_approval_issues_reverse_request_and_clears_flag() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx) = build_actor().await;

            // Stand in for the pager: answer the exit_plan_mode reverse-request
            // with "approved"; ack the fire-and-forget pending broadcasts.
            let responder = tokio::task::spawn_local(async move {
                let mut seen_method = None;
                let mut seen_session = None;
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            seen_method = Some(args.request.method.to_string());
                            let req: serde_json::Value =
                                serde_json::from_str(args.request.params.get()).unwrap();
                            seen_session = req
                                .get("sessionId")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            let _ = args
                                .response_tx
                                .send(Ok(acp::ExtResponse::new(ext_response("approved"))));
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
                (seen_method, seen_session)
            });

            let tool_call_id = acp::ToolCallId::new(Arc::from("tc-resume"));
            let parsed = actor
                .request_plan_approval(&tool_call_id, Some("# Plan".into()))
                .await
                .expect("approval round-trip should succeed");

            assert_eq!(parsed.outcome, "approved");
            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "awaiting flag must clear once the approval is answered"
            );

            let (method, session_id) = responder.await.unwrap();
            assert_eq!(method.as_deref(), Some("x.ai/exit_plan_mode"));
            assert_eq!(
                session_id.as_deref(),
                Some("test-actor"),
                "reverse-request must carry a non-empty sessionId (design §5.4)"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn request_plan_approval_clears_flag_on_request_changes() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx) = build_actor().await;

            let responder = tokio::task::spawn_local(async move {
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            let _ = args
                                .response_tx
                                .send(Ok(acp::ExtResponse::new(ext_response("cancelled"))));
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
            });

            let tool_call_id = acp::ToolCallId::new(Arc::from("tc-resume-revise"));
            let parsed = actor
                .request_plan_approval(&tool_call_id, Some("# Plan".into()))
                .await
                .expect("approval round-trip should succeed");

            assert_eq!(parsed.outcome, "cancelled");
            // Request-changes leaves plan mode active but never strands the bit.
            assert!(!actor.plan_mode.lock().is_awaiting_plan_approval());
            responder.await.unwrap();
        })
        .await;
}

/// An unparseable approval response must fail CLOSED to `"cancelled"` (stay in
/// plan mode), never fall open to `"approved"`.
#[tokio::test(flavor = "current_thread")]
async fn request_plan_approval_parse_fallback_fails_closed() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx) = build_actor().await;

            let responder = tokio::task::spawn_local(async move {
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            // Garbage payload that does not deserialize to ExitPlanModeExtResponse.
                            let garbage: Arc<serde_json::value::RawValue> =
                                serde_json::value::to_raw_value(&serde_json::json!(
                                    "not-an-object"
                                ))
                                .unwrap()
                                .into();
                            let _ = args.response_tx.send(Ok(acp::ExtResponse::new(garbage)));
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
            });

            let tool_call_id = acp::ToolCallId::new(Arc::from("tc-garbage"));
            let parsed = actor
                .request_plan_approval(&tool_call_id, Some("# Plan".into()))
                .await
                .expect("round-trip returns Ok even for a garbage payload");
            assert_eq!(parsed.outcome, "cancelled");
            responder.await.unwrap();
        })
        .await;
}

/// REAL park (not seeded): drive a genuine `exit_plan_mode` tool call through
/// `prepare_tool_call`, simulate the client disconnecting mid-approval, and
/// assert the tool is NOT auto-executed, plan mode stays Active, and
/// `awaiting_plan_approval=true` is PERSISTED (what would land in
/// `plan_mode.json`) so a fresh resume re-parks.
#[tokio::test(flavor = "current_thread")]
async fn real_exit_plan_mode_disconnect_keeps_awaiting_persisted() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx, mut persistence_rx) = actor_with_channels().await;
            // Real enter/exit_plan_mode tools so prepare_tool_call parses a genuine call.
            *actor.agent.borrow_mut() = test_agent_with_plan_tools().await;
            // Plan mode Active with a real plan.md at the tracker path.
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("plan.md"), "# Plan\n- step 1\n").unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
                tracker.activate_from_tool();
            }

            // Pager receives the gate then disconnects (drops the request).
            let responder = tokio::task::spawn_local(async move {
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            drop(args); // no response -> "unable to receive response"
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
            });

            let call = crate::sampling::types::ToolCallResponse {
                id: "call-exit".to_string(),
                kind: "function".to_string(),
                function: crate::sampling::types::ToolCallFunction::new("exit_plan_mode", "{}"),
            };
            let mut deferred = Vec::new();
            let outcome = actor
                .prepare_tool_call(call, &mut deferred)
                .await
                .expect("prepare_tool_call should not error");

            // Disconnect must NOT auto-approve: the tool is not prepared/executed.
            match outcome {
                Err(ToolLoop::Cancelled) => {}
                other => panic!("expected ToolLoop::Cancelled on disconnect, got {other:?}"),
            }
            assert!(
                actor.plan_mode.lock().is_active(),
                "plan mode must remain Active after a disconnect (no auto-approve)"
            );
            assert!(
                actor.plan_mode.lock().is_awaiting_plan_approval(),
                "awaiting flag must remain set after a disconnect"
            );
            assert_eq!(
                last_persisted_awaiting(&mut persistence_rx),
                Some(true),
                "plan_mode.json must persist awaiting_plan_approval=true after disconnect"
            );
            responder.await.unwrap();
        })
        .await;
}

/// Headless / no UI client wired: reverse-request enqueue fails ("unable to
/// send"). Product leaves plan mode with an honest no-panel tool result — not
/// a plan-panel Approve click, not always-approve plan auto-approve, and not
/// a fall-through that runs the bare `exit_plan_mode` tool body (that body is
/// present-only and must not claim approval).
#[tokio::test(flavor = "current_thread")]
async fn real_exit_plan_mode_no_client_executes_tool() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, gateway_rx, _persistence_rx) = actor_with_channels().await;
            // Drop the gateway receiver: the reverse-request enqueue fails
            // ("unable to send"), the headless branch.
            drop(gateway_rx);
            *actor.agent.borrow_mut() = test_agent_with_plan_tools().await;
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("plan.md"), "# Plan\n- step 1\n").unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
                tracker.activate_from_tool();
            }

            let call = crate::sampling::types::ToolCallResponse {
                id: "call-exit-headless".to_string(),
                kind: "function".to_string(),
                function: crate::sampling::types::ToolCallFunction::new("exit_plan_mode", "{}"),
            };
            let mut deferred = Vec::new();
            let outcome = actor
                .prepare_tool_call(call, &mut deferred)
                .await
                .expect("prepare_tool_call should not error");
            // Headless intercept completes in prepare (no tool body fall-through).
            match outcome {
                Err(ToolLoop::Continue) => {}
                other => panic!(
                    "headless exit_plan_mode must complete as ToolLoop::Continue \
                     (honest no-client leave), got {other:?}"
                ),
            }
            assert!(
                !actor.plan_mode.lock().is_active(),
                "no-client path must leave plan mode"
            );
            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "no-client leave must clear awaiting_plan_approval"
            );

            let conv = actor.chat_state_handle.get_conversation().await;
            let exit_text = conv
                .iter()
                .rev()
                .find_map(|item| match item {
                    xai_grok_sampling_types::ConversationItem::ToolResult(tr)
                        if tr.tool_call_id == "call-exit-headless" =>
                    {
                        Some(tr.content.to_string())
                    }
                    _ => None,
                })
                .expect("no-client path must push a tool_result");
            assert!(
                exit_text.contains("No interactive plan panel"),
                "message must name the no-panel path: {exit_text:?}"
            );
            assert!(
                exit_text.contains("NOT a plan-panel Approve"),
                "message must deny plan-panel Approve: {exit_text:?}"
            );
            assert!(
                !exit_text.to_lowercase().contains("has been approved"),
                "must not claim operator approval: {exit_text:?}"
            );
            assert!(
                !exit_text.to_lowercase().contains("start coding"),
                "must not tell the model to start coding from present alone: {exit_text:?}"
            );
            assert!(
                exit_text.contains("# Plan") || exit_text.contains("step 1"),
                "no-client message should embed plan body when present: {exit_text:?}"
            );
        })
        .await;
}

/// A quit-while-parked (disconnect) keeps `awaiting_plan_approval` set so the
/// next resume re-parks the gate. Not seeded: `request_plan_approval` sets the
/// bit itself.
#[tokio::test(flavor = "current_thread")]
async fn request_plan_approval_keeps_flag_when_client_disconnects() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx, mut persistence_rx) = actor_with_channels().await;

            let responder = tokio::task::spawn_local(async move {
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            drop(args); // no response -> ext_method sees Err
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
            });

            let tool_call_id = acp::ToolCallId::new(Arc::from("tc-resume-quit"));
            let result = actor
                .request_plan_approval(&tool_call_id, Some("# Plan".into()))
                .await;

            assert!(result.is_err(), "disconnect should surface as an error");
            assert!(
                actor.plan_mode.lock().is_awaiting_plan_approval(),
                "awaiting flag must survive a client disconnect"
            );
            assert_eq!(
                last_persisted_awaiting(&mut persistence_rx),
                Some(true),
                "the parked approval must be persisted as awaiting=true"
            );
            responder.await.unwrap();
        })
        .await;
}

/// Dropping the `request_plan_approval` future mid-await (the turn-cancel path)
/// must clear the awaiting bit via `AwaitingApprovalGuard`.
#[tokio::test(flavor = "current_thread")]
async fn request_plan_approval_future_drop_clears_flag() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx) = build_actor().await;

            // Pager receives the request but never answers (keeps the parked
            // await pending) so we can drop the future while it is in flight.
            let responder = tokio::task::spawn_local(async move {
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            // Hold the response sender open: never resolve.
                            std::mem::forget(args);
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
            });

            let tool_call_id = acp::ToolCallId::new(Arc::from("tc-drop"));
            let mut fut =
                Box::pin(actor.request_plan_approval(&tool_call_id, Some("# Plan".into())));
            // Poll until the request is parked (awaiting flag set), then drop.
            tokio::select! {
                _ = &mut fut => panic!("request should still be parked (no answer)"),
                _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
            }
            assert!(actor.plan_mode.lock().is_awaiting_plan_approval());
            drop(fut); // turn cancelled -> guard runs

            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "dropping the parked future must clear the awaiting bit"
            );
            responder.abort();
        })
        .await;
}

/// Queued prompts must not promote while plan approval is awaiting — that
/// would start a competing turn and hijack the in-flight exit_plan_mode
/// decision (resume re-park has no running_task).
#[tokio::test(flavor = "current_thread")]
async fn maybe_start_blocked_while_awaiting_plan_approval() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _gateway_rx, _persistence_rx) = actor_with_channels().await;
            actor.plan_mode.lock().set_awaiting_plan_approval(true);

            let (item, mut respond_rx) = user_item_with_rx("pending-plan", "client");
            {
                let mut state = actor.state.lock().await;
                assert!(state.running_task.is_none(), "fixture starts idle");
                state.pending_inputs.push_back(item);
            }

            let (completion_tx, _completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().maybe_start_running_task(completion_tx).await;

            {
                let state = actor.state.lock().await;
                assert!(
                    state.running_task.is_none(),
                    "must not start a turn while awaiting plan approval"
                );
                assert_eq!(
                    state.pending_inputs.len(),
                    1,
                    "queued prompt must stay pending"
                );
            }
            // Respond channel still open — prompt was not promoted/removed.
            assert!(
                respond_rx.try_recv().is_err(),
                "held prompt must not resolve its RPC"
            );

            // Clear the gate and start succeeds.
            actor.plan_mode.lock().set_awaiting_plan_approval(false);
            let (completion_tx, _completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().maybe_start_running_task(completion_tx).await;
            {
                let state = actor.state.lock().await;
                assert!(
                    state.running_task.is_some(),
                    "after approval gate clears, queue must promote"
                );
            }
        })
        .await;
}

/// Resident reconnect: in-memory awaiting is false, but `plan_mode.json`
/// on disk was parked after the client left. Resume must re-issue the
/// reverse-request so `/view-plan` can bind Approve.
#[tokio::test(flavor = "current_thread")]
async fn resume_adopts_disk_awaiting_when_memory_is_clear() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx, _persistence_rx) = actor_with_channels().await;
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("plan.md"), "# Seeded after quit\n\nBody\n").unwrap();
            std::fs::write(
                dir.path().join("plan_mode.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "state": "Active",
                    "was_previously_active": true,
                    "reminder_count": 0,
                    "pending_exit_reminder": false,
                    "awaiting_plan_approval": true,
                }))
                .unwrap(),
            )
            .unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
                assert!(
                    !tracker.is_awaiting_plan_approval(),
                    "fixture: resident memory never parked"
                );
            }

            let responder = tokio::task::spawn_local(async move {
                let mut seen = false;
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            seen = &*args.request.method == "x.ai/exit_plan_mode";
                            let _ = args
                                .response_tx
                                .send(Ok(acp::ExtResponse::new(ext_response("abandoned"))));
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
                seen
            });

            let (completion_tx, _completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().resume_plan_approval(completion_tx).await;
            assert!(
                responder.await.unwrap(),
                "resume must re-issue x.ai/exit_plan_mode after adopting disk awaiting"
            );
            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "abandoned decision must clear the adopted awaiting flag"
            );
        })
        .await;
}

/// Reconnect flush can persist Inactive over a seeded `plan_mode.json`.
/// `RestorePlanApproval` still carries the pre-flush snapshot.
#[tokio::test(flavor = "current_thread")]
async fn resume_uses_pre_flush_snapshot_when_disk_was_clobbered() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx, _persistence_rx) = actor_with_channels().await;
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("plan.md"), "# Seeded after quit\n\nBody\n").unwrap();
            std::fs::write(
                dir.path().join("plan_mode.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "state": "Inactive",
                    "was_previously_active": true,
                    "reminder_count": 0,
                    "pending_exit_reminder": false,
                    "awaiting_plan_approval": false,
                }))
                .unwrap(),
            )
            .unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
            }

            let responder = tokio::task::spawn_local(async move {
                let mut seen = false;
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            seen = &*args.request.method == "x.ai/exit_plan_mode";
                            let _ = args
                                .response_tx
                                .send(Ok(acp::ExtResponse::new(ext_response("abandoned"))));
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
                seen
            });

            actor.adopt_parked_plan_snapshot(crate::session::plan_mode::PlanModeSnapshot {
                state: crate::session::plan_mode::PlanModeState::Active,
                was_previously_active: true,
                reminder_count: 0,
                pending_exit_reminder: false,
                awaiting_plan_approval: true,
                plan_decision_resolved: false,
            });
            let (completion_tx, _completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().resume_plan_approval(completion_tx).await;
            assert!(
                responder.await.unwrap(),
                "resume must re-issue x.ai/exit_plan_mode from the pre-flush snapshot"
            );
            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "abandoned decision must clear the adopted awaiting flag"
            );
        })
        .await;
}

/// Resume after Approve/Quit must not re-issue the reverse-request.
#[tokio::test(flavor = "current_thread")]
async fn resume_skips_when_plan_decision_resolved() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx, _persistence_rx) = actor_with_channels().await;
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("plan.md"), "# Leftover\n\nBody\n").unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
                tracker.activate_from_tool();
                tracker.set_awaiting_plan_approval(true);
                let mut snap = tracker.snapshot();
                snap.plan_decision_resolved = true;
                *tracker = crate::session::plan_mode::PlanModeTracker::from_snapshot(
                    dir.path().to_path_buf(),
                    snap,
                );
            }

            let (completion_tx, _completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().resume_plan_approval(completion_tx).await;

            assert!(
                actor.plan_mode.lock().is_plan_decision_resolved(),
                "resolved sticky must survive resume"
            );
            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "resume must not leave awaiting armed after a decided plan"
            );
            assert!(
                gateway_rx.try_recv().is_err(),
                "no reverse-request should be sent when the plan is already decided"
            );
        })
        .await;
}

/// A first reverse-request that fails to reach the pager must retry after bind.
/// The second round-trip uses abandoned so this test does not start an
/// implement turn.
#[tokio::test(flavor = "current_thread")]
async fn resume_retries_reverse_request_after_bind_failure() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx, _persistence_rx) = actor_with_channels().await;
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("plan.md"), "# Retry\n\nBody\n").unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
                tracker.activate_from_tool();
                tracker.set_awaiting_plan_approval(true);
            }

            let responder = tokio::task::spawn_local(async move {
                let mut ext_seen = 0usize;
                while let Some(msg) = gateway_rx.recv().await {
                    match msg {
                        xai_acp_lib::AcpClientMessage::ExtMethod(args) => {
                            ext_seen += 1;
                            if ext_seen == 1 {
                                drop(args);
                                continue;
                            }
                            let _ = args
                                .response_tx
                                .send(Ok(acp::ExtResponse::new(ext_response("abandoned"))));
                            break;
                        }
                        xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        _ => {}
                    }
                }
                ext_seen
            });

            let (completion_tx, _completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().resume_plan_approval(completion_tx).await;
            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "retry decision must clear the awaiting flag"
            );
            assert_eq!(
                responder.await.unwrap(),
                2,
                "resume must retry the reverse-request after the first send fails"
            );
        })
        .await;
}

/// Resume with the flag set but no `plan.md` on disk: clear the bit and issue NO
/// reverse-request.
#[tokio::test(flavor = "current_thread")]
async fn resume_no_plan_md_clears_flag_without_request() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, mut gateway_rx, _persistence_rx) = actor_with_channels().await;
            // Point the tracker at an empty dir (no plan.md) and arm the flag.
            let dir = tempfile::tempdir().unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
                tracker.activate_from_tool();
                tracker.set_awaiting_plan_approval(true);
            }

            let (completion_tx, _completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().resume_plan_approval(completion_tx).await;

            assert!(
                !actor.plan_mode.lock().is_awaiting_plan_approval(),
                "missing plan.md must clear the stuck awaiting bit"
            );
            assert!(
                gateway_rx.try_recv().is_err(),
                "no reverse-request should be sent when plan.md is missing"
            );
        })
        .await;
}

/// Named contract (dogfood 2026-08-09): same-turn rewrite of session `plan.md`
/// plus `exit_plan_mode` must return the **post-write** body. Without ordering,
/// prepare parks/re-reads while the co-batched write is only prepared, so the
/// tool result freezes plan A ("Deploy automation") after the agent already
/// wrote plan B (secrets).
#[tokio::test(flavor = "current_thread")]
async fn same_batch_plan_write_before_exit_plan_mode_returns_new_body() {
    use xai_grok_tools::implementations::grok_build::enter_plan_mode::EnterPlanModeTool;
    use xai_grok_tools::implementations::grok_build::exit_plan_mode::ExitPlanModeTool;
    use xai_grok_tools::registry::types::ToolConfig;
    use xai_grok_tools::types::resources::PlanFilePath;

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            // Headless: no client (drop gateway) so exit falls through without
            // waiting on approve — still must see the co-batched write first.
            let (actor, gateway_rx, _persistence_rx) = actor_with_channels().await;
            drop(gateway_rx);

            *actor.agent.borrow_mut() = test_agent_with_tools(vec![
                ToolConfig::from_id("GrokBuild:read_file"),
                ToolConfig::from_id("GrokBuild:search_replace"),
                ToolConfig::for_tool::<EnterPlanModeTool>(),
                ToolConfig::for_tool::<ExitPlanModeTool>(),
            ])
            .await;

            let dir = tempfile::tempdir().unwrap();
            let plan_path = dir.path().join("plan.md");
            let content_a = "# Plan A\nold_token_economy_marker\n";
            let content_b = "# Plan B\nsurmount_team_usage_first\n";
            std::fs::write(&plan_path, content_a).unwrap();
            {
                let mut tracker = actor.plan_mode.lock();
                *tracker =
                    crate::session::plan_mode::PlanModeTracker::new(dir.path().to_path_buf());
                tracker.activate_from_tool();
            }
            actor
                .agent
                .borrow()
                .tool_bridge()
                .update_resource(PlanFilePath(plan_path.clone()))
                .await;

            actor
                .workspace_ops
                .bind_local_session(
                    &actor.session_id_string(),
                    actor.tool_context.cwd.as_path().to_path_buf(),
                    actor.tool_context.hunk_tracker_handle.clone(),
                    actor.agent.borrow().tool_bridge().toolset(),
                    None,
                )
                .expect("bind_local_session");

            // search_replace needs skip_read or a prior read; use empty
            // old_string only works for new files. Replace the full A body.
            let write_args = serde_json::json!({
                "file_path": plan_path.to_string_lossy(),
                "old_string": content_a,
                "new_string": content_b,
            })
            .to_string();
            let write_call = crate::sampling::types::ToolCallResponse {
                id: "call-write-plan".to_string(),
                kind: "function".to_string(),
                function: crate::sampling::types::ToolCallFunction::new(
                    "search_replace",
                    write_args,
                ),
            };
            let exit_call = crate::sampling::types::ToolCallResponse {
                id: "call-exit-after-write".to_string(),
                kind: "function".to_string(),
                function: crate::sampling::types::ToolCallFunction::new("exit_plan_mode", "{}"),
            };

            actor
                .execute_tool_calls(vec![write_call, exit_call])
                .await
                .expect("execute_tool_calls");

            let on_disk = std::fs::read_to_string(&plan_path).expect("plan.md readable");
            assert!(
                on_disk.contains("surmount_team_usage_first"),
                "co-batched write must land plan B on disk; got {on_disk:?}"
            );

            let conv = actor.chat_state_handle.get_conversation().await;
            let exit_text = conv
                .iter()
                .rev()
                .find_map(|item| match item {
                    xai_grok_sampling_types::ConversationItem::ToolResult(tr)
                        if tr.tool_call_id == "call-exit-after-write" =>
                    {
                        Some(tr.content.to_string())
                    }
                    _ => None,
                })
                .expect("exit_plan_mode tool_result present");
            assert!(
                exit_text.contains("surmount_team_usage_first"),
                "exit_plan_mode must re-read post-write plan B; got {exit_text:?}"
            );
            assert!(
                !exit_text.contains("old_token_economy_marker"),
                "exit_plan_mode must not embed frozen plan A; got {exit_text:?}"
            );
        })
        .await;
}
