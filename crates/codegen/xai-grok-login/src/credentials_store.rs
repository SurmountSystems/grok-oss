//! URL-keyed secret storage for third-party provider API keys (**Grok-owned**).
//!
//! Conceptually matches Zed's API-key model (lookup by API base URL, username
//! `"Bearer"`, secret = raw key bytes) without depending on GPUI. **Storage
//! layouts differ** from Zed's OS keychain (different service label /
//! attributes), so reading Zed-saved keys is handled separately in
//! [`crate::auth::harness_secrets`].
//!
//! 1. **OS keyring** (service label [`SERVICE_NAME`] = `grok-build`) — real store
//!    for interactive credential login (`grok login --api-key`, OpenRouter login)
//! 2. **File mirror** `$GROK_HOME/provider_credentials.json` (mode 0600 on Unix)
//!    written only **after** a successful keyring write (logout/read resilience)
//!
//! Interactive login writes **require** a secure keyring backend (never a silent
//! plaintext file dump). Backend order:
//! 1. **Primary** — platform default (`keyring` v1: Secret Service on Linux,
//!    Keychain / Credential Manager elsewhere), time-boxed
//! 2. **Fallback (Linux)** — kernel keyutils (no D-Bus; works when Secret
//!    Service is locked or hung). Automatic on primary timeout/error.
//!
//! If **all** secure backends fail, the store fails loudly and does **not**
//! write the secret to the file. [`FORCE_FILE_ENV`] / [`CredentialsStore::at_path`]
//! use the file backend only (tests and headless CI) — not a user recovery path.
//!
//! All OS keyring get/set/delete calls are **time-boxed** ([`KEYRING_OP_TIMEOUT`])
//! so a blocked Secret Service / D-Bus cannot hang `grok login` forever.
//!
//! **Read vs RMW:** [`CredentialsStore::read`] fail-opens to the file mirror when
//! keyring is unavailable (agent resolve). [`CredentialsStore::read_for_update`]
//! fail-closes on keyring error/timeout so multi-add RMW cannot invent an empty
//! key list and clobber keyring state on a later successful write.
//!
//! **Circuit breaker (resolve only):** after a keyring timeout/error, resolve
//! [`CredentialsStore::read`] skips further keyring probes for a short TTL.
//! RMW [`CredentialsStore::read_for_update`] and writes always probe so
//! interactive multi-add login can recover when Secret Service is healthy again.
//!
//! **Write-after-timeout race:** on timeout the parent returns
//! [`CredentialsStoreError::KeyringTimeout`] and abandons the worker. If Secret
//! Service was only slow, the worker may still `set_password` later — login
//! reported failure and skipped the file mirror, but the secret can land in
//! keyring asynchronously (“failed but stored”). Inherent to abandon-on-timeout
//! without D-Bus cancellation. The abandoned worker also retains the secret in
//! thread memory until the blocking call finishes or the process exits.
//!
//! Environment variables for specific providers (e.g. `OPENROUTER_API_KEY`) are
//! checked by the provider helpers, not this store. When an env key is set,
//! callers should refuse to write the store (Zed parity).

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Keyring / file service label. Distinct from Zed's historical
/// `zed-github-account` so Grok credentials do not collide with Zed's.
pub const SERVICE_NAME: &str = "grok-build";

/// Username role stored with Bearer API keys (matches Zed's `ApiKeyState`).
pub const BEARER_USERNAME: &str = "Bearer";

const FILE_NAME: &str = "provider_credentials.json";

/// Set to `1`/`true` to skip the OS keyring and use the file store only
/// (tests, headless CI without a Secret Service). **Not** interactive recovery
/// advice for real secrets — login requires a working OS secret store.
pub const FORCE_FILE_ENV: &str = "GROK_CREDENTIALS_FORCE_FILE";

/// Wall-clock budget for a single OS keyring get/set/delete.
///
/// Secret Service / D-Bus can block indefinitely when the agent is locked or
/// stuck; interactive login must fail loudly instead of hanging after paste.
pub const KEYRING_OP_TIMEOUT: Duration = Duration::from_secs(3);

/// After a keyring timeout/error, skip further **resolve** keyring probes for
/// this long (fail-open file fallthrough). Does **not** apply to RMW
/// [`CredentialsStore::read_for_update`] or writes — those always probe.
const KEYRING_CIRCUIT_TTL: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum CredentialsStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Keyring backend error (not timeout). Message is safe to show (no secrets).
    #[error(
        "could not reach the OS secret store: {0}; unlock or fix Secret Service \
         (or your platform keyring), then retry login"
    )]
    Keyring(String),
    /// D-Bus / keyring op exceeded [`KEYRING_OP_TIMEOUT`].
    ///
    /// Note: an abandoned worker may still complete a slow `set_password` after
    /// this error is returned (see module docs). Login does not claim the secret
    /// was discarded from the OS store — only that the timed wait failed.
    #[error(
        "could not reach the OS secret store (timed out after {secs}s); unlock \
         or fix Secret Service (or your platform keyring), then retry login"
    )]
    KeyringTimeout { secs: u64 },
}

