//! Print an rsync argv that copies a VPS worktree onto the laptop.
//!
//! Prints argv only. Never runs rsync. Never runs git. Destination is local;
//! source is remote `HOST:SRC`. Humans sign: git commit -S.

use std::io::{self, Write};
use std::process::ExitCode;

pub const RSYNC_EXCLUDES: &[&str] = &[".git", "target", ".lake", "result"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsyncPull {
    pub from: String,
    pub to: String,
}

/// rsync treats `host:path` (colon before any `/`) as remote.
pub fn looks_like_rsync_remote(spec: &str) -> bool {
    let s = spec.trim();
    if s.starts_with("rsync://") || s.starts_with("ssh://") {
        return true;
    }
    let Some(colon) = s.find(':') else {
        return false;
    };
    let left = &s[..colon];
    !left.is_empty() && !left.contains('/')
}

/// True when argv tokens would invoke `git commit` or `git push`.
pub fn argv_would_git_mutate(argv: &[String]) -> bool {
    let tokens: Vec<&str> = argv.iter().flat_map(|a| a.split_whitespace()).collect();
    tokens
        .windows(2)
        .any(|w| w[0] == "git" && (w[1] == "commit" || w[1] == "push"))
}

pub fn parse_args(args: &[String]) -> Result<RsyncPull, String> {
    let mut from = None;
    let mut to = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--from" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "print-rsync-pull: --from needs HOST:SRC".to_string())?;
                from = Some(v.clone());
                i += 2;
            }
            "--to" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "print-rsync-pull: --to needs DEST".to_string())?;
                to = Some(v.clone());
                i += 2;
            }
            other => {
                return Err(format!("print-rsync-pull: unknown argument {other}"));
            }
        }
    }
    let from = from.ok_or_else(|| "print-rsync-pull: --from HOST:SRC is required".to_string())?;
    let to = to.ok_or_else(|| "print-rsync-pull: --to DEST is required".to_string())?;
    Ok(RsyncPull { from, to })
}

pub fn rsync_pull_argv(from: &str, to: &str) -> Result<Vec<String>, String> {
    if looks_like_rsync_remote(to) {
        return Err(
            "print-rsync-pull: destination must be a local laptop path, not host:path".to_string(),
        );
    }
    if !looks_like_rsync_remote(from) {
        return Err("print-rsync-pull: source must be remote HOST:SRC".to_string());
    }
    let mut argv = vec!["rsync".to_string(), "-a".to_string()];
    for ex in RSYNC_EXCLUDES {
        argv.push("--exclude".to_string());
        argv.push((*ex).to_string());
    }
    argv.push(from.to_string());
    argv.push(to.to_string());
    if argv_would_git_mutate(&argv) {
        return Err(
            "print-rsync-pull: refusing argv that would git commit or git push".to_string(),
        );
    }
    if argv.iter().any(|t| t == "commit" || t == "push") {
        return Err(
            "print-rsync-pull: refusing argv that would git commit or git push".to_string(),
        );
    }
    Ok(argv)
}

pub fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.chars()
                .all(|c| c.is_ascii_alphanumeric() || "/._-:@~+".contains(c))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn run(args: &[String]) -> ExitCode {
    let pull = match parse_args(args) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            let _ = writeln!(
                io::stderr(),
                "Usage: grok-nix-helper print-rsync-pull --from HOST:SRC --to DEST"
            );
            return ExitCode::from(2);
        }
    };
    match rsync_pull_argv(&pull.from, &pull.to) {
        Ok(argv) => {
            let _ = writeln!(io::stdout(), "{}", shell_join(&argv));
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_rsync_pull_includes_exclude_git_and_laptop_dest() {
        let dest = "/home/hunter/Projects/surmount/grok-build";
        let argv = rsync_pull_argv("surmount-1:/work/grok-build", dest).expect("local dest");
        assert_eq!(argv[0], "rsync");
        assert!(
            argv.windows(2)
                .any(|w| w[0] == "--exclude" && w[1] == ".git"),
            "rsync argv must exclude .git: {argv:?}"
        );
        assert!(
            argv.windows(2)
                .any(|w| w[0] == "--exclude" && w[1] == "target")
        );
        assert!(
            argv.windows(2)
                .any(|w| w[0] == "--exclude" && w[1] == ".lake")
        );
        assert!(
            argv.windows(2)
                .any(|w| w[0] == "--exclude" && w[1] == "result")
        );
        assert_eq!(argv.last().map(String::as_str), Some(dest));
        assert_eq!(
            argv.get(argv.len() - 2).map(String::as_str),
            Some("surmount-1:/work/grok-build")
        );
        assert!(!argv_would_git_mutate(&argv));
    }

    #[test]
    fn print_rsync_pull_refuses_remote_dest() {
        let err =
            rsync_pull_argv("surmount-1:/work/src", "otherhost:/tmp/out").expect_err("remote dest");
        assert!(
            err.contains("local laptop path") || err.contains("host:path"),
            "{err}"
        );
    }

    #[test]
    fn print_rsync_pull_refuses_git_commit() {
        let err = rsync_pull_argv("surmount-1:/work/src", "git commit").expect_err("mutate");
        assert!(
            err.contains("commit") || err.contains("push") || err.contains("refusing"),
            "{err}"
        );
        let err = rsync_pull_argv("git commit", "/tmp/out").expect_err("from mutate");
        assert!(
            !err.is_empty(),
            "must refuse a source that would git commit"
        );
    }

    #[test]
    fn print_rsync_subcommand_never_contains_commit() {
        assert!(
            !parse_args(&["--from".into(), "h:s".into(), "--to".into(), "/tmp".into()])
                .map(|p| format!("{} {}", p.from, p.to))
                .unwrap()
                .split_whitespace()
                .any(|t| t == "commit")
        );
        let argv = rsync_pull_argv("h:/s", "/tmp/laptop").unwrap();
        assert!(!argv.iter().any(|t| t.contains("commit")));
        assert_ne!(argv[0], "git");
    }
}
