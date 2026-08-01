//! In-process bulk-edit policy for `search_replace` (host C3 port).
//!
//! Complements host PreToolUse hooks (`block-bulk-replace-edit.py`). Product
//! path runs **before** the edit is applied so always-approve sessions still
//! get a hard stop without depending on hook fail-open.
//!
//! Policy (defaults match host):
//! - Optional `GROK_DENY_REPLACE_ALL=1` denies any `replace_all: true`.
//! - Identical old→new across ≥ N distinct paths within T seconds in the same
//!   session → deny further (defaults N=5, T=120).
//!
//! State: `~/.grok/bulk-edit-state/` (override `GROK_BULK_EDIT_DIR`).
//! Fail-open on lock/IO errors (same as host hook policy).

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// Env: deny any `replace_all: true` when set to `"1"`.
pub const ENV_DENY_REPLACE_ALL: &str = "GROK_DENY_REPLACE_ALL";
/// Env: storm threshold (distinct paths). Default 5.
pub const ENV_BULK_EDIT_N: &str = "GROK_BULK_EDIT_N";
/// Env: storm window seconds. Default 120.
pub const ENV_BULK_EDIT_T: &str = "GROK_BULK_EDIT_T";
/// Env: override state directory (default `~/.grok/bulk-edit-state`).
pub const ENV_BULK_EDIT_DIR: &str = "GROK_BULK_EDIT_DIR";

/// Inputs evaluated by the policy (tool-params subset).
#[derive(Debug, Clone)]
pub struct BulkEditRequest<'a> {
    /// Session id for storm isolation. **`None` or empty skips storm tracking**
    /// (fail-open isolation — do not collapse unrelated sessions onto `"unknown"`).
    /// `replace_all` deny still applies when env is set.
    pub session_id: Option<&'a str>,
    pub file_path: &'a str,
    pub old_string: &'a str,
    pub new_string: &'a str,
    pub replace_all: bool,
}

/// Deny reason for the model / tool error surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkEditDeny {
    pub reason: String,
}

/// Evaluate bulk-edit policy. Returns `Some(deny)` to block, `None` to allow.
///
/// Fail-open: any unexpected I/O or parse error allows the edit.
pub fn evaluate(req: &BulkEditRequest<'_>) -> Option<BulkEditDeny> {
    evaluate_with_clock(req, now_secs())
}

/// Test seam with injectable clock (unix seconds).
pub fn evaluate_with_clock(req: &BulkEditRequest<'_>, now: f64) -> Option<BulkEditDeny> {
    if req.replace_all && env_flag_true(ENV_DENY_REPLACE_ALL) {
        return Some(BulkEditDeny {
            reason: "Blocked: replace_all is disabled by host policy. \
                     Use unique old_string context for a single site after reading the file."
                .to_owned(),
        });
    }

    let n = env_usize(ENV_BULK_EDIT_N, 5);
    if n == 0 || req.old_string.is_empty() || req.file_path.is_empty() {
        return None;
    }

    // No real session id → skip storm (avoid shared "unknown" bucket).
    let session = req.session_id.map(str::trim).filter(|s| !s.is_empty());
    let Some(session) = session else {
        tracing::debug!(
            "bulk_edit_policy: no OwnerSessionId; skip storm record (replace_all still gated)"
        );
        return None;
    };

    let t_win = env_usize(ENV_BULK_EDIT_T, 120) as f64;
    let state_root = state_dir();
    let pair = pair_hash(req.old_string, req.new_string);
    let safe_session = sanitize_session(session);
    let state_file = state_root.join(format!("{safe_session}-{pair}.jsonl"));

    match record_and_count_paths(&state_file, &state_root, req.file_path, now, t_win) {
        Ok(path_count) if path_count >= n => Some(BulkEditDeny {
            reason: format!(
                "Blocked: bulk multi-file replace storm \
                 ({path_count} files with the same old→new in {t_win:.0}s; limit {n}). \
                 Read each site and use unique surgical context, or ask the user for an \
                 explicit bulk rename. Do not retry with another bulk tool."
            ),
        }),
        Ok(_) => None,
        // Fail-open on I/O.
        Err(_) => None,
    }
}

