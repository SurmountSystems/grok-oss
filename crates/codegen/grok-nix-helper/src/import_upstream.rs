//! Absorb an xAI export INTO Surmount as a content-import branch.
//!
//! Prepares the tree and stages. Default: hands `git commit -S` to a human
//! TTY. DO_COMMIT=1 tries a signed commit. Never suggests gpgsign=false.

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::assert_process_pins;
use crate::fork_paths::FORK_PATHS;
use crate::git_cmd::{
    env_flag, git_output, git_run, git_status_ok, git_stdout, git_stdout_lossy, refuse_dirty,
};

pub fn import_commit_message(
    xai_short: &str,
    xai_tip: &str,
    xai_tree: &str,
    base_ref: &str,
) -> String {
    format!(
        "Import xAI monorepo export {xai_short}\n\n\
Source: xai-org/grok-build {xai_tip}\n\
Tree:   {xai_tree}\n\n\
Content-only import (orphan export has no merge-base with Surmount).\n\
Fork-only paths restored from {base_ref} where present.\n\
Review: docs/upstream-history.md checklist; then append docs/upstream-import-log.md.\n"
    )
}

pub fn signed_commit_hand_command(msg: &str) -> String {
    let first = msg.lines().next().unwrap_or("Import xAI monorepo export");
    format!("git commit -S -m \"{first}\"")
}

/// Switch back to the caller's branch only after a signed import commit exists.
/// Uncommitted staged import stays on `import/*` (same as `--stay`).
pub fn should_checkout_original(stay: bool, committed: bool) -> bool {
    !stay && committed
}

fn path_on_tree(root: &Path, tree_ish: &str, p: &str) -> bool {
    let spec = format!("{tree_ish}:{p}");
    git_status_ok(root, &["cat-file", "-e", &spec])
        || git_status_ok(root, &["ls-tree", "-d", "--name-only", tree_ish, p])
            && !git_stdout_lossy(root, &["ls-tree", "-d", "--name-only", tree_ish, p]).is_empty()
}

