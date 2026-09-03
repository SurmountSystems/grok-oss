use super::super::support::*;
use super::super::*;
use super::{AutoCompactTriggerInfo, SuppressReason};
use crate::session::acp_session::McpReminderMode;
use crate::terminal::AsyncTerminalRunner;
use crate::terminal::runner::{TerminalError, TerminalRunRequest, TerminalRunResult};
use std::sync::OnceLock;
use tokio::sync::mpsc;
use xai_grok_paths::AbsPathBuf;
use xai_grok_workspace::file_system::MockFs;
use xai_grok_workspace::permission::PermissionHandle;
#[derive(Debug)]
struct DummyTerminal;
#[async_trait::async_trait]
impl AsyncTerminalRunner for DummyTerminal {
    async fn run(&self, _request: TerminalRunRequest) -> Result<TerminalRunResult, TerminalError> {
        Err(TerminalError::Other("dummy terminal".into()))
    }
}
/// Create a minimal SessionActor for testing auto-compact logic.
async fn create_test_actor(
    total_tokens: u64,
    context_window: u64,
    threshold_percent: u8,
    gateway_tx: mpsc::UnboundedSender<xai_acp_lib::AcpClientMessage>,
    persistence_tx: mpsc::UnboundedSender<PersistenceMsg>,
) -> SessionActor {
    let cwd = AbsPathBuf::new(std::path::PathBuf::from("/tmp")).unwrap();
    let fs = Arc::new(MockFs::new(cwd.to_path_buf()));
    let terminal = Arc::new(DummyTerminal {});
    let (hunk_tx, _hunk_rx) = tokio::sync::mpsc::unbounded_channel();
    let hunk_tracker_handle = xai_hunk_tracker::HunkTrackerActor::spawn(
        "test-auto-compact".to_string(),
        cwd.to_path_buf(),
        hunk_tx,
        xai_hunk_tracker::TrackingMode::AgentOnly,
        tokio_util::sync::CancellationToken::new(),
    );
    let tool_context = ToolContext::new(cwd.clone(), None, None, fs, terminal, hunk_tracker_handle);
    let state = TokioMutex::new(State {
        running_task: None,
        pending_inputs: VecDeque::new(),
        edit_holds: HashMap::new(),
        pending_notifications: Vec::new(),
        notifications_suppressed: false,
        rewindable: false,
        front_message_committed: false,
        nudges_used_this_session: 0,
    });
    let (chat_event_tx, _chat_event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, _event_rx) =
        tokio::sync::mpsc::unbounded_channel::<crate::session::replay_events::SessionEvent>();
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
            query_params: Default::default(),
            env_http_headers: Default::default(),
            context_window: std::num::NonZeroU64::new(context_window)
                .expect("test context_window must be non-zero"),
            reasoning_effort: None,
            stream_tool_calls: None,
        },
        Box::new(xai_chat_state::NullChatPersistence),
        chat_event_tx,
        tokio_util::sync::CancellationToken::new(),
    );
    chat_state_handle.record_token_usage(total_tokens);
    SessionActor {
        unattributed_background_usage: std::sync::atomic::AtomicBool::new(false),
        session_info: SessionInfo {
            id: acp::SessionId::new("test-auto-compact"),
            cwd: cwd.as_str().to_string(),
        },
        auth_method_id: test_auth_method_id("test-auth"),
        model_auth_memo: std::cell::RefCell::new(None),
        attribution_callback: None,
        auth_manager: None,
        is_chat_kind: false,
        state,
        notifications: NotificationSender {
            gateway: GatewaySender::new(gateway_tx),
            gateway_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            persistence_tx,
            disk_full: crate::session::notifications::idle_disk_full_rx(),
        },
        permissions: PermissionHandle::allow_all(),
        context_only: std::sync::atomic::AtomicBool::new(false),
        tool_context,
        deny_read_globs: Vec::new(),
        mcp_state: Arc::new(TokioMutex::new(McpState::new(vec![]))),
        mcp_strategy: std::cell::Cell::new(McpInitStrategy::Blocking),
        delivery_tools: std::cell::RefCell::new(Vec::new()),
        attach_non_interactive: std::cell::Cell::new(false),
        chat_state_handle,
        current_prompt_id: std::sync::Arc::new(std::sync::Mutex::new(None)),
        pending_interactions: std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        telemetry_enabled: false,
        supports_backend_search: std::cell::Cell::new(false),
        tool_overrides: std::cell::RefCell::new(None),
        resolved_tool_overrides: std::sync::Arc::new(arc_swap::ArcSwapOption::empty()),
        compactions_remaining: std::cell::Cell::new(None),
        compaction_at_tokens: std::cell::Cell::new(None),
        doom_loop_recovery: None,
        doom_loop_turn_tally: Default::default(),
        file_state_tracker: Arc::new(FileStateTracker::new()),
        rewind_pending_prompt: std::sync::Mutex::new(None),
        startup_hints: StartupHints::default(),
        forked_tool_override: None,
        compaction: crate::session::compaction_config::CompactionConfig {
            threshold_percent: std::cell::Cell::new(threshold_percent),
            threshold_tokens: std::cell::Cell::new(None),
            force_compact: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            context_window_override: None,
            economic_mode: std::cell::Cell::new(false),
            model_context_window: std::cell::Cell::new(0),
            count: std::sync::atomic::AtomicU64::new(0),
            auto_compact_suppressed: std::sync::atomic::AtomicU8::new(0),
            last_auto_compact_saved_too_little: std::sync::atomic::AtomicBool::new(false),
            previous_model: std::cell::Cell::new(None),
            compaction_mode: xai_chat_state::CompactionMode::Transcript,
            verbatim_input: true,
            tool_choice: crate::util::config::CompactionToolChoice::Auto,
            prefire: crate::session::compaction_config::PrefireState::default(),
            prefix_released: std::sync::atomic::AtomicBool::new(false),
            cancel: Default::default(),
        },
        memory: crate::session::memory_state::SessionMemory {
            flush_config: crate::config::MemoryFlushConfig::default(),
            is_flushing: std::sync::atomic::AtomicBool::new(false),
            last_flush_compaction: std::sync::atomic::AtomicU64::new(0),
            storage: std::cell::RefCell::new(None),
            save_on_end: true,
            backend_params: None,
            initial_injection_config: Default::default(),
            context_injected: std::sync::atomic::AtomicBool::new(false),
            flush_count: std::sync::atomic::AtomicU64::new(0),
            last_flush_content: std::cell::RefCell::new(None),
            flush_success_count: std::sync::atomic::AtomicU64::new(0),
            flush_error_count: std::sync::atomic::AtomicU64::new(0),
            search_counter: std::cell::RefCell::new(None),
            injection_count: std::sync::atomic::AtomicU64::new(0),
            compaction_recovery_count: std::sync::atomic::AtomicU64::new(0),
            chunks_added: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            dream_config: Default::default(),
            dream_count: std::sync::atomic::AtomicU64::new(0),
            dream_success_count: std::sync::atomic::AtomicU64::new(0),
            dream_error_count: std::sync::atomic::AtomicU64::new(0),
        },
        session_start: std::time::Instant::now(),
        inference_idle_timeout: std::time::Duration::from_secs(300),
        max_retries: 3,
        max_turns: None,
        pending_interjections: InterjectionBuffer::new(),
        pending_skill_reminders: Mutex::new(Vec::new()),
        idle_flush_timeout: None,
        dream_check_timeout: None,
        last_idle_flush_conversation_len: std::sync::atomic::AtomicUsize::new(0),
        event_tx,
        buffering_settings: None,
        client_identifier: None,
        origin_client: None,
        feedback_manager: Arc::new(FeedbackManager::local_only("test-session")),
        upload_queue: Arc::new(OnceLock::new()),
        sync_loop_cancel: None,
        agent: std::cell::RefCell::new(test_agent_default().await),
        last_reported_branch: std::sync::Arc::new(parking_lot::Mutex::new(None)),
        git_head_enabled: false,
        models_manager: Default::default(),
        display_cwd: std::sync::OnceLock::new(),
        active_agent_type: parking_lot::Mutex::new(None),
        queue_exit_reminder_on_approved_exit: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        active_skill: parking_lot::Mutex::new(None),
        current_prompt_mode: Arc::new(parking_lot::Mutex::new(PromptMode::Agent)),
        turn_start_prompt_mode: parking_lot::Mutex::new(PromptMode::Agent),
        turn_prompt_mode: Arc::new(parking_lot::Mutex::new(PromptMode::Agent)),
        plan_mode: Arc::new(parking_lot::Mutex::new(
            crate::session::plan_mode::PlanModeTracker::new(std::path::PathBuf::from(
                "/tmp/test-session",
            )),
        )),
        goal_enabled: false,
        background_workflows_enabled: false,
        goal_harness_enabled: std::sync::atomic::AtomicBool::new(false),
        goal_harness_availability_reconciled: std::sync::atomic::AtomicBool::new(false),
        goal_tracker: Arc::new(parking_lot::Mutex::new(
            crate::session::goal_tracker::GoalTracker::new(std::path::PathBuf::from(
                "/tmp/test-session",
            )),
        )),
        goal_turn_task_ids: parking_lot::Mutex::new(std::collections::HashSet::new()),
        goal_continuation_streak: std::sync::atomic::AtomicU32::new(0),
        goal_blocked_streak: std::sync::atomic::AtomicU32::new(0),
        goal_update_rx: std::cell::RefCell::new(None),
        goal_update_tx: tokio::sync::mpsc::unbounded_channel().0,
        workflow_manager: crate::session::workflow::manager::WorkflowManager::test_bundle().0,
        workflow_launch_tx: tokio::sync::mpsc::unbounded_channel().0,
        goal_classifier_enabled: false,
        goal_planner_enabled: false,
        goal_summary_enabled: false,
        goal_verifier_skeptic_count: 1,
        goal_role_models: Default::default(),
        goal_use_current_model_only: false,
        goal_classifier_max_runs: crate::session::goal_classifier::GOAL_CLASSIFIER_MAX_RUNS_DEFAULT,
        goal_strategist_every: 5,
        goal_reverify_after: crate::session::acp_session::GOAL_REVERIFY_AFTER_DEFAULT,
        goal_plan_reconciled: std::sync::atomic::AtomicBool::new(false),
        pending_classifier_completions: parking_lot::Mutex::new(std::collections::VecDeque::new()),
        goal_classifier_in_flight: std::sync::atomic::AtomicBool::new(false),
        managed_mcp_handle: Default::default(),
        initial_client_mcp_servers: vec![],
        tool_metadata_snapshot: Arc::new(std::sync::Mutex::new(Default::default())),
        mcp_announced_servers: parking_lot::Mutex::new(std::collections::HashMap::new()),
        mcp_reminder_mode: McpReminderMode::Delta,
        mcp_reminder_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        mcp_connecting_reminder_injected: std::cell::Cell::new(false),
        mcp_handshakes_done: Arc::new(tokio::sync::Notify::new()),
        user_input_generation: std::sync::atomic::AtomicU64::new(0),
        laziness_debug_log: None,
        last_live_orphan_reconcile: std::cell::Cell::new(None),
        deferred_prefix: TaskSlot::new(),
        extension_registry: xai_agent_lifecycle::LocalExtensionRegistry::default(),
        last_announced_local_date: std::cell::Cell::new(chrono::Local::now().date_naive()),
        prefix_carries_fallback_date: std::cell::Cell::new(false),
        last_search_prompt_index: std::sync::atomic::AtomicI64::new(-1),
        last_api_request_at: std::sync::atomic::AtomicI64::new(0),
        hook_registry: std::cell::RefCell::new(None),
        client_hooks: Default::default(),
        hook_resolved_workspace_root: String::new(),
        vcs_kind: xai_grok_workspace::session::git::VcsKind::Git,
        hook_load_errors: std::cell::RefCell::new(Vec::new()),
        plugin_registry: std::cell::RefCell::new(None),
        plugin_registry_handle: None,
        events: crate::session::events::EventTracker::new(std::path::Path::new("/tmp")),
        observability_bridge: noop_observability_bridge(),
        current_turn_number: std::cell::Cell::new(0),
        last_recap_main_turn: std::cell::Cell::new(0),
        recap_in_flight: std::cell::Cell::new(false),
        recap_epoch: std::cell::Cell::new(0),
        turn_summary_task: std::cell::RefCell::new(None),
        turn_summary_generation: std::cell::Cell::new(0),
        turn_summary_enabled: false,
        session_turn_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        streaming_turn_capture: parking_lot::Mutex::new(
            crate::session::acp_session::StreamingTurnCapture::default(),
        ),
        turn_stream_drained: parking_lot::Mutex::new(None),
        pending_image_strip: parking_lot::Mutex::new(None),
        sampler_handle: xai_grok_sampler::SamplerHandle::noop(),
        rebuild_spec: crate::session::agent_rebuild::test_rebuild_spec_default(),
        image_description_model: crate::test_support::TEST_MODEL.to_owned(),
        image_describe_cache: Arc::new(crate::session::image_describe::ImageDescribeCache::new()),
        subagent_token_records: parking_lot::Mutex::new(std::collections::HashMap::new()),
        workspace_ops: xai_grok_workspace::WorkspaceOps::for_test(),
        trace_config_template: std::cell::RefCell::new(None),
    }
}
/// Test check_auto_compact_needed uses state values.
#[tokio::test(flavor = "current_thread")]
async fn test_check_auto_compact_needed_uses_state() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(90_000, 100_000, 85, gateway_tx, persistence_tx).await;
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_some(), "Should trigger at 90%");
            let info = result.unwrap();
            assert_eq!(info.percentage, 90);
        })
        .await;
}
/// Test that overriding context_window on the sampling config changes
/// auto-compact behavior. Forked sessions must use the new model's
/// context window, not the source session's. Without this, auto-compact
/// fires at the wrong threshold.
#[tokio::test(flavor = "current_thread")]
async fn test_context_window_override_affects_auto_compact() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(86_000, 100_000, 85, gateway_tx, persistence_tx).await;
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_some(), "Should trigger at 86% of 100K window");
            if let Some(mut cfg) = actor.chat_state_handle.get_sampling_config().await {
                cfg.model = "larger-model".to_string();
                cfg.context_window = std::num::NonZeroU64::new(200_000).unwrap();
                actor.chat_state_handle.update_sampling_config(cfg);
            }
            let result = actor.check_auto_compact_needed().await;
            assert!(
                result.is_none(),
                "Should NOT trigger at 43% of 200K window after context_window override"
            );
        })
        .await;
}
/// Test the reverse direction: overriding to a smaller context window
/// should make auto-compact trigger sooner.
#[tokio::test(flavor = "current_thread")]
async fn test_context_window_override_to_smaller_triggers_compact() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(86_000, 200_000, 85, gateway_tx, persistence_tx).await;
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_none(), "Should NOT trigger at 43% of 200K window");
            if let Some(mut cfg) = actor.chat_state_handle.get_sampling_config().await {
                cfg.model = "smaller-model".to_string();
                cfg.context_window = std::num::NonZeroU64::new(100_000).unwrap();
                actor.chat_state_handle.update_sampling_config(cfg);
            }
            let result = actor.check_auto_compact_needed().await;
            assert!(
                result.is_some(),
                "Should trigger at 86% of 100K window after context_window override"
            );
        })
        .await;
}
/// Suppression gates both AUTO paths; the reset scope depends on the reason:
/// `other` clears next turn, `credit_block` holds until a successful model call,
/// `size` is sticky until a full reset (success / rewind / model switch).
#[tokio::test(flavor = "current_thread")]
async fn suppression_gates_and_reset_is_reason_scoped() {
    use crate::session::compaction_config::{SUPPRESS_NONE, SUPPRESS_TURN, SUPPRESS_UNTIL_SUCCESS};
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(214_000, 200_000, 85, gateway_tx, persistence_tx).await;
            let err = api_error_with_context_window(200_000);
            assert!(actor.check_auto_compact_needed().await.is_some());
            assert!(actor.should_compact_on_error(&err).await);
            actor
                .suppress_auto_compaction(SuppressReason::Other, 1_000, 200_000, None)
                .await;
            assert!(actor.check_auto_compact_needed().await.is_none());
            assert!(!actor.should_compact_on_error(&err).await);
            let _ = actor.compaction.auto_compact_suppressed.compare_exchange(
                SUPPRESS_TURN,
                SUPPRESS_NONE,
                Relaxed,
                Relaxed,
            );
            assert!(actor.check_auto_compact_needed().await.is_some());
            actor
                .suppress_auto_compaction(SuppressReason::CreditBlock, 1_000, 200_000, None)
                .await;
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_UNTIL_SUCCESS
            );
            assert!(actor.check_auto_compact_needed().await.is_none());
            assert!(!actor.should_compact_on_error(&err).await);
            let _ = actor.compaction.auto_compact_suppressed.compare_exchange(
                SUPPRESS_TURN,
                SUPPRESS_NONE,
                Relaxed,
                Relaxed,
            );
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "credit-block suppression must survive the per-turn reset"
            );
            let _ = actor.compaction.auto_compact_suppressed.compare_exchange(
                SUPPRESS_UNTIL_SUCCESS,
                SUPPRESS_NONE,
                Relaxed,
                Relaxed,
            );
            assert!(actor.check_auto_compact_needed().await.is_some());
            actor
                .suppress_auto_compaction(SuppressReason::Size, 1_000, 200_000, None)
                .await;
            assert!(actor.check_auto_compact_needed().await.is_none());
            let _ = actor.compaction.auto_compact_suppressed.compare_exchange(
                SUPPRESS_TURN,
                SUPPRESS_NONE,
                Relaxed,
                Relaxed,
            );
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "sticky suppression must survive the per-turn reset"
            );
            actor
                .compaction
                .auto_compact_suppressed
                .store(SUPPRESS_NONE, Relaxed);
            assert!(actor.check_auto_compact_needed().await.is_some());
        })
        .await;
}
/// A model switch clears suppression the switch (or the fresh budget-driven
/// trigger) can resolve — sticky size/schema and a stale per-turn `other` — so
/// the gates re-evaluate against the new window. Account-state credit/auth is
/// covered by `model_switch_keeps_account_state_suppression`.
#[tokio::test(flavor = "current_thread")]
async fn model_switch_clears_sticky_suppression() {
    use crate::session::compaction_config::{PreviousModelInfo, SUPPRESS_NONE};
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(50_000, 200_000, 85, gateway_tx, persistence_tx).await);
            for reason in [SuppressReason::Size, SuppressReason::Other] {
                actor
                    .suppress_auto_compaction(reason, 1_000, 200_000, None)
                    .await;
                assert_ne!(
                    actor.compaction.auto_compact_suppressed.load(Relaxed),
                    SUPPRESS_NONE,
                    "{reason:?} should set suppression"
                );
                actor.compaction.previous_model.set(Some(PreviousModelInfo {
                    model_slug: "old-small-model".to_string(),
                    context_window: 100_000,
                }));
                actor
                    .maybe_compact_on_model_switch()
                    .await
                    .expect("non-auth model-switch path must not abort");
                assert_eq!(
                    actor.compaction.auto_compact_suppressed.load(Relaxed),
                    SUPPRESS_NONE,
                    "model switch must clear {reason:?} suppression so the gates re-evaluate"
                );
            }
        })
        .await;
}
/// Model switch must not clear credit/auth suppress or compact under it.
#[tokio::test(flavor = "current_thread")]
async fn model_switch_keeps_account_state_suppression() {
    use crate::session::compaction_config::{
        PreviousModelInfo, SUPPRESS_AUTH, SUPPRESS_UNTIL_SUCCESS,
    };
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(214_000, 200_000, 85, gateway_tx, persistence_tx).await);
            for (reason, expected) in [
                (SuppressReason::CreditBlock, SUPPRESS_UNTIL_SUCCESS),
                (SuppressReason::Auth, SUPPRESS_AUTH),
            ] {
                actor
                    .suppress_auto_compaction(reason, 1_000, 200_000, None)
                    .await;
                assert_eq!(
                    actor.compaction.auto_compact_suppressed.load(Relaxed),
                    expected,
                    "{reason:?} suppress state"
                );
                actor.compaction.previous_model.set(Some(PreviousModelInfo {
                    model_slug: "old-big-model".to_string(),
                    context_window: 400_000,
                }));
                actor
                    .maybe_compact_on_model_switch()
                    .await
                    .expect("suppressed model-switch path must not abort");
                assert_eq!(
                    actor.compaction.auto_compact_suppressed.load(Relaxed),
                    expected,
                    "model switch must NOT clear {reason:?} suppression"
                );
                actor
                    .compaction
                    .auto_compact_suppressed
                    .store(crate::session::compaction_config::SUPPRESS_NONE, Relaxed);
            }
        })
        .await;
}
/// Auth suppress clears on credential recovery, not on a model 200.
#[tokio::test(flavor = "current_thread")]
async fn auth_suppress_clears_on_credential_recovery() {
    use crate::session::compaction_config::{SUPPRESS_AUTH, SUPPRESS_NONE};
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(180_000, 200_000, 85, gateway_tx, persistence_tx).await;
            actor
                .suppress_auto_compaction(SuppressReason::Auth, 1_000, 200_000, None)
                .await;
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_AUTH
            );
            assert!(actor.check_auto_compact_needed().await.is_none());
            actor.clear_auth_compact_suppression();
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE
            );
            assert!(actor.check_auto_compact_needed().await.is_some());
        })
        .await;
}
/// Auth recovery must not clear credit suppress.
#[tokio::test(flavor = "current_thread")]
async fn clear_auth_suppress_leaves_credit_suppress() {
    use crate::session::compaction_config::SUPPRESS_UNTIL_SUCCESS;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(180_000, 200_000, 85, gateway_tx, persistence_tx).await;
            actor
                .suppress_auto_compaction(SuppressReason::CreditBlock, 1_000, 200_000, None)
                .await;
            actor.clear_auth_compact_suppression();
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_UNTIL_SUCCESS,
                "credential recovery must not clear a credit-block suppress"
            );
        })
        .await;
}
/// After /login, clearing auth suppress must re-arm pre-sampling compact
/// before the next sample (ordering that prepare_sampler-after-gate broke).
#[tokio::test(flavor = "current_thread")]
async fn clear_auth_suppress_rearms_pre_sampling_compact_gate() {
    use crate::session::compaction_config::SUPPRESS_AUTH;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(180_000, 200_000, 85, gateway_tx, persistence_tx).await;
            actor
                .suppress_auto_compaction(SuppressReason::Auth, 1_000, 200_000, None)
                .await;
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_AUTH
            );
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "auth suppress must block pre-sampling compact"
            );
            actor.clear_auth_compact_suppression();
            assert!(
                actor.check_auto_compact_needed().await.is_some(),
                "after credential recovery, pre-sampling compact must re-arm"
            );
        })
        .await;
}
/// A user-cancelled auto-compact must not immediately re-arm AUTO. Context
/// still over threshold (including "100% full") used to call compact again
/// on the same turn via pre-sampling and the sampling-error recovery path.
#[tokio::test(flavor = "current_thread")]
async fn user_cancelled_auto_compact_does_not_immediately_rearm() {
    use crate::session::compaction_config::{SUPPRESS_NONE, SUPPRESS_TURN};
    use crate::session::helpers::session_compact::COMPACT_CANCELLED_MSG;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, mut persistence_rx) = mpsc::unbounded_channel();
            // Over the 200k window (display 100% full). Threshold 85% still fires.
            let actor = create_test_actor(214_000, 200_000, 85, gateway_tx, persistence_tx).await;
            let overflow = api_error_with_context_window(200_000);
            assert!(
                actor.check_auto_compact_needed().await.is_some(),
                "100% full must still be over the auto-compact threshold"
            );
            assert!(
                actor.should_compact_on_error(&overflow).await,
                "overflow recovery must be armed before cancel"
            );

            let err = actor.emit_compact_cancelled(true).await.unwrap_err();
            let msg = err
                .data
                .as_ref()
                .and_then(|d| d.as_str())
                .unwrap_or_default();
            assert!(
                msg.contains(COMPACT_CANCELLED_MSG),
                "cancel must return the stable compact-cancelled payload, got {msg:?}"
            );
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_TURN,
                "user cancel must suppress AUTO for the rest of this turn"
            );
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "cancelled auto-compact must not re-arm pre-sampling compact"
            );
            assert!(
                !actor.should_compact_on_error(&overflow).await,
                "cancelled auto-compact must not re-arm overflow recovery compact"
            );
            assert!(
                actor.check_preflight_overflow().await.is_none(),
                "cancelled auto-compact must not re-arm preflight overflow compact"
            );

            let mut saw_cancelled = false;
            let mut saw_failed = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(notif)) =
                    msg
                {
                    match &notif.update {
                        crate::extensions::notification::SessionUpdate::AutoCompactCancelled {
                            ..
                        } => saw_cancelled = true,
                        crate::extensions::notification::SessionUpdate::AutoCompactFailed {
                            ..
                        } => saw_failed = true,
                        _ => {}
                    }
                }
            }
            assert!(saw_cancelled, "user cancel must emit AutoCompactCancelled");
            assert!(!saw_failed, "user cancel must not emit AutoCompactFailed");

            // Next user turn may retry: the per-turn reset is the intended re-arm.
            let _ = actor.compaction.auto_compact_suppressed.compare_exchange(
                SUPPRESS_TURN,
                SUPPRESS_NONE,
                Relaxed,
                Relaxed,
            );
            assert!(
                actor.check_auto_compact_needed().await.is_some(),
                "a later user turn may auto-compact again after the per-turn reset"
            );
        })
        .await;
}
#[test]
fn auto_compact_198k_to_190k_is_not_useful() {
    assert!(
        !super::auto_compact_savings_are_useful(198_300, 190_300),
        "8k of 198.3k is under 20% and must not count as useful AUTO savings"
    );
}

