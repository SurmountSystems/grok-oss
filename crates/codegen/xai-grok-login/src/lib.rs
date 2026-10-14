#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unreachable_code,
    dead_code
)]
//! Authentication subsystem for the grok shell crate family.
//!
//! Extracted from `xai-grok-shell::auth`; the shell re-exports this crate as
//! `xai_grok_shell::auth` so existing `crate::*` paths keep resolving.
#![deny(clippy::indexing_slicing)]
pub use xai_grok_telemetry::unified_log;
pub mod api_key_probe;
pub mod attribution;
pub mod auth_method;
pub mod auth_provider;
pub mod backend;
pub mod config;
pub mod credential_provider;
pub mod credentials_store;
pub mod device_code;
pub mod error;
pub mod external_auth;
pub mod flow;
pub mod grok_auth_credentials;
pub mod harness_secrets;
pub mod jwt;
pub mod manager;
pub mod model;
pub mod oidc;
pub mod openrouter;
pub mod pre_tui;
pub mod recovery;
pub mod refresh;
pub mod secret_entry {
    //! No-echo API key prompt. This crate does not depend on `rpassword`.

    use std::io::{self, BufRead, Write};

    /// No-echo prompt for an API key.
    ///
    /// On Unix, disable terminal echo when stdin is a tty. Otherwise read one line.
    pub fn prompt_api_key_no_echo(prompt: &str) -> io::Result<String> {
        eprint!("{prompt}");
        io::stderr().flush()?;
        #[cfg(unix)]
        {
            read_no_echo_unix()
        }
        #[cfg(not(unix))]
        {
            read_line_trimmed()
        }
    }

    fn read_line_trimmed() -> io::Result<String> {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        Ok(line.trim().to_owned())
    }

    #[cfg(unix)]
    fn read_no_echo_unix() -> io::Result<String> {
        use std::io::IsTerminal;
        if !io::stdin().is_terminal() {
            return read_line_trimmed();
        }
        let fd = libc::STDIN_FILENO;
        // SAFETY: tcgetattr writes `original` before any read. On failure we return without reading it.
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
            return read_line_trimmed();
        }
        let mut hidden = original;
        hidden.c_lflag &= !libc::ECHO;
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &hidden) } != 0 {
            return read_line_trimmed();
        }
        let result = read_line_trimmed();
        // SAFETY: `original` is the termios tcgetattr read from this stdin fd.
        unsafe {
            libc::tcsetattr(fd, libc::TCSANOW, &original);
        }
        eprintln!();
        result
    }
}
pub mod secret_store_progress {
    //! Stderr progress while interactive login blocks on OS secret-store RMW+write.
    //!
    //! Budget is dual-backend worst case: 2 times [`KEYRING_OP_TIMEOUT`] (6s).

    use std::io::{self, IsTerminal, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::credentials_store::KEYRING_OP_TIMEOUT;

    /// Wall-clock progress budget for one interactive store op family.
    pub fn secret_store_progress_budget() -> Duration {
        KEYRING_OP_TIMEOUT.saturating_mul(2)
    }

    /// Whether interactive login should show a secret-store progress line.
    pub fn should_show_secret_store_progress() -> bool {
        io::stderr().is_terminal()
    }

    /// Format a single progress line (no trailing newline). No secrets.
    pub fn format_secret_store_progress(elapsed: Duration, budget: Duration) -> String {
        let budget_secs = budget.as_secs().max(1);
        let elapsed_secs = elapsed.as_secs().min(budget_secs);
        let width: u64 = 8;
        let filled = elapsed_secs
            .saturating_mul(width)
            .checked_div(budget_secs)
            .unwrap_or(width)
            .min(width);
        let empty = width.saturating_sub(filled);
        let bar: String = std::iter::repeat_n('=', filled as usize)
            .chain(std::iter::repeat_n('-', empty as usize))
            .collect();
        format!("Saving to OS secret store... [{bar}] {elapsed_secs}s / {budget_secs}s")
    }

    /// Clear the current progress line on stderr.
    pub fn clear_secret_store_progress_line() {
        eprint!("\r\x1b[K");
        let _ = io::stderr().flush();
    }

    /// Run `op` while optionally showing a stderr second-counter up to the budget.
    pub fn with_secret_store_progress<T>(show: bool, op: impl FnOnce() -> T) -> T {
        if !show {
            return op();
        }
        let budget = secret_store_progress_budget();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_tick = Arc::clone(&stop);
        let started = Instant::now();
        let ticker = thread::Builder::new()
            .name("grok-secret-store-progress".into())
            .spawn(move || {
                loop {
                    if stop_tick.load(Ordering::Relaxed) {
                        break;
                    }
                    let line = format_secret_store_progress(started.elapsed(), budget);
                    eprint!("\r{line}");
                    let _ = io::stderr().flush();
                    thread::sleep(Duration::from_millis(250));
                }
            })
            .ok();

        let out = op();
        stop.store(true, Ordering::Relaxed);
        if let Some(handle) = ticker {
            let _ = handle.join();
        }
        clear_secret_store_progress_line();
        out
    }
}
pub mod side_call_bearer;
pub mod single_flight;
pub mod storage;
pub mod token_output;
pub mod token_type;
pub mod xai_console {
    //! First-party console API key store helpers used by credential read-modify-write.

