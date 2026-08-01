//! Durable unsent composer draft, scoped to a session under a CWD.
//!
//! Lives next to session storage (`~/.grok/sessions/<enc-cwd>/<session_id>/`),
//! not in `prompt_history.jsonl` (that file is **submitted** prompts only).
//!
//! File mode is `0600` on Unix (same idea as external-editor temps).

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const UNSENT_PROMPT_DRAFT_FILE: &str = "unsent_prompt_draft";

/// Reject empty / path-traversal session ids so draft files cannot escape the
/// sessions tree.
fn sanitize_session_id(session_id: &str) -> Option<&str> {
    let s = session_id.trim();
    if s.is_empty() || s.contains('/') || s.contains('\\') || s.contains("..") {
        return None;
    }
    Some(s)
}

/// Path for this session's unsent draft under the CWD sessions directory.
///
/// `None` when `session_id` is not a safe single path component.
pub fn unsent_prompt_draft_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
    let sid = sanitize_session_id(session_id)?;
    Some(
        crate::util::grok_home::sessions_cwd_dir(cwd)
            .join(sid)
            .join(UNSENT_PROMPT_DRAFT_FILE),
    )
}

/// Write `text` to `path` (mode `0600` on Unix). Empty text deletes the file.
pub fn write_draft_at(path: &Path, text: &str) -> io::Result<()> {
    if text.is_empty() {
        return clear_draft_at(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
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
        file.write_all(text.as_bytes())?;
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

/// Load draft text from `path`. Missing or empty → `Ok(None)`.
pub fn load_draft_at(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.is_empty() => Ok(None),
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Remove the draft file if present.
pub fn clear_draft_at(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Write the unsent draft for `(cwd, session_id)`. Empty text clears.
pub fn write_unsent_prompt_draft(cwd: &str, session_id: &str, text: &str) -> io::Result<()> {
    let Some(path) = unsent_prompt_draft_path(cwd, session_id) else {
        return Ok(());
    };
    let _ = crate::util::grok_home::ensure_sessions_cwd_dir(cwd)?;
    write_draft_at(&path, text)
}

/// Load the unsent draft for `(cwd, session_id)`.
pub fn load_unsent_prompt_draft(cwd: &str, session_id: &str) -> io::Result<Option<String>> {
    let Some(path) = unsent_prompt_draft_path(cwd, session_id) else {
        return Ok(None);
    };
    load_draft_at(&path)
}

/// Clear the unsent draft for `(cwd, session_id)`.
pub fn clear_unsent_prompt_draft(cwd: &str, session_id: &str) -> io::Result<()> {
    let Some(path) = unsent_prompt_draft_path(cwd, session_id) else {
        return Ok(());
    };
    clear_draft_at(&path)
}

/// Named restore rule: never clobber a non-empty live composer with a draft.
///
/// Returns `true` when the loaded draft should be applied into the composer.
pub fn should_restore_draft_into_composer(composer_text: &str, draft: &str) -> bool {
    composer_text.is_empty() && !draft.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn draft_path(tmp: &TempDir, session_id: &str) -> PathBuf {
        tmp.path().join(session_id).join(UNSENT_PROMPT_DRAFT_FILE)
    }

    /// Contract 1: unsent draft survives "restart" (write → drop buffer → load).
    #[test]
    fn unsent_draft_survives_restart_for_same_session() {
        let tmp = TempDir::new().unwrap();
        let path = draft_path(&tmp, "sess-a");
        write_draft_at(&path, "still typing a plan note").unwrap();
        // Simulate process death: only the file remains.
        let restored = load_draft_at(&path).unwrap();
        assert_eq!(
            restored.as_deref(),
            Some("still typing a plan note"),
            "same session_id path must restore prior unsent text"
        );
    }

    /// Contract 2: successful submit clears durable draft.
    #[test]
    fn submit_clears_durable_draft() {
        let tmp = TempDir::new().unwrap();
        let path = draft_path(&tmp, "sess-a");
        write_draft_at(&path, "about to send this").unwrap();
        assert!(path.exists());
        clear_draft_at(&path).unwrap();
        assert!(
            load_draft_at(&path).unwrap().is_none(),
            "after clear, resume must not resurrect the last message as unsent"
        );
        assert!(!path.exists());
    }

    /// Contract 3: empty draft / non-empty live composer must not restore.
    #[test]
    fn empty_or_live_composer_does_not_clobber() {
        assert!(
            !should_restore_draft_into_composer("already typing", "old draft"),
            "non-empty live composer must not be replaced by disk draft"
        );
        assert!(
            !should_restore_draft_into_composer("", ""),
            "empty draft must not restore"
        );
        assert!(
            should_restore_draft_into_composer("", "recover me"),
            "empty composer + non-empty draft must restore"
        );
    }

    /// Contract 5: different session_id does not cross-restore.
    #[test]
    fn different_session_id_does_not_cross_restore() {
        let tmp = TempDir::new().unwrap();
        let path_a = draft_path(&tmp, "sess-a");
        let path_b = draft_path(&tmp, "sess-b");
        write_draft_at(&path_a, "secret for A only").unwrap();
        assert!(
            load_draft_at(&path_b).unwrap().is_none(),
            "session B must not see session A's draft"
        );
        assert_eq!(
            load_draft_at(&path_a).unwrap().as_deref(),
            Some("secret for A only")
        );
    }

    /// Contract 8: file mode is private (0600) on unix.
    #[cfg(unix)]
    #[test]
    fn draft_file_mode_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let path = draft_path(&tmp, "sess-a");
        write_draft_at(&path, "private notes").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "unsent draft must be owner-read/write only");
    }

    /// Contract 7: writing a draft must not append to prompt_history.jsonl.
    #[test]
    fn draft_write_does_not_touch_prompt_history() {
        let tmp = TempDir::new().unwrap();
        // Simulate a sessions cwd dir layout next to prompt_history.
        let cwd_dir = tmp.path().join("enc-cwd");
        std::fs::create_dir_all(&cwd_dir).unwrap();
        let history = cwd_dir.join("prompt_history.jsonl");
        std::fs::write(&history, b"").unwrap();
        let before = std::fs::read(&history).unwrap();
        let draft = cwd_dir.join("sess-a").join(UNSENT_PROMPT_DRAFT_FILE);
        write_draft_at(&draft, "keystroke draft only").unwrap();
        let after = std::fs::read(&history).unwrap();
        assert_eq!(
            before, after,
            "draft I/O must not append lines to prompt_history.jsonl"
        );
        assert!(draft.exists());
    }

    #[test]
    fn path_includes_session_id_component() {
        let path = unsent_prompt_draft_path("/tmp/proj", "abc-123").expect("safe id");
        let s = path.to_string_lossy();
        assert!(s.contains("abc-123"), "path must key by session_id: {s}");
        assert!(
            s.ends_with(UNSENT_PROMPT_DRAFT_FILE),
            "leaf must be {UNSENT_PROMPT_DRAFT_FILE}: {s}"
        );
    }

    #[test]
    fn unsafe_session_id_yields_no_path() {
        assert!(unsent_prompt_draft_path("/tmp/p", "").is_none());
        assert!(unsent_prompt_draft_path("/tmp/p", "../escape").is_none());
        assert!(unsent_prompt_draft_path("/tmp/p", "a/b").is_none());
    }

    #[test]
    fn write_empty_clears_existing_draft() {
        let tmp = TempDir::new().unwrap();
        let path = draft_path(&tmp, "sess-a");
        write_draft_at(&path, "temp").unwrap();
        write_draft_at(&path, "").unwrap();
        assert!(load_draft_at(&path).unwrap().is_none());
        assert!(!path.exists());
    }
}
