//! Surmount fork-only paths restored after applying an xAI export tree.
//!
//! Authority for import restore. Product seams inside xai-grok-* are not
//! listed. Recon lives in grok-nix-helper, not scripts/*.sh.

/// Paths restored from BASE after applying the xAI tree.
pub const FORK_PATHS: &[&str] = &[
    "FORK.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "README.md",
    "justfile",
    "flake.nix",
    "flake",
    "flake.lock",
    "packaging",
    "AGENTS.md",
    "RESIDUAL.md",
    "docs/upstream-history.md",
    "docs/upstream-import-log.md",
    "docs/upstream-onto-log.md",
    "docs/git-workflow.md",
    "docs/dev",
    "doc/dev",
    "crates/codegen/grok-nix-helper",
    ".github/workflows/upstream-export.yml",
    ".github/workflows/ci.yml",
    ".grok/workflows",
    ".agents/skills",
    "crates/codegen/grok-rate-limit",
];

#[cfg(test)]
fn is_deleted_recon_shell(path: &str) -> bool {
    matches!(
        path,
        "scripts/detect-upstream-export.sh"
            | "scripts/import-upstream-export.sh"
            | "scripts/sync-upstream.sh"
            | "scripts/put-history-on-xai.sh"
            | "scripts/replay-onto-upstream.sh"
            | "scripts/join-main-into-onto.sh"
            | "scripts/recon-status.sh"
            | "scripts/extract-debug-sidecar.sh"
            | "scripts/assert-process-pins.sh"
            | "scripts/with-ci-hermetic-path.sh"
            | "scripts/ensure-working-nix-path.sh"
            | "scripts/nix-current-system.sh"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_paths_do_not_list_replaced_shell_recon() {
        for p in FORK_PATHS {
            assert!(
                !is_deleted_recon_shell(p),
                "FORK_PATHS still lists deleted recon shell: {p}"
            );
            assert!(
                !p.ends_with(".sh"),
                "FORK_PATHS must not list recon .sh after conversion: {p}"
            );
        }
        assert!(FORK_PATHS.contains(&"crates/codegen/grok-nix-helper"));
        assert!(FORK_PATHS.contains(&"flake"));
        assert!(FORK_PATHS.contains(&"AGENTS.md"));
        assert!(FORK_PATHS.contains(&"FORK.md"));
        assert!(FORK_PATHS.contains(&"justfile"));
    }

    #[test]
    fn fork_paths_keep_process_pins_and_packaging() {
        for need in [
            "RESIDUAL.md",
            "docs/upstream-history.md",
            "packaging",
            ".grok/workflows",
            ".agents/skills",
        ] {
            assert!(
                FORK_PATHS.contains(&need),
                "missing FORK_PATHS entry {need}"
            );
        }
    }
}
