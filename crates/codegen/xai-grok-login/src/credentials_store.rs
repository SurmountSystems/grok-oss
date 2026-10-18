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
//! **Success cache:** a completed keyring probe (present or confirmed miss) is
//! kept in process memory, keyed by credential URL. Later
//! [`CredentialsStore::read`] calls for that URL do not SearchItems / GetSecret
//! again. SuperGrok session secrets are omitted from `auth.json`, so restore
//! may call `read` on every load; the cache is what stops idle Secret Service
//! hammering. Writes are write-through. Deletes invalidate. A token refresh
//! re-reads the keyring once (bypass cache) before SetSecret so another
//! process that already refreshed is not overwritten. Confirmed misses still
//! consult the file mirror so a sibling process's post-write mirror is visible
//! without D-Bus.
//!
//! **Circuit breaker (resolve only):** after a keyring timeout/error, resolve
//! [`CredentialsStore::read`] skips further keyring probes for a short TTL.
//! RMW [`CredentialsStore::read_for_update`] and writes always probe so
//! interactive multi-add login can recover when Secret Service is healthy again.
//!
//! **Write-after-timeout race:** on timeout the parent returns
//! [`CredentialsStoreError::KeyringTimeout`] and stops waiting. The same
//! helper thread stays in flight (one per backend). If Secret Service was only
//! slow, that helper may still `set_password` later — login reported failure
//! and skipped the file mirror, but the secret can land in keyring
//! asynchronously (“failed but stored”). Inherent to abandon-the-wait without
//! D-Bus cancellation. The in-flight helper retains the secret in thread
//! memory until the blocking call finishes. A later op does **not** spawn
//! another sleeper while that helper is still blocked.
//!
//! Environment variables for specific providers (e.g. `OPENROUTER_API_KEY`) are
//! checked by the provider helpers, not this store. When an env key is set,
//! callers should refuse to write the store (Zed parity).

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{LazyLock, Mutex};
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

/// Hard cap on live `grok-keyring-op` helper threads: one for the platform
/// Secret Service (primary) and one for the secure fallback (Linux keyutils).
/// A timeout must not spawn another sleeper.
const MAX_KEYRING_HELPERS: usize = 2;

/// After a keyring timeout/error, skip further **resolve** keyring probes for
/// this long (fail-open file fallthrough). Does **not** apply to RMW
/// [`CredentialsStore::read_for_update`] or writes — those always probe.
const KEYRING_CIRCUIT_TTL: Duration = Duration::from_secs(15);

/// One completed keyring probe, cached so idle restore / render ticks do not
/// SearchItems / GetSecret again.
#[derive(Clone, Debug)]
enum CachedCredential {
    Present { username: String, secret: String },
    Absent,
}

/// Process-wide success cache, keyed by credential URL.
/// `HashMap::new` is not const, so this is a `LazyLock` around the mutex.
static KEYRING_SUCCESS_CACHE: LazyLock<Mutex<HashMap<String, CachedCredential>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static KEYRING_SEARCH_ITEMS: AtomicU64 = AtomicU64::new(0);
static KEYRING_GET_SECRET: AtomicU64 = AtomicU64::new(0);
static KEYRING_SET_SECRET: AtomicU64 = AtomicU64::new(0);
static KEYRING_DELETE: AtomicU64 = AtomicU64::new(0);

/// Counts of real keyring backend ops (Secret Service SearchItems / GetSecret /
/// SetSecret / Delete, or the in-process mock of those ops).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyringBackendOpCounts {
    pub search_items: u64,
    pub get_secret: u64,
    pub set_secret: u64,
    pub delete: u64,
}

/// What a refresh-time keyring re-read found. The variants carry no secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringRefreshLook {
    /// Cache already matched the keyring, or this store does not use the keyring.
    Unchanged,
    /// Keyring (or the file mirror after a keyring miss) held a different secret.
    /// The process cache now has that secret.
    Adopted,
    /// Keyring has no secret for this url, and the file mirror does not either.
    Missing,
}

/// Snapshot of backend op counters. Tests assert idle re-reads do not grow these.
pub fn keyring_backend_op_counts() -> KeyringBackendOpCounts {
    KeyringBackendOpCounts {
        search_items: KEYRING_SEARCH_ITEMS.load(Ordering::Relaxed),
        get_secret: KEYRING_GET_SECRET.load(Ordering::Relaxed),
        set_secret: KEYRING_SET_SECRET.load(Ordering::Relaxed),
        delete: KEYRING_DELETE.load(Ordering::Relaxed),
    }
}

