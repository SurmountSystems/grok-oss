//! First-party xAI console / Business API key store helpers.
//!
//! Interactive store writes require the OS keyring service `grok-build`
//! (time-boxed). A file mirror under `$GROK_HOME/provider_credentials.json`
//! (0600) is written only after a successful keyring write — not a silent
//! fallback when Secret Service is blocked. Env (`XAI_API_KEY`) wins and is
//! never written to the store.
//!
//! Multi-add: the store secret may hold a comma-separated list of keys
//! (`grok login --api-key` appends unique keys). Load-for-update is fail-closed
//! on keyring error so a stale empty file mirror cannot clobber existing keys.
//! List shows fingerprints only.
//!
//! Secrets are never accepted as CLI argv values (see [`super::secret_entry`]).
//! Interactive entry uses no-echo TTY reads.
//!
//! Used for dual-auth credit failover with SuperGrok OAuth (session primary,
//! console key failover). See `agent::config::resolve_credentials_preferring`.

use std::io;
use std::path::Path;

use super::credentials_store::{BEARER_USERNAME, CredentialsStore, CredentialsStoreError};

/// Default first-party inference base URL used as the store key.
pub const XAI_CONSOLE_API_URL: &str = "https://api.x.ai/v1";

/// Normalize the credential URL used as the store key.
pub fn credential_url(base_url: Option<&str>) -> String {
    let url = base_url
        .unwrap_or(XAI_CONSOLE_API_URL)
        .trim_end_matches('/');
    if url.is_empty() {
        XAI_CONSOLE_API_URL.to_owned()
    } else {
        url.to_owned()
    }
}

/// Load a stored console API key blob (store only; env is checked by callers).
///
/// May be a single key or a comma-separated multi-key list.
///
/// Uses fail-open [`CredentialsStore::read`] (agent resolve / status). For
/// multi-add RMW use [`load_stored_console_api_keys_for_update`].
pub fn load_stored_console_api_key(
    store: &CredentialsStore,
) -> Result<Option<String>, CredentialsStoreError> {
    let url = credential_url(None);
    Ok(store.read(&url)?.map(|(_, secret)| secret))
}

fn split_unique_console_keys(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in crate::agent::config::split_api_key_list(raw) {
        if !out.iter().any(|k| k == &part) {
            out.push(part);
        }
    }
    out
}

/// Ordered unique keys from the store secret (no env).
///
/// Fail-open read (resolve / list). Split acceptance matches resolve:
/// [`crate::agent::config::split_api_key_list`] (commas, `\n`, `\r`).
pub fn load_stored_console_api_keys(
    store: &CredentialsStore,
) -> Result<Vec<String>, CredentialsStoreError> {
    Ok(match load_stored_console_api_key(store)? {
        Some(raw) => split_unique_console_keys(&raw),
        None => Vec::new(),
    })
}

/// Ordered unique keys for multi-add RMW — fail-closed on keyring error/timeout.
///
/// Must not invent an empty list from a missing file mirror when the keyring
/// is unreachable (would clobber existing multi-key state on write).
pub fn load_stored_console_api_keys_for_update(
    store: &CredentialsStore,
) -> Result<Vec<String>, CredentialsStoreError> {
    let url = credential_url(None);
    Ok(match store.read_for_update(&url)? {
        Some((_, raw)) => split_unique_console_keys(&raw),
        None => Vec::new(),
    })
}

/// Store a console API key (replaces the store blob). Prefer
/// [`add_console_api_key`] for multi-add. Refuses when `XAI_API_KEY` / legacy
/// env is set (env wins; OpenRouter parity).
pub fn store_console_api_key(
    store: &CredentialsStore,
    api_key: &str,
) -> Result<(), XaiConsoleAuthError> {
    if crate::agent::auth_method::has_xai_api_key_env() {
        return Err(XaiConsoleAuthError::EnvVarSet);
    }
    let key = api_key.trim();
    if key.is_empty() {
        return Err(XaiConsoleAuthError::EmptyKey);
    }
    let url = credential_url(None);
    store
        .write(&url, BEARER_USERNAME, key)
        .map_err(XaiConsoleAuthError::Store)
}

