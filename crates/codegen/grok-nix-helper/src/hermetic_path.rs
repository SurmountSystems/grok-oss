//! Rebuild PATH as a nix-store-only allowlist, then exec remaining argv.
//!
//! Used by `just cargo-ci` under CI_LOW_MEM=1 after `nix develop .#ci` so
//! quality cargo/nextest children do not resolve optional host tools from
//! ambient desktop PATH. Escape: `GROK_CI_ALLOW_HOST_PATH=1`.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::{Command, ExitCode};

/// Keep only PATH entries under `/nix/store/`.
pub fn store_only_path(path: &str) -> Option<String> {
    let kept: Vec<&str> = path
        .split(':')
        .filter(|d| d.starts_with("/nix/store/"))
        .collect();
    if kept.is_empty() {
        None
    } else {
        Some(kept.join(":"))
    }
}

pub fn run(cmd: &[OsString]) -> ExitCode {
    if cmd.is_empty() {
        let _ = writeln!(
            io::stderr(),
            "grok-nix-helper hermetic-path: missing command"
        );
        return ExitCode::from(2);
    }

    if env::var("GROK_CI_ALLOW_HOST_PATH").as_deref() == Ok("1") {
        return exec_argv(cmd, None);
    }

    let old = env::var("PATH").unwrap_or_default();
    let Some(hermetic) = store_only_path(&old) else {
        let _ = writeln!(
            io::stderr(),
            "grok-nix-helper hermetic-path: PATH has no /nix/store entries after scrub"
        );
        let _ = writeln!(io::stderr(), "  (run under: nix develop .#ci -c ...)");
        return ExitCode::from(2);
    };
    exec_argv(cmd, Some(&hermetic))
}

fn exec_argv(cmd: &[OsString], path: Option<&str>) -> ExitCode {
    let mut command = Command::new(&cmd[0]);
    if cmd.len() > 1 {
        command.args(&cmd[1..]);
    }
    if let Some(p) = path {
        command.env("PATH", p);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        let _ = writeln!(io::stderr(), "grok-nix-helper hermetic-path: exec: {err}");
        ExitCode::from(1)
    }
    #[cfg(not(unix))]
    {
        match command.status() {
            Ok(st) => ExitCode::from(st.code().unwrap_or(1) as u8),
            Err(e) => {
                let _ = writeln!(io::stderr(), "grok-nix-helper hermetic-path: spawn: {e}");
                ExitCode::from(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_host_dirs_keeps_store() {
        let path = "/usr/bin:/nix/store/abc-git/bin:/home/hunter/bin:/nix/store/def-rustc/bin";
        assert_eq!(
            store_only_path(path).as_deref(),
            Some("/nix/store/abc-git/bin:/nix/store/def-rustc/bin")
        );
    }

    #[test]
    fn empty_when_no_store() {
        assert_eq!(store_only_path("/usr/bin:/bin"), None);
        assert_eq!(store_only_path(""), None);
    }

    #[test]
    fn store_prefix_requires_slash_after_store() {
        // `/nix/store` without a child path is not a bin dir we keep.
        assert_eq!(store_only_path("/nix/store:/usr/bin"), None);
        assert_eq!(
            store_only_path("/nix/store/zz-coreutils/bin").as_deref(),
            Some("/nix/store/zz-coreutils/bin")
        );
    }
}
