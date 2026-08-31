//! Put Surmount commits on top of the current xAI export tip (real cherry-pick).
//!
//! Does not push. Does not rewrite Surmount main. On conflict, stop and hand
//! `git cherry-pick --continue` to a human TTY.

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::git_cmd::{
    env_flag, first_git_object_id_in_backticks, git_path, git_run, git_status_ok, git_stdout,
    git_stdout_lossy, refuse_dirty,
};

/// Default exclusive lower bound when the import log has no seed row.
/// Git object id (40-hex), not a download hash.
pub const DEFAULT_SEED_GIT_ID: &str = "b189869b7755d2b482969acf6c92da3ecfeffd36";

pub fn parse_seed_from_import_log(log: &str) -> Option<String> {
    for line in log.lines() {
        if !line.starts_with("| 20") {
            continue;
        }
        if !line.to_ascii_lowercase().contains("seed") {
            continue;
        }
        if let Some(id) = first_git_object_id_in_backticks(line) {
            return Some(id.to_string());
        }
    }
    None
}

pub fn cherry_picked_from_trailers(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        let rest = if let Some(r) = t.strip_prefix("(cherry picked from commit ") {
            r
        } else if let Some(i) = t.find("cherry picked from commit ") {
            &t[i + "cherry picked from commit ".len()..]
        } else {
            continue;
        };
        let id: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if id.len() == 40 {
            out.push(id);
        }
    }
    out
}

fn mid_cherry_pick(root: &Path) -> bool {
    git_path(root, "CHERRY_PICK_HEAD").is_file() || git_path(root, "sequencer").is_dir()
}

