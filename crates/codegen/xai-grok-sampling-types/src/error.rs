//! Sampling error types.
//!
//! TODO: Move from xai-grok-shell/src/sampling/error.rs

use std::fmt;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SamplingError>;

/// Why the model's response was classified as "empty" by [`ConversationResponse::empty_reason`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyReason {
    /// The model emitted reasoning tokens but produced no visible content
    /// and no tool calls. The stream completed normally (has `finish_reason`).
    ReasoningOnly,
    /// The stream carried at least one `choice` but the final assistant
    /// message has empty `content` and no tool calls (and no reasoning).
    NoVisibleContent,
}

impl EmptyReason {
    pub fn as_str(self) -> &'static str {
        match self {
            EmptyReason::ReasoningOnly => "reasoning_only",
            EmptyReason::NoVisibleContent => "no_visible_content",
        }
    }
}

impl fmt::Display for EmptyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured context captured at L2 stream completion time when the
/// response is classified as empty. Carries everything needed to
/// root-cause the issue from a single log line or error payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyResponseContext {
    pub reason: EmptyReason,
    /// Whether the response contained reasoning tokens.
    pub had_reasoning: bool,
    /// Byte length of the accumulated `content` string (0 for truly empty).
    pub content_len: usize,
    /// Number of tool calls in the final response.
    pub tool_call_count: usize,
    /// The `finish_reason` from the stream, if any.
    pub finish_reason: Option<String>,
    /// Token usage from the response (when available).
    pub completion_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub prompt_tokens: Option<u32>,
    /// Model that produced the response.
    pub model: String,
    /// Whether at least one `choice` was seen in the stream.
    pub first_choice_seen: bool,
}

impl EmptyResponseContext {
    pub fn finish_reason_str(&self) -> &str {
        self.finish_reason.as_deref().unwrap_or("none")
    }
}

/// Model metadata from response headers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseModelMetadata {
    pub context_window: Option<u64>,
    pub max_completion_tokens: Option<u32>,
    /// `x-models-etag` — triggers model catalog refresh when changed.
    pub models_etag: Option<String>,
}

/// Display prefix of [`SamplingError::Serialization`]. Shared with the
/// variant's `#[error(...)]` template so [`SamplingError::serialization_from_rendered`]
/// can never drift from what Display actually emits.
const SERIALIZATION_DISPLAY_PREFIX: &str = "serialization error: ";

#[derive(Debug, Error)]
pub enum SamplingError {
    #[error("{0}")]
    Auth(String),
    #[error("invalid client configuration: {0}")]
    InvalidConfiguration(&'static str),
    #[error("request error: {0}")]
    Http(reqwest::Error),
    #[error("{prefix}{0}", prefix = SERIALIZATION_DISPLAY_PREFIX)]
    Serialization(serde_json::Error),
    /// `status` is formatted via [`format_http_status`] so Cloudflare edge
    /// codes (521, …) never render as `<unknown status code>`.
    #[error("API error (status {}): {message}", format_http_status(*status))]
    Api {
        status: StatusCode,
        message: String,
        model_metadata: Option<ResponseModelMetadata>,
        /// Parsed from the `Retry-After` response header (seconds).
        retry_after_secs: Option<u64>,
        /// Parsed from the `x-should-retry` response header.
        /// `Some(true)` = transient, retry may help.
        /// `Some(false)` = request-content error, don't retry.
        /// `None` = header absent (old server or non-proxy origin).
        should_retry: Option<bool>,
    },
    #[error("reqwest error stream: {0}")]
    EventStreamError(String),
    /// Server-side stream error (sent as JSON within the SSE stream)
    #[error("stream error ({error_type}): {message}")]
    StreamError { error_type: String, message: String },
    /// Per-chunk idle timeout — no SSE chunk received from the model within the
    /// configured deadline. NOT retryable: the model (or network path) is stuck,
    /// and replaying the same request would likely stall again.
    #[error("inference idle timeout after {elapsed_secs}s with no chunks")]
    IdleTimeout { elapsed_secs: u64 },
    #[error("empty response from model ({})", context.reason)]
    EmptyResponse { context: EmptyResponseContext },
    #[error("response truncated by max_tokens")]
    MaxTokensTruncation,
    /// A confident server-reported doom loop on the attempt (mid-stream or
    /// on the completed response). Retryable on the recovery loop's own
    /// budget, separate from the transport budget. Carries the raw trigger
    /// labels (never generation content) plus, for telemetry only, the
    /// stream chunk index the mid-stream abort fired at (`None` when the
    /// signal was only seen on the completed response).
    #[error("doom loop detected: {}", triggers.join(", "))]
    DoomLoopDetected {
        triggers: Vec<String>,
        aborted_at_chunk: Option<u64>,
    },
}

impl SamplingError {
    /// Rebuild a `Serialization` error from a rendered message for non-`Clone`
    /// contexts; it must stay `Serialization` so it remains non-retryable.
    pub fn serialization_message(msg: impl fmt::Display) -> Self {
        Self::Serialization(serde::de::Error::custom(msg))
    }