    use crate::auth_method::has_xai_api_key_env;
    use crate::credentials_store::{BEARER_USERNAME, CredentialsStore, CredentialsStoreError};

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

    fn split_api_key_list(raw: &str) -> Vec<String> {
        let mut out = Vec::new();
        for part in raw.split([',', '\n', '\r']) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if !out.iter().any(|existing| existing == part) {
                out.push(part.to_owned());
            }
        }
        out
    }

    /// Append a console API key. Fail-closed when the keyring read errors.
    ///
    /// Returns `true` when the key was newly added.
    pub fn add_console_api_key(
        store: &CredentialsStore,
        api_key: &str,
    ) -> Result<bool, XaiConsoleAuthError> {
        if has_xai_api_key_env() {
            return Err(XaiConsoleAuthError::EnvVarSet);
        }
        let key = api_key.trim();
        if key.is_empty() {
            return Err(XaiConsoleAuthError::EmptyKey);
        }
        let url = credential_url(None);
        let mut keys = match store.read_for_update(&url)? {
            Some((_, raw)) => split_api_key_list(&raw),
            None => Vec::new(),
        };
        if keys.iter().any(|existing| existing == key) {
            return Ok(false);
        }
        keys.push(key.to_owned());
        store.write(&url, BEARER_USERNAME, &keys.join(","))?;
        Ok(true)
    }

    /// Load a stored console API key blob (store only; env is checked by callers).
    pub fn load_stored_console_api_key(
        store: &CredentialsStore,
    ) -> Result<Option<String>, CredentialsStoreError> {
        let url = credential_url(None);
        Ok(store.read(&url)?.map(|(_, secret)| secret))
    }

    /// Ordered unique keys from the store secret (no env).
    pub fn load_stored_console_api_keys(
        store: &CredentialsStore,
    ) -> Result<Vec<String>, CredentialsStoreError> {
        Ok(match load_stored_console_api_key(store)? {
            Some(raw) => split_api_key_list(&raw),
            None => Vec::new(),
        })
    }

    /// Ordered unique keys for multi-add read-modify-write. Fail-closed on keyring error.
    pub fn load_stored_console_api_keys_for_update(
        store: &CredentialsStore,
    ) -> Result<Vec<String>, CredentialsStoreError> {
        let url = credential_url(None);
        Ok(match store.read_for_update(&url)? {
            Some((_, raw)) => split_api_key_list(&raw),
            None => Vec::new(),
        })
    }

