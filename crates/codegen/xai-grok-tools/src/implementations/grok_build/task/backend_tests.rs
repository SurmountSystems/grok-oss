use super::*;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Helper: receive the next event, match the expected variant, or panic.
macro_rules! recv_event {
    ($rx:expr, Spawn) => {{
        let event = $rx.recv().await.unwrap();
        match event {
            SubagentEvent::Spawn(inner) => inner,
            _ => panic!("Expected SubagentEvent::Spawn, got different variant"),
        }
    }};
    ($rx:expr, $variant:ident) => {{
        let event = $rx.recv().await.unwrap();
        match event {
            SubagentEvent::$variant(inner) => inner,
            _ => panic!(
                "Expected SubagentEvent::{}, got different variant",
                stringify!($variant)
            ),
        }
    }};
}

#[tokio::test]
async fn channel_backend_spawn_success() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        let req = recv_event!(rx, Spawn);
        assert_eq!(req.request.id, "test-id");
        assert_eq!(req.request.prompt, "do something");
        req.result_tx
            .send(SubagentResult {
                success: true,
                output: Arc::from("done"),
                subagent_id: "test-id".to_string(),
                child_session_id: "test-id".to_string(),
                tool_calls: 3,
                turns: 1,
                duration_ms: 500,
                ..Default::default()
            })
            .unwrap();
    });

    let request = SubagentRequest {
        id: "test-id".to_string(),
        prompt: "do something".to_string(),
        description: "test".to_string(),
        subagent_type: "general-purpose".to_string(),
        parent_session_id: "parent".to_string(),
        parent_prompt_id: None,
        resume_from: None,
        cwd: None,
        runtime_overrides: Default::default(),
        run_in_background: false,
        surface_completion: true,
        await_to_completion: false,
        fork_context: false,
        owner: super::super::types::SubagentOwner::Task,
        implement_loop_effort: None,
        cancel_token: tokio_util::sync::CancellationToken::new(),
    };

    let result = backend.spawn(request).await.unwrap();
    assert!(result.success);
    assert_eq!(result.subagent_id, "test-id");
    assert_eq!(result.tool_calls, 3);

    handle.await.unwrap();
}

#[tokio::test]
async fn spawn_registered_returns_on_admit_before_the_child_finishes() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let request = SubagentRequest {
        id: "l3".to_string(),
        prompt: "do something".to_string(),
        description: "test".to_string(),
        subagent_type: "explore".to_string(),
        parent_session_id: "l2".to_string(),
        parent_prompt_id: None,
        resume_from: None,
        cwd: None,
        runtime_overrides: Default::default(),
        run_in_background: true,
        surface_completion: true,
        await_to_completion: false,
        fork_context: false,
        owner: super::super::types::SubagentOwner::Task,
        implement_loop_effort: None,
        cancel_token: tokio_util::sync::CancellationToken::new(),
    };

    let handle = tokio::spawn(async move { backend.spawn_registered(request).await });
    let req = recv_event!(rx, Spawn);
    let admitted_tx = req
        .admitted_tx
        .expect("background spawn must wait on admit");
    admitted_tx.send(Ok(())).unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(2), handle)
        .await
        .expect(
            "spawn_registered must return once admitted, without waiting for the child to finish",
        )
        .expect("spawn_registered task must not panic")
        .expect("admit Ok is not a tool error");

    req.result_tx
        .send(SubagentResult {
            success: true,
            output: Arc::from("done"),
            subagent_id: "l3".to_string(),
            child_session_id: "l3".to_string(),
            ..Default::default()
        })
        .unwrap();
}

fn background_request(id: &str, description: &str) -> SubagentRequest {
    SubagentRequest {
        id: id.to_string(),
        prompt: description.to_string(),
        description: description.to_string(),
        subagent_type: "general-purpose".to_string(),
        parent_session_id: "parent".to_string(),
        parent_prompt_id: None,
        resume_from: None,
        cwd: None,
        runtime_overrides: Default::default(),
        run_in_background: true,
        surface_completion: true,
        await_to_completion: false,
        fork_context: false,
        owner: super::super::types::SubagentOwner::Task,
        implement_loop_effort: None,
        cancel_token: tokio_util::sync::CancellationToken::new(),
    }
}

