//! Dual-auth discoverability: session? N console keys? env wins?
//!
//! Counts and fingerprints only — never raw keys, tokens, emails, or secret
//! identifiers. Used by `grok login --list-api-keys`, doctor, and tests.

use std::path::Path;

use super::credentials_store::CredentialsStore;
use super::model::{API_KEY_SCOPE, AuthMode};
use super::storage::read_auth_json;
use super::xai_console::{fingerprint_console_key, list_console_api_key_fingerprints};

/// Snapshot of dual-auth readiness for operator visibility (no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualAuthStatus {
    /// True when `auth.json` has a non-API-key session credential (OIDC/External).
    pub session_present: bool,
    /// Human label for the session mode when present (`oidc`, `external`).
    pub session_mode: Option<&'static str>,
    /// Number of console keys in the secret store (fingerprints listed separately).
    pub stored_console_key_count: usize,
    /// Fingerprints of stored console keys only (never raw).
    pub stored_fingerprints: Vec<String>,
    /// Number of keys in `XAI_API_KEY` / legacy env (comma/newline-split).
    pub env_key_count: usize,
    /// True when the env var is present in the process environment (even if empty).
    pub env_var_present: bool,
    /// True when env has ≥1 usable key after split (env wins for console paths).
    pub env_wins: bool,
    /// Config pin label: `api_key`, `oidc`, or `None` (default session primary).
    pub preferred_method: Option<&'static str>,
}

impl DualAuthStatus {
    /// True when both a SuperGrok session and at least one console key path exist.
    pub fn dual_auth_ready(&self) -> bool {
        self.session_present && (self.stored_console_key_count > 0 || self.env_key_count > 0)
    }

    /// Console key paths available for failover (store and/or env).
    pub fn console_key_paths_present(&self) -> bool {
        self.stored_console_key_count > 0 || self.env_key_count > 0
    }

    /// Format a multi-line human report (stderr / doctor). No secrets.
    pub fn format_human(&self) -> String {
        let mut out = String::new();
        out.push_str("Dual-auth status (counts and fingerprints only; no secrets)\n");

        match (self.session_present, self.session_mode) {
            (true, Some(mode)) => {
                out.push_str(&format!("  SuperGrok session: yes ({mode})\n"));
            }
            (true, None) => out.push_str("  SuperGrok session: yes\n"),
            (false, _) => out.push_str("  SuperGrok session: no (run `grok login`)\n"),
        }

        if self.stored_console_key_count == 0 {
            out.push_str("  Console keys (store): 0\n");
        } else {
            out.push_str(&format!(
                "  Console keys (store): {}\n",
                self.stored_console_key_count
            ));
            for (i, fp) in self.stored_fingerprints.iter().enumerate() {
                out.push_str(&format!("    {}. {fp}\n", i + 1));
            }
        }

        if self.env_wins {
            out.push_str(&format!(
                "  XAI_API_KEY env: set ({} key{}; env wins over store)\n",
                self.env_key_count,
                if self.env_key_count == 1 { "" } else { "s" }
            ));
        } else if self.env_var_present {
            out.push_str(
                "  XAI_API_KEY env: set but empty (not usable; unset to use store keys)\n",
            );
        } else {
            out.push_str("  XAI_API_KEY env: not set\n");
        }

        match self.preferred_method {
            Some("api_key") => {
                out.push_str("  Preferred method: api_key (console primary when both exist)\n");
            }
            Some("oidc") => {
                out.push_str("  Preferred method: oidc (session primary when both exist)\n");
            }
            Some(other) => out.push_str(&format!("  Preferred method: {other}\n")),
            None => {
                out.push_str("  Preferred method: default (session primary + console failover)\n");
            }
        }

        if self.dual_auth_ready() {
            out.push_str("  Failover: ready (session + console key path)\n");
        } else if self.session_present && !self.console_key_paths_present() {
            out.push_str(
                "  Failover: session only — add a console key (`grok login --api-key`) or set XAI_API_KEY\n",
            );
        } else if !self.session_present && self.console_key_paths_present() {
            out.push_str("  Failover: console key only — run `grok login` for SuperGrok session\n");
        } else {
            out.push_str("  Failover: none configured\n");
        }

        out
    }
}

/// Probe dual-auth status under `$GROK_HOME` (and process env). Never reads raw
/// key material into the returned struct beyond fingerprinting store keys.
pub fn collect_dual_auth_status(grok_home: &Path) -> DualAuthStatus {
    let preferred = preferred_method_label();
    collect_dual_auth_status_with(grok_home, preferred)
}

/// Like [`collect_dual_auth_status`] but injects preferred-method for tests.
pub fn collect_dual_auth_status_with(
    grok_home: &Path,
    preferred_method: Option<&'static str>,
) -> DualAuthStatus {
    let (session_present, session_mode) = probe_session(grok_home);
    let store = CredentialsStore::at_grok_home(grok_home);
    let stored_fingerprints = list_console_api_key_fingerprints(&store);
    let stored_console_key_count = stored_fingerprints.len();
    let (env_var_present, env_wins, env_key_count) = probe_env_keys();

    DualAuthStatus {
        session_present,
        session_mode,
        stored_console_key_count,
        stored_fingerprints,
        env_key_count,
        env_var_present,
        env_wins,
        preferred_method,
    }
}

fn probe_session(grok_home: &Path) -> (bool, Option<&'static str>) {
    let path = grok_home.join("auth.json");
    let Ok(map) = read_auth_json(&path) else {
        return (false, None);
    };
    // Prefer OIDC, then External; skip API-key scope and legacy WebLogin.
    for (scope, auth) in &map {
        if scope == API_KEY_SCOPE {
            continue;
        }
        match auth.auth_mode {
            AuthMode::Oidc => return (true, Some("oidc")),
            AuthMode::External => return (true, Some("external")),
            AuthMode::ApiKey | AuthMode::WebLogin => continue,
        }
    }
    (false, None)
}

