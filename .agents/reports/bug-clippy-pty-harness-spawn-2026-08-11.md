# Clippy: pager PTY harness `spawn_command` enroll

**Date:** 2026-08-11
**Package:** `xai-grok-pager-pty-harness`
**Status:** fixed; verify exit 0

## Problem

`cargo clippy -p xai-grok-pager-pty-harness --all-targets -- -D warnings` failed on:

```
error: use of a disallowed method `portable_pty::SlavePty::spawn_command`
  --> crates/codegen/xai-grok-pager-pty-harness/src/pty.rs
  reason: enroll the shell with xai_tty_utils::ProcessScope::enroll_terminal_pid
```

## Fix

Matched the shell PTY pattern (`xai-grok-shell` `pty_session.rs`):

1. `#[allow(clippy::disallowed_methods)]` on the `spawn_command` site with a short enroll comment.
2. After spawn, `xai_tty_utils::global_process_scope().enroll_terminal_pid(pid)` (best-effort `.ok()` for short-lived child races).
3. Hold `Option<Arc<ProcessGroup>>` on `PtyController` so ProcessScope’s `Weak` stays live until cleanup.
4. Clear `process_group` in `release_process_tree` before tree release (PID-reuse safety).

`TestProcessTree` stays for harness kill/diagnostics (Unix double-attach is fine; Windows job attach remains best-effort).

## Direct fallout for `--all-targets`

`settings_locked_row_e2e` did not compile (dirty tree wire-up, not the spawn lint itself):

- Re-exported `seed_fake_oauth_team_member` / `seed_fake_oauth_zdr_team` from `lib.rs` (already defined in `flows.rs`).
- Added `keys::RIGHT` (`\x1b[C`) and `keys::F2` (`\x1bOQ`, SS3 form used by other pager e2e).

Did **not** change `clippy.toml` (`tokio::process::Command::spawn` allow-invalid).

## Verify

```bash
cargo fmt -p xai-grok-pager-pty-harness
cargo clippy -p xai-grok-pager-pty-harness --all-targets -- -D warnings
```

**Exit code:** 0

## Files

- `crates/codegen/xai-grok-pager-pty-harness/src/pty.rs`
- `crates/codegen/xai-grok-pager-pty-harness/src/lib.rs`
