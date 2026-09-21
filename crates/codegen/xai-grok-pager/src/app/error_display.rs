//! User-facing formatting for terminal request / API errors.
//!
//! Turns raw ACP / `RetryState` dumps (`API error (status 500): {"error":…}`) into the same kind of short warning banner used for 401 re-auth.

/// Wire `RetryState::Failed.error_type` values the pager understands.
/// The vocabulary is the shell's `SamplingErrorKind::as_str` tags plus its special-cased tags (`context_length`, `legacy_auth`, …).
/// Unknown strings map to [`WireErrorType::Other`] rather than being matched as raw `&str` at call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireErrorType {
    Auth,
    AuthTransient,
    LegacyAuth,
    ContextLength,
    EncryptedContentMismatch,
    DiskFull,
    Api,
    Http,
    IdleTimeout,
    EmptyResponse,
    Serialization,
    RateLimited,
    MaxTokensTruncation,
    RepetitiveGeneration,
    Other,
}

impl WireErrorType {
    /// Only the pager-special tags are matched here; the shared vocabulary routes through `SamplingErrorKind`'s `FromStr`.
    /// That keeps a single string table, so a kind added there classifies here automatically.
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        let Some(s) = raw else {
            return Self::Other;
        };
        match s {
            "auth_transient" => Self::AuthTransient,
            "legacy_auth" => Self::LegacyAuth,
            s if s == xai_grok_shell::extensions::notification::CONTEXT_LENGTH_ERROR_TYPE => {
                Self::ContextLength
            }
            "encrypted_content_mismatch" => Self::EncryptedContentMismatch,
            s if s == xai_grok_shell::extensions::notification::DISK_FULL_ERROR_TYPE => {
                Self::DiskFull
            }
            s => s
                .parse::<xai_grok_shell::sampling::error::SamplingErrorKind>()
                .map(Into::into)
                .unwrap_or(Self::Other),
        }
    }
}

/// Absent stays `None`, which keeps the untyped-text recovery available; every place a kind comes off the wire uses this mapping.
/// An unknown kind (a newer shell's) parses to `Some(Other)`, so it is never text-sniffed into a different classification.
pub(crate) fn wire_error_kind(raw: Option<&str>) -> Option<WireErrorType> {
    raw.map(|s| WireErrorType::parse(Some(s)))
}

/// The shared vocabulary maps 1:1 onto the pager's wire types; kinds without their own copy render as [`Self::Other`].
impl From<xai_grok_shell::sampling::error::SamplingErrorKind> for WireErrorType {
    fn from(kind: xai_grok_shell::sampling::error::SamplingErrorKind) -> Self {
        use xai_grok_shell::sampling::error::SamplingErrorKind as K;
        match kind {
            K::Auth => Self::Auth,
            K::Http => Self::Http,
            K::Api => Self::Api,
            K::Serialization => Self::Serialization,
            K::IdleTimeout => Self::IdleTimeout,
            K::RateLimited => Self::RateLimited,
            K::EmptyResponse => Self::EmptyResponse,
            K::MaxTokensTruncation => Self::MaxTokensTruncation,
            K::DoomLoopDetected => Self::Other,
        }
    }
}

/// Clean banner for a terminal request failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormattedRequestFailure {
    pub status: Option<u16>,
    pub headline: String,
    pub detail: String,
    pub(crate) wire: WireErrorType,
}

/// `Headline: detail` (headline alone when there is no detail).
/// Shared with the scrollback block so the two renderings can't drift.
pub(crate) fn banner_message(headline: &str, detail: &str) -> String {
    if detail.is_empty() {
        headline.to_string()
    } else {
        format!("{headline}: {detail}")
    }
}

impl FormattedRequestFailure {
    pub(crate) fn message(&self) -> String {
        banner_message(&self.headline, &self.detail)
    }