/// Returns `(env_var_present, env_wins, env_key_count)`.
///
/// Empty / whitespace-only env is present but not usable — do not claim
/// "env wins" (store keys remain valid for failover discoverability).
fn probe_env_keys() -> (bool, bool, usize) {
    match crate::agent::auth_method::read_xai_api_key_env() {
        Ok(raw) => {
            let n = crate::agent::config::split_api_key_list(&raw).len();
            (true, n > 0, n)
        }
        Err(_) => (false, false, 0),
    }
}

fn preferred_method_label() -> Option<&'static str> {
    // Fail-open: config load is optional for discoverability.
    let value = crate::config::load_effective_config_disk_only().ok()?;
    // Config.toml: `[auth] preferred_method` (alias) or `[grok_com_config]`.
    let method = value
        .get("auth")
        .and_then(|t| t.get("preferred_method"))
        .or_else(|| {
            value
                .get("grok_com_config")
                .and_then(|t| t.get("preferred_method"))
        })
        .and_then(|v| v.as_str())?;
    match method {
        "api_key" => Some("api_key"),
        "oidc" => Some("oidc"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::credentials_store::{CredentialsStore, FORCE_FILE_ENV};
    use crate::auth::model::{AuthMode, AuthStore, GrokAuth};
    use crate::auth::storage::write_auth_json;
    use crate::auth::xai_console::add_console_api_key;
    use tempfile::TempDir;
    use xai_grok_test_support::EnvGuard;

    fn write_oidc_session(home: &Path) {
        let path = home.join("auth.json");
        let mut map = AuthStore::default();
        map.insert(
            "https://auth.x.ai::test-client".to_owned(),
            GrokAuth {
                key: "session-jwt-secret-not-listed".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-1".into(),
                ..Default::default()
            },
        );
        write_auth_json(&path, &map).unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn empty_home_reports_no_session_no_keys() {
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let st = collect_dual_auth_status_with(dir.path(), None);
        assert!(!st.session_present);
        assert_eq!(st.stored_console_key_count, 0);
        assert!(!st.env_var_present);
        assert!(!st.env_wins);
        assert!(!st.dual_auth_ready());
        let text = st.format_human();
        assert!(text.contains("SuperGrok session: no"), "{text}");
        assert!(text.contains("Console keys (store): 0"), "{text}");
        assert!(!text.contains("session-jwt"), "{text}");
    }

    #[test]
    #[serial_test::serial]
    fn session_plus_store_keys_is_dual_ready() {
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        write_oidc_session(dir.path());
        // at_path forces file backend (no keyring hang in CI).
        let store = CredentialsStore::at_path(dir.path().join("provider_credentials.json"));
        assert!(add_console_api_key(&store, "console-key-alpha").unwrap());
        assert!(add_console_api_key(&store, "console-key-beta").unwrap());

        let st = collect_dual_auth_status_with(dir.path(), None);
        assert!(st.session_present);
        assert_eq!(st.session_mode, Some("oidc"));
        assert_eq!(st.stored_console_key_count, 2);
        assert!(st.dual_auth_ready());
        let text = st.format_human();
        assert!(text.contains("Failover: ready"), "{text}");
        assert!(text.contains("SuperGrok session: yes (oidc)"), "{text}");
        // Never dump raw secrets
        assert!(!text.contains("console-key-alpha"), "{text}");
        assert!(!text.contains("console-key-beta"), "{text}");
        assert!(!text.contains("session-jwt"), "{text}");
        for fp in &st.stored_fingerprints {
            assert!(text.contains(fp.as_str()), "missing fingerprint in report");
        }
    }

    #[test]
    #[serial_test::serial]
    fn env_wins_counted_without_raw_key() {
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _key = EnvGuard::set("XAI_API_KEY", "env-secret-one,env-secret-two");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let st = collect_dual_auth_status_with(dir.path(), Some("api_key"));
        assert!(st.env_var_present);
        assert!(st.env_wins);
        assert_eq!(st.env_key_count, 2);
        assert_eq!(st.preferred_method, Some("api_key"));
        let text = st.format_human();
        assert!(text.contains("env wins"), "{text}");
        assert!(text.contains("2 key"), "{text}");
        assert!(!text.contains("env-secret"), "{text}");
        assert!(text.contains("Preferred method: api_key"), "{text}");
    }

    #[test]
    #[serial_test::serial]
    fn empty_env_does_not_claim_env_wins() {
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _key = EnvGuard::set("XAI_API_KEY", "  , \n");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let st = collect_dual_auth_status_with(dir.path(), None);
        assert!(st.env_var_present);
        assert!(!st.env_wins);
        assert_eq!(st.env_key_count, 0);
        let text = st.format_human();
        assert!(text.contains("set but empty"), "{text}");
        assert!(!text.contains("env wins"), "{text}");
    }

    #[test]
    fn format_human_never_includes_raw_fingerprinted_key() {
        let st = DualAuthStatus {
            session_present: false,
            session_mode: None,
            stored_console_key_count: 1,
            stored_fingerprints: vec![fingerprint_console_key("raw-secret-key-xyz")],
            env_key_count: 0,
            env_var_present: false,
            env_wins: false,
            preferred_method: None,
        };
        let text = st.format_human();
        assert!(!text.contains("raw-secret"));
        assert!(!text.contains("xyz"));
        assert!(text.contains(&st.stored_fingerprints[0]));
    }
}
