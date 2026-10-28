//! Actor-held GoalPlan survives setup_goal / GoalClear / restore.
//! Throwaway `GoalPlan::new()` per call is the remaining live `/goal` miss.

use super::support::*;
use super::*;
use crate::session::goal_plan::NO_CURRENT_GOALS;
use crate::session::goal_tracker::{
    GoalEvent, GoalHistoryEntry, GoalStatus, make_base_orchestration,
};
use crate::session::persistence::PersistenceMsg;
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicUsize, Ordering as SeqOrd};
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
    spawn_write_plan_body_then_done(b"# Plan\n")
}

/// Planner coordinator that writes `body` to `{PLAN_FILE}` then replies Done.
fn spawn_write_plan_body_then_done(
    body: &'static [u8],
) -> tokio::sync::mpsc::UnboundedSender<SubagentEvent> {
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
                    let _ = std::fs::write(p, body);
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

/// Like [`spawn_write_plan_then_done`], plus a spawn counter so a same-prompt
/// re-run can prove the planner did not start from scratch.
fn spawn_write_plan_then_done_counting() -> (
    tokio::sync::mpsc::UnboundedSender<SubagentEvent>,
    StdArc<AtomicUsize>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SubagentEvent>();
    let spawn_count = StdArc::new(AtomicUsize::new(0));
    let count_task = StdArc::clone(&spawn_count);
    tokio::task::spawn_local(async move {
        while let Some(ev) = rx.recv().await {
            if let SubagentEvent::Spawn(req) = ev {
                count_task.fetch_add(1, SeqOrd::SeqCst);
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
    (tx, spawn_count)
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

/// Operator 2026-10-21: "When I scrolled up and re-ran the goal prompt, goals are
/// fixed, it's just that your fix also reset the earlier goal."
/// An active GoalPlan with tracker progress, then `setup_goal` / `/goal` with the
/// same prompt, must resume that goal: no "Goal created", progress items stay, no
/// new empty GoalPlan.
#[tokio::test(flavor = "current_thread")]
async fn setup_goal_same_prompt_resumes_earlier_goal_instead_of_resetting() {
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
            let _host_lines = spawn_host_line_drain(event_rx);

            const OBJECTIVE: &str = "We need to update to Lean 4.35, in parallel, using our seven team mates and Hunter as a watcher.";
            let first = actor.setup_goal(OBJECTIVE, None).await;
            match &first {
                GoalSetupOutcome::Inference { reminder } => {
                    assert!(
                        !reply_has_no_current_goals(reminder),
                        "first setup_goal must not reply {NO_CURRENT_GOALS:?}, got {reminder:?}"
                    );
                }
                GoalSetupOutcome::Message(msg) => panic!("first setup_goal must start inference, got {msg:?}"),
            }
            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some(OBJECTIVE),
                "setup_goal must store the objective on the actor-held GoalPlan"
            );

            let (goal_id, created_at, created_count) = {
                let mut tracker = actor.goal_tracker.lock();
                let plan_path = tracker.plan_path();
                {
                    let o = tracker.snapshot_mut().expect("active goal after setup");
                    o.plan_file = Some(plan_path);
                    o.total_worker_rounds = 3;
                    o.tokens_used_high_water = 1200;
                }
                tracker.append_history(GoalHistoryEntry {
                    timestamp: "2026-10-21T00:00:00Z".into(),
                    event: GoalEvent::WorkerCompleted,
                    detail: Some("earlier progress".into()),
                    round: Some(3),
                    tokens_used: Some(1200),
                    unmet: vec!["keep the earlier tracker progress".into()],
                });
                let snap = tracker.snapshot().expect("active goal after progress").clone();
                assert_eq!(snap.status, GoalStatus::Active);
                let created = snap
                    .history
                    .iter()
                    .filter(|e| matches!(e.event, GoalEvent::GoalCreated))
                    .count();
                (snap.goal_id, snap.created_at, created)
            };
            assert_eq!(created_count, 1, "first setup_goal records one Goal created");

            let (planner_tx, spawn_count) = spawn_write_plan_then_done_counting();
            actor.goal_planner_enabled = true;
            actor.tool_context.subagent_event_tx = Some(planner_tx);

            let second = actor.setup_goal(OBJECTIVE, None).await;
            match &second {
                GoalSetupOutcome::Inference { reminder } => {
                    assert!(
                        !reply_has_no_current_goals(reminder),
                        "When I scrolled up and re-ran the goal prompt, goals are fixed, it's just that your fix also reset the earlier goal. \
                         same-prompt setup_goal must not reply {NO_CURRENT_GOALS:?}, got {reminder:?}"
                    );
                }
                GoalSetupOutcome::Message(msg) => {
                    panic!(
                        "When I scrolled up and re-ran the goal prompt, goals are fixed, it's just that your fix also reset the earlier goal. \
                         same-prompt setup_goal must resume inference, got {msg:?}"
                    );
                }
            }

            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some(OBJECTIVE),
                "When I scrolled up and re-ran the goal prompt, goals are fixed, it's just that your fix also reset the earlier goal. \
                 same prompt must keep the earlier GoalPlan, not a new empty one"
            );
            assert!(
                !actor.goal_plan.lock().goals().is_empty(),
                "same prompt must not replace the actor-held GoalPlan with an empty one"
            );

            let snap = actor.goal_tracker.lock().snapshot().cloned().unwrap();
            assert_eq!(
                snap.goal_id, goal_id,
                "same prompt must resume the earlier goal id, not create a new GoalPlan"
            );
            assert_eq!(snap.created_at, created_at, "same prompt must not mint a new created_at");
            assert_eq!(snap.status, GoalStatus::Active);
            assert_eq!(
                snap.total_worker_rounds, 3,
                "progress items stay: worker rounds must survive the same-prompt re-run"
            );
            assert_eq!(snap.tokens_used_high_water, 1200);
            let created_after = snap
                .history
                .iter()
                .filter(|e| matches!(e.event, GoalEvent::GoalCreated))
                .count();
            assert_eq!(
                created_after, created_count,
                "When I scrolled up and re-ran the goal prompt, goals are fixed, it's just that your fix also reset the earlier goal. \
                 same prompt must not paint Goal created"
            );
            let progress = snap.history.iter().find(|e| {
                matches!(e.event, GoalEvent::WorkerCompleted)
                    && e.unmet.iter().any(|item| item == "keep the earlier tracker progress")
            });
            assert!(
                progress.is_some(),
                "progress items stay: WorkerCompleted unmet must survive the same-prompt re-run, got {:?}",
                snap.history
                    .iter()
                    .map(|e| (format!("{:?}", e.event), e.unmet.clone()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                spawn_count.load(SeqOrd::SeqCst),
                0,
                "same prompt must not start the planner from scratch"
            );
        })
        .await;
}

/// Operator: A just-created goal (`Goal created just now`, 0 tokens) must not
/// paint old mill Progress checkmarks (`+332 more` from a prior Lean 4.35 walk)
/// as if they were this planning turn. Stale tracker history must not masquerade
/// as current Progress.
/// This is not the `/goal` same-prompt resume miss: a different new objective
/// must not inherit mill tracker history. Same-prompt resume stays in
/// [`setup_goal_same_prompt_resumes_earlier_goal_instead_of_resetting`].
#[tokio::test(flavor = "current_thread")]
async fn setup_goal_new_objective_does_not_carry_prior_mill_progress() {
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
            let _host_lines = spawn_host_line_drain(event_rx);

            const MILL: &str = "We need to update to Lean 4.35, in parallel, using our seven team mates and Hunter as a watcher.";
            const NEW_OBJECTIVE: &str = "Plan this planning turn";
            let first = actor.setup_goal(MILL, None).await;
            match &first {
                GoalSetupOutcome::Inference { reminder } => {
                    assert!(
                        !reply_has_no_current_goals(reminder),
                        "first mill setup_goal must not reply {NO_CURRENT_GOALS:?}, got {reminder:?}"
                    );
                }
                GoalSetupOutcome::Message(msg) => {
                    panic!("first mill setup_goal must start inference, got {msg:?}")
                }
            }

            let mill_progress: Vec<String> = (0..347)
                .map(|i| format!("Lean 4.35 mill walk {i}"))
                .collect();
            {
                let mut tracker = actor.goal_tracker.lock();
                {
                    let o = tracker.snapshot_mut().expect("active mill goal after setup");
                    o.total_worker_rounds = 3;
                    o.tokens_used_high_water = 1200;
                }
                tracker.append_history(GoalHistoryEntry {
                    timestamp: "2026-10-21T00:00:00Z".into(),
                    event: GoalEvent::WorkerCompleted,
                    detail: Some("prior Lean 4.35 walk".into()),
                    round: Some(3),
                    tokens_used: Some(1200),
                    unmet: mill_progress.clone(),
                });
            }
            actor
                .goal_plan
                .lock()
                .record_progress_items(mill_progress.clone());
            assert_eq!(actor.goal_plan.lock().progress_items().len(), 347);

            let created = actor.setup_goal(NEW_OBJECTIVE, None).await;
            match &created {
                GoalSetupOutcome::Inference { reminder } => {
                    assert!(
                        !reply_has_no_current_goals(reminder),
                        "new just-created setup_goal must not reply {NO_CURRENT_GOALS:?}, got {reminder:?}"
                    );
                }
                GoalSetupOutcome::Message(msg) => {
                    panic!("new just-created setup_goal must start inference, got {msg:?}")
                }
            }

            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some(NEW_OBJECTIVE),
                "a different new objective must replace the mill GoalPlan"
            );
            assert!(
                actor.goal_plan.lock().progress_items().is_empty(),
                "A just-created goal (`Goal created just now`, 0 tokens) must not paint old mill Progress checkmarks (`+332 more` from a prior Lean 4.35 walk) as if they were this planning turn. Stale tracker history must not masquerade as current Progress. got {:?}",
                actor.goal_plan.lock().progress_items()
            );

            let snap = actor.goal_tracker.lock().snapshot().cloned().unwrap();
            assert_eq!(snap.objective, NEW_OBJECTIVE);
            assert_eq!(snap.tokens_used_high_water, 0);
            assert_eq!(snap.total_worker_rounds, 0);
            let created_count = snap
                .history
                .iter()
                .filter(|e| matches!(e.event, GoalEvent::GoalCreated))
                .count();
            assert_eq!(created_count, 1, "just-created goal records Goal created");
            let mill_leftover = snap.history.iter().any(|e| {
                e.unmet.iter().any(|item| item.contains("Lean 4.35 mill walk"))
                    || e.detail
                        .as_deref()
                        .is_some_and(|d| d.contains("prior Lean 4.35 walk"))
            });
            assert!(
                !mill_leftover,
                "A just-created goal (`Goal created just now`, 0 tokens) must not paint old mill Progress checkmarks (`+332 more` from a prior Lean 4.35 walk) as if they were this planning turn. Stale tracker history must not masquerade as current Progress. got {:?}",
                snap.history
                    .iter()
                    .map(|e| (format!("{:?}", e.event), e.detail.clone(), e.unmet.clone()))
                    .collect::<Vec<_>>()
            );
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

fn setup_goal_reminder(outcome: &GoalSetupOutcome) -> &str {
    match outcome {
        GoalSetupOutcome::Inference { reminder } => reminder.as_str(),
        GoalSetupOutcome::Message(msg) => msg.as_str(),
    }
}

/// Operator 2026-10-21: "Resizing performance is fixed. The goal got lost here though."
/// After `maybe_run_goal_planner` returns Planned with PLAN_FILE on disk, a later
/// `setup_goal` / `/goal` reminder must name the current planned work, not reprint
/// the original complaint as if the plan never happened.
#[tokio::test(flavor = "current_thread")]
async fn after_planned_plan_file_later_goal_reminder_names_the_current_planned_work_not_the_original_complaint()
 {
    const ORIGINAL_COMPLAINT: &str =
        "Goal planning is still entirely broken after the claimed fix.";
    const PLANNED_WORK: &str = "Rewrite tests onto production GoalPlan arms";
    const PLAN_BODY: &[u8] =
        b"# Plan: Rewrite tests onto production GoalPlan arms\n\n## Goal kind\ncode-change\n";

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let tmp = TempDir::new().expect("tempdir");
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            spawn_delete_goal_mode_ack(persistence_rx);
            let (mut actor, _event_rx) =
                create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.events = crate::session::events::EventTracker::new(tmp.path());
            actor.goal_enabled = true;
            set_goal_harness_for_tests(&actor);
            actor.goal_planner_enabled = true;
            actor.goal_tracker = StdArc::new(parking_lot::Mutex::new(
                crate::session::goal_tracker::GoalTracker::new(tmp.path().to_path_buf()),
            ));
            actor.tool_context.subagent_event_tx = Some(spawn_write_plan_body_then_done(PLAN_BODY));

            let first = actor.setup_goal(ORIGINAL_COMPLAINT, None).await;
            let first_reminder = setup_goal_reminder(&first);
            assert!(
                matches!(first, GoalSetupOutcome::Inference { .. }),
                "after Planned with PLAN_FILE, setup_goal must seed a Goal reminder, got {first_reminder:?}"
            );
            assert!(
                actor.goal_tracker.lock().snapshot().is_some_and(|o| {
                    o.status == crate::session::goal_tracker::GoalStatus::Active
                        && o.plan_file.is_some()
                }),
                "maybe_run_goal_planner must stay Planned with PLAN_FILE on disk"
            );
            let first_goal_id = actor
                .goal_tracker
                .lock()
                .snapshot()
                .map(|o| o.goal_id.clone())
                .expect("planned goal id");
            assert!(
                first_reminder.contains(&format!("A goal has been set: {PLANNED_WORK}")),
                "Operator: the goal got lost. After Planned with PLAN_FILE, the Goal reminder \
                 must name the current planned work {PLANNED_WORK:?}, got {first_reminder:?}"
            );
            assert!(
                !first_reminder.contains(&format!("A goal has been set: {ORIGINAL_COMPLAINT}")),
                "Operator: the goal got lost. After Planned with PLAN_FILE, the Goal reminder \
                 must not reprint the original complaint as if the plan never happened, got {first_reminder:?}"
            );

            let later = actor.setup_goal(ORIGINAL_COMPLAINT, None).await;
            let later_reminder = setup_goal_reminder(&later);
            assert!(
                matches!(later, GoalSetupOutcome::Inference { .. }),
                "a later /goal after Planned must paint a Goal reminder, got {later_reminder:?}"
            );
            assert_eq!(
                actor.goal_plan.lock().primary_goal(),
                Some(PLANNED_WORK),
                "Operator: the goal got lost. A later /goal must keep the current planned work, \
                 not replace it with the original complaint"
            );
            let later_snap = actor
                .goal_tracker
                .lock()
                .snapshot()
                .cloned()
                .expect("later planned goal");
            assert_eq!(
                later_snap.goal_id, first_goal_id,
                "Operator: the live goal was lost. A later /goal must not create_goal over the Planned PLAN_FILE"
            );
            assert!(
                later_snap.plan_file.is_some(),
                "a later /goal must keep PLAN_FILE; wiping it is the live goal being lost"
            );
            assert_eq!(later_snap.objective, PLANNED_WORK);
            assert!(
                later_reminder.contains(&format!("A goal has been set: {PLANNED_WORK}")),
                "Operator: the goal got lost. A later /goal reminder must name the current \
                 planned work {PLANNED_WORK:?}, not reprint the original complaint, got {later_reminder:?}"
            );
            assert!(
                !later_reminder.contains(&format!("A goal has been set: {ORIGINAL_COMPLAINT}")),
                "Operator: the goal got lost. A later /goal reminder must not reprint the \
                 original complaint-only Goal reminder as if the plan never happened, got {later_reminder:?}"
            );
            assert!(
                !reply_has_no_current_goals(later_reminder),
                "Goals are still broken: /goal still replies no current goals. \
                 later reminder must not reply {NO_CURRENT_GOALS:?}, got {later_reminder:?}"
            );
        })
        .await;
}