pub fn run(raw: &[String]) -> ExitCode {
    let root = crate::git_cmd::find_repo_root();
    if !git_status_ok(&root, &["rev-parse", "--git-dir"]) {
        let _ = writeln!(io::stderr(), "error: not a git repository");
        return ExitCode::from(2);
    }

    let continue_flag = env_flag("CONTINUE");
    let force = env_flag("FORCE");
    let first_parent = env_flag("FIRST_PARENT");

    if mid_cherry_pick(&root) {
        if continue_flag {
            let _ = writeln!(
                io::stderr(),
                "error: cherry-pick still in progress. Finish it first:"
            );
            let _ = writeln!(io::stderr(), "  git add -u && git cherry-pick --continue");
            let _ = writeln!(
                io::stderr(),
                "  then: CONTINUE=1 grok-nix-helper put-history-on-xai"
            );
            return ExitCode::from(1);
        }
        let _ = writeln!(io::stderr(), "error: cherry-pick in progress. Do one of:");
        let _ = writeln!(io::stderr(), "  # finish current pick");
        let _ = writeln!(
            io::stderr(),
            "  git add -u && git cherry-pick --continue && CONTINUE=1 grok-nix-helper put-history-on-xai"
        );
        let _ = writeln!(io::stderr(), "  # or abort and restore a known tip");
        let _ = writeln!(io::stderr(), "  git cherry-pick --abort");
        return ExitCode::from(1);
    }

    if let Err(c) = refuse_dirty(&root, continue_flag || env_flag("ALLOW_DIRTY")) {
        return ExitCode::from(c as u8);
    }

    let remote = if git_status_ok(&root, &["remote", "get-url", "xai-org"]) {
        env::var("UPSTREAM_REMOTE").unwrap_or_else(|_| "xai-org".into())
    } else if git_status_ok(&root, &["remote", "get-url", "upstream"]) {
        env::var("UPSTREAM_REMOTE").unwrap_or_else(|_| "upstream".into())
    } else {
        let _ = writeln!(io::stderr(), "error: add remote xai-org or upstream first");
        return ExitCode::from(1);
    };
    let upstream_branch = env::var("UPSTREAM_BRANCH").unwrap_or_else(|_| "main".into());
    let import_log =
        env::var("IMPORT_LOG").unwrap_or_else(|_| "docs/upstream-import-log.md".into());

    let original_branch = git_stdout_lossy(&root, &["branch", "--show-current"]);
    let original_head = git_stdout_lossy(&root, &["rev-parse", "HEAD"]);

    let _ = git_run(&root, &["fetch", &remote, &upstream_branch, "--force"]);
    let _ = git_run(&root, &["fetch", "origin", "main"]);

    let xai_tip = if let Some(arg) = raw.first() {
        match git_stdout(&root, &["rev-parse", arg]) {
            Ok(t) => t,
            Err(e) => {
                let _ = writeln!(io::stderr(), "error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        let r = format!("{remote}/{upstream_branch}");
        match git_stdout(&root, &["rev-parse", &r]) {
            Ok(t) => t,
            Err(e) => {
                let _ = writeln!(io::stderr(), "error: {e}");
                return ExitCode::from(1);
            }
        }
    };
    let xai_short = git_stdout_lossy(&root, &["rev-parse", "--short=12", &xai_tip]);
    let branch = format!("onto-xai/{xai_short}");

    let surmount_label;
    let mut surmount_ref = env::var("SURMOUNT_REF").unwrap_or_default();
    if surmount_ref.is_empty() {
        if !original_branch.is_empty()
            && !original_branch.starts_with("onto-xai/")
            && !original_branch.starts_with("import/")
            && !original_branch.starts_with("backup/")
        {
            surmount_ref = original_head.clone();
            surmount_label = original_branch.clone();
        } else if git_status_ok(
            &root,
            &["show-ref", "--verify", "--quiet", "refs/heads/merge-2"],
        ) {
            surmount_ref = "merge-2".into();
            surmount_label = "merge-2".into();
        } else if git_status_ok(
            &root,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                "refs/remotes/origin/main",
            ],
        ) {
            surmount_ref = "origin/main".into();
            surmount_label = "origin/main".into();
        } else {
            surmount_ref = "main".into();
            surmount_label = "main".into();
        }
    } else {
        surmount_label = surmount_ref.clone();
    }
    let surmount_ref = match git_stdout(&root, &["rev-parse", "--verify", &surmount_ref]) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: SURMOUNT_REF: {e}");
            return ExitCode::from(1);
        }
    };
    let surmount_short = git_stdout_lossy(&root, &["rev-parse", "--short=12", &surmount_ref]);

    let mut seed_ref = env::var("SEED_REF").unwrap_or_default();
    if seed_ref.is_empty() {
        if let Ok(body) = std::fs::read_to_string(root.join(&import_log)) {
            seed_ref = parse_seed_from_import_log(&body).unwrap_or_default();
        }
        if seed_ref.is_empty() {
            seed_ref = DEFAULT_SEED_GIT_ID.into();
        }
    }
    let seed_ref = match git_stdout(&root, &["rev-parse", &seed_ref]) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: SEED_REF: {e}");
            return ExitCode::from(1);
        }
    };
    if !git_status_ok(
        &root,
        &["merge-base", "--is-ancestor", &seed_ref, &surmount_ref],
    ) {
        let _ = writeln!(io::stderr(), "error: SEED_REF not ancestor of SURMOUNT_REF");
        return ExitCode::from(1);
    }

    let branch_ref = format!("refs/heads/{branch}");
    if !continue_flag
        && !force
        && git_status_ok(&root, &["show-ref", "--verify", "--quiet", &branch_ref])
    {
        let existing = git_stdout_lossy(&root, &["rev-parse", &branch]);
        if git_status_ok(&root, &["merge-base", "--is-ancestor", &xai_tip, &existing]) {
            let range = format!("{xai_tip}..{existing}");
            let ahead = git_stdout_lossy(&root, &["rev-list", "--count", &range])
                .parse::<u64>()
                .unwrap_or(0);
            if ahead > 0 {
                println!("=== Stack already present — not rebuilding (safe default) ===");
                println!("Branch:  {branch}");
                println!(
                    "Tip:     {existing} ({})",
                    git_stdout_lossy(&root, &["rev-parse", "--short", &existing])
                );
                println!("xAI tip: {xai_tip} (ancestor: yes)");
                println!("Ahead:   {ahead} commit(s)");
                println!();
                let _ = git_run(
                    &root,
                    &["log", "--oneline", &format!("{xai_tip}..{branch}")],
                );
                println!();
                println!("This is intentional until the stack is merged/ready.");
                println!(
                    "To rebuild from scratch (DESTRUCTIVE): FORCE=1 SURMOUNT_REF={surmount_label} grok-nix-helper put-history-on-xai"
                );
                let head_br = git_stdout_lossy(&root, &["rev-parse", "--abbrev-ref", "HEAD"]);
                if head_br != branch {
                    let _ = git_run(&root, &["checkout", &branch]);
                }
                return ExitCode::SUCCESS;
            }
        }
    }

    let range = format!("{seed_ref}..{surmount_ref}");
    let list_out = if first_parent {
        git_stdout_lossy(&root, &["rev-list", "--reverse", "--first-parent", &range])
    } else {
        git_stdout_lossy(&root, &["rev-list", "--reverse", "--no-merges", &range])
    };
    let mut commits: Vec<String> = list_out
        .lines()
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect();
    if commits.is_empty() {
        let _ = writeln!(
            io::stderr(),
            "error: no commits to cherry-pick between {seed_ref} and {surmount_ref}"
        );
        return ExitCode::from(1);
    }

    println!("=== REAL cherry-pick: Surmount → on top of xAI ===");
    let orig_label = if original_branch.is_empty() {
        "detached"
    } else {
        &original_branch
    };
    println!("Checkout was: {orig_label} ({original_head})");
    println!("xAI tip:      {xai_tip} ({xai_short})");
    println!("Stacking:     {surmount_label} @ {surmount_short}");
    println!("Seed:         {seed_ref}");
    println!("Commits:      {}", commits.len());
    println!("Branch:       {branch}");
    println!("FORCE:        {}", if force { "1" } else { "0" });
    println!();

    if continue_flag {
        if !git_status_ok(&root, &["show-ref", "--verify", "--quiet", &branch_ref]) {
            let _ = writeln!(io::stderr(), "error: {branch} missing; cannot CONTINUE");
            return ExitCode::from(1);
        }
        let _ = git_run(&root, &["checkout", &branch]);
        let log_range = format!("{xai_tip}..HEAD");
        let bodies = git_stdout_lossy(&root, &["log", "--format=%B", &log_range]);
        let done = cherry_picked_from_trailers(&bodies);
        let mut remaining = Vec::new();
        for c in &commits {
            if done.iter().any(|d| d == c) {
                println!(
                    "  skip already applied: {} {}",
                    git_stdout_lossy(&root, &["rev-parse", "--short", c]),
                    git_stdout_lossy(&root, &["log", "-1", "--format=%s", c])
                );
                continue;
            }
            remaining.push(c.clone());
        }
        commits = remaining;
        if commits.is_empty() {
            println!("Nothing left to cherry-pick. Done.");
            let _ = git_run(&root, &["log", "--oneline", &format!("{xai_tip}..HEAD")]);
            return ExitCode::SUCCESS;
        }
        println!("Continuing with {} remaining commit(s)", commits.len());
    } else {
        if git_status_ok(&root, &["show-ref", "--verify", "--quiet", &branch_ref]) {
            if !force {
                let _ = writeln!(
                    io::stderr(),
                    "error: {branch} exists. Refusing to delete (set FORCE=1 to rebuild)."
                );
                return ExitCode::from(1);
            }
            backup_existing_branch(&root, &branch);
            println!(
                "FORCE=1: replacing {branch} ({})",
                git_stdout_lossy(&root, &["rev-parse", "--short", &branch])
            );
            let head_br = git_stdout_lossy(&root, &["rev-parse", "--abbrev-ref", "HEAD"]);
            if head_br == branch {
                let _ = git_run(&root, &["checkout", "--detach", "HEAD"]);
            }
            let _ = git_run(&root, &["branch", "-D", &branch]);
        }
        if git_run(&root, &["checkout", "-B", &branch, &xai_tip])
            .ok()
            .is_none_or(|s| !s.success())
        {
            let _ = writeln!(io::stderr(), "error: checkout -B {branch} failed");
            return ExitCode::from(1);
        }
    }

    for c in &commits {
        let subj = git_stdout_lossy(&root, &["log", "-1", "--format=%s", c]);
        let short = git_stdout_lossy(&root, &["rev-parse", "--short", c]);
        println!(">>> cherry-pick {short} {subj}");
        match git_run(&root, &["cherry-pick", "-x", c]) {
            Ok(s) if s.success() => {
                println!(
                    "    ok → {}",
                    git_stdout_lossy(&root, &["rev-parse", "--short", "HEAD"])
                );
            }
            _ => {
                println!();
                println!("CONFLICT while cherry-picking {short} ({subj})");
                println!("Resolve every conflict, then:");
                println!("  git add -u");
                println!("  git cherry-pick --continue");
                println!("  CONTINUE=1 grok-nix-helper put-history-on-xai");
                println!("Or abort and restore backup:");
                println!("  git cherry-pick --abort");
                println!();
                println!("Unmerged:");
                let _ = git_run(&root, &["diff", "--name-only", "--diff-filter=U"]);
                return ExitCode::from(2);
            }
        }
    }

    println!();
    println!("=== Done (real stack) ===");
    println!("Branch: {branch}");
    println!(
        "Tip:    {}",
        git_stdout_lossy(&root, &["rev-parse", "HEAD"])
    );
    let ancestor = if git_status_ok(&root, &["merge-base", "--is-ancestor", &xai_tip, "HEAD"]) {
        "yes"
    } else {
        "NO"
    };
    println!("xAI is ancestor: {ancestor}");
    println!("Commits on top of xAI:");
    let _ = git_run(&root, &["log", "--oneline", &format!("{xai_tip}..HEAD")]);
    println!();
    println!("Diff vs xAI tip (summary):");
    let _ = git_run(&root, &["diff", "--stat", &xai_tip, "HEAD"]);
    println!();
    println!("Surmount product branches were NOT modified.");
    println!("XAI_TIP={xai_tip}");
    println!("ONTO_BRANCH={branch}");
    println!(
        "ONTO_TIP={}",
        git_stdout_lossy(&root, &["rev-parse", "HEAD"])
    );
    println!("SURMOUNT_REF={surmount_ref}");
    ExitCode::SUCCESS
}

