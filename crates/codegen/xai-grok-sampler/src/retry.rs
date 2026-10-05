use std::time::Duration;

use xai_grok_sampling_types::{
    SamplingError, is_edge_outage_status, is_retryable_api_status, outage_exhausted_user_message,
};

pub const RATE_LIMIT_RETRY_THRESHOLD: u32 = 2;

pub const RATE_LIMIT_RETRY_DISABLED: u32 = 1;

pub const DEFAULT_MAX_RETRIES: u32 = 15;

pub const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(30);

pub const TRANSPORT_REBUILD_BACKOFF: Duration = Duration::from_millis(200);

/// Under-window budget for 5xx, timeouts, and stream interrupts when the
/// caller passes an unlimited retry count (`u32::MAX`). A finite budget
/// stays that caller's `max_retries`. [`DEFAULT_MAX_RETRIES`] stays 15.
pub const DEFAULT_TRANSPORT_MAX_RETRIES: u32 = 3;

/// Jittered exponential backoff ceiling in seconds. Matches [`MAX_RETRY_BACKOFF`].
pub const MAX_BACKOFF_SECS: u64 = 30;

/// True only for an unlimited retry count. Fifteen retries is a finite budget.
pub fn is_unlimited_retries(max_retries: u32) -> bool {
    max_retries == u32::MAX
}

fn transport_retry_cap(max_retries: u32) -> u32 {
    if is_unlimited_retries(max_retries) {
        DEFAULT_TRANSPORT_MAX_RETRIES
    } else {
        max_retries
    }
}

pub(crate) fn resolve_max_retries_with_env(
    env_override: Option<&str>,
    model_max_retries: Option<u32>,
) -> u32 {
    env_override
        .and_then(|value| value.parse::<u32>().ok())
        .or(model_max_retries)
        .unwrap_or(DEFAULT_MAX_RETRIES)
}

pub fn resolve_max_retries(model_max_retries: Option<u32>) -> u32 {
    let env_override = std::env::var("GROK_MAX_RETRIES").ok();
    resolve_max_retries_with_env(env_override.as_deref(), model_max_retries)
}

pub fn doom_loop_backoff(retry_count: u32) -> Duration {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};

    static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

    let mut hasher = std::hash::DefaultHasher::new();
    JITTER_SEQ.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    retry_count.hash(&mut hasher);
    Duration::from_millis(hasher.finish() % 251)
}

pub fn retry_backoff_with_jitter(retry_count: u32) -> Duration {
    let shift = retry_count.saturating_sub(1);
    let base_ms = 2000u64
        .checked_shl(shift)
        .unwrap_or(u64::MAX)
        .min(MAX_RETRY_BACKOFF.as_millis() as u64);
    jitter_backoff(Duration::from_millis(base_ms))
}

pub fn jitter_backoff(base: Duration) -> Duration {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};

    static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

    let base_ms = base.as_millis() as u64;
    let jitter_range = base_ms / 5;
    let mut hasher = std::hash::DefaultHasher::new();
    JITTER_SEQ.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let jitter = hasher.finish() % (jitter_range * 2 + 1);
    Duration::from_millis(base_ms - jitter_range + jitter)
}

pub fn retry_after_or_backoff(attempt: u32, retry_after_secs: Option<u64>) -> Duration {
    match retry_after_secs.filter(|secs| *secs > 0) {
        Some(secs) => jitter_backoff(Duration::from_secs(secs).min(MAX_RETRY_BACKOFF)),
        None => retry_backoff_with_jitter(attempt),
    }
}

#[derive(Debug)]
pub enum RetryDecision {
    Retry {
        backoff: Duration,
    },

    RetryWithBackoff {
        backoff: Duration,
        is_rate_limited: bool,
    },

    RetryWithImageStrip,

    RetryWithClientRebuild {
        backoff: Duration,
    },

    EmitToSession(SamplingError),

    Fatal(SamplingError),
}

