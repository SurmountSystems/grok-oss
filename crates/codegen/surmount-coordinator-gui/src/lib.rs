//! Application state the Surmount GPUI L0 window will call.
//!
//! Parses `/running`-shaped JSON (`grok-oss running --json`). Keeps pid,
//! session id, cwd, and other safe fields. Drops prompt text, tool
//! arguments, tokens, and JWTs. Tags each row local or remote. Writes a
//! per-session enqueue drop file. `CoordinatorApp` holds the session list
//! and selected index for that window. The laptop-side action
//! **set remote host console API key** writes a staging file for a
//! machine xAI console API key (console API credits / console team
//! prepaid). It never prints the key. It does not open git on the guest.
//!
//! This crate is not a grok-oss TUI dashboard. `/dashboard` stays the
//! pager Agent Dashboard. `/running` stays this machine's grok-oss
//! sessions. The `surmount-coordinator-gui` binary reads `/running --json`
//! and prints safe JSON. It is not a TUI. This crate does not depend on
//! gpui.

use std::fmt;
use std::path::{Path, PathBuf};

mod cli;
mod remote_console_key;

pub use cli::run_cli;
pub use remote_console_key::{
    DEFAULT_GUEST_GROK_HOME, HostFileInstall, RemoteHostConsoleKeyError,
    RemoteHostConsoleKeyReport, SET_REMOTE_HOST_CONSOLE_API_KEY_ACTION, SshDeployInstall,
    SshInstallSpec, SshRequest, scp_copy_argv, set_remote_host_console_api_key, ssh_chmod_argv,
};

use serde::Serialize;
use serde_json::Value;

/// Machine the grok-oss session is on. Local is this host. Remote is a
/// named other host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionHost {
    Local,
    Remote(String),
}

impl Serialize for SessionHost {
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

/// One live grok-oss session after secret fields are stripped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunningSession {
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
    pub host: SessionHost,
}

/// Why `/running` JSON could not become safe session rows.
#[derive(Debug)]
pub enum RunningJsonError {
    Json(serde_json::Error),
    NotAnArray,
    MissingIdentity { index: usize, field: &'static str },
}

impl fmt::Display for RunningJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "running JSON is not valid: {err}"),
            Self::NotAnArray => write!(f, "running JSON must be an array of session rows"),
            Self::MissingIdentity { index, field } => {
                write!(f, "running JSON row {index} is missing {field}")
            }
        }
    }
}

impl std::error::Error for RunningJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for RunningJsonError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

/// Why an enqueue drop file could not be written.
#[derive(Debug)]
pub enum EnqueueError {
    UnsafeSessionId,
    NoSessionSelected,
    /// Selected row is a remote host. This laptop grok home cannot drain that
    /// session. Do not write a local drop file that looks like it will fire.
    RemoteHost {
        host: String,
    },
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for EnqueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeSessionId => {
                write!(f, "session id is empty or not a single path component")
            }
            Self::NoSessionSelected => write!(f, "no running session is selected"),
            Self::RemoteHost { host } => write!(
                f,
                "session is on remote host {host}; this laptop grok home cannot drain that enqueue"
            ),
            Self::Io(err) => write!(f, "could not write enqueue drop file: {err}"),
            Self::Json(err) => write!(f, "could not encode enqueue drop file: {err}"),
        }
    }
}

impl std::error::Error for EnqueueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for EnqueueError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for EnqueueError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

/// Session list and selected row for the GPUI window.
#[derive(Debug, Clone)]
pub struct CoordinatorApp {
    grok_home: PathBuf,
    sessions: Vec<RunningSession>,
    selected: usize,
}

impl CoordinatorApp {
    /// Load local `/running --json`. When `remote` is `Some((host, json))`,
    /// append that host's rows tagged remote.
    pub fn load(
        grok_home: impl Into<PathBuf>,
        local_json: &str,
        remote: Option<(&str, &str)>,
    ) -> Result<Self, RunningJsonError> {
        let mut sessions = parse_running_json(local_json)?;
        if let Some((host, json)) = remote {
            let tagged = parse_running_json_with_host(json, SessionHost::Remote(host.to_string()))?;
            sessions.extend(tagged);
        }
        Ok(Self {
            grok_home: grok_home.into(),
            sessions,
            selected: 0,
        })
    }

