//! GitHub #222: L2 may compact and still resume its L3s.
//! GitHub #224 / FORK.md agent-depth: keep these names. Do not weaken.
//!
//! After compact, that L2 still:
//! - `follow_up` a running L3 (same L3 id), talk-down
//! - `resume_from` a completed L3, talk-down
//! - receive that L3's completion report, talk-up
//! L3 still cannot spawn. There is no L4. `MAX_SUBAGENT_DEPTH` stays 2.
//!
//! Talk-up of an L3 that was not compacted is
//! `nested_grandchild_completion_buffers_for_spawner_not_root`.
//! Named tests are contracts. Do not fit tests to code.

use super::*;
use crate::implementations::grok_build::task::types::{
    SubagentEvent, SubagentFollowUpOutcome, SubagentListActiveRequest, SubagentResumeLookup,
};
use tokio::sync::oneshot;

async fn compact_list_active(harness: &Harness, session: &str) -> Vec<String> {
    let (respond_to, response) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::ListActive(SubagentListActiveRequest {
            parent_session_id: session.to_owned(),
            respond_to,
        }))
        .expect("coordinator command channel open");
    response
        .await
        .unwrap()
        .into_iter()
        .map(|summary| summary.subagent_id)
        .collect()
}

async fn compact_list_completed(harness: &Harness, session: &str) -> Vec<String> {
    let (respond_to, response) = oneshot::channel();
    harness
        .backend
        .sender()
        .send(SubagentEvent::ListCompleted(SubagentListActiveRequest {
            parent_session_id: session.to_owned(),
            respond_to,
        }))
        .expect("coordinator command channel open");
    response
        .await
        .unwrap()
        .into_iter()
        .map(|summary| summary.subagent_id)
        .collect()
}

/// GitHub #222 / #224: compact on L2 does not drop the L3. L2 can follow_up a
/// running L3 and resume_from a completed L3. L1 follow_up of that L3
/// stays LiveL3Unbothered (#143). See FORK.md agent-depth.
#[tokio::test]
async fn l2_after_compact_still_resumes_its_l3s() {
    let mut harness = harness(false, std::time::Duration::from_secs(60));
    let l2_backend = spawn_child(&mut harness, false).await;

    let mut l3 = request("l3", true);
    l3.runtime_overrides.spawn_depth = Some(2);
    l3.runtime_overrides.immediate_parent_session_id = Some("child".to_owned());
    let spawn_l3 = spawn_nested(&mut harness, &l2_backend, l3).await;
    assert_eq!(harness.started.recv().await.as_deref(), Some("l3"));

    let listed = compact_list_active(&harness, "child").await;
    assert_eq!(
        listed,
        ["l3"],
        "L2 compact ListActive must keep the running L3 id"
    );

    let outcome = l2_backend
        .follow_up("l3", "L2 follow-up after compact, same L3 id")
        .await;
    assert_eq!(
        outcome,
        SubagentFollowUpOutcome::Queued {
            child_session_id: "l3".to_owned(),
        },
        "after L2 compact, L2 follow_up of its running L3 must enqueue Interject on that L3"
    );
    let delivered = harness
        .follow_ups
        .recv()
        .await
        .expect("Interject delivered to L3");
    assert_eq!(delivered.child_session_id, "l3");
    assert_eq!(delivered.text, "L2 follow-up after compact, same L3 id");

    let l1_outcome = parent_backend(&harness)
        .follow_up("l3", "L1 must not barge into the specialist")
        .await;
    assert_eq!(
        l1_outcome,
        SubagentFollowUpOutcome::LiveL3Unbothered,
        "L1 follow-up of a live L3 stays refused unless the Operator targeted that specialist"
    );

    let _ = harness.finish_one.send("l3".to_owned());
    assert!(spawn_l3.await.unwrap().unwrap().success);

    let completed = compact_list_completed(&harness, "child").await;
    assert_eq!(
        completed,
        ["l3"],
        "L2 compact ListCompleted must keep the completed L3 id for resume_from"
    );

    let mut resume = request("identity-resume", true);
    resume.resume_from = Some("l3".to_owned());
    let _resumed = spawn_nested(&mut harness, &l2_backend, resume).await;
    let SubagentResumeLookup::Completed(source) = harness.resume_sources.recv().await.unwrap()
    else {
        panic!("after L2 compact, resume_from of the completed L3 must be Completed, not Missing");
    };
    assert_eq!(source.subagent_id, "l3");
    assert_eq!(
        harness.started.recv().await.as_deref(),
        Some("identity-resume")
    );
    let inspection = l2_backend
        .inspect("identity-resume")
        .await
        .expect("L2 reaches resume_from of its completed L3 after compact");
    assert_eq!(inspection.resumed_from.as_deref(), Some("l3"));

    harness.actor.abort();
}

