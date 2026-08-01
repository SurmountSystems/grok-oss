//! Parse `run_terminal_command` strings that invoke host `memory.py`.

use std::path::PathBuf;

/// Allowlisted script path segment (host implement skill + bundled mirror).
const MEMORY_PY_MARKER: &str = "implement/scripts/memory.py";

/// Parsed allowlisted memory.py invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIntercept {
    pub script_path: String,
    pub subcommand: MemorySubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySubcommand {
    Path,
    Read,
    Snapshot,
    Update { stdin: UpdateStdinSource },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStdinSource {
    /// Bare `… memory.py update` (no stdin payload — fails like host SpecError).
    Empty,
    /// `echo '…' | python3 … update` or `echo "…" | …`
    Literal(String),
    /// `python3 … update < /path/to.json`
    FromFile(PathBuf),
}

/// Returns [`Some`] when `command` is a known implement-skill `memory.py` call.
///
/// Unknown Python (user project scripts, one-liners, other helpers) returns
/// [`None`] so the real shell still runs.
pub fn try_parse_memory_intercept(command: &str) -> Option<MemoryIntercept> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }

    // echo '…' | python3 PATH update
    // echo "…" | python3 PATH update
    // echo {} | python3 PATH update  (unquoted simple JSON object)
    if let Some(hit) = try_parse_echo_pipe(trimmed) {
        return Some(hit);
    }

    // python3 PATH subcmd [ < file ]
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
        // Absolute / PATH-qualified python3
        let base = std::path::Path::new(tok)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(tok);
        base == "python3" || base == "python" || (base.starts_with("python3.") && base.len() <= 12)
    }
}

fn is_allowlisted_memory_py(path: &str) -> bool {
    // Normalize backslashes just in case; host is POSIX.
    let p = path.replace('\\', "/");
    p.ends_with(MEMORY_PY_MARKER) || p.contains(&format!("/{MEMORY_PY_MARKER}"))
}

fn parse_subcmd(s: &str) -> Option<fn(UpdateStdinSource) -> MemorySubcommand> {
    match s {
        "path" => Some(|_| MemorySubcommand::Path),
        "read" => Some(|_| MemorySubcommand::Read),
        "snapshot" => Some(|_| MemorySubcommand::Snapshot),
        "update" => Some(|stdin| MemorySubcommand::Update { stdin }),
        _ => None,
    }
}

/// Tokenize a simple shell fragment: whitespace split, keeping single/double
/// quoted strings as one token (quotes stripped). Does not expand vars.
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

fn try_parse_direct(cmd: &str) -> Option<MemoryIntercept> {
    let tokens = simple_tokens(cmd);
    if tokens.len() < 3 {
        return None;
    }

    // Optional leading env assignments: FOO=bar python3 …
    let mut i = 0;
    while i < tokens.len() && tokens[i].contains('=') && !tokens[i].starts_with('-') {
        // Only treat as env if it looks like NAME=value and not a path
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

    // Optional -u / -B flags commonly used with python
    while i < tokens.len() && tokens[i].starts_with('-') && tokens[i] != "-" {
        // Do not accept -c (that is inline code — never intercept)
        if tokens[i] == "-c" || tokens[i].starts_with("-c") {
            return None;
        }
        i += 1;
    }

    if i >= tokens.len() {
        return None;
    }
    let script = tokens[i].clone();
    if !is_allowlisted_memory_py(&script) {
        return None;
    }
    i += 1;

    if i >= tokens.len() {
        return None;
    }
    let sub = tokens[i].clone();
    let make = parse_subcmd(&sub)?;
    i += 1;

    let mut stdin = UpdateStdinSource::Empty;
    // Trailing: < file   or  2>/dev/null etc. (ignore redirects we do not need)
    while i < tokens.len() {
        let t = &tokens[i];
        if t == "<" {
            i += 1;
            if i >= tokens.len() {
                return None;
            }
            stdin = UpdateStdinSource::FromFile(PathBuf::from(&tokens[i]));
            i += 1;
            continue;
        }
        if t.starts_with('<') && t.len() > 1 {
            stdin = UpdateStdinSource::FromFile(PathBuf::from(&t[1..]));
            i += 1;
            continue;
        }
        // Ignore stdout/stderr redirects: >/dev/null 2>&1 etc.
        if t.starts_with('>') || t == "2>&1" || t.starts_with("2>") {
            i += 1;
            continue;
        }
        // Unknown trailing token → not a clean intercept (avoid false positives)
        return None;
    }

    if !matches!(
        make(UpdateStdinSource::Empty),
        MemorySubcommand::Update { .. }
    ) && !matches!(stdin, UpdateStdinSource::Empty)
    {
        // redirect only meaningful for update
        return None;
    }

    Some(MemoryIntercept {
        script_path: script,
        subcommand: make(stdin),
    })
}

fn try_parse_echo_pipe(cmd: &str) -> Option<MemoryIntercept> {
    // Split on first bare `|` at top level (no quotes nesting beyond simple)
    let pipe_idx = find_top_level_pipe(cmd)?;
    let left = cmd[..pipe_idx].trim();
    let right = cmd[pipe_idx + 1..].trim();

    let payload = parse_echo_payload(left)?;
    let right_hit = try_parse_direct(right)?;
    match right_hit.subcommand {
        MemorySubcommand::Update {
            stdin: UpdateStdinSource::Empty,
        } => Some(MemoryIntercept {
            script_path: right_hit.script_path,
            subcommand: MemorySubcommand::Update {
                stdin: UpdateStdinSource::Literal(payload),
            },
        }),
        // Already had redirect — refuse ambiguous
        _ => None,
    }
}

fn find_top_level_pipe(s: &str) -> Option<usize> {
    let mut in_single = false;
    let mut in_double = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_single {
            if c == '\'' {
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        match c {
            '\'' => in_single = true,
            '"' => in_double = true,
            '|' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn parse_echo_payload(left: &str) -> Option<String> {
    let tokens = simple_tokens(left);
    if tokens.is_empty() || tokens[0] != "echo" {
        return None;
    }
    // echo -n payload…  or echo payload
    let mut i = 1;
    while i < tokens.len() && tokens[i].starts_with('-') && tokens[i] != "-" {
        // only allow -n / -e
        if tokens[i] != "-n" && tokens[i] != "-e" && tokens[i] != "-ne" && tokens[i] != "-en" {
            return None;
        }
        i += 1;
    }
    if i >= tokens.len() {
        return Some(String::new());
    }
    // Join remaining tokens with spaces (echo default)
    Some(tokens[i..].join(" "))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn rejects_python_c() {
        assert!(
            try_parse_memory_intercept(
                "python3 -c 'print(1)' /x/implement/scripts/memory.py snapshot"
            )
            .is_none()
        );
    }

    #[test]
    fn allows_python_u_flag() {
        let hit = try_parse_memory_intercept(
            "python3 -u /home/u/.agents/skills/implement/scripts/memory.py snapshot",
        )
        .unwrap();
        assert!(matches!(hit.subcommand, MemorySubcommand::Snapshot));
    }
}
