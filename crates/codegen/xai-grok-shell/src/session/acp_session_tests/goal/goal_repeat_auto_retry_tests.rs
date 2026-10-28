//! Goal-turn repeating-sentence stop: auto-retry instead of `/goal resume`.

use super::support::*;
use super::*;
use std::sync::Arc as StdArc;
use tempfile::TempDir;
use xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE;

fn drain_agent_message_text(
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<SessionEvent>,
) -> Vec<String> {
    let mut texts = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        let SessionEvent::Notification(SessionNotification::Acp(n)) = event else {
            continue;
        };
        let acp::SessionUpdate::AgentMessageChunk(chunk) = n.update else {
            continue;
        };
        let acp::ContentBlock::Text(t) = chunk.content else {
            continue;
        };
        texts.push(t.text);
    }
    texts
}

fn queued_goal_summary(actor: &SessionActor) -> bool {
    actor
        .state
        .try_lock()
        .expect("uncontended test state")
        .pending_inputs
        .iter()
        .any(|item| {
            matches!(
                item.input_origin.as_prompt_origin(),
                crate::session::PromptOrigin::GoalSummary
            )
        })
}

async fn make_goal_actor_with_events() -> (
    StdArc<SessionActor>,
    TempDir,
    tokio::sync::mpsc::UnboundedReceiver<SessionEvent>,
) {
    let tmp = TempDir::new().expect("tempdir");
    let (gateway_tx, _gateway_rx) =
        tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
    let (persistence_tx, _persistence_rx) =
        tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
    let (mut actor, event_rx) =
        create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
    actor.events = crate::session::events::EventTracker::new(tmp.path());
    actor.goal_enabled = true;
    set_goal_harness_for_tests(&actor);
    actor.goal_tracker = StdArc::new(parking_lot::Mutex::new(
        crate::session::goal_tracker::GoalTracker::new(tmp.path().to_path_buf()),
    ));
    (StdArc::new(actor), tmp, event_rx)
}

fn start_test_goal(actor: &SessionActor) {
    actor.goal_tracker.lock().create_goal(
        "g-repeat-stop".into(),
        "land the repeating-sentence auto-retry".into(),
        None,
        0,
        "2026-10-20T00:00:00Z".into(),
        None,
    );
}

fn repeating_sentence_turn_err_message() -> String {
    format!("Turn failed: {REPETITIVE_GENERATION_USER_MESSAGE}")
}

#[tokio::test(flavor = "current_thread")]
async fn goal_turn_repeating_sentence_stop_auto_retries_without_resume_hint() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _tmp, mut event_rx) = make_goal_actor_with_events().await;
            start_test_goal(&actor);

            let paused = actor
                .apply_infra_pause_after_turn_err(repeating_sentence_turn_err_message())
                .await;

            assert!(
                !paused,
                "a repeating-sentence goal-turn stop must auto-retry instead of pausing"
            );
            assert_eq!(
                actor.goal_tracker.lock().status(),
                Some(crate::session::goal_tracker::GoalStatus::Active),
                "auto-retry must leave the goal Active so the Operator does not type /goal resume"
            );
            assert!(
                queued_goal_summary(&actor),
                "auto-retry must queue the same goal turn"
            );
            let slash = drain_agent_message_text(&mut event_rx);
            assert!(
                slash.iter().all(|text| !text.contains("/goal resume")),
                "must not emit the pause line that tells the Operator to /goal resume, got {slash:?}"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn other_turn_errors_still_pause_and_advertise_goal_resume() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let tmp = TempDir::new().expect("tempdir");
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.events = crate::session::events::EventTracker::new(tmp.path());
            actor.goal_enabled = true;
            set_goal_harness_for_tests(&actor);
            actor.goal_tracker = StdArc::new(parking_lot::Mutex::new(
                crate::session::goal_tracker::GoalTracker::new(tmp.path().to_path_buf()),
            ));
            start_test_goal(&actor);

            let paused = actor
                .apply_infra_pause_after_turn_err("Turn failed: rate limited".into())
                .await;

            assert!(paused, "other infra turn errors must still pause");
            assert_eq!(
                actor.goal_tracker.lock().status(),
                Some(crate::session::goal_tracker::GoalStatus::InfraPaused),
                "other turn errors must still infra-pause"
            );
            assert!(
                !queued_goal_summary(&actor),
                "a non-repeat infra pause must not auto-queue a goal turn"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn repeating_sentence_stop_pauses_after_auto_retry_cap() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let tmp = TempDir::new().expect("tempdir");
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.events = crate::session::events::EventTracker::new(tmp.path());
            actor.goal_enabled = true;
            set_goal_harness_for_tests(&actor);
            actor.goal_tracker = StdArc::new(parking_lot::Mutex::new(
                crate::session::goal_tracker::GoalTracker::new(tmp.path().to_path_buf()),
            ));
            start_test_goal(&actor);

            for attempt in 0..GOAL_REPEAT_STOP_AUTO_RETRY_CAP {
                let paused = actor
                    .apply_infra_pause_after_turn_err(repeating_sentence_turn_err_message())
                    .await;
                assert!(
                    !paused,
                    "attempt {attempt} under cap {GOAL_REPEAT_STOP_AUTO_RETRY_CAP} must auto-retry"
                );
            }

            let paused = actor
                .apply_infra_pause_after_turn_err(repeating_sentence_turn_err_message())
                .await;
            assert!(
                paused,
                "after {GOAL_REPEAT_STOP_AUTO_RETRY_CAP} auto-retries the goal must pause as today"
            );
            assert_eq!(
                actor.goal_tracker.lock().status(),
                Some(crate::session::goal_tracker::GoalStatus::InfraPaused),
                "the cap must fall through to today's infra pause"
            );
        })
        .await;
}

/// #228 queued a continuation then the run loop still called `handle_turn_end`
/// with `turn_succeeded=false`, which incremented the non-completing streak
/// and would reset the repeat-stop cap on a later success path mix-up.
/// Production now skips `handle_turn_end` on auto-retry.
#[tokio::test(flavor = "current_thread")]
async fn repeating_sentence_auto_retry_does_not_increment_non_completing_streak() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _tmp, mut event_rx) = make_goal_actor_with_events().await;
            start_test_goal(&actor);

            actor
                .apply_goal_turn_infra_then_maybe_turn_end(
                    Some(repeating_sentence_turn_err_message()),
                    false,
                    false,
                )
                .await;

            assert_eq!(
                actor
                    .goal_continuation_streak
                    .load(std::sync::atomic::Ordering::Relaxed),
                0,
                "auto-retry must not count as a non-completing turn"
            );
            assert_eq!(
                actor
                    .goal_repeat_stop_retries
                    .load(std::sync::atomic::Ordering::Relaxed),
                1,
                "the repeat-stop cap must still advance"
            );
            assert_eq!(
                actor.goal_tracker.lock().status(),
                Some(crate::session::goal_tracker::GoalStatus::Active)
            );
            assert!(queued_goal_summary(&actor));
            let slash = drain_agent_message_text(&mut event_rx);
            assert!(
                slash.iter().all(|text| !text.contains("/goal resume")),
                "must not emit /goal resume on auto-retry, got {slash:?}"
            );
        })
        .await;
}
