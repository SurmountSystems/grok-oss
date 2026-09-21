//! GitHub #143: L1 follow-up to a running L2.
//!
//! Named tests are contracts. Quote the Operator. Do not fit tests to code.

use super::*;
use crate::implementations::grok_build::task::types::SubagentFollowUpOutcome;

/// Operator: "You can't talk to your own L2s? And you're fine with that? Why?"
#[tokio::test]
async fn parent_cannot_talk_to_own_l2s_follow_up_enqueues_interject_on_running_l2_without_kill_or_respawn()
 {
    let mut harness = harness(false, std::time::Duration::from_secs(60));
    let spawn = tokio::spawn({
        let backend = harness.backend.clone();
        async move { backend.spawn(request("l2", true)).await }
    });
    let started = harness.started.recv().await.unwrap();
    assert_eq!(started, "l2");
    let _spawn_req = harness.requests.recv().await.unwrap();

    let running = harness.backend.query("l2", false, None).await.unwrap();
    assert!(
        running.is_running(),
        "follow-up target must still be a running L2"
    );

    let outcome = parent_backend(&harness)
        .follow_up("l2", "additive follow-up, not kill, not respawn")
        .await;
    assert_eq!(
        outcome,
        SubagentFollowUpOutcome::Queued {
            child_session_id: "l2".to_owned(),
        },
        "parent follow-up must enqueue SessionCommand::Interject on the child session and return queued"
    );

    let delivered = harness
        .follow_ups
        .recv()
        .await
        .expect("Interject delivered");
    assert_eq!(delivered.child_session_id, "l2");
    assert_eq!(delivered.text, "additive follow-up, not kill, not respawn");

    let still = harness.backend.query("l2", false, None).await.unwrap();
    assert!(still.is_running(), "follow-up must not kill the running L2");
    assert_eq!(
        harness.backend.registry_counts().await,
        SubagentRegistryCounts {
            pending: 0,
            active: 1,
            completed: 0,
            queued: 0,
        },
        "follow-up must not spawn a second session"
    );
    assert!(
        harness.requests.try_recv().is_err(),
        "follow-up must not emit a second spawn request"
    );

    let _ = harness.finish.send(());
    assert!(spawn.await.unwrap().unwrap().success);
    harness.actor.abort();
}

#[tokio::test]
async fn parent_follow_up_does_not_inject_into_live_l3_unless_operator_targeted_that_specialist() {
    let mut harness = harness(false, std::time::Duration::from_secs(60));
    let spawn_l2 = tokio::spawn({
        let backend = harness.backend.clone();
        async move { backend.spawn(request("l2", true)).await }
    });
    assert_eq!(harness.started.recv().await.unwrap(), "l2");
    let _ = harness.requests.recv().await.unwrap();

    let mut l3 = request("l3", true);
    // Reparented specialist: limits may pin parent_session_id to the root,
    // but immediate_parent_session_id / spawn_depth mark this as L3.
    l3.runtime_overrides.immediate_parent_session_id = Some("l2".to_owned());
    l3.runtime_overrides.spawn_depth = Some(2);
    let spawn_l3 = tokio::spawn({
        let backend = harness.backend.clone();
        async move { backend.spawn(l3).await }
    });
    assert_eq!(harness.started.recv().await.unwrap(), "l3");
    let _ = harness.requests.recv().await.unwrap();

    let outcome = parent_backend(&harness)
        .follow_up("l3", "do not barge into the specialist")
        .await;
    assert_eq!(
        outcome,
        SubagentFollowUpOutcome::LiveL3Unbothered,
        "parent-tool follow-up must refuse a live L3 unless the Operator targeted that specialist"
    );
    assert!(
        harness.follow_ups.try_recv().is_err(),
        "refused L3 follow-up must not enqueue Interject on the specialist session"
    );

    let l3_still = harness.backend.query("l3", false, None).await.unwrap();
    assert!(l3_still.is_running());
    let l2_still = harness.backend.query("l2", false, None).await.unwrap();
    assert!(l2_still.is_running());

    let _ = harness.finish.send(());
    assert!(spawn_l2.await.unwrap().unwrap().success);
    assert!(spawn_l3.await.unwrap().unwrap().success);
    harness.actor.abort();
}

