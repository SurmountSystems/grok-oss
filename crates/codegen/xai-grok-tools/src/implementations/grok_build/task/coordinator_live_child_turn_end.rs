use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::tests::{harness, request};
use crate::implementations::grok_build::task::backend::SubagentBackend;
use crate::implementations::grok_build::task::types::{
    SubagentEvent, SubagentRegistryCounts, SubagentSnapshotStatus,
};

fn spawn_noting_registration(
    backend: crate::implementations::grok_build::task::backend::ChannelBackend,
    request: crate::implementations::grok_build::task::types::SubagentRequest,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    tokio::task::JoinHandle<
        Result<
            crate::implementations::grok_build::task::types::SubagentResult,
            xai_tool_runtime::ToolError,
        >,
    >,
) {
    let (registered_tx, registered_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async move {
        SubagentBackend::spawn(&backend, request, Some(registered_tx)).await
    });
    (registered_rx, handle)
}

async fn start_one(harness: &mut super::tests::Harness, id: &str) -> CancellationToken {
    let _ = harness.start.send(());
    let started = harness
        .started
        .recv()
        .await
        .unwrap_or_else(|| panic!("{id} did not start"));
    assert_eq!(started, id);
    harness
        .tokens
        .recv()
        .await
        .unwrap_or_else(|| panic!("{id} did not publish its cancel token"))
}

/// The completion notice is delivered while a nested agent is still running.
/// The coordinator stays running and is not cancelled until it writes its own
/// report. Session shutdown is a separate function.
#[tokio::test]
async fn coordinator_stays_running_when_turn_returns_with_live_child() {
    let mut harness = harness(true, std::time::Duration::from_secs(60));
    let backend = harness.backend.clone();

    let (registered, coord_spawn) =
        spawn_noting_registration(backend.clone(), request("coord", true));
    registered.await.expect("coordinator registration");
    harness
        .requests
        .recv()
        .await
        .expect("runner received the coordinator");
    let coord_token = start_one(&mut harness, "coord").await;

    let mut child = request("live-child", true);
    child.parent_session_id = "coord".to_owned();
    let (child_registered, child_spawn) = spawn_noting_registration(backend.clone(), child);
    child_registered.await.expect("nested agent registration");
    harness
        .requests
        .recv()
        .await
        .expect("runner received the nested agent");
    let child_token = start_one(&mut harness, "live-child").await;

    let _ = harness.finish_one.send("coord".to_owned());
    let early_notification = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await;
    assert!(
        early_notification.is_ok(),
        "completion notice must be delivered while a nested agent is still running"
    );
    assert!(
        !coord_token.is_cancelled(),
        "coordinator turn return must not cancel the coordinator while its child is running"
    );

    let coord = backend
        .query("coord", false, None)
        .await
        .expect("coordinator stays queryable");
    assert!(
        matches!(coord.status, SubagentSnapshotStatus::Running { .. }),
        "host status must stay running, got {:?}",
        coord.status
    );
    let child_snapshot = backend
        .query("live-child", false, None)
        .await
        .expect("nested agent stays queryable");
    assert!(
        child_snapshot.is_running(),
        "nested agent must still be running"
    );
    assert_eq!(
        backend.registry_counts().await,
        SubagentRegistryCounts {
            pending: 0,
            active: 2,
            completed: 0,
            queued: 0,
        }
    );

    let _ = harness.finish_one.send("live-child".to_owned());
    let _child_done = harness
        .completions
        .recv()
        .await
        .expect("a finished nested agent may still complete");
    assert!(
        child_token.is_cancelled(),
        "a finished nested agent still reaches its terminal cancel"
    );
    let finished_child = backend
        .query("live-child", false, None)
        .await
        .expect("finished nested agent stays queryable");
    assert!(
        matches!(
            finished_child.status,
            SubagentSnapshotStatus::Completed { .. }
        ),
        "a finished nested agent may still complete, got {:?}",
        finished_child.status
    );
    let continued = harness
        .follow_ups
        .try_recv()
        .expect("the same coordinator receives the nested completion");
    assert_eq!(continued.child_session_id, "coord");
    assert!(
        continued.text.contains("live-child"),
        "the completion names the nested agent, got {}",
        continued.text
    );
    let coord_before_report = backend
        .query("coord", false, None)
        .await
        .expect("coordinator stays queryable before its own report");
    assert!(
        matches!(
            coord_before_report.status,
            SubagentSnapshotStatus::Running { .. }
        ),
        "not finished until its own report, got {:?}",
        coord_before_report.status
    );
    assert!(
        !coord_token.is_cancelled(),
        "session shutdown stays separate from the completion notice"
    );
    let (report_tx, report_rx) = oneshot::channel();
    backend
        .sender()
        .send(SubagentEvent::RecordOwnReport {
            subagent_id: "coord".to_owned(),
            respond_to: report_tx,
        })
        .expect("coordinator command channel open");
    assert!(
        report_rx.await.expect("report reply"),
        "the coordinator must accept its own report"
    );
    let _coord_done = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("finished notice after the report; an unbounded wait is a failure")
    .expect("completion channel open");
    let coord_after_child = backend
        .query("coord", false, None)
        .await
        .expect("coordinator stays queryable after its own report");
    assert!(
        matches!(
            coord_after_child.status,
            SubagentSnapshotStatus::Completed { .. }
        ),
        "the coordinator is finished after its own report, got {:?}",
        coord_after_child.status
    );

    let (solo_registered, solo_spawn) =
        spawn_noting_registration(backend.clone(), request("solo", true));
    solo_registered
        .await
        .expect("solo coordinator registration");
    harness
        .requests
        .recv()
        .await
        .expect("runner received the solo coordinator");
    let solo_token = start_one(&mut harness, "solo").await;
    let _ = harness.finish_one.send("solo".to_owned());
    let solo_done = harness
        .completions
        .recv()
        .await
        .expect("a coordinator with no live child still emits the completed notification");
    assert!(solo_done.backgrounded && solo_done.should_surface);
    assert!(solo_token.is_cancelled());
    let solo = backend
        .query("solo", false, None)
        .await
        .expect("solo coordinator stays queryable");
    assert!(
        matches!(solo.status, SubagentSnapshotStatus::Completed { .. }),
        "a coordinator with no live child may still complete, got {:?}",
        solo.status
    );
    let coord_after_solo = backend
        .query("coord", false, None)
        .await
        .expect("coordinator with a live-child history stays queryable");
    assert!(
        matches!(
            coord_after_solo.status,
            SubagentSnapshotStatus::Completed { .. }
        ),
        "the coordinator that had a live child stays completed, got {:?}",
        coord_after_solo.status
    );

    drop(coord_spawn);
    drop(child_spawn);
    drop(solo_spawn);
    harness.actor.abort();
}

