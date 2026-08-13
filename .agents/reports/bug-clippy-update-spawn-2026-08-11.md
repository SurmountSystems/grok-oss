# Report: clippy disallowed `Command::spawn` (xai-grok-update + workspace scan)

Date: 2026-08-11
Package focus: `xai-grok-update`, plus proactive product sites that would fail `just test-clippy` (`cargo clippy --workspace --lib --bins -D warnings`).

## Problem

`clippy.toml` bans bare:

- `std::process::Command::spawn`
- `tokio::process::Command::spawn`
- `portable_pty::SlavePty::spawn_command`

Reason: an unenrolled child can outlive the session; enroll via `xai_tty_utils::ProcessScope` (tokio `enroll` / `spawn`, or std `ProcessGroup::attach_std` + `global_process_scope().register`).

Operator-known red sites:

| File | Line (pre-fix) | API |
|------|------------------|-----|
| `crates/codegen/xai-grok-update/src/auto_update.rs` | ~706 | `tokio::process::Command::spawn` |
| `crates/codegen/xai-grok-update/src/rebuild.rs` | ~676 | `std::process::Command::spawn` |

## Fixes

### 1. `xai-grok-update` (required)

**`auto_update.rs` — NonBlocking `grok update` child**

- Site-local `#[allow(clippy::disallowed_methods)]` at the enroll boundary.
- After `detach_command`, `cmd.spawn()` then `global_process_scope().enroll(&child)?`.
- `ProcessScope` holds only a `Weak`; the `Child` is returned and waited on later (quit-for-update / `BackgroundUpdateCheck::download`). The owning `Arc<ProcessGroup>` is `mem::forget`’d so session `kill_all` can still upgrade and reap for the parent process lifetime (one background download per session at most).
- Uses existing `xai_grok_tools::util::{global_process_scope, ProcessGroup}` re-exports (no new Cargo dep).

**`auto_update.rs` — Windows `restart_grok` spawn+exit**

- `cfg(not(unix))` only (Unix uses `exec`).
- Enroll via `ProcessGroup::attach_std` + `register`; forget Arc before `exit(0)`.

**`rebuild.rs` — `run_command_captured`**

- Std enroll pattern matching tools implement_memory / pager-render link opener:
  - allow + `cmd.spawn()`
  - `ProcessGroup::new` / `attach_std` / `global_process_scope().register`
  - closed scope → kill + wait + `bail!`
  - `drop(group)` after successful `wait`

### 2. Proactive product sites (same class)

**`xai-grok-pager/src/pty_wrap.rs`**

- Bare `portable_pty` `spawn_command` (clippy-banned path) with no allow/enroll.
- Fixed: allow + `enroll_terminal_pid` when pid known; hold `Arc` until `child.wait()`; drop after reap.
- Same pattern as shell `pty_session` / pager-pty-harness.

**`xai-grok-pager/src/app/dispatch/rebuild.rs`**

- Windows-only rebuild relaunch spawn+exit (mirror of update `restart_grok`).
- Enroll std path + forget Arc before `exit(0)`.

## Inventory: remaining bare spawns vs fixed

CI shape: `just test-clippy` → `cargo clippy --workspace --lib --bins` (not `--all-targets`). Tests / `cfg(test)` are out of that gate.

### Fixed this turn

| Site | Kind | Action |
|------|------|--------|
| `xai-grok-update` `auto_update.rs` NonBlocking | tokio | enroll + forget Arc |
| `xai-grok-update` `auto_update.rs` Windows restart | std | attach_std + register |
| `xai-grok-update` `rebuild.rs` `run_command_captured` | std | attach_std + register, drop after wait |
| `xai-grok-pager` `pty_wrap.rs` | portable_pty | enroll_terminal_pid |
| `xai-grok-pager` `dispatch/rebuild.rs` Windows relaunch | std | attach_std + register |

### Already enrolled / allowed (left alone; already green)

Known good packages called out by operator (do not re-break): tools, pager-pty-harness, pager-render, shell.

Also already site-allowed or enrolled elsewhere, including hooks runner, MCP `SafeTokioChildProcess`, mermaid subprocess, workspace envrc/restore_fetch, voice capture, shared clipboard probes, opencode/grok_build grep/glob, tools terminal/shell_state/static_shell, pager notifications, screen_mode_relaunch, config validator, fast-worktree git, plugin-marketplace git, ptyctl, etc.

### Intentionally residual (not product clippy lib/bin red, or not bare std/tokio path)

| Site | Why left |
|------|----------|
| `cargo-mem-guard` | Workspace-**excluded**; host wrapper binary, not `test-clippy` members |
| `streaming_local_terminal.rs` `CommandWrap::spawn` | `process_wrap` API, not `tokio::process::Command::spawn` path; JobObject/ProcessSession teardown |
| `tools` `LocalComputer::spawn_command` method name | Method on computer trait, not `portable_pty::SlavePty::spawn_command` |
| `tools` `terminal.rs` Windows breakaway retry `cmd.spawn` | Covered by outer `#[allow(clippy::disallowed_methods)]` on the Windows spawn block |
| `hooks` multi-line `cmd.spawn()` | Already has allow + session scope register above the call |
| `cfg(test)` / `tests/` fixtures (fast-worktree sleep, lock holder, tty-utils tests, …) | Not in `--lib --bins` CI gate; many already allow-commented |
| `Command::status` / `output` / Unix `exec` paths in update | Not banned methods |
| Third-party / build scripts | Out of product enroll scope |

### Scan method

Workspace `rg` over `crates/**/*.rs` for `.spawn(` / `spawn_command` near `Command`, excluding packages that are the enroll primitive (`xai-tty-utils`) and test-support, plus allow-attribute windows. Re-checked after edits: no remaining **unenrolled product** std/tokio/portable_pty call sites under CI lib/bins that lack allow or enroll.

## Verify

```text
cargo fmt -p xai-grok-update -p xai-grok-pager
# (fmt applied)

cargo clippy -p xai-grok-update --lib --bins -- -D warnings
# exit 0

cargo clippy -p xai-grok-update --all-targets -- -D warnings
# exit 0  (required)

cargo clippy -p xai-grok-pager --lib --bins -- -A dead_code -A unused -D clippy::disallowed_methods
# exit 0  (spawn class clean; full -D warnings hits pre-existing dead_code/unused elsewhere in pager, unrelated to this fix)
```

Packages green for this bug class:

- **`xai-grok-update`**: full `--all-targets -D warnings` green
- **`xai-grok-pager`**: disallowed spawn methods clean on lib+bins

No Cargo.toml dependency add: update already pulls process scope via `xai-grok-tools`; pager already depends on `xai-tty-utils`.
