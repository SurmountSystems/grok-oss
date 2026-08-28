//! Nix CI helpers and git recon: retry, hermetic PATH, working nix, current
//! system, process-pin assert, put-history, import, join, recon-status.
//!
//! Exec remaining words as argv. Never eval untrusted strings. Logs are
//! command names and exit classes only (no tokens, no NIX_SSHOPTS, no
//! GROK_HOME dumps). SHA-1 is git object ids only.

mod assert_process_pins;
mod cargo_ci;
mod current_system;
mod detect_upstream;
mod ensure_nix_path;
mod extract_debug;
mod force_remote;
mod fork_paths;
mod generate_announcements;
mod git_cmd;
mod hermetic_path;
mod import_upstream;
mod join_main;
mod put_history;
mod recon_status;
mod remote_named_cargo;
mod require_remote_builder;
mod retry;
mod sync_upstream;

#[cfg(test)]
mod justfile_contracts;

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

fn usage_text() -> &'static str {
    "\
grok-nix-helper -- Nix CI helpers and git recon (argv exec, never eval)

Usage:
  grok-nix-helper retry [--] <cmd>...
  grok-nix-helper hermetic-path [--] <cmd>...
  grok-nix-helper ensure-nix-path [--export]
  grok-nix-helper current-system [--require]
  grok-nix-helper assert-process-pins [--root PATH] [--strict] [TREE_ISH]
  grok-nix-helper require-remote-builder
  grok-nix-helper cargo-ci [--] <cmd>...
  grok-nix-helper remote-named-cargo <kind> <filter>...
  grok-nix-helper recon-status
  grok-nix-helper detect-upstream-export
  grok-nix-helper import-upstream-export [--stay] [XAI_TIP]
  grok-nix-helper put-history-on-xai [XAI_TIP]
  grok-nix-helper replay-onto-upstream [XAI_TIP]
  grok-nix-helper join-main-into-onto
  grok-nix-helper sync-upstream
  grok-nix-helper extract-debug-sidecar <binary>
  grok-nix-helper generate-announcements [--crate-dir PATH] [--dest PATH]

retry classifies quality miss vs SSH miss vs flake 502 and execs argv.
GROK_NIX_FORCE_REMOTE=1 appends --option max-jobs 0, --cores 64,
--store ssh-ng, --eval-store auto. Banner redacts --store as <builder>.
Recon helpers prepare git state. Humans sign: git commit -S.
"
}

fn usage() {
    let _ = writeln!(io::stderr(), "{}", usage_text());
}

fn strip_leading_dd(args: &[OsString]) -> Vec<OsString> {
    if args.first().map(|s| s.as_os_str()) == Some(std::ffi::OsStr::new("--")) {
        args[1..].to_vec()
    } else {
        args.to_vec()
    }
}

fn to_strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect()
}

fn main() -> ExitCode {
    let mut args: Vec<OsString> = env::args_os().skip(1).collect();
    if args.is_empty() {
        usage();
        return ExitCode::from(2);
    }
    let first = args[0].to_string_lossy().into_owned();
    match first.as_str() {
        "-h" | "--help" | "help" => {
            usage();
            ExitCode::from(2)
        }
        "retry" => {
            args.remove(0);
            retry::run(strip_leading_dd(&args))
        }
        "hermetic-path" => {
            args.remove(0);
            hermetic_path::run(&strip_leading_dd(&args))
        }
        "ensure-nix-path" => {
            args.remove(0);
            ensure_nix_path::run(&to_strings(&args))
        }
        "current-system" => {
            let require = args.iter().any(|a| a == "--require");
            current_system::run(require)
        }
        "assert-process-pins" => {
            args.remove(0);
            assert_process_pins::run(&to_strings(&args))
        }
        "require-remote-builder" => require_remote_builder::run(&[]),
        "cargo-ci" => {
            args.remove(0);
            cargo_ci::run(strip_leading_dd(&args))
        }
        "remote-named-cargo" => {
            args.remove(0);
            remote_named_cargo::run(strip_leading_dd(&args))
        }
        "recon-status" => recon_status::run(&[]),
        "detect-upstream-export" => detect_upstream::run(&[]),
        "import-upstream-export" => {
            args.remove(0);
            import_upstream::run(&to_strings(&args))
        }
        "put-history-on-xai" => {
            args.remove(0);
            put_history::run(&to_strings(&args))
        }
        "replay-onto-upstream" => {
            let _ = writeln!(
                io::stderr(),
                "note: replay-onto-upstream → put-history-on-xai"
            );
            args.remove(0);
            put_history::run(&to_strings(&args))
        }
        "join-main-into-onto" => join_main::run(&[]),
        "sync-upstream" => {
            args.remove(0);
            sync_upstream::run(&to_strings(&args))
        }
        "extract-debug-sidecar" => {
            args.remove(0);
            extract_debug::run(&to_strings(&args))
        }
        "generate-announcements" => {
            args.remove(0);
            generate_announcements::run(&to_strings(&args))
        }
        other => {
            let _ = writeln!(io::stderr(), "grok-nix-helper: unknown subcommand {other}");
            usage();
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_double_dash() {
        let args = vec![OsString::from("--"), OsString::from("false")];
        assert_eq!(strip_leading_dd(&args), vec![OsString::from("false")]);
    }

    #[test]
    fn usage_names_recon_helpers_not_shell_scripts() {
        let text = usage_text();
        assert!(text.contains("put-history-on-xai"));
        assert!(text.contains("import-upstream-export"));
        assert!(text.contains("join-main-into-onto"));
        assert!(!text.contains(".sh"));
        assert!(!text.contains("scripts/"));
    }
}
