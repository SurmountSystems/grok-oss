//! Dual-auth discoverability: session? N console keys? env wins?
//!
//! Counts and fingerprints only — never raw keys, tokens, emails, or secret
//! identifiers. Used by `grok login --list-api-keys`, doctor, and tests.

use std::path::Path;

use super::credentials_store::CredentialsStore;
use super::model::{
    API_KEY_SCOPE, AuthMode, SupergrokPrincipalListing, list_supergrok_principal_listings,
};
use super::storage::read_auth_json;
use super::xai_console::{fingerprint_console_key, list_console_api_key_fingerprints};

/// Snapshot of dual-auth readiness for operator visibility (no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualAuthStatus {
    /// True when `auth.json` has a non-API-key session credential (OIDC/External).
    pub session_present: bool,
    /// Human label for the session mode when present (`oidc`, `external`).
    pub session_mode: Option<&'static str>,
    /// SuperGrok OAuth principals (personal / business), labels + fingerprints only.
    /// Empty when no session; one entry for ordinary single login; two+ after
    /// multi SuperGrok login (personal + Business).
    pub supergrok_principals: Vec<SupergrokPrincipalListing>,
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
    /// Config pin label: `api_key`, `oauth`, or `None` (default session primary).
    pub preferred_method: Option<&'static str>,
    /// `[auth] auto_use_included_limits` — prefer included SuperGrok limits
    /// before $ extras; rank multi-identity by included headroom.
    pub auto_use_included_limits: bool,
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

        if self.supergrok_principals.is_empty() {
            match (self.session_present, self.session_mode) {
                (true, Some(mode)) => {
                    out.push_str(&format!("  SuperGrok session: yes ({mode})\n"));
                }
                (true, None) => out.push_str("  SuperGrok session: yes\n"),
                (false, _) => out.push_str("  SuperGrok session: no (run `grok login`)\n"),
            }
        } else if self.supergrok_principals.len() == 1 {
            let p = &self.supergrok_principals[0];
            out.push_str(&format!(
                "  SuperGrok session: yes ({role}, {mode})\n    fingerprint {fp}\n",
                role = p.role_label,
                mode = p.mode_label,
                fp = p.fingerprint,
            ));
        } else {
            out.push_str(&format!(
                "  SuperGrok sessions: {} (labels and fingerprints only)\n",
                self.supergrok_principals.len()
            ));
            for (i, p) in self.supergrok_principals.iter().enumerate() {
                out.push_str(&format!(
                    "    {}. {role} ({mode}) · fingerprint {fp}\n",
                    i + 1,
                    role = p.role_label,
                    mode = p.mode_label,
                    fp = p.fingerprint,
                ));
            }
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
            Some("oidc") | Some("oauth") => {
                out.push_str(
                    "  Preferred method: oauth (SuperGrok login primary when both exist)\n",
                );
            }
            Some(other) => out.push_str(&format!("  Preferred method: {other}\n")),
            None => {
                out.push_str("  Preferred method: default (session primary + console failover)\n");
            }
        }
        if self.auto_use_included_limits {
            out.push_str(
                "  Auto-use included limits: yes (prefer included SuperGrok weekly before $ extras / console; hop on exhaust; sooner reset ranks among included pools)\n",
            );
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
    let auto_use_included_limits = auto_use_included_limits_from_config();
    collect_dual_auth_status_with(grok_home, preferred, auto_use_included_limits)
}

/// Like [`collect_dual_auth_status`] but injects preferred-method for tests.
pub fn collect_dual_auth_status_with(
    grok_home: &Path,
    preferred_method: Option<&'static str>,
    auto_use_included_limits: bool,
) -> DualAuthStatus {
    let path = grok_home.join("auth.json");
    let map = read_auth_json(&path).ok();
    let supergrok_principals = map
        .as_ref()
        .map(list_supergrok_principal_listings)
        .unwrap_or_default();
    let (session_present, session_mode) = probe_session_from_map(map.as_ref());
    let store = CredentialsStore::at_grok_home(grok_home);
    let stored_fingerprints = list_console_api_key_fingerprints(&store);
    let stored_console_key_count = stored_fingerprints.len();
    let (env_var_present, env_wins, env_key_count) = probe_env_keys();

    DualAuthStatus {
        session_present,
        session_mode,
        supergrok_principals,
        stored_console_key_count,
        stored_fingerprints,
        env_key_count,
        env_var_present,
        env_wins,
        preferred_method,
        auto_use_included_limits,
    }
}

fn probe_session_from_map(map: Option<&super::model::AuthStore>) -> (bool, Option<&'static str>) {
    let Some(map) = map else {
        return (false, None);
    };
    // Prefer OIDC, then External; skip API-key scope and legacy WebLogin.
    for (scope, auth) in map {
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
        "api_key" | "console_api_key" | "api" | "key" => Some("api_key"),
        "oidc" | "oauth" | "oauth_token" => Some("oauth"),
        _ => None,
    }
}

