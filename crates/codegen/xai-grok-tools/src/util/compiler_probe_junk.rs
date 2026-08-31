//! Refuse rustc probe junk at the workspace root from ACP tools.
//!
//! File-level clippy-driver / rustc `--emit metadata` writes `lib*.rmeta` and
//! `*.long-type-*.txt` into the process cwd. Agents also one-shot `rustc`
//! into `a.out` / `rust_out`. Prevention lives in the tool call, not a later
//! mop and not `.gitignore`.

use std::path::{Path, PathBuf};

/// True when `name` is rustc / clippy-driver dump we never want at repo root.
pub fn is_compiler_probe_junk_file_name(name: &str) -> bool {
    let name = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    name == "a.out" || name == "rust_out" || name.ends_with(".rmeta") || is_long_type_dump(name)
}

fn is_long_type_dump(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".txt") else {
        return false;
    };
    let Some((prefix, digits)) = stem.rsplit_once(".long-type-") else {
        return false;
    };
    !prefix.is_empty() && !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// True when `path` is probe junk sitting directly in `workspace_root`.
pub fn is_workspace_root_compiler_probe_junk(path: &Path, workspace_root: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if !is_compiler_probe_junk_file_name(name) {
        return false;
    }
    let parent = path.parent().unwrap_or(path);
    dirs_equal(parent, workspace_root)
}

/// Refuse a write tool path that would create probe junk at the workspace root.
pub fn refuse_write_if_compiler_probe_junk(path: &Path, workspace_root: &Path) -> Option<String> {
    if is_workspace_root_compiler_probe_junk(path, workspace_root) {
        Some(refuse_write_message(path))
    } else {
        None
    }
}

/// Refuse a shell command that would compile or redirect probe junk at `cwd`.
pub fn try_parse_compiler_probe_junk_refuse(command: &str, cwd: &Path) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens = simple_tokens(trimmed);
    if let Some(target) = first_junk_redirect(&tokens, cwd) {
        return Some(refuse_write_message(&target));
    }
    let mut start = 0;
    for i in 0..=tokens.len() {
        let is_sep = i == tokens.len() || matches!(tokens[i].as_str(), "&&" | "||" | ";" | "|");
        if is_sep {
            if let Some(msg) = refuse_rustc_statement(&tokens[start..i], cwd) {
                return Some(msg);
            }
            start = i + 1;
        }
    }
    None
}

fn refuse_write_message(path: &Path) -> String {
    format!(
        "Refused: do not create rustc probe junk at the workspace root ({}). \
         ACP tools must not write *.rmeta, *.long-type-*.txt, a.out, or rust_out there. \
         Use a temp directory for rustc/clippy-driver artifacts. Do not gitignore these names.",
        path.display()
    )
}

fn refuse_rustc_message() -> String {
    "Refused: rustc/clippy-driver one-shots that write a.out, rust_out, *.rmeta, \
     or *.long-type-*.txt at the workspace root are not allowed. \
     Pass --out-dir or -o under a temp directory, or use cargo test in-tree."
        .to_string()
}

fn refuse_rustc_statement(stmt: &[String], cwd: &Path) -> Option<String> {
    let mut i = skip_env_prefix(stmt, 0);
    if i >= stmt.len() || !is_rustc_oneshot_bin(&stmt[i]) {
        return None;
    }
    i += 1;
    let args = &stmt[i..];
    if rustc_is_info_only(args) {
        return None;
    }
    if rustc_has_safe_artifact_dir(args, cwd) {
        return None;
    }
    Some(refuse_rustc_message())
}

fn is_rustc_oneshot_bin(tok: &str) -> bool {
    matches!(file_name(tok), "rustc" | "clippy-driver")
}

fn rustc_is_info_only(args: &[String]) -> bool {
    if args.is_empty() {
        return true;
    }
    let mut i = 0;
    while i < args.len() {
        let t = args[i].as_str();
        if matches!(t, "--version" | "-V" | "--help" | "-h" | "-vV") {
            i += 1;
            continue;
        }
        if t == "--print" {
            i += 2;
            continue;
        }
        if t.starts_with("--print=") {
            i += 1;
            continue;
        }
        return false;
    }
    true
}