    /// Rebuild from this variant's full rendered Display (e.g. a round-tripped
    /// `SamplingErrorInfo` message), stripping the Display prefix so the
    /// rebuilt error does not render it twice.
    pub fn serialization_from_rendered(rendered: &str) -> Self {
        Self::serialization_message(
            rendered
                .strip_prefix(SERIALIZATION_DISPLAY_PREFIX)
                .unwrap_or(rendered),
        )
    }

    pub fn is_auth_error(&self) -> bool {
        // Only 401 Unauthorized means the credentials themselves were rejected
        // and warrant a token refresh / re-auth. 403 Forbidden means the
        // request was authenticated successfully but the action is not
        // permitted (e.g. content-safety blocks, ZDR-blocked operations,
        // or other policy denials unrelated to credentials). Treating 403
        // as an auth error triggers a pointless
        // OIDC refresh and then surfaces as acp::Error::auth_required on
        // the client, which in the desktop app tears down the session and
        // can race with invalid_grant_threshold to wipe auth.json.
        matches!(
            self,
            SamplingError::Auth(_)
                | SamplingError::Api {
                    status: StatusCode::UNAUTHORIZED,
                    ..
                }
        )
    }

    pub fn is_rate_limited(&self) -> bool {
        matches!(
            self,
            SamplingError::Api {
                status: StatusCode::TOO_MANY_REQUESTS,
                ..
            }
        )
    }

    pub fn is_payload_too_large(&self) -> bool {
        matches!(
            self,
            SamplingError::Api {
                status: StatusCode::PAYLOAD_TOO_LARGE,
                ..
            }
        )
    }

    /// `true` when the error looks like a connection reset or broken pipe
    /// during request upload — the pattern nginx produces when it rejects an
    /// oversized payload by closing the connection instead of responding 413.
    ///
    /// Timeouts and connect failures are excluded: those are unrelated to
    /// payload size and stripping images on them would lose context for no
    /// reason.
    pub fn is_likely_body_rejected(&self) -> bool {
        match self {
            SamplingError::Http(err) => {
                // `is_request()` covers broken-pipe / connection-reset during
                // body upload.  `is_body()` covers stream-write failures.
                // Exclude timeouts and connect errors — those are unrelated.
                (err.is_request() || err.is_body()) && !err.is_timeout() && !err.is_connect()
            }
            _ => false,
        }
    }

    /// The server rejected the request because the conversation history
    /// contains `encrypted_content` from a different model family that the
    /// current model cannot decrypt. Never retryable — the user must start
    /// a new session.
    pub fn is_encrypted_content_error(&self) -> bool {
        matches!(
                    self,
                    SamplingError::Api {
                        status: StatusCode::BAD_REQUEST,
                        message,
                        ..
                    }
        if message.contains("encrypted_content")
                )
    }

    /// The API rejected the request because an inline image could not be
    /// processed. Matches both direct 400 and proxy-wrapped 500 responses.
    /// Exact-case match — consistent with `is_encrypted_content_error`.
    pub fn is_image_processing_error(&self) -> bool {
        matches!(
                    self,
                    SamplingError::Api {
                        status,
                        message,
                        ..
                    }
        if matches!(status.as_u16(), 400 | 500) && message.contains("Could not process image")
                )
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            SamplingError::Auth(_) => false,
            SamplingError::InvalidConfiguration(_) => false,
            SamplingError::Http(err) => is_retryable_reqwest(err),
            SamplingError::Serialization(_) => false,
            SamplingError::Api { status, .. } => is_transient_api_status(status.as_u16()),
            SamplingError::EventStreamError(_) => true,
            SamplingError::StreamError { .. } => true,
            SamplingError::IdleTimeout { .. } => false,
            SamplingError::EmptyResponse { .. } => true,
            SamplingError::MaxTokensTruncation => false,
            SamplingError::DoomLoopDetected { .. } => true,
        }
    }

    pub fn model_metadata(&self) -> Option<&ResponseModelMetadata> {
        match self {
            SamplingError::Api { model_metadata, .. } => model_metadata.as_ref(),
            _ => None,
        }
    }

    pub fn retry_after(&self) -> Option<u64> {
        match self {
            SamplingError::Api {
                retry_after_secs, ..
            } => *retry_after_secs,
            _ => None,
        }
    }

    /// Server hint on whether this error is worth retrying.
    pub fn should_retry_header(&self) -> Option<bool> {
        match self {
            SamplingError::Api { should_retry, .. } => *should_retry,
            _ => None,
        }
    }