/// Count one Secret Service SearchItems (or the in-process mock of it).
/// `url` is the keyring account, never the secret.
pub(crate) fn note_keyring_search_items(service: &str, url: &str) {
    KEYRING_SEARCH_ITEMS.fetch_add(1, Ordering::Relaxed);
    tracing::debug!(service, url, op = "SearchItems", "keyring backend");
}

/// Count one Secret Service GetSecret (or the in-process mock of it).
pub(crate) fn note_keyring_get_secret(service: &str, url: &str) {
    KEYRING_GET_SECRET.fetch_add(1, Ordering::Relaxed);
    tracing::debug!(service, url, op = "GetSecret", "keyring backend");
}

fn note_keyring_set_secret(service: &str, url: &str) {
    KEYRING_SET_SECRET.fetch_add(1, Ordering::Relaxed);
    tracing::debug!(service, url, op = "SetSecret", "keyring backend");
}

fn note_keyring_delete(service: &str, url: &str) {
    KEYRING_DELETE.fetch_add(1, Ordering::Relaxed);
    tracing::debug!(service, url, op = "Delete", "keyring backend");
}

fn cache_get(url: &str) -> Option<CachedCredential> {
    KEYRING_SUCCESS_CACHE
        .lock()
        .ok()
        .and_then(|g| g.get(url).cloned())
}

fn cache_put(url: &str, entry: CachedCredential) {
    if let Ok(mut g) = KEYRING_SUCCESS_CACHE.lock() {
        g.insert(url.to_owned(), entry);
    }
}

fn cache_put_present(url: &str, username: String, secret: String) {
    cache_put(url, CachedCredential::Present { username, secret });
}

fn cache_invalidate(url: &str) {
    if let Ok(mut g) = KEYRING_SUCCESS_CACHE.lock() {
        g.remove(url);
    }
}

fn clear_keyring_success_cache() {
    if let Ok(mut g) = KEYRING_SUCCESS_CACHE.lock() {
        g.clear();
    }
}

