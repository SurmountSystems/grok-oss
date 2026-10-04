//! Actor-based sampling layer for xAI grok.
//!
//! This crate holds the HTTP streaming and retry logic extracted from `xai-grok-shell`'s session actor.
//! It is built on the same actor pattern as `xai-hunk-tracker`.
//!
//! ## Layered API
//!
//! - **Layer 1**: [`client::SamplingClient`] returns raw chunk streams.
//! - **Layer 2**: [`stream`] transforms raw streams into [`SamplingEvent`]s.
//! - **Layer 3**: [`SamplerHandle`] manages concurrent requests with retry, cancellation, and event-based coordination via the actor.

#![deny(clippy::indexing_slicing)]

pub mod actor;
pub mod attribution;
pub mod client;
pub mod commands;
pub mod config;
pub mod doom_loop;
mod doom_loop_recovery;
pub mod events;
/// Process-local credit/allowance-exhausted credential fingerprints (dual-auth).
pub mod exhausted_identity;
pub mod handle;
pub mod metrics;
pub mod prefer_live_primary;
mod prewarm;
mod request_compression;
pub mod retry;
pub mod sampling_log;
mod shared_http;
mod span_timing;
pub mod stream;
mod stream_classify;
pub mod types;

// Public re-exports: the API consumers see
pub use actor::SamplerActor;
pub use actor::request_task::CompletionResult;
pub use attribution::{
    Auth401AttributionCallback, BEARER_SUFFIX_LEN, SamplingConsumer, SharedAttributionCallback,
};
pub use client::{ApiBackend, SamplingClient, user_agent_string_for};
pub use config::{
    AuthScheme, BearerResolver, HeaderInjector, OriginClientInfo, RequestCompression, RetryPolicy,
    SamplerConfig, SharedBearerResolver, SharedHeaderInjector,
};
pub use doom_loop::DoomLoopSignalCollector;
pub use events::{
    SamplingChannel, SamplingErrorInfo, SamplingErrorKind, SamplingEvent, StripReason,
};
pub use handle::{CollectedSamplingResult, DoomLoopRecoveryAttempt, SamplerHandle};
pub use metrics::{InferenceLatencyStats, compute_percentiles};
pub use prefer_live_primary::{
    both_included_session_and_console_key_refused,
    ensure_supergrok_recovery_after_console_credit_exhaust,
    prefer_console_identity_for_use_console_pin, prefer_live_identity_after_credit_exhaust,
    prefer_supergrok_identity_for_stay_pin, primary_is_memoized_credit_exhausted,
    prune_exhausted_failover_candidates, rotate_sampling_client_after_credit_exhaust,
    withhold_model_request_when_both_refused,
};
pub use prewarm::{PrewarmOutcome, PrewarmReport, prewarm_transport};
pub use retry::{
    DEFAULT_MAX_RETRIES, MAX_RETRY_BACKOFF, RATE_LIMIT_RETRY_DISABLED, RATE_LIMIT_RETRY_THRESHOLD,
    RetryDecision, classify_error, format_sampling_error, jitter_backoff, resolve_max_retries,
    retry_after_or_backoff, retry_backoff_with_jitter,
};
pub use sampling_log::AuthInfo;
pub use stream::{
    collect_response, first_token_wait, stream_chat_completions, stream_messages, stream_responses,
};
pub use types::RequestId;
pub use xai_grok_sampling_types::ConversationGroupId;
