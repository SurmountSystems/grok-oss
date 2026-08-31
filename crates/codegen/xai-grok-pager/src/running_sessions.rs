//! Live grok-oss window list shared by `/running` and `grok-oss running`.
//!
//! Filter is [`xai_grok_active_sessions::list_live_in`] then
//! [`xai_grok_shell::util::is_grok_process`]. Rows expose only safe fields.

use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use xai_grok_active_sessions::{ActiveSession, SessionActivity};

/// Safe report row for slash and CLI. Optional heartbeat fields are filled
/// from the same registry row on both paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunningSessionRow {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub opened_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub activity: SessionActivity,
    pub title: Option<String>,
    pub activity_line: Option<String>,
}

impl From<&ActiveSession> for RunningSessionRow {
    fn from(session: &ActiveSession) -> Self {
        Self {
            pid: session.pid,
            session_id: session.session_id.0.to_string(),
            cwd: session.cwd.clone(),
            opened_at: session.opened_at,
            updated_at: session.updated_at,
            activity: session.activity,
            title: session.title.clone(),
            activity_line: session.activity_line.clone(),
        }
    }
}

/// Flock-safe live grok-oss rows under the default grok home.
pub fn list_running_sessions() -> io::Result<Vec<RunningSessionRow>> {
    list_running_sessions_in(&xai_grok_config::grok_home())
}

/// Flock-safe live grok-oss rows under `root`.
///
/// Filter is [`xai_grok_shell::util::is_grok_process`] (Linux `/proc` cmdline
/// contains `grok`). That is not
/// [`xai_grok_active_sessions::is_grok_oss_cli_identity`]: `/rebuild` SIGUSR1
/// uses the grok-oss CLI/exe classifier so a stock `grok` comm is not signaled.
pub fn list_running_sessions_in(root: &Path) -> io::Result<Vec<RunningSessionRow>> {
    let live = xai_grok_active_sessions::list_live_in(root)?;
    let mut rows: Vec<RunningSessionRow> = live
        .iter()
        .filter(|session| xai_grok_shell::util::is_grok_process(session.pid))
        .map(RunningSessionRow::from)
        .collect();
    rows.sort_by(|a, b| {
        a.opened_at
            .cmp(&b.opened_at)
            .then(a.pid.cmp(&b.pid))
            .then(a.session_id.cmp(&b.session_id))
    });
    Ok(rows)
}

/// Transcript / human table. `this_pid` marks the calling TUI window.
pub fn format_table(rows: &[RunningSessionRow], this_pid: Option<u32>) -> String {
    if rows.is_empty() {
        return "No running grok-oss sessions.".to_string();
    }
    let header = format!(
        "Running grok-oss session{} ({}):",
        if rows.len() == 1 { "" } else { "s" },
        rows.len()
    );
    let body = rows
        .iter()
        .map(|row| format_row(row, this_pid))
        .collect::<Vec<_>>();
    std::iter::once(header)
        .chain(body)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Same filtered rows as the table, safe fields only.
pub fn format_json(rows: &[RunningSessionRow]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(rows)
}

/// Slash `/running` report for the default grok home.
pub fn slash_report() -> String {
    slash_report_in(&xai_grok_config::grok_home(), Some(std::process::id()))
}

/// Slash `/running` report for an injectable registry root.
pub fn slash_report_in(root: &Path, this_pid: Option<u32>) -> String {
    match list_running_sessions_in(root) {
        Ok(rows) => format_table(&rows, this_pid),
        Err(e) => format!("Couldn't list running grok-oss sessions: {e}"),
    }
}

/// `grok-oss running` and `grok-oss running --json`.
pub fn run_cli(json: bool) -> Result<()> {
    let rows = list_running_sessions()?;
    let rendered = if json {
        format_json(&rows)?
    } else {
        format_table(&rows, None)
    };
    let written = writeln!(io::stdout(), "{rendered}");
    Ok(crate::util::ignore_broken_pipe(written)?)
}

fn format_row(row: &RunningSessionRow, this_pid: Option<u32>) -> String {
    let mut parts = vec![
        format!("  {}", row.pid),
        short_session_id(&row.session_id),
        row.cwd.clone(),
        row.opened_at.to_rfc3339(),
        activity_label(row.activity).to_string(),
    ];
    if let Some(title) = nonempty_opt(row.title.as_deref()) {
        parts.push(title.to_string());
    }
    if let Some(line) = nonempty_opt(row.activity_line.as_deref()) {
        parts.push(line.to_string());
    }
    if this_pid == Some(row.pid) {
        parts.push("(this window)".to_string());
    }
    parts.join("  ")
}

fn short_session_id(id: &str) -> String {
    const SHORT: usize = 8;
    if id.chars().count() <= SHORT {
        id.to_string()
    } else {
        id.chars().take(SHORT).collect()
    }
}

fn activity_label(activity: SessionActivity) -> &'static str {
    match activity {
        SessionActivity::Working => "working",
        SessionActivity::Idle => "idle",
        SessionActivity::Unknown => "unknown",
    }
}

fn nonempty_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::cli::{Command, PagerArgs};
    use clap::Parser;

    #[test]
    fn running_cli_json_omits_prompt_text() {
        let dir = tempfile::tempdir().unwrap();
        let self_pid = std::process::id();
        let fixture = format!(
            r#"[
  {{
    "session_id": "cli-json-sibling",
    "pid": {self_pid},
    "cwd": "/tmp/running-cli-json-cwd",
    "opened_at": "2026-08-16T12:00:00Z",
    "title": "on-disk summary title",
    "activity": "unknown",
    "prompt": "SECRET_PLEASE_IMPLEMENT_THE_LOGIN_FLOW",
    "Prompt": "Another Prompt Variant",
    "tool_arguments": {{"cmd": "cat /etc/shadow"}}
  }}
]"#
        );
        std::fs::write(dir.path().join("active_sessions.json"), fixture).unwrap();
        let rows = list_running_sessions_in(dir.path()).unwrap();
        assert!(
            rows.iter().any(|r| r.session_id == "cli-json-sibling"),
            "CLI rows must include the planted live grok window: {rows:?}"
        );
        let json = format_json(&rows).unwrap();
        assert!(json.contains("cli-json-sibling"));
        assert!(json.contains("/tmp/running-cli-json-cwd"));
        assert!(json.contains(&self_pid.to_string()));
        assert!(json.contains("on-disk summary title"));
        assert!(json.contains("\"activity\""));
        assert!(json.contains("\"updated_at\""));
        assert!(json.contains("\"activity_line\""));
        let lower = json.to_ascii_lowercase();
        for needle in [
            "secret_please_implement_the_login_flow",
            "another prompt variant",
            "tool_arguments",
            "cat /etc/shadow",
            "bearer ",
        ] {
            assert!(
                !lower.contains(needle),
                "CLI JSON must omit prompt text and private fields \
                 (case-insensitive); found {needle:?} in {json}"
            );
        }
    }

    #[test]
    fn cli_running_parses_json_flag() {
        let plain = PagerArgs::try_parse_from(["grok-oss", "running"]).unwrap();
        assert!(matches!(
            plain.command,
            Some(Command::Running { json: false })
        ));
        let json = PagerArgs::try_parse_from(["grok-oss", "running", "--json"]).unwrap();
        assert!(matches!(
            json.command,
            Some(Command::Running { json: true })
        ));
        let sessions = PagerArgs::try_parse_from(["grok-oss", "sessions", "list"]).unwrap();
        assert!(
            matches!(sessions.command, Some(Command::Sessions(_))),
            "running must not overload sessions"
        );
    }
}