#[test]
fn auto_compact_200k_to_12k_is_useful() {
    assert!(
        super::auto_compact_savings_are_useful(200_000, 12_000),
        "a 200k to 12k drop must stay on the useful AUTO success path"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn auto_compact_tiny_savings_sets_sticky_and_does_not_rearm() {
    use crate::session::compaction_config::{SUPPRESS_NONE, SUPPRESS_STICKY};
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, mut persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(198_300, 200_000, 95, gateway_tx, persistence_tx).await;
            assert!(
                actor.check_auto_compact_needed().await.is_some(),
                "198.3k of a 200k sampling window must still arm AUTO"
            );
            let useful = actor.record_auto_compact_savings(198_300, 190_300);
            assert!(!useful, "198.3k to 190.3k must not be useful savings");
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_STICKY,
                "tiny AUTO savings must sticky-suppress further AUTO"
            );
            assert_ne!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE
            );
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "tiny-savings sticky must not start another AUTO compact"
            );
            assert!(
                !actor
                    .should_compact_on_error(&api_error_with_context_window(200_000))
                    .await,
                "tiny-savings sticky must not re-arm overflow recovery compact"
            );
            assert!(
                actor.check_preflight_overflow().await.is_none(),
                "tiny-savings sticky must not re-arm preflight overflow compact"
            );

            let mut saw_skip = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(notif)) =
                    msg
                {
                    if matches!(
                        &notif.update,
                        crate::extensions::notification::SessionUpdate::AutoCompactSkippedTinySavings
                    ) {
                        saw_skip = true;
                    }
                    if let crate::extensions::notification::SessionUpdate::AutoCompactStarted {
                        ..
                    } = &notif.update
                    {
                        panic!("tiny-savings skip must not paint AutoCompactStarted");
                    }
                }
            }
            assert!(
                saw_skip,
                "next AUTO check after tiny savings must say auto-compact is skipped because the last compact saved too little"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn auto_compact_useful_savings_does_not_set_tiny_savings_sticky() {
    use crate::session::compaction_config::{SUPPRESS_NONE, SUPPRESS_STICKY};
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(12_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor
                .compaction
                .auto_compact_suppressed
                .store(SUPPRESS_NONE, Relaxed);
            let useful = actor.record_auto_compact_savings(200_000, 12_000);
            assert!(useful, "200k to 12k must stay on the useful success path");
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE,
                "useful savings must not set tiny-savings sticky"
            );
            assert_ne!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_STICKY,
                "useful savings must not set sticky just because AUTO finished"
            );
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "12k of 200k is under threshold; AUTO must not fire from useful savings"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn inherited_prefix_still_over_sticky_survives_useful_savings() {
    use crate::session::compaction_config::SUPPRESS_STICKY;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(12_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor
                .compaction
                .auto_compact_suppressed
                .store(SUPPRESS_STICKY, Relaxed);
            let useful = actor.record_auto_compact_savings(200_000, 12_000);
            assert!(useful);
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_STICKY,
                "useful savings must not clear inherited-prefix still-over sticky"
            );
        })
        .await;
}