fn rustc_has_safe_artifact_dir(args: &[String], cwd: &Path) -> bool {
    let mut i = 0;
    while i < args.len() {
        let t = args[i].as_str();
        if t == "--out-dir" {
            let dir = args.get(i + 1).map(String::as_str).unwrap_or(".");
            return !dirs_equal(&resolve_against_cwd(dir, cwd), cwd);
        }
        if let Some(dir) = t.strip_prefix("--out-dir=") {
            return !dirs_equal(&resolve_against_cwd(dir, cwd), cwd);
        }
        if t == "-o" {
            let out = args.get(i + 1).map(String::as_str).unwrap_or("a.out");
            return output_parent_is_not_workspace_root(out, cwd);
        }
        if let Some(out) = t.strip_prefix("-o")
            && t.len() > 2
            && !t.starts_with("--")
        {
            return output_parent_is_not_workspace_root(out, cwd);
        }
        i += 1;
    }
    false
}

fn first_junk_redirect(tokens: &[String], cwd: &Path) -> Option<PathBuf> {
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i].as_str();
        let target = if matches!(t, ">" | ">>" | "1>" | "2>" | "&>" | "1>>" | "2>>") {
            tokens.get(i + 1).map(|s| s.as_str())
        } else if let Some(rest) = t.strip_prefix(">>") {
            Some(rest).filter(|s| !s.is_empty())
        } else if let Some(rest) = t.strip_prefix('>') {
            Some(rest).filter(|s| !s.is_empty() && !rest.starts_with('>'))
        } else {
            None
        };
        if let Some(raw) = target.filter(|s| !s.is_empty()) {
            let path = resolve_against_cwd(raw, cwd);
            if is_workspace_root_compiler_probe_junk(&path, cwd) {
                return Some(path);
            }
        }
        i += 1;
    }
    None
}

fn output_parent_is_not_workspace_root(out: &str, cwd: &Path) -> bool {
    let path = resolve_against_cwd(out, cwd);
    path.parent().is_some_and(|p| !dirs_equal(p, cwd))
}

fn resolve_against_cwd(raw: &str, cwd: &Path) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() { p } else { cwd.join(p) }
}

fn dirs_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (dunce::canonicalize(a), dunce::canonicalize(b)) {
        (Ok(aa), Ok(bb)) => aa == bb,
        _ => false,
    }
}

fn skip_env_prefix(stmt: &[String], mut i: usize) -> usize {
    while i < stmt.len() && is_env_assign(&stmt[i]) {
        i += 1;
    }
    if i < stmt.len() && file_name(&stmt[i]) == "env" {
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

fn file_name(tok: &str) -> &str {
    Path::new(tok)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(tok)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn names_match_rmeta_long_type_a_out_rust_out() {
        assert!(is_compiler_probe_junk_file_name(
            "libxai_grok_compaction.rmeta"
        ));
        assert!(is_compiler_probe_junk_file_name(
            "xai_grok_pager.long-type-15498221572048715402.txt"
        ));
        assert!(is_compiler_probe_junk_file_name("a.out"));
        assert!(is_compiler_probe_junk_file_name("rust_out"));
        assert!(!is_compiler_probe_junk_file_name("notes.md"));
        assert!(!is_compiler_probe_junk_file_name("long-type-not-a-dump.rs"));
    }

    #[test]
    fn root_rmeta_is_junk_nested_target_is_not() {
        let root = Path::new("/tmp/ws");
        assert!(is_workspace_root_compiler_probe_junk(
            &root.join("libfoo.rmeta"),
            root
        ));
        assert!(!is_workspace_root_compiler_probe_junk(
            &root.join("target/libfoo.rmeta"),
            root
        ));
    }

    #[test]
    fn rustc_oneshot_is_refused_version_and_tmp_out_dir_are_not() {
        let cwd = Path::new("/tmp/ws");
        assert!(try_parse_compiler_probe_junk_refuse("rustc foo.rs", cwd).is_some());
        assert!(try_parse_compiler_probe_junk_refuse("rustc -", cwd).is_some());
        assert!(try_parse_compiler_probe_junk_refuse("rustc foo.rs -o a.out", cwd).is_some());
        assert!(try_parse_compiler_probe_junk_refuse("echo x > libfoo.rmeta", cwd).is_some());
        assert!(try_parse_compiler_probe_junk_refuse("rustc --version", cwd).is_none());
        assert!(
            try_parse_compiler_probe_junk_refuse(
                "rustc foo.rs --out-dir /tmp/grok-edit-verify-scratch",
                cwd
            )
            .is_none()
        );
        assert!(
            try_parse_compiler_probe_junk_refuse("cargo rustc -p xai-grok-tools --lib", cwd)
                .is_none()
        );
    }
}
