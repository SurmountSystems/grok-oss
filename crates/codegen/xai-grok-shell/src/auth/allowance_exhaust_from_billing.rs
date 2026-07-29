//! Leave SuperGrok when included weekly/monthly allowance is full (billing %).
//!
//! When included SuperGrok usage reports fully used and a console API key
//! failover path exists, mark the session JWT fingerprint out of allowance so
//! the sampler prefers the console key **before** the next request — without
//! waiting for HTTP 402 (extras would still succeed on SuperGrok and burn paid
//! balance).

use std::path::Path;

use super::dual_auth_status::collect_dual_auth_status;
use super::model::{API_KEY_SCOPE, AuthMode};
use super::storage::read_auth_json;

/// Load the SuperGrok/session access token from `auth.json` (OIDC or External).
///
/// Skips API-key and legacy WebLogin scopes. Returns the first non-empty key.
/// Used only to fingerprint for the exhausted-identity memo — never logged.
pub fn load_session_access_token(grok_home: &Path) -> Option<String> {
    let path = grok_home.join("auth.json");
    let map = read_auth_json(&path).ok()?;
    for (scope, auth) in &map {
        if scope == API_KEY_SCOPE {
            continue;
        }
        match auth.auth_mode {
            AuthMode::Oidc | AuthMode::External => {
                let k = auth.key.trim();
                if !k.is_empty() {
                    return Some(k.to_owned());
                }
            }
            AuthMode::ApiKey | AuthMode::WebLogin => continue,
        }
    }
    None
}

/// Apply billing `usage_pct` to the credit-exhausted memo when dual-auth is ready.
///
/// Safe no-op when session or console failover is missing. See
/// [`xai_grok_sampler::sync_allowance_exhaust_from_usage`].
pub fn apply_billing_usage_to_session_exhaust(
    usage_pct: f64,
    grok_home: &Path,
) -> xai_grok_sampler::AllowanceExhaustAction {
    let status = collect_dual_auth_status(grok_home);
    if !status.dual_auth_ready() {
        // Still allow clear of a prior mark if usage dropped but console key
        // was removed — only when we can fingerprint the session.
        let Some(token) = load_session_access_token(grok_home) else {
            return xai_grok_sampler::AllowanceExhaustAction::None;
        };
        return xai_grok_sampler::sync_allowance_exhaust_from_usage(
            usage_pct,
            Some(token.as_str()),
            false,
        );
    }
    let token = load_session_access_token(grok_home);
    let action =
        xai_grok_sampler::sync_allowance_exhaust_from_usage(usage_pct, token.as_deref(), true);
    if matches!(action, xai_grok_sampler::AllowanceExhaustAction::Marked) {
        tracing::info!(
            target: "xai_grok_shell::auth",
            usage_pct,
            "SuperGrok included usage full; remembering session out of allowance so next request uses console key"
        );
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::credentials_store::{CredentialsStore, FORCE_FILE_ENV};
    use crate::auth::model::{AuthStore, GrokAuth};
    use crate::auth::storage::write_auth_json;
    use crate::auth::xai_console::add_console_api_key;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use xai_grok_sampler::{AllowanceExhaustAction, clear_all_including_durable};
    use xai_grok_test_support::EnvGuard;

    /// Serialize tests that touch the process-global exhausted memo + env.
    fn with_isolated_home<R>(f: impl FnOnce(&Path) -> R) -> R {
        static LOCK: Mutex<()> = Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().expect("temp GROK_HOME");
        let _home = EnvGuard::set("GROK_HOME", dir.path());
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        // Operator host may have XAI_API_KEY set (env wins) — isolate tests.
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        clear_all_including_durable();
        let out = f(dir.path());
        clear_all_including_durable();
        out
    }

    fn write_oidc(home: &Path, key: &str) {
        let path = home.join("auth.json");
        let mut map = AuthStore::default();
        map.insert(
            "https://auth.x.ai::test-client".to_owned(),
            GrokAuth {
                key: key.into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-1".into(),
                ..Default::default()
            },
        );
        write_auth_json(&path, &map).unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn apply_billing_100_pct_marks_session_when_dual_auth_ready() {
        with_isolated_home(|home| {
            let session = "session-jwt-for-allowance-exhaust";
            write_oidc(home, session);
            // File-backed store (same path dual_auth_status probes under FORCE_FILE).
            let store = CredentialsStore::at_grok_home(home);
            assert!(add_console_api_key(&store, "console-failover-key").unwrap());

            assert_eq!(
                apply_billing_usage_to_session_exhaust(100.0, home),
                AllowanceExhaustAction::Marked
            );
            // Cleared only fires when a prior mark existed → proves mark.
            assert_eq!(
                apply_billing_usage_to_session_exhaust(12.0, home),
                AllowanceExhaustAction::Cleared
            );
            // Second clear is a no-op.
            assert_eq!(
                apply_billing_usage_to_session_exhaust(5.0, home),
                AllowanceExhaustAction::None
            );
        });
    }

    #[test]
    #[serial_test::serial]
    fn apply_billing_session_only_does_not_mark() {
        with_isolated_home(|home| {
            let session = "session-only-no-console";
            write_oidc(home, session);
            // No console key + env cleared → do not mark SuperGrok out.
            assert_eq!(
                apply_billing_usage_to_session_exhaust(100.0, home),
                AllowanceExhaustAction::None
            );
        });
    }

    #[test]
    fn load_session_skips_api_key_scope() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("auth.json");
        let mut map = AuthStore::default();
        map.insert(
            API_KEY_SCOPE.to_owned(),
            GrokAuth {
                key: "console-as-api-key-scope".into(),
                auth_mode: AuthMode::ApiKey,
                ..Default::default()
            },
        );
        write_auth_json(&path, &map).unwrap();
        assert!(load_session_access_token(dir.path()).is_none());
    }
}