/// Append a console API key to the multi-key store list (unique by exact key).
///
/// Returns `true` when the key was newly added; `false` when already present.
/// Refuses when env wins (same as [`store_console_api_key`]).
pub fn add_console_api_key(
    store: &CredentialsStore,
    api_key: &str,
) -> Result<bool, XaiConsoleAuthError> {
    if crate::agent::auth_method::has_xai_api_key_env() {
        return Err(XaiConsoleAuthError::EnvVarSet);
    }
    let key = api_key.trim();
    if key.is_empty() {
        return Err(XaiConsoleAuthError::EmptyKey);
    }
    // Fail-closed load: never RMW from an empty file view when keyring erred.
    let mut keys =
        load_stored_console_api_keys_for_update(store).map_err(XaiConsoleAuthError::Store)?;
    if keys.iter().any(|k| k == key) {
        return Ok(false);
    }
    keys.push(key.to_owned());
    let blob = keys.join(",");
    let url = credential_url(None);
    store
        .write(&url, BEARER_USERNAME, &blob)
        .map_err(XaiConsoleAuthError::Store)?;
    Ok(true)
}

/// Fingerprints of stored console keys only (never raw secrets). Empty when
/// store empty or unreadable.
pub fn list_console_api_key_fingerprints(store: &CredentialsStore) -> Vec<String> {
    load_stored_console_api_keys(store)
        .unwrap_or_default()
        .into_iter()
        .map(|k| fingerprint_console_key(&k))
        .collect()
}

/// True when an **inference** console / Business API key is available
/// (`XAI_API_KEY` env or secret store under `api.x.ai`).
///
/// This is **not** the Management API key used for team prepaid balance.
/// Surfaces that say "console path" must use this so a stored console key is
/// never shown as "not live / missing" when only the management key is absent.
pub fn console_inference_key_present(store: &CredentialsStore) -> bool {
    if crate::agent::auth_method::has_xai_api_key_env() {
        return true;
    }
    load_stored_console_api_keys(store)
        .map(|keys| !keys.is_empty())
        .unwrap_or(false)
}

/// Process-default store + env for [`console_inference_key_present`].
pub fn console_inference_key_present_default() -> bool {
    console_inference_key_present(&CredentialsStore::default_store())
}

/// Clear the stored console API key (does not unset env).
pub fn clear_console_api_key(store: &CredentialsStore) -> Result<(), CredentialsStoreError> {
    store.delete(&credential_url(None))
}

/// Fingerprint-only description for logs / CLI list (never the raw key).
pub fn fingerprint_console_key(key: &str) -> String {
    blake3::hash(key.trim().as_bytes()).to_hex().to_string()
}

/// `grok login --api-key` (console / Business) — multi-add into secret store.
///
/// `api_key` is `Some` only for library callers/tests or after stdin materialize
/// (`--api-key -`). Interactive CLI uses `None` → no-echo TTY prompt. Never
/// accepts raw argv secrets (bin refuses those before calling). Never prints
/// raw keys; lists fingerprints after store.
pub fn run_xai_console_login(
    grok_home: &Path,
    api_key: Option<&str>,
) -> Result<(), XaiConsoleAuthError> {
    let store = CredentialsStore::at_grok_home(grok_home);
    if crate::agent::auth_method::has_xai_api_key_env() {
        eprintln!(
            "XAI_API_KEY is set; console dual-auth uses the environment \
             (not writing to the secret store)."
        );
        eprintln!("Console authentication ready via XAI_API_KEY.");
        return Ok(());
    }
    let key = if let Some(k) = api_key {
        k.to_owned()
    } else {
        super::secret_entry::prompt_api_key_no_echo(
            "Enter your xAI console / Business API key (https://console.x.ai): ",
        )
        .map_err(XaiConsoleAuthError::Io)?
    };
    // After secret accept: show dual-backend budget progress while RMW+write
    // blocks (TTY stderr only; never prints secrets).
    let show_progress = super::secret_store_progress::should_show_secret_store_progress();
    let added = super::secret_store_progress::with_secret_store_progress(show_progress, || {
        add_console_api_key(&store, &key)
    })?;
    // Mirror into auth.json for legacy paths (fail-open). `store_api_key`
    // dual-writes via add_console_api_key (idempotent when already present).
    if let Err(e) = super::storage::store_api_key(grok_home, &key) {
        tracing::debug!(error = %e, "auth: could not mirror console key into auth.json");
    }
    let fp = fingerprint_console_key(&key);
    if added {
        eprintln!("Console API key saved (fingerprint {fp}).");
    } else {
        eprintln!("Console API key already stored (fingerprint {fp}).");
    }
    let fps = list_console_api_key_fingerprints(&store);
    if fps.len() > 1 {
        eprintln!("Stored console key fingerprints ({}):", fps.len());
        for (i, f) in fps.iter().enumerate() {
            eprintln!("  {}. {f}", i + 1);
        }
    }
    Ok(())
}

