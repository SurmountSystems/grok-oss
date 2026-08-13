//! Shared multi-process HTTP cooldowns for SuperGrok billing and Management API.
//!
//! Product IPC is the flock JSON store in [`grok_rate_limit`] under
//! `$GROK_HOME/rate_limits/` (same path the sampler and OSS GitHub update use).
//! This module does **not** invent a Unix-socket daemon and does **not** fold
//! into the exhausted-credit memo (credit hop only).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use grok_rate_limit::{ProviderKey, RateLimitMeta, SharedRateLimitStore, fingerprint_secret};

/// Fallback wait when the server omits `Retry-After` (and peers have no longer
/// cooldown on disk). Matches the OSS GitHub update default.
pub const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);

#[cfg(test)]
thread_local! {
    /// Hermetic tests may redirect production callers away from
    /// [`SharedRateLimitStore::process_default`] (OnceLock + real `GROK_HOME`).
    static TEST_STORE_OVERRIDE: std::cell::RefCell<Option<SharedRateLimitStore>> =
        const { std::cell::RefCell::new(None) };
}

/// Process default store, or a test override when set.
pub fn shared_store() -> SharedRateLimitStore {
    #[cfg(test)]
    {
        if let Some(store) = TEST_STORE_OVERRIDE.with(|c| c.borrow().clone()) {
            return store;
        }
    }
    SharedRateLimitStore::process_default()
}

/// Install a temp-dir store for the current test thread (cleared on drop).
#[cfg(test)]
pub struct TestStoreGuard;

#[cfg(test)]
impl Drop for TestStoreGuard {
    fn drop(&mut self) {
        TEST_STORE_OVERRIDE.with(|c| {
            *c.borrow_mut() = None;
        });
    }
}

/// Point [`shared_store`] at `store` for this thread until the guard drops.
#[cfg(test)]
pub fn override_shared_store_for_test(store: SharedRateLimitStore) -> TestStoreGuard {
    TEST_STORE_OVERRIDE.with(|c| {
        *c.borrow_mut() = Some(store);
    });
    TestStoreGuard
}

/// SuperGrok billing / CLI-proxy path: host from proxy base + session fingerprint.
///
/// Fingerprint is FNV of the bearer token (not the secret itself). File names
/// under `rate_limits/` never contain the raw token.
pub fn billing_provider_key(proxy_base: &str, access_token: &str) -> ProviderKey {
    let token = access_token.trim();
    if token.is_empty() {
        return ProviderKey::from_base_url(proxy_base);
    }
    ProviderKey::from_base_url_and_key_fingerprint(proxy_base, &fingerprint_secret(token))
}

/// Management API path: host from base URL + management-key fingerprint.
pub fn management_provider_key(base_url: &str, management_key: &str) -> ProviderKey {
    let key = management_key.trim();
    if key.is_empty() {
        return ProviderKey::from_base_url(base_url);
    }
    ProviderKey::from_base_url_and_key_fingerprint(base_url, &fingerprint_secret(key))
}

/// Prefer `Retry-After` seconds; else optional `x-ratelimit-reset` unix epoch.
pub fn wait_from_rate_limit_headers(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    if let Some(secs) = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        return Some(Duration::from_secs(secs.max(1)));
    }
    let reset = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Duration::from_secs(reset.saturating_sub(now).max(1)))
}

/// Whether this status should publish a shared cooldown.
///
/// - **429** always.
/// - **403** only when a retry hint is present (permanent invalid-key 403s must
///   not poison peers; Management key validation treats bare 403 as bad key).
pub fn should_observe_rate_limit(status: u16, headers: &reqwest::header::HeaderMap) -> bool {
    match status {
        429 => true,
        403 => wait_from_rate_limit_headers(headers).is_some(),
        _ => false,
    }
}

/// Wait on the shared cooldown before issuing HTTP (no cancel token on these
/// paths; matches Management / billing poll style, not the sampler Esc path).
pub async fn wait_before_http(key: &ProviderKey) {
    shared_store().wait_if_limited(key).await;
}

