//! Safe L0 roster for the Surmount GPUI window.
//!
//! Parses `/running`-shaped JSON (`grok-oss running --json`). Keeps pid,
//! session id, cwd, and other safe window fields. Drops prompt text, tool
//! arguments, tokens, and JWTs. Tags each row local or remote. The enqueue
//! drop-file path is per session id.
//!
//! This crate is a library for that GPUI app. It is not a grok-oss TUI
//! dashboard. `/dashboard` stays the pager Agent Dashboard. `/running`
//! stays this machine's grok-oss windows.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// Machine the grok-oss window is on. Local is this host. Remote is a
/// named other host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RosterHost {
    Local,
    Remote(String),
}

impl Serialize for RosterHost {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Local => serializer.serialize_str("local"),
            Self::Remote(name) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("remote", name)?;
                map.end()
            }
        }
    }
}

/// One live grok-oss window after secret fields are stripped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RosterRow {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opened_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_line: Option<String>,
    pub host: RosterHost,
}

/// Why `/running` JSON could not become a roster.
#[derive(Debug)]
pub enum RosterError {
    Json(serde_json::Error),
    NotAnArray,
    MissingIdentity { index: usize, field: &'static str },
}

impl fmt::Display for RosterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "running JSON is not valid: {err}"),
            Self::NotAnArray => write!(f, "running JSON must be an array of window rows"),
            Self::MissingIdentity { index, field } => {
                write!(f, "running JSON row {index} is missing {field}")
            }
        }
    }
}

impl std::error::Error for RosterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for RosterError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

/// Parse `/running --json` as a local-host roster.
pub fn parse_running_json(input: &str) -> Result<Vec<RosterRow>, RosterError> {
    parse_running_json_with_host(input, RosterHost::Local)
}

/// Parse `/running --json` and tag rows with `default_host` when the row
/// does not already name a host.
pub fn parse_running_json_with_host(
    input: &str,
    default_host: RosterHost,
) -> Result<Vec<RosterRow>, RosterError> {
    let value: Value = serde_json::from_str(input)?;
    let Value::Array(rows) = value else {
        return Err(RosterError::NotAnArray);
    };
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        out.push(row_from_value(row, index, &default_host)?);
    }
    Ok(out)
}

/// Pretty JSON of safe roster rows only.
pub fn format_roster_json(rows: &[RosterRow]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(rows)
}

const ENQUEUE_DIR: &str = "l0-enqueue";
const ENQUEUE_FILE: &str = "enqueue.json";

/// Path where the GPUI app drops an enqueue file for one session.
///
/// `{grok_home}/l0-enqueue/{session_id}/enqueue.json`. `None` when
/// `session_id` is empty or not a single path component.
pub fn enqueue_drop_path(grok_home: &Path, session_id: &str) -> Option<PathBuf> {
    let sid = sanitize_session_id(session_id)?;
    Some(grok_home.join(ENQUEUE_DIR).join(sid).join(ENQUEUE_FILE))
}

fn sanitize_session_id(session_id: &str) -> Option<&str> {
    let sid = session_id.trim();
    if sid.is_empty() || sid == "." || sid == ".." {
        return None;
    }
    if sid.contains('/') || sid.contains('\\') || sid.contains('\0') {
        return None;
    }
    Some(sid)
}

fn row_from_value(
    value: Value,
    index: usize,
    default_host: &RosterHost,
) -> Result<RosterRow, RosterError> {
    let Value::Object(mut map) = value else {
        return Err(RosterError::MissingIdentity {
            index,
            field: "object",
        });
    };
    let pid = take_pid(&mut map, index)?;
    let session_id = take_required_string(&mut map, index, "session_id")?;
    let cwd = take_required_string(&mut map, index, "cwd")?;
    let host = take_host(&mut map).unwrap_or_else(|| default_host.clone());
    let opened_at = take_safe_string(&mut map, "opened_at");
    let updated_at = take_safe_string(&mut map, "updated_at");
    let activity = take_safe_string(&mut map, "activity");
    let title = take_safe_string(&mut map, "title");
    let activity_line = take_safe_string(&mut map, "activity_line");
    Ok(RosterRow {
        pid,
        session_id,
        cwd,
        opened_at,
        updated_at,
        activity,
        title,
        activity_line,
        host,
    })
}

fn take_pid(map: &mut serde_json::Map<String, Value>, index: usize) -> Result<u32, RosterError> {
    match map.remove("pid") {
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| RosterError::MissingIdentity {
                index,
                field: "pid",
            }),
        _ => Err(RosterError::MissingIdentity {
            index,
            field: "pid",
        }),
    }
}

fn take_required_string(
    map: &mut serde_json::Map<String, Value>,
    index: usize,
    field: &'static str,
) -> Result<String, RosterError> {
    match map.remove(field) {
        Some(Value::String(s)) => Ok(s),
        _ => Err(RosterError::MissingIdentity { index, field }),
    }
}

