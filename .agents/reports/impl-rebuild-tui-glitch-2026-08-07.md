# Implement: `/rebuild` TUI glitch (`bug:rebuild-tui-glitch`)

**Date:** 2026-08-07
**Branch:** `fixes-2`
**Crate:** `xai-grok-pager`
**Mode:** product fix + red/green unit contracts (not diagnosis-only)

---

## Root cause (proved from code)

Prior diagnosis listed five suspects. Code read + pure contracts prove three **real product bugs** on the exit path, and demote the double-TUI race as **not same-PTY double exec**.

### 1. Restore failure still re-exec'd (proved)

In `app/mod.rs` after the event loop:

1. `restore_terminal(...)` always ran.
2. On `Err`, only a `tracing::warn` fired.
3. On `Ok(run_result)` with `rebuild_relaunch` / `relaunch` set, **`exec_*` always ran** regardless of restore outcome.

`restore_terminal_with` still runs teardown on drain failure, but returns `Err` when the writer drain fails. Re-exec after a failed drain is the half-restored glitch class: late frames / partial cleanup under a fresh process image.

**Fix:** shared gate `may_exec_relaunch_after_restore(restore_ok)`. Both rebuild and screen-mode re-exec refuse to `exec` when restore failed, and print a fail-loud resume hint instead.

### 2. Post-restore stderr flash on rebuild (proved)

`exec_rebuild_relaunch` always did:

```text
eprintln!("Relaunching on {} (session {})…", …);
flush; Command::exec
```

Order today (unchanged): **restore first** (leave alt screen, raw off, …), **then** relaunch. Any stderr after restore lands on the **primary screen** between leave-alt-screen and the new process init. Toast + scrollback already told the operator the relaunch was coming.

Screen-mode relaunch keeps its intentional mode-switch line (`Reopening session in … mode…`). Rebuild does **not** need a second line after restore.

**Fix:** `rebuild_relaunch_post_restore_user_message` contract returns `None`; `exec_rebuild_relaunch` no longer `eprintln`s. Progress is `tracing::info` only.

### 3. Rebuild exec-failure hint weaker than screen-mode (proved)

Old rebuild failure text:

```text
Resume with: grok-oss --resume {id}
```

No `GROK_SCREEN_MODE`, no `--minimal`/`--fullscreen`. Screen-mode path already used `screen_mode_relaunch_resume_hint`. A failed rebuild relaunch could reopen the wrong mode.

**Fix:** `print_rebuild_exec_failure_hint` and `print_rebuild_restore_blocked_hint` both use the same full resume hint as screen-mode.

### 4. Argv/env parity (already shared; now locked)

`exec_rebuild_relaunch` already called `build_screen_mode_relaunch_args` + `GROK_SCREEN_MODE`. Extracted `plan_rebuild_relaunch` and tests that the plan **equals** the screen-mode builder for the same session/mode, so drift cannot return silently.

### 5. Multi-process “double relaunch” (not same-PTY double paint)

| Actor | What happens on `/rebuild` |
|-------|----------------------------|
| Install path | `rebuild_and_relaunch` → `signal_leaders_to_relaunch` |
| Leader | Accepts `RelaunchForUpdate`, drains, `ShutdownReason::AutoUpdate`, restarts leader process |
| Invoking TUI | Arms `rebuild_relaunch`, quits, restores terminal, **self-exec** onto installed binary with `--resume` |
| Sibling TUI clients | Stay on **old client binary**; reconnect via `session/load` when leader is back. They do **not** self-exec |

Leader and client are different processes. Leader is not painting the invoking pane. Same-PTY double init is **not** “leader RelaunchForUpdate + self-exec both paint this TTY.” Complementary jobs:

- Leader signal: other leaders / sessions pick up the new **leader** binary.
- Self-exec: **this** client process becomes the new **client** binary.

No product change to suppress leader signal for the local case: siblings still need leader soft-relaunch. Suppressing self-exec would leave the invoking pane on the old binary.

---

## Red tests (contracts) + green evidence

TDD shape: pure contracts that **fail under the old product behavior**, then product wired to them.

