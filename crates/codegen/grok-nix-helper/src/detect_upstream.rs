//! Detect a new xAI monorepo export (force-pushed orphan tip).
//! Exit 0 = up to date with last import; 2 = new export available; 1 = error.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::git_cmd::{
    git_run, git_status_ok, git_stdout, git_stdout_lossy, is_git_object_id,
    nth_git_object_id_in_backticks,
};

/// Last completed import tree (second 40-hex git object id on the last
/// non-pending table row). SHA-1 here is a git tree id, not a download hash.
pub fn last_imported_tree(log: &str) -> Option<String> {
    let mut last: Option<String> = None;
    for line in log.lines() {
        let t = line.trim_start();
        if !t.starts_with("| ") {
            continue;
        }
        if t.to_ascii_lowercase().contains("pending") {
            continue;
        }
        let rest = t.trim_start_matches("| ").trim_start();
        if rest.len() < 4 || !rest.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        if let Some(tree) = nth_git_object_id_in_backticks(line, 1) {
            last = Some(tree.to_string());
        }
    }
    last
}

fn ensure_remote(root: &Path, url: &str) -> Result<String, i32> {
    if git_status_ok(root, &["remote", "get-url", "xai-org"]) {
        Ok(env::var("UPSTREAM_REMOTE").unwrap_or_else(|_| "xai-org".into()))
    } else if git_status_ok(root, &["remote", "get-url", "upstream"]) {
        Ok(env::var("UPSTREAM_REMOTE").unwrap_or_else(|_| "upstream".into()))
    } else {
        let remote = env::var("UPSTREAM_REMOTE").unwrap_or_else(|_| "xai-org".into());
        println!("Adding remote '{remote}' -> {url}");
        if !git_status_ok(root, &["remote", "add", &remote, url]) {
            let _ = writeln!(io::stderr(), "error: git remote add {remote} failed");
            return Err(1);
        }
        Ok(remote)
    }
}

pub fn run(_args: &[String]) -> ExitCode {
    let root = crate::git_cmd::find_repo_root();
    let import_log =
        env::var("IMPORT_LOG").unwrap_or_else(|_| "docs/upstream-import-log.md".into());
    let upstream_url = env::var("UPSTREAM_URL")
        .unwrap_or_else(|_| "https://github.com/xai-org/grok-build.git".into());
    let upstream_branch = env::var("UPSTREAM_BRANCH").unwrap_or_else(|_| "main".into());

    let remote = match ensure_remote(&root, &upstream_url) {
        Ok(r) => r,
        Err(c) => return ExitCode::from(c as u8),
    };

    println!("Fetching {remote}/{upstream_branch} ...");
    if git_run(&root, &["fetch", &remote, &upstream_branch, "--force"])
        .ok()
        .is_none_or(|s| !s.success())
    {
        let _ = writeln!(io::stderr(), "error: git fetch failed");
        return ExitCode::from(1);
    }

    let refname = format!("{remote}/{upstream_branch}");
    let tip = match git_stdout(&root, &["rev-parse", &refname]) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: rev-parse tip: {e}");
            return ExitCode::from(1);
        }
    };
    let tree_arg = format!("{refname}^{{tree}}");
    let tree = match git_stdout(&root, &["rev-parse", &tree_arg]) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: rev-parse tree: {e}");
            return ExitCode::from(1);
        }
    };
    let parents_line = git_stdout_lossy(&root, &["rev-list", "--parents", "-n1", &tip]);
    let parents = parents_line.split_whitespace().count().saturating_sub(1);
    let subject = git_stdout_lossy(&root, &["log", "-1", "--format=%s", &tip]);
    let author = git_stdout_lossy(&root, &["log", "-1", "--format=%an %ci", &tip]);

    println!("xAI tip:    {tip}");
    println!("xAI tree:   {tree}");
    println!("parents:    {parents} (0 = orphan export root)");
    println!("subject:    {subject}");
    println!("author:     {author}");

    let last_tree = fs::read_to_string(root.join(&import_log))
        .ok()
        .and_then(|body| last_imported_tree(&body));

    let Some(last_tree) = last_tree.filter(|s| is_git_object_id(s)) else {
        println!("WARN: no completed import tree in {import_log} — treating as first pin needed");
        println!("NEW_EXPORT=1");
        println!("XAI_TIP={tip}");
        println!("XAI_TREE={tree}");
        return ExitCode::from(2);
    };

    println!("last imported tree: {last_tree}");
    if tree == last_tree {
        println!("OK: xAI export tree matches last import (no new export content).");
        println!("NEW_EXPORT=0");
        println!("XAI_TIP={tip}");
        println!("XAI_TREE={tree}");
        return ExitCode::SUCCESS;
    }

    println!();
    println!("NEW EXPORT DETECTED (trees differ).");
    println!("Content delta vs last import:");
    let last_tree_obj = format!("{last_tree}^{{tree}}");
    if git_status_ok(&root, &["cat-file", "-e", &last_tree_obj]) {
        let _ = git_run(&root, &["diff", "--stat", &last_tree, &tree]);
    } else {
        println!(
            "(last tree not in local object store; fetch older export or rely on full tip diff)"
        );
        let _ = git_run(&root, &["diff", "--stat", &tip]);
    }
    println!();
    println!("Next: grok-nix-helper import-upstream-export");
    println!("NEW_EXPORT=1");
    println!("XAI_TIP={tip}");
    println!("XAI_TREE={tree}");
    println!("LAST_TREE={last_tree}");
    ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_last_completed_tree_not_pending() {
        let log = r#"
| date | tip | tree | note |
| 2026-07-01 | `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` | `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb` | seed |
| 2026-08-01 | `cccccccccccccccccccccccccccccccccccccccc` | `dddddddddddddddddddddddddddddddddddddddd` | pending |
"#;
        assert_eq!(
            last_imported_tree(log).as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
    }

    #[test]
    fn empty_log_is_none() {
        assert_eq!(last_imported_tree("# no rows\n"), None);
    }
}
