//! Sampler configuration types.
//!
//! [`SamplerConfig`] is the per-request configuration handed to the
//! sampler. It deliberately does **not** alias
//! `xai_grok_sampling_types::SamplingConfig` so that the sampler crate
//! avoids transitive dependencies on shell-specific types
//! (`xai-grok-tools`, etc.).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use xai_grok_sampling_types::{
    ApiBackend, CompactionAtTokens, CompactionsRemaining, DoomLoopRecoveryPolicy, ReasoningEffort,
};

use crate::attribution::SharedAttributionCallback;
use crate::retry::{DEFAULT_MAX_RETRIES, RATE_LIMIT_RETRY_THRESHOLD};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    #[default]
    Bearer,
    XApiKey,
}

/// All knobs that control a single sampling request.
///
/// The session typically owns one `SamplerConfig` per active model
/// and passes it (or a per-request override) to the actor on every
/// submit.
///
/// # Construction in `xai-grok-shell`
///
/// `SamplerConfig` is the single source of truth for sampler
/// configuration. The shell builds it directly (see
/// `agent::config::resolve_model_to_sampling_config` and
/// `session::acp_session::SessionActor::reconstruct_full_config`) by
/// composing chat-state's `xai_grok_sampling_types::SamplingConfig`
/// with `Credentials` (api key, client version).
///
/// URL-derived request headers (e.g. `X-XAI-Token-Auth` for the
/// cli-chat-proxy) are
/// folded into [`Self::extra_headers`] by
/// `agent::config::inject_url_derived_headers` before the
/// `SamplerConfig` is handed to the actor. Auth is selected separately
/// via `auth_scheme`, while `api_backend` controls only the request/response
/// protocol shape.
/// Per-request sampler configuration.
///
/// [`Debug`] redacts API keys / session identity so logs never dump secrets.
#[derive(Clone, Serialize, Deserialize)]
pub struct SamplerConfig {
    pub api_key: Option<String>,
    /// Additional API keys tried when the active key hits a credit /
    /// spending-limit error ([`xai_grok_sampling_types::SamplingError::is_credit_exhausted`])
    /// **or** a plain HTTP 429 ([`xai_grok_sampling_types::SamplingError::is_rate_limited`]).
    /// Order is preference; keys already equal to `api_key` are ignored.
    /// Empty (default) disables multi-key failover. Credit hops sticky-memo the
    /// dead identity (~1h process-local); rate-limit hops use temporary shared
    /// cooldown only (return to primary when cool).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failover_api_keys: Vec<String>,
    /// When set, identity hop to a **non-session** failover key also switches
    /// [`Self::base_url`] to this host (console / Business API vs session proxy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failover_base_url: Option<String>,
    /// Session host restored when hopping to [`Self::session_identity_key`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_base_url: Option<String>,
    /// Exact token that marks the SuperGrok / OAuth session identity in the
    /// failover list (or primary). Used when switching API host with the key
    /// (SuperGrok proxy ↔ `api.x.ai`) and for bearer reinstall.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_identity_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub max_completion_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub api_backend: ApiBackend,
    #[serde(default)]
    pub auth_scheme: AuthScheme,
    /// Extra request headers applied verbatim. The sampler never inspects
    /// the URL to derive headers; callers (the session) inject proxy auth
    /// and other access headers here before constructing the config.
    pub extra_headers: IndexMap<String, String>,
    /// Query parameters folded into every request URL (percent-encoded).
    #[serde(default)]
    pub query_params: IndexMap<String, String>,
    /// Header name to environment variable, resolved into request headers at
    /// client build and never persisted.
    #[serde(default)]
    pub env_http_headers: IndexMap<String, String>,
    /// Total context window size in tokens. The sampler does not enforce
    /// it; it is informational metadata used by the session for compaction
    /// decisions.
    pub context_window: u64,
    pub force_http1: bool,
    pub max_retries: Option<u32>,
    pub stream_tool_calls: bool,
    pub idle_timeout_secs: Option<u64>,

    // Reasoning effort
    pub reasoning_effort: Option<ReasoningEffort>,

    // Client identity
    pub origin_client: Option<OriginClientInfo>,
    pub client_identifier: Option<String>,
    pub deployment_id: Option<String>,
    pub user_id: Option<String>,
    pub client_version: Option<String>,

