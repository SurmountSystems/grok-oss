# Implement Work B — Pause (white) vs Stop (red) discoverability

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Scope:** Work B only (not A/C/E). Explore map:
`.agents/reports/explore-pause-stop-chrome-2026-08-09.md`.

## Goal

Make global pause and hard cancel discoverable as **distinct** on-screen
controls: quiet white pause/resume vs red stop. Soft stop stays keyboard-only.

## Named contracts (TDD)

| Contract | Proof |
|----------|--------|
| Hit targets when work is live (primary turn **or** subagents) | `work_control_chrome_matrix_*`, `mid_turn_paints_pause_and_stop_*`, `idle_with_subagents_paints_pause_and_stop_hits` |
| Pause is not CancelTurn; stop is CancelTurn | `pause_button_click_dispatches_global_pause_not_cancel` (mouse), pure `work_control_chrome` matrix |
| Stop hover uses `accent_error`; pause hover uses `text_primary` | `mid_turn_paints_pause_and_stop_with_distinct_hover_colors` |
| Idle + subagents stop path (panel / kill, no parent CancelTurn) | `cancel_turn_idle_with_running_subagents_shows_panel`, `cancel_turn_choice_idle_with_subagents_kills_without_parent_cancel` |
| Global pause idle keeps `[resume]` discoverable | `global_paused_idle_paints_resume_not_stop` + `should_show(..., global_paused)` |
| Work A Enter cues not regressed | `idle_with_subagents_empty_queue_shows_enter_queues_cue`, `idle_with_subagents_and_held_queue_shows_force_hint`, `views::agent::tests` (59 ok) |

### Red → green

1. Added pure `work_control_chrome` + render/dispatch tests first (would fail
   without product chrome and idle-subagent cancel path).
2. Product: status-row buttons, mouse hits, cancel dispatch, shortcuts, docs.
3. Same filters green.

## Product changes

### Status row (`views/turn_status.rs`)

- **`[pause]` / `[resume]`**: quiet gray at rest, `text_primary` on hover;
  binds to process-level global pause (label flips while paused).
- **`[stop]`**: gray at rest, `accent_error` on hover; hard cancel only.
- Show pause when primary turn is live, subagents > 0, **or** global pause
  active (resume stays discoverable after cancel-all).
- Show stop when primary turn is live **or** subagents > 0 (not monitors-only).
- Idle + subagents still-running row keeps Work A `Enter queues` / force
  suffixes and paints both buttons on the right.
- Soft stop: **no** button.
- Parked wait chrome still suppresses pause/stop (unchanged contract).

### Mouse / agent view

- `hit_pause_button` → `Action::ToggleGlobalPause` (never CancelTurn).
- `hit_cancel_button` → `Action::CancelTurn` (unchanged).
- `AppRenderParams.global_paused` + `AgentView.global_work_paused` wire pause
  state into status row and footer resume hint.

### Cancel when primary idle + subagents (`dispatch/turn.rs`)

- `CancelTurn` with live standalone subagents and no running turn:
  - pref **ask** → open existing “Subagents are still running” panel;
  - **always_stop** → kill those subagents;
  - **always_continue** → leave them alone.
- Panel **Stop** choices kill children **without** inventing a parent
  `CancelTurn` effect. Preference defaults are not silently rewritten except
  via Always Stop / Always Continue (existing path).

### Shortcuts bar + action help

- Footer shows **pause** while turn running or background subagents hold;
  **resume** while global pause is active.
- `CancelTurn` / `ToggleGlobalPause` long_help document button + chord split.

### User-guide

- `03-keyboard-shortcuts.md`: complete-sentence tables for hard cancel,
  global pause, soft stop; new **Pause vs stop** subsection under active turn.
- `17-sessions.md`: pause button and soft-stop chord-only note.

## Files touched

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/views/turn_status.rs` | Chrome helper, paint, tests |
| `crates/codegen/xai-grok-pager/src/app/mouse.rs` | Pause hit + hover |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | `hit_pause_button`, `global_work_paused` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` | Init fields |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Wire draw + footer resume |
| `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs` | Ctrl+C cancel with live subagents when idle |
| `crates/codegen/xai-grok-pager/src/app/agent_view/links.rs` | Mouse pause ≠ cancel test |
| `crates/codegen/xai-grok-pager/src/app/app_view.rs` | Pass `global_paused` into draw |
| `crates/codegen/xai-grok-pager/src/app/dispatch/turn.rs` | Idle + subagents cancel path |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/turn.rs` | Idle cancel TDD |
| `crates/codegen/xai-grok-pager/src/views/agent.rs` | Shortcuts pause hint |
| `crates/codegen/xai-grok-pager/src/actions/defaults.rs` | Long help copy |
| `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md` | Operator docs |
| `crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md` | Pause / soft-stop notes |

## Commands

```text
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # exit 0
cargo test -p xai-grok-pager --lib -- views::turn_status::tests   # 59 ok
cargo test -p xai-grok-pager --lib -- views::agent::tests         # 59 ok
cargo test -p xai-grok-pager --lib -- dispatch::tests::turn       # 70 ok
cargo test -p xai-grok-pager --lib -- pause_button_click_dispatches_global_pause_not_cancel cancel_turn_idle  # ok
```

## Not in this slice

- Soft-stop button (explicitly v1 chord-only).
- True freeze-without-cancel of a model stream (product still cancel-and-stash).
- Changing `cancel_subagents_on_turn_cancel` defaults / killall preferences.
- Work A Enter cues or Work C `intent ·` / `Team settlement:` chrome (left alone).