/// On rate-limit-like responses, publish shared cooldown for peer processes.
///
/// Returns `true` when an observe was attempted.
pub fn observe_http_rate_limit(
    key: &ProviderKey,
    status: u16,
    headers: &reqwest::header::HeaderMap,
    reason: impl Into<String>,
) -> bool {
    if !should_observe_rate_limit(status, headers) {
        return false;
    }
    let wait = wait_from_rate_limit_headers(headers).unwrap_or(DEFAULT_RATE_LIMIT_WAIT);
    let meta = RateLimitMeta {
        status: Some(status),
        reason: Some(reason.into()),
    };
    if let Err(e) = shared_store().observe(key, wait, meta) {
        tracing::debug!(error = %e, provider = key.as_str(), "shared rate limit observe failed");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    use grok_rate_limit::{DISABLE_ENV, shared_rate_limits_disabled};
    use tempfile::TempDir;

    /// Serialize env kill-switch mutations across this module's tests.
    static ENV_LOCK: StdMutex<()> = StdMutex::new(());

    fn with_shared_limits_enabled<R>(f: impl FnOnce() -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os(DISABLE_ENV);
        if prev.is_some() {
            // SAFETY: exclusive via ENV_LOCK; restored before unlock.
            unsafe { std::env::remove_var(DISABLE_ENV) };
        }
        let out = f();
        match prev {
            Some(v) => unsafe { std::env::set_var(DISABLE_ENV, v) },
            None => {}
        }
        out
    }

    #[test]
    fn billing_key_uses_host_and_fingerprint_not_raw_token() {
        let token = "super-secret-session-token-value";
        let key = billing_provider_key("https://cli-proxy.example.com/v1", token);
        let s = key.as_str();
        assert!(
            s.starts_with("cli-proxy.example.com_"),
            "expected host+fp key, got {s}"
        );
        assert!(
            !s.contains(token),
            "raw session token must not appear in provider key: {s}"
        );
        assert!(
            !s.contains("super-secret"),
            "token fragment must not appear: {s}"
        );
        // Different sessions → different keys (independent cooldowns).
        let other = billing_provider_key("https://cli-proxy.example.com/v1", "other-session");
        assert_ne!(key.as_str(), other.as_str());
        // Same session → stable.
        let again = billing_provider_key("https://cli-proxy.example.com/v1", token);
        assert_eq!(key.as_str(), again.as_str());
    }

    #[test]
    fn management_key_uses_host_and_fingerprint_not_raw_secret() {
        let secret = "xai-mgmt-key-DEADBEEF-not-for-disk";
        let key = management_provider_key("https://management-api.x.ai", secret);
        let s = key.as_str();
        assert!(
            s.starts_with("management-api.x.ai_"),
            "expected management host+fp, got {s}"
        );
        assert!(
            !s.contains(secret),
            "raw management key must not be in path: {s}"
        );
        assert!(
            !s.contains("DEADBEEF"),
            "secret fragment must not appear: {s}"
        );
    }

    #[test]
    fn empty_secret_falls_back_to_host_only() {
        let k = management_provider_key("https://management-api.x.ai", "  ");
        assert_eq!(k.as_str(), "management-api.x.ai");
        let b = billing_provider_key("https://proxy.example/", "");
        assert_eq!(b.as_str(), "proxy.example");
    }

    #[test]
    fn should_observe_429_always_403_only_with_retry_hint() {
        let mut headers = reqwest::header::HeaderMap::new();
        assert!(should_observe_rate_limit(429, &headers));
        assert!(!should_observe_rate_limit(403, &headers));
        assert!(!should_observe_rate_limit(401, &headers));
        assert!(!should_observe_rate_limit(500, &headers));
        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("30"),
        );
        assert!(should_observe_rate_limit(403, &headers));
        assert!(should_observe_rate_limit(429, &headers));
    }

    #[test]
    fn wait_from_headers_prefers_retry_after() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("12"),
        );
        assert_eq!(
            wait_from_rate_limit_headers(&headers),
            Some(Duration::from_secs(12))
        );
    }

    #[test]
    fn two_store_handles_share_management_shaped_cooldown() {
        with_shared_limits_enabled(|| {
            let dir = TempDir::new().unwrap();
            let a = SharedRateLimitStore::open(dir.path()).unwrap();
            let b = SharedRateLimitStore::open(dir.path()).unwrap();
            let key = management_provider_key("https://management-api.x.ai", "mgmt-key-a");
            a.observe(
                &key,
                Duration::from_secs(15),
                RateLimitMeta {
                    status: Some(429),
                    reason: Some("from-process-a".into()),
                },
            )
            .unwrap();
            let rem_b = b.remaining(&key);
            assert!(
                rem_b >= Duration::from_secs(10),
                "peer B must see A's Management cooldown, got {rem_b:?}"
            );
        });
    }

    #[test]
    fn two_store_handles_share_billing_shaped_cooldown() {
        with_shared_limits_enabled(|| {
            let dir = TempDir::new().unwrap();
            let a = SharedRateLimitStore::open(dir.path()).unwrap();
            let b = SharedRateLimitStore::open(dir.path()).unwrap();
            let key = billing_provider_key("https://cli-proxy.example.com", "session-a");
            a.observe(
                &key,
                Duration::from_secs(15),
                RateLimitMeta {
                    status: Some(429),
                    reason: Some("from-process-a".into()),
                },
            )
            .unwrap();
            let rem_b = b.remaining(&key);
            assert!(
                rem_b >= Duration::from_secs(10),
                "peer B must see A's billing cooldown, got {rem_b:?}"
            );
        });
    }

    #[tokio::test]
    async fn observe_http_rate_limit_writes_shared_store() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os(DISABLE_ENV);
        if prev.is_some() {
            unsafe { std::env::remove_var(DISABLE_ENV) };
        }
        assert!(!shared_rate_limits_disabled());

        let dir = TempDir::new().unwrap();
        let store = SharedRateLimitStore::open(dir.path()).unwrap();
        let _override = override_shared_store_for_test(store.clone());
        let key = management_provider_key("https://management-api.x.ai", "hermetic-key");

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("2"),
        );
        assert!(observe_http_rate_limit(
            &key,
            429,
            &headers,
            "management prepaid rate limit",
        ));
        let rem = store.remaining(&key);
        assert!(
            rem >= Duration::from_millis(500),
            "observe must leave shared remaining, got {rem:?}"
        );

        // Kill switch still honored for new observes after disable.
        unsafe { std::env::set_var(DISABLE_ENV, "1") };
        let key2 = management_provider_key("https://management-api.x.ai", "other-key");
        assert!(observe_http_rate_limit(
            &key2,
            429,
            &headers,
            "should no-op"
        ));
        // Disabled store reports zero remaining for any key.
        assert_eq!(store.remaining(&key2), Duration::ZERO);

        match prev {
            Some(v) => unsafe { std::env::set_var(DISABLE_ENV, v) },
            None => unsafe { std::env::remove_var(DISABLE_ENV) },
        }
    }

    #[tokio::test]
    async fn wait_before_http_respects_shared_cooldown() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os(DISABLE_ENV);
        if prev.is_some() {
            unsafe { std::env::remove_var(DISABLE_ENV) };
        }

        let dir = TempDir::new().unwrap();
        let store = SharedRateLimitStore::open(dir.path()).unwrap();
        let _override = override_shared_store_for_test(store.clone());
        let key = billing_provider_key("https://proxy.test", "tok");
        store
            .observe(
                &key,
                Duration::from_millis(50),
                RateLimitMeta {
                    status: Some(429),
                    reason: Some("pre-seed".into()),
                },
            )
            .unwrap();
        assert!(store.remaining(&key) > Duration::ZERO);
        wait_before_http(&key).await;
        assert_eq!(store.remaining(&key), Duration::ZERO);

        match prev {
            Some(v) => unsafe { std::env::set_var(DISABLE_ENV, v) },
            None => unsafe { std::env::remove_var(DISABLE_ENV) },
        }
    }
}
