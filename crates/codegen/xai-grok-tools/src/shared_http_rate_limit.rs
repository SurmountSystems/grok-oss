//! Shared multi-process HTTP cooldowns for product tools that call rate-limited APIs.
//!
//! Product IPC is the flock JSON store in [`grok_rate_limit`] under
//! `$GROK_HOME/rate_limits/` (same path the sampler, billing, Management, and
//! GitHub update use). Type-appropriate keys use
//! [`grok_rate_limit::api_class`] so Imagine / video / responses do not share
//! cooldown files with each other or with chat inference.
//!
//! Public docs (header-driven waits; accessed 2026-08-03):
//! - <https://docs.x.ai/developers/rate-limits>
//! - <https://openrouter.ai/docs/api_reference/limits>

use grok_rate_limit::{
    ProviderKey, SharedRateLimitStore, api_class, fingerprint_secret, observe_status,
    should_observe_status, wait_from_header_values,
};

/// Re-export default wait for callers that need the constant.
pub use grok_rate_limit::DEFAULT_RATE_LIMIT_WAIT;

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

/// Build a provider key: host + secret fingerprint + API class.
///
/// Raw secrets never appear in key strings / filenames.
pub fn provider_key(base_url: &str, bearer: Option<&str>, class: &str) -> ProviderKey {
    let fp = bearer
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(fingerprint_secret)
        .unwrap_or_default();
    ProviderKey::from_base_url_fingerprint_and_class(base_url, &fp, class)
}

/// Imagine image generate / edit bucket.
pub fn imagine_provider_key(base_url: &str, bearer: Option<&str>) -> ProviderKey {
    provider_key(base_url, bearer, api_class::IMAGINE)
}

/// Imagine video start / poll bucket.
pub fn video_provider_key(base_url: &str, bearer: Option<&str>) -> ProviderKey {
    provider_key(base_url, bearer, api_class::VIDEO)
}

/// Responses API (web_search) bucket.
pub fn responses_provider_key(base_url: &str, bearer: Option<&str>) -> ProviderKey {
    provider_key(base_url, bearer, api_class::RESPONSES)
}

/// Prefer `Retry-After` seconds; else optional `x-ratelimit-reset` unix epoch.
pub fn wait_from_rate_limit_headers(
    headers: &reqwest::header::HeaderMap,
) -> Option<std::time::Duration> {
    let retry_after = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok());
    let reset = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok());
    wait_from_header_values(retry_after, reset)
}

/// Whether this status should publish a shared cooldown.
pub fn should_observe_rate_limit(status: u16, headers: &reqwest::header::HeaderMap) -> bool {
    should_observe_status(status, wait_from_rate_limit_headers(headers).is_some())
}

/// Wait on the shared cooldown before issuing HTTP.
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
    let retry_after = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok());
    let reset = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok());
    observe_status(&shared_store(), key, status, retry_after, reset, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    use grok_rate_limit::{DISABLE_ENV, RateLimitMeta, shared_rate_limits_disabled};
    use tempfile::TempDir;

    static ENV_LOCK: StdMutex<()> = StdMutex::new(());

    fn with_shared_limits_enabled<R>(f: impl FnOnce() -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os(DISABLE_ENV);
        if prev.is_some() {
            unsafe { std::env::remove_var(DISABLE_ENV) };
        }
        let out = f();
        if let Some(v) = prev {
            unsafe { std::env::set_var(DISABLE_ENV, v) };
        }
        out
    }

    #[test]
    fn imagine_key_uses_class_and_fingerprint_not_raw_secret() {
        let secret = "xai-imagine-key-DEADBEEF-not-for-disk";
        let key = imagine_provider_key("https://api.x.ai/v1", Some(secret));
        let s = key.as_str();
        assert!(s.contains("imagine"), "got {s}");
        assert!(!s.contains(secret), "raw secret in key: {s}");
        assert!(!s.contains("DEADBEEF"), "secret fragment in key: {s}");
        let chat_shaped = ProviderKey::from_base_url_and_key_fingerprint(
            "https://api.x.ai/v1",
            &fingerprint_secret(secret),
        );
        assert_ne!(
            s,
            chat_shaped.as_str(),
            "imagine must not share chat cooldown file"
        );
    }

    #[test]
    fn video_and_responses_keys_differ() {
        let secret = "same-bearer";
        let v = video_provider_key("https://api.x.ai/v1", Some(secret));
        let r = responses_provider_key("https://api.x.ai/v1", Some(secret));
        assert_ne!(v.as_str(), r.as_str());
    }

    #[test]
    fn should_observe_429_always_403_only_with_retry_hint() {
        let mut headers = reqwest::header::HeaderMap::new();
        assert!(should_observe_rate_limit(429, &headers));
        assert!(!should_observe_rate_limit(403, &headers));
        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("30"),
        );
        assert!(should_observe_rate_limit(403, &headers));
    }

    #[tokio::test]
    // Process-env serialization across awaits is intentional for this hermetic test.
    #[allow(clippy::await_holding_lock)]
    async fn observe_and_wait_share_across_handles() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os(DISABLE_ENV);
        if prev.is_some() {
            unsafe { std::env::remove_var(DISABLE_ENV) };
        }
        assert!(!shared_rate_limits_disabled());

        let dir = TempDir::new().unwrap();
        let store = SharedRateLimitStore::open(dir.path()).unwrap();
        let _override = override_shared_store_for_test(store.clone());
        let key = imagine_provider_key("https://api.x.ai/v1", Some("hermetic-key"));

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("2"),
        );
        assert!(observe_http_rate_limit(
            &key,
            429,
            &headers,
            "imagine rate limit",
        ));
        let peer = SharedRateLimitStore::open(dir.path()).unwrap();
        let rem = peer.remaining(&key);
        assert!(
            rem >= Duration::from_millis(500),
            "peer must see cooldown, got {rem:?}"
        );

        // Short wait path: pre-seed tiny cooldown then wait_before_http.
        let key2 = video_provider_key("https://api.x.ai/v1", Some("wait-key"));
        store
            .observe(
                &key2,
                Duration::from_millis(40),
                RateLimitMeta {
                    status: Some(429),
                    reason: Some("pre-seed".into()),
                },
            )
            .unwrap();
        wait_before_http(&key2).await;
        assert_eq!(store.remaining(&key2), Duration::ZERO);

        match prev {
            Some(v) => unsafe { std::env::set_var(DISABLE_ENV, v) },
            None => unsafe { std::env::remove_var(DISABLE_ENV) },
        }
    }

    #[test]
    fn two_store_handles_share_imagine_shaped_cooldown() {
        with_shared_limits_enabled(|| {
            let dir = TempDir::new().unwrap();
            let a = SharedRateLimitStore::open(dir.path()).unwrap();
            let b = SharedRateLimitStore::open(dir.path()).unwrap();
            let key = imagine_provider_key("https://api.x.ai/v1", Some("sess-a"));
            a.observe(
                &key,
                Duration::from_secs(15),
                RateLimitMeta {
                    status: Some(429),
                    reason: Some("from-a".into()),
                },
            )
            .unwrap();
            let rem_b = b.remaining(&key);
            assert!(
                rem_b >= Duration::from_secs(10),
                "peer B must see A's imagine cooldown, got {rem_b:?}"
            );
        });
    }
}
