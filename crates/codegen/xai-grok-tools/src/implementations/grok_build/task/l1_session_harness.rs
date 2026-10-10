//! L1 harness for an early L2 exit, soft help between live L1 sessions,
//! a post-restart resume check, and failure feedback from L1, L2, and L3.
//!
//! The task tool calls [`l2_exit_action`] on a blocking exit.
//! This module does not add a permission mode or an auth key.
//! Soft help is not a lock and not a kill.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Seconds that still count as about a couple of minutes.
pub const EARLY_L2_EXIT_WINDOW_SECS: u64 = 180;

/// Tool-call count below this is few. Five or more is not few.
pub const EARLY_L2_FEW_TOOL_CALLS: u64 = 5;

/// Example resource an L1 may mention in a soft message.
pub const LIVE_CHECK_REMOTE_RESOURCE: &str = "a live just check-remote";

/// One JSON object per line under the target session directory.
pub const SOFT_MESSAGES_FILE: &str = "soft_messages.jsonl";

/// One JSON object per line under the session directory.
pub const HARNESS_FEEDBACK_FILE: &str = "harness_feedback.jsonl";

/// What L1 does when an L2 exits.
///
/// [`L2ExitAction::SpawnDuplicate`] is the start of another L2 for the same
/// job. [`l2_exit_action`] does not return it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L2ExitAction {
    /// Resume that same L2 id. Not a new L2.
    ResumeSame { l2_id: String },
    /// The exit was not early and the L2 left a land report.
    Done,
    /// Another L2 for the same job. Not a resume of the same L2.
    SpawnDuplicate { l2_id: String },
}

/// One L1 row for the post-restart check.
///
/// `resumed_gracefully` is `Some(true)` when that L1 resumed, `Some(false)`
/// when it did not, and `None` when the outcome is unknown. Unknown counts
/// as resumed. This check does not invent a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L1ResumeRow {
    pub session_id: String,
    pub resumed_gracefully: Option<bool>,
}

/// Help from one L1 session to another. Not a lock and not a kill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftMessage {
    pub from_session: String,
    pub to_session: String,
    pub text: String,
    pub resource: String,
    /// Not written. A soft message does not record a lock.
    #[serde(skip)]
    lock: bool,
    /// Not written. A soft message does not record a kill.
    #[serde(skip)]
    kill: bool,
}

impl SoftMessage {
    /// Build a help message. The lock flag and the kill flag stay false.
    pub fn help(
        from_session: impl Into<String>,
        to_session: impl Into<String>,
        text: impl Into<String>,
        resource: impl Into<String>,
    ) -> Self {
        Self {
            from_session: from_session.into(),
            to_session: to_session.into(),
            text: text.into(),
            resource: resource.into(),
            lock: false,
            kill: false,
        }
    }

    /// Soft help is not a lock.
    pub fn is_lock(&self) -> bool {
        self.lock
    }

    /// Soft help is not a kill.
    pub fn is_kill(&self) -> bool {
        self.kill
    }
}

/// Which agent layer sent harness feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentLayer {
    L1,
    L2,
    L3,
}

/// What failed, in the words the harness stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessFailure {
    EarlyExit,
    RepeatingSentence,
    MissingReport,
}

/// Feedback the harness can store from an L1, an L2, or an L3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessFeedback {
    pub layer: AgentLayer,
    pub failure: HarnessFailure,
}

/// True when the L2 exit is early.
///
/// Early means the run is still inside about a couple of minutes
/// (`elapsed_secs` <= [`EARLY_L2_EXIT_WINDOW_SECS`]) and at least one of
/// these is true: fewer than [`EARLY_L2_FEW_TOOL_CALLS`] tool calls, no land
/// report, or a stop on a repeating sentence before work.
///
/// A run with a land report and at least [`EARLY_L2_FEW_TOOL_CALLS`] tool
/// calls is not early, even when `elapsed_secs` is short. A long run with a
/// land report is not early.
pub fn early_l2_exit(
    elapsed_secs: u64,
    tool_calls: u64,
    has_land_report: bool,
    stopped_on_repeating_sentence: bool,
) -> bool {
    let in_window = elapsed_secs <= EARLY_L2_EXIT_WINDOW_SECS;
    let few_tool_calls = tool_calls < EARLY_L2_FEW_TOOL_CALLS;
    let thin_exit = few_tool_calls || !has_land_report || stopped_on_repeating_sentence;
    let landed_enough = has_land_report && !few_tool_calls;
    in_window && thin_exit && !landed_enough
}