    /// True when this error is a context-window/size overflow — deterministic,
    /// so retrying the same payload can't help. See [`is_context_length_error`].
    pub fn is_context_length_error(&self) -> bool {
        match self {
            SamplingError::Api { message, .. } | SamplingError::StreamError { message, .. } => {
                is_context_length_error(message)
            }
            _ => false,
        }
    }

    /// True when the provider rejected the request because the account is out
    /// of credits / over its spending limit (not a transient throttle).
    ///
    /// Used by multi-key failover: another credential with remaining balance
    /// may still succeed. Matches 402 Payment Required and credit-flavored
    /// 403/429 bodies (OpenRouter and xAI Build wording).
    pub fn is_credit_exhausted(&self) -> bool {
        match self {
            SamplingError::Api {
                status, message, ..
            } => is_credit_exhausted_status_and_message(status.as_u16(), message),
            SamplingError::StreamError { message, .. } => is_credit_exhausted_message(message),
            SamplingError::Auth(message) => is_credit_exhausted_message(message),
            _ => false,
        }
    }
}

impl From<reqwest::Error> for SamplingError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

impl From<serde_json::Error> for SamplingError {
    fn from(value: serde_json::Error) -> Self {
        tracing::debug!("Serde deserialization error: {:?}", &value);
        Self::Serialization(value)
    }
}

/// OpenAI-standard provider error format: `{"error": {"message": "...", "type": "..."}}`.
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

/// Flat error from the Grok proxy/gateway: `{"code": "...", "error": "..."}`.
#[derive(Debug, Deserialize)]
struct FlatErrorResponse {
    error: String,
    #[serde(default)]
    code: Option<String>,
}

/// Extract `(error_type, message)` from either error format.
fn try_parse_error(data: &str) -> Option<(String, String)> {
    if let Ok(resp) = serde_json::from_str::<ErrorResponse>(data) {
        return Some((
            resp.error.kind.unwrap_or_else(|| "unknown".to_string()),
            resp.error
                .message
                .unwrap_or_else(|| "unknown error".to_string()),
        ));
    }
    if let Ok(flat) = serde_json::from_str::<FlatErrorResponse>(data) {
        return Some((
            flat.code.unwrap_or_else(|| "server_error".to_string()),
            flat.error,
        ));
    }
    None
}

/// Max chars of a structured (JSON) error message shown to users.
pub const MAX_USER_ERROR_BODY_CHARS: usize = 280;

/// Known status phrases for non-IANA / Cloudflare edge codes that
/// [`StatusCode::canonical_reason`] does not know. Used so Display never
/// prints `<unknown status code>` for these outages.
///
/// See [Cloudflare HTTP status codes](https://developers.cloudflare.com/support/troubleshooting/http-status-codes/)
/// (accessed: 2026-08-04).
pub fn http_status_label(code: u16) -> Option<&'static str> {
    match code {
        520 => Some("Web Server Returned an Unknown Error"),
        521 => Some("Web Server Is Down"),
        522 => Some("Connection Timed Out"),
        523 => Some("Origin Is Unreachable"),
        524 => Some("A Timeout Occurred"),
        525 => Some("SSL Handshake Failed"),
        526 => Some("Invalid SSL Certificate"),
        527 => Some("Railgun Error"),
        530 => Some("Origin DNS Error"),
        _ => None,
    }
}

/// Format an HTTP status for user-facing Display.
///
/// Prefers the IANA reason phrase, then our Cloudflare edge map, then the
/// bare code. Never emits `<unknown status code>`.
pub fn format_http_status(status: StatusCode) -> String {
    let code = status.as_u16();
    if let Some(reason) = status.canonical_reason() {
        format!("{code} {reason}")
    } else if let Some(label) = http_status_label(code) {
        format!("{code} {label}")
    } else {
        format!("{code}")
    }
}

/// Transient API / gateway statuses worth retrying with backoff.
///
/// Includes 429, common 5xx gateways, and Cloudflare edge 52x outage codes
/// (origin down, connect fail, timeout, …). Not every 5xx: 501 Not Implemented
/// stays non-retryable.
pub fn is_transient_api_status(code: u16) -> bool {
    matches!(code, 429 | 500 | 502..=504 | 520..=527 | 530)
}

/// True when the status is a Cloudflare-style origin/edge outage (52x), not
/// a normal app 5xx. Used for operator messaging.
pub fn is_edge_outage_status(code: u16) -> bool {
    matches!(code, 520..=527 | 530)
}

/// Plain-English terminal copy when retries on a connection/outage status
/// are exhausted (or the failure is surfaced after soft retries).
///
/// Prefer this over raw `API error (status …)` / Internal error JSON for
/// operator-facing toasts and RetryFailed chrome.
pub fn outage_exhausted_user_message(status: StatusCode, attempts: u32) -> String {
    let code = status.as_u16();
    let tries = attempts.max(1);
    let try_word = if tries == 1 { "try" } else { "tries" };
    format!("xAI connection failed after {tries} {try_word} (HTTP {code}). Try again shortly.")
}

