//! Real `SessionActor::run_compact` for GitHub #222 / #224.
//! Listing ids is not this test. After `actor.run_compact().await`:
//! follow_up of a running L3 is Queued, resume_from of a finished L3 is
//! Completed, and that L3 completion is still delivered to the L2.
//! The session harness is `create_test_actor` from the inline compact tests.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use xai_grok_sampling_types::ConversationItem;
use xai_grok_tools::implementations::grok_build::task::backend::{ChannelBackend, SubagentBackend};
use xai_grok_tools::implementations::grok_build::task::coordinator::{
    ChildCompletion, ChildControl, ChildRunOutput, ChildRunRequest, ChildRunner, CoordinatorConfig,
    LocalBoxFuture, StartedChild, SubagentCoordinator, SubagentProgress,
};
use xai_grok_tools::implementations::grok_build::task::root_control::NoRootControl;
use xai_grok_tools::implementations::grok_build::task::types::{
    SubagentCompletionsRequest, SubagentDescribeOutcome, SubagentEvent, SubagentFollowUpOutcome,
    SubagentOwner, SubagentRequest, SubagentResult, SubagentResumeLookup,
    SubagentValidateTypeOutcome,
};

use super::inline_auto_compact_flow_tests::{create_test_actor, route_compaction_to_mock_server};

const L1_SESSION: &str = "test-auto-compact";
const WAIT_BOUND: Duration = Duration::from_secs(2);

struct DeliveryControl {
    cancellation: CancellationToken,
    follow_ups: mpsc::UnboundedSender<(String, String)>,
    child_session_id: String,
}

impl ChildControl for DeliveryControl {
    type ProgressFuture = std::future::Ready<SubagentProgress>;

    fn progress(&self) -> Self::ProgressFuture {
        std::future::ready(SubagentProgress {
            turn_count: 1,
            tool_call_count: 0,
            tokens_used: 0,
            context_window_tokens: 1_000,
            context_usage_pct: 0,
            tools_used: Vec::new(),
            error_count: 0,
        })
    }

    fn cancel(&self) {
        self.cancellation.cancel();
    }

    fn follow_up(&self, text: String) {
        let _ = self.follow_ups.send((self.child_session_id.clone(), text));
    }
}

struct DeliveryRunner {
    finish_one: tokio::sync::broadcast::Sender<String>,
    started: mpsc::UnboundedSender<String>,
    resume_lookups: mpsc::UnboundedSender<SubagentResumeLookup>,
    follow_ups: mpsc::UnboundedSender<(String, String)>,
    dispositions: mpsc::UnboundedSender<(String, bool)>,
}

impl ChildRunner for DeliveryRunner {
    type Control = DeliveryControl;
    type RootControl = NoRootControl;
    type CompletionData = ();
    type RunFuture = LocalBoxFuture<ChildRunOutput<()>>;
    type ValidateFuture = LocalBoxFuture<SubagentValidateTypeOutcome>;
    type DescribeFuture = LocalBoxFuture<SubagentDescribeOutcome>;

