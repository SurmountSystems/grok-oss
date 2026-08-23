//! Retry classification, backoff, and decision-making.
//!
//! Pure logic only: no I/O, no notifications, no logging side-effects.
//! The actor (M4) wraps this with the actual retry loop.
//!
//! # Retry behavior summary
//!
//! **Retried** with a small under-window transport budget
//! ([`DEFAULT_TRANSPORT_MAX_RETRIES`]; set `GROK_MAX_RETRIES` to a finite
//! value to choose a different cap):
//! - 500, 502, 503, 504 and Cloudflare edge 52x (520–527, 530) outages
//! - Connection errors (timeout, refused, reset)
//! - `EventStreamError` / `StreamError` (mid-stream / first-token / headers)
//! - `EmptyResponse` (model returned no content/tool calls)
//!
//! **Retried** until success (default budget: [`DEFAULT_MAX_RETRIES`] =
//! [`u32::MAX`] — effectively unlimited; set `GROK_MAX_RETRIES` to cap):
//! - 429 (rate limited) — honors `Retry-After`, else exponential backoff
//!
//! **Special handling**:
//! - 413 / image processing errors → strip images and retry. Debits the
//!   retry budget but is never blocked by it; a repeat with nothing left to
//!   strip is fatal, so at most one strip cycle per request.
//!
//! **Not retried** (Fatal immediately — not fixable by waiting):
//! - 400, 401, 403, 404, 408, 422 (client errors)
//! - `Auth` / `InvalidConfiguration` (credential/config issues)
//! - `IdleTimeout` (model stuck, retry would stall again)
//! - `Serialization` (response parsing failure)
//! - `MaxTokensTruncation` (by design)
//!
//! **Server hint** (`x-should-retry` header from CCP):
//! - `false` → Fatal immediately, regardless of status code
//! - `true` / absent → falls through to status-code logic above
//!
//! Backoff: exponential 2s → 4s → 8s → … capped at
//! [`MAX_BACKOFF`] (60s) with ±20% jitter. Rate-limit retries stay
//! unlimited by default. Transport / 5xx / timeout use a small budget.

use std::time::Duration;

use xai_grok_sampling_types::{
    SamplingError, is_edge_outage_status, outage_exhausted_user_message,
};

/// Legacy name: rate-limit retries used to stop early. With unlimited
/// defaults this matches [`DEFAULT_MAX_RETRIES`] so 429s keep retrying.
pub const RATE_LIMIT_RETRY_THRESHOLD: u32 = u32::MAX;

/// Default max retries: effectively unlimited for **rate limits** (429).
/// Transient 5xx / timeout / stream interrupts use
/// [`DEFAULT_TRANSPORT_MAX_RETRIES`] when this value is unlimited.
/// Override with `GROK_MAX_RETRIES` or per-model config.
pub const DEFAULT_MAX_RETRIES: u32 = u32::MAX;

/// Small under-window budget for 5xx, timeouts, and stream interrupts when
/// [`DEFAULT_MAX_RETRIES`] is unlimited. After this many retries the
/// sampler fails with the named error instead of sitting on reconnect
/// forever. Finite `GROK_MAX_RETRIES` still wins as the cap.
pub const DEFAULT_TRANSPORT_MAX_RETRIES: u32 = 3;

fn transport_retry_cap(max_retries: u32) -> u32 {
    if is_unlimited_retries(max_retries) {
        DEFAULT_TRANSPORT_MAX_RETRIES
    } else {
        max_retries
    }
}

/// Ceiling for **jittered exponential** backoff between retries when the
/// server did **not** send `Retry-After`. Shared / server-driven waits are
/// uncapped by default (see `grok-rate-limit` and `extract_retry_after`).
///
/// Override with `GROK_MAX_BACKOFF_SECS` if needed; `0` or unset uses this default.
pub const MAX_BACKOFF_SECS: u64 = 60;

fn max_backoff_secs() -> u64 {
    std::env::var("GROK_MAX_BACKOFF_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(MAX_BACKOFF_SECS)
}

/// Resolve max API retries from an optional env override, model config,
/// or default ([`DEFAULT_MAX_RETRIES`]).
pub(crate) fn resolve_max_retries_with_env(
    env_override: Option<&str>,
    model_max_retries: Option<u32>,
) -> u32 {
    env_override
        .and_then(|value| value.parse::<u32>().ok())
        .or(model_max_retries)
        .unwrap_or(DEFAULT_MAX_RETRIES)
}

/// Resolve max API retries: `GROK_MAX_RETRIES` env > model config > default ([`DEFAULT_MAX_RETRIES`]).
pub fn resolve_max_retries(model_max_retries: Option<u32>) -> u32 {
    let env_override = std::env::var("GROK_MAX_RETRIES").ok();
    resolve_max_retries_with_env(env_override.as_deref(), model_max_retries)
}

/// Whether the retry budget is treated as unlimited (no hard stop).
pub fn is_unlimited_retries(max_retries: u32) -> bool {
    max_retries == u32::MAX
}

/// Backoff for doom-loop resamples: near-immediate with a small jitter.
/// Loops are stochastic at sampling temperature, so a fresh sample is the
/// remedy — waiting buys nothing beyond de-syncing concurrent resamples.
pub fn doom_loop_backoff(retry_count: u32) -> Duration {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};

    static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

    let mut hasher = std::hash::DefaultHasher::new();
    JITTER_SEQ.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    retry_count.hash(&mut hasher);
    Duration::from_millis(hasher.finish() % 251)
}

/// Exponential backoff (2s, 4s, 8s, ..., capped at [`MAX_BACKOFF_SECS`])
/// with +/-20% jitter to prevent thundering-herd retry storms.
pub fn retry_backoff_with_jitter(retry_count: u32) -> Duration {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};

    static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

    let shift = retry_count.saturating_sub(1);
    let cap_ms = max_backoff_secs().saturating_mul(1000);
    let base_ms = 2000u64.checked_shl(shift).unwrap_or(u64::MAX).min(cap_ms);
    let jitter_range = base_ms / 5;
    let mut hasher = std::hash::DefaultHasher::new();
    JITTER_SEQ.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let jitter = hasher.finish() % (jitter_range * 2 + 1);
    Duration::from_millis(base_ms - jitter_range + jitter)
}

