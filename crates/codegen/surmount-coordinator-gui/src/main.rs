//! Read `/running --json` from stdin or a file. Print safe JSON (no prompt).
//!
//! Subcommand `set-remote-host-console-api-key` is the laptop-side action
//! that writes a machine console API key staging file. It never prints
//! the key. This binary is not a grok-oss TUI and not `/dashboard`.

use std::env;
use std::io;
use std::process::ExitCode;

use surmount_coordinator_gui::run_cli;

fn main() -> ExitCode {
    match run_cli(env::args().skip(1), io::stdin()) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
