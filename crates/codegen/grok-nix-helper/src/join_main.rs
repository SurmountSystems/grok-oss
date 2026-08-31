//! Join Surmount `main` into an onto-xai/* tip (`merge -s ours`).
//!
//! Default: stages the merge and hands `git commit -S` to a human TTY.
//! DO_COMMIT=1 tries a signed commit. Never gpgsign=false.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use crate::git_cmd::{
    env_flag, git_path, git_run, git_status_ok, git_stdout, git_stdout_lossy, refuse_dirty,
};

pub const JOIN_SUBJECT: &str = "Merge Surmount main into onto-xai (keep tip tree)";
pub const JOIN_BODY_1: &str = "Join Surmount archive history so main is an ancestor of this tip.";
pub const JOIN_BODY_2: &str =
    "Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main.";

pub fn signed_join_hand_command() -> String {
    format!(
        "git commit -S -m \"{JOIN_SUBJECT}\" \\\n    -m \"{JOIN_BODY_1}\" \\\n    -m \"{JOIN_BODY_2}\""
    )
}

pub fn run(_raw: &[String]) -> ExitCode {
    let root = crate::git_cmd::find_repo_root();
    let do_commit = env_flag("DO_COMMIT");
    let force = env_flag("FORCE");
    let dry_run = env_flag("DRY_RUN");

    if let Err(c) = refuse_dirty(&root, env_flag("ALLOW_DIRTY")) {
        return ExitCode::from(c as u8);
    }

    if git_path(&root, "MERGE_HEAD").is_file() {
        let _ = writeln!(
            io::stderr(),
            "error: merge already in progress. Finish or abort first:"
        );
        let _ = writeln!(io::stderr(), "  git commit -S   # or: git merge --abort");
        return ExitCode::from(1);
    }

    let mut main_ref = env::var("MAIN_REF").unwrap_or_default();
    if main_ref.is_empty() {
        if git_status_ok(&root, &["rev-parse", "--verify", "origin/main"]) {
            main_ref = "origin/main".into();
        } else {
            main_ref = "main".into();
        }
    }
    let main_ref = match git_stdout(&root, &["rev-parse", "--verify", &main_ref]) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: MAIN_REF: {e}");
            return ExitCode::from(1);
        }
    };
    let main_short = git_stdout_lossy(&root, &["rev-parse", "--short=12", &main_ref]);

    let mut onto_ref = env::var("ONTO_REF").unwrap_or_default();
    if onto_ref.is_empty() {
        onto_ref = git_stdout_lossy(&root, &["branch", "--show-current"]);
        if onto_ref.is_empty() {
            let _ = writeln!(
                io::stderr(),
                "error: detached HEAD; set ONTO_REF=onto-xai/<short> or checkout a branch"
            );
            return ExitCode::from(1);
        }
    }
    if !onto_ref.starts_with("onto-xai/") && !env_flag("ALLOW_NON_ONTO") {
        let _ = writeln!(
            io::stderr(),
            "error: expected onto-xai/* branch (got: {onto_ref})."
        );
        let _ = writeln!(
            io::stderr(),
            "  Checkout onto-xai/<tip> or set ALLOW_NON_ONTO=1 if intentional."
        );
        return ExitCode::from(1);
    }

    if git_run(&root, &["checkout", &onto_ref])
        .ok()
        .is_none_or(|s| !s.success())
    {
        let _ = writeln!(io::stderr(), "error: checkout {onto_ref} failed");
        return ExitCode::from(1);
    }
    let onto_tip = git_stdout_lossy(&root, &["rev-parse", "HEAD"]);
    let onto_tree = git_stdout_lossy(&root, &["rev-parse", "HEAD^{tree}"]);
    let onto_short = git_stdout_lossy(&root, &["rev-parse", "--short=12", "HEAD"]);

    println!("=== Join Surmount main into onto (strategy ours) ===");
    println!("Onto branch: {onto_ref} @ {onto_short}");
    println!("Onto tree:   {onto_tree}");
    println!("Main ref:    {main_ref} ({main_short})");
    println!();

    if git_status_ok(&root, &["merge-base", "--is-ancestor", &main_ref, "HEAD"]) {
        if !force {
            println!("=== Already joined — main is an ancestor of HEAD (safe default) ===");
            println!("Tip:  {}", git_stdout_lossy(&root, &["rev-parse", "HEAD"]));
            println!("To force another ours-merge: FORCE=1 grok-nix-helper join-main-into-onto");
            return ExitCode::SUCCESS;
        }
        let _ = writeln!(
            io::stderr(),
            "WARN: main already ancestor; FORCE=1 continues"
        );
    }

    if dry_run {
        println!("DRY_RUN=1 — would run:");
        println!("  git merge -s ours {main_ref} --allow-unrelated-histories --no-commit");
        println!("  verify tree == {onto_tree}");
        println!("  git commit -S  # human TTY when commit.gpgsign=true");
        return ExitCode::SUCCESS;
    }

    if main_ref.contains("origin/")
        || git_status_ok(
            &root,
            &["rev-parse", "--verify", &format!("refs/remotes/{main_ref}")],
        )
    {
        let _ = git_run(&root, &["fetch", "origin", "main"]);
    }

    if git_run(
        &root,
        &[
            "merge",
            "-s",
            "ours",
            &main_ref,
            "--allow-unrelated-histories",
            "--no-commit",
            "-m",
            JOIN_SUBJECT,
        ],
    )
    .ok()
    .is_none_or(|s| !s.success())
    {
        let _ = writeln!(io::stderr(), "error: git merge -s ours failed");
        return ExitCode::from(1);
    }

    let new_tree = git_stdout_lossy(&root, &["write-tree"]);
    if new_tree != onto_tree {
        let _ = writeln!(
            io::stderr(),
            "error: post-merge tree {new_tree} != pre-merge onto tree {onto_tree}"
        );
        let _ = writeln!(io::stderr(), "  aborting merge");
        let _ = git_run(&root, &["merge", "--abort"]);
        return ExitCode::from(1);
    }

    println!("Tree identity OK: {new_tree}");
    println!();

    if do_commit {
        match git_run(
            &root,
            &[
                "commit",
                "-S",
                "-m",
                JOIN_SUBJECT,
                "-m",
                JOIN_BODY_1,
                "-m",
                JOIN_BODY_2,
            ],
        ) {
            Ok(s) if s.success() => {
                println!("=== Merge committed ===");
                let _ = git_run(&root, &["log", "--oneline", "--graph", "-8"]);
            }
            _ => {
                let _ = writeln!(
                    io::stderr(),
                    "error: commit failed (GPG/TTY?). Merge is still staged."
                );
                let _ = writeln!(io::stderr(), "On a real TTY run:");
                let _ = writeln!(io::stderr(), "  {}", signed_join_hand_command());
                return ExitCode::from(1);
            }
        }
    } else {
        println!("=== Merge staged (--no-commit); tree kept ===");
        println!("Pre-merge tip: {onto_tip}");
        println!("On a real TTY (signed):");
        println!("  {}", signed_join_hand_command());
        println!();
        println!("Then verify:");
        println!("  git merge-base --is-ancestor {main_short} HEAD");
        println!("  test \"$(git rev-parse HEAD^{{tree}})\" = \"{onto_tree}\"");
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_command_is_signed_ours_join() {
        let cmd = signed_join_hand_command();
        assert!(cmd.contains("git commit -S"));
        assert!(cmd.contains(JOIN_SUBJECT));
        assert!(!cmd.contains("gpgsign=false"));
        assert!(!cmd.contains("--no-gpg-sign"));
    }
}