/// One stored credential: URL → (username, secret).
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StoredCredential {
    username: String,
    /// Secret as a UTF-8 string (API keys are always text).
    secret: String,
}

impl std::fmt::Debug for StoredCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredCredential")
            .field("username", &self.username)
            .field("secret", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FileStore {
    /// Map of API base URL → credential.
    #[serde(default)]
    credentials: HashMap<String, StoredCredential>,
}

/// URL-keyed provider credential store.
#[derive(Debug, Clone)]
pub struct CredentialsStore {
    file_path: PathBuf,
    force_file: bool,
}

impl CredentialsStore {
    /// Store under `$GROK_HOME/provider_credentials.json`.
    pub fn default_store() -> Self {
        Self::at_grok_home(&crate::util::grok_home::grok_home())
    }

    /// Store under `{grok_home}/provider_credentials.json`.
    pub fn at_grok_home(grok_home: &Path) -> Self {
        Self {
            file_path: grok_home.join(FILE_NAME),
            force_file: force_file_backend(),
        }
    }

    /// Explicit path, **file backend only** (tests).
    pub fn at_path(file_path: PathBuf) -> Self {
        Self {
            file_path,
            force_file: true,
        }
    }

    /// Explicit path but still prefer the OS keyring (tests for keyring policy).
    ///
    /// Unlike [`Self::at_path`], does not force the file backend. Use with test
    /// hooks that simulate a blocked keyring.
    #[cfg(test)]
    pub fn at_path_prefer_keyring(file_path: PathBuf) -> Self {
        Self {
            file_path,
            force_file: false,
        }
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Read `(username, secret)` for `url`, if present.
    ///
    /// Prefer OS keyring (time-boxed); on miss / keyring unavailable, fall back
    /// to the file mirror so dual-written secrets remain readable (agent
    /// **resolve**). Honors the resolve-only keyring circuit breaker (skips
    /// keyring while open). For multi-add RMW, use [`Self::read_for_update`].
    pub fn read(&self, url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
        if url.is_empty() {
            return Ok(None);
        }
        if !self.force_file {
            // Circuit applies to resolve only — silent skip to file (no fake
            // "timed out after 3s" without a real wait).
            if keyring_circuit_open() {
                tracing::debug!("keyring circuit open; resolve read using file store");
            } else {
                match keyring_read(url) {
                    Ok(Some(cred)) => return Ok(Some(cred)),
                    Ok(None) => {}
                    Err(e) => {
                        tracing::debug!(
                            error = %e,
                            "keyring read failed or timed out; trying file store"
                        );
                    }
                }
            }
        }
        self.read_file_only(url)
    }

    /// Read for read-modify-write (console multi-add).
    ///
    /// When not using the file-only backend, keyring errors and timeouts
    /// **propagate** — callers must not invent an empty key list from a
    /// missing/stale file mirror and then rewrite the keyring blob.
    ///
    /// Always probes the keyring (ignores the resolve-only circuit breaker) so
    /// interactive multi-add login can recover after a transient resolve
    /// timeout. On keyring miss (`Ok(None)`), falls through to the file mirror.
    pub fn read_for_update(
        &self,
        url: &str,
    ) -> Result<Option<(String, String)>, CredentialsStoreError> {
        if url.is_empty() {
            return Ok(None);
        }
        if self.force_file {
            return self.read_file_only(url);
        }
        match keyring_read(url)? {
            Some(cred) => Ok(Some(cred)),
            None => self.read_file_only(url),
        }
    }

    fn read_file_only(&self, url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
        let store = self.load_file()?;
        Ok(store
            .credentials
            .get(url)
            .map(|c| (c.username.clone(), c.secret.clone())))
    }

    /// Write a Bearer API key for `url`.
    pub fn write_bearer(&self, url: &str, secret: &str) -> Result<(), CredentialsStoreError> {
        self.write(url, BEARER_USERNAME, secret)
    }

    /// Write credentials for `url`.
    ///
    /// When not using the file-only backend, a **secure** keyring backend is
    /// required: primary platform store first, then (on Linux) automatic
    /// keyutils fallback if primary times out or errors. Only if **all** secure
    /// backends fail does this error — the secret is **not** written to the
    /// file store as recovery (avoids silent disk dump). After a successful
    /// secure write, the file is best-effort mirrored.
    ///
    /// On primary timeout the worker is abandoned; a slow Secret Service may
    /// still complete `set_password` later (module docs). The secret remains in
    /// the abandoned worker's memory until that call finishes or the process
    /// exits.
    pub fn write(
        &self,
        url: &str,
        username: &str,
        secret: &str,
    ) -> Result<(), CredentialsStoreError> {
        if url.is_empty() {
            return Err(CredentialsStoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "credential URL must not be empty",
            )));
        }
        if self.force_file {
            return self.write_file(url, username, secret);
        }
        // Keyring required for interactive / default store writes.
        keyring_write(url, username, secret)?;
        // Mirror only after keyring succeeded (logout/read if keyring later down).
        let _ = self.write_file(url, username, secret);
        Ok(())
    }

    /// Delete credentials for `url` from keyring and file.
    pub fn delete(&self, url: &str) -> Result<(), CredentialsStoreError> {
        if url.is_empty() {
            return Ok(());
        }
        if !self.force_file {
            let _ = keyring_delete(url);
        }
        let mut store = self.load_file()?;
        if store.credentials.remove(url).is_some() {
            self.save_file(&store)?;
        }
        Ok(())
    }

    fn load_file(&self) -> Result<FileStore, CredentialsStoreError> {
        if !self.file_path.exists() {
            return Ok(FileStore::default());
        }
        let mut file = File::open(&self.file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            return Ok(FileStore::default());
        }
        Ok(serde_json::from_str(trimmed)?)
    }

    fn write_file(
        &self,
        url: &str,
        username: &str,
        secret: &str,
    ) -> Result<(), CredentialsStoreError> {
        let mut store = self.load_file()?;
        store.credentials.insert(
            url.to_owned(),
            StoredCredential {
                username: username.to_owned(),
                secret: secret.to_owned(),
            },
        );
        self.save_file(&store)
    }

    fn save_file(&self, store: &FileStore) -> Result<(), CredentialsStoreError> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(store)?;
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&self.file_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.file_path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

fn force_file_backend() -> bool {
    std::env::var(FORCE_FILE_ENV)
        .map(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
}

fn keyring_op_timeout() -> Duration {
    #[cfg(test)]
    {
        if let Some(d) = test_hooks::timeout_override() {
            return d;
        }
    }
    KEYRING_OP_TIMEOUT
}

fn timeout_secs_for_display(timeout: Duration) -> u64 {
    timeout.as_secs().max(1)
}

/// Process-wide short circuit after keyring timeout/error so repeated **resolve**
/// reads do not each wait a full budget / spawn abandoned workers. RMW and
/// writes ignore this (see [`CredentialsStore::read`] vs [`CredentialsStore::read_for_update`]).
static KEYRING_SKIP_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

fn keyring_circuit_open() -> bool {
    KEYRING_SKIP_UNTIL
        .lock()
        .ok()
        .and_then(|g| *g)
        .is_some_and(|until| Instant::now() < until)
}

fn trip_keyring_circuit() {
    if let Ok(mut g) = KEYRING_SKIP_UNTIL.lock() {
        *g = Some(Instant::now() + KEYRING_CIRCUIT_TTL);
    }
}

fn clear_keyring_circuit() {
    if let Ok(mut g) = KEYRING_SKIP_UNTIL.lock() {
        *g = None;
    }
}

fn note_keyring_err(err: &CredentialsStoreError) {
    match err {
        CredentialsStoreError::KeyringTimeout { .. } | CredentialsStoreError::Keyring(_) => {
            trip_keyring_circuit();
        }
        _ => {}
    }
}

/// Which secure backend an op targets (primary = platform default; fallback =
/// Linux keyutils or test mock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyringBackend {
    Primary,
    Fallback,
}

/// Run a blocking keyring call on a helper thread with a wall-clock budget.
///
/// Does **not** update the resolve circuit breaker — callers note composite
/// multi-backend outcomes so a healthy fallback can clear the circuit even when
/// primary timed out.
///
/// On timeout the helper may still be blocked in D-Bus; we abandon waiting so
/// interactive login can fail loudly instead of hanging forever. The abandoned
/// worker may still complete a slow write later (module docs) and retains any
/// secret cloned into the op until then.
fn run_keyring_op<T, F>(backend: KeyringBackend, op: F) -> Result<T, CredentialsStoreError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CredentialsStoreError> + Send + 'static,
{
    #[cfg(test)]
    test_hooks::maybe_inject_before_op(backend)?;

    let timeout = keyring_op_timeout();

    // Test-only: simulate a blocked backend **without** calling the real
    // keyring. Spawns a worker that never completes (same abandon path as
    // production `recv_timeout`) so policy tests exercise spawn + timeout.
    #[cfg(test)]
    if test_hooks::hang_backend(backend) {
        let (tx, rx) = mpsc::channel::<Result<T, CredentialsStoreError>>();
        let handle = std::thread::Builder::new()
            .name("grok-keyring-op".into())
            .spawn(move || {
                // Hold sender open so parent sees Timeout (not Disconnected).
                // Never call real keyring; never send a result.
                let _tx = tx;
                loop {
                    std::thread::sleep(Duration::from_secs(3600));
                }
            })
            .map_err(|e| {
                CredentialsStoreError::Keyring(format!("failed to spawn keyring worker: {e}"))
            })?;
        // `op` is dropped without running — intentional for hang simulation.
        drop(op);
        return match rx.recv_timeout(timeout) {
            Ok(result) => {
                let _ = handle.join();
                result
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(CredentialsStoreError::KeyringTimeout {
                secs: timeout_secs_for_display(timeout),
            }),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = handle.join();
                Err(CredentialsStoreError::Keyring(
                    "keyring worker disconnected before completing".into(),
                ))
            }
        };
    }

    let (tx, rx) = mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("grok-keyring-op".into())
        .spawn(move || {
            let _ = tx.send(op());
        })
        .map_err(|e| {
            CredentialsStoreError::Keyring(format!("failed to spawn keyring worker: {e}"))
        })?;

    match rx.recv_timeout(timeout) {
        Ok(result) => {
            // Worker finished; join to avoid leaking threads on the happy path.
            let _ = handle.join();
            result
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Leave the worker running; joining would re-introduce the hang.
            // Secret may still land in keyring if the op was a slow write.
            Err(CredentialsStoreError::KeyringTimeout {
                secs: timeout_secs_for_display(timeout),
            })
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            Err(CredentialsStoreError::Keyring(
                "keyring worker disconnected before completing".into(),
            ))
        }
    }
}

fn primary_get(url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
    let entry = keyring::Entry::new(SERVICE_NAME, url)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(secret) if !secret.is_empty() => Ok(Some((BEARER_USERNAME.to_owned(), secret))),
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(CredentialsStoreError::Keyring(e.to_string())),
    }
}

