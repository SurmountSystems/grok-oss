//! Read `/running --json` from stdin or a file. Print safe JSON (no prompt).
//!
//! This binary is not a grok-oss TUI and not `/dashboard`.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use surmount_coordinator_gui::{SessionHost, safe_json_from_running};

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<String, String> {
    let parsed = parse_args(args)?;
    let input = match parsed.file {
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|err| format!("could not read stdin: {err}"))?;
            buf
        }
        Some(path) => fs::read_to_string(&path)
            .map_err(|err| format!("could not read {}: {err}", path.display()))?,
    };
    safe_json_from_running(&input, parsed.host).map_err(|err| err.to_string())
}

struct Args {
    file: Option<PathBuf>,
    host: SessionHost,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut file = None;
    let mut host = SessionHost::Local;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--help" || arg == "-h" {
            return Err("surmount-coordinator-gui [--host NAME] [FILE]\n\
                 Read /running --json from FILE or stdin. Print safe JSON (no prompt)."
                .to_string());
        }
        if arg == "--host" {
            let name = iter
                .next()
                .ok_or_else(|| "--host needs a host name".to_string())?;
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err("--host needs a host name".to_string());
            }
            host = if trimmed.eq_ignore_ascii_case("local") {
                SessionHost::Local
            } else {
                SessionHost::Remote(trimmed.to_string())
            };
            continue;
        }
        if arg.starts_with('-') {
            return Err(format!("unknown argument: {arg}"));
        }
        if file.is_some() {
            return Err("only one FILE is allowed".to_string());
        }
        file = Some(PathBuf::from(arg));
    }
    Ok(Args { file, host })
}