fn backup_existing_branch(root: &Path, branch: &str) {
    let branch_ref = format!("refs/heads/{branch}");
    if !git_status_ok(root, &["show-ref", "--verify", "--quiet", &branch_ref]) {
        return;
    }
    let short = git_stdout_lossy(root, &["rev-parse", "--short", branch]);
    let stamp = utc_stamp();
    let bak = format!("backup/{}-{short}-{stamp}", branch.replace('/', "-"));
    if git_run(root, &["branch", &bak, branch])
        .ok()
        .is_some_and(|s| s.success())
    {
        println!("Backed up previous {branch} → {bak}");
    }
}

fn utc_stamp() -> String {
    let out = std::process::Command::new("date")
        .args(["-u", "+%Y%m%dT%H%M%SZ"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_seed_is_git_object_id() {
        assert_eq!(DEFAULT_SEED_GIT_ID.len(), 40);
        assert!(DEFAULT_SEED_GIT_ID.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn seed_from_import_log_prefers_seed_row() {
        let log = "| 2024-01-01 | `b189869b7755d2b482969acf6c92da3ecfeffd36` | `cccccccccccccccccccccccccccccccccccccccc` | seed import |\n";
        assert_eq!(
            parse_seed_from_import_log(log).as_deref(),
            Some("b189869b7755d2b482969acf6c92da3ecfeffd36")
        );
    }

    #[test]
    fn cherry_pick_trailer_parser() {
        let body =
            "Subject\n\n(cherry picked from commit abcdefabcdefabcdefabcdefabcdefabcdefabcd)\n";
        let ids = cherry_picked_from_trailers(body);
        assert_eq!(ids, vec!["abcdefabcdefabcdefabcdefabcdefabcdefabcd"]);
    }
}
