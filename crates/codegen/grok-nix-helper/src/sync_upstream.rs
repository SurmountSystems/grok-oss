//! Detect a new xAI export and point at put-history or import.

use std::io::{self, Write};
use std::process::ExitCode;

use crate::detect_upstream;
use crate::git_cmd::env_flag;
use crate::{import_upstream, put_history};

pub fn run(args: &[String]) -> ExitCode {
    println!("=== Grok OSS upstream sync (export-aware) ===");
    println!("Surmount main = canonical product archive.");
    println!("xai-org/main  = disposable export tip (force-pushed).");
    println!();
    println!("Directions:");
    println!("  grok-nix-helper put-history-on-xai     # our history ON their tip (onto-xai/*)");
    println!("  grok-nix-helper import-upstream-export # their tree INTO Surmount (import/*)");
    println!();

    // ExitCode is not Into<u8> on rustc 1.98 (E0277). detect-upstream-export
    // only returns success, 2 (new export), or failure.
    let code = detect_upstream::run(&[]);
    if code == ExitCode::SUCCESS {
        println!();
        println!("No new export content vs last import log.");
        if env_flag("PUT_ON_XAI") || env_flag("REPLAY_ONTO") {
            return put_history::run(args);
        }
        if env_flag("IMPORT_NOW") {
            return import_upstream::run(args);
        }
        println!("Still useful anytime (rebuild stack on current tip; real cherry-pick):");
        println!("  grok-nix-helper put-history-on-xai");
        println!("  FORCE=1 SURMOUNT_REF=origin/main grok-nix-helper put-history-on-xai");
        ExitCode::SUCCESS
    } else if code == ExitCode::from(2) {
        println!();
        println!("New export available.");
        if env_flag("PUT_ON_XAI") || env_flag("REPLAY_ONTO") {
            return put_history::run(args);
        }
        if env_flag("IMPORT_NOW") {
            return import_upstream::run(args);
        }
        println!(
            "1) Stack Surmount product commits on their tip (preferred when histories break):"
        );
        println!("  grok-nix-helper put-history-on-xai");
        println!("  FORCE=1 SURMOUNT_REF=origin/main grok-nix-helper put-history-on-xai");
        println!("  PUT_ON_XAI=1 grok-nix-helper sync-upstream");
        println!();
        println!("2) Absorb export into Surmount main (reviewed content import → PR):");
        println!("  grok-nix-helper import-upstream-export");
        println!("  IMPORT_NOW=1 grok-nix-helper sync-upstream");
        ExitCode::from(2)
    } else {
        let _ = writeln!(io::stderr(), "detect-upstream-export failed");
        code
    }
}
