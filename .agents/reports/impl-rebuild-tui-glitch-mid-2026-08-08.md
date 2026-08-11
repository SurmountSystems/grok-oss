# Implement: mid-`/rebuild` TUI footer corruption (`impl-rebuild-tui-glitch-mid`)

**Date:** 2026-08-08
**Crates:** `xai-grok-update`, `xai-grok-pager`, `xai-grok-pager-bin`
**Mode:** product fix + red/green unit contracts
**Prior incomplete fix:** `.agents/reports/impl-rebuild-tui-glitch-2026-08-07.md` (restore-fail gate, quiet post-restore eprintln, mode-aware hints, argv parity only)

---

## Root cause (proved from code)

### Mid-rebuild footer / layout corruption

`xai_grok_update::run_install` ran `just install` / `cargo build` with **`.status()` and no stdio redirects**. That **inherits** the parent process TTY.

`/rebuild` path:

1. `Action::RebuildAndRelaunch` → toast + system scrollback intro
2. `Effect::RunRebuild` → `rebuild_and_relaunch` on a JoinSet task
3. While ratatui still owns the **alt-screen**, cargo and just wrote **raw** ANSI, `\r` progress rewrites, and multi-line `==>` echoes onto the same PTY

That is exactly the dogfood mid-build screenshot: yellow intro + green status + magenta `==> cargo build ...` fighting the footer, Shift+Tab / shortcut rows duplicated and garbled, composer garbage.

Highest-priority hypothesis from the job is **confirmed by code**, not guesswork:

| Piece | Old behavior | Effect |
|-------|--------------|--------|
| `Command::new("just").arg("install").status()` | inherit stdout/stderr | justfile `@echo "==> cargo build..."` paints TUI |
| cargo child of just | inherit (TTY-ish) | progress bars / Compiling lines overwrite footer |
| strip fallback path | inherit | same class, smaller |
| Progress to TUI | static toast only | no sanitized channel; raw PTY was the “progress” |

Other long-running Effects (doctor fix, restore) either avoid host cargo on the TTY or push progress through `progress_tx` without inheriting install stdio.

### Post-rebuild broken glyphs (timestamps / yellow triangle)

Most likely **residue of mid-build paint corruption**, not a separate timestamp renderer bug:

- Raw CSI / partial cells left on the scroll surface while the TUI kept painting timestamps and chrome on top.
- After re-exec + restore, session transcript is readable and footer is mostly OK (matches screenshot B). Corrupted glyphs next to “12:39 AM” / right-edge yellow triangle match half-smashed cells, not a missing icon font for one codepath.
- Prior fix already gates re-exec on `restore_terminal` success and silences post-restore rebuild stderr. No evidence of intentionally incomplete leave-alt / mouse / kitty keyboard for the success path.

**Product change for glyphs:** none separate. Capture removes the source of mid-build cell trash. If glyphs still appear after dogfood with this binary, re-open as a dedicated timestamp/render issue with a clean mid-build capture.

---

## Fix

### 1. Always capture install stdio (`xai-grok-update`)

- `InstallStdioPolicy::Capture` only (`install_stdio_policy()`).
- `run_command_captured`: `stdin` null, `stdout`/`stderr` piped; stream lines off the TTY.
- Quiet cargo env: `CARGO_TERM_COLOR=never`, `CARGO_TERM_PROGRESS_WHEN=never`, `NO_COLOR=1`, `TERM=dumb`.
- `run_install_with_progress` + `rebuild_and_relaunch_with_progress` for stage callbacks.
- Failures include a tail of captured output.
- Strip helper uses null stdio.

### 2. Sanitize single-line progress

- `sanitize_rebuild_progress_line`: strip ANSI, take last `\r` segment, last non-empty line, drop C0 controls, truncate.
- `is_stable_height_progress_line`: no `\n` / `\r` / ESC.
- `is_rebuild_progress_stage_line`: forward only `==>`, `Compiling `, `Finished `, errors, etc. (no toast flood).

### 3. TUI toast channel (`xai-grok-pager`)

