<<<<<<< HEAD
use std::collections::BTreeSet;
use std::fmt;
=======
//! Mock inference server with request logging and automatic cleanup.
//!
//! Serves `/v1/chat/completions`, `/v1/responses`, and `/v1/messages` in one
//! of two response modes: echo (default — streams `Echo: <last user message>`)
//! or a fixed text set via [`MockInferenceServer::set_response`] (streamed
//! with byte-exact reconstruction). Named request-matched expectations take
//! precedence, followed by compatibility per-path [`ScriptedResponse`] FIFOs.
//! `/v1/models` and `/v1/settings` return
//! configurable responses (settings is 404 until set). All requests are
//! logged — bodies and headers — for assertion in tests.

use std::collections::VecDeque;
use std::convert::Infallible;
>>>>>>> e3fdf3ed (Merge 2 (#4))
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::Context as _;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

<<<<<<< HEAD
use crate::conversation::{ConversationId, ReadConversation};
use crate::conversation_script::{Conversation, ScriptViolation};
use crate::feedback_endpoint::FeedbackEndpointState;
pub use crate::feedback_endpoint::FeedbackPost;
pub use crate::gated_upload_proxy::GatedUploadProxy;
use crate::inference_override::InferenceOverrides;
pub use crate::inference_override::{InferenceExpectation, InferenceRequestMatcher};
use crate::inference_request::DEFAULT_MODEL;
pub use crate::inference_request::InferenceEndpoint;
use crate::inference_route::InferenceRoute;
pub use crate::managed_gateway_endpoint::ManagedGatewayCall;
use crate::managed_gateway_endpoint::ManagedGatewayEndpointState;
use crate::mock_server_tls::ThrowawayCa;
pub use crate::request_log::LogEntry;
use crate::request_log::RequestLog;
=======
use crate::inference_override::{ClassifiedInferenceRequest, InferenceOverrides};
pub use crate::inference_override::{
    InferenceEndpoint, InferenceExpectation, InferenceRequestMatcher,
};
use crate::scripted::TerminalWait;
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub use crate::scripted::{ScriptedBody, ScriptedResponse, SseEvent};
use crate::storage_endpoint::StorageEndpointState;
pub use crate::storage_endpoint::StorageUpload;
use crate::telemetry_events::TelemetryEventsState;

/// Records that arrived since the previous [`MockInferenceServer::arrived_since_observation`].
pub struct ObservationRecords {
    pub requests: Vec<LogEntry>,
    pub telemetry: Vec<Value>,
    pub storage_uploads: Vec<StorageUpload>,
    pub gateway_calls: Vec<ManagedGatewayCall>,
}

<<<<<<< HEAD
/// A model served by `/v1/models`.
/// Each field is emitted under its camelCase name when set, at the top level except for `agent_type`, which goes in `_meta`.
=======
impl LogEntry {
    /// First value of `name` (case-insensitive), if the request carried it.
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

pub struct RequestLog {
    count: AtomicU32,
    entries: std::sync::Mutex<Vec<LogEntry>>,
}

impl RequestLog {
    fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
            entries: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn record(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        authorization: Option<&str>,
        headers: Vec<(String, String)>,
    ) {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.entries.lock().unwrap().push(LogEntry {
            method: method.to_string(),
            path: path.to_string(),
            body: body.cloned(),
            authorization: authorization.map(String::from),
            headers,
        });
    }
}

/// A model entry for the mock `/v1/models` endpoint.
>>>>>>> e3fdf3ed (Merge 2 (#4))
#[derive(Debug, Clone)]
pub struct MockModelEntry {
    id: String,
    agent_type: Option<String>,
    api_backend: Option<String>,
    supports_backend_search: bool,
    supports_reasoning_effort: bool,
    reasoning_effort: Option<String>,
    /// Each entry is a table carrying a `value` key, or a bare value string.
    /// `parse_remote_model_value` defines the full shape.
    reasoning_efforts: Vec<Value>,
    /// Sets `You are <label>` in the primary system prompt, so a model switch is visible on the wire.
    system_prompt_label: Option<String>,
}

impl MockModelEntry {
    pub fn new(id: impl Into<String>) -> Self {
        MockModelEntry {
            id: id.into(),
            agent_type: None,
            api_backend: None,
            supports_backend_search: false,
            supports_reasoning_effort: false,
            reasoning_effort: None,
            reasoning_efforts: Vec::new(),
            system_prompt_label: None,
        }
    }

    pub fn with_agent_type(id: impl Into<String>, agent_type: impl Into<String>) -> Self {
        MockModelEntry {
            agent_type: Some(agent_type.into()),
            ..MockModelEntry::new(id)
        }
    }

    pub fn with_api_backend(mut self, api_backend: impl Into<String>) -> Self {
        self.api_backend = Some(api_backend.into());
        self
    }

    pub fn with_supports_backend_search(mut self, supports: bool) -> Self {
        self.supports_backend_search = supports;
        self
    }

    pub fn with_supports_reasoning_effort(mut self, supports: bool) -> Self {
        self.supports_reasoning_effort = supports;
        self
    }

    pub fn with_reasoning_effort(mut self, effort: impl Into<String>) -> Self {
        self.reasoning_effort = Some(effort.into());
        self
    }

    pub fn with_reasoning_efforts(mut self, efforts: Vec<Value>) -> Self {
        self.reasoning_efforts = efforts;
        self
    }

    pub fn with_system_prompt_label(mut self, label: impl Into<String>) -> Self {
        self.system_prompt_label = Some(label.into());
        self
    }

    fn to_json(&self) -> Value {
        let mut obj = json!({
            "id": self.id,
            "object": "model",
            "created": 1234567890,
            "owned_by": "test",
            "context_length": 131_072
        });
        if let Some(map) = obj.as_object_mut() {
            if let Some(agent_type) = &self.agent_type {
                map.insert("_meta".into(), json!({ "agentType": agent_type }));
            }
            if let Some(backend) = &self.api_backend {
                map.insert("apiBackend".into(), json!(backend));
            }
            if self.supports_backend_search {
                map.insert("supportsBackendSearch".into(), json!(true));
            }
            if self.supports_reasoning_effort {
                map.insert("supportsReasoningEffort".into(), json!(true));
            }
            if let Some(effort) = &self.reasoning_effort {
                map.insert("reasoningEffort".into(), json!(effort));
            }
            if !self.reasoning_efforts.is_empty() {
                map.insert("reasoningEfforts".into(), json!(self.reasoning_efforts));
            }
            if let Some(label) = &self.system_prompt_label {
                map.insert("systemPromptLabel".into(), json!(label));
            }
        }
        obj
    }
}

#[derive(Clone, Copy, Default)]
enum StartupFetchStall {
    #[default]
    None,
    Delay(Duration),
    Hang,
}

<<<<<<< HEAD
/// `Boot` is the placeholder `start` seeds. `Installed` is a catalog a test has written.
#[derive(Clone)]
enum ModelCatalog {
    Boot(Vec<Value>),
    Installed(Vec<Value>),
}

#[derive(Clone)]
struct RouterState {
    log: Arc<RequestLog>,
    models: Arc<std::sync::RwLock<ModelCatalog>>,
    settings: Arc<std::sync::RwLock<Option<Value>>>,
    overrides: InferenceOverrides,
    inference: InferenceRoute,
    storage: Arc<StorageEndpointState>,
    feedback: Arc<FeedbackEndpointState>,
    telemetry: Arc<TelemetryEventsState>,
    managed_gateway: Arc<ManagedGatewayEndpointState>,
    startup_fetch_stall: Arc<std::sync::RwLock<StartupFetchStall>>,
    startup_stalls_served: Arc<AtomicU32>,
    user_tier: Arc<std::sync::RwLock<Option<String>>>,
    user_team: Arc<std::sync::RwLock<Option<MockUserTeam>>>,
    user_can_administer_team: Arc<std::sync::RwLock<MockCanAdministerTeam>>,
    user_coding_data_retention_opt_out: Arc<std::sync::RwLock<Option<bool>>>,
    user_info_released: Arc<tokio::sync::watch::Sender<bool>>,
    user_info_arrivals: Arc<tokio::sync::watch::Sender<usize>>,
}

/// `teamId`, `teamName`, and `teamRole` on `GET /v1/user`.
=======
/// Emit each SSE event after `delay` and optionally wait before the final one.
fn paced_events(
    events: Vec<axum::response::sse::Event>,
    delay: Option<Duration>,
    before_terminal: Option<TerminalWait>,
) -> impl futures_util::Stream<Item = Result<axum::response::sse::Event, Infallible>> {
    let last_idx = events.len().checked_sub(1);
    stream::unfold(
        (events.into_iter().enumerate(), before_terminal),
        move |(mut events, mut before_terminal)| async move {
            let (idx, event) = events.next()?;
            if let Some(d) = delay {
                tokio::time::sleep(d).await;
            }
            if Some(idx) == last_idx
                && let Some(wait) = before_terminal.take()
            {
                wait().await;
            }
            Some((Ok::<_, Infallible>(event), (events, before_terminal)))
        },
    )
}

/// Max body bytes retained on each accepted [`StorageUpload`] (keeps large
/// e2e artifacts from ballooning test memory; meta/small dumps stay intact).
const STORAGE_BODY_CAPTURE_CAP: usize = 256 * 1024;

/// One accepted (HTTP 200) mock `/v1/storage` upload.
>>>>>>> e3fdf3ed (Merge 2 (#4))
#[derive(Debug, Clone)]
pub struct MockUserTeam {
    pub id: String,
    pub name: String,
    pub role: String,
}

/// `canAdministerTeam` on `GET /v1/user`. `Omitted` leaves the key out; `Unresolved` is `null`,
/// the proxy's answer when it could not resolve the caller's team-administration capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockCanAdministerTeam {
    Omitted,
    Unresolved,
    Allowed,
    Denied,
}

impl MockCanAdministerTeam {
    pub fn wire_value(self) -> Option<Value> {
        match self {
            Self::Omitted => None,
            Self::Unresolved => Some(Value::Null),
            Self::Allowed => Some(Value::Bool(true)),
            Self::Denied => Some(Value::Bool(false)),
        }
    }
}

impl RouterState {
    /// Counted once it is let through.
    async fn stall_startup_fetch(&self) {
        let stall = *self.startup_fetch_stall.read().unwrap();
        match stall {
            StartupFetchStall::None => return,
            StartupFetchStall::Delay(delay) => tokio::time::sleep(delay).await,
            StartupFetchStall::Hang => std::future::pending().await,
        }
        self.startup_stalls_served.fetch_add(1, Ordering::Relaxed);
    }
}

enum Transport {
    Plain,
    Tls,
}

pub struct MockInferenceServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
<<<<<<< HEAD
    state: RouterState,
    tls_ca: Option<ThrowawayCa>,
=======
    log: Arc<RequestLog>,
    models: Arc<std::sync::RwLock<Vec<Value>>>,
    settings: Arc<std::sync::RwLock<Option<Value>>>,
    response_mode: Arc<std::sync::RwLock<ResponseMode>>,
    overrides: InferenceOverrides,
    /// Per-agent-turn assistant texts (see [`set_agent_turns`]).
    ///
    /// [`set_agent_turns`]: Self::set_agent_turns
    agent_turns: Arc<std::sync::Mutex<VecDeque<String>>>,
    /// `stop_reason` emitted by the `/v1/messages` terminal `message_delta`.
    messages_stop_reason: Arc<std::sync::RwLock<String>>,
    /// Optional per-SSE-event delay on all inference endpoints.
    chunk_delay: Arc<std::sync::RwLock<Option<Duration>>>,
    /// Mock `/v1/storage` 401 gate + accepted-upload record.
    storage: Arc<StorageState>,
    /// See [`Self::set_user_subscription_tier`].
    user_tier: Arc<std::sync::RwLock<Option<String>>>,
>>>>>>> e3fdf3ed (Merge 2 (#4))
}

impl MockInferenceServer {
    pub async fn start() -> anyhow::Result<Self> {
        Self::start_with_models(vec![MockModelEntry::new(DEFAULT_MODEL)]).await
    }

    pub async fn start_with_models(models: Vec<MockModelEntry>) -> anyhow::Result<Self> {
        Self::start_inner(models, None, Transport::Plain).await
    }

    /// Start a mock that returns 401 on inference requests missing `Authorization: Bearer <required_token>`.
    pub async fn start_with_required_auth(
        models: Vec<MockModelEntry>,
        required_token: impl Into<String>,
    ) -> anyhow::Result<Self> {
        Self::start_inner(models, Some(required_token.into()), Transport::Plain).await
    }

    /// Serve the same router over HTTPS with a throwaway CA; there is no plaintext listener,
    /// so a logged request implies a completed TLS handshake.
    /// [`Self::url`] is `https://127.0.0.1:PORT/v1` and [`Self::ca_pem_path`] is the path to the
    /// CA PEM the client must trust. Only [`crate::headless::run_headless`] and
    /// [`crate::headless::run_headless_with_env`] inject it; any other runner must set
    /// `GROK_EXTRA_CA_BUNDLE` to [`Self::ca_pem_path`] on its own `TestSandbox` via `set_env`.
    pub async fn start_tls() -> anyhow::Result<Self> {
        Self::start_inner(
            vec![MockModelEntry::new(DEFAULT_MODEL)],
            None,
            Transport::Tls,
        )
        .await
    }

    async fn start_inner(
        models: Vec<MockModelEntry>,
        required_token: Option<String>,
        transport: Transport,
    ) -> anyhow::Result<Self> {
        let log = Arc::new(RequestLog::new());
<<<<<<< HEAD
        let overrides = InferenceOverrides::new(required_token);
        let state = RouterState {
            inference: InferenceRoute::new(log.clone(), overrides.clone()),
            log,
            models: Arc::new(std::sync::RwLock::new(ModelCatalog::Boot(
                models.iter().map(MockModelEntry::to_json).collect(),
            ))),
            settings: Arc::new(std::sync::RwLock::new(None)),
            overrides,
            storage: Arc::new(StorageEndpointState::default()),
            feedback: Arc::new(FeedbackEndpointState::default()),
            telemetry: Arc::new(TelemetryEventsState::default()),
            managed_gateway: Arc::new(ManagedGatewayEndpointState::default()),
            startup_fetch_stall: Arc::new(std::sync::RwLock::new(StartupFetchStall::None)),
            startup_stalls_served: Arc::new(AtomicU32::new(0)),
            user_tier: Arc::new(std::sync::RwLock::new(None)),
            user_team: Arc::new(std::sync::RwLock::new(None)),
            user_can_administer_team: Arc::new(std::sync::RwLock::new(
                MockCanAdministerTeam::Omitted,
            )),
            user_coding_data_retention_opt_out: Arc::new(std::sync::RwLock::new(None)),
            user_info_released: Arc::new(tokio::sync::watch::Sender::new(true)),
            user_info_arrivals: Arc::new(tokio::sync::watch::Sender::new(0)),
        };
        let app = Self::build_router(state.clone());
=======
        let models_json: Vec<Value> = models.iter().map(MockModelEntry::to_json).collect();
        let shared_models = Arc::new(std::sync::RwLock::new(models_json));
        let shared_settings = Arc::new(std::sync::RwLock::new(None::<Value>));
        let response_mode = Arc::new(std::sync::RwLock::new(ResponseMode::Echo));
        let overrides = InferenceOverrides::new(required_token);
        let agent_turns = Arc::new(std::sync::Mutex::new(VecDeque::new()));
        let messages_stop_reason = Arc::new(std::sync::RwLock::new("end_turn".to_string()));
        let chunk_delay = Arc::new(std::sync::RwLock::new(None::<Duration>));
        let storage = Arc::new(StorageState::default());
        let user_tier = Arc::new(std::sync::RwLock::new(None::<String>));
        let app = Self::build_router(
            log.clone(),
            shared_models.clone(),
            shared_settings.clone(),
            response_mode.clone(),
            overrides.clone(),
            agent_turns.clone(),
            messages_stop_reason.clone(),
            chunk_delay.clone(),
            storage.clone(),
            user_tier.clone(),
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("bind mock inference server")?;
        let addr = listener.local_addr().context("local_addr")?;
>>>>>>> e3fdf3ed (Merge 2 (#4))
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let (addr, tls_ca) = match transport {
            Transport::Plain => {
                let listener = TcpListener::bind("127.0.0.1:0")
                    .await
                    .context("bind mock inference server")?;
                let addr = listener.local_addr().context("local_addr")?;
                tokio::spawn(async move {
                    axum::serve(listener, app)
                        .with_graceful_shutdown(async {
                            let _ = shutdown_rx.await;
                        })
                        .await
                        .unwrap();
                });
                (addr, None)
            }
            Transport::Tls => {
                let (addr, ca) = crate::mock_server_tls::serve_tls(app, shutdown_rx).await?;
                (addr, Some(ca))
            }
        };

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::net::TcpStream::connect(addr).await.is_err() {
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("mock server not ready within 5s");
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        Ok(MockInferenceServer {
            addr,
            shutdown_tx: Some(shutdown_tx),
<<<<<<< HEAD
            state,
            tls_ca,
=======
            log,
            models: shared_models,
            settings: shared_settings,
            response_mode,
            overrides,
            agent_turns,
            messages_stop_reason,
            chunk_delay,
            storage,
            user_tier,
>>>>>>> e3fdf3ed (Merge 2 (#4))
        })
    }

    pub fn set_models(&self, models: Vec<MockModelEntry>) {
        *self.state.models.write().unwrap() =
            ModelCatalog::Installed(models.iter().map(MockModelEntry::to_json).collect());
    }

    /// A call that names nothing leaves the catalog as it is, including the model `start` seeds.
    /// The first call that names a model replaces that boot catalog. A later call keeps entries it
    /// does not name, with `leading` in front and `trailing` after.
    pub fn install_models(&self, leading: &[MockModelEntry], trailing: &[MockModelEntry]) {
        if leading.is_empty() && trailing.is_empty() {
            return;
        }
        let mut guard = self.state.models.write().unwrap();
        let merged = match &mut *guard {
            ModelCatalog::Installed(existing) => {
                let named: Vec<&str> = leading
                    .iter()
                    .chain(trailing.iter())
                    .map(|entry| entry.id.as_str())
                    .collect();
                let kept: Vec<Value> = existing
                    .iter()
                    .filter(|model| match model.get("id").and_then(Value::as_str) {
                        Some(id) => !named.contains(&id),
                        None => true,
                    })
                    .cloned()
                    .collect();
                leading
                    .iter()
                    .map(MockModelEntry::to_json)
                    .chain(kept)
                    .chain(trailing.iter().map(MockModelEntry::to_json))
                    .collect()
            }
            ModelCatalog::Boot(_) => leading
                .iter()
                .chain(trailing.iter())
                .map(MockModelEntry::to_json)
                .collect(),
        };
        *guard = ModelCatalog::Installed(merged);
    }

    /// Stream this text instead of echoing. Deltas reconstruct it byte for byte.
    pub fn set_response(&self, text: impl Into<String>) {
        self.state.inference.set_response(text.into());
    }

    pub fn set_auxiliary_hold(&self) {
        self.state.inference.set_auxiliary_hold();
    }

    /// Park the next foreground reply, including a tool call, until [`Self::release_parked_replies`].
    pub fn set_foreground_hold(&self) {
        self.state.inference.set_foreground_hold();
    }

    /// Park the next auxiliary whose `x-grok-req-id` starts with `prefix`.
    /// An auxiliary with a different id leaves the hold armed.
    pub fn set_auxiliary_hold_matching(&self, prefix: impl Into<String>) {
        self.state.inference.set_auxiliary_hold_matching(prefix);
    }

    pub async fn wait_until_parked_replies(&self, at_least: usize) {
        self.state
            .overrides
            .wait_until_parked_replies(at_least)
            .await;
    }

    /// Consumed FIFO per `path`, e.g. `"/v1/chat/completions"`.
    /// An empty queue falls back to the response mode.
    pub fn enqueue_response(&self, path: impl Into<String>, response: ScriptedResponse) {
<<<<<<< HEAD
        self.state.overrides.enqueue_response(path, response);
    }

    /// Default-responder concurrency cap: over `cap` in-flight (each held for `hold`), extra requests get 429 with `Retry-After`.
    /// Scripted responses and expectations bypass the cap.
    pub fn set_inference_concurrency_cap(&self, cap: usize, hold: Duration, retry_after_secs: u64) {
        self.state
            .overrides
            .set_concurrency_cap(cap, hold, retry_after_secs);
    }

=======
        self.overrides.enqueue_response(path, response);
    }

>>>>>>> e3fdf3ed (Merge 2 (#4))
    /// Register one named response matched atomically by endpoint and request kind.
    #[must_use = "keep the handle to synchronize and assert expectation satisfaction"]
    pub fn expect_response(
        &self,
        name: impl Into<String>,
        matcher: InferenceRequestMatcher,
        response: ScriptedResponse,
    ) -> InferenceExpectation {
<<<<<<< HEAD
        self.state.overrides.register_expectation(
            name, matcher, response, /*block_before_terminal*/ false,
        )
=======
        self.overrides
            .register_expectation(name, matcher, response, false)
>>>>>>> e3fdf3ed (Merge 2 (#4))
    }

    /// Register a named response that pauses immediately before completion.
    #[must_use = "keep the handle to release and assert expectation satisfaction"]
    pub fn expect_response_blocked(
        &self,
        name: impl Into<String>,
        matcher: InferenceRequestMatcher,
        response: ScriptedResponse,
    ) -> InferenceExpectation {
<<<<<<< HEAD
        self.state
            .overrides
            .register_expectation(name, matcher, response, /*block_before_terminal*/ true)
    }

    /// Queue one byte-exact response per foreground turn.
=======
        self.overrides
            .register_expectation(name, matcher, response, true)
    }

    /// Queue one byte-exact response per foreground turn as compatibility sugar.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    pub fn set_agent_turns(&self, turns: impl IntoIterator<Item = String>) {
        self.state
            .inference
            .set_agent_turns(turns.into_iter().collect());
    }

    /// Replaces earlier conversations and restarts each script. One not served through stays reported.
    pub fn set_conversations(&self, conversations: impl IntoIterator<Item = Conversation>) {
        self.state
            .overrides
            .conversation_scripts()
            .set(conversations);
    }

    /// Each violation once: those recorded, then every script not served through or never opened.
    #[must_use]
    pub fn script_violations(&self) -> Vec<ScriptViolation> {
        self.state.overrides.conversation_scripts().violations()
    }

    /// Later requests fall through to the fallback modes; the drop check has nothing left to report.
    #[must_use = "assert on the violations; finishing disarms the drop check"]
    pub fn finish_scripts(&self) -> Vec<ScriptViolation> {
        self.state.overrides.conversation_scripts().finish()
    }

    /// A violation serves a reply instead of failing the request, so this also runs on drop.
    #[track_caller]
    pub fn assert_no_script_violations(&self) {
        let violations = self.script_violations();
        assert!(
            violations.is_empty(),
            "conversation scripts departed from:\n{}\nconversations:\n{}\nrequests:\n{}\n\
             a test that returned early through `?` has its error replaced by this panic; \
             call finish_scripts() before returning",
            lines(&violations),
            lines(&self.conversations()),
            self.request_log_summary()
        );
    }

    #[must_use]
    pub fn conversations(&self) -> Vec<ReadConversation> {
        self.state.inference.conversations()
    }

    #[must_use]
    pub fn conversation(&self, number: usize) -> Option<ReadConversation> {
        self.conversations()
            .into_iter()
            .find(|conversation| conversation.number() == number)
    }

    #[must_use]
    pub fn conversation_for_system_prompt(&self, contains: &str) -> Option<ReadConversation> {
        self.conversations().into_iter().find(|conversation| {
            conversation
                .first_system_prompt()
                .is_some_and(|prompt| prompt.contains(contains))
        })
    }

    /// Until this is called, `GET /v1/settings` returns 404.
    pub fn set_settings(&self, settings: impl serde::Serialize) {
        let value = serde_json::to_value(settings).expect("serialize settings");
        let mut guard = self.state.settings.write().unwrap();
        *guard = Some(value);
    }

    /// The smallest settings payload that opens the subscription gate.
    /// Without it a client sits on the upsell screen.
    pub fn preset_allow_access(&self) {
        self.set_settings(json!({ "allow_access": true }));
    }

    pub fn set_hang(&self, hang: bool) {
        *self.state.startup_fetch_stall.write().unwrap() = if hang {
            StartupFetchStall::Hang
        } else {
            StartupFetchStall::None
        };
    }

    pub fn set_startup_fetch_delay(&self, delay: Duration) {
        *self.state.startup_fetch_stall.write().unwrap() = StartupFetchStall::Delay(delay);
    }

    pub fn clear_startup_fetch_stall(&self) {
        *self.state.startup_fetch_stall.write().unwrap() = StartupFetchStall::None;
    }

    pub fn startup_stalls_served(&self) -> u32 {
        self.state.startup_stalls_served.load(Ordering::Relaxed)
    }

    /// The `subscriptionTier` on `GET /v1/user`.
    /// `None`, the default, omits the field, which the shell reads as the free tier.
    pub fn set_user_subscription_tier(&self, tier: Option<&str>) {
        *self.state.user_tier.write().unwrap() = tier.map(str::to_owned);
    }

    pub fn set_user_team(&self, team: MockUserTeam) {
        *self.state.user_team.write().unwrap() = Some(team);
    }

    pub fn set_user_can_administer_team(&self, can_administer: MockCanAdministerTeam) {
        *self.state.user_can_administer_team.write().unwrap() = can_administer;
    }

    /// `codingDataRetentionOptOut` on `GET /v1/user`. `None`, the default, omits the field.
    pub fn set_user_coding_data_retention_opt_out(&self, opt_out: Option<bool>) {
        *self
            .state
            .user_coding_data_retention_opt_out
            .write()
            .unwrap() = opt_out;
    }

    /// Park every `GET /v1/user` after logging it, until [`Self::release_user_info`].
    pub fn hold_user_info(&self) {
        self.state.user_info_released.send_replace(false);
    }

    pub fn release_user_info(&self) {
        self.state.user_info_released.send_replace(true);
    }

    /// Resolves once `n` `GET /v1/user` have arrived, parked ones included; panics after 5s rather than hang.
    pub async fn user_info_arrived(&self, n: usize) {
        let mut arrivals = self.state.user_info_arrivals.subscribe();
        let waited = tokio::time::timeout(
            Duration::from_secs(5),
            arrivals.wait_for(|count| *count >= n),
        )
        .await;
        assert!(
            waited.is_ok(),
            "expected {n} GET /v1/user, saw {} within 5s",
            *self.state.user_info_arrivals.borrow()
        );
    }

    pub fn set_messages_stop_reason(&self, stop_reason: impl Into<String>) {
        self.state
            .inference
            .set_messages_stop_reason(stop_reason.into());
    }

    /// Emit each SSE event after `delay`, so a test can hold a turn visibly streaming.
    /// `None` restores instant streaming.
    /// Applies to requests started after the call.
    pub fn set_chunk_delay(&self, delay: Option<Duration>) {
        self.state.inference.set_chunk_delay(delay);
    }

<<<<<<< HEAD
    /// Hold foreground terminal SSE events until [`Self::release_agent_completions`].
    /// Prefer per-expectation blocking.
    pub fn hold_agent_completions(&self) {
        self.state.overrides.hold_completions();
    }

    /// Arms the reply hold. Cancel and drop both release it.
    pub fn arm_reply_hold(&self) -> crate::inference_override::ArmedReplyHold {
        self.state.overrides.arm_reply_hold()
    }

    /// Later [`SseEvent::hold`] markers each wait for one [`Self::release_one_chunk`].
    /// The completion gate is left as it was.
    pub fn arm_chunk_release(&self) {
        self.state.overrides.arm_chunk_release();
    }

    pub fn release_one_chunk(&self) {
        self.state.overrides.release_one_chunk();
    }

    /// Let every held SSE chunk still waiting through, and leave later holds on the completion gate.
    pub fn release_remaining_chunks(&self) {
        self.state.overrides.release_remaining_chunks();
    }

    /// Other conversations stream.
    pub fn hold_only_conversation(&self, conversation: usize) {
        self.state.overrides.hold_only_conversation(conversation);
=======
    /// Hold foreground terminal SSE events until [`release_agent_completions`].
    /// Compatibility API; per-expectation blocking gives tighter ownership.
    ///
    /// [`release_agent_completions`]: Self::release_agent_completions
    pub fn hold_agent_completions(&self) {
        self.overrides.hold_completions();
>>>>>>> e3fdf3ed (Merge 2 (#4))
    }

    pub fn release_agent_completions(&self) {
<<<<<<< HEAD
        self.state.overrides.release_completions();
    }

    /// Hold the terminal SSE event of every foreground reply in `conversation`, counted from 1, until
    /// [`Self::release_conversation`], so one session's turn stays in flight while others finish.
    pub fn hold_conversation(&self, conversation: usize) {
        self.state
            .overrides
            .hold_conversation(ConversationId::nth(conversation));
    }

    pub fn release_conversation(&self, conversation: usize) {
        self.state
            .overrides
            .release_conversation(ConversationId::nth(conversation));
    }

    /// Resolves once `n` replies for `conversation` (counted from 1) are parked.
    pub async fn wait_until_parked_replies_for(&self, conversation: usize, at_least: usize) {
        self.state
            .overrides
            .wait_until_parked_replies_for(conversation, at_least)
            .await;
    }

    /// Resolves once `n` auxiliaries have parked under [`Self::set_auxiliary_hold_matching`].
    pub async fn wait_until_matching_parked(&self, at_least: usize) {
        self.state
            .overrides
            .wait_until_matching_parked(at_least)
            .await;
    }

    /// Lets a scripted reply parked with `hold` finish before the mock shuts down.
    pub fn release_parked_replies(&self) {
        self.state.overrides.release_parked_replies();
    }

    /// A park that [`Self::release_scripted_park`] releases. The shutdown latch does not arm it.
    pub fn arm_scripted_park(&self) -> u64 {
        self.state.overrides.arm_scripted_park()
    }

    /// Let the reply parked under `token` finish. Other parks stay held.
    pub fn release_scripted_park(&self, token: u64) {
        self.state.overrides.release_scripted_park(token);
    }

    pub async fn wait_until_scripted_park(&self, token: u64) {
        self.state.overrides.wait_until_scripted_park(token).await;
    }

    /// Lets parked auxiliary replies finish. Foreground holds stay parked.
    pub fn release_auxiliary_replies(&self) {
        self.state.overrides.release_auxiliary_replies();
    }

    /// Resolves once `n` foreground inference requests are logged (1 is the first). Panics after 30s.
    pub async fn wait_for_inference_requests(&self, n: usize) {
        let mut arrivals = self.state.log.subscribe_inference();
        let waited = tokio::time::timeout(
            crate::scaled(Duration::from_secs(30)),
            arrivals.wait_for(|count| *count >= n),
        )
        .await;
        assert!(
            waited.is_ok(),
            "expected {n} foreground inference requests, saw {} within 30s",
            self.state.log.inference_count()
        );
    }

    pub fn agent_completion_parked(&self) -> bool {
        self.state.overrides.agent_completion_parked()
    }

    pub async fn wait_until_a_reply_is_held(&self) {
        self.state.overrides.wait_until_a_reply_is_held().await;
=======
        self.overrides.release_completions();
>>>>>>> e3fdf3ed (Merge 2 (#4))
    }

    pub fn url(&self) -> String {
        format!("{}://{}/v1", self.scheme(), self.addr)
    }

    pub fn origin(&self) -> String {
        format!("{}://{}", self.scheme(), self.addr)
    }

    fn scheme(&self) -> &'static str {
        if self.tls_ca.is_some() {
            "https"
        } else {
            "http"
        }
    }

    pub fn ca_pem_path(&self) -> Option<&Path> {
        self.tls_ca.as_ref().map(ThrowawayCa::pem_path)
    }

    pub fn request_count(&self) -> u32 {
        self.state.log.count()
    }

    pub fn request_count_for(&self, path: &str) -> usize {
        self.state.log.count_for(path)
    }

    /// Stop retaining entries. [`Self::request_count`] stays exact.
    pub fn set_keep_requests(&self, enabled: bool) {
        self.state.log.set_keep_entries(enabled);
    }

    /// Without this, inference logs keep only the parsed body.
    pub fn set_capture_request_bytes(&self, enabled: bool) {
        self.state.log.set_capture_request_bytes(enabled);
    }

    #[must_use]
    pub fn replies_served(&self, conversation: usize) -> usize {
        self.state
            .overrides
            .conversation_scripts()
            .replies_served(conversation)
    }

    /// Turns this conversation has already answered, by script index.
    #[must_use]
    pub fn answered_turns(&self, conversation: usize) -> BTreeSet<usize> {
        self.state
            .overrides
            .conversation_scripts()
            .answered_turns(conversation)
    }

    pub fn requests(&self) -> Vec<LogEntry> {
        self.state.log.entries()
    }

    /// What arrived since the previous observation. The full-log getters stay complete.
    pub fn arrived_since_observation(&self) -> ObservationRecords {
        ObservationRecords {
            requests: self.state.log.take_for_observation(),
            telemetry: self.state.telemetry.take_for_observation(),
            storage_uploads: self.state.storage.take_for_observation(),
            gateway_calls: self.state.managed_gateway.take_for_observation(),
        }
    }

    /// Bodies of all received requests, in arrival order (body-less requests such as `GET /v1/models` are skipped).
    pub fn request_bodies(&self) -> Vec<Value> {
        self.state.log.bodies()
    }

    pub fn has_chat_completion_request(&self) -> bool {
        self.state.log.has_path_containing("chat/completions")
    }

    pub fn has_responses_request(&self) -> bool {
        self.state.log.has_path_containing("responses")
    }

    pub fn messages_request_count(&self) -> usize {
        self.state.log.count_for("/v1/messages")
    }

    pub fn request_log_summary(&self) -> String {
        self.state.log.summary()
    }

    pub fn last_system_prompt(&self) -> Option<String> {
        self.state.log.last_system_prompt()
    }

    /// While closed, every `/v1/storage` upload is rejected with 401.
    pub fn set_storage_unauthorized(&self, unauthorized: bool) {
        self.state.storage.set_unauthorized(unauthorized);
    }

    /// Total `/v1/storage` upload attempts seen, including 401-rejected ones.
    pub fn storage_request_count(&self) -> u32 {
        self.state.storage.request_count()
    }

    /// Only the uploads that were accepted.
    pub fn storage_uploads(&self) -> Vec<StorageUpload> {
        self.state.storage.uploads()
    }

    /// While set, every `POST /v1/feedback` answers 500 (the body is still recorded).
    pub fn set_feedback_failure(&self, fail: bool) {
        self.state.feedback.set_failure(fail);
    }

    /// Every `POST /v1/feedback` seen so far, accepted or scripted to fail, in arrival order.
    pub fn feedback_posts(&self) -> Vec<FeedbackPost> {
        self.state.feedback.posts()
    }

    /// Every product-telemetry event posted to `/v1/events` so far, flattened out of its batch, in arrival order.
    pub fn telemetry_events(&self) -> Vec<Value> {
        self.state.telemetry.events()
    }

<<<<<<< HEAD
    /// Serves `catalog` on `GET /v1/mcp/tools/list` and answers every `POST /v1/mcp/tools/call`
    /// with `call_result` as its `result`; both routes 404 until this is called.
    pub fn set_managed_gateway(&self, catalog: Value, call_result: Value) {
        self.state.managed_gateway.set_script(catalog, call_result);
    }

    pub fn managed_gateway_calls(&self) -> Vec<ManagedGatewayCall> {
        self.state.managed_gateway.calls()
    }

    fn build_router(state: RouterState) -> Router {
        Router::new()
            .route(
                InferenceEndpoint::ChatCompletions.path(),
                state.inference.handler(InferenceEndpoint::ChatCompletions),
            )
            .route(
                InferenceEndpoint::Responses.path(),
                state.inference.handler(InferenceEndpoint::Responses),
            )
            .route(
                InferenceEndpoint::Messages.path(),
                state.inference.handler(InferenceEndpoint::Messages),
            )
            .route(
                "/v1/images/generations",
                post({
                    let state = state.clone();
                    move |headers: HeaderMap, Json(body): Json<Value>| {
                        let state = state.clone();
                        async move {
                            state
                                .log
                                .record("POST", "/v1/images/generations", &body, &headers);
                            Json(json!({ "data": [ { "b64_json": "" } ] })).into_response()
                        }
=======
    fn pop_agent_turn(
        agent_turns: &Arc<std::sync::Mutex<VecDeque<String>>>,
        request: &ClassifiedInferenceRequest,
    ) -> Option<String> {
        if !request.is_foreground() {
            return None;
        }
        agent_turns.lock().unwrap().pop_front()
    }

    fn build_router(
        log: Arc<RequestLog>,
        models: Arc<std::sync::RwLock<Vec<Value>>>,
        settings: Arc<std::sync::RwLock<Option<Value>>>,
        response_mode: Arc<std::sync::RwLock<ResponseMode>>,
        overrides: InferenceOverrides,
        agent_turns: Arc<std::sync::Mutex<VecDeque<String>>>,
        messages_stop_reason: Arc<std::sync::RwLock<String>>,
        chunk_delay: Arc<std::sync::RwLock<Option<Duration>>>,
        storage: Arc<StorageState>,
        user_tier: Arc<std::sync::RwLock<Option<String>>>,
    ) -> Router {
        let log_cc = log.clone();
        let log_rs = log.clone();
        let log_msg = log.clone();
        let mode_cc = response_mode.clone();
        let mode_rs = response_mode.clone();
        let mode_msg = response_mode;
        let overrides_cc = overrides.clone();
        let overrides_rs = overrides.clone();
        let overrides_settings = overrides.clone();
        let overrides_msg = overrides;
        let agent_turns_cc = agent_turns.clone();
        let agent_turns_rs = agent_turns.clone();
        let agent_turns_msg = agent_turns;
        let delay_cc = chunk_delay.clone();
        let delay_rs = chunk_delay.clone();
        let delay_msg = chunk_delay;

        Router::new()
            .route(
                "/v1/chat/completions",
                post(move |headers: HeaderMap, Json(body): Json<Value>| {
                    let log = log_cc.clone();
                    let mode = mode_cc.clone();
                    let overrides = overrides_cc.clone();
                    let agent_turns = agent_turns_cc.clone();
                    let delay = delay_cc.clone();
                    async move {
                        let auth = Self::extract_auth(&headers);
                        log.record(
                            "POST",
                            "/v1/chat/completions",
                            Some(&body),
                            auth.as_deref(),
                            Self::headers_vec(&headers),
                        );

                        let request =
                            overrides.classify(InferenceEndpoint::ChatCompletions, &headers, &body);
                        let chunk_delay = *delay.read().unwrap();
                        if let Some(response) = overrides
                            .response_override(&request, &headers, chunk_delay)
                            .await
                        {
                            return response;
                        }

                        let user_msg = body
                            .get("messages")
                            .and_then(|m| m.as_array())
                            .and_then(|msgs| {
                                msgs.iter()
                                    .rev()
                                    .find(|m| m.get("role").and_then(Value::as_str) == Some("user"))
                            })
                            .and_then(|m| m.get("content"))
                            .and_then(Value::as_str)
                            .unwrap_or("hello");

                        let model = body
                            .get("model")
                            .and_then(Value::as_str)
                            .unwrap_or("test-model");

                        let events = match Self::pop_agent_turn(&agent_turns, &request) {
                            Some(text) => sse::chat_completion_events_exact(&text, model),
                            None => match &*mode.read().unwrap() {
                                ResponseMode::Echo => {
                                    sse::chat_completion_events(&format!("Echo: {user_msg}"), model)
                                }
                                ResponseMode::Fixed(text) => {
                                    sse::chat_completion_events_exact(text, model)
                                }
                            },
                        };
                        let gate = overrides.fallback_terminal_wait(&request);
                        let stream = paced_events(events, *delay.read().unwrap(), gate);
                        Sse::new(stream)
                            .keep_alive(KeepAlive::default())
                            .into_response()
>>>>>>> e3fdf3ed (Merge 2 (#4))
                    }
                }),
            )
            .route(
<<<<<<< HEAD
                "/v1/videos/generations",
                post({
                    let state = state.clone();
                    move |headers: HeaderMap, Json(body): Json<Value>| {
                        let state = state.clone();
                        async move {
                            state
                                .log
                                .record("POST", "/v1/videos/generations", &body, &headers);
                            Json(json!({ "request_id": "media-1" })).into_response()
                        }
=======
                "/v1/responses",
                post(move |headers: HeaderMap, Json(body): Json<Value>| {
                    let log = log_rs.clone();
                    let mode = mode_rs.clone();
                    let overrides = overrides_rs.clone();
                    let agent_turns = agent_turns_rs.clone();
                    let delay = delay_rs.clone();
                    async move {
                        let auth = Self::extract_auth(&headers);
                        log.record(
                            "POST",
                            "/v1/responses",
                            Some(&body),
                            auth.as_deref(),
                            Self::headers_vec(&headers),
                        );

                        let request =
                            overrides.classify(InferenceEndpoint::Responses, &headers, &body);
                        let chunk_delay = *delay.read().unwrap();
                        if let Some(response) = overrides
                            .response_override(&request, &headers, chunk_delay)
                            .await
                        {
                            return response;
                        }

                        let user_msg = body
                            .get("input")
                            .and_then(|i| i.as_array())
                            .and_then(|items| {
                                items.iter().rev().find(|item| {
                                    item.get("role").and_then(Value::as_str) == Some("user")
                                })
                            })
                            .and_then(|item| {
                                item.get("content").and_then(|c| {
                                    c.as_str().map(String::from).or_else(|| {
                                        c.as_array().and_then(|parts| {
                                            parts.iter().find_map(|p| {
                                                if p.get("type").and_then(Value::as_str)
                                                    == Some("input_text")
                                                {
                                                    p.get("text")
                                                        .and_then(Value::as_str)
                                                        .map(String::from)
                                                } else {
                                                    None
                                                }
                                            })
                                        })
                                    })
                                })
                            })
                            .unwrap_or_else(|| "hello".to_string());

                        let model = body
                            .get("model")
                            .and_then(Value::as_str)
                            .unwrap_or("test-model");

                        let events = match Self::pop_agent_turn(&agent_turns, &request) {
                            Some(text) => sse::responses_api_events_exact(&text, model),
                            None => match &*mode.read().unwrap() {
                                ResponseMode::Echo => {
                                    sse::responses_api_events(&format!("Echo: {user_msg}"), model)
                                }
                                ResponseMode::Fixed(text) => {
                                    sse::responses_api_events_exact(text, model)
                                }
                            },
                        };
                        let gate = overrides.fallback_terminal_wait(&request);
                        let stream = paced_events(events, *delay.read().unwrap(), gate);
                        Sse::new(stream)
                            .keep_alive(KeepAlive::default())
                            .into_response()
                    }
                }),
            )
            .route(
                "/v1/messages",
                post(move |headers: HeaderMap, Json(body): Json<Value>| {
                    let log = log_msg.clone();
                    let mode = mode_msg.clone();
                    let overrides = overrides_msg.clone();
                    let agent_turns = agent_turns_msg.clone();
                    let stop_reason = messages_stop_reason.clone();
                    let delay = delay_msg.clone();
                    async move {
                        let auth = Self::extract_auth(&headers);
                        log.record(
                            "POST",
                            "/v1/messages",
                            Some(&body),
                            auth.as_deref(),
                            Self::headers_vec(&headers),
                        );

                        let request =
                            overrides.classify(InferenceEndpoint::Messages, &headers, &body);
                        let chunk_delay = *delay.read().unwrap();
                        if let Some(response) = overrides
                            .response_override(&request, &headers, chunk_delay)
                            .await
                        {
                            return response;
                        }

                        // Anthropic content is either a plain string or an
                        // array of typed blocks; extract the last user text.
                        let user_msg = body
                            .get("messages")
                            .and_then(|m| m.as_array())
                            .and_then(|msgs| {
                                msgs.iter()
                                    .rev()
                                    .find(|m| m.get("role").and_then(Value::as_str) == Some("user"))
                            })
                            .and_then(|m| m.get("content"))
                            .and_then(|c| {
                                c.as_str().map(String::from).or_else(|| {
                                    c.as_array().and_then(|blocks| {
                                        blocks.iter().find_map(|b| {
                                            if b.get("type").and_then(Value::as_str) == Some("text")
                                            {
                                                b.get("text")
                                                    .and_then(Value::as_str)
                                                    .map(String::from)
                                            } else {
                                                None
                                            }
                                        })
                                    })
                                })
                            })
                            .unwrap_or_else(|| "hello".to_string());

                        let model = body
                            .get("model")
                            .and_then(Value::as_str)
                            .unwrap_or("test-model");

                        let stop = stop_reason.read().unwrap().clone();
                        let events = match Self::pop_agent_turn(&agent_turns, &request) {
                            Some(text) => sse::messages_api_events(&text, model, &stop),
                            None => match &*mode.read().unwrap() {
                                ResponseMode::Echo => sse::messages_api_events(
                                    &format!("Echo: {user_msg}"),
                                    model,
                                    &stop,
                                ),
                                ResponseMode::Fixed(text) => {
                                    sse::messages_api_events(text, model, &stop)
                                }
                            },
                        };
                        let gate = overrides.fallback_terminal_wait(&request);
                        let stream = paced_events(events, *delay.read().unwrap(), gate);
                        Sse::new(stream)
                            .keep_alive(KeepAlive::default())
                            .into_response()
>>>>>>> e3fdf3ed (Merge 2 (#4))
                    }
                }),
            )
            .route(
                "/v1/models",
                get({
                    let state = state.clone();
                    move |headers: HeaderMap| {
                        let state = state.clone();
                        async move {
                            state.log.record_get_with_authorization(
                                "/v1/models",
                                headers
                                    .get("authorization")
                                    .and_then(|value| value.to_str().ok())
                                    .map(str::to_owned),
                            );
                            state.stall_startup_fetch().await;
                            let models_json = match &*state.models.read().unwrap() {
                                ModelCatalog::Boot(entries) | ModelCatalog::Installed(entries) => {
                                    entries.clone()
                                }
                            };
                            Json(json!({
                                "object": "list",
                                "data": models_json,
                            }))
                        }
                    }
                }),
            )
            .route(
                "/v1/settings",
                get({
                    let state = state.clone();
                    move || {
<<<<<<< HEAD
                        let state = state.clone();
                        async move {
                            state.log.record_get("/v1/settings");
                            state.stall_startup_fetch().await;
                            // Scripts take precedence, so a test can serve a transient payload before the steady-state value
                            if let Some(s) = state.overrides.pop_scripted("/v1/settings") {
=======
                        let log = log.clone();
                        let settings = settings.clone();
                        let overrides = overrides_settings.clone();
                        async move {
                            log.record("GET", "/v1/settings", None, None, Vec::new());
                            // Scripted one-shots take precedence (FIFO), so a
                            // test can serve a transient payload (e.g. one
                            // stale gated snapshot) and fall back to the
                            // steady-state `set_settings` value afterwards.
                            if let Some(s) = overrides.pop_scripted("/v1/settings") {
>>>>>>> e3fdf3ed (Merge 2 (#4))
                                return s.into_response_paced(None, None).await;
                            }
                            let maybe = state.settings.read().unwrap().clone();
                            match maybe {
                                Some(s) => Json(s).into_response(),
                                None => StatusCode::NOT_FOUND.into_response(),
                            }
                        }
                    }
                }),
            )
            .route(
                "/v1/privacy/coding-data-retention",
                put({
                    let state = state.clone();
                    move |headers: HeaderMap, Json(body): Json<Value>| {
                        let state = state.clone();
                        async move {
                            let path = "/v1/privacy/coding-data-retention";
                            state.log.record("PUT", path, &body, &headers);
                            if let Some(s) = state.overrides.pop_scripted(path) {
                                return s.into_response_paced(None, None).await;
                            }
                            let opt_out = body
                                .get("codingDataRetentionOptOut")
                                .cloned()
                                .unwrap_or(Value::Bool(false));
                            Json(json!({ "codingDataRetentionOptOut": opt_out })).into_response()
                        }
                    }
                }),
            )
            .route(
                "/v1/consent/accept",
                post({
                    let state = state.clone();
                    move |headers: HeaderMap, Json(body): Json<Value>| {
                        let state = state.clone();
                        async move {
                            let path = "/v1/consent/accept";
                            state.log.record("POST", path, &body, &headers);
                            if let Some(s) = state.overrides.pop_scripted(path) {
                                return s.into_response_paced(None, None).await;
                            }
                            Json(body).into_response()
                        }
                    }
                }),
            )
            .route(
                "/v1/user",
                get({
                    let state = state.clone();
                    move |axum::extract::RawQuery(query): axum::extract::RawQuery| {
                        let state = state.clone();
                        async move {
                            // Log the query string so a test can count `?include=subscription` on its own
                            let path = match query {
                                Some(q) if !q.is_empty() => format!("/v1/user?{q}"),
                                _ => "/v1/user".to_owned(),
                            };
                            state.log.record_get(&path);
                            state.user_info_arrivals.send_modify(|count| *count += 1);
                            let mut released = state.user_info_released.subscribe();
                            if !*released.borrow_and_update() {
                                let _ = released.wait_for(|r| *r).await;
                            }
                            let tier = state.user_tier.read().unwrap().clone();
                            let team = state.user_team.read().unwrap().clone();
                            let can_administer =
                                state.user_can_administer_team.read().unwrap().wire_value();
                            let opt_out = *state.user_coding_data_retention_opt_out.read().unwrap();
                            let mut body = json!({
                                "userId": "mock-user",
                                "email": "mock-user@test.invalid",
                            });
                            if let Some(obj) = body.as_object_mut() {
                                if let Some(opt_out) = opt_out {
                                    obj.insert("codingDataRetentionOptOut".into(), json!(opt_out));
                                }
                                if let Some(t) = tier {
                                    obj.insert("subscriptionTier".into(), json!(t));
                                }
                                if let Some(team) = team {
                                    obj.insert("teamId".into(), json!(team.id));
                                    obj.insert("teamName".into(), json!(team.name));
                                    obj.insert("teamRole".into(), json!(team.role));
                                }
                                if let Some(can_administer) = can_administer {
                                    obj.insert("canAdministerTeam".into(), can_administer);
                                }
                            }
                            Json(body).into_response()
                        }
                    }
                }),
            )
            .route(
                "/sessions/{id}/data",
                post({
                    let state = state.clone();
                    move |axum::extract::Path(id): axum::extract::Path<String>,
                          headers: HeaderMap,
                          Json(body): Json<Value>| {
                        let state = state.clone();
                        async move {
                            if let Some(reject) = state.overrides.auth_rejection(&headers) {
                                return reject;
                            }
                            state.log.record(
                                "POST",
                                &format!("/sessions/{id}/data"),
                                &body,
                                &headers,
                            );
                            StatusCode::OK.into_response()
                        }
                    }
                }),
            )
            .route(
                "/sessions/{id}",
                put({
                    let state = state.clone();
                    move |axum::extract::Path(id): axum::extract::Path<String>,
                          headers: HeaderMap,
                          Json(body): Json<Value>| {
                        let state = state.clone();
                        async move {
                            if let Some(reject) = state.overrides.auth_rejection(&headers) {
                                return reject;
                            }
                            state
                                .log
                                .record("PUT", &format!("/sessions/{id}"), &body, &headers);
                            StatusCode::OK.into_response()
                        }
                    }
                }),
            )
            .route(
                "/v1/storage",
                post({
                    let storage = state.storage.clone();
                    move |headers: HeaderMap, body: axum::body::Bytes| {
                        let storage = storage.clone();
                        async move { storage.handle(&headers, &body) }
                    }
                }),
            )
            .route(
                "/v1/feedback",
                post({
                    let feedback = state.feedback.clone();
                    move |headers: HeaderMap, body: axum::body::Bytes| {
                        let feedback = feedback.clone();
                        async move { feedback.handle(&headers, &body) }
                    }
                }),
            )
            .route(
                "/v1/events",
                post({
                    let telemetry = state.telemetry.clone();
                    move |body: axum::body::Bytes| {
                        let telemetry = telemetry.clone();
                        async move { telemetry.handle(&body) }
                    }
                }),
            )
            .route(
                "/v1/mcp/tools/list",
                get({
                    let state = state.clone();
                    move || {
                        let state = state.clone();
                        async move {
                            state.log.record_get("/v1/mcp/tools/list");
                            state.managed_gateway.list()
                        }
                    }
                }),
            )
            .route(
                "/v1/mcp/tools/call",
                post({
                    let state = state.clone();
                    move |headers: HeaderMap, Json(body): Json<Value>| {
                        let state = state.clone();
                        async move {
                            state
                                .log
                                .record("POST", "/v1/mcp/tools/call", &body, &headers);
                            state.managed_gateway.call(&headers, &body)
                        }
                    }
                }),
            )
            // 404 reads as an old proxy, so the shell falls back to a plain `POST /v1/storage`
            .route(
                "/v1/storage/exists",
                get(|| async { StatusCode::NOT_FOUND }),
            )
            .route(
                "/v1/storage/batch_exists",
                post(|| async { StatusCode::NOT_FOUND }),
            )
            .route(
                "/v1/storage/batch_upload_json",
                post(|| async { StatusCode::NOT_FOUND }),
            )
            .route(
                "/v1/storage/batch_upload",
                post(|| async { StatusCode::NOT_FOUND }),
            )
            .route(
                "/v1/storage/limits",
                get(|| async { StatusCode::NOT_FOUND }),
            )
            .layer(axum::extract::DefaultBodyLimit::max(256 * 1024 * 1024))
    }
}

fn lines<T: fmt::Display>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

impl Drop for MockInferenceServer {
    fn drop(&mut self) {
        self.state.overrides.release_parked_replies();
        self.state.overrides.release_completions();
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if !std::thread::panicking() {
            self.assert_no_script_violations();
        }
    }
}

#[cfg(test)]
<<<<<<< HEAD
#[path = "mock_server_tests.rs"]
mod tests;
=======
mod tests {
    use super::*;

    const MERMAID_TEXT: &str =
        "Here is a flow:\n\n```mermaid\nflowchart TD\n  A --> B\n```\n\nDone.\n";

    /// Payloads of all `data:` lines in an SSE body, minus the `[DONE]` marker.
    fn sse_data_payloads(body: &str) -> Vec<String> {
        body.lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .map(|d| d.trim_start().to_owned())
            .filter(|d| d != "[DONE]")
            .collect()
    }

    /// Concatenation of all chat-completion content deltas in an SSE body.
    fn chat_stream_text(body: &str) -> String {
        sse_data_payloads(body)
            .iter()
            .filter_map(|d| serde_json::from_str::<Value>(d).ok())
            .filter_map(|v| {
                v.get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("content"))
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .collect()
    }

    /// Concatenation of all responses-API output_text deltas in an SSE body.
    fn responses_stream_text(body: &str) -> String {
        sse_data_payloads(body)
            .iter()
            .filter_map(|d| serde_json::from_str::<Value>(d).ok())
            .filter(|v| v.get("type").and_then(Value::as_str) == Some("response.output_text.delta"))
            .filter_map(|v| v.get("delta").and_then(Value::as_str).map(String::from))
            .collect()
    }

    /// Concatenation of all Anthropic Messages text deltas in an SSE body.
    fn messages_stream_text(body: &str) -> String {
        sse_data_payloads(body)
            .iter()
            .filter_map(|d| serde_json::from_str::<Value>(d).ok())
            .filter(|v| v.get("type").and_then(Value::as_str) == Some("content_block_delta"))
            .filter_map(|v| {
                v.get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .collect()
    }

    fn foreground_body(endpoint: InferenceEndpoint, content: &str) -> Value {
        let tools = json!([
            { "type": "function", "function": { "name": "read_file" } },
            { "type": "function", "function": { "name": "write" } }
        ]);
        match endpoint {
            InferenceEndpoint::ChatCompletions | InferenceEndpoint::Messages => json!({
                "model": "test-model",
                "messages": [{ "role": "user", "content": content }],
                "tools": tools,
            }),
            InferenceEndpoint::Responses => json!({
                "model": "test-model",
                "input": [{ "role": "user", "content": content }],
                "tools": tools,
            }),
        }
    }

    fn endpoint_url(server: &MockInferenceServer, endpoint: InferenceEndpoint) -> String {
        let suffix = match endpoint {
            InferenceEndpoint::ChatCompletions => "chat/completions",
            InferenceEndpoint::Responses => "responses",
            InferenceEndpoint::Messages => "messages",
        };
        format!("{}/{suffix}", server.url())
    }

    async fn post_chat(server: &MockInferenceServer, content: &str) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/chat/completions", server.url()))
            .json(&json!({
                "model": "test-model",
                "messages": [{ "role": "user", "content": content }]
            }))
            .send()
            .await
            .expect("POST /v1/chat/completions")
    }

    async fn post_foreground(
        server: &MockInferenceServer,
        endpoint: InferenceEndpoint,
        request_id: &str,
        content: &str,
    ) -> reqwest::Response {
        reqwest::Client::new()
            .post(endpoint_url(server, endpoint))
            .header("x-grok-req-id", request_id)
            .header("x-grok-turn-idx", "1")
            .json(&foreground_body(endpoint, content))
            .send()
            .await
            .expect("POST foreground inference request")
    }

    async fn read_foreground(
        server: &MockInferenceServer,
        endpoint: InferenceEndpoint,
        request_id: &str,
        content: &str,
    ) -> (reqwest::StatusCode, String) {
        let response = post_foreground(server, endpoint, request_id, content).await;
        let status = response.status();
        let body = response
            .text()
            .await
            .expect("read foreground response body");
        (status, body)
    }

    async fn read_foreground_body(
        server: &MockInferenceServer,
        endpoint: InferenceEndpoint,
        request_id: &str,
        body: Value,
    ) -> (reqwest::StatusCode, String) {
        let response = reqwest::Client::new()
            .post(endpoint_url(server, endpoint))
            .header("x-grok-req-id", request_id)
            .header("x-grok-turn-idx", "1")
            .json(&body)
            .send()
            .await
            .expect("POST foreground inference request");
        let status = response.status();
        let body = response.text().await.expect("read inference response body");
        (status, body)
    }

    #[test]
    fn explicit_request_headers_override_tool_count_heuristic() {
        let overrides = InferenceOverrides::new(None);
        let body = foreground_body(InferenceEndpoint::ChatCompletions, "title");

        let mut headers = HeaderMap::new();
        headers.insert("x-grok-req-id", "title-request".parse().unwrap());
        assert!(
            !overrides
                .classify(InferenceEndpoint::ChatCompletions, &headers, &body)
                .is_foreground()
        );

        headers.insert("x-grok-turn-idx", "1".parse().unwrap());
        assert!(
            overrides
                .classify(InferenceEndpoint::ChatCompletions, &headers, &body)
                .is_foreground()
        );

        headers.insert("x-grok-req-id", "".parse().unwrap());
        headers.insert("x-grok-turn-idx", "".parse().unwrap());
        assert!(
            overrides
                .classify(InferenceEndpoint::ChatCompletions, &headers, &body)
                .is_foreground()
        );
    }

    #[tokio::test]
    async fn auxiliary_request_does_not_consume_foreground_expectation() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut expected = server.expect_response(
            "foreground turn",
            InferenceRequestMatcher::foreground(InferenceEndpoint::ChatCompletions),
            ScriptedResponse::text(209, "foreground"),
        );

        let aux = post_chat(&server, "generate a title").await;
        assert_eq!(aux.status(), 200);
        assert!(!expected.is_satisfied());

        let (status, body) = read_foreground(
            &server,
            InferenceEndpoint::ChatCompletions,
            "turn-1",
            "run the task",
        )
        .await;
        assert_eq!(status.as_u16(), 209);
        assert_eq!(body, "foreground");
        expected.wait_received().await;
        expected.wait_satisfied().await;
    }

    #[tokio::test]
    async fn concurrent_matching_requests_claim_each_expectation_once() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut first = server.expect_response(
            "first concurrent turn",
            InferenceRequestMatcher::foreground(InferenceEndpoint::ChatCompletions),
            ScriptedResponse::text(210, "first"),
        );
        let mut second = server.expect_response(
            "second concurrent turn",
            InferenceRequestMatcher::foreground(InferenceEndpoint::ChatCompletions),
            ScriptedResponse::text(211, "second"),
        );

        let (left, right) = tokio::join!(
            read_foreground(
                &server,
                InferenceEndpoint::ChatCompletions,
                "concurrent-left",
                "left",
            ),
            read_foreground(
                &server,
                InferenceEndpoint::ChatCompletions,
                "concurrent-right",
                "right",
            )
        );
        let mut responses = vec![(left.0.as_u16(), left.1), (right.0.as_u16(), right.1)];
        responses.sort_unstable();
        assert_eq!(
            responses,
            vec![(210, "first".to_owned()), (211, "second".to_owned())]
        );
        first.wait_satisfied().await;
        second.wait_satisfied().await;
    }

    #[tokio::test]
    async fn blocked_expectation_reports_lifecycle_and_drop_releases() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut expected = server.expect_response_blocked(
            "blocked foreground turn",
            InferenceRequestMatcher::foreground(InferenceEndpoint::ChatCompletions),
            ScriptedResponse::sse(vec![
                SseEvent::data(r#"{"chunk":1}"#),
                SseEvent::data("done"),
            ]),
        );
        let request = read_foreground(
            &server,
            InferenceEndpoint::ChatCompletions,
            "blocked-turn",
            "block me",
        );
        tokio::pin!(request);

        tokio::select! {
            response = &mut request => panic!("blocked expectation completed early: {:?}", response.0),
            _ = expected.wait_blocked() => {}
        }
        assert!(!expected.is_satisfied());
        let diagnostic = expected.diagnostic();
        assert!(diagnostic.contains("blocked foreground turn"));
        assert!(diagnostic.contains("Blocked"));
        drop(expected);
        let (status, _) = tokio::time::timeout(Duration::from_secs(1), request)
            .await
            .expect("dropping handle releases blocked response");
        assert_eq!(status.as_u16(), 200);
    }

    #[tokio::test]
    async fn concurrent_retry_obeys_same_barrier_and_primary_owns_satisfaction() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut expected = server.expect_response_blocked(
            "blocked retry",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::sse(vec![SseEvent::data("chunk"), SseEvent::data("terminal")]),
        );
        let first = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "same-call",
            "same body",
        );
        tokio::pin!(first);
        tokio::select! {
            response = &mut first => panic!("primary completed before barrier: {:?}", response.0),
            _ = expected.wait_blocked() => {}
        }

        let retry = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "same-call",
            "same body",
        );
        tokio::pin!(retry);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut retry)
                .await
                .is_err(),
            "retry bypassed the shared release barrier"
        );
        assert!(!expected.is_satisfied());

        expected.release();
        let ((first_status, _), (retry_status, _)) = tokio::join!(first, retry);
        assert_eq!(first_status.as_u16(), 200);
        assert_eq!(retry_status.as_u16(), 200);
        expected.wait_satisfied().await;
    }

    #[tokio::test]
    async fn release_only_signals_and_late_blocked_waiters_still_succeed() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut expected = server.expect_response_blocked(
            "release ownership",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::json(200, json!({ "ok": true })),
        );
        let request = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "release-ownership",
            "hello",
        );
        tokio::pin!(request);
        tokio::select! {
            response = &mut request => panic!("response completed before barrier: {:?}", response.0),
            _ = expected.wait_blocked() => {}
        }

        expected.release();
        assert!(!expected.is_satisfied());
        let (status, _) = request.await;
        assert_eq!(status.as_u16(), 200);
        expected.wait_satisfied().await;
        expected.wait_blocked().await;
    }

    #[tokio::test]
    async fn overlapping_duplicate_replays_but_sequential_identical_request_claims_next() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut first = server.expect_response_blocked(
            "overlapping call",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::sse(vec![SseEvent::data("first"), SseEvent::data("terminal")]),
        );
        let mut second = server.expect_response(
            "later identical call",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::text(215, "second expectation"),
        );

        let primary = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "turn-id",
            "same body",
        );
        tokio::pin!(primary);
        tokio::select! {
            response = &mut primary => panic!("primary completed before barrier: {:?}", response.0),
            _ = first.wait_blocked() => {}
        }
        let replay = tokio::spawn({
            let url = endpoint_url(&server, InferenceEndpoint::Responses);
            let body = foreground_body(InferenceEndpoint::Responses, "same body");
            async move {
                let response = reqwest::Client::new()
                    .post(url)
                    .header("x-grok-req-id", "turn-id")
                    .header("x-grok-turn-idx", "1")
                    .json(&body)
                    .send()
                    .await
                    .expect("POST overlapping duplicate");
                let status = response.status();
                let body = response.text().await.expect("read overlapping duplicate");
                (status, body)
            }
        });
        first.wait_claims(2).await;
        assert!(
            !replay.is_finished(),
            "overlapping duplicate bypassed shared barrier"
        );
        first.release();
        let (primary_result, replay_result) = tokio::join!(primary, replay);
        let (primary_status, _) = primary_result;
        let (replay_status, _) = replay_result.expect("replay task");
        assert_eq!(primary_status.as_u16(), 200);
        assert_eq!(replay_status.as_u16(), 200);
        first.wait_satisfied().await;

        let (status, body) = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "turn-id",
            "same body",
        )
        .await;
        assert_eq!(status.as_u16(), 215);
        assert_eq!(body, "second expectation");
        second.wait_satisfied().await;
    }

    #[tokio::test]
    async fn changed_body_followup_claims_next_expectation() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut first = server.expect_response(
            "tool call",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::text(214, "tool-call-script"),
        );
        let mut followup = server.expect_response(
            "tool follow-up",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::text(215, "follow-up-script"),
        );
        let first_body = foreground_body(InferenceEndpoint::Responses, "run tool");
        let (status, body) =
            read_foreground_body(&server, InferenceEndpoint::Responses, "turn-id", first_body)
                .await;
        assert_eq!(status.as_u16(), 214);
        assert_eq!(body, "tool-call-script");
        first.wait_satisfied().await;

        let mut followup_body = foreground_body(InferenceEndpoint::Responses, "run tool");
        followup_body["input"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "type": "function_call_output", "call_id": "call_1", "output": "done" }));
        let (status, body) = read_foreground_body(
            &server,
            InferenceEndpoint::Responses,
            "turn-id",
            followup_body,
        )
        .await;
        assert_eq!(status.as_u16(), 215);
        assert_eq!(body, "follow-up-script");
        followup.wait_satisfied().await;
    }

    #[tokio::test]
    async fn cancelling_primary_cleans_up_without_satisfying_or_replaying() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut cancelled = server.expect_response_blocked(
            "cancel primary",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::sse(vec![SseEvent::data("chunk"), SseEvent::data("terminal")]),
        );
        let mut next = server.expect_response(
            "after cancellation",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::text(216, "next expectation"),
        );
        let request = Box::pin(read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "cancel-primary",
            "same body",
        ));
        let mut request = request;
        tokio::select! {
            response = &mut request => panic!("primary completed before cancellation: {:?}", response.0),
            _ = cancelled.wait_blocked() => {}
        }
        drop(request);
        assert!(!cancelled.is_satisfied());

        let (status, body) = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "cancel-primary",
            "same body",
        )
        .await;
        assert_eq!(status.as_u16(), 216);
        assert_eq!(body, "next expectation");
        next.wait_satisfied().await;
    }

    #[tokio::test]
    async fn cancelling_replay_waits_for_primary_before_satisfaction() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut expected = server.expect_response_blocked(
            "cancel replay",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::sse(vec![SseEvent::data("chunk"), SseEvent::data("terminal")]),
        );
        let primary = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "cancel-replay",
            "same body",
        );
        tokio::pin!(primary);
        tokio::select! {
            response = &mut primary => panic!("primary completed before barrier: {:?}", response.0),
            _ = expected.wait_blocked() => {}
        }

        let replay = tokio::spawn({
            let url = endpoint_url(&server, InferenceEndpoint::Responses);
            let body = foreground_body(InferenceEndpoint::Responses, "same body");
            async move {
                reqwest::Client::new()
                    .post(url)
                    .header("x-grok-req-id", "cancel-replay")
                    .header("x-grok-turn-idx", "1")
                    .json(&body)
                    .send()
                    .await
                    .expect("POST replay cancellation")
                    .text()
                    .await
                    .expect("read replay cancellation")
            }
        });
        expected.wait_claims(2).await;
        assert!(!replay.is_finished(), "replay bypassed shared barrier");
        replay.abort();
        let _ = replay.await;
        assert!(!expected.is_satisfied());

        expected.release();
        let (status, _) = primary.await;
        assert_eq!(status.as_u16(), 200);
        expected.wait_satisfied().await;
    }

    #[tokio::test]
    #[should_panic(expected = "duplicate inference expectation name `duplicate`")]
    async fn duplicate_expectation_names_are_rejected() {
        let server = MockInferenceServer::start().await.unwrap();
        let _first = server.expect_response(
            "duplicate",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::text(200, "first"),
        );
        let _second = server.expect_response(
            "duplicate",
            InferenceRequestMatcher::auxiliary(InferenceEndpoint::Responses),
            ScriptedResponse::text(200, "second"),
        );
    }

    #[tokio::test]
    async fn unsatisfied_expectation_diagnostic_includes_name_and_state() {
        let server = MockInferenceServer::start().await.unwrap();
        let expected = server.expect_response(
            "must receive a foreground turn",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::text(200, "unused"),
        );
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            expected.assert_satisfied();
        }))
        .expect_err("unsatisfied expectation must panic");
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default();
        assert!(message.contains("must receive a foreground turn"));
        assert!(message.contains("Pending"));
    }

    #[tokio::test]
    async fn echo_mode_echoes_last_user_message() {
        let server = MockInferenceServer::start().await.unwrap();

        let body = post_chat(&server, "ping pong").await.text().await.unwrap();
        assert_eq!(chat_stream_text(&body), "Echo: ping pong");

        // Echo mode keeps its historical whitespace-collapsing semantics.
        let body = post_chat(&server, "a  b\nc").await.text().await.unwrap();
        assert_eq!(chat_stream_text(&body), "Echo: a b c");
    }

    #[tokio::test]
    async fn fixed_mode_reconstructs_byte_exact_over_http() {
        let server = MockInferenceServer::start().await.unwrap();
        server.set_response(MERMAID_TEXT);

        let body = post_chat(&server, "ignored").await.text().await.unwrap();
        assert_eq!(chat_stream_text(&body), MERMAID_TEXT);

        let body = reqwest::Client::new()
            .post(format!("{}/responses", server.url()))
            .json(&json!({
                "model": "test-model",
                "input": [{ "role": "user", "content": "ignored" }]
            }))
            .send()
            .await
            .expect("POST /v1/responses")
            .text()
            .await
            .unwrap();
        assert_eq!(responses_stream_text(&body), MERMAID_TEXT);

        let body = reqwest::Client::new()
            .post(format!("{}/messages", server.url()))
            .json(&json!({
                "model": "test-model",
                "messages": [{ "role": "user", "content": "ignored" }]
            }))
            .send()
            .await
            .expect("POST /v1/messages")
            .text()
            .await
            .unwrap();
        assert_eq!(messages_stream_text(&body), MERMAID_TEXT);
    }

    #[tokio::test]
    async fn settings_404_until_set_then_200() {
        let server = MockInferenceServer::start().await.unwrap();
        let url = format!("{}/settings", server.url());

        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), 404);

        server.set_settings(json!({ "tips": ["t1"] }));
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body, json!({ "tips": ["t1"] }));

        server.preset_allow_access();
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body, json!({ "allow_access": true }));
    }

    #[tokio::test]
    async fn request_bodies_returns_bodies_in_arrival_order() {
        let server = MockInferenceServer::start().await.unwrap();

        post_chat(&server, "first").await.text().await.unwrap();
        // Body-less request in between must be skipped, not break ordering.
        reqwest::get(format!("{}/models", server.url()))
            .await
            .unwrap();
        post_chat(&server, "second").await.text().await.unwrap();

        let bodies = server.request_bodies();
        assert_eq!(bodies.len(), 2);
        assert_eq!(
            bodies[0]["messages"][0]["content"],
            json!("first"),
            "bodies must be in arrival order"
        );
        assert_eq!(bodies[1]["messages"][0]["content"], json!("second"));
    }

    #[tokio::test]
    async fn scripted_responses_serve_fifo_per_path_then_fall_back() {
        let server = MockInferenceServer::start().await.unwrap();
        server.enqueue_response(
            "/v1/chat/completions",
            ScriptedResponse::text(401, "Unauthorized"),
        );
        server.enqueue_response(
            "/v1/chat/completions",
            ScriptedResponse::json(500, json!({ "error": { "message": "boom" } })),
        );

        // FIFO: first the 401 text, then the 500 json.
        let resp = post_chat(&server, "hi").await;
        assert_eq!(resp.status(), 401);
        assert_eq!(resp.text().await.unwrap(), "Unauthorized");

        let resp = post_chat(&server, "hi").await;
        assert_eq!(resp.status(), 500);
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body, json!({ "error": { "message": "boom" } }));

        // Queue drained: falls back to the active mode (echo).
        let body = post_chat(&server, "ping pong").await.text().await.unwrap();
        assert_eq!(chat_stream_text(&body), "Echo: ping pong");

        // Queues are per path: an unrelated endpoint is unaffected.
        server.enqueue_response("/v1/chat/completions", ScriptedResponse::text(503, "later"));
        let resp = reqwest::Client::new()
            .post(format!("{}/responses", server.url()))
            .json(&json!({
                "model": "test-model",
                "input": [{ "role": "user", "content": "hi there" }]
            }))
            .send()
            .await
            .expect("POST /v1/responses");
        assert_eq!(resp.status(), 200);
        assert_eq!(
            responses_stream_text(&resp.text().await.unwrap()),
            "Echo: hi there "
        );
    }

    /// Pins the documented precedence: a script bypasses the required-auth
    /// gate; once the queue empties, the gate is back.
    #[tokio::test]
    async fn scripted_response_takes_precedence_over_required_auth() {
        let server = MockInferenceServer::start_with_required_auth(
            vec![MockModelEntry::new("test-model")],
            "secret-token",
        )
        .await
        .unwrap();
        server.enqueue_response(
            "/v1/chat/completions",
            ScriptedResponse::text(200, "scripted"),
        );

        // No token: the script still serves.
        let resp = post_chat(&server, "hi").await;
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "scripted");

        // Queue drained: the auth gate applies again.
        let resp = post_chat(&server, "hi").await;
        assert_eq!(resp.status(), 401);
    }

    /// Scripted response headers reach the client (the phase-2 script format's
    /// named consumer: 429 + Retry-After error injection).
    #[tokio::test]
    async fn scripted_response_headers_reach_the_client() {
        let server = MockInferenceServer::start().await.unwrap();
        let mut rate_limited = ScriptedResponse::text(429, "slow down");
        rate_limited
            .headers
            .push(("retry-after".to_string(), "7".to_string()));
        server.enqueue_response("/v1/chat/completions", rate_limited);

        let resp = post_chat(&server, "hi").await;
        assert_eq!(resp.status(), 429);
        assert_eq!(
            resp.headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok()),
            Some("7")
        );
        assert_eq!(resp.text().await.unwrap(), "slow down");
    }

    #[tokio::test]
    async fn scripted_raw_body_served_byte_exact() {
        let server = MockInferenceServer::start().await.unwrap();
        let raw = "data: {\"choices\":[]}\n\ndata: not-json-at-all\n\ndata: [DONE]\n\n";
        server.enqueue_response("/v1/chat/completions", ScriptedResponse::text(200, raw));

        let resp = post_chat(&server, "hi").await;
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), raw);
    }

    #[tokio::test]
    async fn scripted_sse_preserves_event_names_and_order() {
        let server = MockInferenceServer::start().await.unwrap();
        server.enqueue_response(
            "/v1/chat/completions",
            ScriptedResponse::sse(vec![
                SseEvent::with_event("custom.kind", "{\"a\":1}"),
                SseEvent::data("{\"b\":2}"),
            ]),
        );

        let body = post_chat(&server, "hi").await.text().await.unwrap();
        let named_then_plain = body
            .find("event: custom.kind")
            .zip(body.find("data: {\"b\":2}"))
            .is_some_and(|(named, plain)| named < plain);
        assert!(
            body.contains("event: custom.kind") && body.contains("data: {\"a\":1}"),
            "named event must carry both fields, got:\n{body}"
        );
        assert!(named_then_plain, "events must be served in order:\n{body}");
    }

    #[tokio::test]
    async fn matched_barriers_cover_all_endpoints_and_body_modes() {
        for endpoint in [
            InferenceEndpoint::ChatCompletions,
            InferenceEndpoint::Responses,
            InferenceEndpoint::Messages,
        ] {
            for (label, response) in [
                (
                    "sse",
                    ScriptedResponse::sse(vec![
                        SseEvent::data("chunk"),
                        SseEvent::data("terminal"),
                    ]),
                ),
                ("empty-sse", ScriptedResponse::sse(Vec::new())),
                ("json", ScriptedResponse::json(200, json!({ "ok": true }))),
                ("raw", ScriptedResponse::text(200, "raw body")),
            ] {
                let server = MockInferenceServer::start().await.unwrap();
                let mut expected = server.expect_response_blocked(
                    format!("blocked {endpoint:?} {label}"),
                    InferenceRequestMatcher::foreground(endpoint),
                    response,
                );
                let request = read_foreground(&server, endpoint, "body-gate", "hello");
                tokio::pin!(request);
                tokio::select! {
                    response = &mut request => panic!("{endpoint:?}/{label} completed before release: {:?}", response.0),
                    _ = expected.wait_blocked() => {}
                }
                expected.release();
                let (status, _) = tokio::time::timeout(Duration::from_secs(1), request)
                    .await
                    .unwrap_or_else(|_| {
                        panic!("{endpoint:?}/{label} did not complete after release")
                    });
                assert_eq!(status.as_u16(), 200);
                expected.wait_satisfied().await;
            }
        }
    }

    #[tokio::test]
    async fn matched_expectation_precedes_auth_then_auth_resumes() {
        let server = MockInferenceServer::start_with_required_auth(
            vec![MockModelEntry::new("test-model")],
            "secret-token",
        )
        .await
        .unwrap();
        let mut expected = server.expect_response(
            "auth bypass",
            InferenceRequestMatcher::foreground(InferenceEndpoint::Responses),
            ScriptedResponse::text(218, "matched without auth"),
        );

        let response = post_foreground(
            &server,
            InferenceEndpoint::Responses,
            "auth-bypass",
            "first",
        )
        .await;
        assert_eq!(response.status().as_u16(), 218);
        assert_eq!(response.text().await.unwrap(), "matched without auth");
        expected.wait_satisfied().await;

        let response = post_foreground(
            &server,
            InferenceEndpoint::Responses,
            "auth-fallback",
            "second",
        )
        .await;
        assert_eq!(response.status(), 401);
    }

    #[tokio::test]
    async fn compatibility_completion_gate_covers_fallback_and_scripted_sse() {
        for endpoint in [
            InferenceEndpoint::ChatCompletions,
            InferenceEndpoint::Responses,
            InferenceEndpoint::Messages,
        ] {
            let server = MockInferenceServer::start().await.unwrap();
            server.hold_agent_completions();
            server.set_agent_turns([format!("{endpoint:?} turn")]);
            let request = read_foreground(&server, endpoint, "global-gate", "hello");
            tokio::pin!(request);
            assert!(
                tokio::time::timeout(Duration::from_millis(50), &mut request)
                    .await
                    .is_err(),
                "{endpoint:?} bypassed the compatibility gate"
            );
            server.release_agent_completions();
            tokio::time::timeout(Duration::from_secs(1), request)
                .await
                .unwrap_or_else(|_| panic!("{endpoint:?} did not complete after release"));
        }

        let server = MockInferenceServer::start().await.unwrap();
        server.hold_agent_completions();
        server.enqueue_response(
            "/v1/responses",
            ScriptedResponse::sse(vec![SseEvent::data("chunk"), SseEvent::data("terminal")]),
        );
        let request = read_foreground(
            &server,
            InferenceEndpoint::Responses,
            "scripted-global-gate",
            "hello",
        );
        tokio::pin!(request);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut request)
                .await
                .is_err(),
            "scripted SSE bypassed the compatibility gate"
        );
        server.release_agent_completions();
        tokio::time::timeout(Duration::from_secs(1), request)
            .await
            .expect("scripted SSE completes after release");
    }

    #[tokio::test]
    async fn compatibility_completion_gate_does_not_hold_json_or_raw() {
        for response in [
            ScriptedResponse::json(219, json!({ "ok": true })),
            ScriptedResponse::text(220, "raw body"),
        ] {
            let server = MockInferenceServer::start().await.unwrap();
            server.hold_agent_completions();
            server.enqueue_response("/v1/responses", response);
            let (status, _) = tokio::time::timeout(
                Duration::from_secs(1),
                read_foreground(
                    &server,
                    InferenceEndpoint::Responses,
                    "compat-non-sse",
                    "hello",
                ),
            )
            .await
            .expect("compatibility JSON/raw must not wait for the SSE gate");
            assert!(matches!(status.as_u16(), 219 | 220));
        }
    }

    #[tokio::test]
    async fn request_log_captures_arbitrary_headers() {
        let server = MockInferenceServer::start().await.unwrap();

        reqwest::Client::new()
            .post(format!("{}/chat/completions", server.url()))
            .header("authorization", "Bearer log-me")
            .header("x-test-marker", "zap")
            .json(&json!({
                "model": "test-model",
                "messages": [{ "role": "user", "content": "hi" }]
            }))
            .send()
            .await
            .expect("POST /v1/chat/completions");

        let entry = server.requests().pop().expect("one logged request");
        assert_eq!(entry.header("x-test-marker"), Some("zap"));
        assert_eq!(entry.header("X-Test-Marker"), Some("zap"));
        assert_eq!(entry.header("authorization"), Some("Bearer log-me"));
        assert_eq!(entry.authorization.as_deref(), Some("Bearer log-me"));
        assert_eq!(entry.header("x-absent"), None);
    }

    #[tokio::test]
    async fn required_auth_enforced_in_both_response_modes() {
        let server = MockInferenceServer::start_with_required_auth(
            vec![MockModelEntry::new("test-model")],
            "secret-token",
        )
        .await
        .unwrap();
        let client = reqwest::Client::new();
        let url = format!("{}/chat/completions", server.url());
        let req_body = json!({
            "model": "test-model",
            "messages": [{ "role": "user", "content": "hi there" }]
        });

        // Echo mode: missing auth rejected, valid auth streams the echo.
        let resp = client.post(&url).json(&req_body).send().await.unwrap();
        assert_eq!(resp.status(), 401);
        let resp = client
            .post(&url)
            .header("authorization", "Bearer secret-token")
            .json(&req_body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(
            chat_stream_text(&resp.text().await.unwrap()),
            "Echo: hi there"
        );

        // Fixed mode: same auth gate, fixed text streamed byte-exact.
        server.set_response(MERMAID_TEXT);
        let resp = client.post(&url).json(&req_body).send().await.unwrap();
        assert_eq!(resp.status(), 401);
        let resp = client
            .post(&url)
            .header("authorization", "Bearer secret-token")
            .json(&req_body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(chat_stream_text(&resp.text().await.unwrap()), MERMAID_TEXT);
    }
}
>>>>>>> e3fdf3ed (Merge 2 (#4))