/// Print dual-auth discoverability + stored console key fingerprints (no raw keys).
///
/// Shows SuperGrok session presence, store key count/fingerprints, whether
/// `XAI_API_KEY` env wins, preferred method, and failover readiness.
pub fn run_list_console_api_keys(grok_home: &Path) -> Result<(), XaiConsoleAuthError> {
    let status = super::dual_auth_status::collect_dual_auth_status(grok_home);
    eprint!("{}", status.format_human());
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum XaiConsoleAuthError {
    #[error("XAI_API_KEY is set; refuse to write the secret store (env wins)")]
    EnvVarSet,
    #[error("API key is empty")]
    EmptyKey,
    #[error(transparent)]
    Store(#[from] CredentialsStoreError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::credentials_store::CredentialsStore;
    use tempfile::TempDir;
    use xai_grok_test_support::EnvGuard;

    #[test]
    #[serial_test::serial]
    fn store_and_load_round_trip() {
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));
        store_console_api_key(&store, "  console-secret-key  ").unwrap();
        let loaded = load_stored_console_api_key(&store).unwrap();
        assert_eq!(loaded.as_deref(), Some("console-secret-key"));
        clear_console_api_key(&store).unwrap();
        assert!(load_stored_console_api_key(&store).unwrap().is_none());
    }

    #[test]
    #[serial_test::serial]
    fn store_refuses_when_env_set() {
        let _key = EnvGuard::set("XAI_API_KEY", "env-key");
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));
        match store_console_api_key(&store, "should-not-write") {
            Err(XaiConsoleAuthError::EnvVarSet) => {}
            other => panic!("expected EnvVarSet, got {other:?}"),
        }
    }

    #[test]
    fn fingerprint_is_not_raw_key() {
        let fp = fingerprint_console_key("super-secret-console-key");
        assert!(!fp.contains("super-secret"));
        assert!(!fp.is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn console_inference_key_present_sees_store_and_env() {
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        assert!(
            !console_inference_key_present(&store),
            "empty store + no env → not present"
        );
        store_console_api_key(&store, "console-key-present-test").unwrap();
        assert!(
            console_inference_key_present(&store),
            "stored console key must count as present"
        );
        clear_console_api_key(&store).unwrap();
        assert!(!console_inference_key_present(&store));
        let _key = EnvGuard::set("XAI_API_KEY", "env-console-key");
        assert!(
            console_inference_key_present(&store),
            "env console key must count as present even with empty store"
        );
    }

    /// B2: multi-add append order is the store half of dual-auth console order
    /// (after `XAI_API_KEY` in `collect_xai_console_api_keys`). First added =
    /// first tried after SuperGrok hop when env is unset — add Business first.
    #[test]
    #[serial_test::serial]
    fn multi_add_console_keys_and_list_fingerprints_only() {
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join("creds.json"));

        assert!(add_console_api_key(&store, "key-alpha").unwrap());
        assert!(add_console_api_key(&store, "key-beta").unwrap());
        assert!(
            !add_console_api_key(&store, "key-alpha").unwrap(),
            "duplicate must not re-add"
        );

        let keys = load_stored_console_api_keys(&store).unwrap();
        assert_eq!(
            keys,
            vec!["key-alpha".to_string(), "key-beta".to_string()],
            "append order stable: first added is first in store list"
        );

        let fps = list_console_api_key_fingerprints(&store);
        assert_eq!(fps.len(), 2);
        assert_eq!(fps[0], fingerprint_console_key("key-alpha"));
        assert_eq!(fps[1], fingerprint_console_key("key-beta"));
        for fp in &fps {
            assert!(!fp.contains("key-alpha"));
            assert!(!fp.contains("key-beta"));
            assert!(!fp.contains("alpha"));
            assert!(!fp.contains("beta"));
        }

        // Blob remains comma-joined for resolve path split.
        let blob = load_stored_console_api_key(&store).unwrap().unwrap();
        assert!(blob.contains("key-alpha") && blob.contains("key-beta"));

        // CRLF / CR-only secrets use the same split as resolve (`split_api_key_list`).
        store_console_api_key(&store, "k1\r\nk2\rk3").unwrap();
        let from_cr = load_stored_console_api_keys(&store).unwrap();
        assert_eq!(
            from_cr,
            vec!["k1".to_string(), "k2".to_string(), "k3".to_string()],
            "store load must accept \\r like resolve split_api_key_list"
        );
    }
}
