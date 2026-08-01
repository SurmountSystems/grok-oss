//! Secure API key entry for console / OpenRouter login.
//!
//! Hard rule: secrets must never be accepted as CLI argument values. Argv
//! lands in shell history, process lists, and some audit logs. Interactive
//! entry uses no-echo TTY reads (`rpassword`, prefers `/dev/tty`). Automation
//! uses env vars or `login --api-key -` (one line on **non-TTY** process stdin).

use std::io::{self, BufRead, IsTerminal, Write};

/// Sentinel value for `--api-key -` (read one line from process stdin).
pub const API_KEY_STDIN_SENTINEL: &str = "-";

/// Errors when classifying or reading a CLI API key value.
#[derive(Debug, thiserror::Error)]
pub enum CliApiKeyError {
    /// Non-empty argv value that is not the stdin sentinel — refused.
    #[error(
        "Refusing to accept an API key on the command line.\n\
         \n\
         Secrets passed as arguments land in shell history (fish/bash/zsh), \
         process lists, and some audit logs — that is thoughtless security.\n\
         \n\
         Instead:\n\
           grok login --api-key\n\
               # enter the key at the no-echo prompt\n\
           # automation: set XAI_API_KEY (or OPENROUTER_API_KEY) in the environment\n\
           # advanced: `login --api-key -` reads one line from non-TTY stdin (not argv)"
    )]
    ArgvSecretRefused,
    /// `--api-key -` with a TTY stdin would echo; use bare flag for no-echo.
    #[error(
        "Refusing to read an API key from a TTY on stdin.\n\
         \n\
         `login --api-key -` is for non-TTY automation only (piped/redirected \
         input). Run `login --api-key` alone for a no-echo prompt."
    )]
    StdinIsTty,
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Classify clap's optional `--api-key` value and materialize a key when needed.
///
/// | Clap value | Result |
/// |------------|--------|
/// | `None` (flag absent) | `Ok(None)` — interactive prompt (caller) |
/// | `Some("")` (bare `--api-key`) | `Ok(None)` — interactive prompt |
/// | `Some("-")` + non-TTY stdin | `Ok(Some(line))` — one stdin line |
/// | `Some("-")` + TTY stdin | `Err(StdinIsTty)` |
/// | `Some(other)` | `Err(ArgvSecretRefused)` — never stores |
///
/// Does **not** store anything; only refuses or reads stdin.
pub fn materialize_cli_api_key(cli_value: Option<&str>) -> Result<Option<String>, CliApiKeyError> {
    materialize_cli_api_key_with(
        cli_value,
        io::stdin().is_terminal(),
        read_api_key_from_stdin_line,
    )
}

/// Testable core of [`materialize_cli_api_key`].
pub fn materialize_cli_api_key_with<F>(
    cli_value: Option<&str>,
    stdin_is_tty: bool,
    read_stdin: F,
) -> Result<Option<String>, CliApiKeyError>
where
    F: FnOnce() -> io::Result<String>,
{
    match cli_value {
        None | Some("") => Ok(None),
        Some(API_KEY_STDIN_SENTINEL) => {
            if stdin_is_tty {
                return Err(CliApiKeyError::StdinIsTty);
            }
            Ok(Some(read_stdin()?.trim().to_owned()))
        }
        Some(_) => Err(CliApiKeyError::ArgvSecretRefused),
    }
}

/// Whether a clap `--api-key` value is a forbidden argv secret (not bare, not `-`).
pub fn is_argv_api_key_secret(cli_value: &str) -> bool {
    !cli_value.is_empty() && cli_value != API_KEY_STDIN_SENTINEL
}

/// No-echo prompt for an API key (reads from the controlling TTY when possible).
///
/// Uses `rpassword` so the secret is not echoed and is not read from piped
/// stdin (which would break scripts that only pipe other data).
pub fn prompt_api_key_no_echo(prompt: &str) -> io::Result<String> {
    let key = rpassword::prompt_password(prompt)?;
    Ok(key.trim().to_owned())
}

/// Read one line from process stdin (for `login --api-key -` automation).
///
/// Call only when stdin is **not** a terminal ([`materialize_cli_api_key`]
/// enforces that). Does not document `echo KEY |` as the happy path.
pub fn read_api_key_from_stdin() -> io::Result<String> {
    if io::stdin().is_terminal() {
        return Err(io::Error::other(
            "stdin is a terminal; use bare --api-key for a no-echo prompt",
        ));
    }
    read_api_key_from_stdin_line()
}

fn read_api_key_from_stdin_line() -> io::Result<String> {
    // Ensure prompts/diagnostics on stderr are visible before blocking on stdin.
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_flag_and_absent_mean_interactive() {
        assert_eq!(materialize_cli_api_key(None).unwrap(), None);
        assert_eq!(materialize_cli_api_key(Some("")).unwrap(), None);
    }

    #[test]
    fn argv_secret_is_refused_and_not_materialized() {
        // Fake non-secret-looking value — still refused (never store from argv).
        let err = materialize_cli_api_key(Some("xai-fake-not-a-real-key")).unwrap_err();
        assert!(matches!(err, CliApiKeyError::ArgvSecretRefused));
        let msg = err.to_string();
        assert!(
            msg.contains("shell history") || msg.contains("history"),
            "stderr explanation must mention history: {msg}"
        );
        assert!(
            msg.contains("process list") || msg.contains("process lists"),
            "stderr explanation must mention process lists: {msg}"
        );
        assert!(
            msg.contains("login --api-key"),
            "must point at flag-only path: {msg}"
        );
        assert!(is_argv_api_key_secret("xai-fake-not-a-real-key"));
        assert!(!is_argv_api_key_secret(""));
        assert!(!is_argv_api_key_secret("-"));
    }

    #[test]
    fn equals_style_value_also_refused() {
        // Clap may surface `--api-key=…` the same as a separate arg value.
        let err = materialize_cli_api_key(Some("sk-or-test-value")).unwrap_err();
        assert!(matches!(err, CliApiKeyError::ArgvSecretRefused));
    }

    #[test]
    fn stdin_sentinel_on_tty_is_refused() {
        let err = materialize_cli_api_key_with(Some("-"), true, || {
            panic!("must not read stdin when TTY")
        })
        .unwrap_err();
        assert!(matches!(err, CliApiKeyError::StdinIsTty));
        let msg = err.to_string();
        assert!(msg.contains("no-echo") || msg.contains("TTY"), "{msg}");
    }

    #[test]
    fn stdin_sentinel_on_pipe_reads_line() {
        let key =
            materialize_cli_api_key_with(Some("-"), false, || Ok("  piped-key  ".into())).unwrap();
        assert_eq!(key.as_deref(), Some("piped-key"));
    }
}