/// Operator: parent serializes on `get_command_or_subagent_output` 10-minute
/// waits while mill L2 runs 40+ minutes. Only one subagent row.
///
/// Fire-and-return: A nested L2 that is a long builder (compile, lake, mill)
/// must not occupy the parent as a blocking 10-minute wait loop. Parent starts
/// it, keeps working, completion is a notification. Named test: parent can
/// spawn a second L2 while the first is still running without waiting for the
/// first to exit.
#[tokio::test]
async fn parent_spawn_subagent_second_l2_while_first_still_running_without_wait() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let mill = tokio::spawn({
        let backend = backend.clone();
        async move {
            backend
                .spawn_registered(background_request("mill-l2", "mill compile on nixbuilder"))
                .await
        }
    });
    let mill_req = recv_event!(rx, Spawn);
    mill_req
        .admitted_tx
        .expect("background mill spawn must wait on admit")
        .send(Ok(()))
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), mill)
        .await
        .expect("first spawn_registered must return on admit, without waiting for mill to finish")
        .expect("mill spawn task must not panic")
        .expect("mill admit Ok is not a tool error");

    let mill_notice = xai_tool_types::format_subagent_started_background(
        "mill-l2",
        "general-purpose",
        "mill compile on nixbuilder",
        &xai_tool_types::BackgroundNoticeNaming {
            task_output_tool: "get_command_or_subagent_output",
            ..xai_tool_types::BackgroundNoticeNaming::CANONICAL
        },
        false,
    );
    assert!(
        mill_notice.contains("Keep working")
            && (mill_notice.contains("notification") || mill_notice.contains("notified")),
        "spawn notice must tell the parent to keep working; completion is a notification, got {mill_notice}"
    );
    assert!(
        !mill_notice.contains("When you need its result")
            && !mill_notice.contains("and a positive timeout_ms."),
        "spawn notice must not teach a blocking positive timeout_ms as the default retrieval, got {mill_notice}"
    );

    let second = tokio::spawn({
        let backend = backend.clone();
        async move {
            backend
                .spawn_registered(background_request(
                    "just-module",
                    "next-row just module while mill runs",
                ))
                .await
        }
    });
    let second_req = recv_event!(rx, Spawn);
    second_req
        .admitted_tx
        .expect("second L2 spawn must wait on admit")
        .send(Ok(()))
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), second)
        .await
        .expect(
            "parent must spawn a second L2 while the first is still running, with no blocking wait in between",
        )
        .expect("second spawn task must not panic")
        .expect("second admit Ok is not a tool error");

    assert_eq!(mill_req.request.id, "mill-l2");
    assert_eq!(second_req.request.id, "just-module");
    assert!(
        mill_req
            .result_tx
            .send(SubagentResult {
                success: true,
                output: Arc::from("mill done"),
                subagent_id: "mill-l2".to_string(),
                child_session_id: "mill-l2".to_string(),
                ..Default::default()
            })
            .is_ok(),
        "first L2 must still be running when the second spawn returns"
    );
    let _ = second_req.result_tx.send(SubagentResult {
        success: true,
        output: Arc::from("module done"),
        subagent_id: "just-module".to_string(),
        child_session_id: "just-module".to_string(),
        ..Default::default()
    });
}

#[tokio::test]
async fn channel_backend_spawn_closed_channel() {
    let (tx, rx) = mpsc::unbounded_channel::<SubagentEvent>();
    drop(rx);

    let backend = ChannelBackend::new(tx);

    let request = SubagentRequest {
        id: "test-id".to_string(),
        prompt: "do something".to_string(),
        description: "test".to_string(),
        subagent_type: "general-purpose".to_string(),
        parent_session_id: "parent".to_string(),
        parent_prompt_id: None,
        resume_from: None,
        cwd: None,
        runtime_overrides: Default::default(),
        run_in_background: false,
        surface_completion: true,
        await_to_completion: false,
        fork_context: false,
        owner: super::super::types::SubagentOwner::Task,
        implement_loop_effort: None,
        cancel_token: tokio_util::sync::CancellationToken::new(),
    };

    let err = backend.spawn(request).await.unwrap_err();
    assert!(err.to_string().contains("channel closed"));
}