#[test]
fn is_auth_compact_error_classifies_401_messages() {
    let auth =
        acp::Error::internal_error().data("compact failed: API error (status 401 Unauthorized)");
    assert!(SessionActor::is_auth_compact_error(&auth));
    let credit = acp::Error::internal_error().data("compact failed: out of credits");
    assert!(!SessionActor::is_auth_compact_error(&credit));
    let size = acp::Error::internal_error()
        .data("compact failed: The prompt is too long for this model's context window.");
    assert!(!SessionActor::is_auth_compact_error(&size));
}
#[tokio::test(flavor = "current_thread")]
async fn surface_compact_auth_failure_emits_reauthable_retry_state() {
    use crate::extensions::notification::SessionUpdate as XaiSessionUpdate;
    use crate::session::storage::SessionUpdate;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, mut persistence_rx) = mpsc::unbounded_channel();
            let actor = create_test_actor(10_000, 200_000, 85, gateway_tx, persistence_tx).await;
            let err = acp::Error::internal_error()
                .data("compact failed: API error (status 401 Unauthorized)");
            let out = actor.surface_compact_auth_failure(err).await;
            assert_eq!(out.code, acp::Error::auth_required().code);
            let mut saw_retry_auth = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(SessionUpdate::Xai(notif)) = msg
                    && let XaiSessionUpdate::RetryState(
                        crate::extensions::notification::RetryState::Failed {
                            error_type,
                            message,
                        },
                    ) = &notif.update
                {
                    assert_eq!(error_type, "auth");
                    assert!(
                        message.contains("Unauthorized (401)") || message.contains("401"),
                        "message={message}"
                    );
                    saw_retry_auth = true;
                }
            }
            assert!(
                saw_retry_auth,
                "expected RetryState::Failed auth notification"
            );
        })
        .await;
}
/// The per-turn suppression notification is tailored to the failure reason.
#[tokio::test(flavor = "current_thread")]
async fn suppression_notification_is_reason_specific() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            async fn notification_for(reason: SuppressReason) -> String {
                let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
                let (persistence_tx, mut persistence_rx) = mpsc::unbounded_channel();
                let actor =
                    create_test_actor(10_000, 200_000, 85, gateway_tx, persistence_tx).await;
                actor
                    .suppress_auto_compaction(reason, 1_000, 200_000, None)
                    .await;
                let mut text = None;
                while let Ok(msg) = persistence_rx.try_recv() {
                    if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(
                        notif,
                    )) = msg
                        && let crate::extensions::notification::SessionUpdate::AutoCompactFailed {
                            error,
                        } = &notif.update
                    {
                        text = Some(error.clone());
                    }
                }
                text.expect("expected an AutoCompactFailed notification")
            }
            let credit = notification_for(SuppressReason::CreditBlock).await;
            assert!(
                !credit.to_ascii_lowercase().contains("add credits"),
                "credit_block must not command add credits: {credit}"
            );
            assert!(
                credit.contains("client printout") || credit.contains("not xAI billing truth"),
                "credit_block: {credit}"
            );
            let auth = notification_for(SuppressReason::Auth).await;
            assert!(auth.contains("/login"), "auth: {auth}");
            let size = notification_for(SuppressReason::Size).await;
            assert!(size.contains("too large to compact"), "size: {size}");
            let schema = notification_for(SuppressReason::Schema).await;
            assert!(schema.contains("can't be summarized"), "schema: {schema}");
            let other = notification_for(SuppressReason::Other).await;
            assert!(other.contains("/new"), "other: {other}");
        })
        .await;
}
/// Mock LLM endpoint answering every request with a deterministic 400.
async fn spawn_deterministic_400_server() -> String {
    spawn_status_body_server(
        400,
        r#"{"error":{"type":"invalid_request_error","message":"bad schema"}}"#,
    )
    .await
}
/// Mock LLM that answers every request with 401.
async fn spawn_deterministic_401_server() -> String {
    spawn_status_body_server(
        401,
        r#"{"error":{"type":"authentication_error","message":"Unauthorized (401)"}}"#,
    )
    .await
}
async fn spawn_status_body_server(status: u16, body: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let status_line = match status {
        400 => "400 Bad Request",
        401 => "401 Unauthorized",
        other => panic!("add status line for {other}"),
    };
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}
/// 401 auto-compact: SUPPRESS_AUTH + reauthable RetryState (abort for /login).
#[tokio::test(flavor = "current_thread")]
async fn e2e_auto_compact_401_suppresses_auth_and_surfaces_reauth() {
    use crate::extensions::notification::SessionUpdate as XaiSessionUpdate;
    use crate::session::compaction_config::SUPPRESS_AUTH;
    use crate::session::storage::SessionUpdate;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, mut persistence_rx) = mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(180_000, 200_000, 85, gateway_tx, persistence_tx).await);
            let base_url = spawn_deterministic_401_server().await;
            let mut cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            cfg.base_url = base_url;
            actor.chat_state_handle.update_sampling_config(cfg);
            actor.chat_state_handle.replace_conversation(vec![
                ConversationItem::system("sys"),
                ConversationItem::user("hello"),
                ConversationItem::assistant("hi"),
                ConversationItem::user("compact me"),
            ]);
            let err = actor
                .run_compact_only(AutoCompactTriggerInfo {
                    tokens_used: 180_000,
                    context_window: 200_000,
                    percentage: 90,
                })
                .await
                .expect_err("401 mock must fail auto-compact");
            assert!(
                SessionActor::is_auth_compact_error(&err),
                "401 compact failure must classify as auth: {err:?}"
            );
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_AUTH,
                "auth compact failure must use SUPPRESS_AUTH (cleared on re-login)"
            );
            let surfaced = actor.surface_compact_auth_failure(err).await;
            assert_eq!(surfaced.code, acp::Error::auth_required().code);
            let mut saw_retry_auth = false;
            let mut saw_auto_failed = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(SessionUpdate::Xai(notif)) = msg {
                    match &notif.update {
                        XaiSessionUpdate::RetryState(
                            crate::extensions::notification::RetryState::Failed {
                                error_type,
                                message,
                            },
                        ) => {
                            assert_eq!(error_type, "auth");
                            assert!(
                                message.contains("Unauthorized") || message.contains("401"),
                                "message={message}"
                            );
                            saw_retry_auth = true;
                        }
                        XaiSessionUpdate::AutoCompactFailed { error } => {
                            assert!(
                                error.contains("/login") || error.contains("authentication"),
                                "auto-failed={error}"
                            );
                            saw_auto_failed = true;
                        }
                        _ => {}
                    }
                }
            }
            assert!(saw_auto_failed, "expected AutoCompactFailed notification");
            assert!(
                saw_retry_auth,
                "expected RetryState::Failed auth so pager can stash + reauth"
            );
            actor.clear_auth_compact_suppression();
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                crate::session::compaction_config::SUPPRESS_NONE
            );
        })
        .await;
}
/// Model-switch compact 401 must surface reauth (same path as pre-sampling).
#[tokio::test(flavor = "current_thread")]
async fn e2e_model_switch_compact_401_surfaces_reauth() {
    use crate::extensions::notification::SessionUpdate as XaiSessionUpdate;
    use crate::session::compaction_config::{PreviousModelInfo, SUPPRESS_AUTH};
    use crate::session::storage::SessionUpdate;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, mut persistence_rx) = mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(214_000, 200_000, 85, gateway_tx, persistence_tx).await);
            let base_url = spawn_deterministic_401_server().await;
            let mut cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            cfg.base_url = base_url;
            actor.chat_state_handle.update_sampling_config(cfg);
            actor.chat_state_handle.replace_conversation(vec![
                ConversationItem::system("sys"),
                ConversationItem::user("hello"),
                ConversationItem::assistant("hi"),
                ConversationItem::user("compact me"),
            ]);
            actor.chat_state_handle.record_token_usage(214_000);
            actor.compaction.previous_model.set(Some(PreviousModelInfo {
                model_slug: "old-big-model".to_string(),
                context_window: 400_000,
            }));
            let err = actor
                .maybe_compact_on_model_switch()
                .await
                .expect_err("model-switch 401 compact must abort for reauth");
            assert_eq!(err.code, acp::Error::auth_required().code);
            assert!(
                SessionActor::is_auth_compact_error(&err)
                    || err.message.to_ascii_lowercase().contains("unauthorized")
                    || format!("{err:?}").contains("401"),
                "surfaced error should be reauthable auth: {err:?}"
            );
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_AUTH,
                "auth compact failure must use SUPPRESS_AUTH"
            );
            let mut saw_retry_auth = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(SessionUpdate::Xai(notif)) = msg
                    && let XaiSessionUpdate::RetryState(
                        crate::extensions::notification::RetryState::Failed {
                            error_type,
                            message,
                        },
                    ) = &notif.update
                {
                    assert_eq!(error_type, "auth");
                    assert!(
                        message.contains("Unauthorized") || message.contains("401"),
                        "message={message}"
                    );
                    saw_retry_auth = true;
                }
            }
            assert!(
                saw_retry_auth,
                "expected RetryState::Failed auth so pager can stash + reauth"
            );
        })
        .await;
}
/// Non-auth model-switch compact failures stay log-only (turn continues).
#[tokio::test(flavor = "current_thread")]
async fn e2e_model_switch_compact_non_auth_failure_does_not_abort() {
    use crate::session::compaction_config::{PreviousModelInfo, SUPPRESS_NONE};
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(214_000, 200_000, 85, gateway_tx, persistence_tx).await);
            let base_url = spawn_deterministic_400_server().await;
            let mut cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            cfg.base_url = base_url;
            actor.chat_state_handle.update_sampling_config(cfg);
            actor.chat_state_handle.replace_conversation(vec![
                ConversationItem::system("sys"),
                ConversationItem::user("hello"),
            ]);
            actor.chat_state_handle.record_token_usage(214_000);
            actor.compaction.previous_model.set(Some(PreviousModelInfo {
                model_slug: "old-big-model".to_string(),
                context_window: 400_000,
            }));
            actor
                .maybe_compact_on_model_switch()
                .await
                .expect("non-auth model-switch compact failure must not abort the turn");
            assert_ne!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE,
                "schema/other compact failure must suppress after attempt"
            );
        })
        .await;
}
/// After clearing auth suppress, a shrink switch can re-evaluate and compact.
#[tokio::test(flavor = "current_thread")]
async fn clear_auth_suppress_allows_model_switch_compact_reeval() {
    use crate::session::compaction_config::{PreviousModelInfo, SUPPRESS_AUTH, SUPPRESS_NONE};
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(214_000, 200_000, 85, gateway_tx, persistence_tx).await);
            actor
                .suppress_auto_compaction(SuppressReason::Auth, 1_000, 200_000, None)
                .await;
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_AUTH
            );
            actor.compaction.previous_model.set(Some(PreviousModelInfo {
                model_slug: "old-big-model".to_string(),
                context_window: 400_000,
            }));
            actor
                .maybe_compact_on_model_switch()
                .await
                .expect("suppressed switch must not abort");
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_AUTH
            );
            actor.clear_auth_compact_suppression();
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE
            );
            actor.compaction.previous_model.set(Some(PreviousModelInfo {
                model_slug: "old-big-model".to_string(),
                context_window: 400_000,
            }));
            let base_url = spawn_deterministic_400_server().await;
            let mut cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            cfg.base_url = base_url;
            actor.chat_state_handle.update_sampling_config(cfg);
            actor.chat_state_handle.replace_conversation(vec![
                ConversationItem::system("sys"),
                ConversationItem::user("hello"),
            ]);
            actor.chat_state_handle.record_token_usage(214_000);
            actor
                .maybe_compact_on_model_switch()
                .await
                .expect("post-clear switch compact re-eval must not abort on non-auth");
            assert_ne!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE,
                "post-clear switch must re-evaluate and attempt compact"
            );
        })
        .await;
}
/// A deterministic failure suppresses auto-compaction only on the AUTO
/// path — never for a bare manual `/compact`.
#[tokio::test(flavor = "current_thread")]
async fn bare_manual_compact_failure_does_not_suppress_auto() {
    use crate::session::compaction_config::SUPPRESS_NONE;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(50_000, 200_000, 85, gateway_tx, persistence_tx).await);
            let base_url = spawn_deterministic_400_server().await;
            let mut cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            cfg.base_url = base_url;
            actor.chat_state_handle.update_sampling_config(cfg);
            actor.chat_state_handle.replace_conversation(vec![
                ConversationItem::system("sys"),
                ConversationItem::user("hello"),
            ]);
            let result = actor.run_compact(None).await;
            assert!(result.is_err(), "mock 400 must fail the compaction");
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE,
                "manual /compact (even without args) must never set auto-compact suppression"
            );
            let result = actor
                .run_compact_only(AutoCompactTriggerInfo {
                    tokens_used: 180_000,
                    context_window: 200_000,
                    percentage: 90,
                })
                .await;
            assert!(result.is_err(), "mock 400 must fail the compaction");
            assert_ne!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE,
                "the same deterministic failure on the AUTO path must suppress"
            );
        })
        .await;
}
/// A forked session whose whole-transcript inherited prefix alone exceeds
/// the auto-compact threshold releases the prefix on compaction (so the
/// conversation can actually shrink below the threshold) and keeps the
/// release sticky across further compactions (no unbounded compaction loop).
#[tokio::test(flavor = "current_thread")]
async fn forked_prefix_released_under_pressure_and_stays_released() {
    use crate::session::compaction_config::SUPPRESS_NONE;
    use std::sync::atomic::Ordering::Relaxed;
    use xai_grok_test_support::MockInferenceServer;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let filler = "x".repeat(8_000);
            let mut conv = vec![ConversationItem::system("small system prompt")];
            for i in 0..9 {
                conv.push(ConversationItem::user(format!("u{i} {filler}")));
                conv.push(ConversationItem::assistant(format!("a{i} {filler}")));
            }
            conv.push(ConversationItem::user("final query"));
            let prefix_len = conv.len();
            let mut actor = create_test_actor(0, 40_000, 80, gateway_tx, persistence_tx).await;
            actor.startup_hints.inherited_prefix_len = Some(prefix_len);
            let actor = Arc::new(actor);
            let server = MockInferenceServer::start().await.unwrap();
            server.set_response("Summary of prior work. ".repeat(30));
            let mut cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            cfg.base_url = server.url();
            actor.chat_state_handle.update_sampling_config(cfg);
            actor.chat_state_handle.replace_conversation(conv);
            let threshold_tokens = 40_000u64 * 80 / 100;
            let before = actor.chat_state_handle.get_total_tokens().await;
            assert!(
                before > threshold_tokens,
                "seed must exceed threshold: {before} <= {threshold_tokens}"
            );
            let result = actor.run_compact(None).await;
            assert!(result.is_ok(), "compaction should succeed: {result:?}");
            assert!(
                actor.compaction.prefix_released.load(Relaxed),
                "prefix must be released under pressure"
            );
            let after = actor.chat_state_handle.get_total_tokens().await;
            assert!(
                after < threshold_tokens,
                "released history must drop below threshold: {after} >= {threshold_tokens}"
            );
            assert!(
                actor.chat_state_handle.get_conversation_len().await < prefix_len,
                "conversation must shrink below the pinned prefix floor"
            );
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_NONE,
                "a shrunk conversation must not suppress AUTO"
            );
            let result = actor.run_compact(None).await;
            assert!(
                result.is_ok(),
                "second compaction should succeed: {result:?}"
            );
            assert!(
                actor.compaction.prefix_released.load(Relaxed),
                "release must stay sticky across compactions"
            );
            let after2 = actor.chat_state_handle.get_total_tokens().await;
            assert!(
                after2 < threshold_tokens,
                "sticky release must keep the session under threshold: {after2}"
            );
        })
        .await;
}
/// When even the released (summarized) history still exceeds the threshold
/// -- the pathological case where the system prompt alone is over budget --
/// a forked session sets sticky suppression (WITHOUT a user-facing failure
/// event) instead of clearing it, so AUTO is not immediately re-armed while the
/// compaction itself still reports success.
#[tokio::test(flavor = "current_thread")]
async fn forked_release_still_over_threshold_suppresses_auto() {
    use crate::session::compaction_config::SUPPRESS_STICKY;
    use std::sync::atomic::Ordering::Relaxed;
    use xai_grok_test_support::MockInferenceServer;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, mut persistence_rx) = mpsc::unbounded_channel();
            let huge_system = "s".repeat(150_000);
            let conv = vec![
                ConversationItem::system(huge_system),
                ConversationItem::user("q"),
                ConversationItem::assistant("a"),
                ConversationItem::user("final query"),
            ];
            let prefix_len = conv.len();
            let mut actor = create_test_actor(0, 40_000, 80, gateway_tx, persistence_tx).await;
            actor.startup_hints.inherited_prefix_len = Some(prefix_len);
            let actor = Arc::new(actor);
            let server = MockInferenceServer::start().await.unwrap();
            server.set_response("Summary. ".repeat(70));
            let mut cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            cfg.base_url = server.url();
            actor.chat_state_handle.update_sampling_config(cfg);
            actor.chat_state_handle.replace_conversation(conv);
            let threshold_tokens = 40_000u64 * 80 / 100;
            let before = actor.chat_state_handle.get_total_tokens().await;
            assert!(
                before > threshold_tokens,
                "seed must exceed threshold: {before}"
            );
            let result = actor.run_compact(None).await;
            assert!(result.is_ok(), "compaction should succeed: {result:?}");
            assert!(
                actor.compaction.prefix_released.load(Relaxed),
                "prefix must be released under pressure"
            );
            assert_eq!(
                actor.compaction.auto_compact_suppressed.load(Relaxed),
                SUPPRESS_STICKY,
                "an over-threshold released history must set sticky suppression"
            );
            let mut saw_failure = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(notif)) =
                    msg
                    && matches!(
                        &notif.update,
                        crate::extensions::notification::SessionUpdate::AutoCompactFailed { .. }
                    )
                {
                    saw_failure = true;
                }
            }
            assert!(
                !saw_failure,
                "successful compaction must not emit AutoCompactFailed"
            );
        })
        .await;
}
/// `classify_suppress_reason` maps each deterministic-failure shape to its
/// fixed [`SuppressReason`].
#[test]
fn classify_suppress_reason_maps_error_text() {
    let classify = SessionActor::classify_suppress_reason;
    assert_eq!(
        classify("caller does not have permission … spending-limit reached"),
        SuppressReason::CreditBlock
    );
    assert_eq!(
        classify("you have run out of credits"),
        SuppressReason::CreditBlock
    );
    assert_eq!(
        classify("API error (status 402 Payment Required): Grok Build usage balance exhausted"),
        SuppressReason::CreditBlock
    );
    assert_eq!(
        classify("API error (status 402 Payment Required): Payment Required"),
        SuppressReason::CreditBlock
    );
    assert_eq!(
        classify("Grok Build usage limit reached"),
        SuppressReason::CreditBlock
    );
    assert_eq!(
        classify("This model's maximum prompt length is 500000"),
        SuppressReason::Size
    );
    assert_eq!(
        classify("compact failed: The prompt is too long for this model's context window."),
        SuppressReason::Size
    );
    assert_eq!(
        classify("provider error: context_length_exceeded"),
        SuppressReason::Size
    );
    assert_eq!(
        classify("API error (status 401 Unauthorized)"),
        SuppressReason::Auth
    );
    assert_eq!(
        classify("provider returned invalid_request_error: messages.3"),
        SuppressReason::Schema
    );
    assert_eq!(
        classify("upstream 500 internal error"),
        SuppressReason::Other
    );
}
/// `SuppressReason::as_str` is the stable telemetry wire value — BQ/OTLP and
/// dashboards key off these exact strings. Lock them so a rename can't break monitoring.
#[test]
fn suppress_reason_as_str_is_stable() {
    assert_eq!(SuppressReason::CreditBlock.as_str(), "credit_block");
    assert_eq!(SuppressReason::Size.as_str(), "size");
    assert_eq!(SuppressReason::Auth.as_str(), "auth");
    assert_eq!(SuppressReason::Schema.as_str(), "schema");
    assert_eq!(SuppressReason::Other.as_str(), "other");
}
mod preserve_prefix {
    use super::super::preserve_inherited_prefix;
    use super::super::project_preserved_reseed_tokens;
    use xai_grok_sampling_types::conversation::ConversationItem;
    #[test]
    fn splices_inherited_with_compacted_suffix() {
        let conversation = vec![
            ConversationItem::system("sys"),
            ConversationItem::user("parent q1"),
            ConversationItem::assistant("parent a1"),
            ConversationItem::user("child q1"),
        ];
        let compacted = vec![
            ConversationItem::system("sys"),
            ConversationItem::user("summary"),
        ];
        let items = preserve_inherited_prefix(&conversation, compacted, 3).expect("Ok");
        assert_eq!(items.len(), 4);
        assert!(matches!(items[0], ConversationItem::System(_)));
    }
    /// Invariant: a head-only prefix lets compaction shrink the conversation;
    /// a whole-transcript prefix does not (that pinned floor is the loop).
    #[test]
    fn head_only_shrinks_full_transcript_does_not() {
        let mut conversation = vec![ConversationItem::system("sys")];
        for i in 0..8 {
            conversation.push(ConversationItem::user(format!("u{i}")));
            conversation.push(ConversationItem::assistant(format!("a{i}")));
        }
        let compacted = vec![
            ConversationItem::system("sys"),
            ConversationItem::assistant("summary"),
        ];
        let fixed = preserve_inherited_prefix(&conversation, compacted.clone(), 1).expect("Ok");
        assert!(fixed.len() < conversation.len(), "head-only shrinks");
        let buggy =
            preserve_inherited_prefix(&conversation, compacted, conversation.len()).expect("Ok");
        assert!(
            buggy.len() >= conversation.len(),
            "full prefix never shrinks"
        );
    }
    /// The reseed projection calibrates the bytes/4 estimate to real tokens
    /// (ratio != 1) and caps at the pre-compaction total, so the release
    /// decision reflects what the trigger applies next turn.
    #[test]
    fn project_preserved_reseed_tokens_calibrates_and_caps() {
        assert_eq!(
            project_preserved_reseed_tokens(30_000, 100_000, 50_000),
            60_000
        );
        assert_eq!(
            project_preserved_reseed_tokens(40_000, 70_000, 35_000),
            70_000
        );
        assert_eq!(
            project_preserved_reseed_tokens(20_000, 40_000, 40_000),
            20_000
        );
        assert_eq!(project_preserved_reseed_tokens(10, 5, 0), 5);
    }
    /// Both prefix and re-injected suffix may carry AGENTS.md; the splice must
    /// leave exactly one (else the model sees project instructions twice).
    #[test]
    fn does_not_duplicate_agents_md() {
        let conversation = vec![
            ConversationItem::system("sys"),
            ConversationItem::project_instructions("AGENTS.md"),
            ConversationItem::user("work"),
        ];
        let compacted = vec![
            ConversationItem::system("sys"),
            ConversationItem::project_instructions("AGENTS.md"),
            ConversationItem::user("summary"),
        ];
        let items = preserve_inherited_prefix(&conversation, compacted, 2).expect("Ok");
        let pi = items
            .iter()
            .filter(|i| super::super::is_project_instructions(i))
            .count();
        assert_eq!(pi, 1, "exactly one project-instructions item, not two");
    }
    #[test]
    fn keeps_reinjected_agents_md_when_prefix_lacks_it() {
        let conversation = vec![
            ConversationItem::system("sys"),
            ConversationItem::user("work"),
        ];
        let compacted = vec![
            ConversationItem::system("sys"),
            ConversationItem::project_instructions("AGENTS.md"),
            ConversationItem::user("summary"),
        ];
        let items = preserve_inherited_prefix(&conversation, compacted, 1).expect("Ok");
        let pi = items
            .iter()
            .filter(|i| super::super::is_project_instructions(i))
            .count();
        assert_eq!(
            pi, 1,
            "re-injected AGENTS.md preserved when prefix lacks one"
        );
    }
}
#[allow(clippy::field_reassign_with_default)]
async fn create_test_actor_with_memory(
    total_tokens: u64,
    context_window: u64,
    threshold_percent: u8,
    gateway_tx: mpsc::UnboundedSender<xai_acp_lib::AcpClientMessage>,
    persistence_tx: mpsc::UnboundedSender<PersistenceMsg>,
    memory_config: Option<crate::config::MemoryConfig>,
) -> SessionActor {
    let tmp = tempfile::TempDir::new().unwrap();
    let cwd_path = tmp.path().to_path_buf();
    let memory_storage = memory_config
        .as_ref()
        .filter(|mc| mc.enabled)
        .map(|_| crate::session::memory::MemoryStorage::new(&cwd_path, None));
    std::mem::forget(tmp);
    let memory_initial_injection_config = memory_config
        .as_ref()
        .map_or_else(Default::default, |mc| mc.initial_injection.clone());
    let mut actor = create_test_actor(
        total_tokens,
        context_window,
        threshold_percent,
        gateway_tx,
        persistence_tx,
    )
    .await;
    actor.memory = crate::session::memory_state::SessionMemory {
        flush_config: memory_config
            .as_ref()
            .map_or_else(Default::default, |mc| mc.flush.clone()),
        is_flushing: std::sync::atomic::AtomicBool::new(false),
        last_flush_compaction: std::sync::atomic::AtomicU64::new(0),
        storage: std::cell::RefCell::new(memory_storage),
        save_on_end: true,
        backend_params: None,
        initial_injection_config: memory_initial_injection_config,
        context_injected: std::sync::atomic::AtomicBool::new(false),
        flush_count: std::sync::atomic::AtomicU64::new(0),
        last_flush_content: std::cell::RefCell::new(None),
        flush_success_count: std::sync::atomic::AtomicU64::new(0),
        flush_error_count: std::sync::atomic::AtomicU64::new(0),
        search_counter: std::cell::RefCell::new(None),
        injection_count: std::sync::atomic::AtomicU64::new(0),
        compaction_recovery_count: std::sync::atomic::AtomicU64::new(0),
        chunks_added: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        dream_config: Default::default(),
        dream_count: std::sync::atomic::AtomicU64::new(0),
        dream_success_count: std::sync::atomic::AtomicU64::new(0),
        dream_error_count: std::sync::atomic::AtomicU64::new(0),
    };
    actor.idle_flush_timeout = memory_config
        .as_ref()
        .and_then(|mc| mc.flush.idle_timeout_secs)
        .map(std::time::Duration::from_secs);
    actor.dream_check_timeout = memory_config
        .as_ref()
        .filter(|mc| mc.dream.enabled)
        .and_then(|mc| mc.dream.check_interval_secs)
        .filter(|&s| s > 0)
        .map(std::time::Duration::from_secs);
    actor
}
/// Verify that `last_idle_flush_conversation_len` is reset after
/// compaction shrinks the conversation. Without this reset the
/// interval flush guard (`current_len > last_len`) stays false
/// because the compacted conversation is shorter than the stored
/// pre-compaction length.
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::field_reassign_with_default)]
async fn test_idle_flush_conversation_len_reset_after_compaction() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel();
            let (persistence_tx, _) = mpsc::unbounded_channel();
            let mut config = crate::config::MemoryConfig::default();
            config.enabled = true;
            config.flush.idle_timeout_secs = Some(60);
            let actor = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx,
                persistence_tx,
                Some(config),
            )
            .await;
            for _ in 0..80 {
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("hello".to_string()));
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            actor
                .last_idle_flush_conversation_len
                .store(80, std::sync::atomic::Ordering::Relaxed);
            {
                let current_len = actor.chat_state_handle.get_conversation_len().await;
                let last_len = actor
                    .last_idle_flush_conversation_len
                    .load(std::sync::atomic::Ordering::Relaxed);
                assert_eq!(current_len, 80);
                assert!(
                    current_len <= last_len,
                    "guard should block: no new messages"
                );
            }
            {
                let compacted = vec![ConversationItem::user("compacted summary".to_string())];
                let new_len = compacted.len();
                actor
                    .chat_state_handle
                    .replace_conversation_for_compaction(compacted);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                actor
                    .last_idle_flush_conversation_len
                    .store(new_len, std::sync::atomic::Ordering::Relaxed);
            }
            {
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("new message".to_string()));
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                let current_len = actor.chat_state_handle.get_conversation_len().await;
                let last_len = actor
                    .last_idle_flush_conversation_len
                    .load(std::sync::atomic::Ordering::Relaxed);
                assert_eq!(current_len, 2, "summary + new message");
                assert_eq!(last_len, 1, "reset to post-compaction length");
                assert!(
                    current_len > last_len,
                    "guard should allow flush after compaction + new message"
                );
            }
        })
        .await;
}
fn api_error_with_context_window(context_window: u64) -> xai_grok_sampler::SamplingErrorInfo {
    xai_grok_sampler::SamplingErrorInfo {
        kind: xai_grok_sampler::SamplingErrorKind::Api,
        status_code: Some(400),
        message: "prompt is too long".to_string(),
        is_retryable: false,
        retry_after_secs: None,
        should_retry: None,
        error_code: None,
        model_metadata: Some(crate::sampling::ResponseModelMetadata {
            context_window: Some(context_window),
            max_completion_tokens: None,
            models_etag: None,
        }),
        empty_response_context: None,
        doom_loop_triggers: None,
        doom_loop_aborted_at_chunk: None,
        credential: xai_grok_sampling_types::SentCredential::Unknown,
    }
}
/// Primary scenario: remote settings shrinks the context window mid-session.
/// The shell's last-known token count (214K) exceeds the new limit (200K) —
/// should_compact_on_error must return true so the session can recover.
#[tokio::test(flavor = "current_thread")]
async fn test_compact_on_error_triggers_when_tokens_exceed_new_window() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(214_000, 1_000_000, 85, gateway_tx, persistence_tx).await;
            let err = api_error_with_context_window(200_000);
            assert!(actor.should_compact_on_error(&err).await);
        })
        .await;
}
/// When tracked tokens are within the new limit, the error was not a context
/// overflow — do not compact.
#[tokio::test(flavor = "current_thread")]
async fn test_compact_on_error_no_trigger_when_tokens_within_new_window() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(150_000, 1_000_000, 85, gateway_tx, persistence_tx).await;
            let err = api_error_with_context_window(200_000);
            assert!(!actor.should_compact_on_error(&err).await);
        })
        .await;
}
/// Used tokens over the session sampling window compact even when error
/// metadata is missing.
#[tokio::test(flavor = "current_thread")]
async fn test_compact_on_error_uses_session_window_without_model_metadata() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(500_000, 200_000, 85, gateway_tx, persistence_tx).await;
            let err = xai_grok_sampler::SamplingErrorInfo {
                kind: xai_grok_sampler::SamplingErrorKind::Api,
                status_code: Some(400),
                message: "prompt is too long".to_string(),
                is_retryable: false,
                retry_after_secs: None,
                should_retry: None,
                error_code: None,
                model_metadata: None,
                empty_response_context: None,
                doom_loop_triggers: None,
                doom_loop_aborted_at_chunk: None,
                credential: xai_grok_sampling_types::SentCredential::Unknown,
            };
            assert!(
                actor.should_compact_on_error(&err).await,
                "used 500k over the 200k session sampling window must compact even without error metadata"
            );
        })
        .await;
}

