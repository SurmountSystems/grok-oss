//! Tracks open TUI sessions in `~/.grok/active_sessions.json`. A clean exit removes the entry,
//! a crash leaves it behind, and the next [`register`] prunes entries whose PID is dead.

#![deny(clippy::indexing_slicing)]

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_client_protocol as acp;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// What the TUI was doing when it last wrote a heartbeat. Unknown is the
/// registry default so an older row without the field still parses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionActivity {
    Working,
    Idle,
    #[default]
    Unknown,
}

impl SessionActivity {
    fn is_unknown(activity: &Self) -> bool {
        matches!(activity, Self::Unknown)
    }
}

/// Short status phrase for a safe activity line. Not prompt text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatPhrase {
    Paused,
    TurnRunning,
    Idle,
}

/// Fields a heartbeat may refresh on an existing registry row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatUpdate {
    pub activity: SessionActivity,
    pub title: Option<String>,
    pub activity_line: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveSession {
    pub session_id: acp::SessionId,
    pub pid: u32,
    pub cwd: String,
    pub opened_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "SessionActivity::is_unknown")]
    pub activity: SessionActivity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_line: Option<String>,
}

impl ActiveSession {
    pub fn new(
        session_id: acp::SessionId,
        pid: u32,
        cwd: impl Into<String>,
        opened_at: DateTime<Utc>,
    ) -> Self {
        Self {
            session_id,
            pid,
            cwd: cwd.into(),
            opened_at,
            updated_at: None,
            activity: SessionActivity::Unknown,
            title: None,
            activity_line: None,
        }
    }
}

const DATA_FILENAME: &str = "active_sessions.json";
const LOCK_FILENAME: &str = "active_sessions.lock";
const TMP_FILENAME: &str = "active_sessions.json.tmp";

/// On an NFS home `flock` is a network-lock-manager lock with no lease: a holder killed at the
/// wrong moment strands it forever, so never wait unbounded. A live holder only does one small
/// read-modify-write, so anything held this long is stranded.
const LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(2);
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Register a session as active (idempotent by session_id) and prune entries whose PID is dead.
/// Fails with [`io::ErrorKind::TimedOut`] if the lock is still held after `LOCK_ACQUIRE_TIMEOUT`.
pub fn register(session: ActiveSession) -> io::Result<()> {
    register_in(&xai_grok_config::grok_home(), session)
}

/// Non-blocking unregister for signal handlers.
/// Returns `Ok(false)` on lock contention; the orphan is pruned by the next `register`.
pub fn try_unregister(session_id: &acp::SessionId) -> io::Result<bool> {
    try_unregister_in(&xai_grok_config::grok_home(), session_id)
}

pub fn register_in(root: &Path, session: ActiveSession) -> io::Result<()> {
    with_locked_state(root, LOCK_ACQUIRE_TIMEOUT, |sessions| {
        sessions.retain(|s| s.session_id != session.session_id && is_pid_alive(s.pid));
        sessions.push(session);
    })
}

pub fn try_unregister_in(root: &Path, session_id: &acp::SessionId) -> io::Result<bool> {
    let outcome = with_locked_state(root, Duration::ZERO, |sessions| {
        sessions.retain(|s| s.session_id != *session_id);
    });
    match outcome {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn list_in(root: &Path) -> io::Result<Vec<ActiveSession>> {
    let data_path = root.join(DATA_FILENAME);
    read_data_file(&data_path)
}

/// Registry rows under the default Grok home, including dead PIDs.
pub fn list() -> io::Result<Vec<ActiveSession>> {
    list_in(&xai_grok_config::grok_home())
}

/// Registry rows under `root` whose PID is still alive.
pub fn list_live_in(root: &Path) -> io::Result<Vec<ActiveSession>> {
    let mut sessions = list_in(root)?;
    sessions.retain(|session| is_pid_alive(session.pid));
    Ok(sessions)
}

/// Safe one-line status. Model id and a count only. Never prompt text.
pub fn format_safe_activity_line(
    model: Option<&str>,
    phrase: HeartbeatPhrase,
    subagent_count: u32,
) -> Option<String> {
    let phrase = match phrase {
        HeartbeatPhrase::Paused => "paused",
        HeartbeatPhrase::TurnRunning => "turn running",
        HeartbeatPhrase::Idle => "idle",
    };
    let mut line = phrase.to_string();
    if let Some(model) = model.map(str::trim).filter(|text| !text.is_empty()) {
        line.push_str(" · ");
        line.push_str(model);
    }
    if subagent_count > 0 {
        line.push_str(" · ");
        line.push_str(&subagent_count.to_string());
        line.push_str(if subagent_count == 1 {
            " subagent"
        } else {
            " subagents"
        });
    }
    Some(line)
}

fn apply_heartbeat(
    sessions: &mut [ActiveSession],
    session_id: &acp::SessionId,
    update: &HeartbeatUpdate,
) -> bool {
    let mut found = false;
    for session in sessions.iter_mut() {
        if session.session_id != *session_id {
            continue;
        }
        found = true;
        session.activity = update.activity;
        session.title = update.title.clone();
        session.activity_line = update.activity_line.clone();
        session.updated_at = Some(Utc::now());
    }
    found
}

/// Refresh activity on the row `register` already wrote for this session.
pub fn heartbeat(
    _pid: u32,
    session_id: &acp::SessionId,
    update: HeartbeatUpdate,
) -> io::Result<()> {
    let found = with_locked_state(
        &xai_grok_config::grok_home(),
        LOCK_ACQUIRE_TIMEOUT,
        |sessions| apply_heartbeat(sessions, session_id, &update),
    )?;
    if found {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no matching active session",
        ))
    }
}

