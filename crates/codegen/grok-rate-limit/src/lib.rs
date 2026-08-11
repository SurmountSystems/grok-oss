//! Cross-process shared rate-limit cooldowns for Grok OSS.
//!
//! Multiple `grok-oss` processes coordinate via flock + JSON under
//! `$GROK_HOME/rate_limits/` so concurrent sessions do not stampede a
//! rate-limited API. See Surmount FORK.md.
//!
//! # Semantics
//!
//! - **Attempt budget**: not this crate’s concern (sampler stays unlimited by default).
//! - **When to call**: `not_before` is the earliest unix-ms any process may issue a request.
//! - **Merge rule**: on observe, `not_before = max(existing, now + wait)` (strictest wins).
//! - **Disable**: `GROK_DISABLE_SHARED_RATE_LIMIT=1` makes all ops no-ops.
//!
//! # Public docs (header-driven waits preferred over hardcoding tier tables)
//!
//! - xAI rate limits (RPS/TPM per model; Imagine separate RPS):
//!   <https://docs.x.ai/developers/rate-limits> (accessed: 2026-08-03)
//! - OpenRouter limits (honor `Retry-After` / `X-RateLimit-*` on 429):
//!   <https://openrouter.ai/docs/api_reference/limits> (accessed: 2026-08-03)
//! - GitHub REST primary + secondary limits (`Retry-After`, `x-ratelimit-reset`):
//!   <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>
//!   (accessed: 2026-08-03)

mod http;
mod store;

pub use http::{
    DEFAULT_RATE_LIMIT_WAIT, observe_status, should_observe_status, wait_from_header_values,
};
pub use store::{
    DISABLE_ENV, ProviderKey, RateLimitMeta, RateLimitSnapshot, SharedRateLimitStore,
    fingerprint_secret, shared_rate_limits_disabled,
};

/// Well-known provider key strings (stable identifiers for callers).
pub mod keys {
    pub const XAI: &str = "xai";
    pub const OPENROUTER: &str = "openrouter";
    pub const GITHUB: &str = "github";
    /// Management API host (Console billing meters). Prefer
    /// [`crate::ProviderKey::from_base_url_and_key_fingerprint`] with the
    /// management key fingerprint when a secret is available.
    pub const XAI_MANAGEMENT: &str = "management-api.x.ai";
}

/// API class suffixes for type-appropriate cooldowns on the same host.
///
/// xAI documents separate rate-limit buckets for text models vs Imagine image
/// vs Imagine video (and Voice is a separate product limit). OpenRouter and
/// GitHub are keyed by host/logical name. See crate-level doc links.
///
/// Chat/inference in the sampler keeps host+fingerprint only (no class) for
/// backward-compatible file names under `$GROK_HOME/rate_limits/`.
pub mod api_class {
    /// Imagine image generate + edit (`/images/generations`, `/images/edits`).
    pub const IMAGINE: &str = "imagine";
    /// Imagine video (`/videos/generations` + poll).
    pub const VIDEO: &str = "video";
    /// Streaming speech-to-text (`wss://…/v1/stt`).
    pub const VOICE: &str = "voice";
    /// Responses API (web_search tool).
    pub const RESPONSES: &str = "responses";
}