fn primary_set(url: &str, secret: &str) -> Result<(), CredentialsStoreError> {
    let entry = keyring::Entry::new(SERVICE_NAME, url)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))?;
    entry
        .set_password(secret)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))
}

fn primary_delete(url: &str) -> Result<(), CredentialsStoreError> {
    let entry = keyring::Entry::new(SERVICE_NAME, url)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CredentialsStoreError::Keyring(e.to_string())),
    }
}

fn fallback_get(url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
    #[cfg(test)]
    if test_hooks::mock_fallback_active() {
        return test_hooks::mock_fallback_get(url);
    }
    fallback_get_os(url)
}

fn fallback_set(url: &str, secret: &str) -> Result<(), CredentialsStoreError> {
    #[cfg(test)]
    if test_hooks::mock_fallback_active() {
        return test_hooks::mock_fallback_set(url, secret);
    }
    fallback_set_os(url, secret)
}

fn fallback_delete(url: &str) -> Result<(), CredentialsStoreError> {
    #[cfg(test)]
    if test_hooks::mock_fallback_active() {
        return test_hooks::mock_fallback_delete(url);
    }
    fallback_delete_os(url)
}

#[cfg(target_os = "linux")]
fn keyutils_store()
-> Result<std::sync::Arc<linux_keyutils_keyring_store::Store>, CredentialsStoreError> {
    use std::sync::OnceLock;
    static STORE: OnceLock<Result<std::sync::Arc<linux_keyutils_keyring_store::Store>, String>> =
        OnceLock::new();
    match STORE
        .get_or_init(|| linux_keyutils_keyring_store::Store::new().map_err(|e| e.to_string()))
    {
        Ok(s) => Ok(std::sync::Arc::clone(s)),
        Err(msg) => Err(CredentialsStoreError::Keyring(format!(
            "linux keyutils store unavailable: {msg}"
        ))),
    }
}

