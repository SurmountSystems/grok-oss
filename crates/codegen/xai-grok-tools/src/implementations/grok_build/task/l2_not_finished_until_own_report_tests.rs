//! An L2 is not finished until it writes its own report.
//! The completion notice and the finished decision are different functions.

use super::*;
use crate::implementations::grok_build::task::types::{
    SubagentEvent, SubagentListActiveRequest, SubagentSnapshotStatus,
};
use tokio::sync::oneshot;

async fn list_ids(
    harness: &Harness,
    session: &str,
    event: fn(SubagentListActiveRequest) -> SubagentEvent,
) -> Vec<String> {
    let (respond_to, response) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(event(SubagentListActiveRequest {
            parent_session_id: session.to_owned(),
            respond_to,
        }))
        .expect("coordinator command channel open");
    response
        .await
        .expect("list response")
        .into_iter()
        .map(|summary| summary.subagent_id)
        .collect()
}

/// An L2 that returns after only spawning, or with no report, stays listed.
/// When the L3 finishes, that same L2 receives the completion. An unbounded
/// wait fails. Finished is `record_own_report`, not the completion notice.
#[tokio::test]
async fn l2_is_not_finished_until_it_writes_its_own_report() {
    let mut harness = harness_l2_omits_own_report();
    let l2_backend = spawn_child(&mut harness, false).await;
    let mut l3 = request("l3", true);
    l3.runtime_overrides.spawn_depth = Some(2);
    l3.runtime_overrides.immediate_parent_session_id = Some("child".to_owned());
    let spawn_l3 = spawn_nested(&mut harness, &l2_backend, l3).await;
    assert_eq!(harness.started.recv().await.as_deref(), Some("l3"));

    let _ = harness.finish_one.send("child".to_owned());
    let notice = tokio::time::timeout(std::time::Duration::from_secs(2), harness.completions.recv())
        .await
        .expect(
            "completion notice must arrive while the L3 is still running; an unbounded wait is a failure",
        );
    assert!(notice.is_some(), "completion channel open");

    let listed = list_ids(&harness, "parent", SubagentEvent::ListActive).await;
    assert!(
        listed.iter().any(|id| id == "child"),
        "L2 stays listed before its own report, got {listed:?}"
    );
    let running = harness
        .backend
        .query("child", false, None)
        .await
        .expect("L2 stays queryable");
    assert!(
        matches!(running.status, SubagentSnapshotStatus::Running { .. }),
        "not finished before its own report, got {:?}",
        running.status
    );
    let completed = list_ids(&harness, "parent", SubagentEvent::ListCompleted).await;
    assert!(
        !completed.iter().any(|id| id == "child"),
        "ListCompleted must not include an L2 with no report, got {completed:?}"
    );

    let _ = harness.finish_one.send("l3".to_owned());
    assert!(spawn_l3.await.unwrap().unwrap().success);
    let l3_done = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("L3 completion notice; an unbounded wait is a failure");
    assert!(l3_done.is_some(), "L3 completion channel open");

    let delivery =
        tokio::time::timeout(std::time::Duration::from_secs(2), harness.follow_ups.recv())
            .await
            .expect("the same L2 receives the L3 completion; an unbounded wait is a failure")
            .expect("follow_up channel open");
    assert_eq!(delivery.child_session_id, "child");
    assert!(
        delivery.text.contains("l3"),
        "delivery names the L3, got {}",
        delivery.text
    );
    let (deliveries_tx, deliveries_rx) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::L2CompletionDeliveries {
            respond_to: deliveries_tx,
        })
        .expect("coordinator command channel open");
    let deliveries = deliveries_rx.await.expect("delivery list");
    assert!(
        deliveries
            .iter()
            .any(|(l2, l3)| l2 == "child" && l3 == "l3"),
        "recorded delivery is the real completion path, got {deliveries:?}"
    );

    let still = harness
        .backend
        .query("child", false, None)
        .await
        .expect("L2 still listed after the L3 completion");
    assert!(
        matches!(still.status, SubagentSnapshotStatus::Running { .. }),
        "L3 completion must not mark the L2 finished, got {:?}",
        still.status
    );

    let (report_tx, report_rx) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::RecordOwnReport {
            subagent_id: "child".to_owned(),
            respond_to: report_tx,
        })
        .expect("coordinator command channel open");
    assert!(report_rx.await.expect("report reply"));
    let finished = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("finished notice after the report; an unbounded wait is a failure");
    assert!(finished.is_some(), "finished completion channel open");
    let done = harness
        .backend
        .query("child", false, None)
        .await
        .expect("L2 query after its report");
    assert!(
        matches!(done.status, SubagentSnapshotStatus::Completed { .. }),
        "finished only after its own report, got {:?}",
        done.status
    );

    harness.actor.abort();
}

