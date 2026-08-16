# Flock-backed shared store (copy pattern for Slice D)

Short inventory of the existing **peer flock + JSON file** style. Copy
`grok-rate-limit` / `included_poll_history` for a SuperGrok limits snapshot
hub. Do **not** copy TUI `LeaderLock` unless Slice D truly needs one elected
poller.

There is **no leader vs follower election** in these stores. Every process is a
peer. Exclusive flock serializes writers (and, for rate limits, readers too).
The OS releases flock when the fd closes (process exit or crash). The next
opener becomes the writer. No PID stamp, no unlink-to-break, no stale-lock
TTL.

---

## 1. Files, names, lock kind, TTL, kill-switch

| Store | Paths | Types / functions | Lock | Stale / TTL | Kill-switch |
|-------|--------|-------------------|------|-------------|-------------|
| **Rate limits** | `$GROK_HOME/rate_limits/{provider}.json` (no sibling `.lock`) | `SharedRateLimitStore`, `ProviderKey`, `RateLimitSnapshot`, `RateLimitMeta`, `StoredRecord`, `DISABLE_ENV`, `shared_rate_limits_disabled`, `fingerprint_secret` | **Exclusive only** (`fs2::FileExt::lock_exclusive`) on the JSON file itself for both `snapshot` and `observe` | **No lock TTL.** Cooldown is `not_before_unix_ms`; `remaining()` is zero when `now >= not_before`. Files stay on disk. | **`GROK_DISABLE_SHARED_RATE_LIMIT`** (any value). `shared_rate_limits_disabled()` is `var_os(...).is_some()`. |
| **Included poll history** | `$GROK_HOME/included_poll_history/{identity}.json` (`DURABLE_SUBDIR = "included_poll_history"`) | `IncludedPollHistoryStore`, `IncludedPollSample`, `DurableFile`, `DurableSample`, `record` / `history_for` / `record_included_poll_sample` | **Exclusive write**, **shared read** (`lock_shared` in `load_ring_at_path`) | **No lock TTL.** Ring cap **32**. Detector window is `DEFAULT_MIN_POLLS = 2` and `DEFAULT_MIN_WINDOW = 30s` (evidence, not lock expiry). | **None.** |
| **Active sessions** (sibling-lock variant) | `$GROK_HOME/active_sessions.json`, `active_sessions.lock`, `active_sessions.json.tmp` | `ActiveSession`, `register` / `try_unregister` / `collect_crashed` | Exclusive on **sibling** lock file. Mutate JSON under lock; `list_in` is an **unlocked** read. `try_unregister` uses `try_lock_exclusive` (WouldBlock → skip). | **No flock TTL.** Stale rows = **dead PID** via `is_pid_alive` + `collect_crashed`. | **None.** |

Home resolve (both peer stores): `GROK_HOME` if set, else `~/.grok`.

Related but **not** the flock pattern: `$GROK_HOME/exhausted_credits/{fingerprint}.json` (`DEFAULT_TTL` = 1 hour). Temp+rename, **no flock**. Do not treat that as the Slice D hub template.

TUI election (different job): `crates/codegen/xai-grok-shell/src/leader/lock.rs` `LeaderLock` holds exclusive flock for the process lifetime, writes PID, clients `try_lock`. That is leader vs follower. The snapshot hub should stay peer-JSON unless a single poller is a named requirement.

---

## 2. How a process becomes “leader” vs “follower”

**It does not.** These stores are peer RMW.

- Any process may `open` / `process_default` and write.
- Writer: `lock_exclusive` → read JSON → merge → rewrite → `unlock` / drop fd.
- Rate-limit readers also take exclusive (simple, no shared path).
- Poll-history readers take **shared** so many cold CLIs can load while one writer appends.
- Merge rules: rate limits `not_before = max(existing, now + wait)` (strictest wins). Poll history append + cap 32 (oldest dropped).

If Slice D wants one poller and many readers, that election is **new**. Closest existing election is `LeaderLock`, not these JSON stores.

---

## 3. Dead process / flock release

