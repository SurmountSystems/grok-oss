//! `just install` verify runs `grok-oss --version` after copy.
//!
//! `/rebuild` captures that recipe (stdin `/dev/null`, stdout/stderr pipes,
//! `setsid` so the child has no controlling terminal). `--version` must print
//! the version to stdout and exit 0 without opening a TTY. A missing dispatch
//! falls through into TUI `enable_raw_mode` and fails with ENXIO
//! ("No such device or address (os error 6)").

use std::process::{Command, Output, Stdio};

/// Resolve the composition-root binary (`grok-oss`). Same order as
/// `update_never_blocked_by_config.rs`.
fn pager_binary() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("PAGER_BINARY") {
        return std::path::absolute(&p)
            .unwrap_or_else(|e| panic!("failed to absolutize PAGER_BINARY {p}: {e}"));
    }
    if let Some(p) = option_env!("CARGO_BIN_EXE_grok_oss") {
        return std::path::PathBuf::from(p);
    }
    if let Some(p) = option_env!("CARGO_BIN_EXE_grok-oss") {
        return std::path::PathBuf::from(p);
    }
    for key in [
        "CARGO_BIN_EXE_grok-oss",
        "CARGO_BIN_EXE_grok_oss",
        "CARGO_BIN_EXE_xai-grok-pager",
        "CARGO_BIN_EXE_xai_grok_pager",
    ] {
        if let Ok(p) = std::env::var(key) {
            let path = std::path::PathBuf::from(&p);
            if path.exists() {
                return path;
            }
        }
    }
    panic!(
        "PAGER_BINARY is unset and cargo did not inject CARGO_BIN_EXE_grok_oss \
         (this integration test must run against the grok-oss bin)"
    );
}

/// Isolated `HOME`/`GROK_HOME` so a broken `--version` path cannot write the
/// operator's real grok home. Keep the [`tempfile::TempDir`] until the child
/// exits.
fn isolated_version_command(args: &[&str]) -> (tempfile::TempDir, Command) {
    let home = tempfile::tempdir().expect("temp HOME");
    let mut cmd = Command::new(pager_binary());
    cmd.args(args)
        .env_clear()
        .env("HOME", home.path())
        .env("GROK_HOME", home.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    (home, cmd)
}

fn assert_version_ok(output: Output, label: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{label}: --version must exit 0 without a terminal\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("No such device or address") && !stderr.contains("os error 6"),
        "{label}: --version must not fail with ENXIO\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let product_token = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("");
    assert_eq!(
        product_token, "grok-oss",
        "{label}: --version product token must be grok-oss, not bare grok \
         (operator saw `grok 1.0.3 (…)`)\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout
            .lines()
            .any(|line| line.split_whitespace().next() == Some("grok")),
        "{label}: --version must not print a line whose product token is bare grok\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.chars().any(|c| c.is_ascii_digit()),
        "{label}: --version must print a real version + git/sha to stdout\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

/// stdin is `/dev/null` (not a TTY). Same fd 0 `just install` inherits from
/// `/rebuild`'s captured `Stdio::null()`.
#[test]
fn version_flag_exits_zero_when_stdin_is_dev_null() {
    let (_home, mut cmd) = isolated_version_command(&["--version"]);
    let output = cmd
        .stdin(Stdio::null())
        .output()
        .expect("spawn grok-oss --version");
    assert_version_ok(output, "stdin=/dev/null");
}

/// stdin is a closed pipe (writer dropped). Distinct from `/dev/null`: some
/// ioctls return ENXIO on a closed fd rather than ENOTTY.
#[test]
fn version_flag_exits_zero_when_stdin_pipe_is_closed() {
    let (_home, mut cmd) = isolated_version_command(&["--version"]);
    let mut child = cmd
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn grok-oss --version");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait grok-oss --version");
    assert_version_ok(output, "stdin closed pipe");
}

/// `/rebuild` `just install` shape: no controlling terminal (`setsid`) and
/// stdin `/dev/null`. Opening `/dev/tty` then returns ENXIO.
#[cfg(unix)]
#[test]
fn version_flag_exits_zero_when_rebuild_captures_stdio() {
    let (_home, mut cmd) = isolated_version_command(&["--version"]);
    cmd.stdin(Stdio::null());
    xai_tty_utils::detach_std_command(&mut cmd);
    let output = cmd.output().expect("spawn detached grok-oss --version");
    assert_version_ok(output, "rebuild capture (setsid + stdin null)");
}
