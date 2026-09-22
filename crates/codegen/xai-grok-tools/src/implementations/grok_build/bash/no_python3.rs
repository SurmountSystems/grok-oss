//! Refuse `python` / `python3` exec from the shell tool.
//!
//! Allowlisted skill stubs are intercepted earlier and run in Rust.
//! Anything else that would start a Python interpreter is refused.
//! grok-oss does not spawn python3.

/// When `command` would execute python or python3, the refuse line.
/// [`None`] when the shell tool may still run the command.
pub(super) fn try_parse_python3_refuse(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    if command_spawns_python(trimmed, 0) {
        Some(refuse_message())
    } else {
        None
    }
}

fn refuse_message() -> String {
    "Refused: grok-oss does not spawn python3. python3 must not be part of \
     how grok-oss behaves or executes. Allowlisted skill helpers already run \
     in Rust. Do not run python or python3 from the shell tool."
        .to_string()
}

fn command_spawns_python(command: &str, depth: usize) -> bool {
    if depth > 5 {
        return false;
    }
    for body in live_substitutions(command) {
        if command_spawns_python(&body, depth + 1) {
            return true;
        }
    }
    let tokens = tokenize(command);
    let mut start = 0;
    for i in 0..=tokens.len() {
        let is_sep = i == tokens.len() || is_separator(&tokens[i]);
        if is_sep {
            if statement_spawns_python(&tokens[start..i], depth) {
                return true;
            }
            start = i + 1;
        }
    }
    false
}

fn statement_spawns_python(stmt: &[String], depth: usize) -> bool {
    let mut i = 0;
    while i < stmt.len() && is_env_assign(&stmt[i]) {
        i += 1;
    }
    while i < stmt.len() {
        let base = file_name(&stmt[i]);
        if is_python_interpreter(base) {
            return true;
        }
        if is_shell(base) {
            return shell_script_spawns_python(&stmt[i + 1..], depth);
        }
        if !is_wrapper(base) {
            return false;
        }
        let wrapper = base.to_string();
        i += 1;
        while i < stmt.len() {
            let tok = &stmt[i];
            if is_env_assign(tok) {
                i += 1;
                continue;
            }
            if tok == "--" {
                i += 1;
                break;
            }
            if tok.starts_with('-') {
                let takes = flag_takes_value(&wrapper, tok);
                i += 1;
                if takes && i < stmt.len() {
                    i += 1;
                }
                continue;
            }
            break;
        }
    }
    false
}

fn shell_script_spawns_python(args: &[String], depth: usize) -> bool {
    let mut i = 0;
    let mut saw_c = false;
    while i < args.len() && args[i].starts_with('-') {
        if args[i].contains('c') {
            saw_c = true;
        }
        i += 1;
    }
    if !saw_c {
        return false;
    }
    let script = args[i..].join(" ");
    !script.is_empty() && command_spawns_python(&script, depth + 1)
}

fn is_separator(tok: &str) -> bool {
    matches!(tok, "&&" | "||" | ";" | "|" | "&" | "\n")
}

fn is_python_interpreter(base: &str) -> bool {
    if matches!(base, "python" | "python2" | "python3") {
        return true;
    }
    let Some(rest) = base.strip_prefix("python3.") else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

fn is_shell(base: &str) -> bool {
    matches!(
        base,
        "sh" | "bash" | "dash" | "zsh" | "ash" | "ksh" | "mksh"
    )
}

fn is_wrapper(base: &str) -> bool {
    matches!(
        base,
        "sudo"
            | "doas"
            | "command"
            | "env"
            | "time"
            | "nice"
            | "nohup"
            | "stdbuf"
            | "exec"
            | "xargs"
            | "setsid"
    )
}

fn flag_takes_value(wrapper: &str, flag: &str) -> bool {
    if flag.contains('=') {
        return false;
    }
    match wrapper {
        "sudo" | "doas" => matches!(
            flag,
            "-u" | "-g"
                | "-h"
                | "-p"
                | "-C"
                | "-T"
                | "-R"
                | "-D"
                | "-U"
                | "-a"
                | "--user"
                | "--group"
                | "--host"
                | "--prompt"
                | "--role"
                | "--chdir"
                | "--command-timeout"
        ),
        "nice" => matches!(flag, "-n" | "--adjustment"),
        "env" => matches!(
            flag,
            "-u" | "-S"
                | "-C"
                | "--unset"
                | "--split-string"
                | "--chdir"
                | "--argv0"
                | "--default-signal"
                | "--ignore-signal"
                | "--block-signal"
        ),
        "xargs" => matches!(
            flag,
            "-I" | "-n"
                | "-P"
                | "-s"
                | "-a"
                | "-E"
                | "-L"
                | "-J"
                | "--replace"
                | "--max-args"
                | "--max-procs"
                | "--max-chars"
                | "--arg-file"
                | "--eof"
                | "--max-lines"
        ),
        "stdbuf" => matches!(
            flag,
            "-i" | "-o" | "-e" | "--input" | "--output" | "--error"
        ),
        "time" => matches!(flag, "-f" | "-o" | "--format" | "--output"),
        "setsid" => matches!(flag, "-w" | "-c" | "--wait" | "--ctty"),
        _ => false,
    }
}

fn is_env_assign(tok: &str) -> bool {
    if tok.starts_with('-') {
        return false;
    }
    match tok.split_once('=') {
        Some((name, _)) => {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false,
    }
}

fn file_name(tok: &str) -> &str {
    std::path::Path::new(tok)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(tok)
}

fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\r' => push_cur(&mut out, &mut cur),
            '\n' => {
                push_cur(&mut out, &mut cur);
                out.push("\n".to_string());
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
            '&' => {
                push_cur(&mut out, &mut cur);
                if chars.peek() == Some(&'&') {
                    chars.next();
                    out.push("&&".to_string());
                } else {
                    out.push("&".to_string());
                }
            }
            '|' => {
                push_cur(&mut out, &mut cur);
                if chars.peek() == Some(&'|') {
                    chars.next();
                    out.push("||".to_string());
                } else {
                    out.push("|".to_string());
                }
            }
            ';' => {
                push_cur(&mut out, &mut cur);
                out.push(";".to_string());
            }
            _ => cur.push(c),
        }
    }
    push_cur(&mut out, &mut cur);
    out
}