fn reset_keyring_op_counts() {
    KEYRING_SEARCH_ITEMS.store(0, Ordering::Relaxed);
    KEYRING_GET_SECRET.store(0, Ordering::Relaxed);
    KEYRING_SET_SECRET.store(0, Ordering::Relaxed);
    KEYRING_DELETE.store(0, Ordering::Relaxed);
}

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
    /// Note: the in-flight helper may still complete a slow `set_password` after
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
        Self::at_grok_home(&xai_grok_config::grok_home())
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
    /// keyring while open). A successful probe is cached in process memory so
    /// later resolve reads (idle restore, render ticks) do not hit Secret
    /// Service again. For multi-add RMW, use [`Self::read_for_update`].
    pub fn read(&self, url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
        if url.is_empty() {
            return Ok(None);
        }
        if self.force_file {
            return self.read_file_only(url);
        }
        match cache_get(url) {
            Some(CachedCredential::Present { username, secret }) => {
                return Ok(Some((username, secret)));
            }
            Some(CachedCredential::Absent) => {
                // Confirmed keyring miss: skip D-Bus. Still see a sibling
                // process's file mirror without SearchItems.
                if let Ok(Some(file_cred)) = self.read_file_only(url) {
                    cache_put_present(url, file_cred.0.clone(), file_cred.1.clone());
                    return Ok(Some(file_cred));
                }
                return Ok(None);
            }
            None => {}
        }
        // Circuit applies to resolve only — silent skip to file (no fake
        // "timed out after 3s" without a real wait). Do not cache a miss
        // observed while the circuit is open: that is not a confirmed probe.
        if keyring_circuit_open() {
            tracing::debug!("keyring circuit open; resolve read using file store");
            return self.read_file_only(url);
        }
        match keyring_read(url) {
            Ok(Some(cred)) => {
                let cred = self.overlay_username_from_file(url, cred);
                cache_put_present(url, cred.0.clone(), cred.1.clone());
                return Ok(Some(cred));
            }
            Ok(None) => {}
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    "keyring read failed or timed out; trying file store"
                );
                return self.read_file_only(url);
            }
        }
        let file = self.read_file_only(url)?;
        cache_put(
            url,
            match &file {
                Some((username, secret)) => CachedCredential::Present {
                    username: username.clone(),
                    secret: secret.clone(),
                },
                None => CachedCredential::Absent,
            },
        );
        Ok(file)
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
            Some(cred) => {
                let cred = self.overlay_username_from_file(url, cred);
                cache_put_present(url, cred.0.clone(), cred.1.clone());
                Ok(Some(cred))
            }
            None => {
                let file = self.read_file_only(url)?;
                cache_put(
                    url,
                    match &file {
                        Some((username, secret)) => CachedCredential::Present {
                            username: username.clone(),
                            secret: secret.clone(),
                        },
                        None => CachedCredential::Absent,
                    },
                );
                Ok(file)
            }
        }
    }

    /// Keyring stores only the secret; the file mirror keeps the caller
    /// username (`key` / `refresh_token` for SuperGrok sessions). Overlay so
    /// restore can match those usernames after a Secret Service read.
    fn overlay_username_from_file(&self, url: &str, cred: (String, String)) -> (String, String) {
        match self.read_file_only(url) {
            Ok(Some((file_user, _))) if !file_user.is_empty() => (file_user, cred.1),
            _ => cred,
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
    /// On primary timeout the wait is abandoned; a slow Secret Service may
    /// still complete `set_password` later on the same helper (module docs).
    /// The secret remains in that helper's memory until the call finishes.
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
        // Anti-stampede: re-read the keyring (bypass cache) in case another
        // process already refreshed this URL.
        match keyring_read(url) {
            Ok(Some((_existing_user, existing_secret))) if existing_secret == secret => {
                cache_put_present(url, username.to_owned(), secret.to_owned());
                let _ = self.write_file(url, username, secret);
                return Ok(());
            }
            Ok(Some((existing_user, existing_secret))) => {
                if let Some(CachedCredential::Present {
                    secret: cached_secret,
                    ..
                }) = cache_get(url)
                {
                    if cached_secret != existing_secret {
                        tracing::debug!(
                            url,
                            "keyring already holds a different secret; adopting instead of overwrite"
                        );
                        let user = if existing_user.is_empty() {
                            username.to_owned()
                        } else {
                            existing_user
                        };
                        cache_put_present(url, user.clone(), existing_secret.clone());
                        let _ = self.write_file(url, &user, &existing_secret);
                        return Ok(());
                    }
                }
                let _ = existing_user;
            }
            _ => {}
        }
        // Keyring required for interactive / default store writes.
        keyring_write(url, username, secret)?;
        cache_put_present(url, username.to_owned(), secret.to_owned());
        // Mirror only after keyring succeeded (logout/read if keyring later down).
        let _ = self.write_file(url, username, secret);
        Ok(())
    }

    /// Delete credentials for `url` from keyring and file.
    pub fn delete(&self, url: &str) -> Result<(), CredentialsStoreError> {
        if url.is_empty() {
            return Ok(());
        }
        cache_invalidate(url);
        if !self.force_file {
            let _ = keyring_delete(url);
        }
        let mut store = self.load_file()?;
        if store.credentials.remove(url).is_some() {
            self.save_file(&store)?;
        }
        Ok(())
    }

    /// Drop one cached probe. The next [`Self::read`] hits the backend again
    /// unless a later write or re-read fills the cache.
    pub fn invalidate_cached_secret(&self, url: &str) {
        if !url.is_empty() {
            cache_invalidate(url);
        }
    }

    /// One real keyring read, ignoring the process cache.
    ///
    /// Refresh calls this before deciding to mint a new token. If the keyring
    /// already holds a different secret, that secret replaces the cache and
    /// the caller must not SetSecret over it. File-only stores return
    /// [`KeyringRefreshLook::Unchanged`] and do not touch the backend. While
    /// the resolve circuit is open this returns an error and does not probe,
    /// so a failed refresh does not retry Secret Service in a tight loop.
    pub fn reread_keyring_before_refresh(
        &self,
        url: &str,
    ) -> Result<KeyringRefreshLook, CredentialsStoreError> {
        if url.is_empty() || self.force_file {
            return Ok(KeyringRefreshLook::Unchanged);
        }
        if keyring_circuit_open() {
            tracing::debug!(url, "keyring circuit open; refresh re-read backed off");
            return Err(CredentialsStoreError::Keyring(
                "keyring circuit open; refresh re-read backed off".into(),
            ));
        }
        let previous = cache_get(url);
        match keyring_read(url) {
            Ok(Some(cred)) => {
                let cred = self.overlay_username_from_file(url, cred);
                let changed = match &previous {
                    Some(CachedCredential::Present { secret, .. }) => secret != &cred.1,
                    _ => true,
                };
                cache_put_present(url, cred.0, cred.1);
                Ok(if changed {
                    KeyringRefreshLook::Adopted
                } else {
                    KeyringRefreshLook::Unchanged
                })
            }
            Ok(None) => {
                let file = self.read_file_only(url)?;
                let changed = match (&previous, &file) {
                    (Some(CachedCredential::Present { secret, .. }), Some((_, file_secret))) => {
                        secret != file_secret
                    }
                    (Some(CachedCredential::Absent) | None, None) => false,
                    _ => true,
                };
                cache_put(
                    url,
                    match &file {
                        Some((username, secret)) => CachedCredential::Present {
                            username: username.clone(),
                            secret: secret.clone(),
                        },
                        None => CachedCredential::Absent,
                    },
                );
                if file.is_none() {
                    Ok(KeyringRefreshLook::Missing)
                } else if changed {
                    Ok(KeyringRefreshLook::Adopted)
                } else {
                    Ok(KeyringRefreshLook::Unchanged)
                }
            }
            Err(err) => Err(err),
        }
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

/// Which Secret Service method a primary attempt stands for.
/// Used when a test hook refuses the op before `primary_get` runs, so the
/// circuit test still sees one counted probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyringOpKind {
    Read,
    Write,
    Delete,
}

fn note_primary_attempt(kind: KeyringOpKind, url: &str) {
    match kind {
        KeyringOpKind::Read => note_keyring_search_items(SERVICE_NAME, url),
        KeyringOpKind::Write => note_keyring_set_secret(SERVICE_NAME, url),
        KeyringOpKind::Delete => note_keyring_delete(SERVICE_NAME, url),
    }
}

/// One queued closure for a backend helper. Capacity on the helper channel is
/// 1 so a blocked Secret Service cannot pile up secrets in RAM.
struct KeyringJob {
    run: Box<dyn FnOnce() + Send>,
}

/// Long-lived `grok-keyring-op` helper for one backend.
struct KeyringHelper {
    tx: mpsc::SyncSender<KeyringJob>,
}

static PRIMARY_HELPER: Mutex<Option<KeyringHelper>> = Mutex::new(None);
static FALLBACK_HELPER: Mutex<Option<KeyringHelper>> = Mutex::new(None);

const _: () = assert!(
    MAX_KEYRING_HELPERS == 2,
    "primary + fallback slots must match MAX_KEYRING_HELPERS"
);

fn helper_slot(backend: KeyringBackend) -> &'static Mutex<Option<KeyringHelper>> {
    match backend {
        KeyringBackend::Primary => &PRIMARY_HELPER,
        KeyringBackend::Fallback => &FALLBACK_HELPER,
    }
}