| Test | Named contract | Would fail under old code |
|------|----------------|---------------------------|
| `may_exec_relaunch_blocked_when_restore_failed` | restore `Err` → no re-exec | Old path ignored restore for relaunch |
| `may_exec_relaunch_allowed_when_restore_ok` | restore `Ok` → allow | — |
| `rebuild_relaunch_has_no_post_restore_user_stderr` | no post-restore user stderr for rebuild | Old `eprintln!("Relaunching on…")` |
| `plan_rebuild_relaunch_matches_screen_mode_args_and_env` | argv+env parity with screen-mode | Drift guard |
| `plan_rebuild_relaunch_minimal_sets_minimal_env_and_flag` | minimal mode plan | Drift guard |
| `restore_blocked_hint_mentions_cleanup_and_resume` | fail-loud blocked path | New |
| `exec_failure_hint_uses_screen_mode_resume_hint` | full mode-aware resume hint | Old bare `grok-oss --resume` |
| `print_screen_mode_restore_blocked_hint_writes_expected_lines` | screen-mode shares restore gate | Old path re-exec'd after restore fail |

### Commands + exit codes

```text
cargo test -p xai-grok-pager --lib may_exec_relaunch
# exit 0 — 2 passed

cargo test -p xai-grok-pager --lib rebuild_relaunch
# exit 0 — 4 passed (struct + quiet + plan x2)

cargo test -p xai-grok-pager --lib plan_rebuild
# exit 0 — 2 passed

cargo test -p xai-grok-pager --lib restore_blocked
# exit 0 — 2 passed (rebuild + screen-mode hints)

cargo test -p xai-grok-pager --lib exec_failure_hint
# exit 0 — 1 passed

cargo test -p xai-grok-pager --lib print_screen_mode_restore
# exit 0 — 1 passed

cargo fmt -p xai-grok-pager
# exit 0

cargo clippy -p xai-grok-pager --lib -- -D warnings
# exit 0

# Note: cargo clippy -p xai-grok-pager --all-targets -- -D warnings fails on
# pre-existing issues outside this change (doctor_early_dispatch canonicalize,
# benches, plan.rs, etc.). Touched lib path is clean.
```

---

## Product change summary

### `crates/codegen/xai-grok-pager/src/app/dispatch/rebuild.rs`

- `may_exec_relaunch_after_restore(bool)` — shared gate.
- `rebuild_relaunch_post_restore_user_message` — always `None` (quiet rebuild).
- `plan_rebuild_relaunch` / `RebuildRelaunchPlan` — pure argv+env plan.
- `exec_rebuild_relaunch` — uses plan; **no** user `eprintln`; `tracing::info` only.
- `print_rebuild_restore_blocked_hint` / `print_rebuild_exec_failure_hint` — mode-aware resume hints.
- Unit tests for all contracts above.

### `crates/codegen/xai-grok-pager/src/app/mod.rs`

- Capture `restore_ok` from `restore_terminal`.
- Rebuild and screen-mode re-exec **blocked** when `!may_exec_relaunch_after_restore(restore_ok)`; fail-loud hints; no `exec`.
- Rebuild exec failure uses `print_rebuild_exec_failure_hint`.
- New `print_screen_mode_restore_blocked_hint` + unit test.

---

## Remaining risk (honest)

| Risk | Status |
|------|--------|
| Post-restore rebuild stderr flash | **Fixed** (unit-locked) |
| Re-exec after failed restore | **Fixed** (unit-locked, both relaunch paths) |
| Argv/env drift vs screen-mode | **Guarded** |
| Resume cold first frame / theme / size | **Untouched** — needs PTY e2e or operator dogfood |
| Multi-pane sibling still on old client binary | **By design** (reconnect only); not a same-PTY double paint |
| Live multi-TUI `/rebuild` paint under Zellij/tmux | **Still dogfood** — unit tests cannot prove full terminal paint |

The unit-testable vertical for this glitch is shipped. Operator one-shot two-pane `/rebuild` is the remaining visual confirmation, not a block on landing these guards.

---

## Files touched

- `crates/codegen/xai-grok-pager/src/app/dispatch/rebuild.rs`
- `crates/codegen/xai-grok-pager/src/app/mod.rs`
- `.agents/reports/impl-rebuild-tui-glitch-2026-08-07.md` (this file)

No git commit / stage (agent policy).