#[cfg(target_os = "linux")]
fn keyutils_entry(url: &str) -> Result<keyring_core::Entry, CredentialsStoreError> {
    use keyring_core::api::CredentialStoreApi;
    let store = keyutils_store()?;
    store
        .build(SERVICE_NAME, url, None)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))
}

#[cfg(target_os = "linux")]
fn fallback_get_os(url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
    let entry = keyutils_entry(url)?;
    match entry.get_password() {
        Ok(secret) if !secret.is_empty() => Ok(Some((BEARER_USERNAME.to_owned(), secret))),
        Ok(_) => Ok(None),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(CredentialsStoreError::Keyring(e.to_string())),
    }
}

#[cfg(target_os = "linux")]
fn fallback_set_os(url: &str, secret: &str) -> Result<(), CredentialsStoreError> {
    let entry = keyutils_entry(url)?;
    entry
        .set_password(secret)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))
}

#[cfg(target_os = "linux")]
fn fallback_delete_os(url: &str) -> Result<(), CredentialsStoreError> {
    let entry = keyutils_entry(url)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(e) => Err(CredentialsStoreError::Keyring(e.to_string())),
    }
}

#[cfg(not(target_os = "linux"))]
fn fallback_get_os(_url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
    Err(CredentialsStoreError::Keyring(
        "no secure keyring fallback on this platform".into(),
    ))
}

#[cfg(not(target_os = "linux"))]
fn fallback_set_os(_url: &str, _secret: &str) -> Result<(), CredentialsStoreError> {
    Err(CredentialsStoreError::Keyring(
        "no secure keyring fallback on this platform".into(),
    ))
}

#[cfg(not(target_os = "linux"))]
fn fallback_delete_os(_url: &str) -> Result<(), CredentialsStoreError> {
    Err(CredentialsStoreError::Keyring(
        "no secure keyring fallback on this platform".into(),
    ))
}