/// The runner returns, the output has no own report, and no nested agent is
/// still running. That is the turn-end path where `spawned_child_still_running`
/// is false. The L2 stays listed until `RecordOwnReport`. The L3 already
/// finished, so this is not the stay test's live-nested path.
#[tokio::test]
async fn l2_runner_return_with_no_live_nested_agent_stays_until_own_report() {
    let mut harness = harness_l2_omits_own_report();
    let l2_backend = spawn_child(&mut harness, false).await;
    let mut l3 = request("l3", true);
    l3.runtime_overrides.spawn_depth = Some(2);
    l3.runtime_overrides.immediate_parent_session_id = Some("child".to_owned());
    let spawn_l3 = spawn_nested(&mut harness, &l2_backend, l3).await;
    let started = tokio::time::timeout(std::time::Duration::from_secs(2), harness.started.recv())
        .await
        .expect("L3 start; an unbounded wait is a failure");
    assert_eq!(started.as_deref(), Some("l3"));

    let _ = harness.finish_one.send("l3".to_owned());
    let joined = tokio::time::timeout(std::time::Duration::from_secs(2), spawn_l3)
        .await
        .expect("L3 join; an unbounded wait is a failure")
        .expect("L3 task");
    assert!(
        joined.expect("L3 result").success,
        "an L3 with no L2 report still finishes"
    );
    let l3_done = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("L3 completion; an unbounded wait is a failure");
    assert!(l3_done.is_some(), "L3 completion channel open");

    // Same harness finish the stay test uses. The output is the prompt, not a report.
    let _ = harness.finish_one.send("child".to_owned());
    let early = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await;
    assert!(
        early.is_err(),
        "L2 runner return with no own report must not finish when no nested agent is still running"
    );

    let listed = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        list_ids(&harness, "parent", SubagentEvent::ListActive),
    )
    .await
    .expect("list active bounded at 2 seconds");
    assert!(
        listed.iter().any(|id| id == "child"),
        "L2 stays listed, got {listed:?}"
    );
    let running = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.backend.query("child", false, None),
    )
    .await
    .expect("query bounded at 2 seconds")
    .expect("L2 stays queryable");
    assert!(
        matches!(running.status, SubagentSnapshotStatus::Running { .. }),
        "not finished, got {:?}",
        running.status
    );
    let completed = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        list_ids(&harness, "parent", SubagentEvent::ListCompleted),
    )
    .await
    .expect("list completed bounded at 2 seconds");
    assert!(
        !completed.iter().any(|id| id == "child"),
        "ListCompleted must not include an L2 with no report, got {completed:?}"
    );

    let (report_tx, report_rx) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::RecordOwnReport {
            subagent_id: "child".to_owned(),
            respond_to: report_tx,
        })
        .expect("coordinator command channel open");
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), report_rx)
            .await
            .expect("report reply bounded at 2 seconds")
            .expect("report channel")
    );
    let finished = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("finished notice after the report; an unbounded wait is a failure");
    assert!(finished.is_some(), "finished completion channel open");
    let done = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.backend.query("child", false, None),
    )
    .await
    .expect("query after report bounded at 2 seconds")
    .expect("L2 query after its report");
    assert!(
        matches!(done.status, SubagentSnapshotStatus::Completed { .. }),
        "finished only after its own report, got {:?}",
        done.status
    );

    harness.actor.abort();
}