#[tokio::test]
async fn channel_backend_query_found() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        let req = recv_event!(rx, Query);
        assert_eq!(req.subagent_id, "sub-1");
        assert!(req.block);
        assert_eq!(req.timeout_ms, Some(5000));
        req.respond_to
            .send(Some(SubagentSnapshot {
                subagent_id: "sub-1".to_string(),
                description: "find bugs".to_string(),
                subagent_type: "explore".to_string(),
                status: super::super::types::SubagentSnapshotStatus::Completed {
                    output: "result".to_string(),
                    tool_calls: 2,
                    turns: 1,
                    worktree_path: None,
                },
                started_at_epoch_ms: 1000,
                duration_ms: 200,
                persona: Some("reviewer".to_string()),
            }))
            .unwrap();
    });

    let snap = backend.query("sub-1", true, Some(5000)).await;
    let snap = snap.expect("snapshot should be present");
    assert_eq!(snap.subagent_id, "sub-1");
    assert_eq!(snap.description, "find bugs");
    assert_eq!(snap.subagent_type, "explore");
    assert_eq!(snap.started_at_epoch_ms, 1000);
    assert_eq!(snap.duration_ms, 200);
    assert_eq!(snap.persona.as_deref(), Some("reviewer"));
    match &snap.status {
        super::super::types::SubagentSnapshotStatus::Completed {
            output,
            tool_calls,
            turns,
            worktree_path,
        } => {
            assert_eq!(output, "result");
            assert_eq!(*tool_calls, 2);
            assert_eq!(*turns, 1);
            assert!(worktree_path.is_none());
        }
        other => panic!("Expected Completed, got {:?}", other),
    }

    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_query_non_blocking_passes_through() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        let req = recv_event!(rx, Query);
        assert_eq!(req.subagent_id, "sub-nb");
        assert!(!req.block, "block should be false");
        assert_eq!(req.timeout_ms, None, "timeout_ms should be None");
        req.respond_to.send(None).unwrap();
    });

    let snap = backend.query("sub-nb", false, None).await;
    assert!(snap.is_none());

    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_query_not_found() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        let req = recv_event!(rx, Query);
        req.respond_to.send(None).unwrap();
    });

    let snap = backend.query("nonexistent", false, None).await;
    assert!(snap.is_none());

    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_cancel_success() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        let req = recv_event!(rx, Cancel);
        match &req.target {
            SubagentCancelTarget::SubagentId(id) => assert_eq!(id, "sub-cancel"),
            other => panic!("Expected SubagentId, got {:?}", other),
        }
        req.respond_to
            .send(SubagentCancelOutcome::Cancelled)
            .unwrap();
    });

    let outcome = backend.cancel("sub-cancel").await;
    assert!(matches!(outcome, SubagentCancelOutcome::Cancelled));

    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_cancel_closed_channel() {
    let (tx, rx) = mpsc::unbounded_channel::<SubagentEvent>();
    drop(rx);

    let backend = ChannelBackend::new(tx);

    let outcome = backend.cancel("sub-cancel").await;
    assert!(matches!(outcome, SubagentCancelOutcome::NotFound));
}

#[tokio::test]
async fn workflow_spawn_future_drop_cancels_but_task_drop_does_not() {
    fn request_for(owner: super::super::types::SubagentOwner) -> SubagentRequest {
        SubagentRequest {
            id: "drop-owner-test".to_string(),
            prompt: "test".to_string(),
            description: "test".to_string(),
            subagent_type: "general-purpose".to_string(),
            parent_session_id: "parent".to_string(),
            parent_prompt_id: None,
            resume_from: None,
            cwd: None,
            runtime_overrides: Default::default(),
            run_in_background: false,
            surface_completion: false,
            await_to_completion: true,
            fork_context: false,
            owner,
            implement_loop_effort: None,
            cancel_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    for (owner, should_cancel) in [
        (super::super::types::SubagentOwner::Task, false),
        (super::super::types::SubagentOwner::workflow("wf-1"), true),
    ] {
        let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
        let backend = Arc::new(ChannelBackend::new(tx));
        let request = request_for(owner);
        let cancel_token = request.cancel_token.clone();
        let task = tokio::spawn({
            let backend = backend.clone();
            async move { backend.spawn(request).await }
        });
        let spawned = recv_event!(rx, Spawn);
        task.abort();
        let _ = task.await;
        assert_eq!(
            cancel_token.is_cancelled(),
            should_cancel,
            "only workflow receiver drop owns cancellation"
        );
        drop(spawned.result_tx);
    }
}

#[tokio::test]
async fn channel_backend_spawn_result_dropped() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        let req = recv_event!(rx, Spawn);
        drop(req.result_tx);
    });

    let request = SubagentRequest {
        id: "drop-test".to_string(),
        prompt: "test".to_string(),
        description: "test".to_string(),
        subagent_type: "general-purpose".to_string(),
        parent_session_id: "parent".to_string(),
        parent_prompt_id: None,
        resume_from: None,
        cwd: None,
        runtime_overrides: Default::default(),
        run_in_background: false,
        surface_completion: true,
        await_to_completion: false,
        fork_context: false,
        owner: super::super::types::SubagentOwner::Task,
        implement_loop_effort: None,
        cancel_token: tokio_util::sync::CancellationToken::new(),
    };

    let err = backend.spawn(request).await.unwrap_err();
    assert!(
        err.to_string().contains("result channel dropped"),
        "error: {err}"
    );

    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_query_closed_channel() {
    let (tx, rx) = mpsc::unbounded_channel::<SubagentEvent>();
    drop(rx);

    let backend = ChannelBackend::new(tx);

    let snap = backend.query("sub-1", false, None).await;
    assert!(snap.is_none());
}

