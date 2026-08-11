# Pause / stop verify (Work B honesty)

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Prior claims:** board `feat:pause-and-red-stop` / `impl:work-b-pause-stop`; report
`.agents/reports/impl-work-b-pause-stop-chrome-2026-08-09.md`
**Explore map:** `.agents/reports/explore-pause-stop-chrome-2026-08-09.md` (pre-Work-B; pause was toast-only then)

## Verdict

**Work B landed in tree and is not label-only chrome.** Paint, hit targets, and action dispatch are wired and covered by unit tests that pass on this checkout. No product code change was required in this verify pass.

If dogfood still “has no way to stop,” the usual cause is an **old binary** (rebuild/install needed), not missing source. Soft stop remains chord-only by design.

---

## What was claimed

Work B report (2026-08-09):

| Claim | Intent |
|-------|--------|
| Status-row **`[pause]` / `[resume]`** (quiet white hover) | Process-level global pause (`ToggleGlobalPause`) |
| Status-row **`[stop]`** (red hover) | Hard cancel (`CancelTurn`) only |
| Soft stop | Keyboard only (`Ctrl+Shift+S`); no status-row button |
| Idle + live subagents | Both buttons; stop can open subagents panel / kill without inventing a parent cancel |
| Global pause idle | Keep **`[resume]`** discoverable |
| Keyboard-only host | Suppress painted hits (`buttons: None`) |

---

## What is in code (verified)

### Chrome and matrix

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/views/turn_status.rs` | `work_control_chrome`, paint `[pause]`/`[resume]`/`[stop]`, hover colors, idle-subagent and global-pause-idle rows, tests |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Passes `buttons: Some(MouseButtons{...})`, `global_paused`, sets `hit_pause_button` / `hit_cancel_button`; footer **resume** when global pause holds idle sessions |
| `crates/codegen/xai-grok-pager/src/app/app_view.rs` | `global_paused: self.global_work_pause.is_active()` into draw |

`work_control_chrome(show_buttons, turn_running, subagents, global_paused)`:

- Pause when turn live **or** subagents > 0 **or** global pause active.
- Stop when turn live **or** subagents > 0 (not monitors-only).
- `pause_is_resume` when global pause active.
- Soft stop: no button.

### Mouse dispatch (not dead labels)

| Hit | Action | File |
|-----|--------|------|
| `hit_pause_button` | `Action::ToggleGlobalPause` | `app/mouse.rs` (~136–140) |
| `hit_cancel_button` | `Action::CancelTurn` (+ mouse trigger hint) | same (~142–146) |

Regression: `pause_button_click_dispatches_global_pause_not_cancel` in `app/agent_view/links.rs`.

### Keyboard / process actions

| Control | Default chord | Dispatch |
|---------|---------------|----------|
| Hard stop / cancel | **2× Esc** (~800ms), empty-prompt **Ctrl+C**, palette “cancel” | `dispatch/turn.rs` → `do_cancel_turn` / idle subagent panel |
| Global pause / resume | **Ctrl+Shift+Space** (always) | `dispatch/global_pause.rs` (cancel all running turns + stash; resume re-queues interrupted prompts once) |
| Soft stop | **Ctrl+Shift+S** (always) | `dispatch/soft_stop.rs` (finish current turn, then hold queue) |

Action help: `actions/defaults.rs` long_help for `CancelTurn` and `ToggleGlobalPause` documents button + chord split.

### Footer shortcuts bar

- Mid-turn (or background subagents holding): **cancel** + **pause** hints (`views/agent.rs`).
- Global pause with sessions idle: **resume** hint still injected (`agent_view/render.rs` shortcuts path).

### Docs

- `docs/user-guide/03-keyboard-shortcuts.md`: Escape table, agent-level chords, **Pause vs stop** subsection.
- `docs/user-guide/17-sessions.md`: pause button + soft-stop chord-only note.

---

## How the operator uses it (after rebuild)

Rebuild/install the pager from this tree first so you are not on pre-Work-B chrome.

### While a turn is running (full TUI with mouse)

1. **Hard stop now:** click **`[stop]`** on the turn-status row (right side; red on hover), or press **Esc twice** within ~800ms (draft kept), or **Ctrl+C** on an empty prompt (non-empty draft clears first, turn keeps running).
2. **Pause all work in this process:** click **`[pause]`** (quiet white on hover) or **Ctrl+Shift+Space**. Turns cancel; queues hold; toast shows paused state. Same control becomes **`[resume]`**.
3. **Soft stop (let this turn finish, then freeze queue):** **Ctrl+Shift+S** only. No status-row button. Toast: armed vs queue held.

Status row shape (conceptual):

```text
⠧ Run command 0.2s         1m20s ⇣12k [pause] [stop]
```

Footer also shows **Esc:cancel** (or Ctrl+C) and **pause** while work is live.

### Idle parent, subagents still running

- Row shows still-running cue plus **`[pause]`** and **`[stop]`**.
- **`[stop]` / cancel:** ask preference opens “Subagents are still running”; Always stop kills children without inventing a parent cancel.
- Monitors/loops alone do **not** paint pause/stop (subagents do).

### After global pause (all sessions idle)

- Row: `Paused all work` + **`[resume]`** (no stop).
- Footer: **resume** chord hint.
- Click **`[resume]`** or **Ctrl+Shift+Space** again.

### Minimal mode

- Terminal mouse capture is off (`mouse_capture = !minimal`). Prefer chords: Esc Esc / Ctrl+C / Ctrl+Shift+Space / Ctrl+Shift+S.
- Fullscreen agent draw still may paint status-row labels; clicks are not the reliable path without mouse capture. Keyboard is the real control surface.

### What pause is not

- Not a media-player freeze of one model stream (product cancels in-flight, then resume re-queues unfinished mid-turn prompts).
- Not soft stop.
- Not process quit / killall (those are exit + optional cancel-resume marker on restart).

---

## Gaps fixed this pass

**None.** Code and tests already match Work B contracts. No red→green product edit required.

---

## Residual (honest, not Work B regressions)

| Gap | Status |
|-----|--------|
| Soft stop has no status-row button | Intentional v1 (chord + toast only) |
| Global pause cancels then resumes (not true stream freeze) | Product design; docs say so |
| Minimal mode: mouse off; rely on chords | Known mode split |
| Soft stop / pause chords are easy to miss without reading shortcuts | Footer shows **pause** when work live; soft stop still toast/chord only |
| True “kill all subagents and parent in one always-visible red control while plan-parked” | Parked wait still suppresses pause/stop so chrome does not lie |
| Operator binary may predate Work B | Rebuild required for dogfood |

Not in scope here: changing `cancel_subagents_on_turn_cancel` defaults, killall resume, soft-stop button, true freeze-without-cancel.

---

## Test proof (this verify)

```text
cargo test -p xai-grok-pager --lib -- \
  views::turn_status::tests::work_control_chrome \
  views::turn_status::tests::mid_turn_paints \
  views::turn_status::tests::idle_with_subagents_paints \
  views::turn_status::tests::idle_with_monitors \
  views::turn_status::tests::global_paused_idle \
  views::turn_status::tests::keyboard_only_suppresses \
  pause_button_click_dispatches \
  cancel_turn_idle
