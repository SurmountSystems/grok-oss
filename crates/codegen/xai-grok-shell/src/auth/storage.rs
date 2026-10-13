use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::model::{API_KEY_SCOPE, AuthMode, AuthStore, GrokAuth};

/// Heartbeat slot on this duplicate writer. The login crate keeps `manager::lock` private.
pub(super) struct LockHeartbeat;

/// RAII guard for an exclusive advisory lock on `auth.json.lock`.
/// The lock is released when the inner `File` is dropped (closing the FD).
///
/// Field order is load-bearing: `_heartbeat` drops before `_file`, so the
/// heartbeat thread is stopped and joined while the flock is still held.
/// A late heartbeat can never write holder info into a lock file a sibling has
/// already re-acquired.
pub(crate) struct AuthFileLock {
    /// Periodic `PID:TS` re-writer. `None` for short holds.
    pub(super) _heartbeat: Option<LockHeartbeat>,
    pub(super) _file: File,
}

impl AuthFileLock {
    /// Returns `true` while this guard still refers to the **live**
    /// `auth.json.lock` inode.
    ///
    /// A waiter that finds a holder stuck past the stale-lock timeout breaks
    /// the lock by `unlink`ing the file and recreating it on a fresh inode
    /// (see [`crate::auth::manager::lock`]). The usual cause of a "stuck"
    /// holder is a process **suspended across system sleep** while holding the
    /// lock: it stays alive (so the kernel never releases its flock) yet makes
    /// no progress, so siblings break it. When such a holder resumes, its
    /// flock lives on the now-deleted inode — it no longer holds the live lock
    /// even though this `AuthFileLock` still exists.
    ///
    /// Callers about to perform an irreversible, lock-protected action
    /// (sending a refresh token to the IdP, writing `auth.json`) MUST
    /// re-validate first; otherwise two processes can spend the same refresh
    /// token and trip token-family revocation.
    ///
    /// Non-Unix has no inode concept, so this conservatively returns `true`.
    #[cfg(unix)]
    pub(crate) fn still_live(&self, auth_json_path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        let lock_path = auth_json_path.with_file_name("auth.json.lock");
        let (Ok(fd_meta), Ok(path_meta)) = (self._file.metadata(), std::fs::metadata(&lock_path))
        else {
            // Lock file gone or unreadable → we no longer hold the live lock.
            return false;
        };
        fd_meta.ino() == path_meta.ino() && fd_meta.dev() == path_meta.dev()
    }

    #[cfg(not(unix))]
    pub(crate) fn still_live(&self, _auth_json_path: &Path) -> bool {
        true
    }

    /// Re-check liveness and mint the witness. `None` means a sibling broke
    /// this lock as stuck (see [`Self::still_live`]); on non-Unix liveness is
    /// assumed (no flock-break protocol there).
    pub(crate) fn live(&self, auth_json_path: &Path) -> Option<LiveAuthFileLock<'_>> {
        self.still_live(auth_json_path)
            .then_some(LiveAuthFileLock(self))
    }
}

/// Liveness-witnessed borrow of an [`AuthFileLock`], only obtainable via
/// [`AuthFileLock::live`]: a function requiring it cannot be reached with a
/// lock whose inode a sibling already broke — the post-suspend case
/// [`AuthFileLock::still_live`] exists for. Borrowed, so it cannot outlive
/// the flock it vouches for (though liveness is proved at mint time, not
/// continuously).
pub(crate) struct LiveAuthFileLock<'a>(#[allow(dead_code)] &'a AuthFileLock);

pub fn read_auth_json(auth_file: &Path) -> std::io::Result<AuthStore> {
    let mut file = File::open(auth_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Tighten world-readable copies (hand-restored, umask edge cases, etc.).
    // Best-effort: a chmod failure must not block login/read paths.
    if let Err(e) = crate::util::secure_file::ensure_owner_only_permissions(auth_file) {
        tracing::warn!(
            path = %auth_file.display(),
            error = %e,
            "auth: failed to enforce owner-only permissions on auth.json"
        );
    }

    // Empty files are valid (recover from prior crash/partial write).
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return Ok(AuthStore::new());
    }

    let mut map = serde_json::from_str(trimmed)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    restore_session_secrets(auth_file, &mut map);
    Ok(map)
}