pub fn run(raw: &[String]) -> ExitCode {
    let root = crate::git_cmd::find_repo_root();
    let mut args: Vec<String> = raw.to_vec();
    let stay = args.first().map(|s| s.as_str()) == Some("--stay");
    if stay {
        args.remove(0);
    }

    let original_branch = git_stdout_lossy(&root, &["branch", "--show-current"]);
    let original_head = git_stdout_lossy(&root, &["rev-parse", "HEAD"]);

    if let Err(c) = refuse_dirty(&root, env_flag("ALLOW_DIRTY")) {
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

    let _ = git_run(&root, &["fetch", &remote, &upstream_branch, "--force"]);
    let _ = git_run(&root, &["fetch", "origin", "main"]);

    let xai_tip = if let Some(arg) = args.first() {
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
    let tree_spec = format!("{xai_tip}^{{tree}}");
    let xai_tree = match git_stdout(&root, &["rev-parse", &tree_spec]) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: {e}");
            return ExitCode::from(1);
        }
    };
    let xai_short = git_stdout_lossy(&root, &["rev-parse", "--short=12", &xai_tip]);

    let mut base_ref = env::var("BASE_REF").unwrap_or_default();
    if base_ref.is_empty() {
        if git_status_ok(
            &root,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                "refs/remotes/origin/main",
            ],
        ) {
            base_ref = "origin/main".into();
        } else if git_status_ok(
            &root,
            &["show-ref", "--verify", "--quiet", "refs/heads/main"],
        ) {
            base_ref = "main".into();
        } else {
            let _ = writeln!(
                io::stderr(),
                "error: cannot find origin/main or main; set BASE_REF="
            );
            return ExitCode::from(1);
        }
    }
    let base_ref = match git_stdout(&root, &["rev-parse", "--verify", &base_ref]) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: BASE_REF: {e}");
            return ExitCode::from(1);
        }
    };

    if !original_branch.is_empty() && original_branch != "main" {
        let range = format!("{base_ref}..{original_head}");
        let ahead = git_stdout_lossy(&root, &["rev-list", "--count", &range]);
        if ahead != "0" && !ahead.is_empty() {
            println!("NOTE: you are on '{original_branch}' ({ahead} commit(s) not in {base_ref}).");
            println!(
                "      Import will base on {base_ref} only — your feature commits are NOT included."
            );
            println!(
                "      Typical order: merge feature → main (no rebase of published PRs), then import; or set"
            );
            println!(
                "      BASE_REF={original_branch} if this import should sit on the feature tip."
            );
            println!();
        }
    }

    let branch = format!("import/xai-export-{xai_short}");
    let branch_ref = format!("refs/heads/{branch}");
    if git_status_ok(&root, &["show-ref", "--verify", "--quiet", &branch_ref]) {
        let _ = writeln!(
            io::stderr(),
            "error: branch {branch} already exists. Delete or rename it first:"
        );
        let _ = writeln!(io::stderr(), "  git branch -D {branch}");
        return ExitCode::from(1);
    }

    println!("Original: {original_branch} ({original_head})");
    println!(
        "Base:     {base_ref} ({})",
        git_stdout_lossy(&root, &["rev-parse", "--short", &base_ref])
    );
    println!("xAI tip:  {xai_tip} ({xai_short})");
    println!("xAI tree: {xai_tree}");
    println!("Branch:   {branch}");
    println!();

    if git_run(&root, &["checkout", "-B", &branch, &base_ref])
        .ok()
        .is_none_or(|s| !s.success())
    {
        let _ = writeln!(io::stderr(), "error: checkout -B {branch} failed");
        return ExitCode::from(1);
    }

    println!("Applying xAI export tree to index + worktree (read-tree -u --reset) ...");
    if git_run(&root, &["read-tree", "-u", "--reset", &xai_tree])
        .ok()
        .is_none_or(|s| !s.success())
    {
        let _ = writeln!(io::stderr(), "error: git read-tree failed");
        return ExitCode::from(1);
    }

    println!("Restoring Surmount fork-only paths from base ...");
    for p in FORK_PATHS {
        if path_on_tree(&root, &base_ref, p) {
            if git_run(&root, &["checkout", &base_ref, "--", p])
                .ok()
                .is_some_and(|s| s.success())
            {
                println!("  keep fork path: {p}");
            } else {
                let _ = writeln!(io::stderr(), "  WARN: could not checkout fork path: {p}");
            }
        } else {
            println!("  skip (absent on base): {p}");
        }
    }

    if root.join("result").exists() || root.join("result").is_symlink() {
        let _ = git_run(&root, &["rm", "-f", "--ignore-unmatch", "result"]);
        let _ = std::fs::remove_file(root.join("result"));
        println!("  removed result (nix build symlink)");
    }

    println!();
    println!("Asserting process-pin paths after restore ...");
    let pin = assert_process_pins::run(&["--root".into(), root.to_string_lossy().into_owned()]);
    if pin != ExitCode::SUCCESS {
        let _ = writeln!(
            io::stderr(),
            "error: process pins missing after FORK_PATHS restore."
        );
        let _ = writeln!(
            io::stderr(),
            "  Extend FORK_PATHS or ensure base ({base_ref}) has the paths."
        );
        let _ = writeln!(
            io::stderr(),
            "  You are on {branch} with a partial import tree; original was {original_branch}"
        );
        return ExitCode::from(1);
    }

    println!();
    println!("NOTE: Product seams inside xai-grok-* are not path-restored. Walk the");
    println!("seven product land classes in FORK.md Land checklist and");
    println!("doc/dev/upstream-regression-filters.md Required land inventory:");
    println!("  1. CLI identity (first token grok-oss)");
    println!("  2. Config is a surface (/settings rows plus runtime readers)");
    println!("  3. grok-oss ledger /spend ingest of usage.jsonl");
    println!("  4. DOGE / chrome paint (theme file is not paint)");
    println!("  5. Dual-auth hop after included SuperGrok period limits are full");
    println!("  6. Last-session on start");
    println!("  7. Product skills are not a Python runtime");
    println!(
        "Chrome-only, paint-only bubble copy, or skills Python reintroduced is a failed land."
    );
    println!("OpenRouter and grok-rate-limit still need a crate-side reconcile:");
    println!("  git diff {base_ref} -- crates/codegen/xai-grok-shell/src/auth/openrouter.rs");
    println!("  git diff {base_ref} -- crates/codegen/xai-grok-pager-bin/");
    println!("  git diff {base_ref} -- crates/codegen/xai-grok-sampler/");
    println!("  git diff {base_ref} -- crates/codegen/grok-rate-limit/");
    println!();

    let cached_quiet = git_status_ok(&root, &["diff", "--cached", "--quiet"]);
    let work_quiet = git_status_ok(&root, &["diff", "--quiet"]);
    if cached_quiet && work_quiet {
        println!("Nothing to commit (tree already matches base+export composition).");
        println!("Staying on {branch} (no new import commit).");
        return ExitCode::SUCCESS;
    }

    let _ = git_output(&root, &["update-index", "--refresh"]);
    let _ = git_run(&root, &["add", "-u"]);
    for p in FORK_PATHS {
        let exists = root.join(p).exists() || root.join(p).is_dir();
        if exists {
            let _ = git_run(&root, &["add", "-f", "--", p]);
        }
    }

    let msg = import_commit_message(&xai_short, &xai_tip, &xai_tree, &base_ref);
    let mut committed = false;
    if env_flag("DO_COMMIT") {
        let first = msg.lines().next().unwrap_or("Import xAI monorepo export");
        let extra = msg.lines().skip(2).collect::<Vec<_>>().join("\n");
        let st = if extra.is_empty() {
            git_run(&root, &["commit", "-S", "-m", first])
        } else {
            git_run(&root, &["commit", "-S", "-m", first, "-m", &extra])
        };
        match st {
            Ok(s) if s.success() => {
                committed = true;
                let new_sha = git_stdout_lossy(&root, &["rev-parse", "HEAD"]);
                println!();
                println!(
                    "Created commit {} on {branch}",
                    git_stdout_lossy(&root, &["rev-parse", "--short", &new_sha])
                );
            }
            _ => {
                let _ = writeln!(
                    io::stderr(),
                    "error: commit failed (GPG/TTY?). Import is still staged."
                );
                let _ = writeln!(io::stderr(), "On a real TTY run:");
                let _ = writeln!(io::stderr(), "  {}", signed_commit_hand_command(&msg));
                let _ = writeln!(
                    io::stderr(),
                    "You are on {branch}; original branch was {original_branch}"
                );
                return ExitCode::from(1);
            }
        }
    } else {
        println!("=== Import staged; tree prepared (no commit) ===");
        println!("The helper does not create a commit object. On a real TTY (signed):");
        println!("  {}", signed_commit_hand_command(&msg));
        println!("  git commit -S -F - <<'EOF'");
        print!("{msg}");
        println!("EOF");
        println!();
        println!("Never use commit.gpgsign=false or --no-gpg-sign.");
        println!("Staying on {branch} until that signed commit exists.");
    }

    println!("  vs base:  git diff --stat {base_ref} HEAD");
    println!("  vs xAI:   git diff --stat {xai_tree} HEAD^{{tree}}   # fork-only delta");
    println!();
    println!("=== Review checklist ===");
    println!("1. git diff {base_ref} --stat");
    println!("2. Walk the seven product land classes (catalog + FORK). Assert is files only.");
    println!(
        "3. Process pins: grok-nix-helper assert-process-pins  (AGENTS, FORK, RESIDUAL, helper crate, ...)"
    );
    println!("4. just ci  (quality only; cannot fail a deleted catalog test)");
    println!("5. Append docs/upstream-import-log.md");
    println!("6. Sign if needed: git commit --amend -S --no-edit");
    println!("7. PR {branch} -> main  (do not force-push main to xAI)");
    println!();
    println!("XAI_TIP={xai_tip}");
    println!("XAI_TREE={xai_tree}");
    println!("IMPORT_BRANCH={branch}");
    println!("BASE_REF={base_ref}");

    if should_checkout_original(stay, committed)
        && !original_branch.is_empty()
        && original_branch != branch
    {
        println!();
        println!("Returning to your previous branch: {original_branch}");
        println!("(import branch left in place: {branch}; use --stay to remain on it)");
        match git_run(&root, &["checkout", &original_branch]) {
            Ok(s) if s.success() => {}
            _ => {
                let _ = writeln!(
                    io::stderr(),
                    "error: git checkout {original_branch} failed. You are still on {branch}."
                );
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_message_names_tree_and_source() {
        let msg = import_commit_message(
            "abc123",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "origin/main",
        );
        assert!(msg.contains("Import xAI monorepo export abc123"));
        assert!(msg.contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        assert!(msg.contains("origin/main"));
        assert!(!msg.contains("gpgsign=false"));
    }

    #[test]
    fn hand_command_is_signed_only() {
        let cmd = signed_commit_hand_command("Import xAI monorepo export abc\n\nbody\n");
        assert!(cmd.contains("git commit -S"));
        assert!(!cmd.contains("gpgsign=false"));
        assert!(!cmd.contains("--no-gpg-sign"));
    }

    #[test]
    fn default_uncommitted_import_does_not_claim_checkout_back() {
        assert!(!should_checkout_original(false, false));
        assert!(should_checkout_original(false, true));
        assert!(!should_checkout_original(true, true));
        assert!(!should_checkout_original(true, false));
    }
}
