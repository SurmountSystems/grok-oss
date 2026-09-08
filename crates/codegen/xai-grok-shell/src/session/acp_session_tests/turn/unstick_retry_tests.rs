use super::support::*;
use super::*;
use crate::session::commands::PromptCompletionKind;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use xai_chat_state::compaction_utils::extract_user_query;
use xai_grok_test_support::sse::responses_api_script_exact;
use xai_grok_test_support::{MockInferenceServer, ScriptedResponse};

fn drain_gateway(mut rx: tokio::sync::mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>) {
    tokio::task::spawn_local(async move {
        while let Some(msg) = rx.recv().await {
            if let xai_acp_lib::AcpClientMessage::SessionNotification(args) = msg {
                let _ = args.response_tx.send(Ok(()));
            }
        }
    });
}

fn drain_persistence(mut rx: tokio::sync::mpsc::UnboundedReceiver<PersistenceMsg>) {
    tokio::task::spawn_local(async move {
        while let Some(msg) = rx.recv().await {
            if let PersistenceMsg::FlushAndAck { respond_to } = msg {
                let _ = respond_to.send(Ok(()));
            }
        }
    });
}

async fn actor_with_mock_sampler(
    server: &MockInferenceServer,
    persistence_tx: tokio::sync::mpsc::UnboundedSender<PersistenceMsg>,
    gateway_tx: tokio::sync::mpsc::UnboundedSender<xai_acp_lib::AcpClientMessage>,
) -> Arc<SessionActor> {
    let sampling_cfg = xai_grok_sampler::SamplerConfig {
        api_key: Some("test-key".to_string()),
        base_url: server.url(),
        model: "test".to_string(),
        api_backend: xai_grok_sampler::ApiBackend::Responses,
        context_window: 256_000,
        max_retries: Some(0),
        idle_timeout_secs: Some(30),
        ..Default::default()
    };
    let (sampler_event_tx, sampler_event_rx) =
        tokio::sync::mpsc::unbounded_channel::<xai_grok_sampler::SamplingEvent>();
    let sampler_handle = xai_grok_sampler::SamplerActor::spawn(
        sampling_cfg,
        xai_grok_sampler::RetryPolicy {
            max_retries: 0,
            rate_limit_retry_threshold: 0,
            ..Default::default()
        },
        sampler_event_tx,
    );

    let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
    actor.sampler_handle = sampler_handle;
    *actor.agent.borrow_mut() = test_grok_build_agent_with_todo().await;

    let mut cfg = actor
        .chat_state_handle
        .get_sampling_config()
        .await
        .expect("test actor has sampling config");
    cfg.base_url = server.url();
    cfg.api_backend = xai_grok_sampling_types::ApiBackend::Responses;
    cfg.model = "test".to_string();
    actor.chat_state_handle.update_sampling_config(cfg);
    let mut creds = actor.chat_state_handle.get_credentials().await;
    creds.api_key = Some("test-key".to_string());
    actor.chat_state_handle.update_credentials(creds);

    actor
        .workspace_ops
        .bind_local_session(
            &actor.session_id_string(),
            actor.tool_context.cwd.as_path().to_path_buf(),
            actor.tool_context.hunk_tracker_handle.clone(),
            actor.agent.borrow().tool_bridge().toolset(),
            None,
        )
        .expect("bind_local_session");

    let actor = Arc::new(actor);
    {
        let drainer = actor.clone();
        let mut sampler_event_rx = sampler_event_rx;
        tokio::task::spawn_local(async move {
            while let Some(event) = sampler_event_rx.recv().await {
                drainer.handle_sampling_event(event).await;
            }
        });
    }
    actor
}

fn matching_user_query_count(conv: &[ConversationItem], expected: &str) -> usize {
    conv.iter()
        .filter(|item| matches!(item, ConversationItem::User(_)))
        .filter(|item| extract_user_query(&item.text_content()).trim() == expected)
        .count()
}

async fn run_prompt(
    actor: &Arc<SessionActor>,
    prompt_id: &str,
    text: &str,
    unstick_retry: bool,
) -> Result<crate::session::commands::PromptTurnOk, acp::Error> {
    let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
        text.to_string(),
    ))];
    tokio::time::timeout(
        Duration::from_secs(60),
        actor.handle_prompt(
            prompt_id,
            prompt_blocks,
            PromptMode::Agent,
            None,
            None,
            None,
            None,
            false,
            /* send_now */ false,
            None,
            None,
            None,
            unstick_retry,
        ),
    )
    .await
    .expect("turn must finish within timeout")
}