/// Like [`heartbeat`], but lock contention is `Ok(None)` and a missing row is `Ok(Some(false))`.
pub fn try_heartbeat(
    _pid: u32,
    session_id: &acp::SessionId,
    update: HeartbeatUpdate,
) -> io::Result<Option<bool>> {
    match with_locked_state(&xai_grok_config::grok_home(), Duration::ZERO, |sessions| {
        apply_heartbeat(sessions, session_id, &update)
    }) {
        Ok(found) => Ok(Some(found)),
        Err(err) if err.kind() == io::ErrorKind::TimedOut => Ok(None),
        Err(err) => Err(err),
    }
}

/// Drop registry rows whose PID is dead. Returns how many rows were removed.
pub fn collect_crashed() -> io::Result<usize> {
    with_locked_state(
        &xai_grok_config::grok_home(),
        LOCK_ACQUIRE_TIMEOUT,
        |sessions| {
            let before = sessions.len();
            sessions.retain(|session| is_pid_alive(session.pid));
            before.saturating_sub(sessions.len())
        },
    )
}

fn with_locked_state<F, R>(root: &Path, timeout: Duration, mutate: F) -> io::Result<R>
where
    F: FnOnce(&mut Vec<ActiveSession>) -> R,
{
    let lock_path = root.join(LOCK_FILENAME);
    let data_path = root.join(DATA_FILENAME);
    let tmp_path = root.join(TMP_FILENAME);

    fs::create_dir_all(root)?;
    // Closing the file releases the lock, so every return path unlocks by dropping it.
    let lock_file = open_lock_file(&lock_path)?;
    let deadline = Instant::now() + timeout;
    loop {
        match lock_file.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Error(e)) => return Err(e),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("active-session registry lock still held after {timeout:?}"),
            ));
        }
        std::thread::sleep(LOCK_POLL_INTERVAL.min(remaining));
    }

    let mut sessions = read_data_file(&data_path)?;
    let result = mutate(&mut sessions);
    write_data_file_atomic(&tmp_path, &data_path, &sessions)?;
    Ok(result)
}

fn open_lock_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

