//! Fail loud before `just check-remote` starts Nix or cargo.
//!
//! Reuses the trusted-user builders file. Does not bake a host address.
//! Does not fall back to local Nix store builds. Never prints tokens,
//! NIX_SSHOPTS values, or machines-file URIs.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::force_remote::ssh_ng_host;

pub fn builders_file() -> PathBuf {
    env::var("GROK_NIX_BUILDERS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_default();
            PathBuf::from(format!("{home}/.config/nix/machines"))
        })
}

pub fn known_hosts_path() -> PathBuf {
    env::var("GROK_NIX_KNOWN_HOSTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_default();
            PathBuf::from(format!("{home}/.ssh/known_hosts"))
        })
}

pub fn extra_sshopts(existing: Option<&str>, known_hosts: &Path) -> String {
    let extra = format!(
        "-o UserKnownHostsFile={} -o StrictHostKeyChecking=yes",
        known_hosts.display()
    );
    match existing.filter(|s| !s.is_empty()) {
        Some(cur) => format!("{cur} {extra}"),
        None => extra,
    }
}

pub fn has_ssh_ng_line(body: &str) -> bool {
    body.lines().any(|l| l.starts_with("ssh-ng://"))
}

pub fn ssh_ng_hosts(body: &str) -> Vec<String> {
    body.lines()
        .filter(|l| l.starts_with("ssh-ng://"))
        .filter_map(|l| l.split_whitespace().next())
        .map(ssh_ng_host)
        .filter(|h| !h.is_empty())
        .collect()
}

pub fn host_key_present(host: &str, known_hosts: &Path) -> bool {
    if !known_hosts.is_file() {
        return false;
    }
    let out = Command::new("ssh-keygen")
        .args(["-F", host, "-f"])
        .arg(known_hosts)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines().any(|line| {
                if line.starts_with('#') {
                    return false;
                }
                let mut cols = line.split_whitespace();
                let _name = cols.next();
                cols.next().is_some_and(|t| t.starts_with("ssh-"))
            })
        }
        Err(_) => false,
    }
}

pub fn lists_surmount_remote(feats: &str) -> bool {
    regex_is_word(feats, "surmount-remote")
}

fn regex_is_word(hay: &str, needle: &str) -> bool {
    let bytes = hay.as_bytes();
    let n = needle.as_bytes();
    let mut i = 0;
    while i + n.len() <= bytes.len() {
        if &bytes[i..i + n.len()] == n {
            let before_ok = i == 0 || matches!(bytes[i - 1], b' ' | b'\t' | b',' | b'{');
            let after = i + n.len();
            let after_ok =
                after == bytes.len() || matches!(bytes[after], b' ' | b'\t' | b',' | b'}');
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn parse_system_features_line(nix_config: &str) -> Option<String> {
    for line in nix_config.lines() {
        if let Some(rest) = line.strip_prefix("system-features") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=').unwrap_or(rest).trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

#[derive(Debug)]
pub enum RequireError {
    MissingFile(PathBuf),
    NoSshNg(PathBuf),
    NoHostKey,
    SshBatchModeFailed,
    FeaturesQueryFailed,
    NoFeaturesLine,
    MissingSurmountRemote,
}

impl RequireError {
    pub fn message(&self) -> String {
        match self {
            RequireError::MissingFile(file) => format!(
                "The Nix builders file is missing or empty: {}.\njust check-remote reuses the trusted-user machines file already named in the user Nix config (override with GROK_NIX_BUILDERS_FILE).\nDefault just check stays local and does not need this file.",
                file.display()
            ),
            RequireError::NoSshNg(file) => format!(
                "The Nix builders file {} has no ssh-ng:// builder line.\njust check-remote will not fall back to local Nix store builds.",
                file.display()
            ),
            RequireError::NoHostKey => {
                "This account's known_hosts has no host key for the machines-file builder.\nUser ssh to Host surmount-1 is not the nix build SSH path (nix-daemon opens ssh-ng).\njust check-remote sets NIX_SSHOPTS to this account's known_hosts and will not fall back to a local rustc.".into()
            }
            RequireError::SshBatchModeFailed => {
                "SSH BatchMode to Host surmount-1 failed.\njust check-remote requires that existing remote builder and will not fall back to local Nix store builds.".into()
            }
            RequireError::FeaturesQueryFailed => {
                "Could not read the remote builder nix-daemon system-features over SSH BatchMode.\njust check-remote will not start the long quality build until that query works.".into()
            }
            RequireError::NoFeaturesLine => {
                "The remote builder SSH reply had no system-features line.\njust check-remote will not start the long quality build until the remote nix-daemon reports its feature list.".into()
            }
            RequireError::MissingSurmountRemote => {
                "The remote nix-daemon does not list surmount-remote in its system-features.\nThe client machines file advertises that feature, so Nix will schedule rustc on the remote, then the daemon will refuse: missing system features.\nAdd surmount-remote to the builder daemon (NixOS extra-system-features / nix.conf) and restart or switch. just check-remote will not start the long quality build until that feature is present.".into()
            }
        }
    }

    pub fn exit_code(&self) -> u8 {
        2
    }
}

pub fn check(
    builders: &Path,
    known_hosts: &Path,
    inject_feats: Option<&str>,
    skip_live_ssh: bool,
) -> Result<String, RequireError> {
    let body = fs::read_to_string(builders).unwrap_or_default();
    if !builders.is_file() || body.trim().is_empty() {
        return Err(RequireError::MissingFile(builders.to_path_buf()));
    }
    if !has_ssh_ng_line(&body) {
        return Err(RequireError::NoSshNg(builders.to_path_buf()));
    }
    for host in ssh_ng_hosts(&body) {
        if !host_key_present(&host, known_hosts) {
            return Err(RequireError::NoHostKey);
        }
    }

    let remote_feats = if let Some(f) = inject_feats.filter(|s| !s.is_empty()) {
        f.to_string()
    } else if skip_live_ssh {
        return Err(RequireError::FeaturesQueryFailed);
    } else {
        let ssh_ok = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=8",
                "-o",
                "StrictHostKeyChecking=yes",
                "surmount-1",
                "true",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ssh_ok {
            return Err(RequireError::SshBatchModeFailed);
        }
        let out = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=8",
                "-o",
                "StrictHostKeyChecking=yes",
                "surmount-1",
                "nix config show",
            ])
            .stderr(Stdio::null())
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout);
                parse_system_features_line(&text).ok_or(RequireError::NoFeaturesLine)?
            }
            _ => return Err(RequireError::FeaturesQueryFailed),
        }
    };

    if !lists_surmount_remote(&remote_feats) {
        return Err(RequireError::MissingSurmountRemote);
    }
    Ok(remote_feats)
}

