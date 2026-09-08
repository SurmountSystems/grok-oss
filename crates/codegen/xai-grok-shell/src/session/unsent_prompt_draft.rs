//! Durable unsent composer draft, scoped to a session under a CWD.
//!
//! Lives next to session storage (`~/.grok/sessions/<enc-cwd>/<session_id>/`),
//! not in `prompt_history.jsonl` (that file is **submitted** prompts only).
//!
//! File mode is `0600` on Unix (same idea as external-editor temps).

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Keystroke draft writes wait this long before hitting disk again.
/// A burst of letters must not `sync_all` on every character.
pub const UNSENT_DRAFT_PERSIST_DEBOUNCE: Duration = Duration::from_millis(400);

/// Whether this composer edit should write the durable unsent draft now.
///
/// `force` is submit, wipe-to-empty, or pane teardown. Those write immediately.
/// Otherwise a write that landed inside the debounce window is skipped.
pub fn should_flush_unsent_draft(
    last_flush: Option<Instant>,
    now: Instant,
    debounce: Duration,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    match last_flush {
        None => true,
        Some(t) => now.saturating_duration_since(t) >= debounce,
    }
}

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
///
/// `fsync` is for submit / wipe / teardown. Keystroke coalesced writes skip
/// `sync_all` so typing is not blocked on disk.
pub fn write_draft_at(path: &Path, text: &str) -> io::Result<()> {
    write_draft_at_with_fsync(path, text, true)
}

/// Same as [`write_draft_at`], with an explicit fsync choice.
pub fn write_draft_at_with_fsync(path: &Path, text: &str, fsync: bool) -> io::Result<()> {
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
        if fsync {
            file.sync_all()?;
        } else {
            file.flush()?;
        }
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
    write_unsent_prompt_draft_with_fsync(cwd, session_id, text, true)
}

/// Keystroke coalesced persist: same path, no `sync_all`.
pub fn write_unsent_prompt_draft_with_fsync(
    cwd: &str,
    session_id: &str,
    text: &str,
    fsync: bool,
) -> io::Result<()> {
    let Some(path) = unsent_prompt_draft_path(cwd, session_id) else {
        return Ok(());
    };
    let _ = crate::util::grok_home::ensure_sessions_cwd_dir(cwd)?;
    write_draft_at_with_fsync(&path, text, fsync)
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

    #[test]
    fn write_without_fsync_still_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let path = draft_path(&tmp, "sess-a");
        write_draft_at_with_fsync(&path, "typed burst", false).unwrap();
        assert_eq!(
            load_draft_at(&path).unwrap().as_deref(),
            Some("typed burst")
        );
    }

    #[test]
    fn keystroke_burst_does_not_flush_unsent_draft_every_char() {
        let t0 = Instant::now();
        let debounce = UNSENT_DRAFT_PERSIST_DEBOUNCE;
        assert!(
            should_flush_unsent_draft(None, t0, debounce, false),
            "first keystroke must write once"
        );
        let mut flushes = 1u32;
        let mut skips = 0u32;
        for i in 1..12 {
            let now = t0 + Duration::from_millis(i * 8);
            if should_flush_unsent_draft(Some(t0), now, debounce, false) {
                flushes += 1;
            } else {
                skips += 1;
            }
        }
        assert_eq!(
            flushes, 1,
            "a burst inside the debounce window must not write every character"
        );
        assert_eq!(skips, 11);
        assert!(
            should_flush_unsent_draft(Some(t0), t0 + debounce, debounce, false),
            "after the debounce window the next edit may write"
        );
        assert!(
            should_flush_unsent_draft(Some(t0), t0 + Duration::from_millis(1), debounce, true),
            "submit or wipe-to-empty must write immediately"
        );
    }
}

