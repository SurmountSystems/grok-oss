//! xai-grok-sampler - Actor-based sampling layer for xAI grok.
//!
//! This crate extracts the HTTP streaming + retry logic out of
//! `xai-grok-shell`'s session actor into a standalone, reusable
//! component built on the same actor pattern as `xai-hunk-tracker`.
//!
//! ## Layered API
//!
//! - **Layer 1**: [`client::SamplingClient`] returns raw chunk streams.
//! - **Layer 2**: [`stream`] transforms raw streams into [`SamplingEvent`]s.
//! - **Layer 3**: [`SamplerHandle`] manages concurrent requests with retry,
//!   cancellation, and event-based coordination via the actor.
//!
//! The type skeleton, the pure retry / metrics / client logic, the
//! Layer-2 stream transforms ([`stream_chat_completions`],
//! [`stream_responses`], [`stream_messages`], [`collect_response`]),
//! and the actor with its per-request task tie these layers together.

pub mod actor;
pub mod attribution;
pub mod client;
pub mod commands;
pub mod config;
pub mod doom_loop;
pub mod events;
/// Process-local credit/allowance-exhausted credential fingerprints (dual-auth).
pub mod exhausted_identity;
pub mod handle;
pub mod metrics;
/// Prefer a live dual-auth identity after credit/allowance exhaust (pre-request).
pub mod prefer_live_primary;
pub mod retry;
pub mod sampling_log;
mod shared_http;
pub mod stream;
pub mod types;

// Public re-exports — the API surface consumers see.
pub use actor::SamplerActor;
pub use attribution::{
    Auth401AttributionCallback, SENT_BEARER_PREFIX_LEN, SamplingConsumer, SharedAttributionCallback,
};
pub use client::{ApiBackend, SamplingClient, user_agent_string_for};
pub use config::{
    AuthScheme, BearerResolver, HeaderInjector, OriginClientInfo, RetryPolicy, SamplerConfig,
    SharedBearerResolver, SharedHeaderInjector,
};
pub use doom_loop::DoomLoopSignalCollector;
pub use events::{SamplingChannel, SamplingErrorInfo, SamplingErrorKind, SamplingEvent};
pub use exhausted_identity::{
    AllowanceExhaustAction, CredentialLabel, HopCause, INCLUDED_ALLOWANCE_EXHAUST_PCT, clear_all,
    clear_all_including_durable, clear_exhausted, format_credential_hop_reason, format_hop_reason,
    format_rate_limit_hop_reason, is_credential_exhausted, is_credential_hop_reason, is_exhausted,
    mark_exhausted, sync_allowance_exhaust_from_usage,
};
pub use handle::SamplerHandle;
pub use metrics::{InferenceLatencyStats, compute_percentiles};
pub use prefer_live_primary::{
    ensure_supergrok_recovery_after_console_credit_exhaust,
    prefer_live_identity_after_credit_exhaust, primary_is_memoized_credit_exhausted,
    prune_exhausted_failover_candidates,
};
pub use retry::{
    DEFAULT_MAX_RETRIES, MAX_BACKOFF_SECS, RATE_LIMIT_RETRY_THRESHOLD, RetryDecision,
    classify_error, format_sampling_error, is_unlimited_retries, resolve_max_retries,
    retry_backoff_with_jitter,
};
pub use sampling_log::AuthInfo;
pub use stream::{collect_response, stream_chat_completions, stream_messages, stream_responses};
pub use types::RequestId;
