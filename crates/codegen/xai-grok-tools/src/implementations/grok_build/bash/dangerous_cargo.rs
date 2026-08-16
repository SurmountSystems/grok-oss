//! Refuse crate-wide / workspace cargo argv from the bash tool.
//!
//! Structured edits already format the files they wrote. Agents must not
//! type the old mop commands. Honest `cargo test -p <crate> --lib <filter>`
//! and file-listed `cargo fmt -- ... <abs.rs>` stay allowed.

/// If `command` is a crate-wide or workspace cargo argv the shell tool must
/// not spawn, return a one-line refuse message. Honest scoped cargo is
/// [`None`] so the real terminal still runs.
pub(super) fn try_parse_dangerous_cargo_refuse(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    for argv in cargo_argvs(trimmed) {
        if let Some(reason) = refuse_reason(&argv) {
            return Some(reason);
        }
    }
    None
}

fn refuse_reason(cargo_args: &[String]) -> Option<String> {
    let (subcommand, rest) = find_subcommand(cargo_args)?;
    match subcommand {
        "fmt" => refuse_fmt(rest),
        "clippy" => refuse_clippy(rest),
        "test" => refuse_test(rest),
        "nextest" => refuse_nextest(rest),
        _ => None,
    }
}

fn refuse_fmt(args: &[String]) -> Option<String> {
    let (cargo_side, passthrough) = split_passthrough(args);
    if has_long_flag(cargo_side, "--all") {
        return Some(refuse_message(
            "cargo fmt --all is crate-wide / workspace-wide",
        ));
    }
    if has_package_selector(cargo_side) && !rustfmt_has_file_list(passthrough) {
        return Some(refuse_message(
            "cargo fmt -p <crate> with no file list after -- is crate-wide",
        ));
    }
    None
}

fn refuse_clippy(args: &[String]) -> Option<String> {
    let (cargo_side, _) = split_passthrough(args);
    if has_long_flag(cargo_side, "--all-targets") {
        return Some(refuse_message("cargo clippy --all-targets is crate-wide"));
    }
    if has_long_flag(cargo_side, "--workspace") {
        return Some(refuse_message("cargo clippy --workspace is workspace-wide"));
    }
    None
}

fn refuse_test(args: &[String]) -> Option<String> {
    let (cargo_side, _) = split_passthrough(args);
    if has_long_flag(cargo_side, "--workspace") {
        return Some(refuse_message("cargo test --workspace is workspace-wide"));
    }
    None
}

fn refuse_nextest(args: &[String]) -> Option<String> {
    let run_args = nextest_run_args(args)?;
    let (run_side, _) = split_passthrough(run_args);
    if has_package_selector(run_side) || has_nextest_filter(run_side) {
        return None;
    }
    Some(refuse_message(
        "cargo nextest run with no -p and no filter is workspace-wide",
    ))
}

fn refuse_message(shape: &str) -> String {
    format!(
        "Refused: {shape}. Do not run crate-wide cargo from the shell tool. \
         The structured edit hook already formatted the files it wrote. \
         Use listed-file rustfmt, cargo clippy -p <crate> --lib --locked, \
         or cargo test -p <crate> --lib <filter>."
    )
}

fn find_subcommand(args: &[String]) -> Option<(&str, &[String])> {
    let mut i = 0;
    while i < args.len() {
        let t = args[i].as_str();
        if t == "--" {
            return None;
        }
        if matches!(t, "fmt" | "clippy" | "test" | "nextest") {
            return Some((t, &args[i + 1..]));
        }
        i += 1;
    }
    None
}

fn split_passthrough(args: &[String]) -> (&[String], &[String]) {
    match args.iter().position(|t| t == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (args, &[]),
    }
}

fn has_long_flag(args: &[String], flag: &str) -> bool {
    let eq = format!("{flag}=");
    args.iter().any(|t| t == flag || t.starts_with(&eq))
}

fn has_package_selector(args: &[String]) -> bool {
    args.iter().any(|t| {
        t == "-p"
            || t == "--package"
            || t.starts_with("--package=")
            || (t.starts_with("-p") && t.len() > 2 && !t.starts_with("--"))
    })
}

fn rustfmt_has_file_list(passthrough: &[String]) -> bool {
    let mut i = 0;
    while i < passthrough.len() {
        let t = passthrough[i].as_str();
        if t == "--" {
            i += 1;
            continue;
        }
        if t.starts_with("--") && t.contains('=') {
            i += 1;
            continue;
        }
        if t.starts_with('-') {
            if rustfmt_flag_takes_value(t) {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        return true;
    }
    false
}

fn rustfmt_flag_takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "--edition"
            | "--config-path"
            | "--config"
            | "--color"
            | "--file-lines"
            | "--emit"
            | "--style-edition"
    )
}