/// Read auth.json, returning an empty map if the file does not exist.
///
/// Non-empty corrupt JSON, permission errors, etc. are returned as errors
/// so the caller can decide whether to skip the write (to avoid clobbering
/// sibling scopes).
///
/// Kept for the test-only `persist_and_swap` and as a strict reader.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "used from tests only; remove expect when wired in production"
    )
)]
pub(crate) fn read_auth_json_or_empty(auth_file: &Path) -> std::io::Result<AuthStore> {
    match read_auth_json(auth_file) {
        Ok(map) => Ok(map),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AuthStore::new()),
        Err(e) => Err(e),
    }
}

/// Best-effort backup of a corrupt (unparseable) auth.json.
///
/// If the file exists and `read_auth_json` fails with `InvalidData`,
/// it is renamed to `auth.json.corrupt.<millis>` (sibling in the same
/// directory) and the backup path is returned. Used before recovery
/// writes so the original bytes are never silently lost.
pub(crate) fn backup_corrupt_auth_file(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    if read_auth_json(path).is_ok() {
        return None;
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "auth.json".to_string());

    let backup_name = format!("{}.corrupt.{}", file_name, ts);
    let backup = path.with_file_name(backup_name);

    match std::fs::rename(path, &backup) {
        Ok(()) => {
            // Corrupt backups still hold token material — keep them owner-only.
            let _ = crate::util::secure_file::ensure_owner_only_permissions(&backup);
            tracing::warn!(
                original = %path.display(),
                backup = %backup.display(),
                "auth: backed up corrupt auth.json before recovery write"
            );
            // Must reach unified.jsonl: the tracing line above is invisible
            // in production captures, and this is the only record of both
            // the corruption and where the original bytes went.
            xai_grok_telemetry::unified_log::error(
                "auth: corrupt auth.json backed up",
                None,
                Some(serde_json::json!({
                    "original": path.display().to_string(),
                    "backup": backup.display().to_string(),
                })),
            );
            Some(backup)
        }
        Err(e) => {
            tracing::warn!(error = %e, "auth: failed to rename corrupt auth.json for backup");
            xai_grok_telemetry::unified_log::error(
                "auth: corrupt auth.json backup failed",
                None,
                Some(serde_json::json!({
                    "original": path.display().to_string(),
                    "error": e.to_string(),
                })),
            );
            None
        }
    }
}

/// Read auth.json for an upcoming write, with recovery for corrupt files.
///
/// - Missing/empty → empty map (safe to write fresh)
/// - Valid JSON → parsed map
/// - Non-empty corrupt JSON → backs up to `auth.json.corrupt.<millis>`,
///   then returns empty map so the caller can write the new credential.
///
/// Other I/O errors (PermissionDenied, etc.) are still returned as errors.
pub(crate) fn read_auth_json_or_empty_recovering_corrupt(
    auth_file: &Path,
) -> std::io::Result<AuthStore> {
    match read_auth_json(auth_file) {
        Ok(map) => Ok(map),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AuthStore::new()),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            let _ = backup_corrupt_auth_file(auth_file);
            Ok(AuthStore::new())
        }
        Err(e) => Err(e),
    }
}

/// Persist `auth.json`, preferring a crash-safe atomic write but falling
/// back to a non-atomic in-place write when the disk is full.
///
/// The atomic path (temp + rename) needs free space >= the file size,
/// because the old file and a full temp copy coexist until the rename. On a
/// nearly-full disk that temp copy can fail with `StorageFull` (ENOSPC)
/// even though the credentials themselves are tiny. When that happens we
/// retry with an in-place truncate+write, which only needs the freed blocks
/// of the old file — far less than the temp-copy approach.
///
/// The in-place path is non-atomic, with two accepted trade-offs:
/// - If the in-place write itself fails (e.g. a concurrent process grabs the
///   just-freed blocks, or a crash mid-write), the prior bytes are restored
///   best-effort so a torn/empty file never *replaces* the previous on-disk
///   credential — on-disk state ends up no worse than before the attempt.
/// - Unlocked concurrent readers can still observe a torn (partial) file
///   during the brief write window; a partial file is healed on the next
///   read via [`read_auth_json_or_empty_recovering_corrupt`] (backup +
///   relogin). This window is inherent to any sub-1×-free single-file
///   replace and is preferable to persisting nothing at all, which would
///   leave every concurrent process with a stale, already-revoked token.
pub(super) fn write_auth_json(auth_file: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    let disk = prepare_auth_store_for_disk(auth_file, auth_store)?;
    write_auth_json_with(auth_file, &disk, write_auth_json_atomic)
}

fn session_secret_store_error() -> std::io::Error {
    std::io::Error::other("session secret store write failed")
}

fn is_supergrok_session(auth: &GrokAuth) -> bool {
    matches!(auth.auth_mode, AuthMode::Oidc | AuthMode::WebLogin)
}

