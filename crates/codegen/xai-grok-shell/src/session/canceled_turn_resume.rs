//! Durable marker for an interrupted mid-turn so restart can resume it.
//!
//! Lives under `$GROK_HOME/sessions/<enc-cwd>/<session_id>/canceled_turn_resume.json`.
//! Not secrets. Written when a **cancel-resumable** mid-turn is known:
//! - **eagerly at turn start** (prompt drain / turn-start shim) so hard
//!   `killall` races still leave a file even if the async SIGTERM task never
//!   runs,
//! - on explicit user cancel (Esc / stop),
//! - on graceful process quit (SIGTERM / first signal → Quit / `/exit`),
//! - on `/rebuild` mid-turn cancel before re-exec,
//! - on fearless global pause when it cancels a running primary turn (the
//!   in-process pause gate stays in RAM; this file is the interrupted prompt),
//! - on session load **history recovery** when no marker exists but the
//!   loaded session still looks mid-work (unfinished subagents / running
//!   scrollback) and a last user prompt is available.
//!
//! Cleared on clean successful turn finish (and rate-limit terminals). Kept
//! on **error** terminals so reopen continues failed work. Not written for
//! network blips alone, global pause when nothing is mid-turn, soft stop, or
//! **SIGKILL** (`kill -9` — no userspace handler can run; eager write is the
//! only defense against total hard death). The pause chip does not persist
//! the RAM gate itself.
//!
//! Resume on session open is gated by `[ui] resume_canceled_turn_on_restart`
//! (default **on**). Order on load: (A) apply marker if present **and** the
//! load is not a stale-marker case (primary user turn finished **successfully**
//! in replay with no mid-work and no error terminal — drop the file, do not
//! re-fire), else (B) recover from mid-work interruption **or** last-turn
//! **error** + last user prompt. Clean successful turns must never invent a
//! resume; error-class failures must auto-resume. `/rebuild` relaunch uses
//! the same session load path.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const CANCELED_TURN_RESUME_FILE: &str = "canceled_turn_resume.json";

/// In-process arm for SIGTERM / hard-exit paths that cannot reach AppView.
///
/// The TUI publishes the active mid-turn prompt here when a turn starts so
/// `request_graceful_or_exit` / hard terminal restore can write the marker
/// even if the event loop never dispatches `Action::Quit` (wedged loop,
/// second signal force-exit). Cleared on idle / turn finish. Not a substitute
/// for the per-agent Quit path when multiple sessions are open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessShutdownResumeArm {
    pub cwd: String,
    pub session_id: String,
    pub prompt_text: String,
    pub prompt_id: Option<String>,
}

static PROCESS_SHUTDOWN_ARM: Mutex<Option<ProcessShutdownResumeArm>> = Mutex::new(None);

/// Publish (or replace) the process-level arm for external signal death.
pub fn arm_process_shutdown_cancel_resume(arm: ProcessShutdownResumeArm) {
    if arm.prompt_text.trim().is_empty() || sanitize_session_id(&arm.session_id).is_none() {
        return;
    }
    if let Ok(mut guard) = PROCESS_SHUTDOWN_ARM.lock() {
        *guard = Some(ProcessShutdownResumeArm {
            cwd: arm.cwd,
            session_id: arm.session_id,
            prompt_text: arm.prompt_text.trim().to_string(),
            prompt_id: arm
                .prompt_id
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        });
    }
}

