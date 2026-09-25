//! `x.ai/interject` on a running nested session that is not a resident handle.

use std::sync::Arc;

use agent_client_protocol as acp;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use xai_grok_tools::implementations::grok_build::task::backend::{ChannelBackend, SubagentBackend};
use xai_grok_tools::implementations::grok_build::task::coordinator::{
    ChildCompletion, ChildControl, ChildRunOutput, ChildRunRequest, ChildRunner, CoordinatorConfig,
    SendBoxFuture, StartedChild, SubagentCoordinator, SubagentProgress,
};
use xai_grok_tools::implementations::grok_build::task::types::{
    SubagentDescribeOutcome, SubagentOwner, SubagentRequest, SubagentResult,
    SubagentValidateTypeOutcome,
};

use super::build_minimal_agent_for_tests;

struct FollowUpDelivery {
    child_session_id: String,
    text: String,
}

#[derive(Clone)]
struct LiveControl {
    cancellation: CancellationToken,
    child_session_id: String,
    follow_ups: mpsc::UnboundedSender<FollowUpDelivery>,
}

impl ChildControl for LiveControl {
    type ProgressFuture = std::future::Ready<SubagentProgress>;

    fn progress(&self) -> Self::ProgressFuture {
        std::future::ready(SubagentProgress::default())
    }

    fn cancel(&self) {
        self.cancellation.cancel();
    }

    fn follow_up(&self, text: String) {
        let _ = self.follow_ups.send(FollowUpDelivery {
            child_session_id: self.child_session_id.clone(),
            text,
        });
    }
}

struct LiveRunner {
    finish: tokio::sync::broadcast::Sender<()>,
    started: mpsc::UnboundedSender<String>,
    follow_ups: mpsc::UnboundedSender<FollowUpDelivery>,
}

impl ChildRunner for LiveRunner {
    type Control = LiveControl;
    type CompletionData = ();
    type RunFuture = SendBoxFuture<ChildRunOutput<()>>;
    type ValidateFuture = SendBoxFuture<SubagentValidateTypeOutcome>;
    type DescribeFuture = SendBoxFuture<SubagentDescribeOutcome>;

