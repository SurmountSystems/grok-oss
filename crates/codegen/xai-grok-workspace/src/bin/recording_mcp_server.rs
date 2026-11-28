//! Stdio stand-in for the computer-use helper.
//!
//! Appends every JSON line it reads to the log path in argv[1]. Answers
//! `initialize` and `tools/call` named `fast`. Leaves `tools/call` named
//! `slow` unanswered so a caller can observe cancellation.

use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use serde_json::{Value, json};

fn reply(msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(Value::as_str)?;
    let id = msg.get("id")?;
    if method == "initialize" {
        let protocol = msg.pointer("/params/protocolVersion")?;
        return Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": protocol,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "recording", "version": "0" }
            }
        }));
    }
    if method == "tools/call" && msg.pointer("/params/name").and_then(Value::as_str) == Some("fast")
    {
        return Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": "done" }],
                "isError": false
            }
        }));
    }
    None
}

fn main() -> ExitCode {
    let Some(log_path) = std::env::args().nth(1) else {
        eprintln!("usage: recording_mcp_server <log-path>");
        return ExitCode::from(2);
    };
    let mut log = match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("open {log_path}: {err}");
            return ExitCode::from(1);
        }
    };
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return ExitCode::from(1);
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            return ExitCode::from(1);
        };
        if writeln!(log, "{line}").is_err() || log.flush().is_err() {
            return ExitCode::from(1);
        }
        if let Some(out) = reply(&msg) {
            let Ok(text) = serde_json::to_string(&out) else {
                return ExitCode::from(1);
            };
            let mut stdout = io::stdout().lock();
            if writeln!(stdout, "{text}").is_err() || stdout.flush().is_err() {
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}