- `RestoreProgressMsg.toast: bool`.
- `Effect::RunRebuild` uses `rebuild_and_relaunch_with_progress` → progress_tx with `toast: true`.
- `TaskResult::RebuildProgress` → `agent.show_toast` only (not scrollback).
- Event loop maps `toast` true → `RebuildProgress`, false → existing session-restore scrollback path.

### 4. CLI

- `grok-oss rebuild` prints sanitized stage lines via the same capture path (never inherit).

---

## Red / green evidence

TDD shape: pure contracts that **fail under the old inherit-stdio product** (old path had no Capture policy, no sanitize, no toast-only progress arm).

| Test | Named contract | Would fail under old code |
|------|----------------|---------------------------|
| `install_stdio_policy_is_always_capture` | install never inherits TTY | Old `.status()` inherit |
| `sanitize_rebuild_progress_strips_ansi_and_carriage_returns` | ANSI+CR → stable line | No sanitizer |
| `sanitize_rebuild_progress_takes_last_line_of_multiline` | multi-line → last line | — |
| `sanitize_rebuild_progress_empty_is_none` | empty → None | — |
| `sanitize_rebuild_progress_truncates_long_lines` | max width | — |
| `stage_filter_keeps_just_and_cargo_markers` | stage filter | — |
| `progress_callback_path_only_forwards_stable_stage_lines` | forward path = sanitize + stage + stable | Old raw PTY |
| `rebuild_progress_updates_toast_not_scrollback` | toast only, no scrollback block | No `RebuildProgress` arm |

### Commands + exit codes

```text
cargo test -p xai-grok-update --lib rebuild::
# exit 0 — 11 passed (4 prior + 7 new capture/sanitize)

cargo test -p xai-grok-pager --lib rebuild_
# exit 0 — 14 passed (includes rebuild_progress_updates_toast_not_scrollback)

cargo fmt -p xai-grok-update -p xai-grok-pager -p xai-grok-pager-bin
# exit 0

cargo clippy -p xai-grok-update --lib -- -D warnings
# exit 0

cargo clippy -p xai-grok-pager --lib -- -D warnings
# exit 0
```

---

## Install / dogfood binary

```text
just install
# exit 0

~/.cargo/bin/grok-oss --version
# grok-oss 0.2.111 (c87f66a61d94) [stable]
```

Path: `/home/hunter/.cargo/bin/grok-oss` (stripped). No lld workaround needed this run; justfile `--config` rustflags were enough.

---

## Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-update/src/rebuild.rs` | Capture stdio, sanitize, progress API, tests |
| `crates/codegen/xai-grok-update/src/lib.rs` | Re-exports |
| `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs` | `RestoreProgressMsg.toast` |
| `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` | RunRebuild progress → toast channel |
| `crates/codegen/xai-grok-pager/src/app/actions.rs` | `TaskResult::RebuildProgress` |
| `crates/codegen/xai-grok-pager/src/app/event_loop.rs` | toast vs scrollback progress map |
| `crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs` | RebuildProgress → show_toast |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/task_result.rs` | toast-not-scrollback contract |
| `crates/codegen/xai-grok-pager-bin/src/main.rs` | CLI progress via capture |

No git commit / stage (agent policy).

---

## Remaining risk (honest)

| Risk | Status |
|------|--------|
| Mid-rebuild raw cargo/just on alt-screen | **Fixed** (capture + unit contracts) |
| Progress only as stable single-line toast | **Fixed** (sanitize + stage filter + toast arm) |
| Post-restore stderr / restore-fail re-exec | **Still fixed** from 2026-08-07 report |
| Glyph corruption after rebuild | **Expected gone** with capture; not a separate renderer fix. Dogfood `/rebuild` once to confirm |
| Live multipane Zellij/tmux paint | Still operator dogfood; units cannot prove full terminal paint |
| Very chatty `Compiling` toast churn | Stage filter includes every Compiling line; may replace toast often. Acceptable vs raw PTY; can throttle later if noisy |

---

## Operator check

1. Run the installed `grok-oss`.
2. `/rebuild` from a live session.
3. Mid-build: footer shortcuts should stay single-stacked; toast may update with `Rebuild: ==> ...` / `Compiling ...` only. No cargo progress bar painting over the composer.
4. After relaunch: timestamps and scroll chrome should not show leftover triangle/garbage from mid-build.