fn session_key_url(account: &str) -> String {
    format!("supergrok-session:{account}")
}

fn session_refresh_url(account: &str) -> String {
    format!("supergrok-session:{account}#refresh_token")
}

fn session_credential_store(auth_file: &Path) -> super::credentials_store::CredentialsStore {
    #[cfg(test)]
    {
        super::credentials_store::CredentialsStore::at_path(
            auth_file.with_file_name("provider_credentials.json"),
        )
    }
    #[cfg(not(test))]
    {
        let home = auth_file.parent().unwrap_or_else(|| Path::new("."));
        super::credentials_store::CredentialsStore::at_grok_home(home)
    }
}

fn session_store_fault(auth_file: &Path) -> bool {
    #[cfg(test)]
    {
        SESSION_SECRET_STORE_FAULT_PATH
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_deref()
            == Some(auth_file)
    }
    #[cfg(not(test))]
    {
        let _ = auth_file;
        false
    }
}

/// Store SuperGrok session secrets, then return the `AuthStore` copy to write.
/// The copy omits those secrets only after the store write succeeds.
/// A store failure returns before `auth.json` is rewritten.
fn prepare_auth_store_for_disk(
    auth_file: &Path,
    auth_store: &AuthStore,
) -> std::io::Result<AuthStore> {
    let has_session = auth_store.values().any(is_supergrok_session);
    if !has_session {
        return Ok(auth_store.clone());
    }
    let needs_store = auth_store.values().any(|auth| {
        is_supergrok_session(auth) && (!auth.key.is_empty() || auth.refresh_token.is_some())
    });
    if needs_store && session_store_fault(auth_file) {
        return Err(session_secret_store_error());
    }
    let store = session_credential_store(auth_file);
    let mut disk = auth_store.clone();
    for (account, auth) in disk.iter_mut() {
        if !is_supergrok_session(auth) {
            continue;
        }
        let key_url = session_key_url(account);
        let refresh_url = session_refresh_url(account);
        if auth.key.is_empty() {
            store
                .delete(&key_url)
                .map_err(|_| session_secret_store_error())?;
        } else {
            store
                .write(&key_url, "key", &auth.key)
                .map_err(|_| session_secret_store_error())?;
            auth.key.clear();
        }
        if let Some(refresh) = auth.refresh_token.clone() {
            store
                .write(&refresh_url, "refresh_token", &refresh)
                .map_err(|_| session_secret_store_error())?;
            auth.refresh_token = None;
        } else {
            store
                .delete(&refresh_url)
                .map_err(|_| session_secret_store_error())?;
        }
    }
    Ok(disk)
}

fn restore_session_secrets(auth_file: &Path, map: &mut AuthStore) {
    let needs_restore = map.values().any(|auth| {
        is_supergrok_session(auth) && (auth.key.is_empty() || auth.refresh_token.is_none())
    });
    if !needs_restore {
        return;
    }
    let store = session_credential_store(auth_file);
    for (account, auth) in map.iter_mut() {
        if !is_supergrok_session(auth) {
            continue;
        }
        if auth.key.is_empty() {
            if let Ok(Some((username, secret))) = store.read(&session_key_url(account)) {
                if username == "key" {
                    auth.key = secret;
                }
            }
        }
        if auth.refresh_token.is_none() {
            if let Ok(Some((username, secret))) = store.read(&session_refresh_url(account)) {
                if username == "refresh_token" {
                    auth.refresh_token = Some(secret);
                }
            }
        }
    }
}

/// Dispatch helper: run `atomic`, and on `StorageFull` fall back to an
/// in-place write. Split out (with `atomic` injectable) so the disk-full
/// fallback is unit-testable without an actually-full filesystem.
fn write_auth_json_with(
    auth_file: &Path,
    auth_store: &AuthStore,
    atomic: fn(&Path, &AuthStore) -> std::io::Result<()>,
) -> std::io::Result<()> {
    match atomic(auth_file, auth_store) {
        Err(e) if e.kind() == std::io::ErrorKind::StorageFull => {
            tracing::warn!(
                path = %auth_file.display(),
                "auth: disk full during atomic write, falling back to in-place write"
            );
            // Must reach unified.jsonl: a silent in-memory-only credential
            // (the prior behavior) leaves sibling processes with a stale
            // refresh token and no record of why. Surface it loudly.
            xai_grok_telemetry::unified_log::warn(
                "auth: disk full, falling back to non-atomic in-place write",
                None,
                Some(serde_json::json!({
                    "path": auth_file.display().to_string(),
                })),
            );
            write_auth_json_in_place(auth_file, auth_store)
        }
        other => other,
    }
}