/// Always probes secure backends (no resolve circuit). Circuit skipping lives
/// only in [`CredentialsStore::read`].
///
/// Order: primary → secure fallback. Composite circuit: clear on any success;
/// trip only when every secure backend fails with a keyring error/timeout.
fn keyring_read(url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
    let url_owned = url.to_owned();
    let primary = run_keyring_op(KeyringBackend::Primary, {
        let url = url_owned.clone();
        move || primary_get(&url)
    });
    let primary_miss: Result<Option<(String, String)>, CredentialsStoreError> = match primary {
        Ok(Some(cred)) => {
            clear_keyring_circuit();
            return Ok(Some(cred));
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    };

    let fallback = run_keyring_op(KeyringBackend::Fallback, {
        let url = url_owned;
        move || fallback_get(&url)
    });
    match (primary_miss, fallback) {
        (_, Ok(Some(cred))) => {
            clear_keyring_circuit();
            Ok(Some(cred))
        }
        (Ok(None), Ok(None)) => {
            clear_keyring_circuit();
            Ok(None)
        }
        (Ok(None), Err(_)) => {
            // Primary healthy and empty; ignore broken fallback.
            clear_keyring_circuit();
            Ok(None)
        }
        (Err(primary_err), Ok(None)) => {
            // Primary unavailable, nothing in fallback — fail closed for RMW.
            note_keyring_err(&primary_err);
            Err(primary_err)
        }
        (Err(primary_err), Err(_)) => {
            note_keyring_err(&primary_err);
            Err(primary_err)
        }
        (Ok(Some(_)), _) => unreachable!("Ok(Some) returned above"),
    }
}

fn keyring_write(url: &str, _username: &str, secret: &str) -> Result<(), CredentialsStoreError> {
    // Writes always attempt secure backends (ignore resolve circuit).
    let url_owned = url.to_owned();
    let secret_owned = secret.to_owned();
    let primary = run_keyring_op(KeyringBackend::Primary, {
        let url = url_owned.clone();
        let secret = secret_owned.clone();
        move || primary_set(&url, &secret)
    });
    if primary.is_ok() {
        clear_keyring_circuit();
        return Ok(());
    }
    let primary_err = primary.expect_err("checked is_err");

    let fallback = run_keyring_op(KeyringBackend::Fallback, {
        let url = url_owned;
        let secret = secret_owned;
        move || fallback_set(&url, &secret)
    });
    match fallback {
        Ok(()) => {
            tracing::info!("credential write used secure keyring fallback after primary failure");
            clear_keyring_circuit();
            Ok(())
        }
        Err(_) => {
            note_keyring_err(&primary_err);
            Err(primary_err)
        }
    }
}

fn keyring_delete(url: &str) -> Result<(), CredentialsStoreError> {
    // Deletes always attempt both backends (ignore resolve circuit); best-effort.
    let url_owned = url.to_owned();
    let primary = run_keyring_op(KeyringBackend::Primary, {
        let url = url_owned.clone();
        move || primary_delete(&url)
    });
    let fallback = run_keyring_op(KeyringBackend::Fallback, {
        let url = url_owned;
        move || fallback_delete(&url)
    });
    // Prefer reporting primary error if both failed; success if either worked.
    match (primary, fallback) {
        (Ok(()), _) | (_, Ok(())) => {
            clear_keyring_circuit();
            Ok(())
        }
        (Err(e), Err(_)) => {
            note_keyring_err(&e);
            Err(e)
        }
    }
}

/// Test-only hooks to simulate a blocked or failing Secret Service without a
/// real D-Bus hang and **without** calling OS keyring APIs.
#[cfg(test)]
pub mod test_hooks {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    #[derive(Default)]
    struct HookState {
        /// Hang primary backend ops without invoking the real keyring.
        hang_primary: bool,
        /// Hang fallback backend ops without invoking mock/OS fallback.
        hang_fallback: bool,
        /// Override wall-clock budget (keep tests fast).
        timeout: Option<Duration>,
        /// Fail primary before work (hard keyring error, no OS call).
        force_err_primary: Option<String>,
        /// Fail fallback before work.
        force_err_fallback: Option<String>,
        /// Use in-memory mock as the secure fallback (hermetic dual-backend tests).
        mock_fallback: bool,
        /// Mock fallback secrets: credential URL → secret.
        mock_secrets: HashMap<String, String>,
    }

    static HOOKS: LazyLock<Mutex<HookState>> = LazyLock::new(|| Mutex::new(HookState::default()));

    /// RAII clear of all keyring test hooks (+ process circuit breaker).
    pub struct HookGuard;

    impl Drop for HookGuard {
        fn drop(&mut self) {
            clear();
        }
    }

    pub fn clear() {
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        *g = HookState::default();
        clear_keyring_circuit();
    }

    /// Simulate **all** secure backends blocked: ops return
    /// [`CredentialsStoreError::KeyringTimeout`] after `timeout` without calling
    /// real OS keyring or mock fallback (no pollution).
    pub fn simulate_blocked_keyring(timeout: Duration) -> HookGuard {
        clear();
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.timeout = Some(timeout);
        g.hang_primary = true;
        g.hang_fallback = true;
        HookGuard
    }

    /// Primary times out; secure **mock** fallback accepts writes/reads.
    ///
    /// Named contract for automatic secure fallback when Secret Service hangs.
    pub fn simulate_blocked_primary_with_mock_fallback(timeout: Duration) -> HookGuard {
        clear();
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.timeout = Some(timeout);
        g.hang_primary = true;
        g.hang_fallback = false;
        g.mock_fallback = true;
        HookGuard
    }

    /// Force **all** secure backend ops to fail immediately with `msg` (no OS call).
    pub fn simulate_keyring_error(msg: impl Into<String>) -> HookGuard {
        clear();
        let msg = msg.into();
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.force_err_primary = Some(msg.clone());
        g.force_err_fallback = Some(msg);
        HookGuard
    }

    /// Primary hard-errors; secure **mock** fallback accepts writes/reads.
    pub fn simulate_primary_error_with_mock_fallback(msg: impl Into<String>) -> HookGuard {
        clear();
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.force_err_primary = Some(msg.into());
        g.mock_fallback = true;
        HookGuard
    }

    pub(super) fn timeout_override() -> Option<Duration> {
        HOOKS.lock().expect("keyring test hooks lock").timeout
    }

    pub(super) fn hang_backend(backend: KeyringBackend) -> bool {
        let g = HOOKS.lock().expect("keyring test hooks lock");
        match backend {
            KeyringBackend::Primary => g.hang_primary,
            KeyringBackend::Fallback => g.hang_fallback,
        }
    }

    pub(super) fn mock_fallback_active() -> bool {
        HOOKS.lock().expect("keyring test hooks lock").mock_fallback
    }

    pub(super) fn mock_fallback_get(
        url: &str,
    ) -> Result<Option<(String, String)>, CredentialsStoreError> {
        let g = HOOKS.lock().expect("keyring test hooks lock");
        Ok(g.mock_secrets
            .get(url)
            .map(|s| (BEARER_USERNAME.to_owned(), s.clone())))
    }

    pub(super) fn mock_fallback_set(url: &str, secret: &str) -> Result<(), CredentialsStoreError> {
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.mock_secrets.insert(url.to_owned(), secret.to_owned());
        Ok(())
    }

    pub(super) fn mock_fallback_delete(url: &str) -> Result<(), CredentialsStoreError> {
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.mock_secrets.remove(url);
        Ok(())
    }

    /// Whether the mock fallback holds `secret` for `url` (test assertions).
    pub fn mock_fallback_has(url: &str, secret: &str) -> bool {
        let g = HOOKS.lock().expect("keyring test hooks lock");
        g.mock_secrets.get(url).is_some_and(|s| s == secret)
    }

    pub(super) fn maybe_inject_before_op(
        backend: KeyringBackend,
    ) -> Result<(), CredentialsStoreError> {
        let msg = {
            let g = HOOKS.lock().expect("keyring test hooks lock");
            match backend {
                KeyringBackend::Primary => g.force_err_primary.clone(),
                KeyringBackend::Fallback => g.force_err_fallback.clone(),
            }
        };
        if let Some(msg) = msg {
            // Do not trip circuit here — composite keyring_read/write decide.
            return Err(CredentialsStoreError::Keyring(msg));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, CredentialsStore) {
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path(dir.path().join(FILE_NAME));
        (dir, store)
    }

    #[test]
    fn roundtrip_bearer_key() {
        let (_dir, store) = temp_store();
        let url = "https://openrouter.ai/api/v1";
        store.write_bearer(url, "sk-or-test").unwrap();
        let got = store.read(url).unwrap().expect("key present");
        assert_eq!(got.0, BEARER_USERNAME);
        assert_eq!(got.1, "sk-or-test");
    }

    #[test]
    fn stored_credential_debug_redacts_secret() {
        let cred = StoredCredential {
            username: BEARER_USERNAME.to_owned(),
            secret: "super-secret-key-value".to_owned(),
        };
        let dbg = format!("{cred:?}");
        assert!(!dbg.contains("super-secret-key-value"), "{dbg}");
        assert!(dbg.contains("<redacted>"), "{dbg}");
    }

    #[test]
    fn delete_removes_key() {
        let (_dir, store) = temp_store();
        let url = "https://openrouter.ai/api/v1";
        store.write_bearer(url, "sk-or-test").unwrap();
        store.delete(url).unwrap();
        assert!(store.read(url).unwrap().is_none());
    }

    #[test]
    fn missing_url_returns_none() {
        let (_dir, store) = temp_store();
        assert!(store.read("https://example.com/v1").unwrap().is_none());
    }

    #[test]
    fn empty_url_read_is_none() {
        let (_dir, store) = temp_store();
        assert!(store.read("").unwrap().is_none());
    }

    #[test]
    fn overwrite_replaces_secret() {
        let (_dir, store) = temp_store();
        let url = "https://openrouter.ai/api/v1";
        store.write_bearer(url, "first").unwrap();
        store.write_bearer(url, "second").unwrap();
        assert_eq!(store.read(url).unwrap().unwrap().1, "second");
    }

    /// Named contract: when **all** secure backends exceed the time budget,
    /// interactive (non-force-file) store write fails with a clear timeout error
    /// and does **not** write the secret to the file store path for that call.
    /// Hook never invokes real keyring (no OS pollution).
    #[test]
    #[serial]
    fn interactive_write_all_backends_timeout_fails_without_file_secret() {
        let _hooks = test_hooks::simulate_blocked_keyring(Duration::from_millis(50));
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(FILE_NAME);
        let store = CredentialsStore::at_path_prefer_keyring(path.clone());
        let secret = "super-secret-must-not-land-on-disk-timeout";
        let url = "https://api.x.ai/v1";

        let err = store
            .write_bearer(url, secret)
            .expect_err("all backends blocked must fail write (no silent file fallback)");

        match &err {
            CredentialsStoreError::KeyringTimeout { secs } => {
                assert!(
                    *secs >= 1,
                    "timeout message should report at least 1s: {secs}"
                );
            }
            other => panic!("expected KeyringTimeout, got {other:?}"),
        }
        let msg = err.to_string();
        assert!(
            msg.contains("OS secret store") || msg.contains("Secret Service"),
            "user-facing timeout must mention secret store: {msg}"
        );
        assert!(
            !path.exists()
                || !std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains(secret),
            "secret must not be written to file store when all secure backends fail"
        );
    }

    /// Named contract: primary Secret Service times out → automatic **secure**
    /// fallback succeeds; secret is readable via secure path; file is only a
    /// post-success mirror (not the recovery path). FORCE_FILE is not used.
    #[test]
    #[serial]
    fn interactive_write_falls_back_when_primary_times_out() {
        let _hooks =
            test_hooks::simulate_blocked_primary_with_mock_fallback(Duration::from_millis(50));
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(FILE_NAME);
        let store = CredentialsStore::at_path_prefer_keyring(path.clone());
        let secret = "secret-via-secure-fallback-timeout";
        let url = "https://api.x.ai/v1/fallback-timeout";

        store
            .write_bearer(url, secret)
            .expect("primary timeout must auto-fallback to secure backend");

        assert!(
            test_hooks::mock_fallback_has(url, secret),
            "secret must live in the secure fallback store, not only as a file dump"
        );

        // RMW read probes secure backends (primary hangs, fallback hits).
        let got = store
            .read_for_update(url)
            .expect("RMW read must succeed via fallback")
            .expect("credential present after fallback write");
        assert_eq!(got.0, BEARER_USERNAME);
        assert_eq!(got.1, secret);

        // File mirror after successful secure write is allowed; must not be the
        // only place the secret exists (asserted via mock_fallback_has above).
        if path.exists() {
            let body = std::fs::read_to_string(&path).unwrap_or_default();
            // Mirror may contain secret — that is dual-write after success.
            let _ = body;
        }
    }

    /// Named contract: all secure backends hard-error → fail loud, no file dump.
    #[test]
    #[serial]
    fn interactive_write_all_backends_error_fails_without_file_secret() {
        let _hooks = test_hooks::simulate_keyring_error("simulated secret service down");
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(FILE_NAME);
        let store = CredentialsStore::at_path_prefer_keyring(path.clone());
        let secret = "super-secret-must-not-land-on-disk-error";
        let url = "https://api.x.ai/v1";

        let err = store
            .write_bearer(url, secret)
            .expect_err("all backends error must fail write (no silent file fallback)");

        match &err {
            CredentialsStoreError::Keyring(m) => {
                assert!(m.contains("simulated secret service down"), "{m}");
            }
            other => panic!("expected Keyring error, got {other:?}"),
        }
        let msg = err.to_string();
        assert!(
            msg.contains("OS secret store") || msg.contains("Secret Service"),
            "user-facing error must mention secret store: {msg}"
        );
        assert!(
            !path.exists()
                || !std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains(secret),
            "secret must not be written to file store when all secure backends fail"
        );
    }

    /// Named contract: primary hard-error → secure mock fallback still writes.
    #[test]
    #[serial]
    fn interactive_write_falls_back_when_primary_errors() {
        let _hooks =
            test_hooks::simulate_primary_error_with_mock_fallback("simulated primary down");
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(FILE_NAME);
        let store = CredentialsStore::at_path_prefer_keyring(path);
        let secret = "secret-via-secure-fallback-error";
        let url = "https://api.x.ai/v1/fallback-error";

        store
            .write_bearer(url, secret)
            .expect("primary error must auto-fallback to secure backend");
        assert!(test_hooks::mock_fallback_has(url, secret));
        let got = store
            .read_for_update(url)
            .expect("read via fallback")
            .expect("present");
        assert_eq!(got.1, secret);
    }

    /// File-only backend still writes (FORCE_FILE / at_path / tests).
    #[test]
    #[serial]
    fn force_file_write_ignores_keyring_hooks() {
        let _hooks = test_hooks::simulate_keyring_error("should not be consulted");
        let (_dir, store) = temp_store();
        store
            .write_bearer("https://api.x.ai/v1", "file-only-secret")
            .expect("force_file path must not call keyring");
        assert_eq!(
            store.read("https://api.x.ai/v1").unwrap().unwrap().1,
            "file-only-secret"
        );
    }

    /// Named contract: console multi-add must not wipe keyring keys when keyring
    /// read fails open to empty file — RMW load fail-closes instead.
    #[test]
    #[serial]
    fn multi_add_fails_closed_when_keyring_read_errors() {
        use crate::auth::xai_console::{XaiConsoleAuthError, add_console_api_key};
        use xai_grok_test_support::EnvGuard;

        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let _hooks = test_hooks::simulate_keyring_error("simulated keyring read failure");

        let dir = TempDir::new().unwrap();
        let path = dir.path().join(FILE_NAME);
        let store = CredentialsStore::at_path_prefer_keyring(path.clone());
        let new_key = "new-key-must-not-replace-existing-blob";

        let err = add_console_api_key(&store, new_key)
            .expect_err("multi-add must fail closed when keyring read errors");

        match &err {
            XaiConsoleAuthError::Store(CredentialsStoreError::Keyring(m)) => {
                assert!(m.contains("simulated keyring read failure"), "{m}");
            }
            other => panic!("expected Store(Keyring), got {other:?}"),
        }
        assert!(
            !path.exists()
                || !std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains(new_key),
            "failed multi-add must not write the new key to the file store"
        );
    }

    /// Named contract: `read_for_update` propagates keyring errors; plain `read`
    /// may still fall through to an empty file (resolve fail-open).
    #[test]
    #[serial]
    fn read_for_update_fail_closed_while_read_fail_opens() {
        let _hooks = test_hooks::simulate_keyring_error("unavailable");
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(FILE_NAME);
        let store = CredentialsStore::at_path_prefer_keyring(path);
        let url = "https://api.x.ai/v1";

        assert!(
            store.read(url).unwrap().is_none(),
            "resolve read fail-opens to empty file"
        );
        // Hook still forces keyring error; RMW re-probes and fail-closes.
        let err = store
            .read_for_update(url)
            .expect_err("RMW read must not invent empty from keyring error");
        assert!(
            matches!(
                err,
                CredentialsStoreError::Keyring(_) | CredentialsStoreError::KeyringTimeout { .. }
            ),
            "got {err:?}"
        );
    }

    /// Named contract: after a keyring timeout, a subsequent resolve read skips
    /// waiting another full keyring budget (circuit breaker).
    #[test]
    #[serial]
    fn read_circuit_breaker_skips_second_keyring_wait() {
        let budget = Duration::from_millis(80);
        let _hooks = test_hooks::simulate_blocked_keyring(budget);
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path_prefer_keyring(dir.path().join(FILE_NAME));
        let url = "https://api.x.ai/v1";

        let t0 = Instant::now();
        let _ = store.read(url); // fail-open after ~budget
        let first = t0.elapsed();

        let t1 = Instant::now();
        let _ = store.read(url);
        let second = t1.elapsed();

        assert!(
            first >= budget / 2,
            "first read should wait near the keyring budget, got {first:?}"
        );
        assert!(
            second < budget / 2,
            "second read must use circuit breaker (no full keyring wait), first={first:?} second={second:?}"
        );
    }

    /// Named contract: circuit breaker skips resolve reads only; multi-add /
    /// login RMW still probes keyring (does not short-circuit on open circuit).
    #[test]
    #[serial]
    fn rmw_probes_keyring_even_when_resolve_circuit_open() {
        let budget = Duration::from_millis(80);
        let _hooks = test_hooks::simulate_blocked_keyring(budget);
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path_prefer_keyring(dir.path().join(FILE_NAME));
        let url = "https://api.x.ai/v1";

        // Trip resolve circuit.
        let _ = store.read(url);
        assert!(
            keyring_circuit_open(),
            "resolve timeout must open the keyring circuit"
        );

        // Second resolve read must be fast (circuit).
        let t_resolve = Instant::now();
        let _ = store.read(url);
        let resolve_elapsed = t_resolve.elapsed();
        assert!(
            resolve_elapsed < budget / 2,
            "resolve must honor circuit, got {resolve_elapsed:?}"
        );

        // RMW must re-probe (wait near budget), not instant circuit skip.
        let t_rmw = Instant::now();
        let err = store
            .read_for_update(url)
            .expect_err("blocked keyring RMW must fail closed after real probe");
        let rmw_elapsed = t_rmw.elapsed();
        assert!(
            rmw_elapsed >= budget / 2,
            "RMW must probe keyring despite open circuit (wait near budget), got {rmw_elapsed:?}"
        );
        assert!(
            matches!(err, CredentialsStoreError::KeyringTimeout { .. }),
            "expected real KeyringTimeout from probe, got {err:?}"
        );
    }
}
