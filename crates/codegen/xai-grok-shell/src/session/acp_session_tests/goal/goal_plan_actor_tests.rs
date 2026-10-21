//! Actor-held GoalPlan survives setup_goal / GoalClear / restore.
//! Throwaway `GoalPlan::new()` per call is the remaining live `/goal` miss.

use super::support::*;
use super::*;
use crate::session::goal_plan::NO_CURRENT_GOALS;
use crate::session::goal_tracker::make_base_orchestration;
use crate::session::persistence::PersistenceMsg;
use std::sync::Arc as StdArc;
use tempfile::TempDir;
use xai_grok_tools::implementations::grok_build::task::types::{SubagentEvent, SubagentResult};

fn reply_has_no_current_goals(text: &str) -> bool {
    text.to_ascii_lowercase().contains(NO_CURRENT_GOALS)
}

fn spawn_delete_goal_mode_ack(
    mut persistence_rx: tokio::sync::mpsc::UnboundedReceiver<PersistenceMsg>,
) {
    tokio::task::spawn_local(async move {
        while let Some(msg) = persistence_rx.recv().await {
            if let PersistenceMsg::DeleteGoalModeState { respond_to } = msg {
                let _ = respond_to.send(Ok(()));
            }
        }
    });
}

fn spawn_host_line_drain(
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<SessionEvent>,
) -> StdArc<std::sync::Mutex<Vec<String>>> {
    let lines = StdArc::new(std::sync::Mutex::new(Vec::new()));
    let lines_task = StdArc::clone(&lines);
    tokio::task::spawn_local(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                SessionEvent::FlushReplay {
                    respond_to: Some(tx),
                } => {
                    let _ = tx.send(());
                }
                SessionEvent::Notification(SessionNotification::Acp(n)) => {
                    if let acp::SessionUpdate::AgentMessageChunk(chunk) = n.update
                        && let acp::ContentBlock::Text(t) = chunk.content
                    {
                        lines_task.lock().unwrap().push(t.text);
                    }
                }
                _ => {}
            }
        }
    });
    lines
}

/// Planner coordinator that writes `{PLAN_FILE}` then replies Done.
fn spawn_write_plan_then_done() -> tokio::sync::mpsc::UnboundedSender<SubagentEvent> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SubagentEvent>();
    tokio::task::spawn_local(async move {
        while let Some(ev) = rx.recv().await {
            if let SubagentEvent::Spawn(req) = ev {
                let plan_path = req
                    .prompt
                    .split('`')
                    .skip(1)
                    .step_by(2)
                    .find(|token| token.ends_with(".md"))
                    .map(str::to_owned);
                if let Some(p) = &plan_path {
                    let _ = std::fs::create_dir_all(std::path::Path::new(p).parent().unwrap());
                    let _ = std::fs::write(p, b"# Plan\n");
                }
                let subagent_id = req.id.clone();
                let _ = req.result_tx.send(SubagentResult {
                    success: true,
                    output: StdArc::from("Done"),
                    subagent_id: subagent_id.clone(),
                    child_session_id: subagent_id,
                    ..Default::default()
                });
            }
        }
    });
    tx
}

fn assert_no_current_goals(texts: &[String], what: &str) {
    for text in texts {
        assert!(
            !reply_has_no_current_goals(text),
            "Goals are still broken: /goal still replies no current goals. \
             {what} must not reply {NO_CURRENT_GOALS:?}, got {texts:?}"
        );
    }
}

/// Operator 2026-10-21: Goals are still broken. An actor-held GoalPlan must
/// survive setup_goal / GoalClear / restore so `/goal` does not reply
/// "no current goals" and a PLAN_FILE stop stays Planned.
#[tokio::test(flavor = "current_thread")]
async fn actor_held_goal_plan_survives_setup_goal_clear_and_restore() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let tmp = TempDir::new().expect("tempdir");
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            spawn_delete_goal_mode_ack(persistence_rx);
            let (mut actor, event_rx) =
                create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.events = crate::session::events::EventTracker::new(tmp.path());
            actor.goal_enabled = true;
            set_goal_harness_for_tests(&actor);
            actor.goal_planner_enabled = false;
            actor.goal_tracker = StdArc::new(parking_lot::Mutex::new(
                crate::session::goal_tracker::GoalTracker::new(tmp.path().to_path_buf()),
            ));
            let host_lines = spawn_host_line_drain(event_rx);

            let outcome = actor.setup_goal("ship the parser", None).await;
            let reminder = match &outcome {
                GoalSetupOutcome::Inference { reminder } => reminder.as_str(),
                GoalSetupOutcome::Message(msg) => msg.as_str(),
            };
            assert!(
                !reply_has_no_current_goals(reminder),
                "Goals are still broken: /goal still replies no current goals. \
                 actor-held setup_goal must not reply {NO_CURRENT_GOALS:?}, got {:?}",
                reminder
            );
            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some("ship the parser"),
                "setup_goal must store the objective on the actor-held GoalPlan"
            );
            let snapshot = actor.goal_plan.lock().snapshot();
            assert_eq!(snapshot.goals, vec!["ship the parser".to_string()]);

            actor.goal_planner_enabled = true;
            actor.tool_context.subagent_event_tx = Some(spawn_write_plan_then_done());
            let actor = StdArc::new(actor);
            actor.maybe_run_goal_planner("ship the parser").await;
            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some("ship the parser"),
                "Goals are still broken: PLAN_FILE persist must not drop the actor-held GoalPlan. \
                 a PLAN_FILE stop stays Planned when restore keeps the goals"
            );
            let snap = actor.goal_tracker.lock().snapshot().cloned().unwrap();
            assert_eq!(
                snap.status,
                crate::session::goal_tracker::GoalStatus::Active,
                "a PLAN_FILE stop stays Planned"
            );
            assert!(
                snap.plan_file.is_some(),
                "maybe_run_goal_planner persist must publish PLAN_FILE"
            );

            let _ = actor
                .execute_builtin_slash_command(BuiltinAction::GoalClear)
                .await;
            assert!(
                actor.goal_plan.lock().primary_goal().is_none(),
                "GoalClear must give_up the same actor-held GoalPlan"
            );
            assert_no_current_goals(&host_lines.lock().unwrap(), "actor-held GoalClear");

            actor.goal_plan.lock().restore(snapshot.clone());
            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some("ship the parser"),
                "restore must rehydrate the actor-held GoalPlan, not a throwaway"
            );

            let _ = actor
                .execute_builtin_slash_command(BuiltinAction::GoalClear)
                .await;
            assert!(
                actor.goal_plan.lock().primary_goal().is_none(),
                "GoalClear after restore must give_up the same actor-held GoalPlan"
            );
            assert_no_current_goals(&host_lines.lock().unwrap(), "give_up after restore");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn restored_orchestration_stores_goal_plan_on_the_actor() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            let orchestration = make_base_orchestration();
            *actor.goal_plan.lock() =
                SessionActor::goal_plan_from_restored_orchestration(Some(&orchestration));
            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some("test objective"),
                "session restore must store GoalPlan on the actor from the persisted objective"
            );
            assert_eq!(actor.goal_plan.lock().post_plan_retry_count(), 0);
            *actor.goal_plan.lock() = SessionActor::goal_plan_from_restored_orchestration(None);
            assert!(
                actor.goal_plan.lock().primary_goal().is_none(),
                "a session with no persisted goal must start with an empty actor-held GoalPlan"
            );
        })
        .await;
}