/// Catalog metadata 500k must not hide an already-over 200k session sampling
/// window. Compact once (L1/L2) instead of retrying the same payload.
#[tokio::test(flavor = "current_thread")]
async fn over_window_model_failure_compacts_against_session_sampling_window() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(411_000, 200_000, 95, gateway_tx, persistence_tx).await;
            let catalog = api_error_with_context_window(500_000);
            assert!(
                actor.should_compact_on_error(&catalog).await,
                "411k used over 200k sampling must compact even if catalog metadata is 500k"
            );
            let timeout = xai_grok_sampler::SamplingErrorInfo {
                kind: xai_grok_sampler::SamplingErrorKind::Http,
                status_code: None,
                message: "timed out waiting for response headers".to_string(),
                is_retryable: true,
                retry_after_secs: None,
                should_retry: None,
                error_code: None,
                model_metadata: Some(crate::sampling::ResponseModelMetadata {
                    context_window: Some(500_000),
                    max_completion_tokens: None,
                    models_etag: None,
                }),
                empty_response_context: None,
                doom_loop_triggers: None,
                doom_loop_aborted_at_chunk: None,
                credential: xai_grok_sampling_types::SentCredential::Unknown,
            };
            assert!(
                actor.should_compact_on_error(&timeout).await,
                "over-window timeout must compact once, not increment Retrying attempts"
            );
        })
        .await;
}

