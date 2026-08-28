//! Named cargo on the remote builder (`just test-remote` / `just cargo-remote`).
//!
//! Validates kind and filter before SSH. Encodes filters as base64
//! NUL-separated env for flake builtins.getEnv. Then force-remote nix build.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use crate::force_remote::base64_encode;
use crate::require_remote_builder;
use crate::retry;

pub const KINDS: &[&str] = &["test", "nextest", "clippy", "build", "check"];

pub fn parse_kind_and_filter(args: &[OsString]) -> Result<(String, Vec<OsString>), String> {
    if args.is_empty() {
        return Err(
            "just cargo-remote needs a kind (test, nextest, clippy, build, or check) and a filter.\n\
Example: just test-remote -p xai-grok-pager --lib -- actions::defaults\n\
That runs cargo test on the remote builder. It does not run rustc on this laptop.\n\
Full gate: just check-remote."
                .into(),
        );
    }
    let kind = args[0].to_string_lossy().into_owned();
    if !KINDS.contains(&kind.as_str()) {
        return Err(format!(
            "just cargo-remote kind must be test, nextest, clippy, build, or check, got: {kind}"
        ));
    }
    let rest: Vec<OsString> = args[1..].to_vec();
    if rest.is_empty() {
        return Err(format!(
            "just cargo-remote {kind} needs a filter (for example -p xai-grok-pager --lib -- actions::defaults).\n\
Refusing to run the whole workspace on the builder from an empty filter. Full gate: just check-remote."
        ));
    }
    if kind == "test" || kind == "nextest" {
        for a in &rest {
            if a == "--no-run" {
                return Err(format!(
                    "just test-remote / just cargo-remote {kind} runs the tests on the remote builder. Do not pass --no-run (that is compile-only)."
                ));
            }
        }
    }
    Ok((kind, rest))
}

pub fn encode_filter_args(args: &[OsString]) -> String {
    let mut bytes = Vec::new();
    for a in args {
        bytes.extend(a.to_string_lossy().as_bytes());
        bytes.push(0);
    }
    base64_encode(&bytes)
}

pub fn run(args: Vec<OsString>) -> ExitCode {
    let (kind, rest) = match parse_kind_and_filter(&args) {
        Ok(v) => v,
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            return ExitCode::from(2);
        }
    };
    let pre = require_remote_builder::run(&[]);
    if pre != ExitCode::SUCCESS {
        return pre;
    }
    unsafe {
        env::set_var("GROK_NIX_FORCE_REMOTE", "1");
        env::set_var("GROK_REMOTE_CARGO_KIND", &kind);
        env::set_var("GROK_REMOTE_TEST_ARGS", encode_filter_args(&rest));
    }
    println!(
        "==> just cargo-remote {kind}: named cargo {kind} as a remote Nix derivation (nix build --impure \".#workspace-cargo-named-test\")"
    );
    println!("==> rustc requires surmount-remote. This laptop does not run that rustc.");
    retry::run(vec![
        OsString::from("nix"),
        OsString::from("build"),
        OsString::from("--impure"),
        OsString::from("-L"),
        OsString::from(".#workspace-cargo-named-test"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_args_need_kind_and_filter() {
        let err = parse_kind_and_filter(&[]).unwrap_err();
        assert!(err.contains("filter") || err.contains("kind"));
    }

    #[test]
    fn bad_kind_rejected() {
        let err = parse_kind_and_filter(&[OsString::from("not-a-kind"), OsString::from("-p")])
            .unwrap_err();
        assert!(err.contains("not-a-kind"));
    }

    #[test]
    fn test_kind_needs_filter() {
        let err = parse_kind_and_filter(&[OsString::from("test")]).unwrap_err();
        assert!(err.contains("filter"));
    }

    #[test]
    fn no_run_rejected_for_test() {
        let err = parse_kind_and_filter(&[
            OsString::from("test"),
            OsString::from("-p"),
            OsString::from("foo"),
            OsString::from("--no-run"),
        ])
        .unwrap_err();
        assert!(err.contains("--no-run"));
    }

    #[test]
    fn quoted_named_test_attr_is_one_argv_word() {
        let word = ".#workspace-cargo-named-test";
        assert!(word.starts_with("."));
        assert!(word.contains('#'));
        assert_eq!(word, ".#workspace-cargo-named-test");
    }

    #[test]
    fn filter_encode_is_nul_separated_base64() {
        let enc = encode_filter_args(&[OsString::from("-p"), OsString::from("xai-grok-pager")]);
        assert!(!enc.is_empty());
        assert!(
            enc.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        );
    }
}
