use super::support::*;
use super::*;
use crate::session::storage::StorageAdapter;
use crate::terminal::AsyncTerminalRunner;
use crate::terminal::runner::{TerminalError, TerminalRunRequest, TerminalRunResult};
use xai_grok_paths::AbsPathBuf;
#[derive(Debug)]
struct DummyTerminal;

fn run_on_large_stack(name: &str, body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(16 * 1024 * 1024)
        .spawn(body)
        .unwrap_or_else(|e| panic!("spawn {name}: {e}"))
        .join()
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
}

fn block_on_local(fut: impl std::future::Future<Output = ()>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime");
    rt.block_on(async {
        let local = tokio::task::LocalSet::new();
        local.run_until(fut).await;
    });
}

fn cancel_opts(
    cancel_subagents: bool,
    kill_background_tasks: bool,
    rewind_if_no_output: bool,
    trigger: Option<&str>,
) -> crate::session::CancelOptions {
    crate::session::CancelOptions {
        cancel_subagents,
        kill_background_tasks,
        rewind_if_no_output,
        trigger: trigger.map(crate::session::CancelTrigger::from_client),
        user_initiated: trigger.is_some(),
    }
}

/// Same gate as `SessionCommand::Cancel` in the actor run loop: bind the
/// cancel outcome and drain queued notifications only when the barrier is
/// `WakeBarrier::Clear`. An `Armed` stop-gesture barrier must outlive the
/// cancel, so those sites skip the drain.
async fn cancel_running_task_and_gate_drain(
    actor: &Arc<SessionActor>,
    options: crate::session::CancelOptions,
) -> WakeBarrier {
    let barrier = actor.cancel_running_task(options).await;
    if barrier == WakeBarrier::Clear {
        let (completion_tx, _completion_rx) =
            tokio::sync::mpsc::unbounded_channel::<(String, PromptTurnResult)>();
        SessionActor::maybe_drain_notifications(Arc::clone(actor), completion_tx).await;
    }
    barrier
}