/// Http/timeout errors carry `model_metadata: None`. Compact recovery must
/// use the session sampling window, not panic on missing catalog metadata.
#[tokio::test(flavor = "current_thread")]
async fn over_window_timeout_without_catalog_metadata_does_not_panic() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(411_000, 200_000, 95, gateway_tx, persistence_tx).await;
            let actor = Arc::new(actor);
            let err = xai_grok_sampler::SamplingErrorInfo {
                kind: xai_grok_sampler::SamplingErrorKind::Http,
                status_code: None,
                message: "timed out waiting for response headers".into(),
                is_retryable: true,
                retry_after_secs: None,
                should_retry: None,
                error_code: None,
                model_metadata: None,
                empty_response_context: None,
                doom_loop_triggers: None,
                doom_loop_aborted_at_chunk: None,
                credential: xai_grok_sampling_types::SentCredential::Unknown,
            };
            assert!(actor.should_compact_on_error(&err).await);
            let result = actor.handle_sampling_failure(err).await;
            match result {
                Ok(crate::session::acp_session::SamplerFailureRecovery::CompactAndResubmit)
                | Err(_) => {}
                Ok(crate::session::acp_session::SamplerFailureRecovery::EndChildWithoutCompact)
                | Ok(
                    crate::session::acp_session::SamplerFailureRecovery::RefreshAuthAndResubmit {
                        ..
                    },
                ) => panic!(
                    "over-window timeout with no catalog metadata must compact once \
                     or fail honestly, not panic"
                ),
            }
        })
        .await;
}

