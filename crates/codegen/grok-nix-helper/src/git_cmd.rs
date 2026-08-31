//! Argv git helpers. Never eval. SHA-1 is git object ids only.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub fn env_flag(name: &str) -> bool {
    matches!(env::var(name).as_deref(), Ok("1"))
}

pub fn find_repo_root() -> PathBuf {
    let mut p = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if p.join("flake.nix").is_file() && p.join("AGENTS.md").is_file() {
            return p;
        }
        if !p.pop() {
            break;
        }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn git_output(root: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new("git").current_dir(root).args(args).output()
}

pub fn git_status_ok(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn git_stdout(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_output(root, args).map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn git_stdout_lossy(root: &Path, args: &[&str]) -> String {
    git_stdout(root, args).unwrap_or_default()
}

/// Inherit stdio so cherry-pick / merge progress is visible. Argv only.
pub fn git_run(root: &Path, args: &[&str]) -> io::Result<std::process::ExitStatus> {
    Command::new("git").current_dir(root).args(args).status()
}

pub fn is_dirty(root: &Path) -> bool {
    !git_stdout_lossy(root, &["status", "--porcelain"]).is_empty()
}

pub fn print_porcelain_head(root: &Path, n: usize) {
    let text = git_stdout_lossy(root, &["status", "--porcelain"]);
    for (i, line) in text.lines().enumerate() {
        if i >= n {
            break;
        }
        let _ = writeln!(io::stderr(), "{line}");
    }
}

pub fn git_path(root: &Path, name: &str) -> PathBuf {
    let p = git_stdout_lossy(root, &["rev-parse", "--git-path", name]);
    if p.is_empty() {
        return root.join(".git").join(name);
    }
    let pb = PathBuf::from(&p);
    if pb.is_absolute() { pb } else { root.join(pb) }
}

pub fn refuse_dirty(root: &Path, allow_dirty: bool) -> Result<(), i32> {
    if !is_dirty(root) {
        return Ok(());
    }
    if allow_dirty {
        let _ = writeln!(
            io::stderr(),
            "WARN: dirty worktree allowed via ALLOW_DIRTY=1"
        );
        return Ok(());
    }
    let _ = writeln!(
        io::stderr(),
        "error: working tree is dirty. Commit/stash first (or ALLOW_DIRTY=1)."
    );
    print_porcelain_head(root, 40);
    Err(1)
}

pub fn first_git_object_id_in_backticks(line: &str) -> Option<&str> {
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('`') {
            let inner = &rest[..end];
            if is_git_object_id(inner) {
                return Some(inner);
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    None
}

pub fn nth_git_object_id_in_backticks(line: &str, n: usize) -> Option<&str> {
    let mut rest = line;
    let mut seen = 0usize;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('`') {
            let inner = &rest[..end];
            if is_git_object_id(inner) {
                if seen == n {
                    return Some(inner);
                }
                seen += 1;
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    None
}

pub fn is_git_object_id(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_object_id_is_40_hex_only() {
        assert!(is_git_object_id("b189869b7755d2b482969acf6c92da3ecfeffd36"));
        assert!(!is_git_object_id("not-a-git-id"));
        assert!(!is_git_object_id("abcd"));
    }

    #[test]
    fn backtick_parser_skips_short_spans() {
        let line = "| 2026-08-01 | `seed` | `b189869b7755d2b482969acf6c92da3ecfeffd36` |";
        assert_eq!(
            first_git_object_id_in_backticks(line),
            Some("b189869b7755d2b482969acf6c92da3ecfeffd36")
        );
        assert_eq!(
            nth_git_object_id_in_backticks(
                "| d | `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` | `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb` |",
                1
            ),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
    }
}
