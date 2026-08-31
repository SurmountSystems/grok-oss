//! Print a Nix system triple without calling `nix`.
//!
//! Prefer `CI_SYSTEM` when set. Otherwise map kernel + arch. Avoids parse-time
//! `nix eval` so a broken host nix cannot fail every just recipe.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

pub fn run(require: bool) -> ExitCode {
    match current_system(
        env::var("CI_SYSTEM").ok().as_deref(),
        env::consts::OS,
        env::consts::ARCH,
    ) {
        Ok(sys) => {
            if require && !system_safe_for_interpolation(&sys) {
                let _ = writeln!(
                    io::stderr(),
                    "==> invalid CI_SYSTEM / system (refuse shell interpolation): {sys}"
                );
                let _ = writeln!(
                    io::stderr(),
                    "    expected a Nix system like x86_64-linux (cpu-os), not a just recipe name"
                );
                return ExitCode::from(2);
            }
            let _ = writeln!(io::stdout(), "{sys}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            let _ = writeln!(io::stderr(), "{msg}");
            ExitCode::from(1)
        }
    }
}

/// Map CI_SYSTEM or kernel/arch into a Nix system string.
pub fn current_system(ci_system: Option<&str>, os: &str, arch: &str) -> Result<String, String> {
    if let Some(sys) = ci_system.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(sys.to_string());
    }

    let kernel = os.to_ascii_lowercase();
    let arch = arch.to_ascii_lowercase();
    match (kernel.as_str(), arch.as_str()) {
        ("linux", "x86_64") => Ok("x86_64-linux".into()),
        ("linux", "aarch64" | "arm64") => Ok("aarch64-linux".into()),
        ("macos" | "darwin", "x86_64") => Ok("x86_64-darwin".into()),
        ("macos" | "darwin", "aarch64" | "arm64") => Ok("aarch64-darwin".into()),
        ("linux", other) => Err(format!(
            "grok-nix-helper current-system: unsupported Linux arch: {other}\n  set CI_SYSTEM=... (e.g. x86_64-linux)"
        )),
        ("macos" | "darwin", other) => Err(format!(
            "grok-nix-helper current-system: unsupported Darwin arch: {other}\n  set CI_SYSTEM=... (e.g. aarch64-darwin)"
        )),
        (kernel, _) => Err(format!(
            "grok-nix-helper current-system: unsupported kernel: {kernel}\n  set CI_SYSTEM=... (e.g. x86_64-linux)"
        )),
    }
}

/// True when `sys` is safe to interpolate into a nix attr / shell word.
///
/// Known Nix systems always pass. Other `cpu-os` words must be two
/// `[A-Za-z0-9_]+` tokens separated by one hyphen, with a digit or
/// underscore somewhere, so a just recipe name like `just-one` is not
/// treated as a system. Named contract:
/// `interpolation_allows_known_triples`.
pub fn system_safe_for_interpolation(sys: &str) -> bool {
    matches!(
        sys,
        "x86_64-linux" | "aarch64-linux" | "x86_64-darwin" | "aarch64-darwin"
    ) || {
        let mut parts = sys.split('-');
        let a = parts.next().unwrap_or("");
        let b = parts.next().unwrap_or("");
        parts.next().is_none()
            && !a.is_empty()
            && !b.is_empty()
            && a.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && b.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && sys.chars().any(|c| c.is_ascii_digit() || c == '_')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_system_wins() {
        assert_eq!(
            current_system(Some("x86_64-linux"), "linux", "aarch64").unwrap(),
            "x86_64-linux"
        );
    }

    #[test]
    fn linux_x86_64() {
        assert_eq!(
            current_system(None, "linux", "x86_64").unwrap(),
            "x86_64-linux"
        );
    }

    #[test]
    fn linux_arm64_alias() {
        assert_eq!(
            current_system(None, "Linux", "arm64").unwrap(),
            "aarch64-linux"
        );
    }

    #[test]
    fn darwin_aarch64() {
        assert_eq!(
            current_system(None, "Darwin", "arm64").unwrap(),
            "aarch64-darwin"
        );
    }

    #[test]
    fn unsupported_arch_names_ci_system() {
        let err = current_system(None, "linux", "riscv64").unwrap_err();
        assert!(err.contains("unsupported Linux arch"));
        assert!(err.contains("CI_SYSTEM"));
    }

    #[test]
    fn interpolation_allows_known_triples() {
        assert!(system_safe_for_interpolation("x86_64-linux"));
        assert!(system_safe_for_interpolation("aarch64-darwin"));
        assert!(system_safe_for_interpolation("wasm32_wasi-foo_bar"));
        assert!(!system_safe_for_interpolation("x86_64-linux;rm"));
        assert!(!system_safe_for_interpolation(""));
        assert!(!system_safe_for_interpolation("just-one"));
    }

    #[test]
    fn require_rejects_metacharacters() {
        assert!(!system_safe_for_interpolation("x86_64-linux;true"));
        assert!(!system_safe_for_interpolation("$(uname)"));
    }
}
