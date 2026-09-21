//! Grok Build compatibility CLI bins of the allowlisted skill Rust functions.
//!
//! argv0 names:
//! - `grok-oss-implement-memory`
//! - `grok-oss-plan-validate`
//! - `grok-oss-session-reader`
//!
//! grok-oss also intercepts these argv shapes in the bash tool so a live
//! session does not spawn Python. Do not generate a throwaway `.py` / `.sh`.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.join(" ");
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if let Some(hit) = xai_grok_tools::util::implement_memory::try_parse_memory_intercept(&cmd) {
        let h = xai_grok_tools::util::implement_memory::execute_intercept(&hit, &cwd, None);
        return emit(h.stdout, h.stderr, h.exit_code);
    }
    if let Some(hit) = xai_grok_tools::util::plan_validate::try_parse_plan_validate_intercept(&cmd)
    {
        let h = xai_grok_tools::util::plan_validate::execute_intercept(&hit, &cwd);
        return emit(h.stdout, h.stderr, h.exit_code);
    }
    if let Some(hit) =
        xai_grok_tools::util::session_reader::try_parse_session_reader_intercept(&cmd)
    {
        let h = xai_grok_tools::util::session_reader::execute_intercept(&hit, &cwd);
        return emit(h.stdout, h.stderr, h.exit_code);
    }

    eprintln!(
        "grok-oss skill CLI: unknown argv {cmd:?}. Use grok-oss-implement-memory, grok-oss-plan-validate, or grok-oss-session-reader."
    );
    ExitCode::from(2)
}

fn emit(stdout: String, stderr: String, exit_code: i32) -> ExitCode {
    if !stdout.is_empty() {
        let _ = std::io::stdout().write_all(stdout.as_bytes());
    }
    if !stderr.is_empty() {
        let _ = std::io::stderr().write_all(stderr.as_bytes());
    }
    ExitCode::from(exit_code.clamp(0, 255) as u8)
}