#[async_trait::async_trait]
impl AsyncTerminalRunner for DummyTerminal {
    async fn run(&self, _request: TerminalRunRequest) -> Result<TerminalRunResult, TerminalError> {
        Err(TerminalError::Other("dummy terminal".into()))
    }
}
#[test]
fn persist_ack_waits_for_disk_flush_before_success() {
    run_on_large_stack("persist-ack", || {
        block_on_local(async {
            let tmp = tempfile::TempDir::new().unwrap();
            let session_dir = tmp.path().join("session");
            let cwd = AbsPathBuf::new(std::path::PathBuf::from("/tmp")).unwrap();
            let fs = Arc::new(xai_grok_workspace::file_system::MockFs::new(
                cwd.to_path_buf(),
            ));
            let terminal = Arc::new(DummyTerminal {});
            let (hunk_tx, _hunk_rx) = tokio::sync::mpsc::unbounded_channel();
            let hunk_tracker_handle = xai_hunk_tracker::HunkTrackerActor::spawn(
                "test-persist-ack".to_string(),
                cwd.to_path_buf(),
                hunk_tx,
                xai_hunk_tracker::TrackingMode::AgentOnly,
                tokio_util::sync::CancellationToken::new(),
            );
            let tool_context =
                ToolContext::new(cwd.clone(), None, None, fs, terminal, hunk_tracker_handle);
            let session_info = SessionInfo {
                id: acp::SessionId::new("test-persist-ack"),
                cwd: cwd.as_str().to_string(),
            };
            let sampling_client = crate::sampling::Client::new(xai_grok_sampler::SamplerConfig {
                api_key: Some("test-key".to_string()),
                failover_api_keys: Vec::new(),
                base_url: "http://localhost".to_string(),
                model: "test".to_string(),
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                api_backend: Default::default(),
                auth_scheme: Default::default(),
                extra_headers: Default::default(),
                extra_response_includes: Vec::new(),
                query_params: Default::default(),
                env_http_headers: Default::default(),
                context_window: 100_000,
                client_version: None,
                force_http1: false,
                max_retries: None,
                stream_tool_calls: false,
                idle_timeout_secs: None,
                client_identifier: None,
                reasoning_effort: None,
                deployment_id: None,
                user_id: None,
                origin_client: None,
                attribution_callback: None,
                bearer_resolver: None,
                supports_backend_search: false,
                compactions_remaining: None,
                compaction_at_tokens: None,
                doom_loop_recovery: None,
                header_injector: None,
                ..Default::default()
            })
            .expect("sampling client should build for persistence actor");
            let persistence = crate::session::persistence::new_with_explicit_dir(
                &crate::session::info::Info {
                    id: session_info.id.clone(),
                    cwd: session_info.cwd.clone(),
                },
                session_dir.clone(),
                acp::ModelId::new("test-model"),
                sampling_client,
                crate::test_support::TEST_MODEL.to_owned(),
            )
            .await
            .expect("persistence actor should start");
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
            let (chat_event_tx, _chat_event_rx) = tokio::sync::mpsc::unbounded_channel();
            let chat_state_handle = xai_chat_state::ChatStateActor::spawn(
                vec![],
                xai_grok_sampling_types::SamplingConfig {
                    base_url: "http://localhost".to_string(),
                    model: "test".to_string(),
                    max_completion_tokens: None,
                    temperature: None,
                    top_p: None,
                    api_backend: Default::default(),
                    extra_headers: Default::default(),
                    env_http_headers: Default::default(),
                    query_params: Default::default(),
                    context_window: std::num::NonZeroU64::new(100_000).unwrap(),
                    reasoning_effort: None,
                    stream_tool_calls: None,
                },
                Box::new(
                    crate::session::chat_persistence::ChannelChatPersistence::new(
                        persistence.tx.clone(),
                    ),
                ),
                chat_event_tx,
                tokio_util::sync::CancellationToken::new(),
            );
            let mut actor =
                create_test_actor(0, 100_000, 85, gateway_tx, persistence.tx.clone()).await;
            actor.session_info = session_info;
            actor.tool_context = tool_context;
            actor.chat_state_handle = chat_state_handle;
            let actor = Arc::new(actor);
            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                "hello persist".to_string(),
            ))];
            let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
            let actor_for_prompt = actor.clone();
            let prompt_task = tokio::task::spawn_local(async move {
                actor_for_prompt
                    .handle_prompt(
                        "persist-ack-test",
                        prompt_blocks,
                        PromptMode::Agent,
                        None,
                        None,
                        None,
                        None,
                        true,
                        false,
                        None,
                        Some(ack_tx),
                        None,
                    )
                    .await
            });
            assert!(ack_rx.await.is_ok(), "persist ack should resolve");
            let storage = crate::session::storage::JsonlStorageAdapter::with_explicit_session_dir(
                session_dir,
            );
            let loaded = storage
                .load_session_without_updates(&actor.session_info)
                .await
                .unwrap();
            assert!(
                loaded
                    .chat_history
                    .iter()
                    .any(|item| item.text_content().contains("hello persist")),
                "loaded chat history should contain the just-persisted prompt"
            );
            let _ = prompt_task.await.expect("prompt task should complete");
        });
    });
}
#[test]
fn first_turn_memory_injection_persists_to_chat_history() {
    run_on_large_stack("first-turn-memory-persist", || {
        block_on_local(async {
            let session_dir = tempfile::tempdir().expect("tempdir");
            let session_info = crate::session::info::Info {
                id: acp::SessionId::new("persist-memory"),
                cwd: session_dir.path().to_string_lossy().to_string(),
            };
            let sampling_client = crate::sampling::Client::new(xai_grok_sampler::SamplerConfig {
                api_key: Some("test-key".to_string()),
                base_url: "http://localhost".to_string(),
                model: "test-model".to_string(),
                max_completion_tokens: None,
                extra_headers: Default::default(),
                extra_response_includes: Vec::new(),
                query_params: Default::default(),
                env_http_headers: Default::default(),
                temperature: None,
                top_p: None,
                api_backend: Default::default(),
                auth_scheme: Default::default(),
                context_window: 100_000,
                client_version: None,
                force_http1: false,
                max_retries: None,
                stream_tool_calls: false,
                idle_timeout_secs: None,
                client_identifier: None,
                reasoning_effort: None,
                deployment_id: None,
                user_id: None,
                origin_client: None,
                attribution_callback: None,
                bearer_resolver: None,
                supports_backend_search: false,
                compactions_remaining: None,
                compaction_at_tokens: None,
                doom_loop_recovery: None,
                header_injector: None,
                ..Default::default()
            })
            .expect("sampling client should build for persistence actor");
            let persistence = crate::session::persistence::new_with_explicit_dir(
                &crate::session::info::Info {
                    id: session_info.id.clone(),
                    cwd: session_info.cwd.clone(),
                },
                session_dir.path().to_path_buf(),
                acp::ModelId::new("test-model"),
                sampling_client,
                crate::test_support::TEST_MODEL.to_owned(),
            )
            .await
            .expect("persistence actor should start");
            let (_event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
            let (chat_event_tx, _chat_event_rx) = tokio::sync::mpsc::unbounded_channel();
            let chat_state_handle = xai_chat_state::ChatStateActor::spawn(
                vec![
                    ConversationItem::system("sys"),
                    ConversationItem::user("<user_info>OS Version: macos</user_info>"),
                ],
                xai_grok_sampling_types::SamplingConfig {
                    base_url: "http://localhost".to_string(),
                    model: "test".to_string(),
                    max_completion_tokens: None,
                    temperature: None,
                    top_p: None,
                    api_backend: Default::default(),
                    extra_headers: Default::default(),
                    env_http_headers: Default::default(),
                    query_params: Default::default(),
                    context_window: std::num::NonZeroU64::new(100_000).unwrap(),
                    reasoning_effort: None,
                    stream_tool_calls: None,
                },
                Box::new(
                    crate::session::chat_persistence::ChannelChatPersistence::new(
                        persistence.tx.clone(),
                    ),
                ),
                chat_event_tx,
                tokio_util::sync::CancellationToken::new(),
            );
            let request = chat_state_handle
                .build_request(
                    vec![],
                    Some(
                        "## Relevant Memory from Past Sessions\n\nPersist this memory reminder."
                            .to_string(),
                    ),
                    true,
                    None,
                    session_info.id.to_string(),
                    "persist-memory".to_string(),
                )
                .await
                .expect("request should build");
            assert!(
                matches!(request.items.first(), Some(ConversationItem::System(sys)) if
                sys.content.contains("Persist this memory reminder."))
            );
            let storage = crate::session::storage::JsonlStorageAdapter::with_explicit_session_dir(
                session_dir.path().to_path_buf(),
            );
            let (flush_tx, flush_rx) = tokio::sync::oneshot::channel();
            persistence
                .tx
                .send(PersistenceMsg::FlushAndAck {
                    respond_to: flush_tx,
                })
                .unwrap();
            flush_rx
                .await
                .expect("flush ack should resolve")
                .expect("persistence flush should succeed");
            let loaded = storage
                .load_session_without_updates(&session_info)
                .await
                .unwrap();
            assert!(
                matches!(loaded.chat_history.first(), Some(ConversationItem::System(sys))
                if sys.content.contains("Persist this memory reminder."))
            );
        });
    });
}
#[test]
fn first_turn_memory_injection_disabled_does_not_persist_to_chat_history() {
    // SessionActor plus process_conversation_turn_with_recovery overflows the
    // default test thread stack in debug. Same named contract; larger stack.
    std::thread::Builder::new()
        .name("memory-injection-disabled".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("current-thread runtime");
            rt.block_on(async {
                let local = tokio::task::LocalSet::new();
                local
                    .run_until(async {
            let session_dir = tempfile::tempdir().expect("tempdir");
            let session_info = crate::session::info::Info {
                id: acp::SessionId::new("persist-memory-disabled"),
                cwd: session_dir.path().to_string_lossy().to_string(),
            };
            let sampling_client = crate::sampling::Client::new(xai_grok_sampler::SamplerConfig {
                api_key: Some("test-key".to_string()),
                failover_api_keys: Vec::new(),
                base_url: "http://localhost".to_string(),
                model: "test-model".to_string(),
                max_completion_tokens: None,
                extra_headers: Default::default(),
                extra_response_includes: Vec::new(),
                query_params: Default::default(),
                env_http_headers: Default::default(),
                temperature: None,
                top_p: None,
                api_backend: Default::default(),
                auth_scheme: Default::default(),
                context_window: 100_000,
                client_version: None,
                force_http1: false,
                max_retries: None,
                stream_tool_calls: false,
                idle_timeout_secs: None,
                client_identifier: None,
                reasoning_effort: None,
                deployment_id: None,
                user_id: None,
                origin_client: None,
                attribution_callback: None,
                bearer_resolver: None,
                supports_backend_search: false,
                compactions_remaining: None,
                compaction_at_tokens: None,
                doom_loop_recovery: None,
                header_injector: None,
                ..Default::default()
            })
            .expect("sampling client should build for persistence actor");
            let persistence = crate::session::persistence::new_with_explicit_dir(
                &crate::session::info::Info {
                    id: session_info.id.clone(),
                    cwd: session_info.cwd.clone(),
                },
                session_dir.path().to_path_buf(),
                acp::ModelId::new("test-model"),
                sampling_client,
                crate::test_support::TEST_MODEL.to_owned(),
            )
            .await
            .expect("persistence actor should start");
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (chat_event_tx, _chat_event_rx) = tokio::sync::mpsc::unbounded_channel();
            let initial_conversation = vec![
                ConversationItem::system("sys"),
                ConversationItem::user(
                    "<user_info>OS Version: macos</user_info>\n\n<user_query>hello</user_query>",
                ),
            ];
            let chat_state_handle = xai_chat_state::ChatStateActor::spawn(
                initial_conversation.clone(),
                xai_grok_sampling_types::SamplingConfig {
                    base_url: "http://localhost".to_string(),
                    model: "test".to_string(),
                    max_completion_tokens: None,
                    temperature: None,
                    top_p: None,
                    api_backend: Default::default(),
                    extra_headers: Default::default(),
                    env_http_headers: Default::default(),
                    query_params: Default::default(),
                    context_window: std::num::NonZeroU64::new(100_000).unwrap(),
                    reasoning_effort: None,
                    stream_tool_calls: None,
                },
                Box::new(
                    crate::session::chat_persistence::ChannelChatPersistence::new(
                        persistence.tx.clone(),
                    ),
                ),
                chat_event_tx,
                tokio_util::sync::CancellationToken::new(),
            );
            chat_state_handle.replace_conversation(initial_conversation);
            let memory_storage =
                crate::session::memory::MemoryStorage::new(session_dir.path(), None);
            memory_storage.ensure_initialized().unwrap();
            let memory_backend_params = crate::session::memory::MemoryBackendParams {
                session_id: session_info.id.to_string(),
                embed_config: None,
                embed_base_url: "http://localhost".to_string(),
                embed_api_key: None,
                search_config: crate::config::MemorySearchConfig::default(),
                watcher: None,
                stale_claim_secs: 60,
                search_source: "tool",
                embedding_credentials: crate::session::memory::EndpointScopedCredentials::none(),
            };
            let mut actor = Box::new(
                create_test_actor(0, 100_000, 85, gateway_tx, persistence.tx.clone()).await,
            );
            actor.session_info = session_info.clone();
            actor.chat_state_handle = chat_state_handle;
            actor.memory.storage = std::cell::RefCell::new(Some(memory_storage));
            actor.memory.backend_params = Some(memory_backend_params);
            actor.memory.initial_injection_config = crate::config::MemoryInitialInjectionConfig {
                enabled: false,
                min_score: Some(0.8),
            };
            let actor: Arc<SessionActor> = Arc::from(actor);
            let _ = actor
                .process_conversation_turn_with_recovery("disabled-memory", None, None, None)
                .await;
            let (flush_tx, flush_rx) = tokio::sync::oneshot::channel();
            persistence
                .tx
                .send(PersistenceMsg::FlushAndAck {
                    respond_to: flush_tx,
                })
                .unwrap();
            flush_rx
                .await
                .expect("flush ack should resolve")
                .expect("persistence flush should succeed");
            let storage = crate::session::storage::JsonlStorageAdapter::with_explicit_session_dir(
                session_dir.path().to_path_buf(),
            );
            let loaded = storage
                .load_session_without_updates(&session_info)
                .await
                .unwrap();
            assert!(
                matches!(loaded.chat_history.first(), Some(ConversationItem::System(sys))
                if sys.content.as_ref() == "sys")
            );
            assert_eq!(
                0,
                actor
                    .memory
                    .injection_count
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
                    })
                    .await;
            });
        })
        .expect("spawn larger-stack test thread")
        .join()
        .expect("memory-injection-disabled thread");
}
/// Hard teardown (`kill_background_tasks = true`, the subagent-shutdown path)
/// aborts the running turn AND drains every queued prompt, responding
/// `Cancelled` to each. Interactive cancel preserves the queue instead — see
/// `cancel_running_task_interactive_preserves_queued_work`.
#[tokio::test(flavor = "current_thread")]
async fn cancel_running_task_teardown_clears_running_and_pending_work() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel::<
                xai_acp_lib::AcpClientMessage,
            >();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel::<
                PersistenceMsg,
            >();
            let actor = {
                let mut actor = create_test_actor(
                    0,
                    100_000,
                    85,
                    gateway_tx,
                    persistence_tx,
                )
                .await;
                actor
                    .agent
                    .borrow()
                    .tool_bridge()
                    .update_resource(
                        xai_grok_tools::implementations::grok_build::task::types::CurrentPromptIdResource(
                            "running".to_string(),
                        ),
                    )
                    .await;
                *actor
                    .current_prompt_id
                    .lock()
                    .expect("current_prompt_id mutex") = Some("running".to_string());
                actor
            };
            let (tx, rx) = tokio::sync::oneshot::channel();
            let bridge = actor.agent.borrow().tool_bridge().clone();
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: tokio::task::spawn_local(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        })
                        .abort_handle(),
                });
                state
                    .pending_inputs
                    .push_back(InputItem {
                        prompt_id: "queued".into(),
                        prompt_blocks: vec![],
                        prompt_mode: PromptMode::Agent,
                        trace_gcs_config: None,
                        artifact_tracker: None,
                        client_identifier: None,
                        screen_mode: None,
                        verbatim: false,
                        json_schema: None,
                        origin: crate::session::PromptOrigin::User,
                        task_wake_fallback: None,
                        tool_overrides_update: None,
                        respond_to: tx,
                        persist_ack: None,
                        parsed_prompt_tx: None,
                        queue_meta: None,
                        send_now: false,
                    });
            }
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(&actor, cancel_opts(true, true, false, None))
                    .await,
                WakeBarrier::Clear,
            );
            let scoped_prompt_id = bridge
                .read_resource::<
                    xai_grok_tools::implementations::grok_build::task::types::CurrentPromptIdResource,
                >()
                .await;
            assert!(
                scoped_prompt_id.is_none() || scoped_prompt_id.as_ref().is_some_and(| p |
                p.0.is_empty()),
                "CurrentPromptIdResource should be cleared on cancellation"
            );
            assert!(
                actor.current_prompt_id.lock().expect("current_prompt_id mutex poisoned")
                .is_none(), "current_prompt_id should be cleared on cancellation"
            );
            let state = actor.state.lock().await;
            assert!(state.running_task.is_none());
            assert!(state.pending_inputs.is_empty());
            drop(state);
            let turn_ok = rx
                .await
                .expect("queued prompt should receive cancellation")
                .expect("queued prompt result should be ok");
            assert_eq!(turn_ok.stop_reason, acp::StopReason::Cancelled);
        })
        .await;
}
/// Interactive cancel (`kill_background_tasks = false`, the Ctrl+C path) aborts
/// the running turn and removes ONLY the running prompt (the front of
/// `pending_inputs`). Every queued prompt is PRESERVED so the `Cancel`
/// handler's follow-up `maybe_start_running_task` promotes the new front (the
/// user's next queued prompt) and rebroadcasts `x.ai/queue/changed`. The
/// cancelling client never pulls a queued prompt back into its input — the
/// server queue is the single source of truth for what runs next.
///
/// Regression for two bugs: (1) every cancel did `std::mem::take` on the queue,
/// silently discarding all queued prompts (which only surfaced to clients on
/// the next prompt's empty broadcast); (2) the running prompt stays at
/// `pending_inputs.front()` while running, so naively preserving the queue
/// would re-run the cancelled turn.
/// A Ctrl+C / ESC cancel (`session/cancel` → `cancel_running_task`) records a
/// `MidTurnAbort` interrupt cause on the EventTracker so the *next* real user
/// prompt gets tagged `PriorTurnInterrupt::MidTurnAbort`. Guards the cancel →
/// next-message marking contract and the one-shot (consumed-once) semantics.
#[tokio::test(flavor = "current_thread")]
async fn cancel_records_mid_turn_abort_interrupt_marker() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("running".to_string());
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: tokio::task::spawn_local(async {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    })
                    .abort_handle(),
                });
            }
            assert_eq!(actor.events.take_prior_interrupt_category(), None);
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(
                    &actor,
                    cancel_opts(true, false, false, Some("ctrl_c")),
                )
                .await,
                WakeBarrier::Armed,
            );
            assert_eq!(
                actor.events.take_prior_interrupt_category(),
                Some(crate::session::events::CancellationCategory::MidTurnAbort),
                "cancel must record a MidTurnAbort interrupt marker"
            );
            assert_eq!(actor.events.take_prior_interrupt_category(), None);
        })
        .await;
}
/// A mid-stream abort with NO tool in flight leaves the model with no visible
/// signal: the partial assistant text is discarded out-of-band and there is no
/// dangling tool call to repair into a "cancelled" tool-result. So
/// `cancel_running_task` must arm the one-shot `pending_interrupt_reminder` that
/// the next real user prompt frames as an interjection-shaped envelope.
#[tokio::test(flavor = "current_thread")]
async fn cancel_without_active_tool_arms_interrupt_reminder() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("running".to_string());
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: tokio::task::spawn_local(async {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    })
                    .abort_handle(),
                });
            }
            assert!(!actor.events.has_active_tool());
            assert!(!actor.events.take_pending_interrupt_reminder());
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(
                    &actor,
                    cancel_opts(true, false, false, Some("ctrl_c")),
                )
                .await,
                WakeBarrier::Armed,
            );
            assert!(
                actor.events.take_pending_interrupt_reminder(),
                "a no-active-tool abort must arm the interrupt reminder"
            );
        })
        .await;
}
/// Send-now is a silent cancel-and-send — the user is continuing, not
/// aborting. Its cancel must arm NEITHER the interrupt envelope (no
/// interrupt lead-in on the continuation turn) NOR the
/// zombie wait guard from the aborted turn can't auto-send-now-cancel (and
/// drop) the next user prompt.
#[tokio::test(flavor = "current_thread")]
async fn send_now_cancel_arms_no_interrupt_signals_and_resets_wait_depth() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("running".to_string());
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: tokio::task::spawn_local(async {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    })
                    .abort_handle(),
                });
            }
            let zombie_guard = crate::tools::tool_context::BlockingWaitGuard::enter(
                actor.tool_context.blocking_wait_depth.clone(),
            );
            assert!(!actor.events.has_active_tool());
            let mut replay_buffer = ReplayBuffer::new(None);
            actor.cancel_turn_for_send_now(&mut replay_buffer).await;
            assert!(
                !actor.events.take_pending_interrupt_reminder(),
                "send-now must not arm the interrupt reminder"
            );
            assert_eq!(
                actor.events.take_prior_interrupt_category(),
                None,
                "send-now must not record a MidTurnAbort interrupt marker"
            );
            let depth = &actor.tool_context.blocking_wait_depth;
            assert_eq!(depth.depth(), 0, "cancel must zero the wait window");
            drop(zombie_guard);
            assert_eq!(
                depth.depth(),
                0,
                "a late guard drop after the reset must not underflow"
            );
        })
        .await;
}
/// When the aborted turn left a committed-but-unanswered tool call, the
/// next-turn dangling repair already emits a "cancelled" tool-result, so arming
/// the reminder too would double-signal. This covers a tool mid-execution AND —
/// critically — a turn parked on a permission prompt, where the tool-call is
/// committed but NO tool is marked active yet (`has_active_tool()` is false).
/// Gating on the dangling state rather than `had_active_tool` keeps both cases
/// covered.
#[tokio::test(flavor = "current_thread")]
async fn cancel_with_dangling_tool_call_skips_interrupt_reminder() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.chat_state_handle.push_assistant_response(
                ConversationItem::assistant_tool_calls(vec![xai_grok_sampling_types::ToolCall {
                    id: "call-1".into(),
                    name: "run_terminal_cmd".into(),
                    arguments: "{}".into(),
                }]),
            );
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("running".to_string());
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: tokio::task::spawn_local(async {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    })
                    .abort_handle(),
                });
            }
            assert!(!actor.events.has_active_tool());
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(
                    &actor,
                    cancel_opts(true, false, false, Some("ctrl_c")),
                )
                .await,
                WakeBarrier::Armed,
            );
            assert!(
                !actor.events.take_pending_interrupt_reminder(),
                "an abort with a dangling tool call is already covered by the \
                 repair; the reminder must stay disarmed"
            );
        })
        .await;
}
/// Once armed, `maybe_apply_interrupt_envelope` frames the next user query
/// with the interjection envelope exactly once (one-shot).
#[tokio::test(flavor = "current_thread")]
async fn maybe_apply_interrupt_envelope_is_one_shot() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _gateway_rx) = build_actor().await;
            actor.events.set_pending_interrupt_reminder();
            let assembled = "<user_query>\nfollow-up after interrupt\n</user_query>";
            let framed = actor.maybe_apply_interrupt_envelope(assembled.into(), false);
            assert_eq!(framed, frame_user_turn(INTERRUPT_NOTE, assembled));
            assert!(!actor.events.take_pending_interrupt_reminder());
            let again = actor.maybe_apply_interrupt_envelope(assembled.into(), false);
            assert_eq!(again, assembled, "interrupt envelope must be one-shot");
        })
        .await;
}
/// Headless `--verbatim` / ACP `_meta.verbatim` owns the exact prompt. Framing
/// would break that contract; the one-shot still fires so it cannot leak onto
/// a later non-verbatim user turn.
#[tokio::test(flavor = "current_thread")]
async fn maybe_apply_interrupt_envelope_skips_verbatim() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _gateway_rx) = build_actor().await;
            actor.events.set_pending_interrupt_reminder();
            let assembled = "caller-owned follow-up";
            let framed = actor.maybe_apply_interrupt_envelope(assembled.into(), true);
            assert_eq!(framed, assembled, "verbatim text must stay byte-identical");
            assert!(
                !actor.events.take_pending_interrupt_reminder(),
                "verbatim still consumes the one-shot"
            );
        })
        .await;
}
/// Integration: with the one-shot armed, a real user turn driven through
/// `handle_prompt` frames the query in the same envelope as an interjection
/// (lead-in + `<user_query>` + unfinished-task trailer) instead of a
/// preceding `<system-reminder>`. Synchronizes on the persist-ack (fires
/// after the user item is pushed, before the model call), then aborts the
/// turn so the dead-URL model call can't hang.
#[test]
fn handle_prompt_frames_interrupt_on_user_message() {
    std::thread::Builder::new()
        .name("handle-prompt-interrupt-frame".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("current-thread runtime");
            rt.block_on(async {
                let local = tokio::task::LocalSet::new();
                local
                    .run_until(async {
                        let actor = actor_with_persistence_drain().await;
                        actor.events.set_pending_interrupt_reminder();
                        let query = "follow-up after interrupt";
                        let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                            query.to_string(),
                        ))];
                        let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
                        let actor_for_prompt = actor.clone();
                        let prompt_task = tokio::task::spawn_local(async move {
                            actor_for_prompt
                                .handle_prompt(
                                    "interrupt-wiring-test",
                                    prompt_blocks,
                                    PromptMode::Agent,
                                    None,
                                    None,
                                    None,
                                    None,
                                    false,
                                    false,
                                    None,
                                    Some(ack_tx),
                                    None,
                                )
                                .await
                        });
                        assert!(ack_rx.await.is_ok(), "persist ack should resolve");
                        let conv = actor.chat_state_handle.get_conversation().await;
                        let user = conv
                .iter()
                .find(|item| {
                    matches!(item, ConversationItem::User(u) if u.synthetic_reason.is_none())
                        && item.text_content().contains(query)
                })
                .expect("the user message must be in the conversation");
                        let text = user.text_content();
                        let expected_assembled = format!("<user_query>\n{query}\n</user_query>");
                        assert_eq!(text, frame_user_turn(INTERRUPT_NOTE, &expected_assembled));
                        assert!(!actor.events.take_pending_interrupt_reminder());
                        prompt_task.abort();
                    })
                    .await;
            });
        })
        .expect("spawn larger-stack test thread")
        .join()
        .expect("handle-prompt-interrupt-frame thread");
}
/// Integration: a verbatim user turn must stay byte-identical to the caller
/// text even when the interrupt one-shot is armed.
#[test]
fn handle_prompt_verbatim_skips_interrupt_envelope() {
    run_on_large_stack("handle-prompt-verbatim", || {
        block_on_local(async {
            let actor = actor_with_persistence_drain().await;
            actor.events.set_pending_interrupt_reminder();
            let query = "caller-owned follow-up";
            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                query.to_string(),
            ))];
            let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
            let actor_for_prompt = actor.clone();
            let prompt_task = tokio::task::spawn_local(async move {
                actor_for_prompt
                    .handle_prompt(
                        "interrupt-verbatim-test",
                        prompt_blocks,
                        PromptMode::Agent,
                        None,
                        None,
                        None,
                        None,
                        true,
                        false,
                        None,
                        Some(ack_tx),
                        None,
                    )
                    .await
            });
            assert!(ack_rx.await.is_ok(), "persist ack should resolve");
            let conv = actor.chat_state_handle.get_conversation().await;
            let user = conv
                .iter()
                .find(|item| {
                    matches!(item, ConversationItem::User(u) if u.synthetic_reason.is_none())
                        && item.text_content().contains(query)
                })
                .expect("the user message must be in the conversation");
            assert_eq!(user.text_content(), query);
            assert!(!user.text_content().contains(INTERRUPT_NOTE));
            assert!(!actor.events.take_pending_interrupt_reminder());
            prompt_task.abort();
        });
    });
}
/// Send-now must use the full interjection envelope (prefix + already-wrapped
/// `<user_query>` + unfinished-task trailer), not the note prefix alone.
#[test]
fn handle_prompt_send_now_frames_interjection_envelope() {
    run_on_large_stack("handle-prompt-send-now", || {
        block_on_local(async {
            let actor = actor_with_persistence_drain().await;
            let query = "create /tmp/A";
            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                query.to_string(),
            ))];
            let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
            let actor_for_prompt = actor.clone();
            let prompt_task = tokio::task::spawn_local(async move {
                actor_for_prompt
                    .handle_prompt(
                        "send-now-envelope-test",
                        prompt_blocks,
                        PromptMode::Agent,
                        None,
                        None,
                        None,
                        None,
                        false,
                        true,
                        None,
                        Some(ack_tx),
                        None,
                    )
                    .await
            });
            assert!(ack_rx.await.is_ok(), "persist ack should resolve");
            let conv = actor.chat_state_handle.get_conversation().await;
            let user = conv
                .iter()
                .find(|item| {
                    matches!(item, ConversationItem::User(u) if u.synthetic_reason.is_none())
                        && item.text_content().contains(query)
                })
                .expect("the send-now user message must be in the conversation");
            let expected_assembled = format!("<user_query>\n{query}\n</user_query>");
            assert_eq!(
                user.text_content(),
                frame_user_turn(
                    xai_interjection_core::INTERJECTION_NOTE,
                    &expected_assembled
                )
            );
            prompt_task.abort();
        });
    });
}
/// Integration: a synthetic-origin turn (here `scheduler-fired-*`) driven
/// between the abort and the user's resend must NOT consume the one-shot or
/// inject the reminder — it has to survive to the next *genuine* user turn.
/// Guards the `PromptOrigin::User` gate on the injection call.
#[test]
fn handle_prompt_synthetic_origin_preserves_interrupt_reminder() {
    run_on_large_stack("handle-prompt-synthetic", || {
        block_on_local(async {
            let actor = actor_with_persistence_drain().await;
            actor.events.set_pending_interrupt_reminder();
            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                "scheduler tick".to_string(),
            ))];
            let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
            let actor_for_prompt = actor.clone();
            let prompt_task = tokio::task::spawn_local(async move {
                actor_for_prompt
                    .handle_prompt(
                        "scheduler-fired-test-1",
                        prompt_blocks,
                        PromptMode::Agent,
                        None,
                        None,
                        None,
                        None,
                        true,
                        false,
                        None,
                        Some(ack_tx),
                        None,
                    )
                    .await
            });
            assert!(ack_rx.await.is_ok(), "persist ack should resolve");
            assert!(
                actor.events.take_pending_interrupt_reminder(),
                "a synthetic-origin turn must NOT consume the interrupt reminder"
            );
            let conv = actor.chat_state_handle.get_conversation().await;
            assert!(
                !conv
                    .iter()
                    .any(|item| item.text_content().contains(INTERRUPT_NOTE)),
                "a synthetic-origin turn must not inject the interrupt envelope"
            );
            prompt_task.abort();
        });
    });
}
#[tokio::test(flavor = "current_thread")]
async fn cancel_running_task_interactive_preserves_queued_work() {
    use tokio::sync::oneshot::error::TryRecvError;
    fn make_item(
        prompt_id: &str,
        queue_id: &str,
    ) -> (InputItem, tokio::sync::oneshot::Receiver<PromptTurnResult>) {
        let (respond_to, rx) = tokio::sync::oneshot::channel();
        let item = InputItem {
            prompt_id: prompt_id.to_string(),
            prompt_blocks: vec![],
            prompt_mode: PromptMode::Agent,
            trace_gcs_config: None,
            artifact_tracker: None,
            client_identifier: None,
            screen_mode: None,
            verbatim: false,
            json_schema: None,
            origin: crate::session::PromptOrigin::User,
            task_wake_fallback: None,
            tool_overrides_update: None,
            respond_to,
            persist_ack: None,
            parsed_prompt_tx: None,
            queue_meta: Some(crate::session::prompt_queue::QueueEntryMeta {
                id: queue_id.to_string(),
                version: 0,
                owner: None,
                last_editor: None,
                kind: "prompt".to_string(),
                text: String::new(),
                combined_texts: None,
            }),
            send_now: false,
        };
        (item, rx)
    }
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("running".to_string());
            let (running_item, mut running_rx) = make_item("running", "running");
            let (q1_item, mut q1_rx) = make_item("q1-pid", "q1");
            let (q2_item, mut q2_rx) = make_item("q2-pid", "q2");
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: tokio::task::spawn_local(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    })
                    .abort_handle(),
                });
                state.pending_inputs.push_back(running_item);
                state.pending_inputs.push_back(q1_item);
                state.pending_inputs.push_back(q2_item);
            }
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(&actor, cancel_opts(true, false, false, None))
                    .await,
                WakeBarrier::Clear,
            );
            assert!(
                actor
                    .current_prompt_id
                    .lock()
                    .expect("current_prompt_id mutex poisoned")
                    .is_none(),
                "current_prompt_id should be cleared on cancellation"
            );
            let state = actor.state.lock().await;
            assert!(state.running_task.is_none(), "running turn must be aborted");
            let surviving: Vec<&str> = state
                .pending_inputs
                .iter()
                .map(|i| i.prompt_id.as_str())
                .collect();
            assert_eq!(
                surviving,
                vec!["q1-pid", "q2-pid"],
                "only the running turn is removed; every queued prompt is preserved"
            );
            drop(state);
            assert!(
                matches!(
                    running_rx.try_recv(),
                    Ok(Ok(crate::session::commands::PromptTurnOk {
                        stop_reason: acp::StopReason::Cancelled,
                        ..
                    }))
                ),
                "running turn must be resolved Cancelled"
            );
            assert!(
                matches!(q1_rx.try_recv(), Err(TryRecvError::Empty)),
                "front queued prompt must remain pending (it runs next), not be cancelled"
            );
            assert!(
                matches!(q2_rx.try_recv(), Err(TryRecvError::Empty)),
                "preserved prompt must remain pending, not be cancelled"
            );
        })
        .await;
}
/// The auto-wake defect chain end-to-end at the actor level: a running
/// `task-completed-{id}` turn at the front, a real user prompt queued behind
/// it, then the consumed-completion sweep (the auto-wake turn polling its own
/// task's output) followed by an interactive Ctrl+C cancel. The sweep must
/// leave the running turn's own front slot alone so the cancel resolves the
/// AUTO-WAKE item with `Cancelled` and the user's prompt survives to run
/// next. If the sweep deletes the front, the user prompt shifts to index 0
/// and the cancel destroys it instead — the message never runs and, since
/// user messages are only persisted when their turn starts, it is silently
/// lost from history.
#[tokio::test(flavor = "current_thread")]
async fn cancel_after_own_completion_sweep_preserves_queued_user_prompt() {
    use tokio::sync::oneshot::error::TryRecvError;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") =
                Some("task-completed-bg-1".to_string());
            let (wake_item, mut wake_rx) = input_with_origin_rx(
                "task-completed-bg-1",
                crate::session::PromptOrigin::TaskCompleted {
                    task_id: "bg-1".to_string(),
                },
            );
            let (user_item, mut user_rx) =
                input_with_origin_rx("user-clarify", crate::session::PromptOrigin::User);
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(running_task_stub("task-completed-bg-1"));
                state.pending_inputs.push_back(wake_item);
                state.pending_inputs.push_back(user_item);
            }
            actor
                .drop_pending_items_for_consumed_completions(&["bg-1"])
                .await;
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(
                    &actor,
                    cancel_opts(true, false, false, Some("ctrl_c")),
                )
                .await,
                WakeBarrier::Armed,
            );
            let state = actor.state.lock().await;
            let surviving: Vec<&str> = state
                .pending_inputs
                .iter()
                .map(|i| i.prompt_id.as_str())
                .collect();
            assert_eq!(
                surviving,
                vec!["user-clarify"],
                "the queued user prompt must survive a cancel that lands during \
                 the auto-wake turn"
            );
            drop(state);
            assert!(
                matches!(
                    wake_rx.try_recv(),
                    Ok(Ok(crate::session::commands::PromptTurnOk {
                        stop_reason: acp::StopReason::Cancelled,
                        ..
                    }))
                ),
                "the front resolved with Cancelled must be the auto-wake turn"
            );
            assert!(
                matches!(user_rx.try_recv(), Err(TryRecvError::Empty)),
                "the user prompt must remain pending (it runs next), not be cancelled"
            );
        })
        .await;
}
/// Regression for the cancel-spinner hang: an interactive cancel must resolve
/// the in-flight front prompt's `respond_to` with `Cancelled` even when
/// `state.running_task` is `None`.
///
/// Background: cancel is fire-and-forget on the client; the TUI spinner only
/// returns to idle when the originating `session/prompt` resolves. Earlier the
/// running turn was resolved only when `running_task.is_some()`
/// (`is_running_turn = idx == 0 && had_running_turn`). In the narrow windows
/// where the front has no live task (a completion was just dequeued before the
/// next prompt is promoted, or a cancel races ahead of
/// `maybe_start_running_task`), the front's `respond_to` was dropped, hanging
/// the client's `session/prompt` and spinning the spinner forever. The front
/// (index 0) must now always be resolved; deeper queued prompts are preserved.
#[tokio::test(flavor = "current_thread")]
async fn cancel_resolves_front_when_running_task_is_none() {
    use tokio::sync::oneshot::error::TryRecvError;
    fn make_item(
        prompt_id: &str,
        queue_id: Option<&str>,
    ) -> (InputItem, tokio::sync::oneshot::Receiver<PromptTurnResult>) {
        let (respond_to, rx) = tokio::sync::oneshot::channel();
        let item = InputItem {
            prompt_id: prompt_id.to_string(),
            prompt_blocks: vec![],
            prompt_mode: PromptMode::Agent,
            trace_gcs_config: None,
            artifact_tracker: None,
            client_identifier: None,
            screen_mode: None,
            verbatim: false,
            json_schema: None,
            origin: crate::session::PromptOrigin::User,
            task_wake_fallback: None,
            tool_overrides_update: None,
            respond_to,
            persist_ack: None,
            parsed_prompt_tx: None,
            queue_meta: queue_id.map(|id| crate::session::prompt_queue::QueueEntryMeta {
                id: id.to_string(),
                version: 0,
                owner: None,
                last_editor: None,
                kind: "prompt".to_string(),
                text: String::new(),
                combined_texts: None,
            }),
            send_now: false,
        };
        (item, rx)
    }
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("running".to_string());
            let (running_item, mut running_rx) = make_item("running", None);
            let (q2_item, mut q2_rx) = make_item("q2-pid", Some("q2"));
            {
                let mut state = actor.state.lock().await;
                state.running_task = None;
                state.pending_inputs.push_back(running_item);
                state.pending_inputs.push_back(q2_item);
            }
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(&actor, cancel_opts(true, false, false, None))
                    .await,
                WakeBarrier::Clear,
            );
            assert!(
                matches!(
                    running_rx.try_recv(),
                    Ok(Ok(crate::session::commands::PromptTurnOk {
                        stop_reason: acp::StopReason::Cancelled,
                        ..
                    }))
                ),
                "front in-flight prompt must be resolved Cancelled even when running_task is None"
            );
            assert!(
                matches!(q2_rx.try_recv(), Err(TryRecvError::Empty)),
                "deeper queued prompt must remain pending, not be cancelled"
            );
            let state = actor.state.lock().await;
            let surviving: Vec<&str> = state
                .pending_inputs
                .iter()
                .map(|i| i.prompt_id.as_str())
                .collect();
            assert_eq!(
                surviving,
                vec!["q2-pid"],
                "front removed; the rest preserved"
            );
            drop(state);
            assert!(
                actor
                    .current_prompt_id
                    .lock()
                    .expect("current_prompt_id mutex poisoned")
                    .is_none(),
                "current_prompt_id should be cleared on cancellation"
            );
        })
        .await;
}
/// Regression: aborting `running_task` must propagate
/// cancellation to the `SamplerHandle` so the sampler stops emitting.
#[tokio::test(flavor = "current_thread")]
async fn cancel_propagates_to_sampler_handle_so_no_further_emission() {
    use axum::Router;
    use axum::response::sse::{Event, Sse};
    use axum::routing::post;
    use futures_util::stream::{self, StreamExt};
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let app = Router::new()
                .route(
                    "/v1/responses",
                    post(|| async {
                        let chunk = serde_json::json!(
                            { "type" : "response.output_text.delta", "sequence_number" :
                            1, "item_id" : "item-1", "output_index" : 0, "content_index"
                            : 0, "delta" : "hi", }
                        );
                        let first = Ok::<
                            _,
                            std::convert::Infallible,
                        >(Event::default().data(chunk.to_string()));
                        Sse::new(stream::iter(vec![first]).chain(stream::pending()))
                    }),
                );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let server_task = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            let cfg = xai_grok_sampler::SamplerConfig {
                api_key: Some("test-key".to_string()),
                failover_api_keys: Vec::new(),
                base_url: format!("http://{addr}/v1"),
                model: "test-model".to_string(),
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                api_backend: xai_grok_sampler::ApiBackend::Responses,
                auth_scheme: Default::default(),
                extra_headers: Default::default(),
                extra_response_includes: Vec::new(),
                query_params: Default::default(),
                env_http_headers: Default::default(),
                context_window: 100_000,
                client_version: None,
                force_http1: false,
                max_retries: Some(0),
                stream_tool_calls: false,
                idle_timeout_secs: Some(60),
                client_identifier: None,
                reasoning_effort: None,
                deployment_id: None,
                user_id: None,
                origin_client: None,
                attribution_callback: None,
                bearer_resolver: None,
                supports_backend_search: false,
                compactions_remaining: None,
                compaction_at_tokens: None,
                doom_loop_recovery: None,
                header_injector: None,
                ..Default::default()
            };
            let (sampler_event_tx, _sampler_event_rx) = tokio::sync::mpsc::unbounded_channel::<
                xai_grok_sampler::SamplingEvent,
            >();
            let sampler_handle = xai_grok_sampler::SamplerActor::spawn(
                cfg,
                xai_grok_sampler::RetryPolicy::default(),
                sampler_event_tx,
            );
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel::<
                xai_acp_lib::AcpClientMessage,
            >();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel::<
                PersistenceMsg,
            >();
            let actor = {
                let mut actor = create_test_actor(
                    0,
                    100_000,
                    85,
                    gateway_tx,
                    persistence_tx,
                )
                .await;
                actor.sampler_handle = sampler_handle.clone();
                actor
                    .agent
                    .borrow()
                    .tool_bridge()
                    .update_resource(
                        xai_grok_tools::implementations::grok_build::task::types::CurrentPromptIdResource(
                            "running".to_string(),
                        ),
                    )
                    .await;
                *actor
                    .current_prompt_id
                    .lock()
                    .expect("current_prompt_id mutex") = Some("running".to_string());
                actor
            };
            let request_id = xai_grok_sampler::RequestId::random();
            let request_id_for_task = request_id.clone();
            let sampler_for_task = sampler_handle.clone();
            let request = ConversationRequest {
                items: vec![
                    ConversationItem::User(xai_grok_sampling_types::UserItem { content :
                    vec![xai_grok_sampling_types::ContentPart::Text { text : "hi".into(),
                    }], synthetic_reason : None, ..Default::default() },)
                ],
                ..Default::default()
            };
            let task = tokio::task::spawn_local(async move {
                let _ = sampler_for_task
                    .submit_and_collect(request_id_for_task, request)
                    .await;
            });
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while !sampler_handle.is_active(request_id.clone()).await {
                if tokio::time::Instant::now() >= deadline {
                    panic!("sampler never registered the in-flight request");
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: task.abort_handle(),
                });
            }
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(&actor, cancel_opts(true, false, false, None))
                    .await,
                WakeBarrier::Clear,
            );
            let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
            let mut still_active = true;
            while tokio::time::Instant::now() < deadline {
                if !sampler_handle.is_active(request_id.clone()).await {
                    still_active = false;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(
                ! still_active, "cancel_running_task did not propagate to the sampler"
            );
            server_task.abort();
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
async fn skill_reminder_deferred_while_turn_running_flushed_when_idle() {
    use xai_grok_tools::types::skill_discovery_tracker::{SkillUpdateEffects, SkillUpdateKind};
    fn effects() -> SkillUpdateEffects {
        SkillUpdateEffects {
            system_reminder: Some("New skill: pdf-tools".into()),
            send_available_commands: false,
            kind: SkillUpdateKind::Discovery,
        }
    }
    async fn reminders_in_conversation(actor: &SessionActor) -> usize {
        actor
            .chat_state_handle
            .get_conversation()
            .await
            .iter()
            .filter(|item| {
                matches!(
                    item, ConversationItem::User(u) if u.content.iter().any(| p |
                    matches!(p, xai_grok_sampling_types::ContentPart::Text { text } if
                    text.contains("pdf-tools")))
                )
            })
            .count()
    }
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gw_tx, _gw_rx) = tokio::sync::mpsc::unbounded_channel();
            let (p_tx, _p_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(0, 200_000, 80, gw_tx, p_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("p1".to_string());
            actor.apply_skill_update_effects(effects()).await;
            assert_eq!(
                actor.pending_skill_reminders.lock().len(),
                1,
                "reminder must be stashed while a turn is running"
            );
            assert_eq!(
                reminders_in_conversation(&actor).await,
                0,
                "reminder must not reach the conversation mid-turn"
            );
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = None;
            actor.flush_pending_skill_reminders().await;
            assert!(
                actor.pending_skill_reminders.lock().is_empty(),
                "flush must drain the stash once the turn is over"
            );
            assert_eq!(
                reminders_in_conversation(&actor).await,
                1,
                "flush must deliver the reminder to the conversation"
            );
            actor.apply_skill_update_effects(effects()).await;
            assert!(
                actor.pending_skill_reminders.lock().is_empty(),
                "idle apply must push immediately, not stash"
            );
            assert_eq!(
                reminders_in_conversation(&actor).await,
                2,
                "idle apply must deliver the reminder immediately"
            );
        })
        .await;
}
/// Cancel (Esc/Ctrl+C) with prompts waiting behind the running one: the queue broadcast to clients
/// keeps waiting prompts in order, drops only the cancelled one, leaves the next free to start.
#[tokio::test(flavor = "current_thread")]
async fn cancel_keeps_remaining_queued_prompts_visible_to_clients() {
    fn make_item(prompt_id: &str, queue_id: &str) -> InputItem {
        let (respond_to, _rx) = tokio::sync::oneshot::channel();
        InputItem {
            prompt_id: prompt_id.to_string(),
            prompt_blocks: vec![],
            prompt_mode: PromptMode::Agent,
            trace_gcs_config: None,
            artifact_tracker: None,
            client_identifier: None,
            screen_mode: None,
            verbatim: false,
            json_schema: None,
            origin: crate::session::PromptOrigin::User,
            task_wake_fallback: None,
            tool_overrides_update: None,
            respond_to,
            persist_ack: None,
            parsed_prompt_tx: None,
            queue_meta: Some(crate::session::prompt_queue::QueueEntryMeta {
                id: queue_id.to_string(),
                version: 0,
                owner: None,
                last_editor: None,
                kind: "prompt".to_string(),
                text: String::new(),
                combined_texts: None,
            }),
            send_now: false,
        }
    }
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            *actor
                .current_prompt_id
                .lock()
                .expect("current_prompt_id mutex poisoned") = Some("running".to_string());
            {
                let mut state = actor.state.lock().await;
                state.running_task = Some(AgentTask {
                    prompt_id: "running".into(),
                    handle: tokio::task::spawn_local(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    })
                    .abort_handle(),
                });
                state
                    .pending_inputs
                    .push_back(make_item("running", "running"));
                state.pending_inputs.push_back(make_item("q1-pid", "q1"));
                state.pending_inputs.push_back(make_item("q2-pid", "q2"));
            }
            let actor = Arc::new(actor);
            assert_eq!(
                cancel_running_task_and_gate_drain(&actor, cancel_opts(true, false, false, None))
                    .await,
                WakeBarrier::Clear,
            );
            let state = actor.state.lock().await;
            let wire = actor.build_queue_wire(&state);
            let wire_ids: Vec<&str> = wire.iter().map(|e| e.id.as_str()).collect();
            assert_eq!(
                wire_ids,
                vec!["q1", "q2"],
                "clients must still see the waiting prompts, in order, cancelled one gone"
            );
            assert_eq!(wire[0].position, 0, "positions must renumber from 0");
            assert_eq!(wire[1].position, 1);
            assert!(
                actor
                    .current_prompt_id
                    .lock()
                    .expect("current_prompt_id mutex poisoned")
                    .is_none(),
                "cancel must clear current_prompt_id so the next prompt can start"
            );
        })
        .await;
}
