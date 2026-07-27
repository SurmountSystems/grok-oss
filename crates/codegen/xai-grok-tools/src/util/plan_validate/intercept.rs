//! Parse `run_terminal_command` strings that invoke host `validate-plan.py`.

use std::path::PathBuf;

/// Allowlisted script path segment (host execute-plan skill + bundled mirror).
const VALIDATE_PLAN_PY_MARKER: &str = "execute-plan/scripts/validate-plan.py";

/// Parsed allowlisted validate-plan.py invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanValidateIntercept {
    pub script_path: String,
    pub doc_path: PathBuf,
}

/// Returns [`Some`] when `command` is a known execute-plan `validate-plan.py` call.
///
/// Unknown Python (user project scripts, one-liners, other helpers) returns
/// [`None`] so the real shell still runs.
pub fn try_parse_plan_validate_intercept(command: &str) -> Option<PlanValidateIntercept> {
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

fn is_allowlisted_validate_plan_py(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.ends_with(VALIDATE_PLAN_PY_MARKER) || p.contains(&format!("/{VALIDATE_PLAN_PY_MARKER}"))
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

fn try_parse_direct(cmd: &str) -> Option<PlanValidateIntercept> {
    let tokens = simple_tokens(cmd);
    if tokens.len() < 3 {
        return None;
    }

    // Optional leading env assignments: FOO=bar python3 …
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

    // Optional -u / -B flags; never intercept -c
    while i < tokens.len() && tokens[i].starts_with('-') && tokens[i] != "-" {
        if tokens[i] == "-c" || tokens[i].starts_with("-c") {
            return None;
        }
        i += 1;
    }

    if i >= tokens.len() {
        return None;
    }
    let script = tokens[i].clone();
    if !is_allowlisted_validate_plan_py(&script) {
        return None;
    }
    i += 1;

    if i >= tokens.len() {
        return None;
    }
    let doc = tokens[i].clone();
    i += 1;

    // Ignore trailing redirects only; anything else is not a clean intercept.
    while i < tokens.len() {
        let t = &tokens[i];
        if t.starts_with('>') || t == "2>&1" || t.starts_with("2>") {
            i += 1;
            continue;
        }
        return None;
    }

    Some(PlanValidateIntercept {
        script_path: script,
        doc_path: PathBuf::from(doc),
    })
}