/// Catalog 500k on a 5xx must not replace this session's 200k sampling window.
#[tokio::test(flavor = "current_thread")]
async fn over_window_5xx_catalog_metadata_does_not_widen_session_sampling_window() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(411_000, 200_000, 95, gateway_tx, persistence_tx).await;
            let actor = Arc::new(actor);
            let err = xai_grok_sampler::SamplingErrorInfo {
                kind: xai_grok_sampler::SamplingErrorKind::Api,
                status_code: Some(500),
                message: "internal server error".into(),
                is_retryable: true,
                retry_after_secs: None,
                should_retry: None,
                error_code: None,
                model_metadata: Some(crate::sampling::ResponseModelMetadata {
                    context_window: Some(500_000),
                    max_completion_tokens: None,
                    models_etag: None,
                }),
                empty_response_context: None,
                doom_loop_triggers: None,
                doom_loop_aborted_at_chunk: None,
                credential: xai_grok_sampling_types::SentCredential::Unknown,
            };
            let _ = actor.handle_sampling_failure(err).await;
            let cw = actor
                .chat_state_handle
                .get_sampling_config()
                .await
                .unwrap()
                .context_window
                .get();
            assert_eq!(
                cw, 200_000,
                "catalog 500k on a 5xx must not replace this session's 200k sampling window"
            );
        })
        .await;
}