/// Arm the process-level payload **and** write `canceled_turn_resume.json` now.
///
/// Call when a mid-turn prompt is known (drain / turn-start shim), not only on
/// SIGTERM. Hard `killall` races can kill the client before the async signal
/// task runs; an on-disk marker from turn start still resumes on next open.
/// Cleared on successful turn finish (same as Esc/SIGTERM markers).
pub fn arm_and_persist_process_shutdown_cancel_resume(arm: ProcessShutdownResumeArm) {
    if arm.prompt_text.trim().is_empty() || sanitize_session_id(&arm.session_id).is_none() {
        return;
    }
    let cwd = arm.cwd.clone();
    let session_id = arm.session_id.clone();
    let prompt_text = arm.prompt_text.trim().to_string();
    let prompt_id = arm
        .prompt_id
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    arm_process_shutdown_cancel_resume(ProcessShutdownResumeArm {
        cwd: cwd.clone(),
        session_id: session_id.clone(),
        prompt_text: prompt_text.clone(),
        prompt_id: prompt_id.clone(),
    });
    let now = chrono::Utc::now().to_rfc3339();
    let Some(marker) = build_user_cancel_marker(&prompt_text, prompt_id.as_deref(), now) else {
        return;
    };
    match write_canceled_turn_resume(&cwd, &session_id, &marker) {
        Ok(()) => {
            if let Some(path) = canceled_turn_resume_path(&cwd, &session_id) {
                tracing::info!(
                    path = %path.display(),
                    session = %session_id,
                    prompt_len = prompt_text.len(),
                    "canceled_turn_resume: marker written (eager active turn)"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                session = %session_id,
                "canceled_turn_resume: eager active-turn write failed"
            );
        }
    }
}

/// Drop the process-level arm (idle, turn finished, clean success).
pub fn clear_process_shutdown_cancel_resume() {
    if let Ok(mut guard) = PROCESS_SHUTDOWN_ARM.lock() {
        *guard = None;
    }
}

/// Snapshot of the current arm (tests / diagnostics).
pub fn process_shutdown_cancel_resume_arm() -> Option<ProcessShutdownResumeArm> {
    PROCESS_SHUTDOWN_ARM.lock().ok().and_then(|g| g.clone())
}

/// Write `canceled_turn_resume.json` from the armed payload if any.
/// Used by signal hard-exit and first-signal graceful notify so killall does
/// not depend solely on the event loop reaching `Action::Quit`.
pub fn write_armed_process_shutdown_cancel_resume() -> io::Result<bool> {
    let arm = match PROCESS_SHUTDOWN_ARM.lock() {
        Ok(g) => g.clone(),
        Err(_) => return Ok(false),
    };
    let Some(arm) = arm else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let Some(marker) = build_user_cancel_marker(&arm.prompt_text, arm.prompt_id.as_deref(), now)
    else {
        return Ok(false);
    };
    write_canceled_turn_resume(&arm.cwd, &arm.session_id, &marker)?;
    if let Some(path) = canceled_turn_resume_path(&arm.cwd, &arm.session_id) {
        tracing::info!(
            path = %path.display(),
            session = %arm.session_id,
            prompt_len = arm.prompt_text.len(),
            "canceled_turn_resume: marker written (process-shutdown arm)"
        );
    }
    Ok(true)
}

/// Why the turn was interrupted for durable resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelResumeReason {
    /// Operator explicitly canceled / stopped the turn.
    UserCancel,
}

/// One-shot resume payload written when the operator cancels mid-turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanceledTurnResume {
    /// Prompt text to re-queue once on session open (not empty).
    pub prompt_text: String,
    /// Wire prompt id when known (identity / dedupe only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    /// RFC 3339 UTC when cancel was recorded.
    pub canceled_at: String,
    pub reason: CancelResumeReason,
}

fn sanitize_session_id(session_id: &str) -> Option<&str> {
    let s = session_id.trim();
    if s.is_empty() || s.contains('/') || s.contains('\\') || s.contains("..") {
        return None;
    }
    Some(s)
}

/// Path for this session's cancel-resume marker.
pub fn canceled_turn_resume_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
    let sid = sanitize_session_id(session_id)?;
    Some(
        crate::util::grok_home::sessions_cwd_dir(cwd)
            .join(sid)
            .join(CANCELED_TURN_RESUME_FILE),
    )
}

/// Build a marker only for non-empty prompt text (never invent finished work).
pub fn build_user_cancel_marker(
    prompt_text: &str,
    prompt_id: Option<&str>,
    now_rfc3339: impl Into<String>,
) -> Option<CanceledTurnResume> {
    let text = prompt_text.trim();
    if text.is_empty() {
        return None;
    }
    Some(CanceledTurnResume {
        prompt_text: text.to_string(),
        prompt_id: prompt_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        canceled_at: now_rfc3339.into(),
        reason: CancelResumeReason::UserCancel,
    })
}

/// Write the marker atomically (mode 0600 on Unix). Empty / invalid → no-op clear.
pub fn write_canceled_turn_resume(
    cwd: &str,
    session_id: &str,
    marker: &CanceledTurnResume,
) -> io::Result<()> {
    let Some(path) = canceled_turn_resume_path(cwd, session_id) else {
        return Ok(());
    };
    if marker.prompt_text.trim().is_empty() {
        return clear_canceled_turn_resume(cwd, session_id);
    }
    let _ = crate::util::grok_home::ensure_sessions_cwd_dir(cwd)?;
    write_marker_at(&path, marker)
}