/// `/unstick` resends the last parent prompt. The shell must not append a
/// second `<user_query>` when that text already matches the last user turn,
/// then it must sample again (kick the stuck sampler).
#[test]
fn unstick_retry_does_not_append_second_user_query_when_last_turn_matches() {
    run_on_large_stack("unstick-retry-skip-append", || {
        block_on_local(false, async {
            let server = MockInferenceServer::start()
                .await
                .expect("mock inference server");
            server.enqueue_response(
                "/v1/responses",
                ScriptedResponse::sse(responses_api_script_exact("first", "test")),
            );
            server.enqueue_response(
                "/v1/responses",
                ScriptedResponse::sse(responses_api_script_exact("retry", "test")),
            );

            let (gateway_tx, gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            drain_gateway(gateway_rx);
            let (persistence_tx, persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            drain_persistence(persistence_rx);

            let actor = actor_with_mock_sampler(&server, persistence_tx, gateway_tx).await;
            let first = run_prompt(&actor, "unstick-first", "hello", false)
                .await
                .expect("first turn must complete");
            assert_eq!(first.stop_reason, acp::StopReason::EndTurn);
            let after_first = server.request_count();
            assert!(
                after_first >= 1,
                "first turn must sample; request_count={after_first}"
            );

            let retry = run_prompt(&actor, "unstick-retry", "hello", true)
                .await
                .expect("unstick retry must complete");
            assert_eq!(retry.stop_reason, acp::StopReason::EndTurn);

            let conv = actor.chat_state_handle.get_conversation().await;
            assert_eq!(
                matching_user_query_count(&conv, "hello"),
                1,
                "unstick retry must not append a second <user_query> when the last user turn already matches; conversation={conv:#?}"
            );
            let after_retry = server.request_count();
            assert!(
                after_retry > after_first,
                "unstick retry must kick the sampler; before={after_first} after={after_retry}"
            );
        });
    });
}

/// `/unstick` must orphan a hung `running_task` the way a reconnecting
/// client drops an in-flight RPC, then queue the retry in front. It must
/// not wait behind the hung turn, cancel nested work, or rewind tokens.
#[test]
fn unstick_retry_orphans_stuck_running_task_then_samples_again() {
    run_on_large_stack("unstick-retry-orphan-running-task", || {
        block_on_local(false, async {
            let server = MockInferenceServer::start()
                .await
                .expect("mock inference server");
            server.enqueue_response(
                "/v1/responses",
                ScriptedResponse::sse(responses_api_script_exact("first", "test")),
            );
            server.enqueue_response(
                "/v1/responses",
                ScriptedResponse::sse(responses_api_script_exact("retry", "test")),
            );

            let (gateway_tx, gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            drain_gateway(gateway_rx);
            let (persistence_tx, persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            drain_persistence(persistence_rx);

            let actor = actor_with_mock_sampler(&server, persistence_tx, gateway_tx).await;
            let first = run_prompt(&actor, "unstick-first", "hello", false)
                .await
                .expect("first turn must complete");
            assert_eq!(first.stop_reason, acp::StopReason::EndTurn);
            let after_first = server.request_count();
            let tokens_before = actor.chat_state_handle.get_conversation().await.len();

            let (hung_item, hung_rx) = user_item_with_rx("hung-turn", "client");
            let held = user_item("held-later", "client");
            {
                let mut state = actor.state.lock().await;
                state.pending_inputs.push_back(hung_item);
                state.running_task = Some(running_task_stub("hung-turn"));
                state.pending_inputs.push_back(held);
                state.notifications_suppressed = false;
            }

            let (respond_to, _retry_rx) = oneshot::channel();
            let mut req = queue_input_request(
                vec![acp::ContentBlock::Text(acp::TextContent::new(
                    "hello".to_string(),
                ))],
                "unstick-retry",
                respond_to,
            );
            req.unstick_retry = true;
            let cancel = actor.queue_input(req).await;
            assert!(
                !cancel,
                "unstick must orphan, not send-now cancel the hung turn"
            );

            {
                let state = actor.state.lock().await;
                assert!(
                    state.running_task.is_none(),
                    "hung running_task must be orphaned so the retry is not queued behind it"
                );
                let ids: Vec<&str> = state
                    .pending_inputs
                    .iter()
                    .map(|i| i.prompt_id.as_str())
                    .collect();
                assert_eq!(
                    ids,
                    vec!["unstick-retry", "held-later"],
                    "retry must not sit behind hung-turn; pending={ids:?}"
                );
                assert!(
                    !state.notifications_suppressed,
                    "orphan must not suppress task wakes the way a Stop cancel does"
                );
            }
            let hung = hung_rx.await.expect("orphaned hung RPC must resolve");
            match hung {
                Ok(ok) => assert!(
                    matches!(ok.completion_kind, PromptCompletionKind::RemovedFromQueue),
                    "orphaned hung RPC must resolve like a dropped reconnect RPC, not a failed turn: {ok:?}"
                ),
                Err(e) => panic!("orphaned hung RPC must not look like a failed turn: {e:?}"),
            }

            let (completion_tx, mut completion_rx) = tokio::sync::mpsc::unbounded_channel();
            actor.clone().maybe_start_running_task(completion_tx).await;
            let (pid, result) = tokio::time::timeout(Duration::from_secs(60), completion_rx.recv())
                .await
                .expect("unstick retry must start")
                .expect("completion");
            assert_eq!(pid, "unstick-retry");
            result.expect("unstick retry must sample");

            let conv = actor.chat_state_handle.get_conversation().await;
            assert_eq!(
                matching_user_query_count(&conv, "hello"),
                1,
                "unstick retry must not append a second <user_query>; conversation={conv:#?}"
            );
            assert!(
                conv.len() >= tokens_before,
                "must not rewind the transcript; before={tokens_before} after={}",
                conv.len()
            );
            let after_retry = server.request_count();
            assert!(
                after_retry > after_first,
                "unstick retry must kick the sampler after orphaning; before={after_first} after={after_retry}"
            );
        });
    });
}