/// An L2 that never spawned a nested agent, and whose runner returns with no
/// own report, stays listed. The wait is two seconds. `RecordOwnReport`
/// finishes it.
#[tokio::test]
async fn l2_that_never_spawned_stays_listed_until_own_report() {
    let mut harness = harness_l2_omits_own_report();
    let _l2_backend = spawn_child(&mut harness, false).await;

    let _ = harness.finish_one.send("child".to_owned());
    let early = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await;
    assert!(
        early.is_err(),
        "an L2 that never spawned and has no own report must stay unfinished"
    );

    let listed = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        list_ids(&harness, "parent", SubagentEvent::ListActive),
    )
    .await
    .expect("list active bounded at 2 seconds");
    assert!(
        listed.iter().any(|id| id == "child"),
        "L2 that never spawned stays listed, got {listed:?}"
    );
    let running = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.backend.query("child", false, None),
    )
    .await
    .expect("query bounded at 2 seconds")
    .expect("L2 stays queryable");
    assert!(
        matches!(running.status, SubagentSnapshotStatus::Running { .. }),
        "not finished, got {:?}",
        running.status
    );
    let completed = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        list_ids(&harness, "parent", SubagentEvent::ListCompleted),
    )
    .await
    .expect("list completed bounded at 2 seconds");
    assert!(
        !completed.iter().any(|id| id == "child"),
        "ListCompleted must not include an L2 with no report, got {completed:?}"
    );

    let (report_tx, report_rx) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::RecordOwnReport {
            subagent_id: "child".to_owned(),
            respond_to: report_tx,
        })
        .expect("coordinator command channel open");
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), report_rx)
            .await
            .expect("report reply bounded at 2 seconds")
            .expect("report channel")
    );
    let finished = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("finished notice after the report; an unbounded wait is a failure");
    assert!(finished.is_some(), "finished completion channel open");
    let done = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.backend.query("child", false, None),
    )
    .await
    .expect("query after report bounded at 2 seconds")
    .expect("L2 query after its report");
    assert!(
        matches!(done.status, SubagentSnapshotStatus::Completed { .. }),
        "finished only after its own report, got {:?}",
        done.status
    );

    harness.actor.abort();
}

async fn recorded_l2(harness: &Harness, id: &str) -> (Option<String>, Vec<String>) {
    let (respond_to, response) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::L2RecordedTask {
            subagent_id: id.to_owned(),
            respond_to,
        })
        .expect("coordinator command channel open");
    tokio::time::timeout(std::time::Duration::from_secs(2), response)
        .await
        .expect("recorded L2 task bounded at 2 seconds")
        .expect("recorded L2 task channel")
}