/// Resume the same L2 when the exit is early or there is no land report.
///
/// Done only when the exit is not early and `has_land_report` is true.
/// This does not return [`L2ExitAction::SpawnDuplicate`].
pub fn l2_exit_action(
    l2_id: &str,
    elapsed_secs: u64,
    tool_calls: u64,
    has_land_report: bool,
    stopped_on_repeating_sentence: bool,
) -> L2ExitAction {
    let early = early_l2_exit(
        elapsed_secs,
        tool_calls,
        has_land_report,
        stopped_on_repeating_sentence,
    );
    if early || !has_land_report {
        L2ExitAction::ResumeSame {
            l2_id: l2_id.to_string(),
        }
    } else {
        L2ExitAction::Done
    }
}

/// True when `action` resumes `l2_id` and not some other id.
pub fn resumes_same_l2(action: &L2ExitAction, l2_id: &str) -> bool {
    matches!(action, L2ExitAction::ResumeSame { l2_id: resumed } if resumed == l2_id)
}

/// True when the action is [`L2ExitAction::Done`].
pub fn is_done(action: &L2ExitAction) -> bool {
    matches!(action, L2ExitAction::Done)
}

/// True when the action would start another L2 for the same job.
pub fn is_spawn_duplicate(action: &L2ExitAction) -> bool {
    matches!(action, L2ExitAction::SpawnDuplicate { .. })
}

/// Session ids in `active_sessions.json` under `root`, excluding `this_session`.
///
/// This crate does not call the pager live-window list. The read does not
/// take a lock. Callers choose `root`. Tests pass a directory under
/// [`std::env::temp_dir`], not the operator grok home.
pub fn other_live_l1_session_ids(root: &Path, this_session: &str) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(root.join("active_sessions.json"))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(json_to_io)?;
    let Some(rows) = value.as_array() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "active_sessions.json is not an array",
        ));
    };
    Ok(rows
        .iter()
        .filter_map(|row| {
            row.get("session_id")
                .and_then(|id| id.as_str())
                .map(str::to_string)
        })
        .filter(|id| id != this_session)
        .collect())
}

/// Path of the soft-message log under a session directory.
pub fn soft_messages_path(session_dir: &Path) -> PathBuf {
    session_dir.join(SOFT_MESSAGES_FILE)
}

/// Append one soft help line. Does not create a lock file and does not
/// record a kill. Refuses a message whose lock flag or kill flag is set.
pub fn append_soft_message(session_dir: &Path, message: &SoftMessage) -> io::Result<()> {
    if message.is_lock() || message.is_kill() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a soft message is help, not a lock and not a kill",
        ));
    }
    append_jsonl_line(session_dir, SOFT_MESSAGES_FILE, message)
}

/// Read soft help lines back. Lock and kill stay false because they are not
/// stored in the file.
pub fn read_soft_messages(session_dir: &Path) -> io::Result<Vec<SoftMessage>> {
    read_jsonl(session_dir, SOFT_MESSAGES_FILE)
}

/// This post-restart check is allowed. It is not a new permission mode and
/// it does not read an auth key.
pub fn post_restart_check_is_allowed() -> bool {
    true
}

/// One soft help message for each other L1 that did not resume gracefully.
///
/// Unknown (`None`) counts as resumed, so this does not invent a failure.
/// The check runs only when [`post_restart_check_is_allowed`] is true.
pub fn post_restart_help(this_l1: &L1ResumeRow, others: &[L1ResumeRow]) -> Vec<SoftMessage> {
    if !post_restart_check_is_allowed() {
        return Vec::new();
    }
    others
        .iter()
        .filter(|row| row.session_id != this_l1.session_id)
        .filter(|row| row.resumed_gracefully == Some(false))
        .map(|row| {
            SoftMessage::help(
                this_l1.session_id.clone(),
                row.session_id.clone(),
                format!(
                    "After a Grok OSS rebuild and process restart, this L1 is helping session {} resume.",
                    row.session_id
                ),
                "graceful resume",
            )
        })
        .collect()
}

