# bug: clippy update + pager — greened (2026-08-12)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**No git commit.**

## Goal

1. Green `xai-grok-update` (2 disallowed `Command::spawn` blocking pager).
2. Green `cargo clippy -p xai-grok-pager --all-targets -- -D warnings`.

Under max nice.

## Results

| Check | Exit | Notes |
|-------|-----:|-------|
| `cargo clippy -p xai-grok-update --all-targets -- -D warnings` | **0** | Enroll + site-local allow |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **0** | Full all-targets green |
| `cargo fmt -p xai-grok-update -p xai-grok-pager` | **0** | Clean |
| `cargo test -p xai-grok-pager --lib is_session_update_ext_method` | **0** | 1 passed smoke |

Host still prints the pre-existing `clippy.toml` warning that `tokio::process::Command::spawn` is not a reachable path (does not fail `-D warnings`).

## 1. `xai-grok-update` (required blocker)

| Site | API | Fix |
|------|-----|-----|
| `src/auto_update.rs` NonBlocking `grok update` | `tokio::process::Command::spawn` | `#[allow(clippy::disallowed_methods)]` + `global_process_scope().enroll(&child)?` + `mem::forget(group)` so quit-for-update can still wait while session `kill_all` can reap |
| `src/auto_update.rs` Windows `restart_grok` | `std::process::Command::spawn` | `detach_std_command` + allow + `ProcessGroup::attach_std` + `register` + forget before `exit(0)` (`cfg(not(unix))`; Unix keeps `exec`) |
| `src/rebuild.rs` `run_command_captured` | `std::process::Command::spawn` | `detach_std_command` + allow + attach/register; closed scope → kill + wait + `bail!`; `drop(group)` after successful wait |

Pattern matches tools implement_memory / pager-render link opener / prior `.agents/reports/bug-clippy-update-spawn-2026-08-11.md` intent (that enroll was not present on disk when this run started).

No Cargo.toml change: process scope via existing `xai-grok-tools` re-exports.

## 2. `xai-grok-pager` residual (after update dep green)

### Disallowed methods

| Site | Fix |
|------|-----|
| `src/pty_wrap.rs` `portable_pty::SlavePty::spawn_command` | allow + `enroll_terminal_pid` when pid known; `drop(process_group)` after `child.wait()` |
| `src/diagnostics/fix_tests.rs` `Path::canonicalize` | `dunce::canonicalize` |
| `tests/doctor_early_dispatch.rs` `Path::canonicalize` | `dunce::canonicalize` |

### Visibility / style

| Kind | Sites | Fix |
|------|-------|-----|
| `private_interfaces` | `StructuralScrollAnchor` vs `ScrollbackState` field | `pub(crate)` on the type |
| `empty_line_after_doc_comments` | setters / status / turn (~21) | remove blank line between doc and item |
| `manual_contains` | `agent_view/plan.rs` test | `names.contains(&…)` |
| `expect_fun_call` | `agent_view/render.rs` test | `unwrap_or_else(\|\| panic!(…))` |
| `len_zero` | acp_handler plan_mode test | `!is_empty()` |
| `nonminimal_bool` | dispatch/queue test | `is_none_or` form |
| `bool_assert_comparison` | session_startup test | `assert!(!…)` |
| `identity_op` | scrollback/selection test | drop `0 +` |
| `needless_range_loop` | benches/edit_highlight | iterate slice with enumerate |
| `assertions_on_constants` | settings_e2e residual test | empty body + comment (no `assert!(true)`) |
| non-exhaustive match | settings_e2e filter mode | cover `ActionThenClose` |
| unused enum variants | pty_e2e `WakeCancelGesture::{Esc,StopClick}` | `#[allow(dead_code)]` (mirror cases not in `pty_e2e_queue` crate) |

### Dead / unused product leftovers

Site-local `#[allow(dead_code)]` (or `#[cfg_attr(not(test), allow(dead_code))]`) on intentional but currently unwired product helpers: version_mismatch module + classifier (orphan `headless/` tree is not the live `headless.rs` module), cancel-resend reconcile, privacy banner Action aliases, session-picker delete confirm, external-auth detect, late-replay grace, dashboard stop helpers, etc.

`#[cfg(test)]` re-exports restored for cancel-resend + `should_drop_duplicate_auto_recap` so unit tests keep compiling without lib unused-import red.

## Touch list (high level)

| Package | Paths |
|---------|--------|
| update | `auto_update.rs`, `rebuild.rs` |
| pager enroll / path | `pty_wrap.rs`, `diagnostics/fix_tests.rs`, `tests/doctor_early_dispatch.rs` |
| pager dead_code / re-export | `acp/{mod,tracker,version_mismatch}`, `agent_view/{mod,session}`, `app_view`, `acp_handler/{mod,permissions}`, `dispatch/{mod,status,turn}`, `error_display`, `slash/{mod,commands/exit,usage}`, `views/session_picker` |
| pager style / tests | settings setters/status/turn docs, plan/render/queue/session_startup/selection tests, benches/edit_highlight, settings_e2e, pty_e2e wake-cancel enum, screen_mode_relaunch/switch test cleanups, scrollback StructuralScrollAnchor |

## Residual

1. Orphan `src/headless/` directory (not `headless.rs`) still not wired; version_mismatch / session_update helpers stay allowed until that tree is reattached.
2. Several product helpers keep `#[allow(dead_code)]` pending real call sites (cancel resend event-loop tick, session-picker delete, external-auth detect, …).
3. Host `clippy.toml` unreachable-path warning for `tokio::process::Command::spawn` remains.

## Commands

```bash
nice -n 19 ionice -c3 cargo clippy -p xai-grok-update --all-targets -- -D warnings   # exit 0
nice -n 19 ionice -c3 cargo clippy -p xai-grok-pager --all-targets -- -D warnings    # exit 0
cargo fmt -p xai-grok-update -p xai-grok-pager                                         # exit 0
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib is_session_update_ext_method -- --test-threads=1  # exit 0
```
