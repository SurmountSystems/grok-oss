//! `/goal` when two subagents fail.
//!
//! Live card, issue 234, 2026-11-26: `Ran 2 subagents · 2 failed`, then
//! `Goal paused. No plan was produced. Retry with /goal resume.`
//! Both were the research role. The research notes were already written.
//! That is not the empty-file research-role sampling-window test.

use super::super::GoalSetupOutcome;
use super::super::planner_failure_pause_message;
use super::make_planner_actor;
use super::plan_path_from_prompt;
use super::research_role_sampling_window_session_error;
use std::sync::atomic::{AtomicUsize, Ordering as SeqOrd};
use std::sync::{Arc, Mutex};
use xai_grok_tools::implementations::grok_build::task::types::{SubagentEvent, SubagentResult};

/// Two failed `/goal` subagents, with research notes already on disk, must not
/// pause because no plan was produced. The goal keeps going.
#[tokio::test(flavor = "current_thread")]
async fn goal_two_failed_subagents_does_not_pause_with_no_plan_was_produced() {
    let pause = format!("Goal paused. {}", planner_failure_pause_message());
    assert_eq!(
        pause, "Goal paused. No plan was produced. Retry with /goal resume.",
        "the paused /goal outcome is this sentence"
    );
    let runtime = research_role_sampling_window_session_error();
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SubagentEvent>();
            let spawn_count = Arc::new(AtomicUsize::new(0));
            let count_task = Arc::clone(&spawn_count);
            let descriptions = Arc::new(Mutex::new(Vec::new()));
            let descriptions_task = Arc::clone(&descriptions);
            let research_files = Arc::new(Mutex::new(Vec::new()));
            let research_files_task = Arc::clone(&research_files);
            let runtime_task = runtime.clone();
            tokio::task::spawn_local(async move {
                while let Some(ev) = rx.recv().await {
                    let SubagentEvent::Spawn(req) = ev else {
                        continue;
                    };
                    count_task.fetch_add(1, SeqOrd::SeqCst);
                    descriptions_task
                        .lock()
                        .expect("descriptions")
                        .push(req.description.clone());
                    if req.prompt.contains("ROLE: research.") {
                        if let Some(path) = plan_path_from_prompt(&req.prompt) {
                            let file = std::path::PathBuf::from(&path);
                            if let Some(parent) = file.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(&file, b"research notes for this run\n");
                            research_files_task
                                .lock()
                                .expect("research files")
                                .push(path);
                        }
                    }
                    let result = SubagentResult {
                        success: false,
                        error: Some(runtime_task.clone()),
                        cancelled: false,
                        subagent_id: req.id.clone(),
                        child_session_id: req.id.clone(),
                        ..Default::default()
                    };
                    let _ = req.result_tx.send(result);
                }
            });

            let (actor, _tmp) = make_planner_actor(Some(tx), true).await;
            let plan_path = actor.goal_tracker.lock().plan_path();
            let outcome = actor
                .setup_goal(
                    "keep going through the list and use the eyes tool on this issue",
                    None,
                )
                .await;

            assert_eq!(
                spawn_count.load(SeqOrd::SeqCst),
                2,
                "the /goal card is two failed subagents, not one research stop"
            );
            let seen = descriptions.lock().expect("descriptions").clone();
            assert_eq!(
                seen,
                vec![
                    "goal research specialist".to_string(),
                    "goal research specialist".to_string(),
                ],
                "both failed subagents are the research role, not the plan writer: {seen:?}"
            );
            let files = research_files.lock().expect("research files").clone();
            assert_eq!(files.len(), 2, "each failed research session wrote notes");
            for path in &files {
                let body = std::fs::read_to_string(path).unwrap_or_default();
                assert!(
                    !body.trim().is_empty(),
                    "research notes must be on disk before the pause decision, got {path} {body:?}"
                );
                let name = std::path::Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy();
                assert!(
                    name.starts_with("research"),
                    "the written file is the research notes, not the plan: {name}"
                );
            }
            assert!(
                !plan_path.is_file() || std::fs::read(&plan_path).unwrap_or_default().is_empty(),
                "this path still has no plan file"
            );

            let GoalSetupOutcome::Inference { reminder } = outcome else {
                panic!("two failed subagents must keep the goal going, not return \"{pause}\"");
            };
            assert!(
                !reminder.contains("Goal paused. No plan was produced."),
                "the /goal outcome must not pause for a missing plan, got: {reminder}"
            );
            assert!(
                !reminder.contains("/goal resume"),
                "do not tell the Operator to /goal resume, got: {reminder}"
            );
            let snap = actor.goal_tracker.lock().snapshot().cloned().unwrap();
            assert_eq!(
                snap.status,
                crate::session::goal_tracker::GoalStatus::Active,
                "two failed subagents must not pause the goal"
            );
            assert!(
                snap.pause_message.is_none(),
                "must not store \"{}\", got {:?}",
                planner_failure_pause_message(),
                snap.pause_message
            );
            assert!(
                snap.plan_file.is_none(),
                "keeping going does not invent a plan"
            );
        })
        .await;
}
