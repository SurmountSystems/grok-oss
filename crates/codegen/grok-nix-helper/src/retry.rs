//! Retry classification and argv exec for `grok-nix-helper retry`.
//!
//! Live `just nix_retry` is the justfile recipe and must not require this
//! binary. This module stays for the helper subcommand and unit tests. Keep
//! fail-fast classes and operator sentences aligned with that recipe.
//!
//! Quality miss vs SSH miss vs flake 502. Exec command as argv; never eval
//! untrusted strings. Unclassified non-zero retries (5s, 15s, 45s).

use std::env;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::ensure_nix_path;
use crate::force_remote::{self, redact_store_banner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitClass {
    SshMiss,
    MissingSystemFeatures,
    RustfmtDiff,
    ClippyCompile,
    LockedLockfile,
    FixedOutputHashMismatch,
    NextestFailed,
    MachinesLineArgv,
    Transient,
}

/// collect2 prints this when the linker is SIGKILL'd (OOM).
fn linker_sigkill(log: &str) -> bool {
    log.contains("ld returned 137")
}

pub fn classify(status: i32, log: &str) -> ExitClass {
    if log.contains("failed to start SSH connection")
        || log.contains("Failed to find a machine for remote build")
    {
        return ExitClass::SshMiss;
    }
    if log.contains("missing system features") {
        return ExitClass::MissingSystemFeatures;
    }
    if log.contains("Diff in ") {
        return ExitClass::RustfmtDiff;
    }
    // rustc wraps a SIGKILL'd linker as "error: could not compile".
    // Exit 137 is 128+9 SIGKILL (OOM killer). That is builder memory,
    // not a rustc type error. Classify before ClippyCompile so a
    // quality log that also says "could not compile" still retries.
    if linker_sigkill(log) {
        return ExitClass::Transient;
    }
    if log.contains("error: could not compile") || log.contains("clippy::") {
        return ExitClass::ClippyCompile;
    }
    if log.contains("cannot update the lock file") || log.contains("--locked was passed") {
        return ExitClass::LockedLockfile;
    }
    if log.contains("hash mismatch in fixed-output derivation") {
        return ExitClass::FixedOutputHashMismatch;
    }
    if log.contains("error: test run failed") || log.contains("test run failed") {
        return ExitClass::NextestFailed;
    }
    if status == 127 && log.contains("ssh-ng://") && log.contains("No such file or directory") {
        return ExitClass::MachinesLineArgv;
    }
    ExitClass::Transient
}

pub fn operator_sentence(class: ExitClass) -> &'static str {
    match class {
        ExitClass::SshMiss => {
            "==> nix_retry: the builder is listed, but SSH did not start. rustc was not run locally. Not retrying this hard remote miss."
        }
        ExitClass::MissingSystemFeatures => {
            "==> nix_retry: the remote builder refused this derivation: missing system features. The client scheduled it because the machines file advertises surmount-remote. The remote nix-daemon does not list that feature in its system-features. Add surmount-remote to the builder daemon (NixOS extra-system-features / nix.conf) and restart or switch, then retry. Not retrying this hard remote miss."
        }
        ExitClass::RustfmtDiff => {
            "==> nix_retry: cargo fmt / rustfmt check failed (Diff in). That is a quality fail, not a flake 502/503. Format the listed files and retry. Not retrying this hard quality miss."
        }
        ExitClass::ClippyCompile => {
            "==> nix_retry: cargo clippy / rustc quality failed (could not compile). That is a quality fail, not a flake 502/503. Fix the listed errors and retry. Not retrying this hard quality miss."
        }
        ExitClass::LockedLockfile => {
            "==> nix_retry: cargo lockfile / --locked mismatch (cannot update the lock file). That is a quality fail, not a flake 502/503. Format/lock the listed files and retry. Not retrying this hard quality miss."
        }
        ExitClass::FixedOutputHashMismatch => {
            "==> nix_retry: nix fixed-output hash mismatch. That is a pin miss, not a flake 502/503. Update the listed sha256 and retry. Not retrying this hard quality miss."
        }
        ExitClass::NextestFailed => {
            "==> nix_retry: cargo nextest / test run failed. That is a quality fail, not a flake 502/503. Fix the listed tests and retry. Not retrying this hard quality miss."
        }
        ExitClass::MachinesLineArgv => {
            "==> nix_retry: the command was a machines-file line (exit 127). Force-remote builders belong in --option builders @file after nix. Not retrying this hard recipe miss."
        }
        ExitClass::Transient => "",
    }
}