fn write_marker_at(path: &Path, marker: &CanceledTurnResume) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(marker)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("tmp");
    {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(&bytes)?;
        // Durable before rename so a kill mid-write cannot leave a half file
        // as the only survivor after a rare rename race.
        file.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path)?;
    // Fsync the parent dir so the rename itself survives power loss / hard death
    // soon after return (killall mid-turn dogfood).
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// Load the marker if present and valid. Missing / corrupt / empty prompt → `Ok(None)`.
pub fn load_canceled_turn_resume(
    cwd: &str,
    session_id: &str,
) -> io::Result<Option<CanceledTurnResume>> {
    let Some(path) = canceled_turn_resume_path(cwd, session_id) else {
        return Ok(None);
    };
    load_marker_at(&path)
}

fn load_marker_at(path: &Path) -> io::Result<Option<CanceledTurnResume>> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(None),
        Ok(s) => match serde_json::from_str::<CanceledTurnResume>(&s) {
            Ok(m) if !m.prompt_text.trim().is_empty() => Ok(Some(m)),
            Ok(_) => Ok(None),
            Err(_) => Ok(None),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Remove the marker if present.
pub fn clear_canceled_turn_resume(cwd: &str, session_id: &str) -> io::Result<()> {
    let Some(path) = canceled_turn_resume_path(cwd, session_id) else {
        return Ok(());
    };
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Whether restart should auto-resume: config on + valid marker present.
pub fn should_auto_resume_on_restart(
    resume_enabled: bool,
    marker: Option<&CanceledTurnResume>,
) -> bool {
    resume_enabled
        && marker.is_some_and(|m| {
            !m.prompt_text.trim().is_empty() && m.reason == CancelResumeReason::UserCancel
        })
}

/// Toast when auto-continuing a canceled turn after restart (marker path).
///
/// Plain English: **continue interrupted turn**. Not `/resume` (session pick).
pub fn auto_resume_toast() -> &'static str {
    "Continuing interrupted turn..."
}

/// Toast when auto-continuing from session history evidence (no marker).
///
/// Same operator moment as [`auto_resume_toast`]: reopen after killall /
/// process death mid-work when `canceled_turn_resume.json` was never written
/// (old binary, parent success clear, race). Same user-facing name as the
/// marker path (**continue interrupted turn**); internal logs still distinguish
/// marker vs history recovery.
pub fn auto_resume_interrupted_toast() -> &'static str {
    "Continuing interrupted turn..."
}