# 12 passed

cargo test -p xai-grok-pager --lib -- \
  dispatch::tests::global_pause \
  dispatch::tests::soft_stop \
  global_work_pause
# 23 passed

cargo test -p xai-grok-pager --lib -- views::agent::tests
# 59 passed
```

Named contracts covered:

- Pause ≠ CancelTurn; stop = CancelTurn (matrix + mouse dispatch test).
- Distinct hover tokens (pause `text_primary`, stop `accent_error`).
- Idle + subagents paints both; monitors-only paints neither.
- Global pause idle paints resume, not stop.
- Keyboard-only (`buttons: None`) suppresses painted hits.
- Global pause engage/resume lifecycle and soft-stop queue hold.

---

## Dogfood checklist (one screen)

1. Rebuild/install from this tree.
2. Start a turn that streams or runs a tool.
3. Confirm status row shows **`[pause]`** and **`[stop]`**.
4. Hover stop → red; hover pause → quiet white.
5. Click **stop** → turn cancels.
6. New turn → **Ctrl+Shift+Space** or **`[pause]`** → toast + work holds; **`[resume]`** continues unfinished work only.
7. Soft stop: **Ctrl+Shift+S** mid-turn → turn finishes → queue held until toggle again.
8. Idle + live subagent → **`[stop]`** opens panel or kills per preference.

If step 3 fails on a rebuild from this tree, that is a new bug; open `bug:` with terminal size, full vs minimal, and a screenshot. Pre-rebuild absence of buttons is expected for old builds.