/// +/-20% jitter around a fixed wait (generic Retry-After clamp path).
fn jitter_around(base: Duration) -> Duration {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};

    static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

    let base_ms = base.as_millis() as u64;
    if base_ms == 0 {
        return base;
    }
    let jitter_range = base_ms / 5;
    let mut hasher = std::hash::DefaultHasher::new();
    JITTER_SEQ.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let jitter = hasher.finish() % (jitter_range * 2 + 1);
    Duration::from_millis(base_ms - jitter_range + jitter)
}

/// What the actor should do next given a sampling error and retry context.
///
/// Pure data: callers (the actor's per-request task) are responsible for
/// performing the actual sleep, image strip, client rebuild, or emit.
#[derive(Debug)]
pub enum RetryDecision {
    /// Retry with exponential backoff (transport errors, 5xx,
    /// empty responses).
    Retry { backoff: Duration },

    /// Retry honoring the server's `Retry-After` header (429 rate
    /// limits). `is_rate_limited` distinguishes 429s from generic
    /// retry-with-backoff cases for telemetry.
    RetryWithBackoff {
        backoff: Duration,
        is_rate_limited: bool,
    },

    /// Retry after stripping inline images from the request (413
    /// Payload Too Large or image processing rejection).
    RetryWithImageStrip,

    /// Retry after rebuilding the HTTP client with HTTP/1.1 (transport
    /// error, first retry only).
    RetryWithClientRebuild { backoff: Duration },

    /// Emit the error to the session and let it decide what to do
    /// (auth refresh, encrypted-content mismatch).
    EmitToSession(SamplingError),

    /// Fatal: no further retries possible. Surface to the caller as the
    /// final outcome of the sampling request.
    Fatal(SamplingError),
}

/// True when used tokens are at or over this session's sampling window.
///
/// The same oversized payload cannot recover by retrying. Session compact
/// (L1/L2) or fail honestly. L3 never compact.
pub fn over_window_blocks_retry(used_tokens: Option<u64>, sampling_window: u64) -> bool {
    sampling_window > 0 && used_tokens.is_some_and(|used| used >= sampling_window)
}

/// Classify a sampling error, refusing retries when the request is already
/// at or over the session sampling window.
///
/// Auth and encrypted-content errors still emit to the session. Image-strip
/// recovery may still change the payload. Doom-loop resample is not a size
/// overflow.
pub fn classify_error_with_window(
    err: &SamplingError,
    retry_count: u32,
    max_retries: u32,
    rate_limit_threshold: u32,
    estimated_input_tokens: Option<u64>,
    sampling_window: u64,
) -> RetryDecision {
    if over_window_blocks_retry(estimated_input_tokens, sampling_window)
        && err.is_retryable()
        && !err.is_payload_too_large()
        && !err.is_image_processing_error()
        && !matches!(err, SamplingError::DoomLoopDetected { .. })
        && !err.is_auth_error()
        && !err.is_encrypted_content_error()
    {
        return RetryDecision::Fatal(clone_error(err));
    }
    classify_error(err, retry_count, max_retries, rate_limit_threshold)
}

/// Classify a sampling error into a [`RetryDecision`].
///
/// `retry_count` is the number of retries already performed (0 on first
/// failure). `max_retries` is the total budget. `rate_limit_threshold`
/// caps consecutive 429 retries (see [`RATE_LIMIT_RETRY_THRESHOLD`]).
///
/// The function is pure: it does not sleep, log, or perform I/O.
pub fn classify_error(
    err: &SamplingError,
    retry_count: u32,
    max_retries: u32,
    rate_limit_threshold: u32,
) -> RetryDecision {
    // Auth and encrypted-content errors are session-owned. The sampler
    // surfaces the raw error and lets the session refresh credentials
    // or show a friendly message.
    if err.is_auth_error() {
        return RetryDecision::EmitToSession(clone_error(err));
    }
    if err.is_encrypted_content_error() {
        return RetryDecision::EmitToSession(clone_error(err));
    }
    if max_retries == 0 {
        return RetryDecision::Fatal(clone_error(err));
    }

    // 413 Payload Too Large: strip inline images and try once. The
    // caller checks if there are images left after the strip; if not,
    // upgrade to Fatal.
    if err.is_payload_too_large() {
        return RetryDecision::RetryWithImageStrip;
    }

    // Image processing errors (direct 400, proxy-wrapped 500, or mid-stream
    // SSE error): strip images and retry, same recovery as 413.
    if err.is_image_processing_error() {
        return RetryDecision::RetryWithImageStrip;
    }

    // Server explicitly said don't retry (x-should-retry: false).
    // Trust the server — it knows if the error is request-content-caused
    // (e.g. malformed tool call in conversation history) vs transient.
    //
    // x-should-retry: true is intentionally NOT handled here — we only
    // use the header to suppress retries (false), not to force them
    // (true). Forcing retries on non-retryable status codes could
    // amplify failures. true falls through to existing status-code logic.
    //
    // Checked AFTER image-strip guards: image stripping changes the
    // request payload, so a server "don't retry" on the original
    // request doesn't apply to the stripped request.
    if let Some(false) = err.should_retry_header() {
        return RetryDecision::Fatal(clone_error(err));
    }

    // Context-window / size overflow is deterministic — re-sending the same (or
    // larger) payload always fails — so never retry it, whatever status the backend
    // used (in-stream `ResponseError`→500, HTTP 400/500, OpenAI/Anthropic variants).
    if err.is_context_length_error() {
        return RetryDecision::Fatal(clone_error(err));
    }

    // Doom-loop failures: always Retry with near-immediate backoff. The
    // recovery loop intercepts these BEFORE classification and runs its own
    // budget (`policy.max_retries`, enforced by disarming the abort); this
    // arm only keeps classification total so a stray doom failure through
    // any other path can never be Fatal.
    if matches!(err, SamplingError::DoomLoopDetected { .. }) {
        return RetryDecision::Retry {
            backoff: doom_loop_backoff(retry_count + 1),
        };
    }

    // Rate-limited (429): honor Retry-After / exponential backoff.
    // Unlimited budget (default) keeps trying until credits/upstream recover.
    if err.is_rate_limited() {
        let next_attempt = retry_count.saturating_add(1);
        let effective_cap = max_retries.min(rate_limit_threshold);
        if !is_unlimited_retries(effective_cap) && next_attempt >= effective_cap {
            return RetryDecision::Fatal(clone_error(err));
        }
        let backoff = err
            .retry_after()
            .map(Duration::from_secs)
            .unwrap_or_else(|| retry_backoff_with_jitter(next_attempt));
        return RetryDecision::RetryWithBackoff {
            backoff,
            is_rate_limited: true,
        };
    }

    // Generic retryable transport / 5xx / timeout / stream interrupts.
    // First retry rebuilds the HTTP client with HTTP/1.1 to escape
    // poisoned HTTP/2 pools; later retries just back off. Under-window
    // uses a small honest budget (not unlimited). Over-window is Fatal
    // before this arm (`classify_error_with_window`).
    if err.is_retryable() {
        let next_attempt = retry_count.saturating_add(1);
        let cap = transport_retry_cap(max_retries);
        if next_attempt >= cap {
            return RetryDecision::Fatal(clone_error(err));
        }
        let backoff = match err.retry_after() {
            // Cloudflare 52x often sends Retry-After: 60-120. Honor values
            // at or under 30s exactly; clamp longer waits to 30s +/- 20%
            // jitter so 14 retries do not stall the turn for tens of minutes.
            Some(secs) if secs > 30 => jitter_around(Duration::from_secs(30)),
            Some(secs) => Duration::from_secs(secs),
            None => retry_backoff_with_jitter(next_attempt),
        };
        if next_attempt == 1 {
            return RetryDecision::RetryWithClientRebuild { backoff };
        }
        return RetryDecision::Retry { backoff };
    }

    // Everything else is fatal.
    RetryDecision::Fatal(clone_error(err))
}