fn auto_use_included_limits_from_config() -> bool {
    let Ok(value) = crate::config::load_effective_config_disk_only() else {
        return false;
    };
    let table_bool = |section: &str, key: &str| -> Option<bool> {
        value
            .get(section)
            .and_then(|t| t.get(key))
            .and_then(|v| v.as_bool())
    };
    table_bool("auth", "auto_use_included_limits")
        .or_else(|| table_bool("grok_com_config", "auto_use_included_limits"))
        // One-release dogfood alias.
        .or_else(|| table_bool("auth", "prefer_sooner_reset"))
        .or_else(|| table_bool("grok_com_config", "prefer_sooner_reset"))
        .unwrap_or(false)
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
        let st = collect_dual_auth_status_with(dir.path(), None, false);
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

        let st = collect_dual_auth_status_with(dir.path(), None, false);
        assert!(st.session_present);
        assert_eq!(st.session_mode, Some("oidc"));
        assert_eq!(st.stored_console_key_count, 2);
        assert!(st.dual_auth_ready());
        let text = st.format_human();
        assert!(text.contains("Failover: ready"), "{text}");
        // Single principal: role + mode + session fingerprint (not only "oidc").
        assert!(
            text.contains("SuperGrok session: yes") && text.contains("oidc"),
            "{text}"
        );
        assert!(text.contains("personal"), "{text}");
        // Never dump raw secrets
        assert!(!text.contains("console-key-alpha"), "{text}");
        assert!(!text.contains("console-key-beta"), "{text}");
        assert!(!text.contains("session-jwt"), "{text}");
        for fp in &st.stored_fingerprints {
            assert!(text.contains(fp.as_str()), "missing fingerprint in report");
        }
        assert_eq!(st.supergrok_principals.len(), 1);
    }

    #[test]
    #[serial_test::serial]
    fn env_wins_counted_without_raw_key() {
        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _key = EnvGuard::set("XAI_API_KEY", "env-secret-one,env-secret-two");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let st = collect_dual_auth_status_with(dir.path(), Some("api_key"), false);
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
        let st = collect_dual_auth_status_with(dir.path(), None, false);
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
            supergrok_principals: Vec::new(),
            stored_console_key_count: 1,
            stored_fingerprints: vec![fingerprint_console_key("raw-secret-key-xyz")],
            env_key_count: 0,
            env_var_present: false,
            env_wins: false,
            preferred_method: None,
            auto_use_included_limits: false,
        };
        let text = st.format_human();
        assert!(!text.contains("raw-secret"));
        assert!(!text.contains("xyz"));
        assert!(text.contains(&st.stored_fingerprints[0]));
    }

    /// Multi SuperGrok: doctor/list shows two fingerprints, never raw tokens.
    #[test]
    #[serial_test::serial]
    fn dual_supergrok_principals_listed_with_fingerprints_only() {
        use crate::auth::model::{
            fingerprint_session_token, list_supergrok_principal_listings, upsert_supergrok_session,
        };

        let _force = EnvGuard::set(FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let base = "https://auth.x.ai::multi-client";
        let mut map = AuthStore::default();
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "personal-jwt-secret-never-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-personal".into(),
                principal_type: None,
                team_id: None,
                ..Default::default()
            },
        );
        upsert_supergrok_session(
            &mut map,
            base,
            GrokAuth {
                key: "business-jwt-secret-never-print".into(),
                auth_mode: AuthMode::Oidc,
                user_id: "user-biz".into(),
                principal_type: Some("Team".into()),
                principal_id: Some("team-biz-1".into()),
                team_id: Some("team-biz-1".into()),
                ..Default::default()
            },
        );
        write_auth_json(&dir.path().join("auth.json"), &map).unwrap();

        let listings = list_supergrok_principal_listings(&map);
        assert_eq!(listings.len(), 2, "personal + business");
        let roles: Vec<_> = listings.iter().map(|p| p.role_label).collect();
        assert!(roles.contains(&"personal"), "{roles:?}");
        assert!(roles.contains(&"business"), "{roles:?}");

        let st = collect_dual_auth_status_with(dir.path(), None, true);
        assert!(st.session_present);
        assert_eq!(st.supergrok_principals.len(), 2);
        let text = st.format_human();
        assert!(text.contains("SuperGrok sessions: 2"), "{text}");
        assert!(text.contains("personal"), "{text}");
        assert!(text.contains("business"), "{text}");
        assert!(!text.contains("personal-jwt"), "{text}");
        assert!(!text.contains("business-jwt"), "{text}");
        assert!(!text.contains("never-print"), "{text}");
        let fp_p = fingerprint_session_token("personal-jwt-secret-never-print");
        let fp_b = fingerprint_session_token("business-jwt-secret-never-print");
        assert!(text.contains(&fp_p), "missing personal fingerprint");
        assert!(text.contains(&fp_b), "missing business fingerprint");
    }
}