    /// Store a console API key (replaces the store blob). Refuses when `XAI_API_KEY` is set.
    pub fn store_console_api_key(
        store: &CredentialsStore,
        api_key: &str,
    ) -> Result<(), XaiConsoleAuthError> {
        if has_xai_api_key_env() {
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

    /// Fingerprints of stored console keys. Empty when the store is empty or unreadable.
    pub fn list_console_api_key_fingerprints(store: &CredentialsStore) -> Vec<String> {
        load_stored_console_api_keys(store)
            .unwrap_or_default()
            .into_iter()
            .map(|key| fingerprint_console_key(&key))
            .collect()
    }

    /// True when an inference console key is in the environment or the secret store.
    pub fn console_inference_key_present(store: &CredentialsStore) -> bool {
        if has_xai_api_key_env() {
            return true;
        }
        load_stored_console_api_keys(store)
            .map(|keys| !keys.is_empty())
            .unwrap_or(false)
    }

    /// Process-default store plus env for [`console_inference_key_present`].
    pub fn console_inference_key_present_default() -> bool {
        console_inference_key_present(&CredentialsStore::default_store())
    }

    /// Fingerprint-only description for logs. Never the raw key.
    pub fn fingerprint_console_key(key: &str) -> String {
        blake3::hash(key.trim().as_bytes()).to_hex().to_string()
    }

    #[derive(Debug, thiserror::Error)]
    pub enum XaiConsoleAuthError {
        #[error("XAI_API_KEY is set; refuse to write the secret store (env wins)")]
        EnvVarSet,
        #[error("API key is empty")]
        EmptyKey,
        #[error(transparent)]
        Store(#[from] CredentialsStoreError),
    }
}

/// Shell paths (`crate::auth::...`) resolve in this crate, which is the auth crate.
pub mod auth {
    pub use crate::credentials_store;
    pub use crate::harness_secrets;
    pub use crate::manager;
    pub use crate::oidc;
    pub use crate::xai_console;
}
pub use api_key_probe::{
    DEFAULT_PROBE_TIMEOUT, first_party_env_key_allows_advertise, should_probe_first_party_env_key,
};
pub use auth_provider::AuthProviderRef;
pub use auth_provider::{
    PROVIDER_TIMEOUT_CEILING_SECS, PROVIDER_TOKEN_EXPIRY_SKEW_SECS, ProviderRefreshOutcome,
};
#[cfg(any(test, feature = "test-support"))]
pub use auth_provider::{test_backdate_provider_mint, test_counting_provider};
pub use config::LEGACY_AUTH_SCOPE;
pub use config::{
    CLI_CHAT_PROXY_BASE_URL_DEFAULT, ForceLoginTeam, GrokComConfig, OAuth2ProviderConfig,
    OidcAuthConfig, PreferredAuthMethod, XAI_OAUTH2_ISSUER, expand_auth_alias,
    is_xai_oauth2_issuer, xai_oauth2_issuer,
};
pub use config::{
    force_login_team_from_env, force_login_team_from_requirements,
    force_login_team_from_requirements_value, resolve_force_login_team,
};
pub use external_auth::{ExternalRefreshError, parse_output, refresh_with_command};
pub use flow::{
    AuthChannels, mint_session_noninteractive, run_auth_flow, run_auth_flow_with_stderr_bridge,
    try_noninteractive_auth_no_mint,
};
pub use flow::{
    AuthUrlInfo, AuthUrlMode, LoginTransportOverride, LogoutResult, ensure_authenticated,
    ensure_authenticated_or_noninteractive, ensure_authenticated_with_override, perform_logout,
    run_cli_login, try_ensure_fresh_auth,
};
pub use jwt::{is_jwt_expired_or_near, parse_jwt_expiration, parse_jwt_subject};
pub use pre_tui::{PreTuiLoginOutcome, maybe_run_pre_tui_external_login};
pub use xai_grok_config_types::AuthProviderConfig;
pub mod meta;
pub use error::{AuthError, RefreshTokenError, RefreshTokenFailedReason};
pub use manager::AuthManager;
pub use manager::{
    AuthRemedy, CachedTokenState, Login, LoginChanges, LoginSnapshot, SilentRefresh,
};
pub use meta::{AuthMeta, GateInfo};
pub use model::{AuthMode, GrokAuth, lookup_auth};
pub use model::{TOKEN_TTL, UserInfo, default_coding_data_retention_opt_out, is_expired};
pub use refresh::DiagnosticUploader;
pub use side_call_bearer::{SharedAuthKeyProvider, shared_api_key_provider};
pub use storage::auth_json_path;
pub use storage::{clear_api_key, read_api_key, read_auth_json, store_api_key};