/// L1 spawns an L2. That L2 records spawn, wait, read_l3_report, and
/// write_l2_report on the coordinator, one site at a time.
#[tokio::test]
async fn default_accomplishment_is_l1_spawn_of_l2_not_l1_doing_the_multi_file_job() {
    let mut harness = harness_l2_omits_own_report();
    let l2_backend = spawn_child(&mut harness, false).await;
    let (task_text, actions) = recorded_l2(&harness, "child").await;
    let task_text = task_text.expect("L2 task text is stored at admit");
    assert!(
        task_text.contains("The L2 records spawn, wait, read_l3_report, write_l2_report."),
        "stored L2 task text must record the L2 actions, got {task_text}"
    );
    assert!(
        task_text.contains("The L2 does not grep, edit, or review."),
        "stored L2 task text forbids grep, edit, and review, got {task_text}"
    );
    assert!(
        !task_text.contains("one already-named host command"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert!(
        !task_text.contains("one already-named path read"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert!(
        !task_text.contains("one asked-for report quote"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert!(
        !task_text.contains("one already-named one-line edit"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert!(
        actions.is_empty(),
        "admit of the L2 must not pre-seed actions, got {actions:?}"
    );

    let mut l3 = request("l3", true);
    l3.runtime_overrides.spawn_depth = Some(2);
    l3.runtime_overrides.immediate_parent_session_id = Some("child".to_owned());
    let spawn_l3 = spawn_nested(&mut harness, &l2_backend, l3).await;
    let started = tokio::time::timeout(std::time::Duration::from_secs(2), harness.started.recv())
        .await
        .expect("L3 start; an unbounded wait is a failure");
    assert_eq!(started.as_deref(), Some("l3"));
    let (_task_text, actions) = recorded_l2(&harness, "child").await;
    assert_eq!(
        actions,
        ["spawn"],
        "admitting the L3 appends only spawn, got {actions:?}"
    );

    let _ = harness.finish_one.send("child".to_owned());
    let notice = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("still-running branch must deliver a notice; an unbounded wait is a failure");
    assert!(notice.is_some(), "completion channel open");
    let (_task_text, actions) = recorded_l2(&harness, "child").await;
    assert_eq!(
        actions,
        ["spawn", "wait"],
        "the still-running branch appends only wait, got {actions:?}"
    );

    let _ = harness.finish_one.send("l3".to_owned());
    let joined = tokio::time::timeout(std::time::Duration::from_secs(2), spawn_l3)
        .await
        .expect("L3 join; an unbounded wait is a failure")
        .expect("L3 task");
    assert!(joined.expect("L3 result").success);
    let l3_done = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.completions.recv(),
    )
    .await
    .expect("L3 completion; an unbounded wait is a failure");
    assert!(l3_done.is_some(), "L3 completion channel open");
    let delivery = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness.follow_ups.recv(),
    )
    .await
    .expect("continue_coordinators_waiting_on delivers to the L2; an unbounded wait is a failure")
    .expect("follow_up channel open");
    assert_eq!(delivery.child_session_id, "child");
    assert!(
        delivery.text.contains("l3"),
        "delivery names the L3, got {}",
        delivery.text
    );
    let (_task_text, actions) = recorded_l2(&harness, "child").await;
    assert_eq!(
        actions,
        ["spawn", "wait", "read_l3_report"],
        "the L3 completion appends only read_l3_report, got {actions:?}"
    );

    let (report_tx, report_rx) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::RecordOwnReport {
            subagent_id: "child".to_owned(),
            respond_to: report_tx,
        })
        .expect("coordinator command channel open");
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), report_rx)
            .await
            .expect("report reply bounded at 2 seconds")
            .expect("report channel")
    );
    let (task_text, actions) = recorded_l2(&harness, "child").await;
    let task_text = task_text.expect("L2 task text stays stored after the report");
    assert!(
        task_text.contains("The L2 records spawn, wait, read_l3_report, write_l2_report."),
        "stored L2 task text must record the L2 actions, got {task_text}"
    );
    assert!(
        task_text.contains("The L2 does not grep, edit, or review."),
        "stored L2 task text forbids grep, edit, and review, got {task_text}"
    );
    assert!(
        !task_text.contains("one already-named host command"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert!(
        !task_text.contains("one already-named path read"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert!(
        !task_text.contains("one asked-for report quote"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert!(
        !task_text.contains("one already-named one-line edit"),
        "L1 fast path text is absent from the stored L2 task, got {task_text}"
    );
    assert_eq!(
        actions,
        ["spawn", "wait", "read_l3_report", "write_l2_report"],
        "recorded actions must be exactly spawn, wait, read_l3_report, write_l2_report, got {actions:?}"
    );

    harness.actor.abort();
}
