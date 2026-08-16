# Restore status-row [pause] / [resume] chips

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Agent:** L2 implementer (`spawn_subagent` not in this session; no L3)  
**Date:** 2026-08-13  
**Diagnosis:** `.agents/reports/fork-gaps-remaining-seams-2026-08-13.md` (Fearless status chips: paint dropped, dispatch present). Not re-litigated.

SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

## Named contract

Fearless Work B:

1. Turn-status row paints clickable **`[pause]`** when the primary turn is live or background subagents run.
2. Same row paints clickable **`[resume]`** when process-level global pause is active (including idle, so resume stays discoverable).
3. Click on that chip dispatches `Action::ToggleGlobalPause`, never `CancelTurn`.
4. Soft stop stays chord-only (no button). Hard **`[stop]`** already on this row; do not invent a second stop.
5. Pause hover is quiet white (`text_primary`). Stop hover is `accent_error`. Keyboard-only hosts paint neither chip.

Chips live on the **turn-status row** (`views/turn_status.rs`), not the footer credits bar and not `views/agent.rs`. Verified against Surmount `origin/main` and current paint.

## TDD (red → green)

Tests written first. First cargo against `/tmp/grok-oss-pause-chips-target` hit **ENOSPC** (tmpfs ~90%). Retry moved to `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-pause-chips-target`.

**Red (product paint already landed; hover contract still failed under `NO_COLOR`):**

```
CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-pause-chips-target
cargo --offline test -p xai-grok-pager --lib -- \
  work_control_chrome_matrix \
  mid_turn_paints_pause_and_stop_with_distinct_hover_colors \
  idle_with_subagents_paints_pause_and_stop_hits \
  idle_with_monitors_only_does_not_paint_pause_or_stop \
  global_paused_idle_paints_resume_not_stop \
  keyboard_only_suppresses_pause_and_stop_hits \
  pause_button_click_dispatches_global_pause_not_cancel \
  cancelling_keeps_stop_button_clickable
```

- `mid_turn_paints_pause_and_stop_with_distinct_hover_colors`  
  panicked: `sanity: pause and stop hover tokens must differ`  
  `left: Reset` `right: Reset`  
  (`Theme::current()` with no TrueColor pin collapses both tokens.)
- Other seven named tests already green on that run (paint + click).

**Green (same filters after `pin_theme()` + Doge in the hover test):**

```
# same command as above
# 8 passed; 0 failed
```

| Test | Result |
|------|--------|
| `work_control_chrome_matrix_pause_not_cancel_stop_not_pause` | ok |
| `mid_turn_paints_pause_and_stop_with_distinct_hover_colors` | ok |
| `idle_with_subagents_paints_pause_and_stop_hits` | ok |
| `idle_with_monitors_only_does_not_paint_pause_or_stop` | ok |
| `global_paused_idle_paints_resume_not_stop` | ok |
| `keyboard_only_suppresses_pause_and_stop_hits` | ok |
| `cancelling_keeps_stop_button_clickable` | ok |
| `pause_button_click_dispatches_global_pause_not_cancel` | ok |

## Product edits (smallest)

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/turn_status.rs` | `WorkControlChrome`, `work_control_chrome`, `[pause]`/`[resume]`/`[stop]` paint, hover colors, `TurnStatusOutput.pause_button`, `MouseButtons.pause_hovered`, `TurnStatusArgs.global_paused`, idle "Paused all work" + `[resume]`, `should_show_with_global_pause`, named paint tests. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | `AppRenderParams.global_paused`; snapshot `global_work_paused`; height via `should_show_with_global_pause`; pass hover + pause rect; click test next to credits tests. |
| `crates/codegen/xai-grok-pager/src/app/mouse.rs` | Pause hit (before cancel) → `ToggleGlobalPause`. Hover update. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | `hit_pause_button`, `global_work_paused`. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` | Field init. |
| `crates/codegen/xai-grok-pager/src/app/app_view.rs` | Pass `global_work_pause.is_active()` on main draw and dashboard overlay. |
| `crates/codegen/xai-grok-pager-minimal/src/live.rs` | `global_paused: false` so `AppRenderParams` still compiles. |

Dispatch (`ToggleGlobalPause`, `global_work_pause`, `dispatch/tests/global_pause.rs`) was already present. Not rewritten.

## Post-impl verify

```
cargo fmt -p xai-grok-pager
# FMT_EXIT:0

CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-pause-chips-target
cargo --offline clippy -p xai-grok-pager --lib -- -D warnings
# CLIPPY_EXIT:0

# named tests: 8 passed (command above)
```

Parent asked for `/tmp/grok-oss-pause-chips-target`. That path ENOSPC'd on the first compile. Later runs used `/home/hunter/.cache/grok-oss-pause-chips-target`.

## Leftovers

- **Live TUI is old until a successful rebuild** and a full quit/reopen.
- No footer / shortcuts-bar pause hint. `ActionId` has no `ToggleGlobalPause`; parent forbade `actions.rs` F9 bind (mop owns leftover).
- Did not touch `settings/**`, `settings_writes.rs`, `views/prompt_widget/**`, token economy / spend, or welcome chrome.
- `views/agent.rs` was not the chip home; left alone.
- Catalog cheat sheet should list the eight named tests. Not edited here.

## Paths

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/turn_status.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/mouse.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/session.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/app_view.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager-minimal/src/live.rs`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/fork-gaps-remaining-seams-2026-08-13.md`