/// Build a human-readable, telemetry-friendly description of a sampling
/// error.
///
/// `retry_count`, when present, is rendered as a "Request failed after
/// N retries." prefix. The function is pure string formatting: no
/// logging, no I/O, no allocation beyond the produced `String`.
pub fn format_sampling_error(err: &SamplingError, retry_count: Option<u32>) -> String {
    let retry_prefix = match retry_count {
        Some(count) => format!("Request failed after {} retries. ", count),
        None => String::new(),
    };

    match err {
        SamplingError::Auth { message, .. } => {
            format!(
                "{}Authentication failed: {}. Please check your API key configuration.",
                retry_prefix, message
            )
        }
        SamplingError::InvalidConfiguration(msg) => {
            format!(
                "{}Invalid configuration: {}. Please check your model settings.",
                retry_prefix, msg
            )
        }

        SamplingError::Http(e) => {
            let mut details = Vec::new();
            if e.is_timeout() {
                details.push("timeout".to_string());
            }
            if e.is_connect() {
                details.push("connection failed".to_string());
            }
            if let Some(status) = e.status() {
                details.push(format!("status {}", status));
            }
            if let Some(url) = e.url() {
                details.push(format!("url: {}", url));
            }
            let detail_str = if details.is_empty() {
                e.to_string()
            } else {
                format!("{} ({})", e, details.join(", "))
            };
            format!(
                "{}HTTP request failed: {}. This may be a network issue or the API endpoint may be unavailable.",
                retry_prefix, detail_str
            )
        }
        SamplingError::Serialization(e) => {
            format!(
                "{}Failed to parse API response at line {} column {}: {}. This indicates an unexpected response format from the server.",
                retry_prefix,
                e.line(),
                e.column(),
                e
            )
        }
        SamplingError::Api {
            status, message, ..
        } => {
            let code = status.as_u16();
            // Exhausted (or multi-try) edge outages: plain English, not raw
            // "API error (status 521 <unknown status code>)" / Internal JSON.
            if is_edge_outage_status(code) || matches!(code, 502..=504) {
                if let Some(count) = retry_count {
                    return outage_exhausted_user_message(*status, count);
                }
            }
            let status_hint = match code {
                400 => " (bad request - check your input)",
                401 | 403 => " (authentication issue - check your API key)",
                404 => " (endpoint not found - check model configuration)",
                413 => " (request too large - try /compact or start new session)",
                429 => " (rate limited - please wait and retry)",
                500 => " (server internal error)",
                502..=504 => " (server unavailable - please retry)",
                c if is_edge_outage_status(c) => " (xAI connection interrupted - please retry)",
                _ => "",
            };
            format!(
                "{}API error (HTTP {}{}): {}",
                retry_prefix, code, status_hint, message
            )
        }
        SamplingError::EventStreamError(msg) => {
            format!(
                "{}Event stream error: {}. The connection to the server was interrupted.",
                retry_prefix, msg
            )
        }
        SamplingError::StreamError {
            error_type,
            message,
            ..
        } => {
            format!(
                "{}Server stream error ({}): {}. The server encountered an error while streaming the response.",
                retry_prefix, error_type, message
            )
        }
        SamplingError::IdleTimeout { elapsed_secs } => {
            format!(
                "{}Model stopped responding after {}s. The model may be overloaded or stuck. Try again or use a different model.",
                retry_prefix, elapsed_secs
            )
        }
        SamplingError::EmptyResponse { context } => {
            format!(
                "{}Empty response from model ({}): model={}, had_reasoning={}, finish_reason={}, completion_tokens={}",
                retry_prefix,
                context.reason,
                context.model,
                context.had_reasoning,
                context.finish_reason_str(),
                context.completion_tokens.unwrap_or(0),
            )
        }
        SamplingError::MaxTokensTruncation => {
            format!("{}Response truncated by max_tokens.", retry_prefix)
        }
        SamplingError::DoomLoopDetected { triggers, .. } => {
            format!(
                "{}Server detected a reasoning loop ({}); resampling the response.",
                retry_prefix,
                triggers.join(", ")
            )
        }
    }
}