// ── validate_type ────────────────────────────────────────────────

#[tokio::test]
async fn channel_backend_validate_type_round_trips_outcome() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        let event = rx.recv().await.unwrap();
        match event {
            SubagentEvent::ValidateType(req) => {
                assert_eq!(req.subagent_type, "explore");
                assert_eq!(req.parent_session_id, "parent-1");
                req.respond_to
                    .send(SubagentValidateTypeOutcome::Ok)
                    .unwrap();
            }
            _ => panic!("Expected ValidateType event"),
        }
    });

    let outcome = backend.validate_type("explore", "parent-1").await;
    assert!(matches!(outcome, SubagentValidateTypeOutcome::Ok));
    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_validate_type_propagates_unknown_outcome() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        if let Some(SubagentEvent::ValidateType(req)) = rx.recv().await {
            req.respond_to
                .send(SubagentValidateTypeOutcome::Unknown {
                    available: vec!["explore".into(), "plan".into()],
                })
                .unwrap();
        }
    });

    let outcome = backend.validate_type("invented", "p").await;
    match outcome {
        SubagentValidateTypeOutcome::Unknown { available } => {
            assert_eq!(available, vec!["explore".to_string(), "plan".to_string()]);
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_validate_type_returns_validation_unavailable_when_channel_closed() {
    let (tx, rx) = mpsc::unbounded_channel::<SubagentEvent>();
    drop(rx);
    let backend = ChannelBackend::new(tx);
    let outcome = backend.validate_type("explore", "p").await;
    assert!(matches!(
        outcome,
        SubagentValidateTypeOutcome::ValidationUnavailable
    ));
}

#[tokio::test]
async fn channel_backend_validate_type_returns_validation_unavailable_when_responder_dropped() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);
    let handle = tokio::spawn(async move {
        if let Some(SubagentEvent::ValidateType(req)) = rx.recv().await {
            drop(req.respond_to);
        }
    });
    let outcome = backend.validate_type("explore", "p").await;
    assert!(matches!(
        outcome,
        SubagentValidateTypeOutcome::ValidationUnavailable,
    ));
    handle.await.unwrap();
}

use super::super::types::test_capture;

#[tokio::test(start_paused = true)]
async fn channel_backend_validate_type_logs_warn_on_timeout() {
    let captured = test_capture::capture();
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    // Coordinator receives but never replies; keeps the responder
    // alive so the timeout arm fires (not responder-dropped).
    let holder = tokio::spawn(async move {
        if let Some(SubagentEvent::ValidateType(req)) = rx.recv().await {
            std::mem::forget(req.respond_to);
            std::future::pending::<()>().await;
        }
    });

    let validate = tokio::spawn(async move { backend.validate_type("explore", "p").await });
    tokio::time::advance(VALIDATE_TYPE_TIMEOUT + std::time::Duration::from_millis(1)).await;
    let outcome = validate.await.unwrap();
    assert!(matches!(
        outcome,
        SubagentValidateTypeOutcome::ValidationUnavailable
    ));

    let mut events_rx = captured.events_rx;
    let mut saw_timeout_warn = false;
    while let Ok(event) = events_rx.try_recv() {
        if event.level == tracing::Level::WARN
            && event.fields.contains("coordinator validation timed out")
            && event.fields.contains("subagent_type=explore")
            && event.fields.contains("timeout_ms=")
        {
            saw_timeout_warn = true;
            break;
        }
    }
    assert!(saw_timeout_warn, "must emit WARN with timeout_ms field");

    holder.abort();
}

// ── describe_subagent_type ───────────────────────────────────────

