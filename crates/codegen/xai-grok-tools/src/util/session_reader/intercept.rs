//! Parse `run_terminal_command` strings that invoke host `session_reader.py`.

/// Allowlisted script path segment (host resume-session skill + bundled mirror).
const SESSION_READER_PY_MARKER: &str = "resume-session/session_reader.py";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTool {
    Claude,
    Codex,
    Cursor,
}

impl SessionTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "cursor" => Some(Self::Cursor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    List,
    Show,
}

impl SessionAction {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "list" => Some(Self::List),
            "show" => Some(Self::Show),
            _ => None,
        }
    }
}

/// Parsed allowlisted session_reader.py invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReaderIntercept {
    pub script_path: String,
    pub tool: SessionTool,
    pub action: SessionAction,
    pub ref_arg: Option<String>,
    /// Explicit `--cwd` from the command line. `None` means default to the
    /// bash tool working directory (host `os.getcwd()` of the shell).
    pub cwd: Option<String>,
    pub within_min: i64,
    pub json: bool,
    pub max_tool_chars: usize,
}

/// Returns [`Some`] when `command` is a known resume-session `session_reader.py` call.
pub fn try_parse_session_reader_intercept(command: &str) -> Option<SessionReaderIntercept> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    try_parse_direct(trimmed)
}

fn is_python_bin(tok: &str) -> bool {
    matches!(
        tok,
        "python3"
            | "python"
            | "python3.11"
            | "python3.12"
            | "python3.13"
            | "python3.10"
            | "python3.9"
    ) || {
        let base = std::path::Path::new(tok)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(tok);
        base == "python3" || base == "python" || (base.starts_with("python3.") && base.len() <= 12)
    }
}

fn is_allowlisted(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.ends_with(SESSION_READER_PY_MARKER) || p.contains(&format!("/{SESSION_READER_PY_MARKER}"))
}

fn simple_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '\'' => {
                for c2 in chars.by_ref() {
                    if c2 == '\'' {
                        break;
                    }
                    cur.push(c2);
                }
            }
            '"' => {
                while let Some(c2) = chars.next() {
                    if c2 == '"' {
                        break;
                    }
                    if c2 == '\\' {
                        if let Some(n) = chars.next() {
                            cur.push(n);
                        }
                    } else {
                        cur.push(c2);
                    }
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn try_parse_direct(cmd: &str) -> Option<SessionReaderIntercept> {
    let tokens = simple_tokens(cmd);
    if tokens.len() < 4 {
        return None;
    }

    let mut i = 0;
    while i < tokens.len() && tokens[i].contains('=') && !tokens[i].starts_with('-') {
        let t = &tokens[i];
        if let Some((name, _)) = t.split_once('=') {
            if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !name.is_empty() {
                i += 1;
                continue;
            }
        }
        break;
    }

    if i >= tokens.len() || !is_python_bin(&tokens[i]) {
        return None;
    }
    i += 1;

    while i < tokens.len() && tokens[i].starts_with('-') && tokens[i] != "-" {
        if tokens[i] == "-c" || tokens[i].starts_with("-c") {
            return None;
        }
        i += 1;
    }

    if i >= tokens.len() || !is_allowlisted(&tokens[i]) {
        return None;
    }
    let script = tokens[i].clone();
    i += 1;

    if i >= tokens.len() {
        return None;
    }
    let tool = SessionTool::parse(&tokens[i])?;
    i += 1;

    if i >= tokens.len() {
        return None;
    }
    let action = SessionAction::parse(&tokens[i])?;
    i += 1;

    // Optional positional ref (not starting with -)
    let mut ref_arg = None;
    if i < tokens.len() && !tokens[i].starts_with('-') {
        ref_arg = Some(tokens[i].clone());
        i += 1;
    }

    // None until `--cwd` is seen; execute_intercept fills bash tool cwd.
    let mut cwd: Option<String> = None;
    let mut within_min: i64 = 0;
    let mut json = false;
    let mut max_tool_chars: usize = 300;

    while i < tokens.len() {
        let t = &tokens[i];
        match t.as_str() {
            "--cwd" => {
                i += 1;
                if i >= tokens.len() {
                    return None;
                }
                cwd = Some(tokens[i].clone());
                i += 1;
            }
            "--within-min" => {
                i += 1;
                if i >= tokens.len() {
                    return None;
                }
                within_min = tokens[i].parse().ok()?;
                i += 1;
            }
            "--json" => {
                json = true;
                i += 1;
            }
            "--max-tool-chars" => {
                i += 1;
                if i >= tokens.len() {
                    return None;
                }
                max_tool_chars = tokens[i].parse().ok()?;
                i += 1;
            }
            t if t.starts_with('>') || t == "2>&1" || t.starts_with("2>") => {
                i += 1;
            }
            _ => return None,
        }
    }

    Some(SessionReaderIntercept {
        script_path: script,
        tool,
        action,
        ref_arg,
        cwd,
        within_min,
        json,
        max_tool_chars,
    })
}