/// Toast when mid-work evidence exists on load but auto-continue could not start.
///
/// Dogfood-visible: reopen must not look silently idle when interrupted work
/// was found and then skipped (no prompt text, setting off, drain blocked).
/// Says **continue**, not "resume", so operators do not confuse this with
/// `/resume` session pick.
pub fn interrupted_resume_failed_toast(reason: &str) -> String {
    format!("Interrupted work found but could not continue: {reason}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn build_rejects_empty_prompt() {
        assert!(build_user_cancel_marker("", Some("p1"), "t").is_none());
        assert!(build_user_cancel_marker("   ", None, "t").is_none());
    }

    #[test]
    fn build_accepts_non_empty() {
        let m = build_user_cancel_marker("  fix the gate  ", Some("pid"), "2026-08-03T00:00:00Z")
            .unwrap();
        assert_eq!(m.prompt_text, "fix the gate");
        assert_eq!(m.prompt_id.as_deref(), Some("pid"));
        assert_eq!(m.reason, CancelResumeReason::UserCancel);
    }

    #[test]
    fn round_trip_write_load_clear() {
        let dir = TempDir::new().unwrap();
        // Point sessions root at temp by using an absolute cwd under temp.
        let cwd = dir.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy();
        let sid = "sess-cancel-1";

        let m =
            build_user_cancel_marker("continue this", Some("p-9"), "2026-08-03T12:00:00Z").unwrap();
        write_canceled_turn_resume(&cwd_str, sid, &m).unwrap();
        let loaded = load_canceled_turn_resume(&cwd_str, sid).unwrap().unwrap();
        assert_eq!(loaded.prompt_text, "continue this");
        assert_eq!(loaded.prompt_id.as_deref(), Some("p-9"));

        clear_canceled_turn_resume(&cwd_str, sid).unwrap();
        assert!(load_canceled_turn_resume(&cwd_str, sid).unwrap().is_none());
    }

    #[test]
    fn should_auto_resume_requires_enabled_and_user_cancel() {
        let m = build_user_cancel_marker("work", None, "t").unwrap();
        assert!(should_auto_resume_on_restart(true, Some(&m)));
        assert!(!should_auto_resume_on_restart(false, Some(&m)));
        assert!(!should_auto_resume_on_restart(true, None));
    }

    #[test]
    fn finished_work_has_no_marker_so_no_resume() {
        // No write → no invent.
        assert!(!should_auto_resume_on_restart(true, None));
    }

    /// Graceful process quit (SIGTERM class) reuses the same UserCancel marker
    /// shape so load re-queues once. Empty prompt still never invents work.
    #[test]
    fn process_shutdown_class_marker_is_auto_resume_eligible() {
        let m = build_user_cancel_marker(
            "interrupted by killall/SIGTERM path",
            Some("pid-term"),
            "2026-08-07T22:00:00Z",
        )
        .unwrap();
        assert_eq!(m.reason, CancelResumeReason::UserCancel);
        assert!(should_auto_resume_on_restart(true, Some(&m)));
        assert!(!should_auto_resume_on_restart(false, Some(&m)));
        assert!(build_user_cancel_marker("", None, "t").is_none());
    }

    /// Named contract: armed process-shutdown payload writes the same marker
    /// without AppView (signal hard-exit / first SIGTERM before Quit).
    #[test]
    fn armed_process_shutdown_writes_cancel_resume_marker() {
        let dir = TempDir::new().unwrap();
        let cwd = dir.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "sess-sigterm-arm";
        clear_process_shutdown_cancel_resume();
        arm_process_shutdown_cancel_resume(ProcessShutdownResumeArm {
            cwd: cwd_str.clone(),
            session_id: sid.into(),
            prompt_text: "mid-subagent killall path".into(),
            prompt_id: Some("pid-arm".into()),
        });
        assert!(write_armed_process_shutdown_cancel_resume().unwrap());
        let loaded = load_canceled_turn_resume(&cwd_str, sid).unwrap().unwrap();
        assert_eq!(loaded.prompt_text, "mid-subagent killall path");
        assert_eq!(loaded.prompt_id.as_deref(), Some("pid-arm"));
        assert!(should_auto_resume_on_restart(true, Some(&loaded)));
        clear_process_shutdown_cancel_resume();
        clear_canceled_turn_resume(&cwd_str, sid).unwrap();
        assert!(!write_armed_process_shutdown_cancel_resume().unwrap());
    }

    /// Named contract: turn-start arm+persist leaves a marker without waiting
    /// for SIGTERM / Action::Quit (killall race dogfood).
    #[test]
    fn arm_and_persist_writes_cancel_resume_marker_eagerly() {
        let dir = TempDir::new().unwrap();
        let cwd = dir.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "sess-eager-arm";
        clear_process_shutdown_cancel_resume();
        clear_canceled_turn_resume(&cwd_str, sid).unwrap();
        arm_and_persist_process_shutdown_cancel_resume(ProcessShutdownResumeArm {
            cwd: cwd_str.clone(),
            session_id: sid.into(),
            prompt_text: "eager active turn for killall".into(),
            prompt_id: Some("pid-eager".into()),
        });
        let loaded = load_canceled_turn_resume(&cwd_str, sid).unwrap().unwrap();
        assert_eq!(loaded.prompt_text, "eager active turn for killall");
        assert_eq!(loaded.prompt_id.as_deref(), Some("pid-eager"));
        assert!(should_auto_resume_on_restart(true, Some(&loaded)));
        // Process arm is global and other tests may clear it in parallel; the
        // durable marker on disk is the killall contract.
        let _ = process_shutdown_cancel_resume_arm();
        clear_process_shutdown_cancel_resume();
        clear_canceled_turn_resume(&cwd_str, sid).unwrap();
    }

    #[test]
    fn unsafe_session_id_rejected() {
        assert!(canceled_turn_resume_path("/tmp/p", "").is_none());
        assert!(canceled_turn_resume_path("/tmp/p", "../x").is_none());
        assert!(canceled_turn_resume_path("/tmp/p", "a/b").is_none());
    }
}