    /// Optional hook invoked at every UNAUTHORIZED (401) response
    /// site. The sampler passes the bearer that was actually sent on
    /// the wire to the callback; the implementation is free to do
    /// whatever it wants with it (typically: join it with a live
    /// credential source and emit an attribution event for diagnosis
    /// of stale-token vs. server-rejected-live-token 401s). `None`
    /// (default) is a no-op -- the 401 arm returns the same
    /// `SamplingError::Auth` it always did.
    ///
    /// `Arc<dyn Trait>` is not serializable, so the field is skipped
    /// in (de)serialization. Round-tripping a config through serde
    /// drops the callback; callers that deserialize a `SamplerConfig`
    /// from disk must re-attach the callback before passing it to
    /// [`crate::SamplingClient::new`] or 401 attribution will be
    /// silently disabled for the rebuilt client.
    #[serde(skip)]
    pub attribution_callback: Option<SharedAttributionCallback>,

    /// Live bearer resolve per request. `None` uses construction-time `api_key`.
    #[serde(skip)]
    pub bearer_resolver: Option<SharedBearerResolver>,

    /// Stashed live resolver after hop-away-from-session; reinstalled on hop-to-session.
    #[serde(skip)]
    pub stashed_bearer_resolver: Option<SharedBearerResolver>,

    /// Durable session live resolver for hop-to-session **without** a prior stash
    /// (key-primary dual-auth mid-hop, or next-turn re-resolve). Shell wires
    /// `AuthManager` here; hop-to-session prefers stash, then this field.
    /// Not cleared when hopping session→key.
    #[serde(skip)]
    pub session_bearer_resolver: Option<SharedBearerResolver>,

    #[serde(default)]
    pub supports_backend_search: bool,

    /// Per-model config for the `x-compactions-remaining` header; `None` disables it.
    #[serde(default)]
    pub compactions_remaining: Option<CompactionsRemaining>,

    /// Per-model config for the `x-compaction-at` header; `None` disables it.
    #[serde(default)]
    pub compaction_at_tokens: Option<CompactionAtTokens>,

    /// Server-side doom-loop check policy; `None` disables it. When set, the
    /// client itself sends the opt-in `x-grok-doom-loop-check` header on
    /// streaming Responses API requests and absorbs the reported trigger
    /// events (unlike the environment headers in [`Self::extra_headers`],
    /// this header gates the client's own decode behavior, so it lives with
    /// the decoder).
    #[serde(default)]
    pub doom_loop_recovery: Option<DoomLoopRecoveryPolicy>,

    /// Per-request header injector (e.g. OTel traceparent). Called in `post()`.
    #[serde(skip)]
    pub header_injector: Option<SharedHeaderInjector>,
}

/// Debug helper: show map keys, redact values (auth headers / query secrets).
struct RedactedStrMap<'a> {
    map: &'a IndexMap<String, String>,
}

impl std::fmt::Debug for RedactedStrMap<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_map();
        for k in self.map.keys() {
            d.entry(k, &"<redacted>");
        }
        d.finish()
    }
}

impl std::fmt::Debug for SamplerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplerConfig")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field(
                "failover_api_keys",
                &format_args!("[{} redacted]", self.failover_api_keys.len()),
            )
            .field("failover_base_url", &self.failover_base_url)
            .field("session_base_url", &self.session_base_url)
            .field(
                "session_identity_key",
                &self.session_identity_key.as_ref().map(|_| "<redacted>"),
            )
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("max_completion_tokens", &self.max_completion_tokens)
            .field("temperature", &self.temperature)
            .field("top_p", &self.top_p)
            .field("api_backend", &self.api_backend)
            .field("auth_scheme", &self.auth_scheme)
            .field(
                "extra_headers",
                &RedactedStrMap {
                    map: &self.extra_headers,
                },
            )
            .field(
                "query_params",
                &RedactedStrMap {
                    map: &self.query_params,
                },
            )
            .field(
                "env_http_headers",
                &RedactedStrMap {
                    map: &self.env_http_headers,
                },
            )
            .field("context_window", &self.context_window)
            .field("force_http1", &self.force_http1)
            .field("max_retries", &self.max_retries)
            .field("stream_tool_calls", &self.stream_tool_calls)
            .field("idle_timeout_secs", &self.idle_timeout_secs)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("origin_client", &self.origin_client)
            .field("client_identifier", &self.client_identifier)
            .field("deployment_id", &self.deployment_id)
            .field("user_id", &self.user_id)
            .field("client_version", &self.client_version)
            .field(
                "attribution_callback",
                &self.attribution_callback.as_ref().map(|_| "<callback>"),
            )
            .field(
                "bearer_resolver",
                &self.bearer_resolver.as_ref().map(|_| "<resolver>"),
            )
            .field(
                "stashed_bearer_resolver",
                &self.stashed_bearer_resolver.as_ref().map(|_| "<resolver>"),
            )
            .field(
                "session_bearer_resolver",
                &self.session_bearer_resolver.as_ref().map(|_| "<resolver>"),
            )
            .field("supports_backend_search", &self.supports_backend_search)
            .field("compactions_remaining", &self.compactions_remaining)
            .field("compaction_at_tokens", &self.compaction_at_tokens)
            .field("doom_loop_recovery", &self.doom_loop_recovery)
            .field(
                "header_injector",
                &self.header_injector.as_ref().map(|_| "<injector>"),
            )
            .finish()
    }
}