/// Path of the harness feedback log under a session directory.
pub fn harness_feedback_path(session_dir: &Path) -> PathBuf {
    session_dir.join(HARNESS_FEEDBACK_FILE)
}

/// Build one feedback record. Does not take a lock and does not kill.
///
/// The task tool calls this when an exit is an early exit, a repeating
/// sentence, or a missing report. Layers are only L1, L2, and L3.
pub fn record_harness_feedback(layer: AgentLayer, failure: HarnessFailure) -> HarnessFeedback {
    HarnessFeedback { layer, failure }
}

/// Append one feedback line from an L1, an L2, or an L3.
pub fn append_harness_feedback(session_dir: &Path, feedback: &HarnessFeedback) -> io::Result<()> {
    append_jsonl_line(session_dir, HARNESS_FEEDBACK_FILE, feedback)
}

/// Read feedback lines back in append order.
pub fn read_harness_feedback(session_dir: &Path) -> io::Result<Vec<HarnessFeedback>> {
    read_jsonl(session_dir, HARNESS_FEEDBACK_FILE)
}

fn append_jsonl_line<T: Serialize>(
    session_dir: &Path,
    file_name: &str,
    value: &T,
) -> io::Result<()> {
    fs::create_dir_all(session_dir)?;
    let path = session_dir.join(file_name);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(value).map_err(json_to_io)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn read_jsonl<T: DeserializeOwned>(session_dir: &Path, file_name: &str) -> io::Result<Vec<T>> {
    let text = fs::read_to_string(session_dir.join(file_name))?;
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(line).map_err(json_to_io)?);
    }
    Ok(rows)
}