fn env_flag_true(name: &str) -> bool {
    std::env::var(name).ok().as_deref() == Some("1")
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn pair_hash(old: &str, new: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(old.as_bytes());
    hasher.update([0]);
    hasher.update(new.as_bytes());
    let digest = hasher.finalize();
    // 16 hex chars = first 8 bytes (host uses [:16] of hex digest).
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn sanitize_session(session: &str) -> String {
    session
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect()
}

fn state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(ENV_BULK_EDIT_DIR) {
        return PathBuf::from(dir);
    }
    crate::util::grok_home().join("bulk-edit-state")
}

fn record_and_count_paths(
    state_file: &Path,
    state_root: &Path,
    path: &str,
    now: f64,
    t_win: f64,
) -> std::io::Result<usize> {
    std::fs::create_dir_all(state_root)?;

    // Occasional prune of abandoned pair files (best-effort).
    if path_hash_mod8(state_file) == 0 {
        prune_stale_state_files(state_root, t_win.max(3600.0));
    }

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(state_file)?;

    // Best-effort exclusive flock (host parity). `_lock_ok` is `bool`, not a
    // RAII guard — `FileExt::lock_exclusive` returns `Result<()>`.
    use fs2::FileExt;
    let lock_ok = file.lock_exclusive().is_ok();
    let result = rewrite_window(&mut file, path, now, t_win);
    if lock_ok {
        let _ = file.unlock();
    }
    result
}

fn rewrite_window(
    file: &mut std::fs::File,
    path: &str,
    now: f64,
    t_win: f64,
) -> std::io::Result<usize> {
    file.seek(SeekFrom::Start(0))?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)?;

    let mut paths: HashSet<String> = HashSet::new();
    let mut kept: Vec<String> = Vec::new();
    for line in raw.lines() {
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let ts = ev.get("ts").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if now - ts <= t_win {
            kept.push(line.to_owned());
            if let Some(p) = ev.get("path").and_then(|v| v.as_str()) {
                paths.insert(p.to_owned());
            }
        }
    }
    let new_line = serde_json::json!({ "ts": now, "path": path }).to_string();
    kept.push(new_line);
    paths.insert(path.to_owned());

    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    if !kept.is_empty() {
        file.write_all(kept.join("\n").as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
    }
    Ok(paths.len())
}

fn prune_stale_state_files(state_root: &Path, max_age: f64) {
    let Ok(entries) = std::fs::read_dir(state_root) else {
        return;
    };
    let now = now_secs();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.contains('-') || !name.ends_with(".jsonl") {
            continue;
        }
        if let Ok(meta) = entry.metadata()
            && let Ok(mtime) = meta.modified()
            && let Ok(dur) = mtime.duration_since(UNIX_EPOCH)
            && now - dur.as_secs_f64() > max_age
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn path_hash_mod8(path: &Path) -> u8 {
    let s = path.to_string_lossy();
    s.bytes().fold(0u8, |a, b| a.wrapping_add(b)) % 8
}

/// Test helpers for process-global bulk-edit env mutation.
///
/// All tests that set `GROK_DENY_REPLACE_ALL` / `GROK_BULK_EDIT_*` must take
/// [`test_env::ENV_LOCK`] so unit and `search_replace` integration tests do
/// not race or leak env into unrelated cases (e.g. `crlf_replace_all`).
#[cfg(test)]
pub mod test_env {
    use std::sync::Mutex;

    /// Serialize env-mutating bulk-edit tests across the crate.
    pub static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Sets env pairs; removes those keys on drop (panic-safe cleanup).
    pub struct EnvGuard {
        keys: Vec<&'static str>,
    }

    impl EnvGuard {
        pub fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
            let keys: Vec<_> = pairs.iter().map(|(k, _)| *k).collect();
            for (k, v) in pairs {
                match v {
                    Some(val) => unsafe { std::env::set_var(k, val) },
                    None => unsafe { std::env::remove_var(k) },
                }
            }
            Self { keys }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for k in &self.keys {
                unsafe { std::env::remove_var(k) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_env::{ENV_LOCK, EnvGuard};
    use super::*;

    fn tmp_state() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn diverse_edits_allow() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tmp_state();
        let _env = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_BULK_EDIT_N, Some("5")),
            (ENV_BULK_EDIT_T, Some("120")),
            (ENV_DENY_REPLACE_ALL, None),
        ]);
        for i in 0..6 {
            let r = evaluate(&BulkEditRequest {
                session_id: Some("s1"),
                file_path: &format!("/tmp/f{i}.rs"),
                old_string: &format!("unique_context_{i}"),
                new_string: &format!("new_{i}"),
                replace_all: false,
            });
            assert!(r.is_none(), "diverse edit {i} should allow: {r:?}");
        }
    }

    #[test]
    fn same_old_new_storm_denies_at_n() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tmp_state();
        let _env = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_BULK_EDIT_N, Some("5")),
            (ENV_BULK_EDIT_T, Some("120")),
            (ENV_DENY_REPLACE_ALL, None),
        ]);
        let mut reasons = Vec::new();
        for i in 0..5 {
            let r = evaluate(&BulkEditRequest {
                session_id: Some("s2"),
                file_path: &format!("/tmp/same{i}.rs"),
                old_string: "FOO_RENAME",
                new_string: "BAR_RENAME",
                replace_all: false,
            });
            reasons.push(r);
        }
        assert!(
            reasons[..4].iter().all(|x| x.is_none()),
            "paths 1-4 allow: {reasons:?}"
        );
        let fifth = reasons[4].as_ref().expect("5th should deny");
        assert!(
            fifth.reason.contains("storm"),
            "expected storm deny: {}",
            fifth.reason
        );
    }

    #[test]
    fn replace_all_denied_when_env_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tmp_state();
        let _env = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_DENY_REPLACE_ALL, Some("1")),
            (ENV_BULK_EDIT_N, Some("5")),
        ]);
        let r = evaluate(&BulkEditRequest {
            session_id: Some("s3"),
            file_path: "/tmp/x.rs",
            old_string: "a",
            new_string: "b",
            replace_all: true,
        });
        let deny = r.expect("replace_all should deny");
        assert!(
            deny.reason.contains("replace_all"),
            "reason: {}",
            deny.reason
        );
    }

    #[test]
    fn replace_all_allowed_when_env_unset() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tmp_state();
        let _env = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_DENY_REPLACE_ALL, None),
            (ENV_BULK_EDIT_N, Some("50")),
        ]);
        let r = evaluate(&BulkEditRequest {
            session_id: Some("s4"),
            file_path: "/tmp/x.rs",
            old_string: "a",
            new_string: "b",
            replace_all: true,
        });
        assert!(r.is_none(), "replace_all default allows: {r:?}");
    }

    #[test]
    fn empty_or_missing_path_allows() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tmp_state();
        let _env = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_DENY_REPLACE_ALL, None),
        ]);
        assert!(
            evaluate(&BulkEditRequest {
                session_id: Some("s"),
                file_path: "",
                old_string: "a",
                new_string: "b",
                replace_all: false,
            })
            .is_none()
        );
        assert!(
            evaluate(&BulkEditRequest {
                session_id: Some("s"),
                file_path: "/tmp/x",
                old_string: "",
                new_string: "b",
                replace_all: false,
            })
            .is_none()
        );
    }

    #[test]
    fn missing_session_id_skips_storm_not_shared_bucket() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tmp_state();
        let _env = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_BULK_EDIT_N, Some("2")),
            (ENV_DENY_REPLACE_ALL, None),
        ]);
        for i in 0..5 {
            assert!(
                evaluate(&BulkEditRequest {
                    session_id: None,
                    file_path: &format!("/tmp/u{i}.rs"),
                    old_string: "SAME",
                    new_string: "X",
                    replace_all: false,
                })
                .is_none(),
                "missing session must not storm-deny"
            );
        }
        // replace_all deny still works without session id
        let _env2 = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_DENY_REPLACE_ALL, Some("1")),
        ]);
        let r = evaluate(&BulkEditRequest {
            session_id: None,
            file_path: "/tmp/x.rs",
            old_string: "a",
            new_string: "b",
            replace_all: true,
        });
        assert!(r.is_some() && r.unwrap().reason.contains("replace_all"));
    }

    #[test]
    fn n_zero_disables_storm() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tmp_state();
        let _env = EnvGuard::set(&[
            (ENV_BULK_EDIT_DIR, Some(tmp.path().to_str().unwrap())),
            (ENV_BULK_EDIT_N, Some("0")),
            (ENV_DENY_REPLACE_ALL, None),
        ]);
        for i in 0..10 {
            assert!(
                evaluate(&BulkEditRequest {
                    session_id: Some("s-n0"),
                    file_path: &format!("/tmp/z{i}.rs"),
                    old_string: "SAME",
                    new_string: "X",
                    replace_all: false,
                })
                .is_none()
            );
        }
    }
}