    pub fn grok_home(&self) -> &Path {
        &self.grok_home
    }

    pub fn sessions(&self) -> &[RunningSession] {
        &self.sessions
    }

    /// Fields the window may show. Prompt text is not among them.
    pub fn displayed_fields(&self) -> &[RunningSession] {
        &self.sessions
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected(&self) -> Option<&RunningSession> {
        self.sessions.get(self.selected)
    }

    /// Select a row. Ignores an index that is not in the list.
    pub fn select(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.selected = index;
        }
    }

    /// Write the enqueue drop file for the selected **local** session.
    /// Remote-tagged rows error instead of writing a laptop drop file that
    /// the remote grok-oss window will never drain.
    pub fn enqueue_selected(&self, prompt: &str) -> Result<PathBuf, EnqueueError> {
        let session = self.selected().ok_or(EnqueueError::NoSessionSelected)?;
        if let SessionHost::Remote(host) = &session.host {
            return Err(EnqueueError::RemoteHost { host: host.clone() });
        }
        write_enqueue(&self.grok_home, &session.session_id, prompt)
    }

    /// Laptop-side action: set a machine console API key for a remote host.
    /// Writes staging files under this laptop grok home. Never prints the key.
    pub fn set_remote_host_console_api_key(
        &self,
        host: &str,
        key: &str,
    ) -> Result<RemoteHostConsoleKeyReport, RemoteHostConsoleKeyError> {
        crate::remote_console_key::set_remote_host_console_api_key(&self.grok_home, host, key, None)
    }
}

/// Parse `/running --json` as local-host sessions.
pub fn parse_running_json(input: &str) -> Result<Vec<RunningSession>, RunningJsonError> {
    parse_running_json_with_host(input, SessionHost::Local)
}

/// Parse `/running --json` and tag rows with `default_host` when the row
/// does not already name a host.
pub fn parse_running_json_with_host(
    input: &str,
    default_host: SessionHost,
) -> Result<Vec<RunningSession>, RunningJsonError> {
    let value: Value = serde_json::from_str(input)?;
    let Value::Array(rows) = value else {
        return Err(RunningJsonError::NotAnArray);
    };
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        out.push(row_from_value(row, index, &default_host)?);
    }
    Ok(out)
}

/// Pretty JSON of safe session rows only.
pub fn format_running_sessions_json(rows: &[RunningSession]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(rows)
}

/// Parse `/running --json` and return safe pretty JSON (no prompt).
pub fn safe_json_from_running(
    input: &str,
    default_host: SessionHost,
) -> Result<String, RunningJsonError> {
    let rows = parse_running_json_with_host(input, default_host)?;
    format_running_sessions_json(&rows).map_err(RunningJsonError::from)
}

const ENQUEUE_DIR: &str = "l0-enqueue";
const ENQUEUE_FILE: &str = "enqueue.json";

#[derive(Serialize)]
struct EnqueueDrop<'a> {
    prompt: &'a str,
}

/// Path where the GPUI app drops an enqueue file for one session.
///
/// `{grok_home}/l0-enqueue/{session_id}/enqueue.json`. `None` when
/// `session_id` is empty or not a single path component.
pub fn enqueue_drop_path(grok_home: &Path, session_id: &str) -> Option<PathBuf> {
    let sid = sanitize_session_id(session_id)?;
    Some(grok_home.join(ENQUEUE_DIR).join(sid).join(ENQUEUE_FILE))
}