#[tokio::test]
async fn resume_from_of_running_l2_still_fails_active() {
    let mut harness = harness(false, std::time::Duration::from_secs(60));
    let spawn = tokio::spawn({
        let backend = harness.backend.clone();
        async move { backend.spawn(request("l2", true)).await }
    });
    assert_eq!(harness.started.recv().await.unwrap(), "l2");
    let _ = harness.requests.recv().await.unwrap();

    let mut resume = request("resume-l2", true);
    resume.resume_from = Some("l2".to_owned());
    let result = harness
        .backend
        .spawn(resume)
        .await
        .expect("spawn result channel");
    assert!(!result.success);
    assert_eq!(
        result.error.as_deref(),
        Some(
            "Cannot resume from subagent 'l2': it is still running. \
             Wait for it to complete before resuming."
        ),
        "resume_from on an Active L2 stays the existing fail-closed error"
    );
    assert!(
        harness
            .backend
            .query("l2", false, None)
            .await
            .unwrap()
            .is_running(),
        "failed resume_from must leave the running L2 alive"
    );

    let _ = harness.finish.send(());
    assert!(spawn.await.unwrap().unwrap().success);
    harness.actor.abort();
}

#[tokio::test]
async fn parent_follow_up_off_is_upstream_spawn_wait_resume_from_completed_only() {
    let mut harness = harness_with_config(
        false,
        CoordinatorConfig {
            parent_follow_up: false,
            ..CoordinatorConfig::default()
        },
    );
    let spawn = tokio::spawn({
        let backend = harness.backend.clone();
        async move { backend.spawn(request("l2", true)).await }
    });
    assert_eq!(harness.started.recv().await.unwrap(), "l2");
    let _ = harness.requests.recv().await.unwrap();

    let outcome = parent_backend(&harness)
        .follow_up("l2", "upstream has no live-L2 parent-tool enqueue")
        .await;
    assert_eq!(
        outcome,
        SubagentFollowUpOutcome::Disabled,
        "[subagents] parent_follow_up = false is SpaceXAI spawn/wait/resume_from completed-only"
    );
    assert!(
        harness.follow_ups.try_recv().is_err(),
        "off must not enqueue Interject"
    );
    assert!(
        harness
            .backend
            .query("l2", false, None)
            .await
            .unwrap()
            .is_running()
    );

    let _ = harness.finish.send(());
    assert!(spawn.await.unwrap().unwrap().success);
    harness.actor.abort();
}

/// Operator: "You can't talk to your own L2s? And you're fine with that? Why?"
/// Follow-up must not inject into a live L3 unless the Operator targeted that specialist.
#[tokio::test]
async fn parent_follow_up_onto_running_l2_with_live_l3_hits_l2_not_l3() {
    let mut harness = harness(false, std::time::Duration::from_secs(60));
    let spawn_l2 = tokio::spawn({
        let backend = harness.backend.clone();
        async move { backend.spawn(request("l2", true)).await }
    });
    assert_eq!(harness.started.recv().await.unwrap(), "l2");
    let _ = harness.requests.recv().await.unwrap();

    let mut l3 = request("l3", true);
    l3.runtime_overrides.immediate_parent_session_id = Some("l2".to_owned());
    l3.runtime_overrides.spawn_depth = Some(2);
    let spawn_l3 = tokio::spawn({
        let backend = harness.backend.clone();
        async move { backend.spawn(l3).await }
    });
    assert_eq!(harness.started.recv().await.unwrap(), "l3");
    let _ = harness.requests.recv().await.unwrap();

    let outcome = parent_backend(&harness)
        .follow_up("l2", "additive follow-up stays on the running L2")
        .await;
    assert_eq!(
        outcome,
        SubagentFollowUpOutcome::Queued {
            child_session_id: "l2".to_owned(),
        },
        "follow-up onto a running L2 that already has a live L3 must enqueue on the L2"
    );

    let delivered = harness
        .follow_ups
        .recv()
        .await
        .expect("Interject delivered");
    assert_eq!(delivered.child_session_id, "l2");
    assert_eq!(delivered.text, "additive follow-up stays on the running L2");
    assert!(
        harness.follow_ups.try_recv().is_err(),
        "follow-up must not enqueue Interject on the live L3"
    );

    let l2_still = harness.backend.query("l2", false, None).await.unwrap();
    assert!(
        l2_still.is_running(),
        "follow-up must not kill the running L2"
    );
    let l3_still = harness.backend.query("l3", false, None).await.unwrap();
    assert!(
        l3_still.is_running(),
        "follow-up onto the L2 must not touch the live L3"
    );
    assert_eq!(
        harness.backend.registry_counts().await,
        SubagentRegistryCounts {
            pending: 0,
            active: 2,
            completed: 0,
            queued: 0,
        },
        "follow-up must not spawn a second L2 session"
    );

    let _ = harness.finish.send(());
    assert!(spawn_l2.await.unwrap().unwrap().success);
    assert!(spawn_l3.await.unwrap().unwrap().success);
    harness.actor.abort();
}