/// Serialize `auth_store` to `path` (truncate + rewrite), owner-only (0o600)
/// and `fsync`'d. Shared core of the atomic path (which targets the temp
/// file) and the in-place fallback (which targets `auth.json` directly).
///
/// Uses streaming `to_writer_pretty` through a `BufWriter` to avoid
/// allocating the entire JSON string in memory — eliminates OOM risk under
/// severe memory pressure.
fn write_store_to(path: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    use crate::util::secure_file::open_secure_file;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = open_secure_file(path)?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, auth_store)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writer.flush()?;
    writer
        .into_inner()
        .map_err(|e| e.into_error())?
        .sync_all()?;
    // `open_secure_file` mode bits apply only on create; tighten existing paths.
    // Best-effort after durable content: a chmod-only failure must not look
    // like a failed write. The in-place fallback restores the prior snapshot
    // on any `write_store_to` Err, which would discard freshly written tokens.
    // Load path re-tightens on next read.
    if let Err(e) = crate::util::secure_file::ensure_owner_only_permissions(path) {
        tracing::warn!(
            error = %e,
            path = %path.display(),
            "auth: failed to ensure owner-only permissions after write"
        );
    }
    Ok(())
}

/// Test-only, path-scoped write fault: `write_auth_json_atomic` fails with
/// `Unsupported` for exactly this `auth.json` path. Path-scoped so parallel
/// tests in the same process do not sabotage each other.
#[cfg(test)]
pub(super) static WRITE_FAULT_PATH: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Test-only fault: the session-secret store write fails for this `auth.json` path before the file is rewritten.
/// Path-scoped so parallel tests in the same process do not sabotage each other.
#[cfg(test)]
pub(super) static SESSION_SECRET_STORE_FAULT_PATH: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);

/// Atomic write: tmp + rename. Unix `rename(2)` replaces atomically;
/// Windows `rename` requires removing the target first.
fn write_auth_json_atomic(auth_file: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    #[cfg(test)]
    if WRITE_FAULT_PATH
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_deref()
        == Some(auth_file)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "injected write fault (WRITE_FAULT_PATH)",
        ));
    }
    // Unique per write (pid + monotonic seq): two concurrent in-process writers
    // (e.g. background mint + proactive refresher) must not share one tmp path.
    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
    let tmp = auth_file.with_extension(format!(
        "json.{}.{}.tmp",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));

    // Reclaim the temp file on any early return (write/sync/rename failure); the
    // unique name otherwise accumulates one orphan per failed write.
    struct TmpReclaim<'a>(Option<&'a Path>);
    impl Drop for TmpReclaim<'_> {
        fn drop(&mut self) {
            if let Some(p) = self.0 {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    let mut tmp_reclaim = TmpReclaim(Some(&tmp));

    write_store_to(&tmp, auth_store)?;
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(auth_file);
    }
    std::fs::rename(&tmp, auth_file)?;
    tmp_reclaim.0 = None; // renamed into place; nothing to reclaim
    // Re-assert on the final path (covers rename edge cases / FS quirks).
    // Best-effort: rename already published the new tokens.
    if let Err(e) = crate::util::secure_file::ensure_owner_only_permissions(auth_file) {
        tracing::warn!(
            error = %e,
            path = %auth_file.display(),
            "auth: failed to ensure owner-only permissions after rename"
        );
    }
    Ok(())
}

/// Non-atomic fallback: truncate and rewrite `auth.json` in place.
///
/// Used only when [`write_auth_json_atomic`] fails with `StorageFull`.
/// Opening with truncation first frees the old content's blocks before the
/// new bytes are written, so this needs only the file size in free space
/// rather than the temp-copy approach's file-size-of-headroom.
///
/// Truncation is destructive, so the prior bytes are snapshotted first and
/// restored best-effort if the rewrite fails partway — a failed fallback
/// must not leave an empty/torn file where a parseable (if stale) credential
/// used to be. A partial file that survives (because even the restore failed)
/// is healed on the next read via [`read_auth_json_or_empty_recovering_corrupt`].
fn write_auth_json_in_place(auth_file: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    write_auth_json_in_place_with(auth_file, auth_store, write_store_to)
}