fn push_cur(out: &mut Vec<String>, cur: &mut String) {
    if !cur.is_empty() {
        out.push(std::mem::take(cur));
    }
}

/// Bodies of live `$(...)` and backtick substitutions. Single quotes hide them.
fn live_substitutions(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
                i += 1;
                continue;
            }
            if q == '"' {
                if c == '$' && i + 1 < chars.len() && chars[i + 1] == '(' {
                    let (next, body) = read_paren(&chars, i);
                    out.push(body);
                    i = next;
                    continue;
                }
                if c == '`' {
                    let (next, body) = read_backtick(&chars, i);
                    out.push(body);
                    i = next;
                    continue;
                }
            }
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            i += 1;
            continue;
        }
        if c == '$' && i + 1 < chars.len() && chars[i + 1] == '(' {
            let (next, body) = read_paren(&chars, i);
            out.push(body);
            i = next;
            continue;
        }
        if c == '`' {
            let (next, body) = read_backtick(&chars, i);
            out.push(body);
            i = next;
            continue;
        }
        i += 1;
    }
    out
}

fn read_paren(chars: &[char], start: usize) -> (usize, String) {
    let mut i = start + 2;
    let body_at = i;
    let mut depth = 1;
    let mut quote: Option<char> = None;
    while i < chars.len() && depth > 0 {
        let c = chars[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        i += 1;
    }
    let body: String = chars[body_at..i].iter().collect();
    (i + 1, body)
}

fn read_backtick(chars: &[char], start: usize) -> (usize, String) {
    let mut i = start + 1;
    let body_at = i;
    while i < chars.len() && chars[i] != '`' {
        i += 1;
    }
    let body: String = chars[body_at..i].iter().collect();
    (i + 1, body)
}

#[cfg(test)]
mod unit_tests {
    use super::try_parse_python3_refuse;

    fn refused(cmd: &str) {
        let msg = try_parse_python3_refuse(cmd).unwrap_or_else(|| panic!("expected refuse: {cmd}"));
        assert!(
            msg.contains("does not spawn python3"),
            "refuse for `{cmd}` must say grok-oss does not spawn python3: {msg}"
        );
    }

    fn allowed(cmd: &str) {
        assert!(
            try_parse_python3_refuse(cmd).is_none(),
            "must not refuse `{cmd}`"
        );
    }

    #[test]
    fn python3_exec_is_refused() {
        refused("python3 /home/u/myproject/foo.py");
        refused("python3 -c 'print(1)'");
        refused("python script.py");
        refused("/usr/bin/python3 -c 'print(1)'");
        refused("env python3 -c 'print(1)'");
        refused("sudo -u root python3 x.py");
        refused("bash -c 'python3 -c print(1)'");
        refused("echo hello && python3 x.py");
        refused("python3.12 -c 'print(1)'");
        refused("$(python3 -c 'print(1)')");
    }

    #[test]
    fn non_python_commands_are_not_refused() {
        allowed("echo hello");
        allowed("echo python3");
        allowed("grep python3 file");
        allowed("echo '$(python3 -c 1)'");
        allowed("cargo test -p xai-grok-tools --lib grok_oss_does_not_spawn_python3");
    }
}