/// GitHub #222 / #224: `resume_from` of a still-running L3 stays fail-closed Active.
/// A running id uses `follow_up`. See FORK.md agent-depth.
#[tokio::test]
async fn l2_resume_from_of_running_l3_still_fails_active() {
    let mut harness = harness(false, std::time::Duration::from_secs(60));
    let l2_backend = spawn_child(&mut harness, false).await;
    let mut l3 = request("l3", true);
    l3.runtime_overrides.spawn_depth = Some(2);
    l3.runtime_overrides.immediate_parent_session_id = Some("child".to_owned());
    let spawn_l3 = spawn_nested(&mut harness, &l2_backend, l3).await;
    assert_eq!(harness.started.recv().await.as_deref(), Some("l3"));

    let mut resume = request("identity-resume", true);
    resume.resume_from = Some("l3".to_owned());
    // Do not use spawn_nested: that waits on the runner request, and Active
    // is a pre-start reject (no nested HTTP).
    let resume_spawn = tokio::spawn({
        let backend = l2_backend.clone();
        async move { backend.spawn(resume, None).await }
    });
    assert!(
        matches!(
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                harness.resume_sources.recv(),
            )
            .await
            .expect("resume_from of a running L3 must emit Active without waiting on nested HTTP")
            .expect("resume_sources open"),
            SubagentResumeLookup::Active
        ),
        "resume_from of a running L3 after L2 compact stays Active; use follow_up"
    );
    let resume_result = tokio::time::timeout(std::time::Duration::from_secs(2), resume_spawn)
        .await
        .expect("rejected resume_from must not wait on the running L3")
        .expect("join")
        .expect("spawn result");
    assert!(
        !resume_result.success,
        "resume_from of a running L3 must fail closed"
    );

    let _ = harness.finish.send(());
    assert!(spawn_l3.await.unwrap().unwrap().success);
    harness.actor.abort();
}

/// GitHub #222 / #224: L3 still cannot spawn. There is no L4. Follow-up is not spawn.
/// `MAX_SUBAGENT_DEPTH` stays 2. See FORK.md agent-depth.
#[tokio::test]
async fn l2_follow_up_of_l3_does_not_spawn_l4() {
    let mut harness = harness(false, std::time::Duration::from_secs(60));
    let l2_backend = spawn_child(&mut harness, false).await;
    let mut l3 = request("l3", true);
    l3.runtime_overrides.spawn_depth = Some(2);
    l3.runtime_overrides.immediate_parent_session_id = Some("child".to_owned());
    let spawn_l3 = spawn_nested(&mut harness, &l2_backend, l3).await;
    assert_eq!(harness.started.recv().await.as_deref(), Some("l3"));

    let outcome = l2_backend
        .follow_up("l3", "follow_up must not allocate a fourth layer")
        .await;
    assert_eq!(
        outcome,
        SubagentFollowUpOutcome::Queued {
            child_session_id: "l3".to_owned(),
        },
        "L2 follow_up of a running L3 must enqueue Interject, not spawn"
    );
    assert_eq!(
        harness.backend.registry_counts().await,
        SubagentRegistryCounts {
            pending: 0,
            active: 2,
            completed: 0,
            queued: 0,
        },
        "L2 follow_up of L3 must not spawn L4"
    );
    assert!(
        harness.requests.try_recv().is_err(),
        "follow_up must not emit a spawn request"
    );

    let _ = harness.finish.send(());
    assert!(spawn_l3.await.unwrap().unwrap().success);
    harness.actor.abort();
}

/// GitHub #224: L2 compact still leaves talk-up. An L3 completion report
/// buffers for that L2, not for L1. Talk-down is `l2_after_compact_still_resumes_its_l3s`.
/// See FORK.md agent-depth. Discarded L3 context reports up; it does not compact into L1.
#[tokio::test]
async fn l2_after_compact_still_receives_l3_completion_report() {
    let mut harness = harness_with_config(false, buffering());
    let l2_backend = spawn_child(&mut harness, false).await;
    let mut l3 = request("l3", true);
    l3.runtime_overrides.spawn_depth = Some(2);
    l3.runtime_overrides.immediate_parent_session_id = Some("child".to_owned());
    let spawn_l3 = spawn_nested(&mut harness, &l2_backend, l3).await;
    assert_eq!(harness.started.recv().await.as_deref(), Some("l3"));

    let listed = compact_list_active(&harness, "child").await;
    assert_eq!(
        listed,
        ["l3"],
        "L2 compact ListActive must keep the running L3 id before that L3 reports up"
    );

    let _ = harness.finish_one.send("l3".to_owned());
    assert!(spawn_l3.await.unwrap().unwrap().success);
    let disposition = harness.completions.recv().await.expect("L3 done");
    assert!(
        !disposition.should_surface,
        "L3 completion reports to L2. L1 is not woken."
    );
    assert_eq!(
        buffered_completion_ids(&harness, Some("child")).await,
        ["l3"],
        "after L2 compact, the L3 completion report still buffers for that L2"
    );
    assert!(
        buffered_completion_ids(&harness, Some("parent"))
            .await
            .is_empty(),
        "discarded L3 context reports to L2. It does not compact into L1."
    );

    harness.actor.abort();
}