fn apply_sshopts(known_hosts: &Path) {
    let existing = env::var("NIX_SSHOPTS").ok();
    let opts = extra_sshopts(existing.as_deref(), known_hosts);
    // SAFETY: single-threaded helper; NIX_SSHOPTS is process env for this exec.
    unsafe {
        env::set_var("NIX_SSHOPTS", opts);
    }
}

pub fn run(_args: &[String]) -> ExitCode {
    let file = builders_file();
    let known_hosts = known_hosts_path();
    apply_sshopts(&known_hosts);
    let inject = env::var("GROK_NIX_REMOTE_SYSTEM_FEATURES").ok();
    match check(&file, &known_hosts, inject.as_deref(), false) {
        Ok(_) => {
            println!(
                "==> just check-remote: using builders file {}",
                file.display()
            );
            println!(
                "==> just check-remote: NIX_SSHOPTS uses this account's known_hosts (host-key checks stay on)"
            );
            println!(
                "==> just check-remote: rustc, clippy, and nextest require the remote builder surmount-remote feature (fallback=false). This laptop does not advertise that feature, so local nixbld cannot take the rustc job."
            );
            println!(
                "==> just check-remote: force-remote nix sets max-jobs 0. This laptop must not build. Fixed-output derivations and toolchain downloads (crates.io, static.rust-lang.org) go to the remote builder. This laptop must not curl those hosts for this gate. The VPS fetches them from the web (builders-use-substitutes)."
            );
            println!(
                "==> just check-remote: force-remote nix uses --store ssh-ng (same machines-file builder) and --eval-store auto. Cargo-package NARs stay on the VPS. This laptop does not download those NARs from the builder, and does not substitute them from cache.nixos.org. -L logs still stream. nix build --no-link skips a local result symlink."
            );
            println!(
                "==> just check-remote: force-remote nix uses --cores 64. Host machines max-jobs should advertise that many jobs on the builder."
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "{}", e.message());
            ExitCode::from(e.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_builders_file_fails_loud() {
        let missing = PathBuf::from("/tmp/does-not-exist-builders-grok-helper-test");
        let hosts = PathBuf::from("/tmp/does-not-exist-known-hosts-grok-helper-test");
        let err = check(&missing, &hosts, Some("surmount-remote"), true).unwrap_err();
        let msg = err.message();
        assert!(msg.contains("missing or empty"), "{msg}");
        assert!(!msg.contains("ssh-ng://"));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn no_ssh_ng_line_fails() {
        let dir = env::temp_dir().join(format!("grok-req-remote-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let machines = dir.join("machines");
        let hosts = dir.join("known_hosts");
        fs::write(&machines, "not-a-builder\n").unwrap();
        fs::write(&hosts, "").unwrap();
        let err = check(&machines, &hosts, Some("surmount-remote"), true).unwrap_err();
        assert!(err.message().contains("ssh-ng://"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_known_hosts_fails_without_printing_uri() {
        let dir = env::temp_dir().join(format!("grok-req-remote-kh-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let machines = dir.join("machines");
        let hosts = dir.join("known_hosts");
        fs::write(
            &machines,
            "ssh-ng://probe@example.invalid x86_64-linux - 1 1 surmount-remote\n",
        )
        .unwrap();
        fs::write(&hosts, "").unwrap();
        let err = check(&machines, &hosts, Some("surmount-remote"), true).unwrap_err();
        let msg = err.message();
        assert!(
            msg.contains("known_hosts") || msg.contains("host key"),
            "{msg}"
        );
        assert!(
            msg.contains("surmount-1") || msg.contains("user ssh") || msg.contains("User ssh"),
            "{msg}"
        );
        assert!(!msg.contains("ssh-ng://"));
        assert!(!msg.contains("example.invalid"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inject_without_surmount_remote_fails() {
        assert!(!lists_surmount_remote(
            "benchmark big-parallel kvm nixos-test"
        ));
        assert!(lists_surmount_remote(
            "benchmark big-parallel kvm nixos-test surmount-remote"
        ));
        assert!(lists_surmount_remote("{big-parallel,surmount-remote}"));
    }

    #[test]
    fn extra_sshopts_keeps_host_key_checks_on() {
        let p = Path::new("/home/hunter/.ssh/known_hosts");
        let s = extra_sshopts(None, p);
        assert!(s.contains("UserKnownHostsFile"));
        assert!(s.contains("StrictHostKeyChecking=yes"));
        assert!(!s.contains("StrictHostKeyChecking=no"));
    }

    #[test]
    fn parse_features_line() {
        let cfg = "cores = 4\nsystem-features = kvm nixos-test surmount-remote\n";
        assert_eq!(
            parse_system_features_line(cfg).as_deref(),
            Some("kvm nixos-test surmount-remote")
        );
    }
}
