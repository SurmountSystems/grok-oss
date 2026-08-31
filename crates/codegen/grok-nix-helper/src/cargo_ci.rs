//! Env hygiene and optional hermetic PATH for `just cargo-ci`.
//!
//! Exec remaining words as argv. Never eval.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use crate::ensure_nix_path;
use crate::git_cmd::env_flag;

/// One argv word so `#` cannot become a bash comment (`nix develop .#ci`).
pub const CI_DEV_SHELL: &str = ".#ci";

pub fn apply_test_env() {
    unsafe {
        env::set_var(
            "RULES_RUST_RUNFILES_WORKSPACE_NAME",
            env::var("RULES_RUST_RUNFILES_WORKSPACE_NAME").unwrap_or_else(|_| "grok-oss".into()),
        );
        env::remove_var("NO_COLOR");
        env::remove_var("CARGO_TERM_COLOR");
        env::remove_var("OPENROUTER_API_KEY");
        env::set_var(
            "GROK_DISABLE_SHARED_HARNESS_SECRETS",
            env::var("GROK_DISABLE_SHARED_HARNESS_SECRETS").unwrap_or_else(|_| "1".into()),
        );
        env::set_var(
            "GROK_CREDENTIALS_FORCE_FILE",
            env::var("GROK_CREDENTIALS_FORCE_FILE").unwrap_or_else(|_| "1".into()),
        );
        env::set_var(
            "GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY",
            env::var("GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY").unwrap_or_else(|_| "1".into()),
        );
    }
}

fn prepend_working_nix() -> Result<String, String> {
    let nix_bin = env::var_os("NIX_BIN").map(PathBuf::from);
    let path = env::var("PATH").unwrap_or_default();
    match ensure_nix_path::resolve_path(nix_bin.as_deref(), &path) {
        Ok((new_path, _)) => {
            unsafe {
                env::set_var("PATH", &new_path);
            }
            Ok(new_path)
        }
        Err(ensure_nix_path::EnsureError::NixBinNotExecutable(p)) => Err(format!(
            "grok-nix-helper cargo-ci: NIX_BIN is not executable: {}",
            p.display()
        )),
        Err(ensure_nix_path::EnsureError::NoneFound) => {
            Err("grok-nix-helper cargo-ci: no working nix found".into())
        }
    }
}

pub fn run(cmd: Vec<OsString>) -> ExitCode {
    if cmd.is_empty() {
        let _ = writeln!(io::stderr(), "grok-nix-helper cargo-ci: missing command");
        return ExitCode::from(2);
    }
    if let Err(e) = prepend_working_nix() {
        let _ = writeln!(io::stderr(), "grok-nix-helper cargo-ci: {e}");
        return ExitCode::from(2);
    }
    apply_test_env();

    if env::var("CI_LOW_MEM").as_deref() == Ok("1") {
        return exec_low_mem(cmd);
    }
    exec_argv(&cmd)
}

fn exec_low_mem(cmd: Vec<OsString>) -> ExitCode {
    let helper = env::current_exe().unwrap_or_else(|_| PathBuf::from("grok-nix-helper"));
    let mut nix = Command::new("nix");
    if env_flag("CI_LOW_MEM") {
        nix.args(["--option", "cores", "2", "--option", "max-jobs", "1"]);
    }
    if env_flag("GROK_NIX_FORCE_REMOTE") {
        let home = env::var("HOME").unwrap_or_default();
        let builders = env::var("GROK_NIX_BUILDERS_FILE")
            .unwrap_or_else(|_| format!("{home}/.config/nix/machines"));
        nix.args([
            "--option",
            "builders",
            &format!("@{builders}"),
            "--option",
            "builders-use-substitutes",
            "true",
            "--option",
            "fallback",
            "false",
            "--option",
            "system-features",
            "kvm nixos-test uid-range",
        ]);
    }
    nix.args(["develop", CI_DEV_SHELL, "-c"]);
    nix.arg(&helper);
    nix.args(["hermetic-path", "--", "cargo-mem-guard", "--"]);
    nix.args(&cmd);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = nix.exec();
        let _ = writeln!(
            io::stderr(),
            "grok-nix-helper cargo-ci: exec nix develop: {err}"
        );
        ExitCode::from(1)
    }
    #[cfg(not(unix))]
    {
        match nix.status() {
            Ok(st) => ExitCode::from(st.code().unwrap_or(1) as u8),
            Err(e) => {
                let _ = writeln!(io::stderr(), "grok-nix-helper cargo-ci: {e}");
                ExitCode::from(1)
            }
        }
    }
}

fn exec_argv(cmd: &[OsString]) -> ExitCode {
    let mut command = Command::new(&cmd[0]);
    if cmd.len() > 1 {
        command.args(&cmd[1..]);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        let _ = writeln!(io::stderr(), "grok-nix-helper cargo-ci: exec: {err}");
        ExitCode::from(1)
    }
    #[cfg(not(unix))]
    {
        match command.status() {
            Ok(st) => ExitCode::from(st.code().unwrap_or(1) as u8),
            Err(e) => {
                let _ = writeln!(io::stderr(), "grok-nix-helper cargo-ci: {e}");
                ExitCode::from(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_mem_uses_quoted_ci_shell_attr() {
        assert!(CI_DEV_SHELL.contains('#'));
        assert_eq!(CI_DEV_SHELL, ".#ci");
    }
}