/// Sticky tiny-savings suppress: do not CompactAndResubmit again. Fail
/// honestly instead of spinning Retrying the model request.
#[tokio::test(flavor = "current_thread")]
async fn over_window_sticky_suppress_does_not_rearm_overflow_compact() {
    use crate::session::compaction_config::SUPPRESS_STICKY;
    use std::sync::atomic::Ordering::Relaxed;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(411_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor
                .compaction
                .auto_compact_suppressed
                .store(SUPPRESS_STICKY, Relaxed);
            let overflow = api_error_with_context_window(200_000);
            assert!(
                !actor.should_compact_on_error(&overflow).await,
                "sticky suppress must not compact-and-resubmit forever"
            );
            assert!(
                actor.sampling_window_is_full().await,
                "411k of 200k must still be over the sampling window so the turn can fail honestly"
            );
        })
        .await;
}

/// Pre-sampling check uses estimated tokens (includes tool-result delta).
#[tokio::test(flavor = "current_thread")]
async fn test_pre_sampling_uses_estimated_tokens() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(80_000, 100_000, 85, gateway_tx, persistence_tx).await;
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_none(), "80% should not trigger at 85% threshold");
            actor.chat_state_handle.record_token_usage(90_000);
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_some(), "90% should trigger");
            assert_eq!(result.unwrap().percentage, 90);
        })
        .await;
}
/// Model-switch compaction fires when switching to a smaller context window.
#[tokio::test(flavor = "current_thread")]
async fn test_model_switch_compaction_triggers_on_downgrade() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(86_000, 100_000, 85, gateway_tx, persistence_tx).await;
            actor.compaction.previous_model.set(Some(
                crate::session::compaction_config::PreviousModelInfo {
                    model_slug: "large-model".to_string(),
                    context_window: 200_000,
                },
            ));
            let prev = actor.compaction.previous_model.take();
            assert!(prev.is_some());
            let prev = prev.unwrap();
            assert_eq!(prev.context_window, 200_000);
            let cfg = actor.chat_state_handle.get_sampling_config().await.unwrap();
            assert!(prev.context_window > cfg.context_window.get());
            let total = actor.chat_state_handle.get_estimated_total_tokens().await;
            let trigger = actor.should_auto_compact(total, cfg.context_window);
            assert!(trigger.is_some(), "86% > 85% threshold, should trigger");
            actor.compaction.previous_model.set(Some(
                crate::session::compaction_config::PreviousModelInfo {
                    model_slug: "small-model".to_string(),
                    context_window: 50_000,
                },
            ));
            let prev = actor.compaction.previous_model.take().unwrap();
            assert!(prev.context_window <= cfg.context_window.get());
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
async fn get_transcript_path_returns_some_when_file_exists() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) =
                mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor =
                create_test_actor(50_000, 200_000, 85, gateway_tx, persistence_tx).await;
            actor.compaction.compaction_mode = xai_chat_state::CompactionMode::Transcript;
            let session_dir = crate::session::persistence::session_dir(&actor.session_info);
            std::fs::create_dir_all(&session_dir).unwrap();
            let updates_path = session_dir.join("updates.jsonl");
            std::fs::write(&updates_path, "{}\n").unwrap();
            let result = actor.get_transcript_path();
            assert!(result.is_some(), "file exists → Some");
            assert!(
                result.as_ref().unwrap().ends_with("updates.jsonl"),
                "path should end with updates.jsonl, got: {:?}",
                result,
            );
            let hint = actor.transcript_hint().expect("transcript hint present");
            assert!(hint.contains("read the full transcript"));
            assert!(hint.ends_with("updates.jsonl"));
            actor.compaction.compaction_mode = xai_chat_state::CompactionMode::Summary;
            assert!(actor.transcript_hint().is_none());
            let _ = std::fs::remove_file(&updates_path);
            let _ = std::fs::remove_dir_all(&session_dir);
        })
        .await;
}