impl Default for SamplerConfig {
    /// Empty defaults so callers can use `..Default::default()` and
    /// new fields don't ripple through every literal site.
    fn default() -> Self {
        Self {
            api_key: None,
            failover_api_keys: Vec::new(),
            failover_base_url: None,
            session_base_url: None,
            session_identity_key: None,
            base_url: String::new(),
            model: String::new(),
            max_completion_tokens: None,
            temperature: None,
            top_p: None,
            api_backend: ApiBackend::default(),
            auth_scheme: AuthScheme::default(),
            extra_headers: IndexMap::new(),
            query_params: IndexMap::new(),
            env_http_headers: IndexMap::new(),
            context_window: 0,
            force_http1: false,
            max_retries: None,
            stream_tool_calls: false,
            idle_timeout_secs: None,
            reasoning_effort: None,
            origin_client: None,
            client_identifier: None,
            deployment_id: None,
            user_id: None,
            client_version: None,
            attribution_callback: None,
            bearer_resolver: None,
            stashed_bearer_resolver: None,
            session_bearer_resolver: None,
            supports_backend_search: false,
            compactions_remaining: None,
            compaction_at_tokens: None,
            doom_loop_recovery: None,
            header_injector: None,
        }
    }
}

/// Cheap sync read of the current bearer for [`SamplerConfig::bearer_resolver`].
pub trait BearerResolver: Send + Sync + std::fmt::Debug {
    fn current_bearer(&self) -> Option<String>;
}

pub type SharedBearerResolver = std::sync::Arc<dyn BearerResolver>;

/// Per-request header injection (e.g. OTel `traceparent`).
pub trait HeaderInjector: Send + Sync + std::fmt::Debug {
    fn inject(&self, headers: &mut reqwest::header::HeaderMap);
}

pub type SharedHeaderInjector = std::sync::Arc<dyn HeaderInjector>;

/// Retry knobs for the sampler's internal transport-error retry loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retries before giving up.
    pub max_retries: u32,
    /// After this many rate-limit (429) retries, escalate to the caller.
    /// Lower than `max_retries` because rate-limit waits can be long.
    pub rate_limit_retry_threshold: u32,
    #[serde(default)]
    pub retry_only_before_output: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            rate_limit_retry_threshold: RATE_LIMIT_RETRY_THRESHOLD,
            retry_only_before_output: false,
        }
    }
}

/// Identity of the client that originated the request, used for
/// User-Agent rendering. The shell layer composes this with platform
/// info into a final UA string.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OriginClientInfo {
    pub product: String,
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_defaults() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(
            policy.rate_limit_retry_threshold,
            RATE_LIMIT_RETRY_THRESHOLD
        );
    }

    /// Configs serialized before the field existed must keep deserializing.
    #[test]
    fn config_without_doom_loop_recovery_deserializes_to_none() {
        let mut stripped = serde_json::to_value(SamplerConfig::default()).unwrap();
        stripped
            .as_object_mut()
            .unwrap()
            .remove("doom_loop_recovery");
        let config: SamplerConfig = serde_json::from_value(stripped).unwrap();
        assert!(config.doom_loop_recovery.is_none());

        let with_policy = SamplerConfig {
            doom_loop_recovery: Some(DoomLoopRecoveryPolicy {
                max_threshold: 8,
                max_retries: 2,
            }),
            ..Default::default()
        };
        let round_tripped: SamplerConfig =
            serde_json::from_value(serde_json::to_value(&with_policy).unwrap()).unwrap();
        assert_eq!(
            round_tripped.doom_loop_recovery,
            with_policy.doom_loop_recovery
        );
    }
}
