//! Durable marker for an explicitly canceled turn so restart can resume it.
//!
//! Lives under `$GROK_HOME/sessions/<enc-cwd>/<session_id>/canceled_turn_resume.json`.
//! Not secrets. Only written on **explicit user cancel** (Esc / stop), not on
//! clean success, network blips, or fearless global pause.
//!
//! Resume on session open is gated by `[ui] resume_canceled_turn_on_restart`
//! (default **on**). Finished work must never invent a resume marker.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CANCELED_TURN_RESUME_FILE: &str = "canceled_turn_resume.json";

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
        file.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path)?;
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

/// Toast when auto-resuming a canceled turn after restart.
pub fn auto_resume_toast() -> &'static str {
    "Resuming canceled turn..."
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

    #[test]
    fn unsafe_session_id_rejected() {
        assert!(canceled_turn_resume_path("/tmp/p", "").is_none());
        assert!(canceled_turn_resume_path("/tmp/p", "../x").is_none());
        assert!(canceled_turn_resume_path("/tmp/p", "a/b").is_none());
    }
}