/// Sender for the single helper thread of `backend`. Spawns at most once.
fn helper_sender(
    backend: KeyringBackend,
) -> Result<mpsc::SyncSender<KeyringJob>, CredentialsStoreError> {
    let slot = helper_slot(backend);
    let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(existing) = guard.as_ref() {
        return Ok(existing.tx.clone());
    }
    let (tx, rx) = mpsc::sync_channel::<KeyringJob>(1);
    std::thread::Builder::new()
        .name("grok-keyring-op".into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                (job.run)();
            }
        })
        .map_err(|e| {
            CredentialsStoreError::Keyring(format!("failed to spawn keyring worker: {e}"))
        })?;
    *guard = Some(KeyringHelper { tx: tx.clone() });
    Ok(tx)
}

/// Run a blocking keyring call on the reused helper for `backend`.
///
/// Does **not** update the resolve circuit breaker — callers note composite
/// multi-backend outcomes so a healthy fallback can clear the circuit even when
/// primary timed out.
///
/// On timeout the helper may still be blocked in D-Bus; we abandon **waiting**
/// so interactive login can fail loudly instead of hanging forever. The same
/// helper stays in flight. A later op is queued on that helper or times out
/// without spawning another sleeper. The in-flight helper may still complete
/// a slow write later (module docs) and retains any secret cloned into the op
/// until then.
fn run_keyring_op<T, F>(
    backend: KeyringBackend,
    kind: KeyringOpKind,
    url: &str,
    op: F,
) -> Result<T, CredentialsStoreError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CredentialsStoreError> + Send + 'static,
{
    // `kind` and `url` are read on the test injection path. Keep them live
    // for the non-test build, where the probe is counted inside `primary_*`.
    let _ = (kind, url);
    #[cfg(test)]
    if let Err(err) = test_hooks::maybe_inject_before_op(backend) {
        // Injection returns on this thread, before `primary_get`. Count the
        // primary attempt here so a circuit test sees one probe, then a flat
        // second read. Fallback injection is not Secret Service SearchItems.
        if backend == KeyringBackend::Primary {
            note_primary_attempt(kind, url);
        }
        return Err(err);
    }

    let timeout = keyring_op_timeout();
    let helper = helper_sender(backend)?;
    let (reply_tx, reply_rx) = mpsc::channel();

    // Capture hang at enqueue so a leftover queued job cannot call the real
    // keyring after the test hook is cleared.
    #[cfg(test)]
    let hang = test_hooks::hang_backend(backend);

    let job = KeyringJob {
        run: Box::new(move || {
            #[cfg(test)]
            if hang {
                while test_hooks::hang_backend(backend) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                drop(op);
                return;
            }
            let _ = reply_tx.send(op());
        }),
    };

    // `SyncSender::send_timeout` is unstable; poll `try_send` until the
    // helper accepts the job or the same wall-clock budget expires.
    let enqueue_deadline = Instant::now() + timeout;
    let mut pending = job;
    loop {
        match helper.try_send(pending) {
            Ok(()) => break,
            Err(mpsc::TrySendError::Disconnected(_)) => {
                return Err(CredentialsStoreError::Keyring(
                    "keyring worker disconnected before accepting work".into(),
                ));
            }
            Err(mpsc::TrySendError::Full(back)) => {
                if Instant::now() >= enqueue_deadline {
                    return Err(CredentialsStoreError::KeyringTimeout {
                        secs: timeout_secs_for_display(timeout),
                    });
                }
                pending = back;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    match reply_rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(CredentialsStoreError::KeyringTimeout {
            secs: timeout_secs_for_display(timeout),
        }),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(CredentialsStoreError::Keyring(
            "keyring worker disconnected before completing".into(),
        )),
    }
}

fn primary_get(url: &str) -> Result<Option<(String, String)>, CredentialsStoreError> {
    #[cfg(test)]
    if test_hooks::mock_primary_active() {
        return test_hooks::mock_primary_get(url);
    }
    note_keyring_search_items(SERVICE_NAME, url);
    let entry = keyring::Entry::new(SERVICE_NAME, url)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(secret) if !secret.is_empty() => {
            note_keyring_get_secret(SERVICE_NAME, url);
            Ok(Some((BEARER_USERNAME.to_owned(), secret)))
        }
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(CredentialsStoreError::Keyring(e.to_string())),
    }
}

fn primary_set(url: &str, secret: &str) -> Result<(), CredentialsStoreError> {
    #[cfg(test)]
    if test_hooks::mock_primary_active() {
        return test_hooks::mock_primary_set(url, secret);
    }
    note_keyring_set_secret(SERVICE_NAME, url);
    let entry = keyring::Entry::new(SERVICE_NAME, url)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))?;
    entry
        .set_password(secret)
        .map_err(|e| CredentialsStoreError::Keyring(e.to_string()))
}