- Locks are **advisory `flock`** via `fs2` (`FileExt`).
- Unlock is best-effort `file.unlock()`; drop of `File` also releases.
- **Crash / kill:** kernel releases flock with the last fd. No breaker needed. Next `lock_exclusive` / `lock_shared` proceeds.
- **No** PID-in-file, **no** mtime stale break, **no** unlink-to-recreate (those live in auth `manager/lock.rs`, not here).
- Corrupt / empty JSON: treat as empty (`serde` fail → `None` / empty ring). Active sessions warn and start empty.
- Active sessions: dead **data** is PID liveness, not flock stale. `try_unregister` must not block on a hung peer.

---

## 4. What is stored (no secrets)

**Rate limits** (`StoredRecord` / `RateLimitSnapshot`): `not_before_unix_ms`, optional `last_status`, optional `last_reason`, `updated_at_unix_ms`. Key is sanitized host, optional 16-char **fingerprint** (FNV-1a hex of the secret, not the secret), optional API class. `ProviderKey::new` keeps `[A-Za-z0-9-_.]` only.

**Poll history** (`DurableFile`): `identity_id` (label; filename sanitized, max 120 chars) plus samples: `ts_unix_ms`, `credit_usage_percent`, optional `build_usage_percent`, optional `prepaid_balance_cents`. Module docs: never tokens or secrets. Test `two_store_handles_share_poll_samples` forbids `token` / `bearer` / `sk-` in the file body.

**Active sessions:** `session_id`, `pid`, `cwd`, `opened_at`. No credentials.

---

## 5. Code sites (acquire, write, read, stale, kill-switch)

### Kill-switch (rate limits only)

```14:20:crates/codegen/grok-rate-limit/src/store.rs
/// Env kill-switch: when set (any value), shared coordination is disabled.
pub const DISABLE_ENV: &str = "GROK_DISABLE_SHARED_RATE_LIMIT";

/// Whether shared rate limits are disabled for this process.
pub fn shared_rate_limits_disabled() -> bool {
    std::env::var_os(DISABLE_ENV).is_some()
}
```

Guards: `snapshot` / `remaining` / `observe` / `wait_if_limited` return `None` / `ZERO` / `Ok(())` when set. Public crate docs: `crates/codegen/grok-rate-limit/src/lib.rs` line 12.

### Rate-limit exclusive acquire + read

```296:313:crates/codegen/grok-rate-limit/src/store.rs
    fn with_locked<R>(
        &self,
        key: &ProviderKey,
        f: impl FnOnce(Option<StoredRecord>) -> R,
    ) -> std::io::Result<R> {
        let path = self.path_for(key);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.lock_exclusive()?;
        let rec = read_record(&mut file)?;
        let out = f(rec);
        let _ = file.unlock();
        Ok(out)
    }
```

JSON path: `root.join(format!("{}.json", key.as_str()))` (`path_for`, lines 180–182). `open` uses `grok_home/rate_limits`.

### Rate-limit exclusive write + “stale” (cooldown, not lock TTL)

```315:332:crates/codegen/grok-rate-limit/src/store.rs
    fn with_locked_mut(
        &self,
        key: &ProviderKey,
        f: impl FnOnce(&mut Option<StoredRecord>),
    ) -> std::io::Result<()> {
        let path = self.path_for(key);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.lock_exclusive()?;
        let mut rec = read_record(&mut file)?;
        f(&mut rec);
        write_record(&mut file, rec.as_ref())?;
        let _ = file.unlock();
        Ok(())
    }
```

```336:356:crates/codegen/grok-rate-limit/src/store.rs
fn read_record(file: &mut File) -> std::io::Result<Option<StoredRecord>> {
    file.seek(SeekFrom::Start(0))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    if buf.trim().is_empty() {
        return Ok(None);
    }
    Ok(serde_json::from_str(&buf).ok())
}

fn write_record(file: &mut File, rec: Option<&StoredRecord>) -> std::io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    if let Some(rec) = rec {
        let data = serde_json::to_vec_pretty(rec)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        file.write_all(&data)?;
        file.sync_all()?;
    }
    Ok(())
}
```

Observe merge (strictest `not_before`): `store.rs` 246–273. Snapshot remaining: `RateLimitSnapshot::remaining` / `is_active` (lines 123–134). Process cache is `not_before` only; full metadata always re-reads under flock.

### Poll history exclusive write + shared read

```272:289:crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.lock_exclusive()?;
        let mut ring = read_ring_from_file(&mut file, id);
        // ... append, cap RING_CAP ...
        write_ring_to_file(&mut file, id, &ring)?;
        let _ = file.unlock();
```