/// Inner of [`write_auth_json_in_place`] with `write` injectable so the
/// rollback-on-failure path is unit-testable without an actually-full disk.
fn write_auth_json_in_place_with(
    auth_file: &Path,
    auth_store: &AuthStore,
    write: fn(&Path, &AuthStore) -> std::io::Result<()>,
) -> std::io::Result<()> {
    // Snapshot the prior bytes so a torn/empty write can be rolled back to
    // the previous on-disk credential. `None` when the file is absent.
    let prior = std::fs::read(auth_file).ok();
    match write(auth_file, auth_store) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(prior) = prior
                && let Err(restore_err) = restore_prior_bytes(auth_file, &prior)
            {
                tracing::warn!(
                    error = %restore_err,
                    "auth: failed to restore prior auth.json after in-place write failure"
                );
            }
            Err(e)
        }
    }
}

/// Best-effort rollback: rewrite `bytes` (owner-only, `fsync`'d) after a
/// failed in-place write so a torn/empty file does not replace the prior
/// credential.
fn restore_prior_bytes(auth_file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use crate::util::secure_file::open_secure_file;

    let mut file = open_secure_file(auth_file)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    crate::util::secure_file::ensure_owner_only_permissions(auth_file)?;
    Ok(())
}

/// Read the API key from the `xai::api_key` scope in auth.json.
pub fn read_api_key(grok_home: &Path) -> Option<String> {
    let path = grok_home.join("auth.json");
    let map = read_auth_json(&path).ok()?;
    map.get(API_KEY_SCOPE).map(|a| a.key.clone())
}

/// Store a plain API key in auth.json under the `xai::api_key` scope.
///
/// Uses the corrupt-recovery reader so a malformed auth.json (e.g. from a
/// previous crash) can be healed when the user sets an API key.
///
/// Also best-effort mirrors into the Grok secret store (keyring /
/// `provider_credentials.json`) for dual-auth console-key failover when
/// `XAI_API_KEY` is not set (env wins; store write is skipped).
pub fn store_api_key(grok_home: &Path, api_key: &str) -> std::io::Result<()> {
    let path = grok_home.join("auth.json");
    let mut map = read_auth_json_or_empty_recovering_corrupt(&path)?;
    map.insert(
        API_KEY_SCOPE.to_owned(),
        GrokAuth {
            key: api_key.to_owned(),
            auth_mode: AuthMode::ApiKey,
            ..Default::default()
        },
    );
    write_auth_json(&path, &map)?;
    // Dual-write to secret store for dual-auth resolve (fail-open on store).
    // Multi-add: append unique keys so `grok login --api-key` can stack.
    let store = super::credentials_store::CredentialsStore::at_grok_home(grok_home);
    if let Err(e) = super::xai_console::add_console_api_key(&store, api_key) {
        match e {
            super::xai_console::XaiConsoleAuthError::EnvVarSet => {}
            other => tracing::debug!(
                error = %other,
                "auth: could not mirror console API key into secret store"
            ),
        }
    }
    Ok(())
}

/// Remove the `xai::api_key` scope from auth.json.
pub fn clear_api_key(grok_home: &Path) -> std::io::Result<()> {
    let path = grok_home.join("auth.json");
    if let Ok(mut map) = read_auth_json(&path) {
        map.remove(API_KEY_SCOPE);
        if map.is_empty() {
            let _ = std::fs::remove_file(&path);
        } else {
            write_auth_json(&path, &map)?;
        }
    }
    let store = super::credentials_store::CredentialsStore::at_grok_home(grok_home);
    if let Err(e) = store.delete(&super::xai_console::credential_url(None)) {
        tracing::debug!(error = %e, "auth: could not clear console API key from secret store");
    }
    Ok(())
}

#[cfg(test)]
mod write_fallback_tests {
    use super::*;

    fn sample_store() -> AuthStore {
        let mut map = AuthStore::new();
        map.insert(
            API_KEY_SCOPE.to_owned(),
            GrokAuth {
                key: "secret-key".to_owned(),
                auth_mode: AuthMode::ApiKey,
                ..Default::default()
            },
        );
        map
    }

    fn read_key(path: &Path) -> Option<String> {
        read_auth_json(path)
            .ok()
            .and_then(|m| m.get(API_KEY_SCOPE).map(|a| a.key.clone()))
    }