/// Short status-based copy when the body is not a structured JSON error.
///
/// Edge proxies (Cloudflare 52x, 502/503/504) return HTML pages; we never
/// sniff body text — only the HTTP status drives this fallback.
pub fn status_user_message(status: StatusCode) -> String {
    match status.as_u16() {
        code @ 502..=504 => {
            format!("Grok is temporarily unavailable. Please try again in a moment. (HTTP {code}).")
        }
        // Cloudflare edge: origin down (521), connect fail, timeout, …
        521 => {
            "xAI is temporarily unreachable (origin down). Please try again shortly. (HTTP 521)."
                .to_string()
        }
        code @ 520..=527 | code @ 530 => {
            format!(
                "Connection to Grok timed out or was interrupted. Please try again. (HTTP {code})."
            )
        }
        code if status.is_server_error() => {
            format!("Something went wrong on the server (HTTP {code}).")
        }
        code if status.is_client_error() => format!("Request failed (HTTP {code})."),
        code => format!("Request failed (HTTP {code})."),
    }
}

fn truncate_user_error(s: &str) -> String {
    let s = s.trim();
    let count = s.chars().count();
    if count <= MAX_USER_ERROR_BODY_CHARS {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(MAX_USER_ERROR_BODY_CHARS).collect();
    out.push('\u{2026}');
    out
}

/// Format a known JSON error envelope; `None` if the body is not structured.
fn structured_error_message(bytes: &[u8]) -> Option<String> {
    let (error_type, message) = std::str::from_utf8(bytes).ok().and_then(try_parse_error)?;
    let msg = if error_type == "unknown" || error_type == "server_error" {
        message
    } else {
        format!("{error_type}: {message}")
    };
    Some(truncate_user_error(&msg))
}

/// Parse an API error body into a short string.
///
/// Only structured JSON error envelopes are surfaced. Non-JSON bodies
/// (HTML edge pages, plain text dumps) return a fixed placeholder — never
/// the raw bytes. Prefer [`user_facing_api_error_message`] when a status
/// code is available.
pub fn parse_error_bytes(bytes: &[u8]) -> String {
    structured_error_message(bytes).unwrap_or_else(|| "upstream error".into())
}

/// User-facing message for a failed API call.
///
/// Structured JSON error envelopes keep their message. Everything else
/// (including Cloudflare HTML) maps to a status-based string — no body
/// content matching.
pub fn user_facing_api_error_message(status: StatusCode, bytes: &[u8]) -> String {
    structured_error_message(bytes).unwrap_or_else(|| status_user_message(status))
}

pub fn try_parse_stream_error(data: &str) -> Option<SamplingError> {
    let (error_type, message) = try_parse_error(data)?;
    tracing::warn!(error_type, message, "Server-side stream error");
    Some(SamplingError::StreamError {
        error_type,
        message,
    })
}

/// True when an error message indicates a context-window overflow. Backends report
/// this inconsistently with no stable error code, so we match the message text; it's
/// deterministic (re-sending the same payload always fails), so callers must not retry.
pub fn is_context_length_error(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("too long for this model")
        || m.contains("prompt is too long")
        || m.contains("maximum prompt length")
        || m.contains("maximum context length")
        || m.contains("context_length_exceeded")
}

/// Credit / spending-limit wording shared by xAI Build, OpenRouter, SuperGrok
/// Heavy subscription caps, and proxies.
///
/// SuperGrok **Heavy / usage limit** bodies (e.g. "SuperGrok Heavy usage limit")
/// are treated as credit-exhausted so dual-auth can hop to a console key with
/// sticky memo — not only plain 429 throttle. Keep tight: bare 403 and
/// "usage guidelines" policy text must not match.
pub fn is_credit_exhausted_message(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("out of credits")
        || m.contains("run out of credits")
        || m.contains("spending-limit")
        || m.contains("spending limit")
        || m.contains("usage balance exhausted")
        // Subscription / SuperGrok Heavy caps ("…usage limit", not only "…reached").
        || m.contains("usage limit")
        || m.contains("heavy limit")
        || m.contains("resource_exhausted")
        || m.contains("insufficient credits")
        || m.contains("insufficient_quota")
        || m.contains("payment required")
        || m.contains("credit balance is too low")
        || m.contains("exceeded your current quota")
        || m.contains("add credits")
}

fn is_credit_exhausted_status_and_message(status: u16, message: &str) -> bool {
    if status == 402 {
        return true;
    }
    // 403 is overloaded (policy, ZDR, credits). Only treat as credits when the
    // body says so — never promote bare 403 into failover.
    if matches!(status, 403 | 429 | 400) && is_credit_exhausted_message(message) {
        return true;
    }
    is_credit_exhausted_message(message) && status != 401
}

/// Strip the [`SamplingError::Api`] Display wrap
/// (`API error (status 403 Forbidden): ...`) so terminal copy is the body only.
pub fn strip_api_error_status_prefix(raw: &str) -> &str {
    let s = raw.trim();
    if let Some(rest) = s.strip_prefix("API error (status ")
        && let Some(idx) = rest.find("): ")
    {
        return rest[idx + 3..].trim();
    }
    s
}

/// Plain American English for team credit / monthly spending-limit failures.
///
/// Prefer the upstream team sentence when present. Never invent Internal error
/// JSON. Operators get admin guidance (add credits / raise monthly spend limit)
/// without confusing free SuperGrok period with console team credits.
pub fn credit_exhausted_user_message(raw: &str) -> String {
    let body = strip_api_error_status_prefix(raw).trim();
    if body.is_empty() {
        return TEAM_CREDIT_FALLBACK.to_string();
    }
    // Upstream already names team credits / monthly spending limit.
    let lower = body.to_ascii_lowercase();
    if lower.contains("monthly spending limit")
        || lower.contains("used all available credits")
        || (lower.contains("team") && lower.contains("credit"))
    {
        return body.to_string();
    }
    if is_credit_exhausted_message(body) {
        // Short bodies ("out of credits") get a plain admin line.
        return format!("{body}. {TEAM_CREDIT_FALLBACK}");
    }
    body.to_string()
}

const TEAM_CREDIT_FALLBACK: &str = "Your team has either used all available credits or \
reached its monthly spending limit. Add credits or raise the monthly spend limit on console.x.ai.";

/// Decide whether a [`reqwest::Error`] is worth retrying.
pub fn is_retryable_reqwest(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() {
        return true;
    }

    if err.is_status() {
        return matches!(
            err.status(),
            Some(status) if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
        );
    }

    if err.is_request() || err.is_body() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_length_error_matches_backend_variants() {
        for msg in [
            "This model's maximum prompt length is 256000 but the request contains 1500000",
            "The prompt is too long for this model's context window.",
            "none: The prompt is too long for this model's context window.",
            "This model's maximum context length is 200000 tokens",
            "invalid_request_error: prompt is too long: 300000 tokens > 200000 maximum",
            "error type: context_length_exceeded",
        ] {
            assert!(is_context_length_error(msg), "should match: {msg}");
        }
        for msg in ["rate limited", "internal server error", "connection reset"] {
            assert!(!is_context_length_error(msg), "should not match: {msg}");
        }
        // The method delegates for the Api/StreamError variants.
        let api = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "none: The prompt is too long for this model's context window.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(api.is_context_length_error());
        assert!(
            SamplingError::StreamError {
                error_type: "overloaded_error".into(),
                message: "prompt is too long".into(),
            }
            .is_context_length_error()
        );
        assert!(!SamplingError::Auth("nope".into()).is_context_length_error());
    }

    #[test]
    fn serialization_message_stays_serialization_and_non_retryable() {
        let err = SamplingError::serialization_message("bad payload at line 1 column 7");
        assert!(matches!(err, SamplingError::Serialization(_)));
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("bad payload at line 1 column 7"));
    }

    #[test]
    fn serialization_from_rendered_round_trips_display() {
        // Derived from a REAL error's Display so a template rewording cannot
        // silently desynchronize the strip from the prefix it mirrors.
        let original =
            SamplingError::Serialization(serde_json::from_str::<i32>("not a number").unwrap_err());
        let rendered = original.to_string();
        let rebuilt = SamplingError::serialization_from_rendered(&rendered);
        assert!(matches!(rebuilt, SamplingError::Serialization(_)));
        assert!(!rebuilt.is_retryable());
        assert_eq!(
            rebuilt.to_string(),
            rendered,
            "rendered Display must round-trip without double-prefixing"
        );
        // Bare (non-rendered) input gains the prefix exactly once.
        assert_eq!(
            SamplingError::serialization_from_rendered("bare message").to_string(),
            format!("{SERIALIZATION_DISPLAY_PREFIX}bare message"),
        );
    }

    #[test]
    fn idle_timeout_is_not_retryable() {
        let err = SamplingError::IdleTimeout { elapsed_secs: 300 };
        assert!(
            !err.is_retryable(),
            "IdleTimeout must not be retried — would cause 3× amplification"
        );
    }

    #[test]
    fn event_stream_error_is_retryable() {
        // Verify the existing contract hasn't changed — EventStreamError is retryable.
        let err = SamplingError::EventStreamError("connection reset".into());
        assert!(err.is_retryable());
    }

    #[test]
    fn idle_timeout_display() {
        let err = SamplingError::IdleTimeout { elapsed_secs: 120 };
        let msg = err.to_string();
        assert!(
            msg.contains("120s"),
            "Display should include elapsed_secs: {msg}"
        );
    }

    #[test]
    fn try_parse_stream_error_flat_format() {
        let data = r#"{"code":"The service is currently unavailable","error":"Service temporarily unavailable. The model did not respond to this request."}"#;
        let err = try_parse_stream_error(data).expect("should parse flat error");
        match err {
            SamplingError::StreamError {
                error_type,
                message,
            } => {
                assert_eq!(error_type, "The service is currently unavailable");
                assert_eq!(
                    message,
                    "Service temporarily unavailable. The model did not respond to this request."
                );
            }
            other => panic!("expected StreamError, got {other:?}"),
        }
    }

    #[test]
    fn try_parse_stream_error_valid_chunk_returns_none() {
        let data = r#"{"id":"abc","object":"chat.completion.chunk","created":0,"model":"test","choices":[]}"#;
        assert!(
            try_parse_stream_error(data).is_none(),
            "valid chunk should not be parsed as error"
        );
    }

    #[test]
    fn parse_error_bytes_flat_format() {
        let bytes =
            br#"{"code":"The service is currently unavailable","error":"Service temporarily unavailable."}"#;
        let msg = parse_error_bytes(bytes);
        assert_eq!(
            msg,
            "The service is currently unavailable: Service temporarily unavailable."
        );
    }

    #[test]
    fn parse_error_bytes_rejects_non_json_body() {
        let html = br#"<!DOCTYPE html>
<html lang="en-US">
<head><title>grok.com | 524: A timeout occurred</title></head>
<body><h1>A timeout occurred Error code 524</h1></body>
</html>"#;
        let msg = parse_error_bytes(html);
        assert_eq!(msg, "upstream error");
        // Plain non-JSON text is also rejected (no body sniffing).
        assert_eq!(
            parse_error_bytes(b"some random gateway text"),
            "upstream error"
        );
    }

    #[test]
    fn user_facing_api_error_message_maps_non_json_by_status() {
        let html = br#"<!DOCTYPE html><html><body>timeout</body></html>"#;
        let msg = user_facing_api_error_message(StatusCode::from_u16(524).unwrap(), html);
        assert_eq!(msg, status_user_message(StatusCode::from_u16(524).unwrap()));

        let msg_503 =
            user_facing_api_error_message(StatusCode::SERVICE_UNAVAILABLE, b"not json either");
        assert_eq!(
            msg_503,
            status_user_message(StatusCode::SERVICE_UNAVAILABLE)
        );
    }

    #[test]
    fn user_facing_keeps_json_error_message() {
        let bytes = br#"{"error":{"message":"rate limit exceeded","type":"rate_limit_error"}}"#;
        let msg = user_facing_api_error_message(StatusCode::TOO_MANY_REQUESTS, bytes);
        assert_eq!(msg, "rate_limit_error: rate limit exceeded");
    }

    #[test]
    fn structured_error_message_is_length_capped() {
        let long_msg = "x".repeat(MAX_USER_ERROR_BODY_CHARS + 50);
        let bytes = format!(r#"{{"error":{{"message":"{long_msg}","type":"server_error"}}}}"#);
        let msg = parse_error_bytes(bytes.as_bytes());
        assert!(msg.chars().count() <= MAX_USER_ERROR_BODY_CHARS + 1);
        assert!(msg.ends_with('\u{2026}'));
    }

    /// Regression test: 403 Forbidden must NOT be classified as an auth
    /// error. The proxy returns 403 for policy denials that are unrelated
    /// to the caller's credentials (content-safety blocks, ZDR-gated
    /// operations, or other usage-policy blocks). Misclassifying these as
    /// auth errors triggers a pointless OIDC
    /// refresh and surfaces as acp::Error::auth_required on the client,
    /// tearing down the session and risking an
    /// `invalid_grant_threshold`-triggered wipe of auth.json.
    #[test]
    fn forbidden_is_not_auth_error() {
        let err = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "Content violates usage guidelines.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            !err.is_auth_error(),
            "403 Forbidden must not be treated as an auth error"
        );
    }

    #[test]
    fn unauthorized_is_auth_error() {
        let err = SamplingError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid or expired credentials".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            err.is_auth_error(),
            "401 Unauthorized must be an auth error"
        );
    }

    #[test]
    fn auth_variant_is_auth_error() {
        let err = SamplingError::Auth("bad key".into());
        assert!(err.is_auth_error());
    }

    #[test]
    fn rate_limited_api_error_is_detected() {
        let err = SamplingError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Rate limit exceeded".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(err.is_rate_limited());
        assert!(err.is_retryable(), "429 should be retryable");
        assert!(!err.is_auth_error());
        assert!(!err.is_payload_too_large());
        assert!(
            !err.is_credit_exhausted(),
            "plain 429 is throttle, not credits"
        );
    }

    #[test]
    fn credit_exhausted_detects_402_and_wording() {
        let payment = SamplingError::Api {
            status: StatusCode::PAYMENT_REQUIRED,
            message: "Payment Required".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(payment.is_credit_exhausted());
        assert!(!payment.is_auth_error());

        let or_body = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "status 403: run out of credits".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(or_body.is_credit_exhausted());
        assert!(!or_body.is_auth_error());

        let build = SamplingError::Api {
            status: StatusCode::PAYMENT_REQUIRED,
            message: "Grok Build usage balance exhausted".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(build.is_credit_exhausted());

        let plain_403 = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "Content violates usage guidelines.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(!plain_403.is_credit_exhausted());

        let unauthorized = SamplingError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: "out of credits".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            !unauthorized.is_credit_exhausted(),
            "401 stays auth even if body mentions credits"
        );
    }

    /// SuperGrok Heavy / subscription usage-limit bodies that should hop to a
    /// console key (credit path + sticky memo), not sleep as plain throttle.
    ///
    /// Bare 403 and generic "usage guidelines" stay non-credit.
    #[test]
    fn credit_exhausted_detects_supergrok_heavy_and_usage_limit() {
        // Named fixtures: subscription Heavy cap without the older
        // "usage limit reached" / "out of credits" exact phrases.
        let heavy_403 = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message:
                "You have reached your SuperGrok Heavy usage limit. Upgrade or wait for reset."
                    .into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            heavy_403.is_credit_exhausted(),
            "SuperGrok Heavy usage limit must hop as credit-exhausted"
        );

        let heavy_429 = SamplingError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Heavy usage limit exceeded for this SuperGrok plan".into(),
            model_metadata: None,
            retry_after_secs: Some(60),
            should_retry: None,
        };
        assert!(
            heavy_429.is_credit_exhausted(),
            "Heavy usage limit on 429 is sticky credit, not plain rate-limit only"
        );
        // Still rate-limited status-wise; credit path runs first in the sampler.
        assert!(heavy_429.is_rate_limited());

        let plan_limit = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "resource_exhausted: monthly usage limit for grok-heavy".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            plan_limit.is_credit_exhausted(),
            "resource_exhausted + heavy monthly usage limit must hop"
        );

        // Must not over-broaden: bare 403 / policy text still no-hop.
        let bare_403 = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "Forbidden".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(!bare_403.is_credit_exhausted());

        let guidelines = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "Content violates usage guidelines.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            !guidelines.is_credit_exhausted(),
            "usage guidelines is not a credit/usage-limit cap"
        );
    }

    /// Exact console team dogfood body (2026-08-05): credits **or** monthly
    /// spending limit under HTTP 403 must classify as credit-exhausted hop.
    #[test]
    fn credit_exhausted_detects_console_team_monthly_spending_limit_403() {
        let team_body = "Your team 61fab250-b2c1-40cf-b5b8-628e673a2eeb has either \
            used all available credits or reached its monthly spending limit. \
            Please contact your team admin to purchase more credits or raise \
            the spending limit.";
        let err = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: team_body.into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            err.is_credit_exhausted(),
            "console team credits/monthly spending limit 403 must hop as credit-exhausted"
        );
        assert!(!err.is_auth_error());

        // Bare 403 / usage guidelines still false (policy / ZDR).
        let bare = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "Forbidden".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(!bare.is_credit_exhausted());
        let guidelines = SamplingError::Api {
            status: StatusCode::FORBIDDEN,
            message: "Content violates usage guidelines.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(!guidelines.is_credit_exhausted());

        // Plain terminal copy strips Display wrap and keeps the team sentence.
        let wrapped = format!("API error (status 403 Forbidden): {team_body}");
        let plain = credit_exhausted_user_message(&wrapped);
        assert!(
            plain.contains("used all available credits")
                || plain.contains("monthly spending limit"),
            "plain copy must keep team sentence: {plain}"
        );
        assert!(
            !plain.contains("API error (status"),
            "must not keep Display status prefix: {plain}"
        );
        assert!(
            !plain.contains("Internal error"),
            "must not invent Internal error chrome: {plain}"
        );
    }

    #[test]
    fn non_rate_limit_errors_are_not_rate_limited() {
        let server_error = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(!server_error.is_rate_limited());

        let auth_error = SamplingError::Auth("bad key".into());
        assert!(!auth_error.is_rate_limited());

        let timeout = SamplingError::IdleTimeout { elapsed_secs: 30 };
        assert!(!timeout.is_rate_limited());
    }

    #[test]
    fn retry_after_returns_header_value() {
        let err = SamplingError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow down".into(),
            model_metadata: None,
            retry_after_secs: Some(42),
            should_retry: None,
        };
        assert_eq!(err.retry_after(), Some(42));
    }

    #[test]
    fn retry_after_returns_none_when_absent() {
        let err = SamplingError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow down".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert_eq!(err.retry_after(), None);
    }

    #[test]
    fn retry_after_returns_none_for_non_api_errors() {
        assert_eq!(SamplingError::Auth("x".into()).retry_after(), None);
        assert_eq!(
            SamplingError::IdleTimeout { elapsed_secs: 10 }.retry_after(),
            None
        );
    }

    #[test]
    fn encrypted_content_400_is_detected() {
        let err = SamplingError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Could not decrypt the provided encrypted_content. Ensure the value is the unmodified encrypted_content from a previous response.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(err.is_encrypted_content_error());
        assert!(
            !err.is_retryable(),
            "encrypted_content errors must not be retried"
        );
    }

    #[test]
    fn encrypted_content_wrong_status_not_detected() {
        let err = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "encrypted_content decryption failed".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            !err.is_encrypted_content_error(),
            "only 400 should match, not 500"
        );
    }

    #[test]
    fn encrypted_content_unrelated_400_not_detected() {
        let err = SamplingError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Invalid model parameter".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            !err.is_encrypted_content_error(),
            "unrelated 400 errors must not match"
        );
    }

    #[test]
    fn image_processing_error_direct_400_detected() {
        let err = SamplingError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Could not process image: unsupported format".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(err.is_image_processing_error());
        assert!(!err.is_encrypted_content_error());
    }

    #[test]
    fn image_processing_error_500_wrapped_detected() {
        let err = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "upstream error: 400 Bad Request: Could not process image".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(err.is_image_processing_error());
    }

    #[test]
    fn image_processing_error_unrelated_400_not_detected() {
        let err = SamplingError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Invalid model parameter".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(!err.is_image_processing_error());
    }

    #[test]
    fn image_processing_error_unrelated_500_not_detected() {
        let err = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(!err.is_image_processing_error());
    }

    #[test]
    fn image_processing_error_wrong_status_not_detected() {
        let err = SamplingError::Api {
            status: StatusCode::BAD_GATEWAY,
            message: "Could not process image".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            !err.is_image_processing_error(),
            "only 400 and 500 should match"
        );
    }

    #[test]
    fn image_processing_error_400_is_not_retryable_standalone() {
        let err = SamplingError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Could not process image".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        };
        assert!(
            !err.is_retryable(),
            "direct 400 must not be retryable by is_retryable()"
        );
    }

    fn api_status(code: u16, message: &str) -> SamplingError {
        SamplingError::Api {
            status: StatusCode::from_u16(code).expect("valid status"),
            message: message.into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
        }
    }

    /// Cloudflare 521 (origin down) and sibling edge outages must soft-retry
    /// with backoff — not Fatal on first sight as "unknown status".
    #[test]
    fn cloudflare_edge_outage_statuses_are_retryable() {
        for code in [520u16, 521, 522, 523, 524, 525, 526, 527, 530] {
            assert!(
                is_transient_api_status(code),
                "status {code} must be transient"
            );
            let err = api_status(code, "edge outage");
            assert!(
                err.is_retryable(),
                "HTTP {code} must be retryable (was only 520 historically)"
            );
            assert!(
                !err.is_credit_exhausted(),
                "HTTP {code} is network/outage, not credit exhaust"
            );
            assert!(!err.is_rate_limited(), "HTTP {code} is not a 429 throttle");
        }
    }

    #[test]
    fn http_521_display_uses_known_label_not_unknown_status() {
        let body = status_user_message(StatusCode::from_u16(521).unwrap());
        let err = api_status(521, &body);
        let s = err.to_string();
        assert!(
            !s.contains("unknown status"),
            "must not print unknown status code: {s}"
        );
        assert!(
            s.contains("521") && s.contains("Web Server Is Down"),
            "expected known 521 label in Display: {s}"
        );
        assert_eq!(
            http_status_label(521),
            Some("Web Server Is Down"),
            "status map entry for 521"
        );
        assert_eq!(
            format_http_status(StatusCode::from_u16(521).unwrap()),
            "521 Web Server Is Down"
        );
    }

    #[test]
    fn outage_exhausted_message_is_plain_english() {
        let msg = outage_exhausted_user_message(StatusCode::from_u16(521).unwrap(), 4);
        assert_eq!(
            msg,
            "xAI connection failed after 4 tries (HTTP 521). Try again shortly."
        );
        // attempts=0 still reads as one try (surface never claims zero tries).
        let once = outage_exhausted_user_message(StatusCode::from_u16(521).unwrap(), 0);
        assert_eq!(
            once,
            "xAI connection failed after 1 try (HTTP 521). Try again shortly."
        );
    }

    #[test]
    fn format_http_status_keeps_iana_reason_for_standard_codes() {
        assert_eq!(
            format_http_status(StatusCode::TOO_MANY_REQUESTS),
            "429 Too Many Requests"
        );
        assert_eq!(
            format_http_status(StatusCode::BAD_GATEWAY),
            "502 Bad Gateway"
        );
    }

    #[test]
    fn non_transient_5xx_like_501_not_retryable() {
        let err = api_status(501, "not implemented");
        assert!(!is_transient_api_status(501));
        assert!(!err.is_retryable());
    }
}