fn parse_positive(raw: &str) -> Option<u32> {
    let n: u32 = raw.parse().ok()?;
    (n >= 1).then_some(n)
}

fn join_display(cmd: &[OsString], extra: &[String]) -> String {
    let mut parts: Vec<String> = cmd
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    parts.extend(extra.iter().cloned());
    parts.join(" ")
}

fn pump(mut r: impl Read, cap: Arc<Mutex<Vec<u8>>>) {
    let mut buf = [0u8; 8192];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                let _ = io::stdout().write_all(chunk);
                let _ = io::stdout().flush();
                if let Ok(mut c) = cap.lock() {
                    c.extend_from_slice(chunk);
                }
            }
            Err(_) => break,
        }
    }
}

fn run_once(
    program: &OsString,
    args: &[OsString],
    extra: &[String],
    path: Option<&str>,
    nix_sshopts: Option<&str>,
) -> (i32, String) {
    let mut command = Command::new(program);
    command.args(args);
    command.args(extra);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if let Some(p) = path {
        command.env("PATH", p);
    }
    if let Some(opts) = nix_sshopts {
        command.env("NIX_SSHOPTS", opts);
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            let log = format!("{}: {e}\n", program.to_string_lossy());
            let _ = io::stdout().write_all(log.as_bytes());
            let status = if e.kind() == io::ErrorKind::NotFound {
                127
            } else {
                1
            };
            return (status, log);
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let cap = Arc::new(Mutex::new(Vec::new()));
    let cap_o = Arc::clone(&cap);
    let cap_e = Arc::clone(&cap);
    let t_out = stdout.map(|r| thread::spawn(move || pump(r, cap_o)));
    let t_err = stderr.map(|r| thread::spawn(move || pump(r, cap_e)));
    let status = child.wait().ok().and_then(|s| s.code()).unwrap_or(1);
    if let Some(t) = t_out {
        let _ = t.join();
    }
    if let Some(t) = t_err {
        let _ = t.join();
    }
    let bytes = cap.lock().map(|g| g.clone()).unwrap_or_default();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

pub fn run(cmd: Vec<OsString>) -> ExitCode {
    if cmd.is_empty() {
        let _ = writeln!(io::stderr(), "grok-nix-helper retry: missing command");
        return ExitCode::from(2);
    }
    let argv0 = cmd[0].to_string_lossy();
    if argv0.starts_with("ssh-ng://") {
        let _ = writeln!(
            io::stderr(),
            "==> nix_retry: the first argument is a machines-file line, not the nix command. Pass --option builders @file after the command; do not put the machines line in \"$@\"."
        );
        return ExitCode::from(2);
    }

    let raw_attempts = env::var("NIX_RETRY_ATTEMPTS").unwrap_or_else(|_| "4".into());
    let Some(attempts) = parse_positive(&raw_attempts) else {
        let _ = writeln!(
            io::stderr(),
            "==> nix_retry: NIX_RETRY_ATTEMPTS must be a positive integer, got: {raw_attempts}"
        );
        return ExitCode::from(2);
    };

    let old_path = env::var("PATH").unwrap_or_default();
    let nix_bin = env::var_os("NIX_BIN").map(PathBuf::from);
    let new_path = match ensure_nix_path::resolve_path(nix_bin.as_deref(), &old_path) {
        Ok((p, _)) => p,
        Err(e) => {
            let msg = match e {
                ensure_nix_path::EnsureError::NixBinNotExecutable(p) => format!(
                    "grok-nix-helper ensure-nix-path: NIX_BIN is not executable: {}",
                    p.display()
                ),
                ensure_nix_path::EnsureError::NoneFound => {
                    "grok-nix-helper ensure-nix-path: no working nix found\n  Set NIX_BIN to a working binary, or repair the host nix install.".into()
                }
            };
            let _ = writeln!(io::stderr(), "{msg}");
            return ExitCode::from(2);
        }
    };

    let force = env::var("GROK_NIX_FORCE_REMOTE").as_deref() == Ok("1");
    let mut prepared = None;
    if force {
        let home = env::var("HOME").unwrap_or_default();
        let builders = env::var("GROK_NIX_BUILDERS_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(format!("{home}/.config/nix/machines")));
        let known_hosts = env::var("GROK_NIX_KNOWN_HOSTS")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(format!("{home}/.ssh/known_hosts")));
        let max_conn = env::var("GROK_NIX_SSH_NG_MAX_CONNECTIONS").unwrap_or_else(|_| "8".into());
        let existing = env::var("NIX_SSHOPTS").ok();
        let second = cmd.get(1).map(|s| s.to_string_lossy().into_owned());
        match force_remote::prepare(
            &builders,
            &known_hosts,
            &max_conn,
            existing.as_deref(),
            second.as_deref(),
        ) {
            Ok(fr) => prepared = Some(fr),
            Err(e) => {
                let _ = writeln!(io::stderr(), "{}", e.message());
                return ExitCode::from(e.exit_code());
            }
        }
    }

    let extra: &[String] = prepared.as_ref().map(|p| p.extra.as_slice()).unwrap_or(&[]);
    let nix_sshopts = prepared.as_ref().map(|p| p.nix_sshopts.as_str());
    let banner_extra = if extra.is_empty() {
        Vec::new()
    } else {
        redact_store_banner(extra)
    };

    let mut backoff: u64 = 5;
    let mut n: u32 = 1;
    loop {
        let cmd_disp = join_display(&cmd, &banner_extra);
        println!("==> nix attempt {n}/{attempts}: {cmd_disp}");
        let (status, log) = run_once(&cmd[0], &cmd[1..], extra, Some(&new_path), nix_sshopts);
        if status == 0 {
            return ExitCode::SUCCESS;
        }
        let class = classify(status, &log);
        if class != ExitClass::Transient {
            let _ = writeln!(io::stderr(), "{}", operator_sentence(class));
            return ExitCode::from(status as u8);
        }
        if n >= attempts {
            let _ = writeln!(
                io::stderr(),
                "==> nix FAILED after {n} attempt(s) (exit {status}): {}",
                join_display(&cmd, &[])
            );
            return ExitCode::from(status as u8);
        }
        let _ = writeln!(
            io::stderr(),
            "==> nix attempt {n} failed (exit {status}); retrying in {backoff}s..."
        );
        thread::sleep(Duration::from_secs(backoff));
        backoff = backoff.saturating_mul(3);
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_miss_is_not_transient() {
        assert_eq!(
            classify(19, "failed to start SSH connection"),
            ExitClass::SshMiss
        );
        assert_eq!(
            classify(19, "Failed to find a machine for remote build"),
            ExitClass::SshMiss
        );
        let msg = operator_sentence(ExitClass::SshMiss);
        assert!(msg.contains("SSH did not start"));
        assert!(msg.contains("rustc was not run locally"));
    }

    #[test]
    fn missing_system_features() {
        let log = "error: Cannot build 'workspace-cargo-quality-deps'. Reason: missing system features Required features: {big-parallel, surmount-remote}";
        assert_eq!(classify(19, log), ExitClass::MissingSystemFeatures);
        let msg = operator_sentence(ExitClass::MissingSystemFeatures);
        assert!(msg.contains("missing system features"));
        assert!(msg.contains("machines file advertises surmount-remote"));
        assert!(msg.contains("extra-system-features") || msg.contains("nix.conf"));
        assert!(!msg.contains("SSH did not start"));
    }

    #[test]
    fn rustfmt_diff_in() {
        assert_eq!(
            classify(
                19,
                "Diff in /build/source/crates/xai-grok-tui/src/session.rs:65:"
            ),
            ExitClass::RustfmtDiff
        );
        let msg = operator_sentence(ExitClass::RustfmtDiff);
        assert!(msg.contains("cargo fmt") || msg.contains("rustfmt"));
        assert!(msg.contains("Diff in"));
        assert!(msg.contains("quality fail"));
    }

    #[test]
    fn clippy_could_not_compile() {
        assert_eq!(
            classify(
                19,
                "error: could not compile `xai-grok-pager` (lib) due to 5 previous errors"
            ),
            ExitClass::ClippyCompile
        );
        let msg = operator_sentence(ExitClass::ClippyCompile);
        assert!(msg.contains("clippy") || msg.contains("rustc"));
        assert!(msg.contains("could not compile"));
    }

    #[test]
    fn linker_sigkill_is_transient_even_when_could_not_compile() {
        let log = "\
error: could not compile `xai-grok-shell` (test \"test_leader_death_repro\") due to 1 previous error
collect2: error: ld returned 137 exit status
error: command `cargo test --no-run --workspace --locked` exited with code 101
";
        assert_eq!(classify(101, log), ExitClass::Transient);
        assert_eq!(operator_sentence(ExitClass::Transient), "");
        assert!(linker_sigkill(log));
        assert!(!linker_sigkill(
            "error: could not compile `xai-grok-pager` (lib) due to 5 previous errors"
        ));
    }

    #[test]
    fn locked_lockfile() {
        assert_eq!(
            classify(
                19,
                "error: cannot update the lock file /build/source/Cargo.lock because --locked was passed to prevent this"
            ),
            ExitClass::LockedLockfile
        );
        let msg = operator_sentence(ExitClass::LockedLockfile);
        assert!(msg.contains("lockfile") || msg.contains("--locked"));
        assert!(msg.contains("cannot update the lock file"));
        assert!(msg.contains("Format/lock") || msg.contains("quality fail"));
    }

    #[test]
    fn fixed_output_hash() {
        assert_eq!(
            classify(
                19,
                "error: hash mismatch in fixed-output derivation '/nix/store/sz7d1n6cbqwc77lvmlqy6fzgpikphz5x-channel-rust-stable.toml.drv':"
            ),
            ExitClass::FixedOutputHashMismatch
        );
        let msg = operator_sentence(ExitClass::FixedOutputHashMismatch);
        assert!(msg.contains("fixed-output hash mismatch") || msg.contains("pin miss"));
        assert!(msg.contains("sha256"));
    }

    #[test]
    fn nextest_test_run_failed() {
        assert_eq!(
            classify(19, "error: test run failed"),
            ExitClass::NextestFailed
        );
        let msg = operator_sentence(ExitClass::NextestFailed);
        assert!(msg.contains("nextest"));
        assert!(msg.contains("test run failed"));
        assert!(msg.contains("Fix the listed tests") || msg.contains("quality fail"));
    }

    #[test]
    fn unclassified_is_transient_flake_502() {
        assert_eq!(
            classify(1, "error: unable to download ... HTTP 502"),
            ExitClass::Transient
        );
        assert_eq!(operator_sentence(ExitClass::Transient), "");
    }

    #[test]
    fn machines_line_127() {
        assert_eq!(
            classify(
                127,
                "ssh-ng://probe@example.invalid: No such file or directory"
            ),
            ExitClass::MachinesLineArgv
        );
        assert_eq!(
            classify(127, "false: No such file or directory"),
            ExitClass::Transient
        );
    }

    #[test]
    fn argv_is_words_not_eval() {
        // Classification never treats the command line as a shell string.
        let cmd = vec![OsString::from("false")];
        assert_eq!(cmd.len(), 1);
        assert_eq!(join_display(&cmd, &[]), "false");
    }
}