/// Reconstruct an owned [`SamplingError`] from a borrowed one.
///
/// `SamplingError` does not implement `Clone` because its `Http` and
/// `Serialization` variants wrap non-`Clone` types. The retry loop
/// only borrows the error during classification, then needs to surface
/// it; this helper produces a faithful copy where possible. `Http`
/// falls back to a structured `EventStreamError` (still retryable, like
/// the original transport error). `Serialization` must stay
/// `Serialization`: laundering it into `EventStreamError` would flip a
/// fatal response-parse failure into a retryable one and burn the full
/// retry budget re-generating a response that fails the same way.
pub(crate) fn clone_error(err: &SamplingError) -> SamplingError {
    match err {
        SamplingError::Auth {
            message,
            credential,
        } => SamplingError::Auth {
            message: message.clone(),
            credential: *credential,
        },
        SamplingError::InvalidConfiguration(msg) => SamplingError::InvalidConfiguration(msg),
        SamplingError::Http(e) => {
            // reqwest::Error is not Clone; preserve the rendered message
            // as an EventStreamError (the closest retryable transport
            // variant) so callers see an equivalent description.
            SamplingError::EventStreamError(e.to_string())
        }
        SamplingError::Serialization(e) => {
            // serde_json::Error is not Clone; its Display already carries the
            // original line/column exactly once.
            SamplingError::serialization_message(e)
        }
        SamplingError::Api {
            status,
            message,
            model_metadata,
            retry_after_secs,
            should_retry,
            error_code,
        } => SamplingError::Api {
            status: *status,
            message: message.clone(),
            model_metadata: model_metadata.clone(),
            retry_after_secs: *retry_after_secs,
            should_retry: *should_retry,
            error_code: error_code.clone(),
        },
        SamplingError::EventStreamError(msg) => SamplingError::EventStreamError(msg.clone()),
        SamplingError::StreamError {
            error_type,
            message,
            code,
        } => SamplingError::StreamError {
            error_type: error_type.clone(),
            message: message.clone(),
            code: code.clone(),
        },
        SamplingError::IdleTimeout { elapsed_secs } => SamplingError::IdleTimeout {
            elapsed_secs: *elapsed_secs,
        },
        SamplingError::EmptyResponse { context } => SamplingError::EmptyResponse {
            context: context.clone(),
        },
        SamplingError::MaxTokensTruncation => SamplingError::MaxTokensTruncation,
        SamplingError::DoomLoopDetected {
            triggers,
            aborted_at_chunk,
        } => SamplingError::DoomLoopDetected {
            triggers: triggers.clone(),
            aborted_at_chunk: *aborted_at_chunk,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;
    use xai_grok_sampling_types::ApiErrorCode;
    use xai_grok_sampling_types::is_transient_api_status;

    fn api_err(status: StatusCode, message: &str) -> SamplingError {
        SamplingError::Api {
            status,
            message: message.to_string(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            error_code: None,
        }
    }

    fn api_err_with_retry_after(status: StatusCode, retry_after: u64) -> SamplingError {
        SamplingError::Api {
            status,
            message: "x".to_string(),
            model_metadata: None,
            retry_after_secs: Some(retry_after),
            should_retry: None,
            error_code: None,
        }
    }

    #[test]
    fn resolve_max_retries_env_override_takes_precedence() {
        assert_eq!(resolve_max_retries_with_env(Some("9"), Some(3)), 9);
    }

    #[test]
    fn resolve_max_retries_falls_back_to_model() {
        assert_eq!(resolve_max_retries_with_env(None, Some(7)), 7);
    }

    #[test]
    fn resolve_max_retries_default() {
        assert_eq!(
            resolve_max_retries_with_env(None, None),
            DEFAULT_MAX_RETRIES
        );
    }

    #[test]
    fn resolve_max_retries_invalid_env_falls_through() {
        assert_eq!(resolve_max_retries_with_env(Some("abc"), Some(4)), 4);
    }

    #[test]
    fn backoff_first_retry_is_around_two_seconds() {
        let backoff = retry_backoff_with_jitter(1);
        // Base 2000ms +/- 20% jitter (400ms range).
        assert!(
            backoff >= Duration::from_millis(1600) && backoff <= Duration::from_millis(2400),
            "first retry backoff out of range: {:?}",
            backoff
        );
    }

    #[test]
    fn backoff_doubles_then_caps_at_max_backoff() {
        // retry_count=2: base 4s
        let r2 = retry_backoff_with_jitter(2);
        assert!(r2 >= Duration::from_millis(3200) && r2 <= Duration::from_millis(4800));

        // retry_count=10: base would exceed cap → MAX_BACKOFF_SECS (60s) ±20%
        let r10 = retry_backoff_with_jitter(10);
        let cap_ms = MAX_BACKOFF_SECS * 1000;
        assert!(
            r10 >= Duration::from_millis(cap_ms * 4 / 5)
                && r10 <= Duration::from_millis(cap_ms * 6 / 5),
            "expected ~{cap_ms}ms cap, got {r10:?}"
        );
    }

    #[test]
    fn unlimited_default_does_not_retry_5xx_forever() {
        let err = api_err(StatusCode::BAD_GATEWAY, "proxy blip");
        match classify_error(&err, 100, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::BAD_GATEWAY);
            }
            other => panic!("unlimited default must still cap under-window 5xx, got {other:?}"),
        }
    }

    #[test]
    fn unlimited_retries_never_fatals_on_429() {
        let err = api_err(StatusCode::TOO_MANY_REQUESTS, "slow down");
        match classify_error(&err, 50, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                is_rate_limited: true,
                ..
            } => {}
            other => panic!("expected rate-limit Retry under unlimited budget, got {other:?}"),
        }
    }

    #[test]
    fn default_max_retries_is_unlimited() {
        assert_eq!(DEFAULT_MAX_RETRIES, u32::MAX);
        assert!(is_unlimited_retries(DEFAULT_MAX_RETRIES));
        assert_eq!(resolve_max_retries_with_env(None, None), u32::MAX);
    }

    #[test]
    fn resolve_max_retries_env_can_cap() {
        assert_eq!(resolve_max_retries_with_env(Some("3"), None), 3);
        assert_eq!(resolve_max_retries_with_env(Some("3"), Some(99)), 3);
    }

    #[test]
    fn long_retry_after_on_429_is_used() {
        let err = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 3600);
        match classify_error(&err, 0, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                backoff,
                is_rate_limited: true,
            } => {
                assert_eq!(backoff, Duration::from_secs(3600));
            }
            other => panic!("expected 3600s Retry-After, got {other:?}"),
        }
    }

    #[test]
    fn backoff_zero_retry_count_is_well_defined() {
        // retry_count = 0 corresponds to "before the first retry"; ensure
        // it does not panic and stays in the lowest backoff bucket.
        let backoff = retry_backoff_with_jitter(0);
        assert!(backoff >= Duration::from_millis(1600) && backoff <= Duration::from_millis(2400));
    }

    #[test]
    fn classify_auth_error_emits_to_session() {
        let err = SamplingError::auth_unknown("bad token");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(SamplingError::Auth { .. }) => {}
            other => panic!("expected EmitToSession(Auth), got {other:?}"),
        }
    }

    #[test]
    fn classify_unauthorized_emits_to_session() {
        let err = api_err(StatusCode::UNAUTHORIZED, "no");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(SamplingError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::UNAUTHORIZED);
            }
            other => panic!("expected EmitToSession(Api 401), got {other:?}"),
        }
    }

    #[test]
    fn classify_forbidden_bad_credentials_emits_to_session() {
        let err = api_err(
            StatusCode::FORBIDDEN,
            "unauthenticated:bad-credentials: The OAuth2 access token could not be validated.",
        );
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(SamplingError::Api {
                status, message, ..
            }) => {
                assert_eq!(status, StatusCode::FORBIDDEN);
                assert!(message.contains("bad-credentials"));
            }
            other => panic!("expected EmitToSession(Api 403 bad-credentials), got {other:?}"),
        }
    }

    #[test]
    fn classify_forbidden_policy_is_fatal_not_auth() {
        let err = api_err(StatusCode::FORBIDDEN, "Content violates usage guidelines.");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::FORBIDDEN);
            }
            other => panic!("expected Fatal policy 403, got {other:?}"),
        }
    }

    #[test]
    fn classify_encrypted_content_emits_to_session() {
        let err = api_err(
            StatusCode::BAD_REQUEST,
            "Could not decrypt the provided encrypted_content",
        );
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(_) => {}
            other => panic!("expected EmitToSession, got {other:?}"),
        }
    }

    #[test]
    fn classify_payload_too_large_strips_images() {
        let err = api_err(StatusCode::PAYLOAD_TOO_LARGE, "too big");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_processing_error_400_strips_images() {
        let err = api_err(StatusCode::BAD_REQUEST, "Could not process image");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_processing_error_500_wrapped_strips_images() {
        let err = api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "upstream: 400 Bad Request: Could not process image",
        );
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_processing_error_takes_priority_over_5xx_retry() {
        // A 500 wrapping "Could not process image" is retryable by status
        // code alone — verify the image-processing guard intercepts first.
        let err = api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not process image: bad format",
        );
        assert!(
            err.is_retryable(),
            "500 is retryable without the image-processing guard"
        );
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_400_strips_even_with_should_retry_false() {
        // The server stamps invalid-image 400s with `x-should-retry: false`;
        // that verdict only covers resending the SAME payload, and the strip
        // retry sends a different one — the code must beat the header veto.
        let err = SamplingError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "some future wording without the legacy phrase".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: Some(false),
            error_code: Some(ApiErrorCode::InvalidImage),
        };
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_stream_error_strips_instead_of_blind_retry() {
        // Coded mid-stream image rejections must strip immediately; a
        // code-less stream error keeps the ordinary retry path.
        let err = SamplingError::StreamError {
            error_type: "invalid_request_error".into(),
            message: "Base64 string of provided image cannot be decoded.".into(),
            code: Some(ApiErrorCode::InvalidImage),
        };
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));

        let unrelated = SamplingError::StreamError {
            error_type: "overloaded_error".into(),
            message: "The server is overloaded.".into(),
            code: None,
        };
        assert!(!matches!(
            classify_error(&unrelated, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_rate_limited_uses_retry_after() {
        let err = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 7);
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                backoff,
                is_rate_limited,
            } => {
                assert!(is_rate_limited);
                assert_eq!(backoff, Duration::from_secs(7));
            }
            other => panic!("expected RetryWithBackoff, got {other:?}"),
        }
    }

    #[test]
    fn classify_rate_limited_capped_at_threshold() {
        let err = api_err(StatusCode::TOO_MANY_REQUESTS, "slow");
        // Explicit finite threshold: retry_count=1, threshold=2 -> next=2 >= 2 -> Fatal.
        match classify_error(&err, 1, 5, 2) {
            RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
            }
            other => panic!("expected Fatal at threshold, got {other:?}"),
        }
    }

    #[test]
    fn zero_retry_budget_never_reuses_a_model_output_cap() {
        for err in [
            api_err(StatusCode::INTERNAL_SERVER_ERROR, "boom"),
            api_err(StatusCode::PAYLOAD_TOO_LARGE, "too big"),
            api_err(StatusCode::BAD_REQUEST, "Could not process image"),
            SamplingError::EmptyResponse {
                context: xai_grok_sampling_types::EmptyResponseContext {
                    reason: xai_grok_sampling_types::EmptyReason::NoVisibleContent,
                    had_reasoning: false,
                    content_len: 0,
                    tool_call_count: 0,
                    finish_reason: Some("stop".into()),
                    completion_tokens: Some(1),
                    reasoning_tokens: Some(0),
                    prompt_tokens: Some(10),
                    model: "m".into(),
                    first_choice_seen: true,
                },
            },
        ] {
            assert!(matches!(
                classify_error(&err, 0, 0, RATE_LIMIT_RETRY_THRESHOLD),
                RetryDecision::Fatal(_)
            ));
        }
    }

    #[test]
    fn classify_5xx_first_retry_rebuilds_client() {
        let err = api_err(StatusCode::INTERNAL_SERVER_ERROR, "boom");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => {
                assert!(backoff >= Duration::from_millis(1600));
            }
            other => panic!("expected RetryWithClientRebuild, got {other:?}"),
        }
    }

    #[test]
    fn classify_cloudflare_522_is_retryable() {
        let err = api_err(
            StatusCode::from_u16(522).unwrap(),
            "Connection to Grok timed out or was interrupted. (HTTP 522).",
        );
        match classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild for 522, got {other:?}"),
        }
        match classify_error(&err, 1, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { .. } => {}
            other => panic!("expected Retry for 522 attempt 2, got {other:?}"),
        }
    }

    #[test]
    fn classify_cloudflare_525_is_fatal_even_with_should_retry_true() {
        // `x-should-retry: true` is deliberately ignored (only `false` is
        // honored), so 525/526 stay Fatal whatever a future header says.
        for should_retry in [None, Some(true)] {
            let err = SamplingError::Api {
                status: StatusCode::from_u16(525).unwrap(),
                message: "Secure connection to Grok failed. (HTTP 525).".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry,
                error_code: None,
            };
            match classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
                RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                    assert_eq!(status.as_u16(), 525);
                }
                other => panic!("expected Fatal for 525 ({should_retry:?}), got {other:?}"),
            }
        }
    }

    /// HTTP 502 with a 29m26s Retry-After must not sit that long. Clamp to
    /// ~30s like other gateway outages. 502 is not billing empty and is not
    /// a 429 hop.
    #[test]
    fn classify_502_long_retry_after_clamps_and_does_not_hop() {
        let err = api_err_with_retry_after(StatusCode::BAD_GATEWAY, 1766);
        assert!(err.is_retryable());
        assert!(!err.is_rate_limited());
        assert!(!err.is_credit_exhausted());
        match classify_error(&err, 0, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => {
                assert!(
                    backoff >= Duration::from_secs(24) && backoff <= Duration::from_secs(36),
                    "502 Retry-After 1766s (29m26s) must clamp to ~30s, got {backoff:?}"
                );
            }
            other => panic!("first 502 must rebuild the client, got {other:?}"),
        }
        match classify_error(
            &err,
            DEFAULT_TRANSPORT_MAX_RETRIES.saturating_sub(1),
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
        ) {
            RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::BAD_GATEWAY);
            }
            other => panic!(
                "unlimited budget must still stop 502 after a small transport cap, got {other:?}"
            ),
        }
        match classify_error(&err, 0, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                is_rate_limited: true,
                ..
            } => panic!("502 must not take the 429 hop/wait path"),
            _ => {}
        }
    }

    #[test]
    fn classify_clamps_and_jitters_retry_after_on_generic_path_but_not_on_429() {
        // Cloudflare answers 52x with Retry-After: 60-120. Honoring that
        // verbatim across 14 retries would stall the turn ~28 min, and an
        // unjittered wait would re-hit the recovering origin in lockstep.
        let edge = api_err_with_retry_after(StatusCode::from_u16(522).unwrap(), 120);
        match classify_error(&edge, 1, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { backoff } => {
                // 30s clamp with +/-20% jitter.
                assert!(backoff >= Duration::from_secs(24), "got {backoff:?}");
                assert!(backoff <= Duration::from_secs(36), "got {backoff:?}");
            }
            other => panic!("expected Retry for 522, got {other:?}"),
        }

        // The 429 path keeps the full wait; its total is bounded by
        // RATE_LIMIT_RETRY_THRESHOLD attempts and the parse-level 120s cap.
        let rate_limited = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 120);
        match classify_error(&rate_limited, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff { backoff, .. } => {
                assert_eq!(backoff, Duration::from_secs(120));
            }
            other => panic!("expected RetryWithBackoff for 429, got {other:?}"),
        }
    }

    #[test]
    fn classify_5xx_subsequent_retry_uses_plain_retry() {
        let err = api_err(StatusCode::BAD_GATEWAY, "boom");
        match classify_error(&err, 1, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { backoff } => {
                assert!(backoff >= Duration::from_millis(3200));
            }
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn classify_5xx_exhausted_retries_is_fatal() {
        let err = api_err(StatusCode::SERVICE_UNAVAILABLE, "boom");
        match classify_error(&err, 4, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(SamplingError::Api { .. }) => {}
            other => panic!("expected Fatal, got {other:?}"),
        }
    }

    fn api_status_code(code: u16, message: &str) -> SamplingError {
        api_err(StatusCode::from_u16(code).expect("valid status"), message)
    }

    /// HTTP 521 (Cloudflare "Web Server Is Down") and sibling edge outages
    /// soft-retry like 502/503 — not Fatal on first response.
    #[test]
    fn classify_521_is_retryable_with_client_rebuild() {
        let err = api_status_code(521, "origin down");
        assert!(err.is_retryable());
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => {
                assert!(backoff >= Duration::from_millis(1600));
            }
            other => panic!("expected RetryWithClientRebuild for 521, got {other:?}"),
        }
        match classify_error(&err, 1, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { .. } => {}
            other => panic!("expected plain Retry on subsequent 521, got {other:?}"),
        }
    }

    #[test]
    fn classify_521_honors_retry_after_when_present() {
        let err = SamplingError::Api {
            status: StatusCode::from_u16(521).unwrap(),
            message: "origin down".into(),
            model_metadata: None,
            retry_after_secs: Some(12),
            should_retry: None,
            error_code: None,
        };
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => {
                assert_eq!(backoff, Duration::from_secs(12));
            }
            other => panic!("expected Retry-After honored for 521, got {other:?}"),
        }
    }

    #[test]
    fn classify_521_exhausted_budget_is_fatal() {
        let err = api_status_code(521, "origin down");
        match classify_error(&err, 4, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                assert_eq!(status.as_u16(), 521);
            }
            other => panic!("expected Fatal after budget, got {other:?}"),
        }
    }

    #[test]
    fn format_521_exhausted_is_plain_english() {
        let err = api_status_code(521, "ignored body");
        let s = format_sampling_error(&err, Some(3));
        assert_eq!(
            s,
            "xAI connection failed after 3 tries (HTTP 521). Try again shortly."
        );
        assert!(!s.contains("unknown status"));
        assert!(!s.contains("API error"));
    }

    #[test]
    fn format_521_without_retry_count_keeps_status_hint() {
        let err = api_status_code(521, "origin down");
        let s = format_sampling_error(&err, None);
        assert!(s.contains("HTTP 521"));
        assert!(s.contains("xAI connection interrupted") || s.contains("origin down"));
    }

    #[test]
    fn cloudflare_edge_range_is_transient() {
        // Transient edge outages rebuild the HTTP client. Origin-TLS 525/526
        // stay Fatal via RetryPolicy::edge_client (a broken cert never clears).
        // is_transient_api_status may still list 525/526; classify uses
        // is_retryable, which does not treat those two as retryable.
        for code in [520u16, 521, 522, 523, 524, 527, 530] {
            assert!(
                is_transient_api_status(code),
                "{code} should be transient for classify"
            );
            let err = api_status_code(code, "edge");
            assert!(
                matches!(
                    classify_error(&err, 0, 3, RATE_LIMIT_RETRY_THRESHOLD),
                    RetryDecision::RetryWithClientRebuild { .. }
                ),
                "classify {code}"
            );
        }
        for code in [525u16, 526] {
            let err = api_status_code(code, "edge");
            match classify_error(&err, 0, 3, RATE_LIMIT_RETRY_THRESHOLD) {
                RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                    assert_eq!(status.as_u16(), code);
                }
                other => panic!("expected Fatal for origin-TLS {code}, got {other:?}"),
            }
        }
    }

    #[test]
    fn classify_event_stream_error_is_retryable() {
        let err = SamplingError::EventStreamError("connection reset".into());
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild, got {other:?}"),
        }
    }

    #[test]
    fn classify_stream_error_is_retryable() {
        let err = SamplingError::StreamError {
            error_type: "transient".into(),
            message: "x".into(),
            code: None,
        };
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild for StreamError, got {other:?}"),
        }
    }

    #[test]
    fn classify_idle_timeout_is_fatal() {
        let err = SamplingError::IdleTimeout { elapsed_secs: 300 };
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(SamplingError::IdleTimeout { elapsed_secs: 300 }) => {}
            other => panic!("expected Fatal(IdleTimeout), got {other:?}"),
        }
    }

    #[test]
    fn classify_invalid_config_is_fatal() {
        let err = SamplingError::InvalidConfiguration("missing model");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(SamplingError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn classify_api_400_non_encrypted_is_fatal() {
        let err = api_err(StatusCode::BAD_REQUEST, "Invalid model parameter");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    fn serialization_err() -> SamplingError {
        SamplingError::Serialization(serde_json::from_str::<i32>("not a number").unwrap_err())
    }

    /// Regression: `clone_error` used to launder `Serialization` into the
    /// retryable `EventStreamError`, turning a deterministic parse failure
    /// into a full-budget retry storm.
    #[test]
    fn clone_error_preserves_serialization_and_non_retryability() {
        let cloned = clone_error(&serialization_err());
        assert!(
            matches!(cloned, SamplingError::Serialization(_)),
            "expected Serialization, got {cloned:?}"
        );
        assert!(!cloned.is_retryable());
        assert!(
            cloned.to_string().contains("line 1 column"),
            "original position text must survive the clone: {cloned}"
        );
    }

    /// The tee cell captures mid-stream errors via `clone_error`; dropping
    /// the code there would silently disable mid-stream strip recovery.
    #[test]
    fn clone_error_preserves_stream_error_code() {
        let cloned = clone_error(&SamplingError::StreamError {
            error_type: "invalid_request_error".into(),
            message: "bad image".into(),
            code: Some(ApiErrorCode::InvalidImage),
        });
        let SamplingError::StreamError { code, .. } = &cloned else {
            panic!("expected StreamError, got {cloned:?}");
        };
        assert_eq!(*code, Some(ApiErrorCode::InvalidImage));
        assert!(cloned.is_image_processing_error());
    }

    #[test]
    fn classify_serialization_is_fatal_on_first_attempt() {
        match classify_error(&serialization_err(), 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(SamplingError::Serialization(_)) => {}
            other => panic!("expected Fatal(Serialization) on attempt 1, got {other:?}"),
        }
    }

    #[test]
    fn format_includes_retry_prefix_when_count_present() {
        let err = SamplingError::auth_unknown("bad");
        let s = format_sampling_error(&err, Some(3));
        assert!(s.starts_with("Request failed after 3 retries."));
    }

    #[test]
    fn format_omits_retry_prefix_when_count_absent() {
        let err = SamplingError::auth_unknown("bad");
        let s = format_sampling_error(&err, None);
        assert!(!s.starts_with("Request failed after"));
        assert!(s.starts_with("Authentication failed:"));
    }

    #[test]
    fn format_includes_status_hint_for_known_codes() {
        let err = api_err(StatusCode::PAYLOAD_TOO_LARGE, "big");
        let s = format_sampling_error(&err, None);
        assert!(s.contains("HTTP 413"));
        assert!(s.contains("request too large"));
    }

    #[test]
    fn format_idle_timeout_includes_elapsed_secs() {
        let err = SamplingError::IdleTimeout { elapsed_secs: 240 };
        let s = format_sampling_error(&err, None);
        assert!(s.contains("240s"));
    }

    #[test]
    fn should_retry_false_overrides_retryable_status() {
        let err = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: Some(false),
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn context_length_overflow_is_fatal_even_as_500() {
        // The backend streams a size overflow as a ResponseError that becomes a 500 with no
        // should_retry hint; without the context-length check it would retry the full budget.
        let err = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "none: The prompt is too long for this model's context window.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn should_retry_true_falls_through_to_existing_logic() {
        let err = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: Some(true),
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithClientRebuild { .. }
        ));
    }

    #[test]
    fn should_retry_absent_falls_through() {
        let err = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithClientRebuild { .. }
        ));
    }

    #[test]
    fn classify_doom_loop_detected_is_retry_with_immediate_backoff() {
        let err = SamplingError::DoomLoopDetected {
            triggers: vec!["tail_repetition:8@thinking".into()],
            aborted_at_chunk: None,
        };
        // Whatever the counters say, classification is Retry — the recovery
        // loop owns the budget by disarming the abort when it is spent.
        for retry_count in [0, 5, 99] {
            match classify_error(&err, retry_count, 2, RATE_LIMIT_RETRY_THRESHOLD) {
                RetryDecision::Retry { backoff } => {
                    assert!(backoff <= Duration::from_millis(250), "near-immediate");
                }
                other => panic!("expected Retry, got {other:?}"),
            }
        }
    }

    #[test]
    fn should_retry_false_on_429_is_fatal() {
        // Server says don't retry, even though 429 is normally retryable.
        // should_retry check runs before rate-limit check.
        let err = SamplingError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "rate limited".into(),
            model_metadata: None,
            retry_after_secs: Some(10),
            should_retry: Some(false),
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn over_window_blocks_retry_when_used_meets_sampling_window() {
        assert!(over_window_blocks_retry(Some(411_000), 200_000));
        assert!(over_window_blocks_retry(Some(200_000), 200_000));
        assert!(!over_window_blocks_retry(Some(199_999), 200_000));
        assert!(!over_window_blocks_retry(None, 200_000));
        assert!(!over_window_blocks_retry(Some(500_000), 0));
    }

    /// Named leftover from the over-window Fatal slice: under-window 5xx and
    /// first-token / headers timeout must not retry forever. Small honest
    /// budget, then Fatal with the same named error.
    #[test]
    fn under_window_timeout_retries_are_capped_then_fatal() {
        let err = SamplingError::EventStreamError(
            "timed out waiting for the first token after 2m0s".into(),
        );
        match classify_error_with_window(
            &err,
            0,
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(384_000),
            500_000,
        ) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("first under-window first-token timeout still retries, got {other:?}"),
        }
        match classify_error_with_window(
            &err,
            DEFAULT_TRANSPORT_MAX_RETRIES.saturating_sub(1),
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(384_000),
            500_000,
        ) {
            RetryDecision::Fatal(SamplingError::EventStreamError(msg)) => {
                assert!(
                    msg.contains("first token"),
                    "exhausted timeout must keep the named cause, got {msg}"
                );
            }
            other => panic!(
                "under-window first-token timeout must stop after a small budget, got {other:?}"
            ),
        }
    }

    #[test]
    fn under_window_5xx_retries_are_capped_then_fatal() {
        let err = api_err(StatusCode::INTERNAL_SERVER_ERROR, "boom");
        match classify_error_with_window(
            &err,
            0,
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(100_000),
            200_000,
        ) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("first under-window 5xx still retries, got {other:?}"),
        }
        match classify_error_with_window(
            &err,
            DEFAULT_TRANSPORT_MAX_RETRIES.saturating_sub(1),
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(100_000),
            200_000,
        ) {
            RetryDecision::Fatal(SamplingError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            }
            other => panic!("under-window 5xx must stop after a small budget, got {other:?}"),
        }
    }

    /// Named contract: over-window model failure does not keep incrementing
    /// retry attempts. Same oversized payload cannot recover via 5xx retry.
    #[test]
    fn over_window_model_failure_does_not_keep_incrementing_retries() {
        let err = api_err(StatusCode::INTERNAL_SERVER_ERROR, "boom");
        match classify_error_with_window(
            &err,
            0,
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(411_000),
            200_000,
        ) {
            RetryDecision::Fatal(_) => {}
            other => {
                panic!("over-window 5xx must be Fatal (compact or honest fail), not {other:?}")
            }
        }
        match classify_error_with_window(
            &err,
            1,
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(411_000),
            200_000,
        ) {
            RetryDecision::Fatal(_) => {}
            other => panic!("a later over-window attempt must stay Fatal, got {other:?}"),
        }
        match classify_error_with_window(
            &err,
            0,
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(100_000),
            200_000,
        ) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("under-window 5xx still retries, got {other:?}"),
        }
    }

    #[test]
    fn over_window_timeout_is_fatal_not_retry_chrome() {
        let err = SamplingError::EventStreamError("timed out waiting for response headers".into());
        match classify_error_with_window(
            &err,
            0,
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
            Some(411_000),
            200_000,
        ) {
            RetryDecision::Fatal(_) => {}
            other => panic!("over-window timeout must not Retrying, got {other:?}"),
        }
    }
}