#[tokio::test]
async fn channel_backend_describe_round_trips_summary() {
    use super::super::types::{SubagentDescribeOutcome, SubagentTypeSummary};
    use crate::types::tool::ToolKind;

    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        match rx.recv().await.unwrap() {
            SubagentEvent::DescribeType(req) => {
                assert_eq!(req.subagent_type, "explore");
                assert_eq!(req.harness_agent_type.as_deref(), Some("cursor"));
                assert_eq!(req.parent_session_id, "parent-1");
                let mut summary = SubagentTypeSummary {
                    can_read: true,
                    can_search: true,
                    ..Default::default()
                };
                summary
                    .tool_names
                    .insert(ToolKind::Read, "read_file".to_string());
                req.respond_to
                    .send(SubagentDescribeOutcome::Ok(summary))
                    .unwrap();
            }
            _ => panic!("Expected DescribeType event"),
        }
    });

    let outcome = backend
        .describe_subagent_type("explore", Some("cursor"), "parent-1")
        .await;
    match outcome {
        SubagentDescribeOutcome::Ok(summary) => {
            assert!(summary.can_read && summary.can_search && !summary.can_execute);
            assert_eq!(
                summary.tool_names.get(&ToolKind::Read).unwrap(),
                "read_file"
            );
        }
        other => panic!("expected Ok, got {other:?}"),
    }
    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_describe_propagates_not_allowed_outcome() {
    use super::super::types::SubagentDescribeOutcome;

    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let handle = tokio::spawn(async move {
        if let Some(SubagentEvent::DescribeType(req)) = rx.recv().await {
            req.respond_to
                .send(SubagentDescribeOutcome::NotAllowed {
                    allowed: vec!["explore".into()],
                })
                .unwrap();
        }
    });

    match backend.describe_subagent_type("plan", None, "p").await {
        SubagentDescribeOutcome::NotAllowed { allowed } => {
            assert_eq!(allowed, vec!["explore".to_string()]);
        }
        other => panic!("expected NotAllowed, got {other:?}"),
    }
    handle.await.unwrap();
}

#[tokio::test]
async fn channel_backend_describe_returns_unavailable_when_channel_closed() {
    use super::super::types::SubagentDescribeOutcome;
    let (tx, rx) = mpsc::unbounded_channel::<SubagentEvent>();
    drop(rx);
    let backend = ChannelBackend::new(tx);
    assert!(matches!(
        backend.describe_subagent_type("explore", None, "p").await,
        SubagentDescribeOutcome::Unavailable
    ));
}

#[tokio::test]
async fn channel_backend_describe_returns_unavailable_when_responder_dropped() {
    use super::super::types::SubagentDescribeOutcome;
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);
    let handle = tokio::spawn(async move {
        if let Some(SubagentEvent::DescribeType(req)) = rx.recv().await {
            drop(req.respond_to);
        }
    });
    assert!(matches!(
        backend.describe_subagent_type("explore", None, "p").await,
        SubagentDescribeOutcome::Unavailable
    ));
    handle.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn channel_backend_describe_returns_unavailable_on_timeout() {
    use super::super::types::SubagentDescribeOutcome;
    let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let backend = ChannelBackend::new(tx);

    let holder = tokio::spawn(async move {
        if let Some(SubagentEvent::DescribeType(req)) = rx.recv().await {
            std::mem::forget(req.respond_to);
            std::future::pending::<()>().await;
        }
    });

    let describe =
        tokio::spawn(async move { backend.describe_subagent_type("explore", None, "p").await });
    tokio::time::advance(VALIDATE_TYPE_TIMEOUT + std::time::Duration::from_millis(1)).await;
    assert!(matches!(
        describe.await.unwrap(),
        SubagentDescribeOutcome::Unavailable
    ));
    holder.abort();
}

#[test]
fn parse_timeout_ms_returns_none_for_unset() {
    assert_eq!(parse_timeout_ms(None), None);
}

#[test]
fn parse_timeout_ms_returns_none_for_unparseable() {
    assert_eq!(parse_timeout_ms(Some("not-a-number")), None);
    assert_eq!(parse_timeout_ms(Some("")), None);
    assert_eq!(parse_timeout_ms(Some("3.14")), None);
    assert_eq!(parse_timeout_ms(Some("-100")), None);
}

#[test]
fn parse_timeout_ms_returns_none_for_zero() {
    assert_eq!(parse_timeout_ms(Some("0")), None);
}

#[test]
fn parse_timeout_ms_returns_value_for_positive_integer() {
    assert_eq!(parse_timeout_ms(Some("5000")), Some(5000));
    assert_eq!(parse_timeout_ms(Some("1")), Some(1));
}