    pub(crate) fn into_session_event(self) -> crate::scrollback::blocks::SessionEvent {
        crate::scrollback::blocks::SessionEvent::RequestFailed {
            status: self.status,
            headline: self.headline,
            detail: self.detail,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum RetryLabelStyle {
    /// Composer / turn-status: `Retrying (attempt N)...`
    Status,
    /// Title bar and subagent activity: `Retrying (N/M)`
    Compact,
}

/// `{headline} | Retrying …` using [`format_request_failure`] headlines, else the bare retry clause.
pub(crate) fn format_retry_activity_label(
    attempt: u32,
    max_retries: u32,
    reason: &str,
    error_type: Option<&str>,
    style: RetryLabelStyle,
) -> String {
    let base = retry_clause(attempt, max_retries, style);
    match classified_retry_headline(reason, error_type) {
        Some(headline) => format!("{headline} | {base}"),
        None => base,
    }
}

pub(crate) fn retry_clause(attempt: u32, max_retries: u32, style: RetryLabelStyle) -> String {
    // Longer headline plus U+2026 wraps this status row into the prompt.
    match style {
        RetryLabelStyle::Status => format!("Retrying (attempt {attempt})..."),
        RetryLabelStyle::Compact => format!("Retrying ({attempt}/{max_retries})"),
    }
}

fn classified_retry_headline(reason: &str, error_type: Option<&str>) -> Option<String> {
    let reason = reason.trim();
    let kind = wire_error_kind(error_type);
    if reason.is_empty() && kind.is_none() {
        return None;
    }
    let formatted = format_request_failure(None, kind, reason);
    let generic = formatted.status.is_none() && matches!(formatted.wire, WireErrorType::Other);
    if generic {
        None
    } else {
        Some(formatted.headline)
    }
}

/// Otherwise the status is recovered from the message text.
/// Server text is kept only when it adds information.
/// A status-level next step is always kept when we have one.
pub(crate) fn format_request_failure(
    status: Option<u16>,
    error_type: Option<WireErrorType>,
    raw: &str,
) -> FormattedRequestFailure {
    let untyped = error_type.is_none();
    let wire = if truncation_recovered_from_untyped_raw(error_type, raw) {
        WireErrorType::MaxTokensTruncation
    } else {
        error_type.unwrap_or(WireErrorType::Other)
    };
    // A sniffed status must not demote a dedicated wire-type headline to generic status copy
    // An `auth_transient` message contains "Unauthorized (401)", so only `Api` and `Other` recover a status from the text
    let status = status.or_else(|| {
        matches!(wire, WireErrorType::Api | WireErrorType::Other)
            .then(|| parse_http_status(raw))
            .flatten()
    });
    let wire = refine_untyped_wire(wire, untyped, status, raw);
    let extracted = extract_error_detail(raw);
    if is_headers_timeout_cold_start(raw)
        || extracted
            .as_deref()
            .is_some_and(is_headers_timeout_cold_start)
    {
        return FormattedRequestFailure {
            status,
            headline: "Cold start: response headers timed out".to_string(),
            detail: "The model host may still be starting. Retry is in progress. This is not Thought-only."
                .to_string(),
        };
    }
    if is_image_transcription_transport_miss(raw)
        || extracted
            .as_deref()
            .is_some_and(is_image_transcription_transport_miss)
    {
        return FormattedRequestFailure {
            status,
            headline: "Image transcription unavailable".to_string(),
            detail: "Transport miss: error sending request. The Human image line stays. This is not a silent hang. Try sending again."
                .to_string(),
        };
    }
    if is_transport_send_miss(raw) || extracted.as_deref().is_some_and(is_transport_send_miss) {
        return FormattedRequestFailure {
            status,
            headline: "Connection failed".to_string(),
            detail: "Transport miss: error sending request. This is not a silent hang. Check your network and try again."
                .to_string(),
        };
    }
    let team_prepaid = xai_grok_sampling_types::is_console_team_prepaid_message(raw)
        || extracted
            .as_deref()
            .is_some_and(xai_grok_sampling_types::is_console_team_prepaid_message);
    if team_prepaid {
        let code = status.unwrap_or(403);
        return FormattedRequestFailure {
            status: Some(code),
            headline: format!("Request denied ({code})"),
            detail: xai_grok_sampling_types::console_team_prepaid_stay_on_supergrok_user_message()
                .to_string(),
        };
    }
    // Safety refusal bodies (gRPC-style `permission-denied: I can't help with
    // that request.`) sometimes ride HTTP 403. That is not endpoint forbid
    // and not a tool-permission hub deny. Do not headline Request denied (403).
    if is_safety_refusal_message(raw) || extracted.as_deref().is_some_and(is_safety_refusal_message)
    {
        return FormattedRequestFailure {
            status: None,
            headline: "Safety refusal".to_string(),
            detail: safety_refusal_detail(extracted.as_deref().unwrap_or(raw)),
        };
    }
    let class = classify(status, wire);
    let why = extracted
        .filter(|d| !is_server_fault(status, wire) && !is_headline_echo(d, &class.headline))
        .or_else(|| class.default_why.map(str::to_string));
    let detail = compose_detail(why.as_deref(), class.action);
    FormattedRequestFailure {
        status,
        headline: class.headline,
        detail,
        wire,
    }
}

/// A present-but-unknown error type is a newer shell's kind and is never reclassified.
/// TODO: error-kind-fallback-removal — this recovery is a version shim for terminals that predate the typed `errorKind`/`error_kind` fields.
/// It is also the only truncation classifier for the exhausted-retry path, which passes no error type at all.
fn truncation_recovered_from_untyped_raw(error_type: Option<WireErrorType>, raw: &str) -> bool {
    error_type.is_none()
        && parse_http_status(raw).is_none()
        && raw.contains(xai_grok_shell::sampling::error::MAX_TOKENS_TRUNCATION_MESSAGE)
}

fn refine_untyped_wire(
    wire: WireErrorType,
    untyped: bool,
    status: Option<u16>,
    raw: &str,
) -> WireErrorType {
    match wire {
        WireErrorType::Other if untyped && status.is_none() => {
            http_wire_from_dump(raw).unwrap_or(wire)
        }
        typed => typed,
    }
}

fn http_wire_from_dump(raw: &str) -> Option<WireErrorType> {
    // SamplingError::Http Display is `request error: {source}`.
    raw.trim()
        .starts_with("request error:")
        .then_some(WireErrorType::Http)
}

struct Classified {
    headline: String,
    /// What the user can do. Omitted when we have no real next step.
    action: Option<&'static str>,
    /// Used only when the server body added nothing.
    default_why: Option<&'static str>,
}

fn classify(status: Option<u16>, wire: WireErrorType) -> Classified {
    if let Some(code) = status {
        let (prefix, action, default_why) = match code {
            400 | 422 => (
                "Bad request",
                None,
                Some("The server rejected this request."),
            ),
            403 => (
                "Request denied",
                None,
                Some("You don't have permission to do this."),
            ),
            404 => (
                "Not found",
                Some("Run /model to pick another."),
                Some("This model isn't available."),
            ),
            408 | 504 => (
                "Request timed out",
                Some("Try again shortly."),
                Some("The server took too long to respond."),
            ),
            409 => (
                "Conflict",
                Some("Try again."),
                Some("The request conflicted with the current state."),
            ),
            413 => (
                "Request too large",
                Some("Try a smaller prompt or run /compact."),
                None,
            ),
            429 => (
                "Rate limited",
                Some("Try again later."),
                Some("You've hit the rate limit for your plan."),
            ),
            502 | 503 => (
                "Service unavailable",
                Some("The service is busy. Wait a minute and send again."),
                None,
            ),
            100..=399 => (
                "Request failed",
                None,
                Some("The request did not complete successfully."),
            ),
            400..=499 => (
                "Request failed",
                None,
                Some("The server rejected this request."),
            ),
            _ => (
                "Server error",
                Some("Something went wrong on our side. Wait a minute and send again."),
                None,
            ),
        };
        return Classified {
            headline: format!("{prefix} ({code})"),
            action,
            default_why,
        };
    }
    let (headline, action, default_why) = match wire {
        WireErrorType::IdleTimeout => (
            "No response from the model",
            Some("Try sending again."),
            Some("It may be stuck."),
        ),
        WireErrorType::EmptyResponse => (
            "Empty response",
            Some("Try sending again."),
            Some("The model returned no content."),
        ),
        WireErrorType::Serialization => (
            "Couldn't read the response",
            Some("Try sending again."),
            None,
        ),
        WireErrorType::Http => (
            "Connection failed",
            Some("Check your network and try again."),
            None,
        ),
        WireErrorType::MaxTokensTruncation => (
            "Response truncated",
            None,
            Some("The model hit its output limit."),
        ),
        WireErrorType::RepetitiveGeneration => (
            "Stopped: repeating sentence",
            None,
            Some("The reply was repeating the same sentence."),
        ),
        WireErrorType::RateLimited => (
            "Rate limited",
            Some("Try again later."),
            Some("You've hit the rate limit for your plan."),
        ),
        WireErrorType::Api => (
            "Server error",
            Some("Something went wrong on our side. Wait a minute and send again."),
            None,
        ),
        WireErrorType::AuthTransient => (
            "Authentication temporarily unavailable",
            Some("Try sending again in a moment."),
            None,
        ),
        _ => (
            "Request failed",
            Some("Try sending again."),
            Some("Something went wrong."),
        ),
    };
    Classified {
        headline: headline.to_string(),
        action,
        default_why,
    }
}

/// `why. action`, deduplicating (ignoring case/punctuation) when one already contains the other.
fn compose_detail(why: Option<&str>, action: Option<&str>) -> String {
    match (why, action) {
        (None, None) => String::new(),
        (None, Some(one)) | (Some(one), None) => one.to_string(),
        (Some(why), Some(action)) => {
            let (w, a) = (normalize_phrase(why), normalize_phrase(action));
            if w.contains(&a) {
                why.to_string()
            } else if a.contains(&w) {
                action.to_string()
            } else {
                format!("{}. {action}", why.trim_end_matches('.').trim_end())
            }
        }
    }
}

/// Server-fault responses (5xx and their wire equivalents) carry internal detail ("upstream exploded") users can't act on, so always use our copy.
/// 429 stays client-side: its body may explain plan limits.
fn is_server_fault(status: Option<u16>, wire: WireErrorType) -> bool {
    match status {
        Some(code) => code >= 500,
        // The wire type `classify` headlines as "Server error".
        None => wire == WireErrorType::Api,
    }
}

/// The server body restates the headline (e.g. "Not Found" under a 404).
fn is_headline_echo(detail: &str, headline: &str) -> bool {
    let detail = normalize_phrase(detail);
    let headline = normalize_phrase(headline);
    detail.is_empty() || headline.starts_with(&detail) || detail.starts_with(&headline)
}

/// Lowercase, alphanumeric words joined by single spaces.
fn normalize_phrase(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// Pull an HTTP error status out of a raw dump: `API error (status 500): …`, `Unauthorized (401)`, or our own formatted `Server error (500): …`.
/// 4xx/5xx only: prose like "status 200" or a year must never classify a failure.
pub(crate) fn parse_http_status(raw: &str) -> Option<u16> {
    // Every "status " occurrence, so "status unknown; … status 503" still finds the code
    let mut from = 0;
    while let Some(tail) = raw.get(from..)
        && let Some(i) = find_ignore_ascii_case(tail, "status ")
    {
        let after = from + i + "status ".len();
        if let Some(code) = raw.get(after..).and_then(|s| parse_status_digits(s, false)) {
            return Some(code);
        }
        from = after;
    }
    const MARKERS: &[&str] = &[
        "Unauthorized (",
        "Forbidden (",
        "Not Found (",
        "Bad Request (",
        "Payment Required (",
        "Too Many Requests (",
        "Internal Server Error (",
        "Bad Gateway (",
        "Service Unavailable (",
        "Gateway Timeout (",
        "Payload Too Large (",
        "Request Entity Too Large (",
        "Server error (",
        "Request denied (",
        "Request failed (",
        "Not found (",
        "Bad request (",
        "Request too large (",
        "Service unavailable (",
        "Rate limited (",
        "Request timed out (",
        "Conflict (",
    ];
    for marker in MARKERS {
        if let Some(i) = find_ignore_ascii_case(raw, marker)
            && let Some(code) = raw
                .get(i + marker.len()..)
                .and_then(|s| parse_status_digits(s, true))
        {
            return Some(code);
        }
    }
    None
}

/// Exactly three digits in 400..600. `require_close_paren` for the `"… ("` markers, so prose like "merge conflict (300 files" can't match.
fn parse_status_digits(s: &str, require_close_paren: bool) -> Option<u16> {
    let bytes = s.as_bytes();
    if !bytes
        .get(..3)
        .is_some_and(|p| p.iter().all(u8::is_ascii_digit))
    {
        return None;
    }
    if bytes.get(3).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    if require_close_paren && bytes.get(3) != Some(&b')') {
        return None;
    }
    let code: u16 = s.get(..3)?.parse().ok()?;
    (400..600).contains(&code).then_some(code)
}

/// Header-timeout / cold-start class: TCP accepted but no HTTP response
/// headers within the stream-headers budget (default 2m). Not billing.
pub(crate) fn is_headers_timeout_cold_start(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("timed out waiting for response headers")
        || (lower.contains("response headers") && lower.contains("timed out"))
}

/// Fail-closed describe path: "image transcription failed" plus a send miss.
/// Named chrome so the pager does not paint a generic Request failed turn kill
/// as if the Human image were gone.
pub(crate) fn is_image_transcription_transport_miss(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("image transcription failed") && lower.contains("error sending request")
}

/// HTTP/SSE send miss (`error sending request`). Covers `request error stream`
/// and `reqwest error stream` when they carry that cause. Not a silent hang.
/// Not billing.
pub(crate) fn is_transport_send_miss(raw: &str) -> bool {
    raw.to_ascii_lowercase().contains("error sending request")
}

/// Output-cap truncation (`max_tokens_truncation` / `response truncated by
/// max_tokens`). Thought dumps hit this; it is not the Operator writing a
/// long prompt.
pub(crate) fn is_max_tokens_truncation(error_type: Option<&str>, raw: &str) -> bool {
    if WireErrorType::parse(error_type) == WireErrorType::MaxTokensTruncation {
        return true;
    }
    let lower = raw.to_ascii_lowercase();
    lower.contains("max_tokens")
        || lower.contains("try asking for a shorter answer")
        || lower.contains("the model hit its output limit")
        || lower.contains("response truncated")
}

/// Dest/resume report already written and nested implementors still running:
/// a safety refusal or thought output-cap is not a failed Operator request.
pub(crate) fn written_report_with_nested_running_is_not_turn_failure(
    nested_implementers_running: bool,
    had_output: bool,
    error_type: Option<&str>,
    message: &str,
) -> bool {
    nested_implementers_running
        && had_output
        && (is_safety_refusal_message(message) || is_max_tokens_truncation(error_type, message))
}

/// Model-host safety refusal, not HTTP 403 endpoint forbid and not a tool
/// permission deny (`permission_denied` / "permission denied for tool").
///
/// Operator-visible body: `permission-denied: I can't help with that request.`
pub(crate) fn is_safety_refusal_message(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("i can't help with that request")
        || lower.contains("i cannot help with that request")
        || lower.contains("can't help with that request")
        || lower.contains("cannot help with that request")
}

fn safety_refusal_detail(raw: &str) -> String {
    let trimmed = raw.trim();
    let after_prefix = ["permission-denied:", "permission_denied:"]
        .iter()
        .find_map(|prefix| {
            let lower = trimmed.to_ascii_lowercase();
            lower
                .starts_with(prefix)
                .then(|| trimmed[prefix.len()..].trim())
        })
        .unwrap_or(trimmed);
    if after_prefix.is_empty() {
        "I can't help with that request.".to_string()
    } else {
        after_prefix.to_string()
    }
}

fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

fn extract_error_detail(raw: &str) -> Option<String> {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return None;
    }

    if let Some(stripped) = strip_retry_prefix(&s) {
        s = stripped;
    }

    if let Some(rest) = strip_api_error_prefix(&s) {
        s = rest;
    }

    // JSON before the URL-clause strip: a URL inside a JSON string would otherwise split the body at its own ": " and leave garbage
    if let Some(json_start) = s.find('{')
        && let Some(extracted) = s.get(json_start..).and_then(extract_from_json)
    {
        s = extracted;
    }

    s = strip_from_url_clause(&s);

    // Prefer the "X is not in your available models" sentence when present (before dropping the Model/Auth/Version dump that contains it)
    if let Some(idx) = s.find("is not in your available models") {
        let line_start = s
            .get(..idx)
            .and_then(|h| h.rfind('\n'))
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = s
            .get(idx..)
            .and_then(|t| t.find('\n'))
            .map(|i| idx + i)
            .unwrap_or(s.len());
        let snippet = s.get(line_start..line_end).map_or("", str::trim);
        if !snippet.is_empty() {
            s = snippet.to_string();
        }
    } else {
        for marker in ["\n\n  Model:", "\n  Model:", "\n\nModel:", "\nModel:"] {
            if let Some(idx) = s.find(marker) {
                s.truncate(idx);
                break;
            }
        }
    }

    clean_detail(&s)
}

fn strip_retry_prefix(s: &str) -> Option<String> {
    let rest = s.strip_prefix("failed after ")?;
    let idx = rest.find(" retries: ")?;
    rest.get(idx + " retries: ".len()..).map(str::to_string)
}

fn strip_api_error_prefix(s: &str) -> Option<String> {
    let start = find_ignore_ascii_case(s, "API error (status ")?;
    let after = s.get(start + "API error (status ".len()..)?;
    let colon = after.find("): ")?;
    Some(after.get(colon + 3..)?.trim().to_string())
}

fn strip_from_url_clause(s: &str) -> String {
    // For "Unauthorized (401) from https://…: body", keep the body when present, otherwise drop the URL clause
    if let Some(from) = find_ignore_ascii_case(s, " from http") {
        if let Some(after_from) = s.get(from + " from ".len()..)
            && let Some(colon) = after_from.find(": ")
            && let Some(body) = after_from.get(colon + 2..).map(str::trim)
            && !body.is_empty()
            && !body.starts_with("http")
        {
            return body.to_string();
        }
        return s.get(..from).map_or(s, str::trim).to_string();
    }
    s.to_string()
}

fn extract_from_json(s: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(s.trim()).ok()?;
    if let Some(err) = value.get("error") {
        if let Some(msg) = err.as_str().filter(|m| !m.is_empty()) {
            return Some(msg.to_string());
        }
        if let Some(msg) = err
            .get("message")
            .and_then(|m| m.as_str())
            .filter(|m| !m.is_empty())
        {
            return Some(msg.to_string());
        }
    }
    value
        .get("message")
        .and_then(|m| m.as_str())
        .filter(|m| !m.is_empty())
        .map(str::to_string)
}

fn clean_detail(s: &str) -> Option<String> {
    let sanitized = crate::app::effects::sanitize_user_error(s.trim());
    let stripped = strip_urls(&sanitized);
    let t = stripped.trim();
    if t.is_empty() || is_noise_detail(t) {
        return None;
    }
    Some(t.to_string())
}

/// Remove `http(s)://…` tokens so no endpoint leaks into a banner; `sanitize_user_error` only rewrites known service names, not URLs.
/// Also drops the `for url (…)` clause reqwest wraps its URL in.
fn strip_urls(s: &str) -> String {
    if !s.contains("http://") && !s.contains("https://") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let Some(i) = rest
            .find("http://")
            .into_iter()
            .chain(rest.find("https://"))
            .min()
        else {
            out.push_str(rest);
            break;
        };
        let url_end = rest
            .get(i..)
            .and_then(|t| t.find(|c: char| c.is_whitespace() || c == ')'))
            .map_or(rest.len(), |e| i + e);
        let head = rest.get(..i).unwrap_or("");
        let head = head
            .strip_suffix("for url (")
            .or_else(|| head.strip_suffix('('))
            .unwrap_or(head);
        out.push_str(head);
        let tail = rest.get(url_end..).unwrap_or("");
        rest = tail.strip_prefix(')').unwrap_or(tail);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Bodies that just restate an HTTP reason phrase or generic filler add nothing over the headline and canned copy.
fn is_noise_detail(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "internal server error"
            | "internal error"
            | "server error"
            | "forbidden"
            | "unauthorized"
            | "bad request"
            | "not found"
            | "error"
            | "unknown error"
            | "unknown"
            | "none"
            | "ok"
            | "request error"
            | "model does not exist"
            | "model not found"
            | "too many requests"
            | "payment required"
            | "gateway timeout"
            | "request timeout"
            | "conflict"
            | "service unavailable"
            | "bad gateway"
            | "payload too large"
            | "request entity too large"
            | "overloaded"
    ) || lower.starts_with("json-rpc")
        || lower.starts_with("request error -")
        || s.starts_with('{')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_500_json_dump() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            r#"API error (status 500 Internal Server Error): {"error":"upstream exploded","request_id":"abc"}"#,
        );
        assert_eq!(formatted.status, Some(500));
        assert_eq!(formatted.headline, "Server error (500)");
        assert_eq!(
            formatted.detail,
            "Something went wrong on our side. Wait a minute and send again."
        );
        assert_eq!(
            formatted.message(),
            "Server error (500): Something went wrong on our side. Wait a minute and send again."
        );
        assert!(!formatted.message().contains("exploded"));
    }

    /// A parsed provider reason on a 4xx survives the banner formatting end-to-end.
    /// The message shape is what `user_facing_api_error_message` produces via `SamplingError::Api`'s Display.
    /// The sampler's body parser recovers double-encoded relay bodies, so the reason arrives already parsed.
    #[test]
    fn keeps_parsed_provider_reason_on_4xx() {
        let formatted = format_request_failure(
            Some(400),
            Some(WireErrorType::Api),
            "API error (status 400 Bad Request): invalid_request_error: \
             Values detected in request that violate rules: JWT Token",
        );
        assert_eq!(formatted.headline, "Bad request (400)");
        assert!(
            formatted
                .detail
                .contains("Values detected in request that violate rules: JWT Token"),
            "provider reason must survive: {}",
            formatted.detail
        );
    }

    #[test]
    fn untyped_truncation_text_recovers_truncation_copy() {
        // Pre-`errorKind` terminals deliver only the raw string; the literal (not the const) pins the historical wire bytes
        let formatted = format_request_failure(None, None, "response truncated by max_tokens");
        assert_eq!(
            formatted.message(),
            "Response truncated: The model hit its output limit."
        );
    }

    #[test]
    fn untyped_other_errors_keep_generic_fallback() {
        let formatted = format_request_failure(None, None, "connection reset by peer");
        assert_eq!(
            formatted.message(),
            "Request failed: connection reset by peer. Try sending again."
        );
    }

    #[test]
    fn untyped_unknown_kind_is_not_sniff_reclassified() {
        // A genuinely unknown kind (a newer shell's) enters as `Some(Other)` via `wire_error_kind`, not `None`
        // Quoting the truncation phrase must not steal its classification
        let kind = wire_error_kind(Some("a_future_kind"));
        assert_eq!(kind, Some(WireErrorType::Other));
        let formatted = format_request_failure(
            None,
            kind,
            "a future failure quoting: response truncated by max_tokens",
        );
        assert_eq!(formatted.headline, "Request failed");
        assert!(formatted.detail.contains("Try sending again."));
    }

    #[test]
    fn untyped_status_bearing_raw_prefers_status_copy() {
        // An upstream body may quote the client's truncation phrase; a raw carrying a parseable HTTP status keeps its status classification
        let formatted = format_request_failure(
            None,
            None,
            "API error (status 400 Bad Request): response truncated by max_tokens is not \
             acceptable input",
        );
        assert_eq!(formatted.headline, "Bad request (400)");
        assert!(!formatted.message().contains("output limit"));
    }

    #[test]
    fn dedups_action_already_in_client_error_detail() {
        // 4xx bodies are kept (unlike 5xx), and the canned action is dropped when the server text already says it
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            "API error (status 429 Too Many Requests): Plan limit reached, try again later",
        );
        assert_eq!(
            formatted.message(),
            "Rate limited (429): Plan limit reached, try again later"
        );
    }