fn json_to_io(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARLY_EXIT_SENTENCE: &str = "If an L2 exits early (about a couple of minutes, few or zero tool calls, no land report, or stopped for a repeating sentence before work), L1 resumes that same L2. Do not treat the exit as done. Do not start a duplicate L2 for the same job.";

    const SOFT_MESSAGE_SENTENCE: &str = "L1s can read other L1 sessions on this machine and send soft messages about resources they are using (for example a live just check-remote). Soft means help, not a lock and not a kill.";

    const POST_RESTART_SENTENCE: &str = "After a Grok OSS rebuild and process restart, this Grok OSS L1 is privileged to check whether other L1s resumed gracefully and to help them along.";

    const FEEDBACK_SENTENCE: &str = "The harness should also be able to receive feedback from L1, L2, and L3 agents about what failed (early exit, repeating sentence, missing report).";

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str, sentence: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!(
                "grok-oss-l1-harness-{label}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            assert!(
                path.starts_with(std::env::temp_dir()),
                "{sentence} tests use std::env::temp_dir, got {path:?}"
            );
            if let Some(home) = std::env::var_os("HOME") {
                let grok_home = PathBuf::from(home).join(".grok");
                assert!(
                    !path.starts_with(&grok_home),
                    "{sentence} must not write the operator grok home, got {path:?}"
                );
            }
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn assert_resume_same(action: &L2ExitAction, l2_id: &str) {
        assert!(
            resumes_same_l2(action, l2_id),
            "{EARLY_EXIT_SENTENCE} action={action:?}"
        );
        assert!(!is_done(action), "{EARLY_EXIT_SENTENCE} action={action:?}");
        assert!(
            !is_spawn_duplicate(action),
            "{EARLY_EXIT_SENTENCE} action={action:?}"
        );
        let forbidden = L2ExitAction::SpawnDuplicate {
            l2_id: l2_id.to_string(),
        };
        assert_ne!(action, &forbidden, "{EARLY_EXIT_SENTENCE}");
    }

    fn assert_done(action: &L2ExitAction, l2_id: &str) {
        assert!(is_done(action), "{EARLY_EXIT_SENTENCE} action={action:?}");
        assert!(
            !resumes_same_l2(action, l2_id),
            "{EARLY_EXIT_SENTENCE} action={action:?}"
        );
        assert!(
            !is_spawn_duplicate(action),
            "{EARLY_EXIT_SENTENCE} action={action:?}"
        );
    }

    fn assert_session_dir_has_no_lock_or_kill(dir: &Path, sentence: &str) {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().file_name();
            let name = name.to_string_lossy().into_owned();
            assert!(
                !name.contains("lock") && !name.contains("kill"),
                "{sentence} session dir has {name}"
            );
            names.push(name);
        }
        assert!(
            !names.iter().any(|name| name.ends_with(".lock")),
            "{sentence} names={names:?}"
        );
    }

    #[test]
    fn l1_resumes_same_l2_after_early_exit() {
        let l2_id = "l2-same-job";

        assert!(
            early_l2_exit(120, 0, false, false),
            "{EARLY_EXIT_SENTENCE} zero tool calls and no land report inside the window"
        );
        assert_resume_same(&l2_exit_action(l2_id, 120, 0, false, false), l2_id);

        assert!(
            early_l2_exit(60, 0, false, true),
            "{EARLY_EXIT_SENTENCE} repeating sentence before work"
        );
        assert_resume_same(&l2_exit_action(l2_id, 60, 0, false, true), l2_id);

        assert!(
            early_l2_exit(EARLY_L2_EXIT_WINDOW_SECS, 4, true, false),
            "{EARLY_EXIT_SENTENCE} few tool calls at the window edge"
        );
        assert_resume_same(
            &l2_exit_action(l2_id, EARLY_L2_EXIT_WINDOW_SECS, 4, true, false),
            l2_id,
        );

        assert!(
            early_l2_exit(90, 12, false, false),
            "{EARLY_EXIT_SENTENCE} no land report inside the window"
        );
        assert_resume_same(&l2_exit_action(l2_id, 90, 12, false, false), l2_id);

        assert!(
            early_l2_exit(90, 12, false, true),
            "{EARLY_EXIT_SENTENCE} repeating sentence and no land report"
        );
        assert_resume_same(&l2_exit_action(l2_id, 90, 12, false, true), l2_id);

        assert!(
            !early_l2_exit(30, EARLY_L2_FEW_TOOL_CALLS, true, false),
            "{EARLY_EXIT_SENTENCE} land report and enough tool calls, short elapsed"
        );
        assert_done(
            &l2_exit_action(l2_id, 30, EARLY_L2_FEW_TOOL_CALLS, true, false),
            l2_id,
        );

        assert!(
            !early_l2_exit(30, EARLY_L2_FEW_TOOL_CALLS, true, true),
            "{EARLY_EXIT_SENTENCE} land report and enough tool calls stays not early"
        );
        assert_done(
            &l2_exit_action(l2_id, 30, EARLY_L2_FEW_TOOL_CALLS, true, true),
            l2_id,
        );

        assert!(
            !early_l2_exit(181, 20, true, false),
            "{EARLY_EXIT_SENTENCE} a long run with a land report is not early"
        );
        assert_done(&l2_exit_action(l2_id, 181, 20, true, false), l2_id);

        assert!(
            !early_l2_exit(10_000, 0, true, false),
            "{EARLY_EXIT_SENTENCE} a long run with a land report is not early"
        );
        assert_done(&l2_exit_action(l2_id, 10_000, 0, true, false), l2_id);

        assert!(
            !early_l2_exit(181, 0, false, true),
            "{EARLY_EXIT_SENTENCE} outside the couple-of-minutes window"
        );
        assert_resume_same(&l2_exit_action(l2_id, 181, 0, false, true), l2_id);
    }

    #[test]
    fn soft_message_is_help_not_a_lock_or_a_kill() {
        let root = TempDir::new("soft", SOFT_MESSAGE_SENTENCE);
        let this_id = "l1-this";
        let other_id = "l1-other";
        let pid = std::process::id();
        let fixture = format!(
            r#"[
  {{
    "session_id": "{this_id}",
    "pid": {pid},
    "cwd": "/tmp/l1-harness-this",
    "opened_at": "2026-09-21T12:00:00Z",
    "activity": "working"
  }},
  {{
    "session_id": "{other_id}",
    "pid": {pid},
    "cwd": "/tmp/l1-harness-other",
    "opened_at": "2026-09-21T12:00:01Z",
    "activity": "working"
  }}
]"#
        );
        std::fs::write(root.path().join("active_sessions.json"), fixture).unwrap();

        let others = other_live_l1_session_ids(root.path(), this_id).unwrap();
        assert!(
            others.iter().any(|id| id == other_id),
            "{SOFT_MESSAGE_SENTENCE} others={others:?}"
        );
        assert!(
            !others.iter().any(|id| id == this_id),
            "{SOFT_MESSAGE_SENTENCE} others={others:?}"
        );

        let message = SoftMessage::help(
            this_id,
            other_id,
            format!("Soft help from this L1 about {LIVE_CHECK_REMOTE_RESOURCE}."),
            LIVE_CHECK_REMOTE_RESOURCE,
        );
        assert!(!message.is_lock(), "{SOFT_MESSAGE_SENTENCE}");
        assert!(!message.is_kill(), "{SOFT_MESSAGE_SENTENCE}");
        assert_eq!(
            message.resource, LIVE_CHECK_REMOTE_RESOURCE,
            "{SOFT_MESSAGE_SENTENCE}"
        );

        let session_dir = root.path().join("sessions").join(other_id);
        append_soft_message(&session_dir, &message).unwrap();

        let raw = std::fs::read_to_string(soft_messages_path(&session_dir)).unwrap();
        let lines: Vec<&str> = raw.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines.len(), 1, "{SOFT_MESSAGE_SENTENCE} raw={raw}");
        let value: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let obj = value.as_object().unwrap();
        assert!(
            obj.get("lock").is_none(),
            "{SOFT_MESSAGE_SENTENCE} raw={raw}"
        );
        assert!(
            obj.get("kill").is_none(),
            "{SOFT_MESSAGE_SENTENCE} raw={raw}"
        );
        assert_eq!(
            obj.get("resource").and_then(|item| item.as_str()),
            Some(LIVE_CHECK_REMOTE_RESOURCE),
            "{SOFT_MESSAGE_SENTENCE} raw={raw}"
        );
        assert_eq!(
            obj.get("from_session").and_then(|item| item.as_str()),
            Some(this_id),
            "{SOFT_MESSAGE_SENTENCE} raw={raw}"
        );
        assert_eq!(
            obj.get("to_session").and_then(|item| item.as_str()),
            Some(other_id),
            "{SOFT_MESSAGE_SENTENCE} raw={raw}"
        );

        let stored = read_soft_messages(&session_dir).unwrap();
        assert_eq!(stored.len(), 1, "{SOFT_MESSAGE_SENTENCE}");
        assert!(!stored[0].is_lock(), "{SOFT_MESSAGE_SENTENCE}");
        assert!(!stored[0].is_kill(), "{SOFT_MESSAGE_SENTENCE}");
        assert_eq!(stored[0], message, "{SOFT_MESSAGE_SENTENCE}");
        assert_session_dir_has_no_lock_or_kill(&session_dir, SOFT_MESSAGE_SENTENCE);
    }

    #[test]
    fn after_rebuild_this_l1_helps_an_l1_that_did_not_resume() {
        assert!(post_restart_check_is_allowed(), "{POST_RESTART_SENTENCE}");
        let this_l1 = L1ResumeRow {
            session_id: "l1-after-restart".to_string(),
            resumed_gracefully: None,
        };
        let others = vec![
            L1ResumeRow {
                session_id: "l1-did-not-resume".to_string(),
                resumed_gracefully: Some(false),
            },
            L1ResumeRow {
                session_id: "l1-also-did-not-resume".to_string(),
                resumed_gracefully: Some(false),
            },
            L1ResumeRow {
                session_id: "l1-resumed".to_string(),
                resumed_gracefully: Some(true),
            },
            L1ResumeRow {
                session_id: "l1-unknown".to_string(),
                resumed_gracefully: None,
            },
            L1ResumeRow {
                session_id: this_l1.session_id.clone(),
                resumed_gracefully: Some(false),
            },
        ];
        let messages = post_restart_help(&this_l1, &others);
        assert_eq!(messages.len(), 2, "{POST_RESTART_SENTENCE}");
        assert_eq!(
            messages[0].to_session, "l1-did-not-resume",
            "{POST_RESTART_SENTENCE}"
        );
        assert_eq!(
            messages[1].to_session, "l1-also-did-not-resume",
            "{POST_RESTART_SENTENCE}"
        );
        assert!(
            messages
                .iter()
                .all(|message| message.from_session == this_l1.session_id),
            "{POST_RESTART_SENTENCE}"
        );
        assert!(
            messages
                .iter()
                .all(|message| !message.is_lock() && !message.is_kill()),
            "{POST_RESTART_SENTENCE}"
        );
        assert!(
            !messages
                .iter()
                .any(|message| message.to_session == "l1-unknown"),
            "{POST_RESTART_SENTENCE} unknown must stay resumed"
        );
        assert!(
            !messages
                .iter()
                .any(|message| message.to_session == "l1-resumed"),
            "{POST_RESTART_SENTENCE}"
        );
        assert!(
            !messages
                .iter()
                .any(|message| message.to_session == this_l1.session_id),
            "{POST_RESTART_SENTENCE}"
        );

        let root = TempDir::new("restart", POST_RESTART_SENTENCE);
        for message in &messages {
            let session_dir = root.path().join("sessions").join(&message.to_session);
            append_soft_message(&session_dir, message).unwrap();
            let stored = read_soft_messages(&session_dir).unwrap();
            assert_eq!(stored.len(), 1, "{POST_RESTART_SENTENCE}");
            assert!(!stored[0].is_lock(), "{POST_RESTART_SENTENCE}");
            assert!(!stored[0].is_kill(), "{POST_RESTART_SENTENCE}");
            assert_session_dir_has_no_lock_or_kill(&session_dir, POST_RESTART_SENTENCE);
        }
    }

    #[test]
    fn harness_accepts_feedback_from_l1_l2_and_l3() {
        let root = TempDir::new("feedback", FEEDBACK_SENTENCE);
        let session_dir = root.path().join("sessions").join("l1-harness");
        let layers = [AgentLayer::L1, AgentLayer::L2, AgentLayer::L3];
        let failures = [
            HarnessFailure::EarlyExit,
            HarnessFailure::RepeatingSentence,
            HarnessFailure::MissingReport,
        ];
        for layer in layers {
            for failure in failures {
                append_harness_feedback(&session_dir, &HarnessFeedback { layer, failure }).unwrap();
            }
        }
        let stored = read_harness_feedback(&session_dir).unwrap();
        assert_eq!(stored.len(), 9, "{FEEDBACK_SENTENCE}");
        let mut index = 0;
        for layer in layers {
            for failure in failures {
                assert_eq!(stored[index].layer, layer, "{FEEDBACK_SENTENCE}");
                assert_eq!(stored[index].failure, failure, "{FEEDBACK_SENTENCE}");
                index += 1;
            }
        }
        let raw = std::fs::read_to_string(harness_feedback_path(&session_dir)).unwrap();
        assert!(
            raw.contains("\"layer\":\"L1\"")
                && raw.contains("\"layer\":\"L2\"")
                && raw.contains("\"layer\":\"L3\""),
            "{FEEDBACK_SENTENCE} raw={raw}"
        );
        assert!(
            raw.contains("\"failure\":\"EarlyExit\"")
                && raw.contains("\"failure\":\"RepeatingSentence\"")
                && raw.contains("\"failure\":\"MissingReport\""),
            "{FEEDBACK_SENTENCE} raw={raw}"
        );
        assert_session_dir_has_no_lock_or_kill(&session_dir, FEEDBACK_SENTENCE);
    }
}