/// Write `{grok_home}/l0-enqueue/{session_id}/enqueue.json` with `prompt`.
pub fn write_enqueue(
    grok_home: &Path,
    session_id: &str,
    prompt: &str,
) -> Result<PathBuf, EnqueueError> {
    let path = enqueue_drop_path(grok_home, session_id).ok_or(EnqueueError::UnsafeSessionId)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(&EnqueueDrop { prompt })?;
    std::fs::write(&path, body)?;
    Ok(path)
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
    default_host: &SessionHost,
) -> Result<RunningSession, RunningJsonError> {
    let Value::Object(mut map) = value else {
        return Err(RunningJsonError::MissingIdentity {
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
    Ok(RunningSession {
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

fn take_pid(
    map: &mut serde_json::Map<String, Value>,
    index: usize,
) -> Result<u32, RunningJsonError> {
    match map.remove("pid") {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or(
            RunningJsonError::MissingIdentity {
                index,
                field: "pid",
            },
        ),
        _ => Err(RunningJsonError::MissingIdentity {
            index,
            field: "pid",
        }),
    }
}

fn take_required_string(
    map: &mut serde_json::Map<String, Value>,
    index: usize,
    field: &'static str,
) -> Result<String, RunningJsonError> {
    match map.remove(field) {
        Some(Value::String(s)) => Ok(s),
        _ => Err(RunningJsonError::MissingIdentity { index, field }),
    }
}

fn take_safe_string(map: &mut serde_json::Map<String, Value>, field: &str) -> Option<String> {
    match map.remove(field) {
        Some(Value::String(s)) if !s.trim().is_empty() && !is_secret_string(&s) => Some(s),
        _ => None,
    }
}

fn take_host(map: &mut serde_json::Map<String, Value>) -> Option<SessionHost> {
    match map.remove("host") {
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.eq_ignore_ascii_case("local") {
                Some(SessionHost::Local)
            } else {
                Some(SessionHost::Remote(trimmed.to_string()))
            }
        }
        Some(Value::Object(obj)) => obj
            .get("remote")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| SessionHost::Remote(s.to_string())),
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
    use super::{
        CoordinatorApp, SessionHost, enqueue_drop_path, format_running_sessions_json,
        parse_running_json, parse_running_json_with_host, write_enqueue,
    };
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

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

    fn two_session_json() -> &'static str {
        r#"[
  {"pid": 1, "session_id": "sess-aaa", "cwd": "/tmp/a"},
  {"pid": 2, "session_id": "sess-bbb", "cwd": "/tmp/b", "prompt": "PLANTED_DISPLAY_PROMPT"}
]"#
    }

    fn test_home() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "surmount-coordinator-gui-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).expect("temp grok home");
        p
    }

    #[test]
    fn omits_prompt_text() {
        let rows = parse_running_json(&planted_running_json()).unwrap();
        let json = format_running_sessions_json(&rows).unwrap();
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
                "safe JSON must omit prompt text, tool arguments, tokens, and JWTs; found {needle:?} in {json}"
            );
        }
    }

    #[test]
    fn keeps_pid_session_cwd() {
        let rows = parse_running_json_with_host(
            &planted_running_json(),
            SessionHost::Remote("surmount-1".to_string()),
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
            SessionHost::Remote("surmount-1".to_string()),
            "missing host on /running JSON uses the local-or-remote argument"
        );
        let json = format_running_sessions_json(&rows).unwrap();
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

    #[test]
    fn write_enqueue_creates_per_session_file() {
        let home = test_home();
        let a = write_enqueue(&home, "sess-aaa", "prompt-aaa").expect("write a");
        let b = write_enqueue(&home, "sess-bbb", "prompt-bbb").expect("write b");
        assert_eq!(a, home.join("l0-enqueue/sess-aaa/enqueue.json"));
        assert_eq!(b, home.join("l0-enqueue/sess-bbb/enqueue.json"));
        assert_ne!(a, b);
        let body_a = fs::read_to_string(&a).expect("read a");
        let body_b = fs::read_to_string(&b).expect("read b");
        let val_a: Value = serde_json::from_str(&body_a).unwrap();
        let val_b: Value = serde_json::from_str(&body_b).unwrap();
        assert_eq!(val_a["prompt"], "prompt-aaa");
        assert_eq!(val_b["prompt"], "prompt-bbb");
        assert!(!body_a.contains("prompt-bbb"));
        assert!(write_enqueue(&home, "../escape", "nope").is_err());
        let _ = fs::remove_dir_all(&home);
    }

    #[allow(non_snake_case)]
    #[test]
    fn CoordinatorApp_selects_row() {
        let home = test_home();
        let mut app = CoordinatorApp::load(&home, two_session_json(), None).unwrap();
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.selected().unwrap().session_id, "sess-aaa");
        app.select(1);
        assert_eq!(app.selected_index(), 1);
        let row = app.selected().expect("selected row");
        assert_eq!(row.session_id, "sess-bbb");
        assert_eq!(row.pid, 2);
        app.select(99);
        assert_eq!(app.selected_index(), 1);
        let remote_json = r#"[{"pid": 9, "session_id": "sess-remote", "cwd": "/tmp/r"}]"#;
        let app =
            CoordinatorApp::load(&home, two_session_json(), Some(("surmount-1", remote_json)))
                .unwrap();
        assert_eq!(app.sessions().len(), 3);
        assert_eq!(app.sessions()[0].host, SessionHost::Local);
        assert_eq!(
            app.sessions()[2].host,
            SessionHost::Remote("surmount-1".to_string())
        );
        let _ = fs::remove_dir_all(&home);
    }

    #[allow(non_snake_case)]
    #[test]
    fn CoordinatorApp_omits_prompt_in_displayed_fields() {
        let home = test_home();
        let app = CoordinatorApp::load(&home, &planted_running_json(), None).unwrap();
        let json = format_running_sessions_json(app.displayed_fields()).unwrap();
        let lower = json.to_ascii_lowercase();
        assert!(
            !lower.contains(&PLANTED_PROMPT.to_ascii_lowercase()),
            "displayed fields must omit prompt text; found planted prompt in {json}"
        );
        assert!(
            !lower.contains("\"prompt\""),
            "displayed fields must not name a prompt key; got {json}"
        );
        assert!(json.contains("keep-session"));
        let _ = fs::remove_dir_all(&home);
    }

    #[allow(non_snake_case)]
    #[test]
    fn CoordinatorApp_enqueue_writes_drop_file() {
        let home = test_home();
        let mut app = CoordinatorApp::load(&home, two_session_json(), None).unwrap();
        app.select(1);
        let path = app
            .enqueue_selected("do the selected work")
            .expect("enqueue selected");
        assert_eq!(path, home.join("l0-enqueue/sess-bbb/enqueue.json"));
        let body = fs::read_to_string(&path).expect("read drop file");
        let val: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(val["prompt"], "do the selected work");
        assert!(!home.join("l0-enqueue/sess-aaa/enqueue.json").exists());
        let _ = fs::remove_dir_all(&home);
    }

    /// Named contract: a remote-tagged selected session must not write a
    /// laptop drop file as if this host will drain it.
    #[allow(non_snake_case)]
    #[test]
    fn CoordinatorApp_enqueue_remote_session_does_not_write_local_drop_file() {
        let home = test_home();
        let remote_json = r#"[{"pid": 9, "session_id": "sess-remote", "cwd": "/tmp/r"}]"#;
        let mut app =
            CoordinatorApp::load(&home, two_session_json(), Some(("surmount-1", remote_json)))
                .unwrap();
        app.select(2);
        assert_eq!(
            app.selected().unwrap().host,
            SessionHost::Remote("surmount-1".to_string())
        );
        let err = app
            .enqueue_selected("do the remote work")
            .expect_err("remote enqueue");
        match err {
            crate::EnqueueError::RemoteHost { host } => {
                assert_eq!(host, "surmount-1");
            }
            other => panic!("expected RemoteHost, got {other}"),
        }
        assert!(
            !home.join("l0-enqueue/sess-remote/enqueue.json").exists(),
            "must not write a local drop file for a remote session"
        );
        let _ = fs::remove_dir_all(&home);
    }
}