    #[test]
    fn formats_403_with_server_message() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            "API error (status 403 Forbidden): Access to the chat endpoint is denied",
        );
        assert_eq!(
            formatted.message(),
            "Request denied (403): Access to the chat endpoint is denied"
        );
    }

    /// Operator screenshot 2026-09-19: resume transcript painted
    /// `Request denied (403) – permission-denied: I can't help with that request.`
    /// That body is a safety refusal, not HTTP 403 endpoint forbid.
    #[test]
    fn safety_refusal_permission_denied_must_not_paint_request_denied_403() {
        let operator_body = "permission-denied: I can't help with that request.";
        assert!(operator_body.contains("permission-denied: I can't help with that request."));
        for (status, error_type, raw) in [
            (
                None,
                Some("api"),
                "API error (status 403 Forbidden): permission-denied: I can't help with that request.",
            ),
            (Some(403), Some("api"), operator_body),
            (None, None, operator_body),
            (None, Some("api"), "I can't help with that request."),
        ] {
            let formatted = format_request_failure(status, error_type, raw);
            let msg = formatted.message();
            assert!(
                !msg.contains("Request denied (403)"),
                "safety refusal must not be labeled HTTP 403, got {msg} from {raw}"
            );
            assert!(
                msg.contains("Safety refusal"),
                "must paint safety refusal chrome, got {msg} from {raw}"
            );
            assert!(
                msg.contains("I can't help with that request"),
                "must keep the refusal text, got {msg} from {raw}"
            );
            assert_eq!(formatted.status, None, "must not keep HTTP 403 status");
        }
        let tool_deny = format_request_failure(None, None, "tool permission denied for bash");
        assert!(
            !tool_deny.message().contains("Request denied (403)"),
            "tool permission deny is not HTTP 403, got {}",
            tool_deny.message()
        );
        assert!(
            !tool_deny.message().contains("Safety refusal"),
            "hub tool deny is not a model safety refusal, got {}",
            tool_deny.message()
        );
    }

    /// Operator shot 2026-08-22: yellow banner must not paint console team
    /// prepaid 403 as included SuperGrok period limits truth.
    #[test]
    fn request_denied_403_team_credits_must_not_paint_purchase_more_credits_as_supergrok_included_truth()
     {
        let formatted = format_request_failure(
            None,
            Some("api"),
            "API error (status 403 Forbidden): Your team 61fab250-b2c1-40cf-b5b8-628e673a2eeb \
             has either used all available credits or reached its monthly spending limit. \
             To continue making API requests, please purchase more credits or raise \
             the spending limit.",
        );
        assert_eq!(formatted.headline, "Request denied (403)");
        let detail = formatted.detail.to_ascii_lowercase();
        assert!(
            detail.contains("console team prepaid") && detail.contains("console api credits"),
            "detail must name console team prepaid / console API credits: {}",
            formatted.detail
        );
        assert!(
            detail.contains("stay on supergrok"),
            "detail must say stay on SuperGrok: {}",
            formatted.detail
        );
        assert!(
            !detail.contains("purchase more")
                && !detail.contains("add credits")
                && !detail.contains("used all available credits"),
            "must not command a SuperGrok included top-up: {}",
            formatted.detail
        );
        assert!(
            !detail.contains("used up"),
            "must not say used up: {}",
            formatted.detail
        );
    }

    #[test]
    fn formats_401_dump_without_triggering_reauth_copy() {
        // Re-auth is a dedicated banner; this helper only pretty-prints.
        let formatted = format_request_failure(
            Some(401),
            Some(WireErrorType::Api),
            r#"Unauthorized (401) from https://cli-chat-proxy.grok.com/v1/responses: {"error":"Invalid or expired credentials (auth_kind=bearer)"}"#,
        );
        assert_eq!(formatted.status, Some(401));
        assert!(formatted.detail.contains("Invalid or expired credentials"));
        assert!(!formatted.message().contains("cli-chat-proxy"));
        assert!(!formatted.message().contains("https://"));
    }

    #[test]
    fn formats_413() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            "API error (status 413 Payload Too Large): request too large",
        );
        assert_eq!(
            formatted.message(),
            "Request too large (413): Try a smaller prompt or run /compact."
        );
    }

    #[test]
    fn formats_openai_shaped_json() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            r#"API error (status 400 Bad Request): {"error":{"message":"model does not support tools","type":"invalid_request_error"}}"#,
        );
        assert_eq!(
            formatted.message(),
            "Bad request (400): model does not support tools"
        );
    }

    #[test]
    fn formats_idle_timeout_without_status() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::IdleTimeout),
            "inference idle timeout after 90s with no chunks",
        );
        assert_eq!(formatted.status, None);
        assert_eq!(
            formatted.message(),
            "No response from the model: inference idle timeout after 90s with no chunks. \
             Try sending again."
        );
    }

    #[test]
    fn typed_headline_survives_status_in_message_text() {
        // `auth_transient` copy routinely embeds "Unauthorized (401)"
        // The sniffed status must not demote the dedicated headline to generic "Request failed (401)" copy
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::AuthTransient),
            "Unauthorized (401): token refresh already in progress",
        );
        assert_eq!(formatted.status, None);
        assert_eq!(formatted.headline, "Authentication temporarily unavailable");

        // An explicit caller-parsed status still wins over the wire type.
        let formatted =
            format_request_failure(Some(503), Some(WireErrorType::IdleTimeout), "whatever");
        assert_eq!(formatted.headline, "Service unavailable (503)");
    }

    #[test]
    fn formats_exhausted_retry_prefix() {
        let formatted = format_request_failure(
            None,
            None,
            r#"failed after 3 retries: API error (status 503): {"error":"overloaded"}"#,
        );
        assert_eq!(
            formatted.message(),
            "Service unavailable (503): The service is busy. Wait a minute and send again."
        );
    }

    #[test]
    fn formats_404_short_body_tells_user_to_switch_model() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            r#"API error (status 404 Not Found): {"error":"model does not exist"}"#,
        );
        assert_eq!(
            formatted.message(),
            "Not found (404): This model isn't available. Run /model to pick another."
        );
    }

    #[test]
    fn drops_model_catalog_dump() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            "API error (status 404 Not Found): model does not exist\n\n  Model:     grok-foo\n  Auth:      ApiKey\n  Version:   0.1.0\n  Available: grok-build\n\n  'grok-foo' is not in your available models.\n  Switch models with /model or start a new session.",
        );
        assert_eq!(formatted.status, Some(404));
        assert_eq!(
            formatted.message(),
            "Not found (404): 'grok-foo' is not in your available models. \
             Run /model to pick another."
        );
        assert!(!formatted.message().contains("Available:"));
        assert!(!formatted.message().contains("Version:"));
    }

    #[test]
    fn recovers_status_from_own_banner_text() {
        // PromptResponse race fallbacks (401 re-auth, 402 credit limit) sniff the already-formatted error text when http_status is absent
        // Our own headlines must parse
        assert_eq!(
            parse_http_status("Request failed (402): usage balance exhausted"),
            Some(402)
        );
        assert_eq!(
            parse_http_status("Server error (500): Something went wrong on our side."),
            Some(500)
        );
    }

    /// Operator: "Connection failed – request error stream: timed out waiting
    /// for response headers after 2m0s"; retry attempt 2; "might be caused by
    /// it being a cold start, not warm." Chrome must name the cold-start
    /// class and keep a retry path. Not Thought-only. Not billing.
    #[test]
    fn header_timeout_is_named_cold_start_class_with_retry_path() {
        let raw = "Connection failed – request error stream: timed out waiting for response headers after 2m0s";
        let formatted = format_request_failure(None, Some("http"), raw);
        let msg = formatted.message();
        assert!(
            msg.contains("Cold start") && msg.contains("response headers timed out"),
            "header timeout must name the cold-start class, got {msg}"
        );
        assert!(
            msg.contains("Retry is in progress"),
            "must keep a retry path, not Thought-only, got {msg}"
        );
        assert!(
            !msg.contains("Thought") || msg.contains("not Thought-only"),
            "must not leave Thought-only chrome, got {msg}"
        );
        assert!(
            !msg.to_ascii_lowercase().contains("dollar")
                && !msg.to_ascii_lowercase().contains("billing"),
            "must not invent billing, got {msg}"
        );
        let retry = crate::app::subagent::format_activity_label(
            &crate::acp::tracker::TurnActivity::Retrying {
                attempt: 2,
                max_retries: u32::MAX,
                reason: "cold start: response headers timed out".into(),
            },
        );
        assert!(
            retry.contains("Retrying (2)"),
            "retry attempt 2 must stay Retrying chrome, got {retry}"
        );
    }

    #[test]
    fn strips_reqwest_url_from_connection_error() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Http),
            "error sending request for url (https://server.grok.com/v1/responses)",
        );
        let msg = formatted.message();
        assert!(!msg.contains("http"), "{msg}");
        assert!(
            msg.contains("Connection failed") && msg.contains("Transport miss"),
            "send miss must name transport, got {msg}"
        );
        assert_eq!(
            formatted.message(),
            "Connection failed: error sending request. \
             Check your network and try again."
        );
    }

    #[test]
    fn url_inside_json_body_does_not_mangle_detail() {
        let formatted = format_request_failure(
            None,
            Some(WireErrorType::Api),
            r#"API error (status 400 Bad Request): {"error":"fetch from https://example.com: connection refused"}"#,
        );
        assert_eq!(formatted.message(), "Bad request (400): connection refused");
    }

    #[test]
    fn parse_http_status_rejects_non_status_numbers() {
        // Success codes, years, and prose parentheticals are not failures.
        assert_eq!(parse_http_status("expected status 200 but got EOF"), None);
        assert_eq!(parse_http_status("status 2024 items processed"), None);
        assert_eq!(
            parse_http_status("merge conflict (300 files changed)"),
            None
        );
        // A later occurrence still parses.
        assert_eq!(
            parse_http_status("status unknown; API error (status 503): overloaded"),
            Some(503)
        );
    }

    #[test]
    fn wire_error_type_parse_known_and_unknown() {
        assert_eq!(WireErrorType::parse(Some("auth")), WireErrorType::Auth);
        assert_eq!(
            WireErrorType::parse(Some("auth_transient")),
            WireErrorType::AuthTransient
        );
        assert_eq!(
            WireErrorType::parse(Some("encrypted_content_mismatch")),
            WireErrorType::EncryptedContentMismatch
        );
        assert_eq!(
            WireErrorType::parse(Some("disk_full")),
            WireErrorType::DiskFull
        );
        assert_eq!(
            WireErrorType::parse(Some("rate_limited")),
            WireErrorType::RateLimited
        );
        assert_eq!(
            WireErrorType::parse(Some("max_tokens_truncation")),
            WireErrorType::MaxTokensTruncation
        );
        assert_eq!(
            WireErrorType::parse(Some("repetitive_generation")),
            WireErrorType::RepetitiveGeneration
        );
        assert_eq!(WireErrorType::parse(Some("nope")), WireErrorType::Other);
        assert_eq!(WireErrorType::parse(None), WireErrorType::Other);
    }

    /// Operator screenshot 2026-09-20: after dest completeOk (Worked for 48s),
    /// L1 thought 29m26s then yellow
    /// `Response truncated – The model hit its output limit. Try asking for a shorter answer.`
    /// The Operator did not write a long prompt. Do not tell them to ask for a
    /// shorter answer.
    #[test]
    fn max_tokens_truncation_must_not_tell_operator_to_ask_for_a_shorter_answer() {
        let operator_chrome =
            "Response truncated – The model hit its output limit. Try asking for a shorter answer.";
        assert!(operator_chrome.contains(
            "Response truncated – The model hit its output limit. Try asking for a shorter answer."
        ));
        assert!(operator_chrome.contains("Try asking for a shorter answer"));
        let formatted = format_request_failure(
            None,
            Some("max_tokens_truncation"),
            "response truncated by max_tokens",
        );
        let msg = formatted.message();
        assert!(
            !msg.contains("Try asking for a shorter answer"),
            "must not blame the Operator for a long prompt, got {msg}"
        );
        assert!(
            !msg.contains(operator_chrome),
            "must not paint the Operator-blaming screenshot chrome, got {msg}"
        );
        assert!(
            is_max_tokens_truncation(
                Some("max_tokens_truncation"),
                "response truncated by max_tokens"
            ),
            "wire type max_tokens_truncation must classify as output-cap truncation"
        );
    }

    /// Isolated Preview 2026-09-20 looped
    /// `Spawn dests of dest encoder skip. I'll spawn dests of dest encoder skip.`
    /// The stop must not paint Request denied (403).
    #[test]
    fn dest_encoder_skip_repetitive_generation_is_not_request_denied_403() {
        assert!(
            "Spawn dests of dest encoder skip. I'll spawn dests of dest encoder skip."
                .contains("Spawn dests of dest encoder skip")
        );
        let formatted = format_request_failure(
            None,
            Some("repetitive_generation"),
            "Stopped: the reply was repeating the same sentence.",
        );
        let msg = formatted.message();
        assert!(
            !msg.contains("403") && !msg.contains("Request denied"),
            "must not look like HTTP 403, got {msg}"
        );
        assert!(
            msg.contains("repeating") || msg.contains("sentence"),
            "must name the repeating-sentence stop, got {msg}"
        );
    }

    #[test]
    fn parse_http_status_from_common_shapes() {
        assert_eq!(
            parse_http_status("API error (status 502 Bad Gateway): nope"),
            Some(502)
        );
        assert_eq!(
            parse_http_status("Unauthorized (401) from https://x"),
            Some(401)
        );
        assert_eq!(parse_http_status("Server error (500): boom"), Some(500));
        assert_eq!(parse_http_status("connection reset"), None);
    }

    #[test]
    fn retry_activity_label_uses_request_failure_headline() {
        let dns = "request error: error sending request for url (https://api.x.ai/v1/responses): client error (Connect): dns error: failed to lookup address information: Temporary failure in name resolution";
        assert_eq!(
            format_retry_activity_label(8, 10, dns, None, RetryLabelStyle::Status),
            "Connection failed | Retrying (attempt 8)..."
        );
        assert!(
            !format_retry_activity_label(8, 10, dns, None, RetryLabelStyle::Status)
                .contains("http")
        );
        assert_eq!(
            format_retry_activity_label(
                2,
                5,
                "API error (status 429 Too Many Requests): rate limit exceeded",
                None,
                RetryLabelStyle::Compact
            ),
            "Rate limited (429) | Retrying (2/5)"
        );
        assert_eq!(
            format_retry_activity_label(2, 5, "", None, RetryLabelStyle::Status),
            "Retrying (attempt 2)..."
        );
        assert_eq!(
            format_retry_activity_label(
                1,
                3,
                "Re-authenticated after 401; retrying request",
                None,
                RetryLabelStyle::Status
            ),
            "Retrying (attempt 1)..."
        );
        assert_eq!(
            format_retry_activity_label(3, 5, "weird dump", Some("http"), RetryLabelStyle::Status),
            "Connection failed | Retrying (attempt 3)..."
        );
        assert_eq!(
            format_retry_activity_label(
                2,
                5,
                "slow down",
                Some("rate_limited"),
                RetryLabelStyle::Compact
            ),
            "Rate limited | Retrying (2/5)"
        );
        assert_eq!(
            format_retry_activity_label(1, 3, dns, Some("a_future_kind"), RetryLabelStyle::Status),
            "Retrying (attempt 1)..."
        );
        assert_eq!(
            format_request_failure(
                None,
                Some(WireErrorType::Other),
                "error sending request for url (https://api.x.ai)"
            )
            .headline,
            "Request failed"
        );
    }
}