fn nextest_run_args(args: &[String]) -> Option<&[String]> {
    let mut i = 0;
    while i < args.len() {
        let t = args[i].as_str();
        if t == "--" {
            return None;
        }
        if t == "run" {
            return Some(&args[i + 1..]);
        }
        if t.starts_with('-') {
            i += 1;
            continue;
        }
        return None;
    }
    None
}

fn has_nextest_filter(args: &[String]) -> bool {
    let mut i = 0;
    while i < args.len() {
        let t = args[i].as_str();
        if t == "--" {
            break;
        }
        if t == "-E"
            || t == "--filterset"
            || t == "--filter"
            || (t.starts_with("-E") && t.len() > 2)
            || t.starts_with("--filterset=")
            || t.starts_with("--filter=")
        {
            return true;
        }
        if t.starts_with('-') {
            if nextest_flag_takes_value(t) {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        return true;
    }
    false
}

fn nextest_flag_takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "-p" | "--package"
            | "-j"
            | "--test-threads"
            | "-E"
            | "--filterset"
            | "--filter"
            | "--profile"
            | "--manifest-path"
            | "--target-dir"
            | "--features"
            | "--color"
            | "--config"
            | "--target"
            | "-Z"
            | "--bin"
            | "--example"
            | "--test"
            | "--bench"
            | "--exclude"
            | "--retries"
    )
}

fn cargo_argvs(command: &str) -> Vec<Vec<String>> {
    let tokens = simple_tokens(command);
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..=tokens.len() {
        let is_sep = i == tokens.len() || matches!(tokens[i].as_str(), "&&" | "||" | ";" | "|");
        if is_sep {
            if let Some(argv) = cargo_argv_from_statement(&tokens[start..i]) {
                out.push(argv);
            }
            start = i + 1;
        }
    }
    out
}

fn cargo_argv_from_statement(stmt: &[String]) -> Option<Vec<String>> {
    let mut i = skip_env_prefix(stmt, 0);
    if i >= stmt.len() || !is_cargo_bin(&stmt[i]) {
        return None;
    }
    i += 1;
    if i < stmt.len() && stmt[i].starts_with('+') {
        i += 1;
    }
    Some(stmt[i..].to_vec())
}

fn skip_env_prefix(stmt: &[String], mut i: usize) -> usize {
    while i < stmt.len() && is_env_assign(&stmt[i]) {
        i += 1;
    }
    if i < stmt.len() && is_env_bin(&stmt[i]) {
        i += 1;
        while i < stmt.len() && is_env_assign(&stmt[i]) {
            i += 1;
        }
    }
    i
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

fn is_env_bin(tok: &str) -> bool {
    file_name(tok) == "env"
}

fn is_cargo_bin(tok: &str) -> bool {
    file_name(tok) == "cargo"
}

fn file_name(tok: &str) -> &str {
    std::path::Path::new(tok)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(tok)
}

/// Whitespace split; single/double quotes keep one token (quotes stripped).
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

#[cfg(test)]
mod unit_tests {
    use super::try_parse_dangerous_cargo_refuse;

    #[test]
    fn honest_package_lib_filter_is_not_refused() {
        assert!(
            try_parse_dangerous_cargo_refuse(
                "cargo test -p xai-grok-tools --lib implement_memory_snapshot_intercept"
            )
            .is_none()
        );
    }

    #[test]
    fn listed_file_fmt_is_not_refused() {
        assert!(
            try_parse_dangerous_cargo_refuse(
                "cargo fmt -- --edition 2024 --config-path rustfmt.toml /tmp/foo.rs"
            )
            .is_none()
        );
        assert!(
            try_parse_dangerous_cargo_refuse(
                "cargo fmt -p xai-grok-pager -- --edition 2024 --config-path rustfmt.toml /tmp/foo.rs"
            )
            .is_none()
        );
    }

    #[test]
    fn env_prefixed_fmt_all_is_refused() {
        let msg =
            try_parse_dangerous_cargo_refuse("CARGO_TARGET_DIR=/tmp TMPDIR=/tmp cargo fmt --all")
                .expect("env-prefixed fmt --all");
        assert!(msg.to_lowercase().contains("refuse"));
        assert!(msg.to_lowercase().contains("cargo"));
    }
}
