//! `/goal` for Nix reflection and justfile lockfile utilities.
//!
//! Talents session, issue 234, 2026-11-27. The Operator asked for that work
//! in a session that has not compacted. After two subagents the product
//! painted `Goal paused. No plan was produced. Retry with /goal resume.`
//! The product reads accumulated tokens from `get_total_tokens`, not the
//! live `195K / 256K` context figure. The screenshot accumulated total is
//! 345400. Compaction count is 0. The harness must not write the plan.

use super::super::GoalSetupOutcome;
use super::make_planner_actor;
use super::plan_path_from_prompt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as SeqOrd};
use std::sync::{Arc, Mutex};
use xai_grok_tools::implementations::grok_build::task::types::{SubagentEvent, SubagentResult};

const OPERATOR_GOAL: &str = "Made some changes and updates. Make sure those are reflected in Nix. Also please make sure that the justfile has good utilities to help me check, maintain, and update all lockfiles and flakes.";

const PAUSE: &str = "Goal paused. No plan was produced. Retry with /goal resume.";

/// Accumulated tokens from the screenshot (`345.4k`). `get_total_tokens` is
/// that accumulated total. It is not the `195K / 256K` context line.
const ACCUMULATED_TOKENS: u64 = 345_400;

/// The session has not compacted. Accumulated tokens are 345400. The Operator
/// asked to reflect changes in Nix and to give the justfile utilities that
/// check, maintain, and update lockfiles and flakes. Research notes are on
/// disk. Recon returns without a recon file. That is two subagents and no
/// plan yet. The harness does not write a plan. The product must write one
/// and must not store "No plan was produced."
#[tokio::test(flavor = "current_thread")]
async fn nix_and_justfile_lockfile_goal_does_not_pause_with_no_plan_was_produced() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SubagentEvent>();
            let spawn_count = Arc::new(AtomicUsize::new(0));
            let count_task = Arc::clone(&spawn_count);
            let descriptions = Arc::new(Mutex::new(Vec::new()));
            let descriptions_task = Arc::clone(&descriptions);
            let recon_files = Arc::new(Mutex::new(Vec::new()));
            let recon_files_task = Arc::clone(&recon_files);
            let fork_flags = Arc::new(Mutex::new(Vec::new()));
            let fork_flags_task = Arc::clone(&fork_flags);
            let harness_wrote_plan = Arc::new(AtomicBool::new(false));
            let harness_wrote_plan_task = Arc::clone(&harness_wrote_plan);
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
                    fork_flags_task
                        .lock()
                        .expect("fork flags")
                        .push(req.fork_context);
                    let path = plan_path_from_prompt(&req.prompt);
                    if req.description == "goal research specialist" {
                        if let Some(path) = path {
                            let file = std::path::PathBuf::from(&path);
                            if let Some(parent) = file.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(
                                &file,
                                b"Nix and justfile lockfile notes for this run\n",
                            );
                        }
                    } else if req.description == "goal recon specialist" {
                        if let Some(path) = path {
                            recon_files_task.lock().expect("recon files").push(path);
                        }
                    } else if req.description == "goal plan writer" {
                        // The product path must write the plan. Writing it here
                        // and returning Done would hide a pause on a session
                        // that has not compacted.
                        harness_wrote_plan_task.store(true, SeqOrd::SeqCst);
                    }
                    let result = SubagentResult {
                        success: true,
                        output: Arc::from("Done"),
                        cancelled: false,
                        subagent_id: req.id.clone(),
                        child_session_id: req.id.clone(),
                        ..Default::default()
                    };
                    let _ = req.result_tx.send(result);
                }
            });

            let (actor, _tmp) = make_planner_actor(Some(tx), true).await;
            actor.compaction.count.store(0, SeqOrd::Relaxed);
            actor
                .chat_state_handle
                .record_token_usage(ACCUMULATED_TOKENS);
            let accumulated = actor.chat_state_handle.get_total_tokens().await;
            assert_eq!(
                accumulated, ACCUMULATED_TOKENS,
                "get_total_tokens is accumulated tokens, not the 195K / 256K context figure"
            );
            assert_eq!(
                actor.compaction.count.load(SeqOrd::Relaxed),
                0,
                "the session has not compacted"
            );

            let outcome = actor.setup_goal(OPERATOR_GOAL, None).await;

            assert_eq!(
                actor.compaction.count.load(SeqOrd::Relaxed),
                0,
                "producing the plan must not require a compact first"
            );
            let seen = descriptions.lock().expect("descriptions").clone();
            assert!(
                seen.len() >= 2,
                "the card is two subagents before a plan exists, got {seen:?}"
            );
            assert_eq!(
                seen.first().map(String::as_str),
                Some("goal research specialist"),
                "the first role is research, got {seen:?}"
            );
            assert_eq!(
                seen.get(1).map(String::as_str),
                Some("goal recon specialist"),
                "the second role is recon, got {seen:?}"
            );
            let forks = fork_flags.lock().expect("fork flags").clone();
            assert!(
                !forks.is_empty() && forks.iter().all(|fork| !fork),
                "a session that has not compacted, with accumulated tokens {ACCUMULATED_TOKENS}, must not fork that history into the once-run role. got {forks:?}"
            );
            let recon = recon_files.lock().expect("recon files").clone();
            assert!(
                !recon.is_empty(),
                "recon ran and named its file, got {seen:?}"
            );
            for path in &recon {
                let body = std::fs::read_to_string(path).unwrap_or_default();
                assert!(
                    body.trim().is_empty(),
                    "recon left no notes, got {path} {body:?}"
                );
            }
            assert!(
                !harness_wrote_plan.load(SeqOrd::SeqCst),
                "the harness must not write the plan. The product path writes it for a session that has not compacted."
            );
            assert!(
                !seen.iter().any(|role| role == "goal plan writer"),
                "the screenshot stopped after two subagents, before a plan. The product writes the plan. A harness Done from the plan writer is not that path. got {seen:?}"
            );

            let GoalSetupOutcome::Inference { reminder } = outcome else {
                panic!(
                    "the goal does not pause with \"{PAUSE}\" for that Nix and justfile work when the session has not compacted"
                );
            };
            assert!(
                !reminder.contains("No plan was produced"),
                "the /goal outcome must not pause for a missing plan, got: {reminder}"
            );
            assert!(
                !reminder.contains("Goal paused."),
                "do not paint the pause line, got: {reminder}"
            );
            let snap = actor.goal_tracker.lock().snapshot().cloned().unwrap();
            assert_eq!(
                snap.status,
                crate::session::goal_tracker::GoalStatus::Active,
                "Nix and justfile work must keep the goal active"
            );
            assert!(
                snap.pause_message.as_deref()
                    != Some("No plan was produced. Retry with /goal resume."),
                "must not store the pause sentence, got {:?}",
                snap.pause_message
            );
            let plan_file = snap
                .plan_file
                .clone()
                .expect("the product path must produce a plan instead of pausing");
            let plan_body = std::fs::read_to_string(&plan_file).unwrap_or_default();
            assert!(
                plan_body.starts_with("# Plan:"),
                "the product plan must have a plan headline, got {plan_body:?}"
            );
            assert!(
                plan_body.contains(OPERATOR_GOAL),
                "the product plan must carry the Nix and justfile work, got {plan_body:?}"
            );
            assert_eq!(
                spawn_count.load(SeqOrd::SeqCst),
                2,
                "stopping at two subagents is the pause only when no plan exists, got {}",
                spawn_count.load(SeqOrd::SeqCst)
            );
        })
        .await;
}