    fn run(&self, run: ChildRunRequest<Self::Control>) -> Self::RunFuture {
        let mut finish_one = self.finish_one.subscribe();
        let started_tx = self.started.clone();
        let resume_tx = self.resume_lookups.clone();
        let follow_ups = self.follow_ups.clone();
        Box::pin(async move {
            let ChildRunRequest {
                request,
                cancellation,
                reporter,
                ..
            } = run;
            if let Some(source_id) = request.resume_from.clone() {
                let lookup = match tokio::time::timeout(
                    WAIT_BOUND,
                    reporter.resume_source(&source_id, &request.parent_session_id),
                )
                .await
                {
                    Ok(lookup) => lookup,
                    Err(_) => SubagentResumeLookup::Missing,
                };
                let _ = resume_tx.send(lookup);
            }
            let child = StartedChild {
                child_session_id: request.id.clone(),
                persona: None,
                resumed_from: request.resume_from.clone(),
                child_cwd: String::new(),
                worktree_path: None,
                effective_model_id: "test-model".to_owned(),
                definition_background: false,
                control: DeliveryControl {
                    cancellation: cancellation.clone(),
                    follow_ups,
                    child_session_id: request.id.clone(),
                },
            };
            let promoted = match tokio::time::timeout(WAIT_BOUND, reporter.started(child)).await {
                Ok(promoted) => promoted,
                Err(_) => false,
            };
            if !promoted {
                return cancelled_output(&request);
            }
            let _ = started_tx.send(request.id.clone());
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => {
                        return cancelled_output(&request);
                    }
                    step = tokio::time::timeout(WAIT_BOUND, finish_one.recv()) => {
                        match step {
                            Ok(Ok(id)) if id == request.id => break,
                            Ok(Ok(_)) => {}
                            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                                return cancelled_output(&request);
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
            finished_output(&request)
        })
    }

    fn validate_type(
        &self,
        _subagent_type: String,
        _parent_session_id: String,
    ) -> Self::ValidateFuture {
        Box::pin(std::future::ready(SubagentValidateTypeOutcome::Ok))
    }

    fn describe_type(
        &self,
        _subagent_type: String,
        _harness_agent_type: Option<String>,
        _parent_session_id: String,
    ) -> Self::DescribeFuture {
        Box::pin(std::future::ready(SubagentDescribeOutcome::Unavailable))
    }

    fn supports_wake(&self) -> bool {
        true
    }

    fn on_completed(
        &self,
        completion: ChildCompletion<Self::CompletionData>,
        terminal_published: Box<dyn FnOnce() + Send>,
    ) {
        let _ = self.dispositions.send((
            completion.result.subagent_id.clone(),
            completion.disposition.should_surface,
        ));
        terminal_published();
    }
}

fn finished_output(request: &SubagentRequest) -> ChildRunOutput<()> {
    ChildRunOutput {
        result: SubagentResult {
            success: true,
            output: Arc::from(request.prompt.as_str()),
            subagent_id: request.id.clone(),
            child_session_id: request.id.clone(),
            tool_calls: 1,
            turns: 1,
            ..Default::default()
        },
        completion_data: (),
        snapshot_ref: None,
    }
}

fn cancelled_output(request: &SubagentRequest) -> ChildRunOutput<()> {
    ChildRunOutput {
        result: SubagentResult {
            success: false,
            cancelled: true,
            error: Some("cancelled".to_owned()),
            subagent_id: request.id.clone(),
            child_session_id: request.id.clone(),
            ..Default::default()
        },
        completion_data: (),
        snapshot_ref: None,
    }
}

fn request(id: &str, session_id: &str) -> SubagentRequest {
    SubagentRequest {
        id: id.to_owned(),
        prompt: "work".to_owned(),
        description: format!("test specialist {id}"),
        subagent_type: "explore".to_owned(),
        parent_session_id: session_id.to_owned(),
        parent_prompt_id: Some("prompt".to_owned()),
        resume_from: None,
        cwd: None,
        runtime_overrides: Default::default(),
        run_in_background: true,
        surface_completion: true,
        await_to_completion: false,
        fork_context: false,
        owner: SubagentOwner::Task,
        implement_loop_effort: None,
        cancel_token: CancellationToken::new(),
        spawn_root: Default::default(),
        tool_call_id: Some(format!("call-{id}")),
    }
}

fn l3_request(id: &str) -> SubagentRequest {
    let mut request = request(id, "l2");
    request.runtime_overrides.spawn_depth = Some(2);
    request.runtime_overrides.immediate_parent_session_id = Some("l2".to_owned());
    request
}

fn spawn_task(
    backend: ChannelBackend,
    request: SubagentRequest,
) -> tokio::task::JoinHandle<Result<SubagentResult, xai_tool_runtime::ToolError>> {
    tokio::task::spawn_local(async move { backend.spawn(request, None).await })
}

async fn next_started(started: &mut mpsc::UnboundedReceiver<String>) -> String {
    tokio::time::timeout(WAIT_BOUND, started.recv())
        .await
        .expect("started must arrive within 2 seconds")
        .expect("started channel open")
}

async fn completion_ids(
    sender: &mpsc::UnboundedSender<SubagentEvent>,
    session: &str,
) -> Vec<String> {
    let (respond_to, response) = tokio::sync::oneshot::channel();
    sender
        .send(SubagentEvent::Completions(SubagentCompletionsRequest {
            parent_session_id: Some(session.to_owned()),
            suppress_ids: Vec::new(),
            respond_to,
        }))
        .expect("coordinator command channel open");
    tokio::time::timeout(WAIT_BOUND, response)
        .await
        .expect("completions must arrive within 2 seconds")
        .expect("completions response")
        .into_iter()
        .map(|summary| summary.snapshot.subagent_id)
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn l2_after_compact_still_receives_l3_completion_report() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (finish_one, _) = tokio::sync::broadcast::channel(8);
            let (started_tx, mut started) = mpsc::unbounded_channel();
            let (resume_tx, mut resume_lookups) = mpsc::unbounded_channel();
            let (follow_tx, mut follow_ups) = mpsc::unbounded_channel();
            let (disposition_tx, mut dispositions) = mpsc::unbounded_channel();
            let (coordinator_sender, receiver) = SubagentCoordinator::<DeliveryRunner>::channel();
            let coordinator = SubagentCoordinator::from_channel(
                receiver,
                DeliveryRunner {
                    finish_one: finish_one.clone(),
                    started: started_tx,
                    resume_lookups: resume_tx,
                    follow_ups: follow_tx,
                    dispositions: disposition_tx,
                },
                CoordinatorConfig {
                    foreground_budget: WAIT_BOUND,
                    buffer_completions: true,
                    parent_follow_up: true,
                    ..CoordinatorConfig::default()
                },
            );
            let coordinator_task = tokio::task::spawn_local(coordinator.run());
            let l1_backend =
                ChannelBackend::for_coordinator_session(coordinator_sender, L1_SESSION);
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let mut session = tokio::time::timeout(
                WAIT_BOUND,
                create_test_actor(50_000, 200_000, 85, gateway_tx, persistence_tx),
            )
            .await
            .expect("create_test_actor must finish within 2 seconds");
            session.tool_context.subagent_event_tx = Some(l1_backend.sender());
            let _server = tokio::time::timeout(
                WAIT_BOUND,
                route_compaction_to_mock_server(&session, "Summary of prior work. ".repeat(30)),
            )
            .await
            .expect("mock compact server must start within 2 seconds");
            let filler = "x".repeat(8_000);
            session.chat_state_handle.replace_conversation(vec![
                ConversationItem::system("sys"),
                ConversationItem::user(format!("u0 {filler}")),
                ConversationItem::assistant(format!("a0 {filler}")),
                ConversationItem::user("final query"),
            ]);
            let actor = Arc::new(session);

            let l2_backend = l1_backend.scoped_to_session("l2");
            let l2_spawn = spawn_task(l1_backend.clone(), request("l2", L1_SESSION));
            assert_eq!(next_started(&mut started).await, "l2");

            let done_spawn = spawn_task(l2_backend.clone(), l3_request("l3-done"));
            assert_eq!(next_started(&mut started).await, "l3-done");
            let _ = finish_one.send("l3-done".to_owned());
            let done_result = tokio::time::timeout(WAIT_BOUND, done_spawn)
                .await
                .expect("finished L3 must complete within 2 seconds")
                .expect("finished L3 join")
                .expect("finished L3 spawn");
            assert!(
                done_result.success,
                "finished L3 must succeed before run_compact"
            );
            let (finished_id, should_surface) =
                tokio::time::timeout(WAIT_BOUND, dispositions.recv())
                    .await
                    .expect("L3 disposition must arrive within 2 seconds")
                    .expect("L3 disposition");
            assert_eq!(finished_id, "l3-done");
            assert!(
                !should_surface,
                "L3 completion reports to the L2. L1 is not woken."
            );

            let live_spawn = spawn_task(l2_backend.clone(), l3_request("l3-live"));
            assert_eq!(next_started(&mut started).await, "l3-live");

            let compact_result =
                tokio::time::timeout(WAIT_BOUND, async { actor.run_compact().await })
                    .await
                    .expect("run_compact must finish within 2 seconds");
            assert!(
                compact_result.is_ok(),
                "SessionActor::run_compact must succeed: {compact_result:?}"
            );

            let outcome = tokio::time::timeout(
                WAIT_BOUND,
                l2_backend.follow_up(
                    "l3-live",
                    "L2 follow-up of the running L3 after run_compact",
                ),
            )
            .await
            .expect("follow_up must finish within 2 seconds");
            assert_eq!(
                outcome,
                SubagentFollowUpOutcome::Queued {
                    child_session_id: "l3-live".to_owned(),
                },
                "follow_up of a running L3 is Queued"
            );
            let (delivered_session, delivered_text) =
                tokio::time::timeout(WAIT_BOUND, follow_ups.recv())
                    .await
                    .expect("follow_up delivery must arrive within 2 seconds")
                    .expect("follow_up enqueued on the running L3");
            assert_eq!(delivered_session, "l3-live");
            assert_eq!(
                delivered_text,
                "L2 follow-up of the running L3 after run_compact"
            );

            let mut resume = l3_request("l3-resume");
            resume.resume_from = Some("l3-done".to_owned());
            let resume_spawn = spawn_task(l2_backend.clone(), resume);
            let lookup = tokio::time::timeout(WAIT_BOUND, resume_lookups.recv())
                .await
                .expect("resume_from lookup must arrive within 2 seconds")
                .expect("resume lookup");
            let source = match lookup {
                SubagentResumeLookup::Completed(source) => source,
                other => panic!("resume_from of a finished L3 is Completed, got {other:?}"),
            };
            assert_eq!(source.subagent_id, "l3-done");

            assert_eq!(
                completion_ids(&l1_backend.sender(), "l2").await,
                vec!["l3-done".to_owned()],
                "the L3 completion is still delivered to the L2 after run_compact"
            );
            assert!(
                completion_ids(&l1_backend.sender(), L1_SESSION)
                    .await
                    .is_empty(),
                "the L3 completion is not delivered to L1"
            );

            l2_spawn.abort();
            live_spawn.abort();
            resume_spawn.abort();
            coordinator_task.abort();
        })
        .await;
}