/// Durable pager prompt queue (`pending_prompts.json` next to the unsent draft).
/// Not `prompt_tasks` and not `prompt_history.jsonl`.
pub mod pending_prompts {
    use super::sanitize_session_id;
    use serde::{Deserialize, Serialize};
    use std::fs::OpenOptions;
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};

    const PENDING_PROMPTS_FILE: &str = "pending_prompts.json";

    /// Ordinary pager-queue snapshots flush without `sync_all`.
    ///
    /// Operator: typing, cancel, and interject stay responsive. WAL is the
    /// crash-durable operator-text log (`prompt_wal.jsonl`). Rebuild still
    /// fsyncs via [`write_pending_prompts`].
    pub const PENDING_PROMPTS_QUEUE_SNAPSHOT_FSYNC: bool = false;

    /// One queued follow-up restored after grok-oss exits.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PersistedQueuedPrompt {
        pub id: u64,
        pub text: String,
        pub kind: String,
    }

    /// Path for this session's pending prompt queue under the CWD sessions directory.
    pub fn pending_prompts_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
        let sid = sanitize_session_id(session_id)?;
        Some(
            crate::util::grok_home::sessions_cwd_dir(cwd)
                .join(sid)
                .join(PENDING_PROMPTS_FILE),
        )
    }

    /// Write `rows` to `path` (mode `0600` on Unix). An empty list deletes the file.
    pub fn write_pending_prompts_at(path: &Path, rows: &[PersistedQueuedPrompt]) -> io::Result<()> {
        write_pending_prompts_at_with_fsync(path, rows, true)
    }

    /// Same as [`write_pending_prompts_at`], with an explicit fsync choice.
    /// Queue snapshots pass `false` so send/queue/interject-fallback do not
    /// `sync_all` on the UI thread. Rebuild / teardown pass `true`.
    pub fn write_pending_prompts_at_with_fsync(
        path: &Path,
        rows: &[PersistedQueuedPrompt],
        fsync: bool,
    ) -> io::Result<()> {
        if rows.is_empty() {
            return clear_pending_prompts_at(path);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        let body = serde_json::to_vec_pretty(rows).map_err(io::Error::other)?;
        {
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&tmp)?;
            file.write_all(&body)?;
            if fsync {
                file.sync_all()?;
            } else {
                file.flush()?;
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Load queued prompts from `path`. Missing or empty → `Ok(vec![])`.
    pub fn load_pending_prompts_at(path: &Path) -> io::Result<Vec<PersistedQueuedPrompt>> {
        match std::fs::read_to_string(path) {
            Ok(s) if s.trim().is_empty() => Ok(Vec::new()),
            Ok(s) => serde_json::from_str(&s).map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Remove the pending-prompts file if present.
    pub fn clear_pending_prompts_at(path: &Path) -> io::Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Write the pending prompt queue for `(cwd, session_id)`. Empty clears.
    /// Fsyncs. Prefer [`write_pending_prompts_with_fsync`] for queue snapshots.
    pub fn write_pending_prompts(
        cwd: &str,
        session_id: &str,
        rows: &[PersistedQueuedPrompt],
    ) -> io::Result<()> {
        write_pending_prompts_with_fsync(cwd, session_id, rows, true)
    }

    /// Queue snapshot persist: same path, optional `sync_all`.
    pub fn write_pending_prompts_with_fsync(
        cwd: &str,
        session_id: &str,
        rows: &[PersistedQueuedPrompt],
        fsync: bool,
    ) -> io::Result<()> {
        let Some(path) = pending_prompts_path(cwd, session_id) else {
            return Ok(());
        };
        let _ = crate::util::grok_home::ensure_sessions_cwd_dir(cwd)?;
        write_pending_prompts_at_with_fsync(&path, rows, fsync)
    }

    /// Load the pending prompt queue for `(cwd, session_id)`.
    pub fn load_pending_prompts(
        cwd: &str,
        session_id: &str,
    ) -> io::Result<Vec<PersistedQueuedPrompt>> {
        let Some(path) = pending_prompts_path(cwd, session_id) else {
            return Ok(Vec::new());
        };
        load_pending_prompts_at(&path)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::TempDir;

        fn queue_path(tmp: &TempDir, session_id: &str) -> PathBuf {
            tmp.path().join(session_id).join(PENDING_PROMPTS_FILE)
        }

        fn row(id: u64, text: &str) -> PersistedQueuedPrompt {
            PersistedQueuedPrompt {
                id,
                text: text.into(),
                kind: "prompt".into(),
            }
        }

        #[test]
        fn pending_prompts_survive_restart_without_prompt_tasks() {
            let tmp = TempDir::new().unwrap();
            let path = queue_path(&tmp, "sess-a");
            write_pending_prompts_at(&path, &[row(1, "first queued"), row(2, "second queued")])
                .unwrap();
            let restored = load_pending_prompts_at(&path).unwrap();
            assert_eq!(restored.len(), 2);
            assert_eq!(restored[0].text, "first queued");
            assert_eq!(restored[1].text, "second queued");
            assert!(
                !path.to_string_lossy().contains("prompt_tasks"),
                "pager queue must not live in grok_oss.db prompt_tasks: {path:?}"
            );
        }

        #[test]
        fn write_without_fsync_still_roundtrips() {
            let tmp = TempDir::new().unwrap();
            let path = queue_path(&tmp, "sess-a");
            write_pending_prompts_at_with_fsync(&path, &[row(1, "queued without fsync")], false)
                .unwrap();
            let restored = load_pending_prompts_at(&path).unwrap();
            assert_eq!(restored.len(), 1);
            assert_eq!(restored[0].text, "queued without fsync");
        }

        /// Operator: typing/cancel/interject stay responsive. Ordinary queue
        /// snapshots must not `sync_all`. WAL is the durability log.
        #[test]
        fn pending_prompts_queue_snapshot_skips_sync_all() {
            const {
                assert!(
                    !PENDING_PROMPTS_QUEUE_SNAPSHOT_FSYNC,
                    "pending_prompts.json queue snapshots must flush without sync_all"
                );
            }
        }

        #[test]
        fn empty_queue_clears_file() {
            let tmp = TempDir::new().unwrap();
            let path = queue_path(&tmp, "sess-a");
            write_pending_prompts_at(&path, &[row(1, "temp")]).unwrap();
            write_pending_prompts_at(&path, &[]).unwrap();
            assert!(load_pending_prompts_at(&path).unwrap().is_empty());
            assert!(!path.exists());
        }

        #[test]
        fn write_does_not_touch_prompt_history_or_unsent_draft() {
            let tmp = TempDir::new().unwrap();
            let cwd_dir = tmp.path().join("enc-cwd");
            std::fs::create_dir_all(&cwd_dir).unwrap();
            let history = cwd_dir.join("prompt_history.jsonl");
            let draft = cwd_dir.join("sess-a").join("unsent_prompt_draft");
            std::fs::write(&history, b"").unwrap();
            std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
            std::fs::write(&draft, b"composer draft").unwrap();
            let before_hist = std::fs::read(&history).unwrap();
            let before_draft = std::fs::read(&draft).unwrap();
            let queue = cwd_dir.join("sess-a").join(PENDING_PROMPTS_FILE);
            write_pending_prompts_at(&queue, &[row(1, "queued body")]).unwrap();
            assert_eq!(std::fs::read(&history).unwrap(), before_hist);
            assert_eq!(std::fs::read(&draft).unwrap(), before_draft);
            assert!(queue.exists());
        }

        #[test]
        fn different_session_id_does_not_cross_restore() {
            let tmp = TempDir::new().unwrap();
            let path_a = queue_path(&tmp, "sess-a");
            let path_b = queue_path(&tmp, "sess-b");
            write_pending_prompts_at(&path_a, &[row(1, "only A")]).unwrap();
            assert!(load_pending_prompts_at(&path_b).unwrap().is_empty());
        }

        #[test]
        fn path_includes_session_id_component() {
            let path = pending_prompts_path("/tmp/proj", "abc-123").expect("safe id");
            let s = path.to_string_lossy();
            assert!(s.contains("abc-123"), "path must key by session_id: {s}");
            assert!(
                s.ends_with(PENDING_PROMPTS_FILE),
                "leaf must be {PENDING_PROMPTS_FILE}: {s}"
            );
        }

        #[test]
        fn unsafe_session_id_yields_no_path() {
            assert!(pending_prompts_path("/tmp/p", "").is_none());
            assert!(pending_prompts_path("/tmp/p", "../escape").is_none());
            assert!(pending_prompts_path("/tmp/p", "a/b").is_none());
        }
    }
}

/// Session-local operator prompt write-ahead log (`prompt_wal.jsonl`).
///
/// Lives next to `unsent_prompt_draft` under the session directory. Append-only.
/// Not git. Not conversation. Not model tokens. Compact must not rewrite it.
pub mod prompt_wal {
    use super::sanitize_session_id;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use std::fs::OpenOptions;
    use std::io::{self, BufRead, BufReader, Write};
    use std::path::{Path, PathBuf};

    /// Leaf name next to `unsent_prompt_draft`.
    pub const PROMPT_WAL_FILE: &str = "prompt_wal.jsonl";

    /// Why this operator text was written.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum PromptWalKind {
        Send,
        Interject,
        Queue,
        PlanNotes,
        RebuildFlush,
    }

    /// `[Image #N]` plus the durable file name under `images/`. Never a data URL.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PromptWalImage {
        /// Display number in `[Image #N]`.
        pub n: u32,
        /// File name under the session `images/` directory.
        pub file_id: String,
    }

    /// One append-only JSONL object.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PromptWalRecord {
        /// ULID for this line.
        pub id: String,
        /// Wall clock when the line was appended (RFC 3339).
        pub wall_time: DateTime<Utc>,
        pub session_id: String,
        pub kind: PromptWalKind,
        /// Full operator text, including `[Image #N]` tokens.
        pub text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub images: Vec<PromptWalImage>,
    }

    impl PromptWalRecord {
        /// Mint a new record. `file_id` values that look like data URLs are dropped.
        pub fn new(
            session_id: impl Into<String>,
            kind: PromptWalKind,
            text: impl Into<String>,
            images: Vec<PromptWalImage>,
        ) -> Self {
            Self {
                id: ulid::Ulid::new().to_string(),
                wall_time: Utc::now(),
                session_id: session_id.into(),
                kind,
                text: text.into(),
                images: images
                    .into_iter()
                    .filter(|img| image_file_id_is_safe(&img.file_id))
                    .collect(),
            }
        }
    }

    /// Reject inline data URLs. WAL stores file ids under `images/` only.
    pub fn image_file_id_is_safe(file_id: &str) -> bool {
        let t = file_id.trim();
        !t.is_empty()
            && !t.contains('/')
            && !t.contains('\\')
            && !t.contains('\0')
            && !t.contains("..")
            && !t.to_ascii_lowercase().starts_with("data:")
    }

    /// Path for this session's WAL under the CWD sessions directory.
    pub fn prompt_wal_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
        let sid = sanitize_session_id(session_id)?;
        Some(
            crate::util::grok_home::sessions_cwd_dir(cwd)
                .join(sid)
                .join(PROMPT_WAL_FILE),
        )
    }

    /// Append one JSON object and fsync that line. Never rewrites prior lines.
    pub fn append_prompt_wal_at(path: &Path, record: &PromptWalRecord) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut line = serde_json::to_vec(record).map_err(io::Error::other)?;
        line.push(b'\n');
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(&line)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Append for `(cwd, session_id)`.
    pub fn append_prompt_wal(
        cwd: &str,
        session_id: &str,
        record: &PromptWalRecord,
    ) -> io::Result<()> {
        let Some(path) = prompt_wal_path(cwd, session_id) else {
            return Ok(());
        };
        let _ = crate::util::grok_home::ensure_sessions_cwd_dir(cwd)?;
        append_prompt_wal_at(&path, record)
    }

    /// Load every well-formed line. Missing file is empty. Never rewrites the WAL.
    pub fn load_prompt_wal_at(path: &Path) -> io::Result<Vec<PromptWalRecord>> {
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<PromptWalRecord>(&line) {
                out.push(rec);
            }
        }
        Ok(out)
    }

    /// Load the WAL for `(cwd, session_id)`.
    pub fn load_prompt_wal(cwd: &str, session_id: &str) -> io::Result<Vec<PromptWalRecord>> {
        let Some(path) = prompt_wal_path(cwd, session_id) else {
            return Ok(Vec::new());
        };
        load_prompt_wal_at(&path)
    }

    /// `chat_history.jsonl` next to the WAL (same session directory).
    pub fn chat_history_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
        Some(
            prompt_wal_path(cwd, session_id)?
                .parent()?
                .join("chat_history.jsonl"),
        )
    }

    /// Whether `text` already exists as a Human turn in history or the pager queue.
    pub fn operator_text_already_recorded(
        text: &str,
        prompt_history: &[String],
        queue_texts: &[String],
        chat_history_blob: Option<&str>,
    ) -> bool {
        let needle = text.trim();
        if needle.is_empty() {
            return true;
        }
        if prompt_history.iter().any(|p| p.trim() == needle) {
            return true;
        }
        if queue_texts.iter().any(|p| p.trim() == needle) {
            return true;
        }
        if let Some(blob) = chat_history_blob
            && blob.contains(needle)
        {
            return true;
        }
        false
    }

    /// WAL sends (and interject/queue) missing from chat/prompt/queue.
    /// Restore those as pending Human turns. Plan notes and rebuild flush
    /// have their own draft/queue restore paths.
    pub fn wal_sends_missing_from_history(
        records: &[PromptWalRecord],
        prompt_history: &[String],
        queue_texts: &[String],
        chat_history_blob: Option<&str>,
    ) -> Vec<PromptWalRecord> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for rec in records {
            match rec.kind {
                PromptWalKind::Send | PromptWalKind::Interject | PromptWalKind::Queue => {}
                PromptWalKind::PlanNotes | PromptWalKind::RebuildFlush => continue,
            }
            let key = rec.text.trim().to_string();
            if key.is_empty() || !seen.insert(key) {
                continue;
            }
            if operator_text_already_recorded(
                &rec.text,
                prompt_history,
                queue_texts,
                chat_history_blob,
            ) {
                continue;
            }
            out.push(rec.clone());
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::TempDir;

        fn rec(kind: PromptWalKind, text: &str) -> PromptWalRecord {
            PromptWalRecord::new("sess-a", kind, text, Vec::new())
        }

        #[test]
        fn append_fsyncs_a_line_and_does_not_rewrite_prior_lines() {
            let tmp = TempDir::new().unwrap();
            let path = tmp.path().join("sess-a").join(PROMPT_WAL_FILE);
            let first = rec(PromptWalKind::Send, "first operator send");
            let second = rec(PromptWalKind::Queue, "queued later");
            append_prompt_wal_at(&path, &first).unwrap();
            append_prompt_wal_at(&path, &second).unwrap();
            let loaded = load_prompt_wal_at(&path).unwrap();
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded[0].text, "first operator send");
            assert_eq!(loaded[0].kind, PromptWalKind::Send);
            assert_eq!(loaded[1].text, "queued later");
            assert_eq!(
                loaded[0].id, first.id,
                "append must not rewrite the first line"
            );
            assert!(
                !loaded
                    .iter()
                    .any(|r| r.images.iter().any(|i| i.file_id.starts_with("data:"))),
                "WAL must never store inline data URLs"
            );
        }

        #[test]
        fn drops_data_url_image_file_ids() {
            let rec = PromptWalRecord::new(
                "sess-a",
                PromptWalKind::Send,
                "see [Image #1]",
                vec![
                    PromptWalImage {
                        n: 1,
                        file_id: "data:image/png;base64,AAAA".into(),
                    },
                    PromptWalImage {
                        n: 2,
                        file_id: "abc.png".into(),
                    },
                ],
            );
            assert_eq!(rec.images.len(), 1);
            assert_eq!(rec.images[0].file_id, "abc.png");
        }

        #[test]
        fn wal_helper_selects_send_missing_from_prompt_history() {
            let send = rec(PromptWalKind::Send, "lost operator enter send");
            let already = rec(PromptWalKind::Send, "already in history");
            let missing = wal_sends_missing_from_history(
                &[send.clone(), already],
                &["already in history".into()],
                &[],
                None,
            );
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0].text, "lost operator enter send");
        }

        #[test]
        fn does_not_restore_when_chat_history_or_queue_has_the_send() {
            let send = rec(PromptWalKind::Send, "present in chat");
            assert!(
                wal_sends_missing_from_history(
                    std::slice::from_ref(&send),
                    &[],
                    &[],
                    Some("{\"role\":\"user\",\"content\":\"present in chat\"}"),
                )
                .is_empty()
            );
            assert!(
                wal_sends_missing_from_history(&[send], &[], &["present in chat".into()], None)
                    .is_empty()
            );
        }

        #[test]
        fn plan_notes_and_rebuild_flush_are_not_pending_human_turns() {
            let notes = rec(PromptWalKind::PlanNotes, "approve notes");
            let flush = rec(PromptWalKind::RebuildFlush, "composer leftover");
            assert!(wal_sends_missing_from_history(&[notes, flush], &[], &[], None).is_empty());
        }

        #[test]
        fn prompt_wal_is_not_a_conversation_transcript_filename() {
            assert_ne!(PROMPT_WAL_FILE, "chat_history.jsonl");
            assert_ne!(PROMPT_WAL_FILE, "prompt_history.jsonl");
            assert_ne!(PROMPT_WAL_FILE, "unsent_prompt_draft");
        }

        #[test]
        fn path_sits_next_to_unsent_draft() {
            let path = prompt_wal_path("/tmp/proj", "abc-123").expect("safe id");
            let s = path.to_string_lossy();
            assert!(s.contains("abc-123"));
            assert!(s.ends_with(PROMPT_WAL_FILE));
        }

        #[test]
        fn unsafe_session_id_yields_no_path() {
            assert!(prompt_wal_path("/tmp/p", "").is_none());
            assert!(prompt_wal_path("/tmp/p", "../escape").is_none());
            assert!(prompt_wal_path("/tmp/p", "a/b").is_none());
        }
    }
}