/// The session loop stops on shutdown, then a continuation send is discarded.
/// `shutdown_session_on_turn_end` is what the shell calls before that send.
#[tokio::test]
async fn coordinator_session_is_not_shut_down_before_nested_agent_finishes() {
    async fn session_after_turn(live_child: bool) -> (bool, Option<&'static str>) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let session = tokio::spawn(async move {
            let mut saw_shutdown = false;
            let mut continuation = None;
            while let Some(cmd) = rx.recv().await {
                if cmd == "shutdown" {
                    saw_shutdown = true;
                    break;
                }
                continuation = Some(cmd);
            }
            (saw_shutdown, continuation)
        });
        if super::shutdown_session_on_turn_end(live_child) {
            let _ = tx.send("shutdown");
        }
        let sent = tx.send("interject");
        drop(tx);
        let (saw_shutdown, continuation) = session.await.expect("session loop");
        if saw_shutdown {
            assert!(
                sent.is_err() || continuation.is_none(),
                "continuation after shutdown must not be the command the session ran"
            );
        } else {
            assert!(
                sent.is_ok(),
                "continuation must not be a discarded Interject after shutdown"
            );
        }
        (saw_shutdown, continuation)
    }

    let (saw_shutdown, continuation) = session_after_turn(true).await;
    assert!(
        !saw_shutdown,
        "must not shut down the coordinator session while a nested agent is still running"
    );
    assert_eq!(
        continuation,
        Some("interject"),
        "the same session must receive the continuation"
    );

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    assert!(
        super::shutdown_session_on_turn_end(false),
        "a coordinator with no live child may still shut down"
    );
    let _ = tx.send("shutdown");
    assert_eq!(rx.recv().await, Some("shutdown"));
    drop(rx);
    assert!(
        tx.send("interject").is_err(),
        "Interject after shutdown is discarded because the session stopped reading"
    );
}