    fn fake_storage_full(_: &Path, _: &AuthStore) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
    }

    fn fake_permission_denied(_: &Path, _: &AuthStore) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    }

    /// Simulates an in-place write that truncates the file (destroying the
    /// old content, as `open_secure_file` does) and then fails partway — the
    /// torn-write case the rollback must recover from.
    fn fake_truncate_then_fail(path: &Path, _: &AuthStore) -> std::io::Result<()> {
        crate::util::secure_file::open_secure_file(path)?; // truncates to 0 bytes
        Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
    }

    #[test]
    fn in_place_write_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        assert_eq!(read_key(&path).as_deref(), Some("secret-key"));
    }

    #[cfg(unix)]
    #[test]
    fn in_place_write_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "in-place write must stay 0o600");
    }

    #[cfg(unix)]
    #[test]
    fn write_tightens_preexisting_world_readable_auth_json() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::write(&path, b"{}").unwrap();
        let mut loose = std::fs::metadata(&path).unwrap().permissions();
        loose.set_mode(0o644);
        std::fs::set_permissions(&path, loose).unwrap();

        write_auth_json(&path, &sample_store()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "rewrite must tighten preexisting open perms"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_tightens_world_readable_auth_json() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json(&path, &sample_store()).unwrap();
        let mut loose = std::fs::metadata(&path).unwrap().permissions();
        loose.set_mode(0o644);
        std::fs::set_permissions(&path, loose).unwrap();

        let _ = read_auth_json(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "load must tighten open auth.json perms"
        );
    }

    /// A `StorageFull` (ENOSPC) failure on the atomic path must fall back to
    /// the in-place write so the credential still lands on disk.
    #[test]
    fn falls_back_to_in_place_on_storage_full() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_with(&path, &sample_store(), fake_storage_full).unwrap();
        assert_eq!(
            read_key(&path).as_deref(),
            Some("secret-key"),
            "disk-full atomic write must fall back to a successful in-place write"
        );
    }

    /// Non-ENOSPC errors must propagate unchanged and must NOT trigger the
    /// in-place fallback (e.g. a permission error should not write the file).
    #[test]
    fn propagates_non_storage_full_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let err = write_auth_json_with(&path, &sample_store(), fake_permission_denied).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(!path.exists(), "non-ENOSPC failure must not write the file");
    }

    /// The normal (real atomic) path still works end to end.
    #[test]
    fn atomic_write_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json(&path, &sample_store()).unwrap();
        assert_eq!(read_key(&path).as_deref(), Some("secret-key"));
    }

    /// On a failed atomic write, the `TmpReclaim` guard must remove the temp
    /// file so no orphan accumulates. Here `auth.json` is a directory, so the
    /// `rename` fails after the temp file is written.
    #[test]
    fn atomic_write_reclaims_tmp_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::create_dir(&path).unwrap();

        assert!(
            write_auth_json_atomic(&path, &sample_store()).is_err(),
            "rename onto a directory must fail"
        );

        let orphans: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(
            orphans.is_empty(),
            "TmpReclaim must remove the temp file on failure: {orphans:?}"
        );
    }

    /// A fallback write that truncates then fails must roll back to the prior
    /// bytes instead of leaving an empty/torn file — otherwise a second
    /// disk-full failure would destroy a previously-valid credential.
    #[test]
    fn in_place_restores_prior_bytes_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        // Seed a valid prior credential.
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        assert_eq!(read_key(&path).as_deref(), Some("secret-key"));

        let mut replacement = AuthStore::new();
        replacement.insert(
            API_KEY_SCOPE.to_owned(),
            GrokAuth {
                key: "replacement-key".to_owned(),
                auth_mode: AuthMode::ApiKey,
                ..Default::default()
            },
        );
        let err = write_auth_json_in_place_with(&path, &replacement, fake_truncate_then_fail)
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::StorageFull);
        assert_eq!(
            read_key(&path).as_deref(),
            Some("secret-key"),
            "a failed in-place write must restore the prior credential, not leave an empty file"
        );
    }

    /// Rollback after a failed write must keep the file owner-only (0o600).
    #[cfg(unix)]
    #[test]
    fn in_place_restore_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        let _ = write_auth_json_in_place_with(&path, &sample_store(), fake_truncate_then_fail);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "restored file must stay 0o600");
    }

    const PLACEHOLDER_ACCOUNT: &str = "placeholder-account";
    const PLACEHOLDER_SESSION_KEY: &str = "placeholder-session-key";
    const PLACEHOLDER_REFRESH: &str = "placeholder-refresh-token";

    fn session_store(key: &str, refresh: Option<&str>) -> AuthStore {
        let mut map = AuthStore::new();
        map.insert(
            PLACEHOLDER_ACCOUNT.to_owned(),
            GrokAuth {
                key: key.to_owned(),
                refresh_token: refresh.map(str::to_owned),
                auth_mode: AuthMode::Oidc,
                user_id: "placeholder-user".to_owned(),
                ..Default::default()
            },
        );
        map
    }

    fn credentials_beside(auth_file: &Path) -> xai_grok_login::credentials_store::CredentialsStore {
        xai_grok_login::credentials_store::CredentialsStore::at_path(
            auth_file.with_file_name("provider_credentials.json"),
        )
    }

    fn file_hides_session_secret(path: &Path, secret: &str) {
        let bytes = std::fs::read(path).unwrap_or_default();
        assert!(
            secret.is_empty()
                || !bytes
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes()),
            "auth.json contains a session secret"
        );
    }

    fn dir_hides_session_secret(dir: &Path, secret: &str) {
        let entries = std::fs::read_dir(dir).expect("temp dir readable");
        for entry in entries {
            let path = entry.expect("temp entry").path();
            if !path.is_file() {
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("provider_credentials.json")
            {
                continue;
            }
            file_hides_session_secret(&path, secret);
        }
    }

    fn store_has_username_secret(
        store: &xai_grok_login::credentials_store::CredentialsStore,
        url: &str,
        username: &str,
        secret: &str,
    ) {
        let read = store.read(url).expect("credentials store readable");
        let Some((stored_user, stored_secret)) = read else {
            panic!("session secret missing from credentials store");
        };
        assert_eq!(stored_user, username, "credentials username");
        assert!(
            stored_secret == secret,
            "credentials store missing the session secret"
        );
    }

    fn read_restores_session(path: &Path, key: &str, refresh: Option<&str>) {
        let loaded = read_auth_json(path).expect("read finds the session");
        let session = loaded
            .get(PLACEHOLDER_ACCOUNT)
            .expect("account entry remains");
        assert!(session.key == key, "read path restores the session secret");
        assert!(
            session.refresh_token.as_deref() == refresh,
            "read path restores the refresh token"
        );
    }

    fn fresh_auth_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        (dir, path)
    }

    fn grok_build_session_json() -> String {
        format!(
            r#"{{"{PLACEHOLDER_ACCOUNT}":{{"key":"{PLACEHOLDER_SESSION_KEY}","auth_mode":"oidc","create_time":"2026-01-01T00:00:00Z","user_id":"placeholder-user","refresh_token":"{PLACEHOLDER_REFRESH}"}}}}"#
        )
    }

    fn bytes_contain(bytes: &[u8], secret: &str) -> bool {
        !secret.is_empty()
            && bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes())
    }

    struct ClearSessionStoreFault;
    impl Drop for ClearSessionStoreFault {
        fn drop(&mut self) {
            *super::SESSION_SECRET_STORE_FAULT_PATH
                .lock()
                .unwrap_or_else(|err| err.into_inner()) = None;
        }
    }

    /// A SuperGrok session secret is stored in CredentialsStore. `write_auth_json` returns Ok. `auth.json` keeps the account and omits the secret strings. A Grok Build file that still has the secrets still loads. A failed store write leaves that file unchanged.
    #[test]
    fn supergrok_session_secret_is_not_written_to_auth_json() {
        let key_url = format!("supergrok-session:{PLACEHOLDER_ACCOUNT}");
        let refresh_url = format!("supergrok-session:{PLACEHOLDER_ACCOUNT}#refresh_token");

        let (dir, path) = fresh_auth_file();
        let key_only = session_store(PLACEHOLDER_SESSION_KEY, None);
        write_auth_json(&path, &key_only).expect("session secret store write returns Ok");
        file_hides_session_secret(&path, PLACEHOLDER_SESSION_KEY);
        dir_hides_session_secret(dir.path(), PLACEHOLDER_SESSION_KEY);
        let store = credentials_beside(&path);
        store_has_username_secret(&store, &key_url, "key", PLACEHOLDER_SESSION_KEY);
        read_restores_session(&path, PLACEHOLDER_SESSION_KEY, None);
        assert!(
            path.exists()
                && std::fs::read_to_string(&path)
                    .unwrap()
                    .contains(PLACEHOLDER_ACCOUNT),
            "account entry remains on disk"
        );

        let (dir, path) = fresh_auth_file();
        let refresh_only = session_store("", Some(PLACEHOLDER_REFRESH));
        write_auth_json(&path, &refresh_only).expect("session secret store write returns Ok");
        file_hides_session_secret(&path, PLACEHOLDER_REFRESH);
        dir_hides_session_secret(dir.path(), PLACEHOLDER_REFRESH);
        let store = credentials_beside(&path);
        store_has_username_secret(&store, &refresh_url, "refresh_token", PLACEHOLDER_REFRESH);
        read_restores_session(&path, "", Some(PLACEHOLDER_REFRESH));

        let (dir, path) = fresh_auth_file();
        let both = session_store(PLACEHOLDER_SESSION_KEY, Some(PLACEHOLDER_REFRESH));
        write_auth_json(&path, &both).expect("session secret store write returns Ok");
        file_hides_session_secret(&path, PLACEHOLDER_SESSION_KEY);
        file_hides_session_secret(&path, PLACEHOLDER_REFRESH);
        dir_hides_session_secret(dir.path(), PLACEHOLDER_SESSION_KEY);
        dir_hides_session_secret(dir.path(), PLACEHOLDER_REFRESH);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains(PLACEHOLDER_ACCOUNT),
            "account id remains in auth.json"
        );
        let store = credentials_beside(&path);
        store_has_username_secret(&store, &key_url, "key", PLACEHOLDER_SESSION_KEY);
        store_has_username_secret(&store, &refresh_url, "refresh_token", PLACEHOLDER_REFRESH);
        read_restores_session(&path, PLACEHOLDER_SESSION_KEY, Some(PLACEHOLDER_REFRESH));

        let (_dir, path) = fresh_auth_file();
        let empty_refresh = session_store("", Some(""));
        write_auth_json(&path, &empty_refresh).expect("empty refresh token may be written");
        let store = credentials_beside(&path);
        store_has_username_secret(&store, &refresh_url, "refresh_token", "");
        read_restores_session(&path, "", Some(""));

        let (_dir, path) = fresh_auth_file();
        let seeded = grok_build_session_json();
        std::fs::write(&path, &seeded).unwrap();
        let before = std::fs::read(&path).unwrap();
        let loaded = read_auth_json(&path).expect("Grok Build auth.json still loads");
        let session = loaded
            .get(PLACEHOLDER_ACCOUNT)
            .expect("account entry remains");
        assert_eq!(session.auth_mode, AuthMode::Oidc, "auth mode still loads");
        assert_eq!(session.user_id, "placeholder-user", "user id still loads");
        assert_eq!(
            session.key, PLACEHOLDER_SESSION_KEY,
            "Grok Build file still loads the session key"
        );
        assert_eq!(
            session.refresh_token.as_deref(),
            Some(PLACEHOLDER_REFRESH),
            "Grok Build file still loads the refresh token"
        );
        let after_read = std::fs::read(&path).unwrap();
        assert_eq!(
            before, after_read,
            "reading a Grok Build auth.json must not rewrite it"
        );
        assert!(
            bytes_contain(&after_read, PLACEHOLDER_SESSION_KEY)
                && bytes_contain(&after_read, PLACEHOLDER_REFRESH),
            "Grok Build auth.json still contains the session secret"
        );

        let (_dir, path) = fresh_auth_file();
        let seeded = grok_build_session_json();
        std::fs::write(&path, &seeded).unwrap();
        let before = std::fs::read(&path).unwrap();
        let _clear_fault = ClearSessionStoreFault;
        *super::SESSION_SECRET_STORE_FAULT_PATH
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = Some(path.clone());
        let write_result = write_auth_json(&path, &both_from_seed());
        let after = std::fs::read(&path).unwrap();
        assert_eq!(
            before, after,
            "failed store write leaves auth.json bytes unchanged"
        );
        assert!(
            bytes_contain(&after, PLACEHOLDER_SESSION_KEY)
                && bytes_contain(&after, PLACEHOLDER_REFRESH),
            "failed store write keeps the session secret in auth.json"
        );
        let omitted = write_result.is_ok()
            && !bytes_contain(&after, PLACEHOLDER_SESSION_KEY)
            && !bytes_contain(&after, PLACEHOLDER_REFRESH);
        assert!(
            !omitted,
            "a failed store write must not succeed at a migration that omitted the secret"
        );
        drop(_clear_fault);

        let (_dir, path) = fresh_auth_file();
        let clear = session_store("", None);
        write_auth_json(&path, &clear).expect("empty key and no refresh token may be written");
        file_hides_session_secret(&path, PLACEHOLDER_SESSION_KEY);
        file_hides_session_secret(&path, PLACEHOLDER_REFRESH);
        let loaded = read_auth_json(&path).expect("read finds the session");
        let session = loaded
            .get(PLACEHOLDER_ACCOUNT)
            .expect("account entry remains");
        assert!(session.key.is_empty(), "cleared session key stays empty");
        assert!(
            session.refresh_token.is_none(),
            "cleared refresh token stays absent"
        );
    }

    fn both_from_seed() -> AuthStore {
        session_store(PLACEHOLDER_SESSION_KEY, Some(PLACEHOLDER_REFRESH))
    }
}
