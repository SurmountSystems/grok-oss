//! Header-driven rate-limit wait helpers (no HTTP client dependency).
//!
//! Prefer server `Retry-After` / reset headers over hardcoding tier tables.
//! Docs (accessed 2026-08-03):
//! - <https://docs.x.ai/developers/rate-limits>
//! - <https://openrouter.ai/docs/api_reference/limits>
//! - <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::store::{ProviderKey, RateLimitMeta, SharedRateLimitStore};

/// Fallback wait when the server omits retry hints. Matches OSS GitHub update
/// and shell billing/Management default.
pub const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);

/// Parse wait from optional header values.
///
/// Prefers `Retry-After` seconds; else `x-ratelimit-reset` (unix epoch seconds).
pub fn wait_from_header_values(
    retry_after: Option<&str>,
    ratelimit_reset: Option<&str>,
) -> Option<Duration> {
    if let Some(secs) = retry_after.and_then(|s| s.trim().parse::<u64>().ok()) {
        return Some(Duration::from_secs(secs.max(1)));
    }
    let reset = ratelimit_reset.and_then(|s| s.trim().parse::<u64>().ok())?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Duration::from_secs(reset.saturating_sub(now).max(1)))
}

/// Whether this status should publish a shared cooldown.
///
/// - **429** always.
/// - **403** only when a retry hint is present (permanent invalid-key 403s must
///   not poison peers).
pub fn should_observe_status(status: u16, has_retry_hint: bool) -> bool {
    match status {
        429 => true,
        403 => has_retry_hint,
        _ => false,
    }
}

/// Publish a shared cooldown when status is rate-limit-like.
///
/// Returns `true` when an observe was attempted (including store no-op when
/// disabled). Bare 403 without retry headers returns `false` (does not poison).
pub fn observe_status(
    store: &SharedRateLimitStore,
    key: &ProviderKey,
    status: u16,
    retry_after: Option<&str>,
    ratelimit_reset: Option<&str>,
    reason: impl Into<String>,
) -> bool {
    let wait_hint = wait_from_header_values(retry_after, ratelimit_reset);
    if !should_observe_status(status, wait_hint.is_some()) {
        return false;
    }
    let wait = wait_hint.unwrap_or(DEFAULT_RATE_LIMIT_WAIT);
    let meta = RateLimitMeta {
        status: Some(status),
        reason: Some(reason.into()),
    };
    if let Err(e) = store.observe(key, wait, meta) {
        tracing::debug!(error = %e, provider = key.as_str(), "shared rate limit observe failed");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_prefers_retry_after_seconds() {
        assert_eq!(
            wait_from_header_values(Some("12"), Some("9999999999")),
            Some(Duration::from_secs(12))
        );
    }

    #[test]
    fn wait_from_reset_epoch() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let wait = wait_from_header_values(None, Some(&(now + 30).to_string())).unwrap();
        assert!(wait >= Duration::from_secs(25), "got {wait:?}");
        assert!(wait <= Duration::from_secs(35), "got {wait:?}");
    }

    #[test]
    fn should_observe_429_always_403_only_with_hint() {
        assert!(should_observe_status(429, false));
        assert!(!should_observe_status(403, false));
        assert!(should_observe_status(403, true));
        assert!(!should_observe_status(401, true));
        assert!(!should_observe_status(500, false));
    }

    #[test]
    fn observe_status_skips_bare_403() {
        let prev = std::env::var_os(crate::DISABLE_ENV);
        if prev.is_some() {
            unsafe { std::env::remove_var(crate::DISABLE_ENV) };
        }
        let dir = tempfile::TempDir::new().unwrap();
        let store = SharedRateLimitStore::open(dir.path()).unwrap();
        let key = ProviderKey::new("bare-403");
        assert!(!observe_status(
            &store,
            &key,
            403,
            None,
            None,
            "invalid key",
        ));
        assert_eq!(store.remaining(&key), Duration::ZERO);
        match prev {
            Some(v) => unsafe { std::env::set_var(crate::DISABLE_ENV, v) },
            None => {}
        }
    }

    #[test]
    fn observe_status_writes_on_429() {
        let prev = std::env::var_os(crate::DISABLE_ENV);
        if prev.is_some() {
            unsafe { std::env::remove_var(crate::DISABLE_ENV) };
        }
        let dir = tempfile::TempDir::new().unwrap();
        let store = SharedRateLimitStore::open(dir.path()).unwrap();
        let key = ProviderKey::new("rl-429");
        assert!(observe_status(
            &store,
            &key,
            429,
            Some("2"),
            None,
            "rate limited",
        ));
        assert!(
            store.remaining(&key) >= Duration::from_millis(500),
            "remaining={:?}",
            store.remaining(&key)
        );
        match prev {
            Some(v) => unsafe { std::env::set_var(crate::DISABLE_ENV, v) },
            None => {}
        }
    }
}