fn primary_delete(url: &str) -> Result<(), CredentialsStoreError> {
    #[cfg(test)]
    if test_hooks::mock_primary_active() {
        return test_hooks::mock_primary_delete(url);
    }
    note_keyring_delete(SERVICE_NAME, url);
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

/// Kernel keyutils. No D-Bus, no Secret Service Unlock, and no Prompt.
/// A locked login collection stays a credentials-store miss on the primary
/// backend; this fallback does not ask the operator to unlock it.
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
    let primary = run_keyring_op(KeyringBackend::Primary, KeyringOpKind::Read, url, {
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

    #[cfg(test)]
    if test_hooks::mock_primary_active() && !test_hooks::mock_fallback_active() {
        // Isolated in-memory primary: do not touch OS keyutils on a mock miss.
        return match primary_miss {
            Ok(None) => {
                clear_keyring_circuit();
                Ok(None)
            }
            other => other,
        };
    }

    let fallback = run_keyring_op(KeyringBackend::Fallback, KeyringOpKind::Read, url, {
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
    let primary = run_keyring_op(KeyringBackend::Primary, KeyringOpKind::Write, url, {
        let url = url_owned.clone();
        let secret = secret_owned.clone();
        move || primary_set(&url, &secret)
    });
    if primary.is_ok() {
        clear_keyring_circuit();
        return Ok(());
    }
    let primary_err = primary.expect_err("checked is_err");

    #[cfg(test)]
    if test_hooks::mock_primary_active() && !test_hooks::mock_fallback_active() {
        note_keyring_err(&primary_err);
        return Err(primary_err);
    }

    let fallback = run_keyring_op(KeyringBackend::Fallback, KeyringOpKind::Write, url, {
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
    let primary = run_keyring_op(KeyringBackend::Primary, KeyringOpKind::Delete, url, {
        let url = url_owned.clone();
        move || primary_delete(&url)
    });
    #[cfg(test)]
    if test_hooks::mock_primary_active() && !test_hooks::mock_fallback_active() {
        return match primary {
            Ok(()) => {
                clear_keyring_circuit();
                Ok(())
            }
            Err(e) => {
                note_keyring_err(&e);
                Err(e)
            }
        };
    }
    let fallback = run_keyring_op(KeyringBackend::Fallback, KeyringOpKind::Delete, url, {
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
        /// Use in-memory mock as the primary backend (hermetic cache tests).
        mock_primary: bool,
        /// Use in-memory mock as the secure fallback (hermetic dual-backend tests).
        mock_fallback: bool,
        /// Mock secrets: credential URL → secret (shared by primary and fallback mocks).
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
        clear_keyring_success_cache();
        reset_keyring_op_counts();
    }

    /// In-memory primary keyring. No D-Bus, no OS keyutils.
    ///
    /// Named contract tests for the success cache use this mock as the spy.
    pub fn simulate_in_memory_keyring() -> HookGuard {
        clear();
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.mock_primary = true;
        HookGuard
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

    pub(super) fn mock_primary_active() -> bool {
        HOOKS.lock().expect("keyring test hooks lock").mock_primary
    }

    pub(super) fn mock_primary_get(
        url: &str,
    ) -> Result<Option<(String, String)>, CredentialsStoreError> {
        note_keyring_search_items(SERVICE_NAME, url);
        let secret = {
            let g = HOOKS.lock().expect("keyring test hooks lock");
            g.mock_secrets.get(url).cloned()
        };
        match secret {
            Some(s) if !s.is_empty() => {
                note_keyring_get_secret(SERVICE_NAME, url);
                Ok(Some((BEARER_USERNAME.to_owned(), s)))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn mock_primary_set(url: &str, secret: &str) -> Result<(), CredentialsStoreError> {
        note_keyring_set_secret(SERVICE_NAME, url);
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.mock_secrets.insert(url.to_owned(), secret.to_owned());
        Ok(())
    }

    pub(super) fn mock_primary_delete(url: &str) -> Result<(), CredentialsStoreError> {
        note_keyring_delete(SERVICE_NAME, url);
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.mock_secrets.remove(url);
        Ok(())
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

    /// Seed the in-memory mock without counting a SetSecret (spy setup).
    pub fn mock_seed(url: &str, secret: &str) {
        let mut g = HOOKS.lock().expect("keyring test hooks lock");
        g.mock_secrets.insert(url.to_owned(), secret.to_owned());
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

    /// Named contract: a locked or timed-out Secret Service call must not leave
    /// an extra sleeping `grok-keyring-op` worker. At most one helper per
    /// backend (primary + fallback = 2) may stay in flight. Repeated resolve,
    /// RMW, and write timeouts must reuse those helpers, not abandon a new
    /// sleeper on every attempt (that is the overnight 3,200-thread leak).
    #[test]
    #[serial]
    fn timed_out_secret_service_does_not_leave_unbounded_sleeping_workers() {
        let _hooks = test_hooks::simulate_blocked_keyring(Duration::from_millis(40));
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path_prefer_keyring(dir.path().join(FILE_NAME));
        let url = "https://api.x.ai/v1";

        let before = count_threads_named("grok-keyring-op");
        for i in 0..10 {
            let _ = store.read(url);
            let _ = store
                .read_for_update(url)
                .expect_err("blocked keyring RMW must fail closed");
            let _ = store
                .write_bearer(url, &format!("secret-must-not-dump-{i}"))
                .expect_err("blocked keyring write must fail loud");
        }
        let after = count_threads_named("grok-keyring-op");
        let grown = after.saturating_sub(before);
        assert!(
            grown <= MAX_KEYRING_HELPERS,
            "timed-out Secret Service calls must reuse at most {MAX_KEYRING_HELPERS} \
             grok-keyring-op helpers (one per backend); this burst grew {grown} \
             (before={before} after={after})"
        );
    }

    fn session_url() -> &'static str {
        "supergrok-session:https://auth.x.ai::placeholder::personal"
    }

    fn prefer_keyring_store() -> (TempDir, CredentialsStore) {
        let dir = TempDir::new().unwrap();
        let store = CredentialsStore::at_path_prefer_keyring(dir.path().join(FILE_NAME));
        (dir, store)
    }

    /// Named contract: after one successful read of a session url, a second
    /// read does not call the keyring backend. SearchItems/GetSecret stay 1.
    #[test]
    #[serial]
    fn second_read_of_a_session_url_does_not_hit_the_keyring() {
        let _hooks = test_hooks::simulate_in_memory_keyring();
        let (_dir, store) = prefer_keyring_store();
        let url = session_url();
        test_hooks::mock_seed(url, "session-token-one");

        let first = store.read(url).unwrap().expect("present");
        assert_eq!(first.1, "session-token-one");
        let after_first = keyring_backend_op_counts();
        assert_eq!(after_first.search_items, 1, "first read SearchItems");
        assert_eq!(after_first.get_secret, 1, "first read GetSecret");

        let second = store.read(url).unwrap().expect("cached");
        assert_eq!(second.1, "session-token-one");
        let after_second = keyring_backend_op_counts();
        assert_eq!(
            after_second.search_items, 1,
            "second read must not SearchItems"
        );
        assert_eq!(after_second.get_secret, 1, "second read must not GetSecret");
        assert_eq!(after_second.set_secret, 0);
        assert_eq!(after_second.delete, 0);
    }

    /// Named contract: write updates the cache so the next read does not hit
    /// the keyring.
    #[test]
    #[serial]
    fn write_is_write_through_and_the_next_read_skips_the_keyring() {
        let _hooks = test_hooks::simulate_in_memory_keyring();
        let (_dir, store) = prefer_keyring_store();
        let url = session_url();

        store.write(url, "key", "session-token-written").unwrap();
        let after_write = keyring_backend_op_counts();
        assert_eq!(
            after_write.search_items, 1,
            "write re-reads once before SetSecret"
        );
        assert_eq!(after_write.set_secret, 1, "write SetSecret");

        let got = store.read(url).unwrap().expect("cached after write");
        assert_eq!(got.0, "key");
        assert_eq!(got.1, "session-token-written");
        let after_read = keyring_backend_op_counts();
        assert_eq!(
            after_read.search_items, after_write.search_items,
            "read after write must not SearchItems"
        );
        assert_eq!(
            after_read.get_secret, after_write.get_secret,
            "read after write must not GetSecret"
        );
        assert_eq!(after_read.set_secret, 1);
    }

    /// Named contract: refresh re-reads the keyring once before writing a new
    /// token (anti-stampede).
    #[test]
    #[serial]
    fn refresh_re_reads_the_keyring_once_before_writing_a_new_token() {
        let _hooks = test_hooks::simulate_in_memory_keyring();
        let (_dir, store) = prefer_keyring_store();
        let url = session_url();

        store.write(url, "key", "token-v1").unwrap();
        let after_first = keyring_backend_op_counts();
        store.write(url, "key", "token-v2").unwrap();
        let after_refresh = keyring_backend_op_counts();

        assert_eq!(
            after_refresh.search_items,
            after_first.search_items + 1,
            "refresh must SearchItems once before SetSecret"
        );
        assert_eq!(
            after_refresh.get_secret,
            after_first.get_secret + 1,
            "refresh must GetSecret the live token before overwrite"
        );
        assert_eq!(
            after_refresh.set_secret,
            after_first.set_secret + 1,
            "refresh SetSecret once"
        );
        let got = store.read(url).unwrap().expect("v2 cached");
        assert_eq!(got.1, "token-v2");
        let after_read = keyring_backend_op_counts();
        assert_eq!(after_read.search_items, after_refresh.search_items);
        assert_eq!(after_read.set_secret, after_refresh.set_secret);
    }

    /// Named contract: on refresh, if another process already wrote a new
    /// token, adopt it instead of overwriting.
    #[test]
    #[serial]
    fn refresh_adopts_a_token_another_process_already_wrote() {
        let _hooks = test_hooks::simulate_in_memory_keyring();
        let (_dir, store) = prefer_keyring_store();
        let url = session_url();

        store.write(url, "key", "token-v1").unwrap();
        test_hooks::mock_seed(url, "token-from-other-process");
        let before = keyring_backend_op_counts();

        store.write(url, "key", "token-v2-stale-refresh").unwrap();
        let after = keyring_backend_op_counts();
        assert_eq!(
            after.search_items,
            before.search_items + 1,
            "anti-stampede re-read"
        );
        assert_eq!(
            after.set_secret, before.set_secret,
            "must not SetSecret over the other process token"
        );
        let got = store.read(url).unwrap().expect("adopted");
        assert_eq!(got.1, "token-from-other-process");
    }

    fn assert_error_circuit_skips_the_second_read() {
        let _hooks = test_hooks::simulate_keyring_error("simulated secret service down");
        let (_dir, store) = prefer_keyring_store();
        let url = session_url();
        let before = keyring_backend_op_counts();

        let _ = store.read(url);
        assert!(
            keyring_circuit_open(),
            "resolve error must open the keyring circuit"
        );
        let after_first = keyring_backend_op_counts();
        assert!(
            after_first.search_items > before.search_items,
            "the failing probe must count SearchItems before the circuit opens"
        );
        let _ = store.read(url);
        let after_second = keyring_backend_op_counts();
        assert_eq!(
            after_second.search_items, after_first.search_items,
            "open circuit must not SearchItems again"
        );
        assert_eq!(after_second.get_secret, after_first.get_secret);
        assert_eq!(after_second.set_secret, after_first.set_secret);

        let refresh = store.reread_keyring_before_refresh(url);
        assert!(
            refresh.is_err(),
            "refresh re-read must back off while the circuit is open"
        );
        let after_refresh = keyring_backend_op_counts();
        assert_eq!(
            after_refresh.search_items, after_second.search_items,
            "backed-off refresh re-read must not SearchItems"
        );
    }

    /// Named contract: circuit/error backoff still skips tight retries after
    /// failure. A second resolve read does not dispatch another SearchItems.
    #[test]
    #[serial]
    fn keyring_error_circuit_skips_tight_retries() {
        assert_error_circuit_skips_the_second_read();
    }

    /// Named contract: circuit/error backoff still skips tight retries after
    /// failure.
    #[test]
    #[serial]
    fn circuit_error_backoff_skips_tight_retries_after_failure() {
        assert_error_circuit_skips_the_second_read();
    }

    /// Named contract: delete invalidates the cache so the next read probes.
    #[test]
    #[serial]
    fn delete_invalidates_the_cache() {
        let _hooks = test_hooks::simulate_in_memory_keyring();
        let (_dir, store) = prefer_keyring_store();
        let url = session_url();

        store.write(url, "key", "session-token-del").unwrap();
        let _ = store.read(url).unwrap();
        store.delete(url).unwrap();
        let after_delete = keyring_backend_op_counts();
        assert_eq!(after_delete.delete, 1);

        let got = store.read(url).unwrap();
        assert!(got.is_none(), "deleted url is absent");
        let after_read = keyring_backend_op_counts();
        assert_eq!(
            after_read.search_items,
            after_delete.search_items + 1,
            "read after delete must SearchItems"
        );
    }

    /// Named contract: refresh re-reads the keyring once and adopts a sibling
    /// token without SetSecret. The following read stays on the cache.
    #[test]
    #[serial]
    fn reread_before_refresh_adopts_a_sibling_token_with_one_probe() {
        let _hooks = test_hooks::simulate_in_memory_keyring();
        let (_dir, store) = prefer_keyring_store();
        let url = session_url();

        store.write(url, "key", "token-v1").unwrap();
        test_hooks::mock_seed(url, "token-from-sibling");
        let before = keyring_backend_op_counts();
        let look = store.reread_keyring_before_refresh(url).unwrap();
        assert_eq!(look, KeyringRefreshLook::Adopted);
        let after = keyring_backend_op_counts();
        assert_eq!(after.search_items, before.search_items + 1);
        assert_eq!(after.get_secret, before.get_secret + 1);
        assert_eq!(after.set_secret, before.set_secret);
        let got = store.read(url).unwrap().expect("adopted");
        assert_eq!(got.1, "token-from-sibling");
        assert_eq!(
            keyring_backend_op_counts().search_items,
            after.search_items,
            "read after adopt must not SearchItems"
        );
    }

    /// File-only stores (unit tests, `GROK_CREDENTIALS_FORCE_FILE`) must not
    /// probe the keyring on the refresh re-read.
    #[test]
    #[serial]
    fn file_only_reread_does_not_touch_the_keyring() {
        let _hooks = test_hooks::simulate_in_memory_keyring();
        let (_dir, store) = temp_store();
        let before = keyring_backend_op_counts();
        let look = store.reread_keyring_before_refresh(session_url()).unwrap();
        assert_eq!(look, KeyringRefreshLook::Unchanged);
        let after = keyring_backend_op_counts();
        assert_eq!(after.search_items, before.search_items);
        assert_eq!(after.get_secret, before.get_secret);
        assert_eq!(after.set_secret, before.set_secret);
    }
}

/// Linux `/proc` thread-name count. Used by the leak contract test.
#[cfg(test)]
fn count_threads_named(name: &str) -> usize {
    let dir = std::fs::read_dir("/proc/self/task")
        .expect("need /proc/self/task to count grok-keyring-op workers");
    dir.filter_map(|entry| {
        let entry = entry.ok()?;
        let comm = std::fs::read_to_string(entry.path().join("comm")).ok()?;
        (comm.trim() == name).then_some(())
    })
    .count()
}