```358:369:crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs
fn load_ring_at_path(path: &Path, identity_id: &str) -> Option<VecDeque<IncludedPollSample>> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open(path)
        .ok()?;
    // Shared lock for multi-reader safety with exclusive writers.
    file.lock_shared().ok()?;
    let ring = read_ring_from_file(&mut file, identity_id);
    let _ = file.unlock();
    Some(ring)
}
```

Write body: seek 0, `set_len(0)`, pretty JSON, `sync_all` (`write_ring_to_file`, 340–356).

### Active sessions sibling lock (optional contrast)

```89:104:crates/codegen/xai-grok-active-sessions/src/lib.rs
fn with_locked_state<F, R>(root: &Path, mutate: F) -> io::Result<R>
// ...
    let lock_file = open_lock_file(&lock_path)?;
    lock_file.lock_exclusive()?;
    let result = locked_mutate(&data_path, &tmp_path, mutate);
    let _ = lock_file.unlock();
```

Atomic JSON: write `active_sessions.json.tmp` then `rename` (168–178).

---

## 6. How tests isolate this

**Rate limits** (`store.rs` `#[cfg(test)]`):

- `tempfile::TempDir` + `SharedRateLimitStore::open(dir.path())` (not live `~/.grok`).
- Two handles on the same temp root simulate two processes (`two_store_handles_share_file_state`).
- `ENV_LOCK` mutex serializes tests that touch `GROK_DISABLE_SHARED_RATE_LIMIT`.
- `with_shared_limits_enabled` clears the env for the test body, then restores.
- `disable_env_makes_ops_noop` sets `DISABLE_ENV` to `"1"` and asserts observe is a no-op (no remaining, no snapshot).

**Sampler / shell wrappers:** `xai-grok-shell/src/shared_http_rate_limit.rs` has thread-local `TEST_STORE_OVERRIDE` + `override_shared_store_for_test` so product callers do not hit `OnceLock` + real `GROK_HOME`.

**Poll history:**

- `with_history_lock` (`included_poll_history.rs` 669–686): process-wide `Mutex`, `TempDir`, `EnvGuard::set("GROK_HOME", dir.path())`, `clear_included_poll_history` before and after.
- `clear_process_included_poll_history_only` for cold-process tests (disk remains).
- Multi-handle: `IncludedPollHistoryStore::open(temp)` × 2 (`two_store_handles_share_poll_samples`).
- `EnvGuard`: `crates/codegen/xai-grok-test-support/src/env.rs` (restore prior value on drop).

**Active sessions:** `register_in` / `list_in` / `collect_crashed_in(TempDir)` injectable root. No env kill-switch.

---

## Slice D copy checklist

1. Subdir under `$GROK_HOME` (e.g. `limits_snapshot/` or similar plain name). One JSON per identity or one hub file. Sanitize names like `ProviderKey` / `safe_identity_filename`.
2. Lock **on the JSON file** with `fs2::FileExt` (rate-limit / poll history). Sibling `.lock` + tmp+rename only if you need `list` without locking (active sessions).
3. Exclusive for write. Prefer **shared** for read if many followers only consume a snapshot.
4. Seek 0 / truncate / pretty JSON / `sync_all`. Empty or bad JSON = empty snapshot.
5. Store **meters only**: included SuperGrok period used %, SuperGrok dollar credits cents, timestamps, identity labels, fingerprints. Never tokens, bearers, or raw keys.
6. Optional `GROK_DISABLE_…` modeled on `GROK_DISABLE_SHARED_RATE_LIMIT` (any value disables).
7. Tests: temp `GROK_HOME` or `Store::open(temp)`, mutex if the env is process-global, two handles share disk, kill-switch no-op.
8. Do **not** add leader election, PID lock-break, or a lock TTL unless Slice D names that. Snapshot **freshness** can be a field (`updated_at_unix_ms`) plus a reader-side max-age. That is data TTL, not flock stale recovery.
9. Dead writer: rely on kernel flock release. Readers should tolerate a missing or empty file.

Primary sources: `crates/codegen/grok-rate-limit/src/{lib.rs,store.rs}`, `crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs`, `crates/codegen/xai-grok-active-sessions/src/lib.rs`, `FORK.md` § Multi-session rate limits.