fn take_safe_string(map: &mut serde_json::Map<String, Value>, field: &str) -> Option<String> {
    match map.remove(field) {
        Some(Value::String(s)) if !s.trim().is_empty() && !is_secret_string(&s) => Some(s),
        _ => None,
    }
}

fn take_host(map: &mut serde_json::Map<String, Value>) -> Option<RosterHost> {
    match map.remove("host") {
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.eq_ignore_ascii_case("local") {
                Some(RosterHost::Local)
            } else {
                Some(RosterHost::Remote(trimmed.to_string()))
            }
        }
        Some(Value::Object(obj)) => obj
            .get("remote")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| RosterHost::Remote(s.to_string())),
        _ => None,
    }
}

fn is_secret_string(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() >= 20 && trimmed.to_ascii_lowercase().starts_with("bearer ") {
        return true;
    }
    looks_like_jwt(trimmed)
}

fn looks_like_jwt(value: &str) -> bool {
    if !value.starts_with("eyJ") {
        return false;
    }
    let mut dots = 0usize;
    for ch in value.chars() {
        if ch == '.' {
            dots += 1;
            if dots > 2 {
                return false;
            }
        } else if !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-' {
            return false;
        }
    }
    dots == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLANTED_PROMPT: &str = "SECRET_PLEASE_IMPLEMENT_THE_LOGIN_FLOW";
    const PLANTED_TOOL: &str = "cat /etc/shadow";
    const PLANTED_JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0In0.abc";

    fn planted_running_json() -> String {
        format!(
            r#"[
  {{
    "pid": 4242,
    "session_id": "keep-session",
    "cwd": "/tmp/keep-cwd",
    "opened_at": "2026-08-16T12:00:00Z",
    "updated_at": "2026-08-16T12:01:00Z",
    "activity": "working",
    "title": "on-disk summary title",
    "activity_line": "turn running",
    "prompt": "{PLANTED_PROMPT}",
    "Prompt": "Another Prompt Variant",
    "tool_arguments": {{"cmd": "{PLANTED_TOOL}"}},
    "tokens": 99999,
    "jwt": "{PLANTED_JWT}",
    "authorization": "Bearer {PLANTED_JWT}"
  }}
]"#
        )
    }

    #[test]
    fn roster_omits_prompt_text() {
        let rows = parse_running_json(&planted_running_json()).unwrap();
        let json = format_roster_json(&rows).unwrap();
        let lower = json.to_ascii_lowercase();
        for needle in [
            PLANTED_PROMPT.to_ascii_lowercase(),
            "another prompt variant".to_string(),
            PLANTED_TOOL.to_string(),
            "tool_arguments".to_string(),
            "\"tokens\"".to_string(),
            "99999".to_string(),
            PLANTED_JWT.to_ascii_lowercase(),
            "bearer ".to_string(),
            "\"prompt\"".to_string(),
            "\"jwt\"".to_string(),
            "\"authorization\"".to_string(),
        ] {
            assert!(
                !lower.contains(&needle),
                "roster JSON must omit prompt text, tool arguments, tokens, and JWTs; found {needle:?} in {json}"
            );
        }
    }

    #[test]
    fn roster_keeps_pid_session_cwd() {
        let rows = parse_running_json_with_host(
            &planted_running_json(),
            RosterHost::Remote("surmount-1".to_string()),
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.pid, 4242);
        assert_eq!(row.session_id, "keep-session");
        assert_eq!(row.cwd, "/tmp/keep-cwd");
        assert_eq!(row.title.as_deref(), Some("on-disk summary title"));
        assert_eq!(row.activity.as_deref(), Some("working"));
        assert_eq!(
            row.host,
            RosterHost::Remote("surmount-1".to_string()),
            "missing host on /running JSON uses the local-or-remote argument"
        );
        let json = format_roster_json(&rows).unwrap();
        assert!(json.contains("4242"));
        assert!(json.contains("keep-session"));
        assert!(json.contains("/tmp/keep-cwd"));
        assert!(json.contains("surmount-1"));
        assert!(json.contains("\"remote\""));
    }

    #[test]
    fn enqueue_drop_path_is_per_session_id() {
        let home = Path::new("/tmp/grok-home-l0");
        let a = enqueue_drop_path(home, "sess-aaa").expect("safe session id");
        let b = enqueue_drop_path(home, "sess-bbb").expect("safe session id");
        assert_ne!(a, b);
        assert_eq!(
            a,
            PathBuf::from("/tmp/grok-home-l0/l0-enqueue/sess-aaa/enqueue.json")
        );
        assert_eq!(
            b,
            PathBuf::from("/tmp/grok-home-l0/l0-enqueue/sess-bbb/enqueue.json")
        );
        assert!(
            a.to_string_lossy().contains("sess-aaa") && !a.to_string_lossy().contains("sess-bbb")
        );
        assert!(enqueue_drop_path(home, "../escape").is_none());
        assert!(enqueue_drop_path(home, "sess/nested").is_none());
        assert!(enqueue_drop_path(home, "").is_none());
    }
}
