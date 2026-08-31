//! Read-only recon probe: cherry-pick / merge / onto / next human action.
//! Never commits, aborts, or FORCE-rebuilds.

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::git_cmd::{git_path, git_status_ok, git_stdout, git_stdout_lossy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconProbe {
    pub branch: String,
    pub cherry_pick: bool,
    pub merge_head: bool,
    pub sequencer: bool,
    pub unmerged: Vec<String>,
    pub onto_ish: bool,
    pub onto_name: String,
    pub main_ancestor: MainAncestor,
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainAncestor {
    Yes,
    No,
    Unknown,
}

impl MainAncestor {
    fn as_str(self) -> &'static str {
        match self {
            MainAncestor::Yes => "yes",
            MainAncestor::No => "no",
            MainAncestor::Unknown => "unknown",
        }
    }
}

/// Recommended next human action (plain English; no invented modes).
pub fn next_action(p: &ReconProbe) -> String {
    if !p.unmerged.is_empty() {
        if p.cherry_pick || p.sequencer {
            return "resolve UU paths (spawn if multi-file), stage, then human: git cherry-pick --continue (signed TTY)".into();
        }
        if p.merge_head {
            return "resolve UU paths, stage, then human: git commit -S (finish merge)".into();
        }
        return "resolve UU paths and stage; re-run recon-status for next step".into();
    }
    if p.cherry_pick || p.sequencer {
        return "human: git cherry-pick --continue (signed TTY); then CONTINUE=1 SURMOUNT_REF=origin/main grok-nix-helper put-history-on-xai if stack continues".into();
    }
    if p.merge_head {
        return "human: git commit -S (join/merge already staged — do not invent new merge)".into();
    }
    if p.onto_ish && p.main_ancestor == MainAncestor::No {
        return "run grok-nix-helper join-main-into-onto (stages -s ours), then human: git commit -S join message".into();
    }
    if p.onto_ish && p.main_ancestor == MainAncestor::Yes {
        return "clean recon state (onto tip; main is ancestor). Land: grok-nix-helper assert-process-pins HEAD; walk FORK/catalog named tests in doc/dev/upstream-regression-filters.md (seven product classes plus listed neighbors; rg each identifier for fn; chrome-only is a failed land); just check is quality only. Push/PR only if asked".into();
    }
    "clean (not mid cherry-pick/merge). Route if needed: grok-nix-helper detect-upstream-export or put-history / import (see git-recon recon:route)".into()
}

pub fn probe(root: &Path) -> Result<ReconProbe, String> {
    if !git_status_ok(root, &["rev-parse", "--git-dir"]) {
        return Err(format!(
            "error: not a git repository (cwd={})",
            root.display()
        ));
    }
    let mut branch = git_stdout_lossy(root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    if branch.is_empty() || branch == "HEAD" {
        let short = git_stdout_lossy(root, &["rev-parse", "--short", "HEAD"]);
        let short = if short.is_empty() {
            "unknown".into()
        } else {
            short
        };
        branch = format!("DETACHED@{short}");
    }
    let cherry_pick = git_path(root, "CHERRY_PICK_HEAD").is_file();
    let merge_head = git_path(root, "MERGE_HEAD").is_file();
    let sequencer = git_path(root, "sequencer").is_dir();

    let unmerged_raw =
        git_stdout(root, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
    let mut unmerged: Vec<String> = unmerged_raw
        .lines()
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect();
    if unmerged.len() == 1 && unmerged[0].is_empty() {
        unmerged.clear();
    }

    let onto_ish = branch.starts_with("onto-xai/");
    let onto_name = if onto_ish {
        branch.clone()
    } else {
        String::new()
    };

    let main_ancestor = if git_status_ok(root, &["rev-parse", "--verify", "origin/main"]) {
        if git_status_ok(
            root,
            &["merge-base", "--is-ancestor", "origin/main", "HEAD"],
        ) {
            MainAncestor::Yes
        } else {
            MainAncestor::No
        }
    } else if git_status_ok(root, &["rev-parse", "--verify", "main"]) {
        if git_status_ok(root, &["merge-base", "--is-ancestor", "main", "HEAD"]) {
            MainAncestor::Yes
        } else {
            MainAncestor::No
        }
    } else {
        MainAncestor::Unknown
    };

    let dirty = crate::git_cmd::is_dirty(root);
    Ok(ReconProbe {
        branch,
        cherry_pick,
        merge_head,
        sequencer,
        unmerged,
        onto_ish,
        onto_name,
        main_ancestor,
        dirty,
    })
}

pub fn print_probe(p: &ReconProbe) {
    let yn = |b: bool| if b { "yes" } else { "no" };
    println!("branch:           {}", p.branch);
    println!("CHERRY_PICK_HEAD: {}", yn(p.cherry_pick));
    println!("MERGE_HEAD:       {}", yn(p.merge_head));
    println!("sequencer:        {}", yn(p.sequencer));
    println!("unmerged:         {}", p.unmerged.len());
    if !p.unmerged.is_empty() {
        let max_show = 40usize;
        for (i, path) in p.unmerged.iter().enumerate() {
            if i >= max_show {
                println!("  ... and {} more", p.unmerged.len() - max_show);
                break;
            }
            println!("  - {path}");
        }
    }
    if !p.onto_name.is_empty() {
        println!("onto-ish:         {} ({})", yn(p.onto_ish), p.onto_name);
    } else {
        println!("onto-ish:         {}", yn(p.onto_ish));
    }
    println!("main_ancestor:    {}", p.main_ancestor.as_str());
    println!("dirty_worktree:   {}", yn(p.dirty));
    println!("next:             {}", next_action(p));
}

pub fn run(_args: &[String]) -> ExitCode {
    let root = crate::git_cmd::find_repo_root();
    match probe(&root) {
        Ok(p) => {
            print_probe(&p);
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ReconProbe {
        ReconProbe {
            branch: "main".into(),
            cherry_pick: false,
            merge_head: false,
            sequencer: false,
            unmerged: vec![],
            onto_ish: false,
            onto_name: String::new(),
            main_ancestor: MainAncestor::Unknown,
            dirty: false,
        }
    }

    #[test]
    fn next_action_never_names_deleted_shell_scripts() {
        let cases = [
            ReconProbe {
                cherry_pick: true,
                ..base()
            },
            ReconProbe {
                onto_ish: true,
                onto_name: "onto-xai/deadbeef".into(),
                main_ancestor: MainAncestor::No,
                branch: "onto-xai/deadbeef".into(),
                ..base()
            },
            ReconProbe {
                onto_ish: true,
                onto_name: "onto-xai/deadbeef".into(),
                main_ancestor: MainAncestor::Yes,
                branch: "onto-xai/deadbeef".into(),
                ..base()
            },
            ReconProbe {
                unmerged: vec!["foo.rs".into()],
                merge_head: true,
                ..base()
            },
            base(),
        ];
        for p in cases {
            let n = next_action(&p);
            assert!(
                !n.contains(".sh"),
                "next action must not name deleted shell scripts: {n}"
            );
            assert!(
                !n.contains("scripts/put-history") && !n.contains("scripts/join-main"),
                "next action must name grok-nix-helper, not scripts/: {n}"
            );
        }
    }

    #[test]
    fn onto_without_main_ancestor_hands_join_helper() {
        let p = ReconProbe {
            branch: "onto-xai/abc".into(),
            onto_ish: true,
            onto_name: "onto-xai/abc".into(),
            main_ancestor: MainAncestor::No,
            ..base()
        };
        let n = next_action(&p);
        assert!(n.contains("grok-nix-helper join-main-into-onto"));
        assert!(n.contains("git commit -S"));
    }

    #[test]
    fn cherry_pick_continue_names_put_history_helper() {
        let p = ReconProbe {
            cherry_pick: true,
            ..base()
        };
        let n = next_action(&p);
        assert!(n.contains("git cherry-pick --continue"));
        assert!(n.contains("grok-nix-helper put-history-on-xai"));
    }
}