pub fn classify_error(
    err: &SamplingError,
    retry_count: u32,
    max_retries: u32,
    rate_limit_threshold: u32,
) -> RetryDecision {
    if err.is_auth_error() {
        return RetryDecision::EmitToSession(clone_error(err));
    }
    if err.is_encrypted_content_error() {
        return RetryDecision::EmitToSession(clone_error(err));
    }
    if max_retries == 0 {
        return RetryDecision::Fatal(clone_error(err));
    }

    // Token overflows fail fast via the retry veto below; byte-coded rejections (413 or a byte-size code) strip images and retry
    if err.is_payload_too_large() || err.is_byte_size_overflow_coded() {
        return RetryDecision::RetryWithImageStrip;
    }

    if err.is_image_processing_error() {
        return RetryDecision::RetryWithImageStrip;
    }

    if err.is_retry_vetoed() {
        return RetryDecision::Fatal(clone_error(err));
    }

    if matches!(err, SamplingError::DoomLoopDetected { .. }) {
        return RetryDecision::Retry {
            backoff: doom_loop_backoff(retry_count + 1),
        };
    }

    if err.is_rate_limited() {
        let next_attempt = retry_count + 1;
        if !is_unlimited_retries(max_retries)
            && next_attempt >= max_retries.min(rate_limit_threshold)
        {
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

    if err.is_retryable() {
        let next_attempt = retry_count.saturating_add(1);
        let cap = transport_retry_cap(max_retries);
        if next_attempt >= cap {
            return RetryDecision::Fatal(clone_error(err));
        }
        if next_attempt == 1 {
            let backoff = match err {
                SamplingError::Http(_) => jitter_backoff(TRANSPORT_REBUILD_BACKOFF),
                _ => retry_after_or_backoff(next_attempt, err.retry_after()),
            };
            return RetryDecision::RetryWithClientRebuild { backoff };
        }
        return RetryDecision::Retry {
            backoff: retry_after_or_backoff(next_attempt, err.retry_after()),
        };
    }

    RetryDecision::Fatal(clone_error(err))
}

/// True when used tokens are at or over this session's sampling window.
/// The same oversized payload cannot recover by retrying.
pub fn over_window_blocks_retry(used_tokens: Option<u64>, sampling_window: u64) -> bool {
    sampling_window > 0 && used_tokens.is_some_and(|used| used >= sampling_window)
}

/// Classify a sampling error, refusing retries when the request is already
/// at or over the session sampling window.
///
/// Auth and encrypted-content errors still emit to the session. Image-strip
/// recovery may still change the payload. Doom-loop resample is not a size overflow.
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
        SamplingError::MtlsConfiguration(msg) => {
            format!(
                "{}Invalid mTLS configuration: {}. Please check the model endpoint and certificate directory.",
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
                _ if is_retryable_api_status(*status) => " (server unavailable - please retry)",
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
        SamplingError::RepetitiveGeneration { .. } => {
            format!(
                "{}{}",
                retry_prefix,
                xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE
            )
        }
    }
}

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
        SamplingError::MtlsConfiguration(msg) => SamplingError::MtlsConfiguration(msg.clone()),
        SamplingError::Http(e) => SamplingError::EventStreamError(e.to_string()),
        SamplingError::Serialization(e) => SamplingError::serialization_message(e),
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
        SamplingError::RepetitiveGeneration {
            channel,
            aborted_at_chunk,
        } => SamplingError::RepetitiveGeneration {
            channel: channel.clone(),
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
        assert!(
            backoff >= Duration::from_millis(1600) && backoff <= Duration::from_millis(2400),
            "first retry backoff out of range: {:?}",
            backoff
        );
    }

    #[test]
    fn backoff_doubles_then_caps_at_thirty_seconds() {
        let r2 = retry_backoff_with_jitter(2);
        assert!(r2 >= Duration::from_millis(3200) && r2 <= Duration::from_millis(4800));

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
    fn classify_many_image_dimension_400_strips_images() {
        let err = api_err(
            StatusCode::BAD_REQUEST,
            "invalid_request_error: messages.0.content.4.image.source.base64.data: \
             At least one of the image dimensions exceed max allowed size for \
             many-image requests: 2000 pixels",
        );
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
    fn tpm_429_with_retry_after_backs_off_despite_size_text() {
        // A TPM 429 often carries size wording ("Request too large for model...") plus a Retry-After promising capacity later
        // The size veto must not fast-fail it
        let err = SamplingError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Request too large for model: Limit 30000, Requested 50000 tokens per min"
                .to_string(),
            model_metadata: None,
            retry_after_secs: Some(7),
            should_retry: None,
            error_code: None,
        };
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                is_rate_limited, ..
            } => assert!(is_rate_limited),
            other => panic!("expected RetryWithBackoff, got {other:?}"),
        }
        // Without Retry-After the same message fast-fails: the request exceeds the per-minute cap outright and retrying is futile
        let no_retry_after = api_err(
            StatusCode::TOO_MANY_REQUESTS,
            "Request too large for model: Limit 30000, Requested 50000 tokens per min",
        );
        assert!(matches!(
            classify_error(&no_retry_after, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn rate_limit_retry_layer_splits_by_threshold() {
        let err = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 5);
        assert!(
            matches!(
                classify_error(&err, 0, 15, RATE_LIMIT_RETRY_DISABLED),
                RetryDecision::Fatal(_)
            ),
            "disabled threshold must surface the first 429, not wait internally"
        );
        assert!(
            matches!(
                classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
                RetryDecision::RetryWithBackoff {
                    is_rate_limited: true,
                    ..
                }
            ),
            "default threshold keeps the sampler's own 429 retry"
        );
    }

    #[test]
    fn classify_rate_limited_capped_at_threshold() {
        let err = api_err(StatusCode::TOO_MANY_REQUESTS, "slow");
        match classify_error(&err, 1, 5, RATE_LIMIT_RETRY_THRESHOLD) {
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

    #[tokio::test]
    async fn transport_failure_first_retry_skips_the_server_backoff() {
        const CONNECT_GUARD: Duration = Duration::from_secs(5);

        let send_err = tokio::time::timeout(CONNECT_GUARD, reqwest::get("http://127.0.0.1:0"))
            .await
            .expect("port 0 connect fails well within the guard")
            .expect_err("connecting to port 0 must fail");
        let err = SamplingError::Http(send_err);

        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => assert!(
                backoff >= Duration::from_millis(160) && backoff <= Duration::from_millis(240),
                "transport rebuild must not wait the 2s server backoff: {backoff:?}"
            ),
            other => panic!("expected RetryWithClientRebuild, got {other:?}"),
        }

        match classify_error(&err, 1, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { backoff } => {
                assert!(backoff >= Duration::from_millis(3200), "{backoff:?}");
            }
            other => panic!("expected Retry, got {other:?}"),
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

    #[test]
    fn classify_clamps_and_jitters_retry_after_on_generic_path_but_not_on_429() {
        let edge = api_err_with_retry_after(StatusCode::from_u16(522).unwrap(), 120);
        match classify_error(&edge, 1, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { backoff } => {
                assert!(backoff >= Duration::from_secs(24), "got {backoff:?}");
                assert!(backoff <= Duration::from_secs(36), "got {backoff:?}");
            }
            other => panic!("expected Retry for 522, got {other:?}"),
        }

        let rate_limited = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 120);
        match classify_error(&rate_limited, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff { backoff, .. } => {
                assert_eq!(backoff, Duration::from_secs(120));
            }
            other => panic!("expected RetryWithBackoff for 429, got {other:?}"),
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
        if let RetryDecision::RetryWithBackoff {
            is_rate_limited: true,
            ..
        } = classify_error(&err, 0, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD)
        {
            panic!("502 must not take the 429 hop/wait path");
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
    fn drifted_size_overflow_wordings_are_fatal_on_turn_path() {
        // Size-worded errors with no code must fail fast, not burn the retry budget
        let api_500 = SamplingError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "exceed_context_size_error: request (300000 tokens) exceeds the model \
                      context size"
                .into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&api_500, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));

        let stream = SamplingError::StreamError {
            error_type: "BAD_REQUEST".into(),
            message: "Input length (300000 tokens) exceeds the maximum allowed length \
                      (200000 tokens)"
                .into(),
            code: None,
        };
        assert!(matches!(
            classify_error(&stream, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));

        // Token-tier code with an opaque message: fatal, no strip.
        let coded = SamplingError::StreamError {
            error_type: "BAD_REQUEST".into(),
            message: "request rejected".into(),
            code: Some(xai_grok_sampling_types::ApiErrorCode::parse(
                "exceed_context_size_error",
            )),
        };
        assert!(matches!(
            classify_error(&coded, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn byte_size_coded_errors_get_image_strip_before_the_veto() {
        // Byte-size codes get the 413 remedy: strip images and retry once; the caller upgrades to Fatal when nothing is left to strip
        for code in ["413", "payload_too_large", "request_too_large"] {
            let coded = SamplingError::StreamError {
                error_type: "BAD_REQUEST".into(),
                message: "request rejected".into(),
                code: Some(xai_grok_sampling_types::ApiErrorCode::parse(code)),
            };
            assert!(
                matches!(
                    classify_error(&coded, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
                    RetryDecision::RetryWithImageStrip
                ),
                "expected image strip for coded {code}"
            );
        }
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

    /// Surmount fork: `RepetitiveGeneration` is Fatal (stop the turn).
    /// SpaceXAI `DoomLoopDetected` still retries / resamples thinking.
    /// Do not treat a dest-encoder-skip assistant wall as resample.
    #[test]
    fn classify_repetitive_generation_is_fatal() {
        let err = SamplingError::RepetitiveGeneration {
            channel: "text".into(),
            aborted_at_chunk: Some(3),
        };
        match classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(fatal) => {
                assert!(!fatal.is_retryable());
                assert!(fatal.to_string().contains("repeating the same sentence"));
            }
            other => panic!("expected Fatal, got {other:?}"),
        }
    }

    #[test]
    fn classify_doom_loop_detected_is_retry_with_immediate_backoff() {
        let err = SamplingError::DoomLoopDetected {
            triggers: vec!["tail_repetition:8@thinking".into()],
            aborted_at_chunk: None,
        };
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

    /// Named contract: HTTP 500 `Internal error during token generation` retries
    /// with the transport cap, not unlimited 429, not context-length Fatal, not
    /// idle/serialization. Attempt 2 must not sit on an unbounded wait.
    #[test]
    fn classify_500_token_generation_retries_with_transport_cap() {
        let body = "error: Internal error during token generation";
        let err = api_err(StatusCode::INTERNAL_SERVER_ERROR, body);
        assert!(
            err.to_string()
                .contains("API error (status 500 Internal Server Error): error: Internal error during token generation"),
            "Display must quote the operator 500: {}",
            err
        );
        assert!(err.is_retryable());
        assert!(err.is_token_generation_internal_error());
        assert!(!err.is_context_length_error());
        assert!(!err.is_rate_limited());
        assert!(!matches!(err, SamplingError::IdleTimeout { .. }));
        assert!(!matches!(err, SamplingError::Serialization(_)));
        match classify_error(&err, 0, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => {
                assert!(
                    backoff < Duration::from_secs(60),
                    "first token-generation 500 backoff must stay under a minute, got {backoff:?}"
                );
            }
            other => panic!("first token-generation 500 must retry, got {other:?}"),
        }
        match classify_error(&err, 1, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { backoff } => {
                assert!(
                    backoff < Duration::from_secs(120),
                    "attempt 2 token-generation 500 must not sit ~11 minutes, got {backoff:?}"
                );
            }
            other => panic!("attempt 2 token-generation 500 must Retry, got {other:?}"),
        }
        match classify_error(
            &err,
            DEFAULT_TRANSPORT_MAX_RETRIES.saturating_sub(1),
            u32::MAX,
            RATE_LIMIT_RETRY_THRESHOLD,
        ) {
            RetryDecision::Fatal(SamplingError::Api {
                status, message, ..
            }) => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
                assert!(
                    message.contains("Internal error during token generation"),
                    "exhausted 500 must keep the named cause, got {message}"
                );
            }
            other => panic!(
                "unlimited budget must still stop token-generation 500 after the transport cap, got {other:?}"
            ),
        }
        if let RetryDecision::RetryWithBackoff {
            is_rate_limited: true,
            ..
        } = classify_error(&err, 0, u32::MAX, RATE_LIMIT_RETRY_THRESHOLD)
        {
            panic!("token-generation 500 must not take the unlimited 429 path");
        }
    }
}
