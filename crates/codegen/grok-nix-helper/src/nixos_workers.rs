//! Source contracts for the host-importable grok-oss workers NixOS fragment.
//! Isolated crate tests skip when the repo root is not next to this crate
//! (crane helperSrc). Quality workspace nextest from the full tree proves these.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> Option<PathBuf> {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..8 {
        if p.join("justfile").is_file() && p.join("flake.nix").is_file() {
            return Some(p);
        }
        if !p.pop() {
            break;
        }
    }
    None
}

fn skip_or_fragment() -> Option<String> {
    let Some(root) = repo_root() else {
        eprintln!(
            "skipping nixos workers contracts: repo root not next to crate (isolated helperSrc)"
        );
        return None;
    };
    let rel = "packaging/nixos/grok-oss-workers.nix";
    Some(fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}")))
}

fn production_lines(src: &str) -> impl Iterator<Item = &str> {
    src.lines().filter(|line| {
        let t = line.trim();
        !t.is_empty() && !t.starts_with('#')
    })
}

#[test]
fn grok_oss_workers_nix_requires_memory_max() {
    let Some(src) = skip_or_fragment() else {
        return;
    };
    assert!(
        src.contains("MemoryMax"),
        "packaging/nixos/grok-oss-workers.nix must set systemd MemoryMax"
    );
    assert!(
        src.contains("memoryMax") && src.contains("!= \"\""),
        "packaging/nixos/grok-oss-workers.nix must refuse an empty MemoryMax when enable is true:\n{src}"
    );
}

#[test]
fn grok_oss_workers_nix_does_not_start_nix_daemon() {
    let Some(src) = skip_or_fragment() else {
        return;
    };
    for line in production_lines(&src) {
        assert!(
            !line.contains("nix.daemon") && !line.contains("nix-daemon"),
            "workers fragment must not set nix.daemon or start an extra nix-daemon:\n{line}"
        );
    }
}

#[test]
fn grok_oss_workers_nix_does_not_disable_surmount_scram() {
    let Some(src) = skip_or_fragment() else {
        return;
    };
    assert!(
        src.contains("Workers are killable; scram is not"),
        "workers fragment must document that workers are killable and scram is not:\n{src}"
    );
    for line in production_lines(&src) {
        let t = line.replace(' ', "");
        assert!(
            !t.contains("surmount-scram.enable=false")
                && !t.contains("services.surmount-scram.enable=false"),
            "workers fragment must not disable surmount-scram:\n{line}"
        );
    }
}

#[test]
fn grok_oss_workers_nix_has_no_docker() {
    let Some(src) = skip_or_fragment() else {
        return;
    };
    assert!(
        !src.to_ascii_lowercase().contains("docker"),
        "workers fragment must not mention docker:\n{src}"
    );
}

#[test]
fn grok_oss_workers_nix_no_boot_tui_and_sshd_class_nice() {
    let Some(src) = skip_or_fragment() else {
        return;
    };
    assert!(
        src.contains("instanceCwds"),
        "workers fragment must offer an optional instance cwd list:\n{src}"
    );
    assert!(
        src.contains("no boot TUI") || src.contains("Does not start the grok-oss TUI on boot"),
        "workers fragment must say it does not start the TUI on boot:\n{src}"
    );
    for line in production_lines(&src) {
        assert!(
            !line.contains("wantedBy"),
            "workers fragment must not wantedBy a boot TUI:\n{line}"
        );
        let t = line.replace(' ', "");
        assert!(
            !t.contains("Nice="),
            "workers fragment must not raise Nice above sshd class:\n{line}"
        );
    }
}