    fn run(&self, run: ChildRunRequest<Self::Control>) -> Self::RunFuture {
        let mut finish = self.finish.subscribe();
        let started = self.started.clone();
        let follow_ups = self.follow_ups.clone();
        Box::pin(async move {
            let ChildRunRequest {
                request,
                cancellation,
                reporter,
                ..
            } = run;
            let promoted = reporter
                .started(StartedChild {
                    child_session_id: request.id.clone(),
                    persona: None,
                    resumed_from: None,
                    child_cwd: String::new(),
                    worktree_path: None,
                    effective_model_id: "test-model".to_owned(),
                    definition_background: false,
                    control: LiveControl {
                        cancellation: cancellation.clone(),
                        child_session_id: request.id.clone(),
                        follow_ups,
                    },
                })
                .await;
            if !promoted {
                return ChildRunOutput {
                    result: SubagentResult {
                        cancelled: true,
                        subagent_id: request.id.clone(),
                        child_session_id: request.id.clone(),
                        ..Default::default()
                    },
                    completion_data: (),
                    snapshot_ref: None,
                };
            }
            let _ = started.send(request.id.clone());
            let result = tokio::select! {
                _ = cancellation.cancelled() => SubagentResult {
                    cancelled: true,
                    subagent_id: request.id.clone(),
                    child_session_id: request.id.clone(),
                    ..Default::default()
                },
                _ = finish.recv() => SubagentResult {
                    success: true,
                    output: Arc::from(request.prompt),
                    subagent_id: request.id.clone(),
                    child_session_id: request.id.clone(),
                    ..Default::default()
                },
            };
            ChildRunOutput {
                result,
                completion_data: (),
                snapshot_ref: None,
            }
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

    fn on_completed(&self, _completion: ChildCompletion<Self::CompletionData>) {}
}

fn live_request(id: &str) -> SubagentRequest {
    SubagentRequest {
        id: id.to_owned(),
        prompt: "work".to_owned(),
        description: format!("live nested {id}"),
        subagent_type: "explore".to_owned(),
        parent_session_id: "parent".to_owned(),
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
    }
}

fn ext_request(params: serde_json::Value) -> acp::ExtRequest {
    acp::ExtRequest::new(
        "x.ai/interject",
        serde_json::value::to_raw_value(&params)
            .expect("interject params")
            .into(),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn interject_to_a_live_nested_session_is_delivered_and_a_missing_session_says_gone() {
    let agent = build_minimal_agent_for_tests();
    let event_rx = agent
        .subagent_event_rx
        .borrow_mut()
        .take()
        .expect("subagent event rx");
    let (finish, _) = tokio::sync::broadcast::channel(1);
    let (started_tx, mut started_rx) = mpsc::unbounded_channel();
    let (follow_tx, mut follow_rx) = mpsc::unbounded_channel();
    let actor = tokio::spawn(
        SubagentCoordinator::new(
            event_rx,
            LiveRunner {
                finish: finish.clone(),
                started: started_tx,
                follow_ups: follow_tx,
            },
            CoordinatorConfig::default(),
        )
        .run(),
    );

    // Same id shape as a running L2: the subagent id is the child session id,
    // and it is not inserted as a resident handle.
    let live_id = "01a0d5e4-85b2-7103-99a7-fb22bb0cddfb";
    assert!(
        !agent.is_resident(&acp::SessionId::new(live_id)),
        "a running L2 is not a resident handle"
    );
    let backend = ChannelBackend::new(agent.subagent_event_tx.clone());
    let spawn = tokio::spawn({
        let backend = backend.clone();
        let id = live_id.to_owned();
        async move { backend.spawn(live_request(&id)).await }
    });
    let started = tokio::time::timeout(std::time::Duration::from_secs(5), started_rx.recv())
        .await
        .expect("running L2 must become active")
        .expect("started channel");
    assert_eq!(started, live_id);

    let accepted = crate::extensions::interject::handle(
        &agent,
        &ext_request(serde_json::json!({
            "sessionId": live_id,
            "text": "raw display text",
            "content": [{ "type": "text", "text": "rewritten steer" }],
        })),
    )
    .await
    .expect("live nested interjection must be accepted, not session not found");
    let body = super::parse_ext_body(&accepted);
    assert_eq!(
        body.get("status").and_then(|value| value.as_str()),
        Some("queued"),
        "accepted interjection must report status queued"
    );
    let delivered = tokio::time::timeout(std::time::Duration::from_secs(5), follow_rx.recv())
        .await
        .expect("follow-up must be delivered onto the live id")
        .expect("follow-up channel");
    assert_eq!(delivered.child_session_id, live_id);
    assert_eq!(
        delivered.text, "rewritten steer",
        "content text override wins, same as the resident path"
    );
    assert!(
        follow_rx.try_recv().is_err(),
        "queued follow-up must not send a second interject"
    );

    let gone_id = "01a0d5e4-gone-not-resident";
    let rejected = crate::extensions::interject::handle(
        &agent,
        &ext_request(serde_json::json!({
            "sessionId": gone_id,
            "text": "nobody home",
        })),
    )
    .await;
    let err = rejected.expect_err("a missing session must not report status queued");
    let shown = err.to_string();
    assert!(
        shown.contains(gone_id),
        "missing-session failure must contain the id, got {shown}"
    );
    assert!(
        shown.contains("session is gone"),
        "missing-session failure must say the session is gone, got {shown}"
    );
    assert!(
        !shown.contains("queued"),
        "missing-session failure must not claim the interjection was sent, got {shown}"
    );

    let _ = finish.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), spawn).await;
    actor.abort();
}