fn read_data_file(path: &Path) -> io::Result<Vec<ActiveSession>> {
    match fs::read(path) {
        Ok(bytes) if bytes.is_empty() => Ok(Vec::new()),
        Ok(bytes) => match serde_json::from_slice::<Vec<ActiveSession>>(&bytes) {
            Ok(sessions) => Ok(sessions),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "active_sessions.json is corrupted, starting with empty list"
                );
                Ok(Vec::new())
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

fn write_data_file_atomic(
    tmp_path: &Path,
    data_path: &Path,
    sessions: &[ActiveSession],
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(sessions)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(tmp_path, json.as_bytes())?;
    fs::rename(tmp_path, data_path).inspect_err(|_| {
        let _ = fs::remove_file(tmp_path);
    })
}

/// Whether `pid` appears alive on this host (for inventory / crash hygiene).
pub fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let pid_i = match i32::try_from(pid) {
            Ok(p) if p > 0 => p,
            _ => return false,
        };
        let ret = unsafe { libc::kill(pid_i as libc::pid_t, 0) };
        if ret == 0 {
            return true;
        }
        io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
        match handle {
            Ok(h) => {
                let _ = unsafe { CloseHandle(h) };
                true
            }
            Err(_) => false,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        // Conservative: assume alive if we can't check.
        true
    }
}

/// Install artifact and process basename for this product. Not stock `grok`.
pub const PRODUCT_CLI_NAME: &str = "grok-oss";

/// Stock process named `grok` is not grok-oss. Rebuild `SIGUSR1` must not target it.
///
/// When `exe` is present it is authoritative: the basename must be [`PRODUCT_CLI_NAME`].
/// Linux appends ` (deleted)` after unlink; that suffix is not part of the name.
/// A cargo binary whose name only contains `grok` is not this CLI. With no exe,
/// the first cmdline (`NUL`-separated) or comm token is classified the same way.
pub fn is_grok_oss_cli_identity(cmdline_or_comm: &str, exe: Option<&str>) -> bool {
    if let Some(exe) = exe.map(str::trim).filter(|exe| !exe.is_empty()) {
        return is_product_cli_token(exe);
    }
    first_identity_token(cmdline_or_comm).is_some_and(is_product_cli_token)
}

fn is_product_cli_token(path_or_name: &str) -> bool {
    let trimmed = path_or_name.trim();
    let without_deleted = trimmed.strip_suffix(" (deleted)").unwrap_or(trimmed);
    let basename = without_deleted
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(without_deleted);
    basename == PRODUCT_CLI_NAME
}

fn first_identity_token(cmdline_or_comm: &str) -> Option<&str> {
    let has_nul = cmdline_or_comm.contains('\0');
    cmdline_or_comm
        .split(|c: char| {
            if has_nul {
                c == '\0'
            } else {
                c.is_whitespace()
            }
        })
        .map(str::trim)
        .find(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_session(id: &str, pid: u32) -> ActiveSession {
        ActiveSession::new(acp::SessionId::new(id), pid, "/tmp/test", Utc::now())
    }

    /// Contract: stock process named `grok` is not grok-oss. Rebuild SIGUSR1
    /// must not target it. Product CLI / exe basename is `grok-oss`.
    #[test]
    fn grok_oss_cli_identity_rejects_stock_grok_and_accepts_product_exe() {
        assert!(
            !is_grok_oss_cli_identity("grok", Some("/usr/bin/grok")),
            "stock grok comm must not look like grok-oss"
        );
        assert!(!is_grok_oss_cli_identity(
            "/usr/bin/grok\0--resume\0sess",
            Some("/usr/bin/grok")
        ));
        assert!(
            !is_grok_oss_cli_identity("xai-grok-update-abc123", None),
            "a cargo test binary whose path contains grok is not grok-oss"
        );
        assert!(is_grok_oss_cli_identity(
            "/home/me/.cargo/bin/grok-oss\0--resume\0sess",
            Some("/home/me/.cargo/bin/grok-oss")
        ));
        assert!(is_grok_oss_cli_identity(
            "grok-oss",
            Some("/home/me/.cargo/bin/grok-oss (deleted)")
        ));
        assert_eq!(PRODUCT_CLI_NAME, "grok-oss");
        assert_ne!(PRODUCT_CLI_NAME, "grok");
    }

    #[test]
    fn register_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let s = make_session("s1", std::process::id());
        register_in(dir.path(), s.clone()).unwrap();
        register_in(dir.path(), s).unwrap();
        assert_eq!(1, list_in(dir.path()).unwrap().len());
    }

    #[test]
    fn concurrent_registers_no_corruption() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        std::thread::scope(|s| {
            for i in 0..10 {
                let p = path.clone();
                s.spawn(move || {
                    register_in(&p, make_session(&format!("s{i}"), std::process::id())).unwrap()
                });
            }
        });
        assert_eq!(10, list_in(dir.path()).unwrap().len());
    }

    #[test]
    fn try_unregister_skips_if_locked() {
        let dir = TempDir::new().unwrap();
        let s = make_session("s1", std::process::id());
        register_in(dir.path(), s.clone()).unwrap();

        let lock_file = open_lock_file(&dir.path().join(LOCK_FILENAME)).unwrap();
        lock_file.lock().unwrap();
        assert!(!try_unregister_in(dir.path(), &s.session_id).unwrap());
        lock_file.unlock().unwrap();
        assert_eq!(1, list_in(dir.path()).unwrap().len());
    }

    #[test]
    fn locked_update_times_out_when_lock_stays_held() {
        let dir = TempDir::new().unwrap();
        register_in(dir.path(), make_session("s1", std::process::id())).unwrap();
        let holder = open_lock_file(&dir.path().join(LOCK_FILENAME)).unwrap();
        holder.lock().unwrap();

        let outcome = with_locked_state(dir.path(), Duration::from_millis(50), Vec::clear);

        assert_eq!(io::ErrorKind::TimedOut, outcome.unwrap_err().kind());
        assert_eq!(1, list_in(dir.path()).unwrap().len());
    }

    #[test]
    fn locked_update_retries_until_holder_releases() {
        let dir = TempDir::new().unwrap();
        let holder = open_lock_file(&dir.path().join(LOCK_FILENAME)).unwrap();
        holder.lock().unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            holder.unlock().unwrap();
        });

        with_locked_state(dir.path(), LOCK_ACQUIRE_TIMEOUT, |sessions| {
            sessions.push(make_session("s1", std::process::id()));
        })
        .unwrap();
        release.join().unwrap();
        assert_eq!(1, list_in(dir.path()).unwrap().len());
    }

    #[test]
    fn corrupt_file_recovers() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(DATA_FILENAME), "garbage{{{").unwrap();
        assert!(list_in(dir.path()).unwrap().is_empty());
        register_in(dir.path(), make_session("s1", std::process::id())).unwrap();
        assert_eq!(1, list_in(dir.path()).unwrap().len());
    }
}