/// L3 specialists never AUTO compact. 95% of the nested 200k window must
/// not start compact-and-continue.
#[tokio::test(flavor = "current_thread")]
async fn l3_auto_compact_does_not_fire_when_nested_window_is_full() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let mut actor =
                create_test_actor(190_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor.startup_hints.is_subagent = true;
            actor.tool_context.subagent_depth = 2;
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "L3 at 95% of the nested 200k window must not AUTO compact"
            );
            assert!(
                !actor.should_prefire_two_pass().await,
                "L3 must not start two-pass prefire compact"
            );
        })
        .await;
}

/// L3 overflow must not compact or CompactAndResubmit. L2 still uses last
/// assistant text / the on-disk report when the child ends.
#[tokio::test(flavor = "current_thread")]
async fn l3_preflight_overflow_does_not_compact_and_resubmit() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let mut actor =
                create_test_actor(214_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor.startup_hints.is_subagent = true;
            actor.tool_context.subagent_depth = 2;
            let overflow = api_error_with_context_window(200_000);
            assert!(
                actor.check_preflight_overflow().await.is_none(),
                "L3 must not start preflight overflow compact"
            );
            assert!(
                !actor.should_compact_on_error(&overflow).await,
                "L3 overflow must not CompactAndResubmit"
            );
        })
        .await;
}

/// Goal Plan Writer is a disposable once-run nested role. 95% of the nested
/// 200k window must not start compact-and-continue. Same class as L3:
/// summarize and stop (`EndChildWithoutCompact`).
///
/// Owed outcome: a once-run Goal Plan Writer at L2 depth (fork from L1)
/// must not AUTO compact when the nested 200k sampling window is full.
#[tokio::test(flavor = "current_thread")]
async fn goal_plan_writer_once_run_must_not_auto_compact_when_nested_window_is_full() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let mut actor =
                create_test_actor(190_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor.startup_hints.is_subagent = true;
            actor.startup_hints.once_run = true;
            actor.startup_hints.subagent_type = Some("general-purpose".into());
            actor.tool_context.subagent_depth = 1;
            assert!(
                !actor.is_l3_session(),
                "Goal Plan Writer from L1 is L2 depth; the once-run gate must not pretend it is L3"
            );
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "once-run Goal Plan Writer at 95% of the nested 200k window must not AUTO compact"
            );
            assert!(
                !actor.should_prefire_two_pass().await,
                "once-run Goal Plan Writer must not start two-pass prefire compact"
            );
            let overflow = api_error_with_context_window(200_000);
            assert!(
                !actor.should_compact_on_error(&overflow).await,
                "once-run Goal Plan Writer overflow must not CompactAndResubmit"
            );
        })
        .await;
}

/// A forked parent that already fills the nested 200k window must not
/// immediately compact a Goal Plan Writer child on the first turn. The
/// child must summarize and stop instead of compact-and-continue.
///
/// Owed outcome: nested 200k window full at spawn (forked parent) must not
/// immediately compact a goal-role child.
#[tokio::test(flavor = "current_thread")]
async fn goal_plan_writer_forked_parent_full_window_at_spawn_must_not_immediately_compact() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let mut actor =
                create_test_actor(200_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor.startup_hints.is_subagent = true;
            actor.startup_hints.once_run = true;
            actor.startup_hints.subagent_type = Some("general-purpose".into());
            actor.tool_context.subagent_depth = 1;
            assert!(
                actor.check_auto_compact_needed().await.is_none(),
                "forked Goal Plan Writer at 100% of the nested 200k window must not AUTO compact"
            );
            assert!(
                actor.check_preflight_overflow().await.is_none(),
                "forked Goal Plan Writer at a full nested window must not preflight compact"
            );
            assert!(
                actor.l3_nested_window_is_full().await,
                "full nested window on a once-run goal role must end the child without compact"
            );
        })
        .await;
}

/// L2 nested sessions still AUTO compact at 95% of the 200k window.
#[tokio::test(flavor = "current_thread")]
async fn l2_auto_compact_still_fires_at_95_percent_of_200k() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
            let mut actor =
                create_test_actor(190_000, 200_000, 95, gateway_tx, persistence_tx).await;
            actor.startup_hints.is_subagent = true;
            actor.tool_context.subagent_depth = 1;
            let trigger = actor.check_auto_compact_needed().await;
            assert!(
                trigger.is_some(),
                "L2 at 95% of the nested 200k window must still AUTO compact"
            );
            let info = trigger.expect("L2 AUTO trigger");
            assert_eq!(info.context_window, 200_000);
            assert_eq!(info.tokens_used, 190_000);
        })
        .await;
}
