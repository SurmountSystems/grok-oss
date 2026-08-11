# Join: fearless global pause + robust interrupt/restart

**Date:** 2026-08-03
**Scope:** `xai-grok-pager` (no shell package edits; did not touch `token_economy/*`)

## Product

- **Chord:** `Ctrl+Shift+Space` (`ActionId::ToggleGlobalPause`, `When::Always`).
  - Spacebar combo that does not steal bare Space (prompt focus / typing) or
    voice `Ctrl+Space`.
  - Documented as the open product default; lives in the action registry for
    future remapping.
- **Global:** toggles pause across **every** agent session in this pager
  process (`app.agents`), not only the focused one.
- **Engage:** cancels running turns (subagents stopped with the turn), holds
  local queue drain, toasts held session count.
- **While paused:** toast refreshed on tick with duration + sessions held.
- **Resume:** re-queues interrupted mid-turn prompts **once** (front of local
  queue), then drains true pending work. Idle/finished sessions get no invent
  work. Empty resume is a no-op toast.
- **Waiting vs finished:** pure lifecycle enum never treats finished as
  waiting; resume does not re-spawn agents.

## Files

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/app/global_work_pause.rs` | Pure state machine + unit tests |
| `crates/codegen/xai-grok-pager/src/app/dispatch/global_pause.rs` | Engage / resume dispatch |
| `crates/codegen/xai-grok-pager/src/app/dispatch/turn.rs` | `do_cancel_turn_for` + rewind opt-out for pause |
| `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` | Drain blocked while paused |
| `crates/codegen/xai-grok-pager/src/actions/{mod,defaults}.rs` | ActionId + binding |
| `crates/codegen/xai-grok-pager/src/app/app_view.rs` | Field + Always key route + tick toast |
| `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md` | User-facing docs |
| `FORK.md` | Product bullet |

## Tests (all green)

```bash
cargo test -p xai-grok-pager --lib -- global_work_pause global_pause global_pause_bound_to_ctrl_shift_space
# 18 passed
```

Contracts covered: mid-turn pause → resume continues once; resume with nothing
pending does nothing; finished agent not re-spawned; multi-session cancel +
count/duration; drain held while paused; keybinding not colliding with voice.

## Out of scope / residual

- Cross-process pause of other grok clients attached to the same leader is
  not broadcast yet (each process holds its own gate). In-process multi-session
  is covered.
- Key remapping UX is still "built-in" (same as other chords); registry is the
  hook when remapping lands.
- Minimal incidental fix: trailing comma syntax error in
  `agent_view/render.rs` that blocked `cargo fmt` for the package.

## Verify commands used

```bash
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib -- global_work_pause global_pause global_pause_bound_to_ctrl_shift_space
```

No `just check` / full workspace clippy (per task). No git add/commit.
