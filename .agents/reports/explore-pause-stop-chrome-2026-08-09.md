# Explore: pause vs stop/cancel chrome (TUI)

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Scope:** map existing pause / stop / cancel controls; no product edits.

---

## Executive summary

Three **distinct** work-control concepts already exist, plus process-exit and per-task kill. On-screen chrome is **asymmetric**: mid-turn **cancel** has a clickable **`[stop]`** (red on hover). **Pause** and **soft stop** are **keyboard-only** (chords + toast), with **no** white pause/play button and **no** footer/toolbar CTA for them. Operator “I can’t find pause” matches the code: pause is real, but not painted as a button.

| Concept | Default chord | Cancels mid-turn? | Visible chrome |
|--------|---------------|-------------------|----------------|
| **Cancel turn** | Esc Esc, Ctrl+C (guards), mouse `[stop]` | Yes (focused session) | Yes: `[stop]` + Esc:cancel hint + subagent panel |
| **Fearless global pause** | `Ctrl+Shift+Space` | Yes (all sessions; then resume stash) | Toast only (“Paused Ns · …”) |
| **Soft stop** | `Ctrl+Shift+S` | No (finish turn, then hold queue) | Toast only (“Soft stop armed/held”) |
| **Quit / killall / SIGINT** | Ctrl+C escalate, `/exit`, SIGTERM | Process exit; may cancel + resume marker | Double-press quit confirm |
| **Per-task / subagent kill** | Tasks pane `[x]`, tool | Kills that task/subagent only | Tasks pane kill hits |

---

## 1. Cancel (stop generation, keep session)

### Keyboard

| Gesture | Effect | Key files |
|---------|--------|-----------|
| **2× Esc within ~800ms** (default UI; not fullscreen vim) | Arm then **CancelTurn**; draft **preserved** | `app/agent_view/prompt.rs:872-884`, `app/mod.rs:153-165` (`esc_cancels_turn`), `app/app_view.rs:521-576` (`PendingAction::ESC_DOUBLE_PRESS_TTL` = 800ms) |
| First Esc only | “press again to cancel” via shortcuts bar | `views/shortcuts_bar.rs:6-7` |
| Fullscreen **vim** mid-turn Esc | Swallowed; **does not** cancel | `app/mod.rs:163-165`, user-guide `03-keyboard-shortcuts.md:100-105` |
| **Ctrl+C** empty prompt + turn running | CancelTurn | `actions/defaults.rs:507-519`, `agent_view/mod.rs:20-27` |
| **Ctrl+C** non-empty draft + turn running | Clears draft **only**; turn keeps running | same; docs `03-keyboard-shortcuts.md:116` |
| Cancelling spinner + Esc | Re-sends cancel (retry lost ack) | `prompt.rs:863-870`, `dispatch/turn.rs:66-94` |
| Cancelling + Ctrl+C | Escalates toward **Quit** | `agent_view/mod.rs:22-23`, docs Escape table |

**Action registry:** `ActionId::CancelTurn` label `"cancel"`, default `Ctrl+C`, `When::AgentScreen`
`actions/defaults.rs:507-519`, `actions/mod.rs:74`.

### Mouse / chrome

| Control | Look | When | Semantics |
|---------|------|------|-----------|
| **`[stop]`** on turn status row | Gray at rest; **`theme.accent_error` (red) on hover** | `TurnRunning` / `CommandRunning` and mouse host | Fires `Action::CancelTurn` with `CancelTrigger::Mouse` |

- Layout comment: `views/turn_status.rs:1-14`, button paint `424-432`, `653-663`
- Hit test: `app/mouse.rs:136-140`
- Hidden when idle, already cancelling, keyboard-only (minimal), or plan-approval “parked” status (`turn_status.rs:371-383`)
- Sibling mid-turn control: **`[↓]` / `[send to bg]`** (demote execute to background), hover `accent_running` (`turn_status.rs:410-421`, `638-644`)

### Shortcuts bar

Running turn shows **Esc:cancel** (or Ctrl+C when Esc would not cancel) when `esc_would_cancel_turn`
`views/agent.rs:1024`, `1063`, `1178`, `1304`.

### Subagents still running (cancel panel)

If cancel preference unset and live subagents exist, cancel opens panel instead of immediately killing children:

- Title: “Subagents are still running. Stop them?”
  `views/modal.rs:858-899`
- Choices: Stop running / Continue to run / Always stop / Always continue
  `CancelTurnChoice` `modal.rs:130-150`
- **Esc on panel = dismiss only** (parent + subagents keep running)
  docs `03-keyboard-shortcuts.md:106`, FORK soft-interject note
- Dispatch: `dispatch/turn.rs:54-176`

### Cancel semantics (product)

- Stops **current top-level turn** generation; session stays open
- Optional cancel of subagents (pref `cancel_subagents_on_turn_cancel`)
- Interactive cancel writes **cancel-resume** marker for restart (`allow_local_rewind: true`)
  `dispatch/turn.rs:186-234`, `325-341`
- Soft interject is **never** cancel: “Cancel is Esc/stop only”
  FORK.md ~91, `actions/defaults.rs:686-713`, user-guide § during active turn

---

## 2. Fearless global pause (closest product “pause”)

| Item | Detail |
|------|--------|
| **Chord** | `Ctrl+Shift+Space` (`When::Always`) |
| **Action** | `ToggleGlobalPause` / label `"pause all"` |
| **Registry** | `actions/defaults.rs:521-537`, tests `actions/mod.rs:905-924` |
| **State** | `app/global_work_pause.rs` |
| **Dispatch** | `app/dispatch/global_pause.rs` |

### Behavior

1. **Engage:** snapshot every in-process session; **cancel running turns** on those with work (`do_cancel_turn_for(..., cancel_subagents: true, allow_local_rewind: false)`).
   `global_pause.rs:43-68`
2. **Hold** queue drain while active (`dispatch/queue.rs` checks global pause).
3. **Toast / status:** e.g. `Paused all work · N sessions held` then recurring
   `Paused Ns · N sessions held (Ctrl+Shift+Space to resume)`
   `global_work_pause.rs:218-238`, tick refresh `app_view.rs:5145-5153`
4. **Resume:** re-queue interrupted mid-turn prompt **once** + drain true pending; never re-spawn finished agents.
   `global_pause.rs:71-113`
5. **Does not** write durable cancel-resume marker (in-process stash only).
   FORK.md:289-294, `17-sessions.md` “Fearless global pause — in-process stash only”

### What pause is **not**

- Not a freeze-without-cancel of the model stream (it **cancels** turns then resumes by re-queue).
- Not soft stop.
- Not plan soft-park (plan approval UI).
- **No** on-screen pause/play button, **no** status-bar hit target for pause.

---

## 3. Soft stop (finish current turn, then hold queue)

| Item | Detail |
|------|--------|
| **Chord** | `Ctrl+Shift+S` (`When::Always`) |
| **Action** | `ToggleSoftStop` / label `"soft stop"` |
| **Code** | `app/soft_stop.rs`, `dispatch/soft_stop.rs` |
| **Phases** | Off → Armed → Holding (blocks automatic queue drain) |

- **Does not** cancel mid-flight.
  `soft_stop.rs:1-10`, FORK.md:295-299
- Status toast:
  - Armed: `"Soft stop armed (Ctrl+Shift+S)"`
  - Holding: `"Soft stop: queue held (Ctrl+Shift+S to resume)"`
  `soft_stop.rs:93-99`
- Tick shows soft-stop label only when global pause is **not** active
  `app_view.rs:5154-5157`
- **No** clickable chrome.

---

## 4. Process quit / SIGINT / killall

| Path | Behavior | Files |
|------|----------|--------|
| **SIGINT / SIGTERM / SIGHUP** (unix) | First signal → graceful quit notify (same family as `/exit`); second → hard exit after terminal restore | `app/signal_handler.rs:41-95`, `131-145` |
| **Ctrl+C** escalating from cancel / idle quit | PendingAction double-press **Quit** | `agent_view/input.rs` Ctrl+C arms, `app_view` PendingAction |
| **`killall` / SIGTERM mid-turn** | Eager + signal-path **cancel-resume marker** so session can resume on restart (not fearless pause) | FORK.md:300-318, `agent.rs` ~975, `dispatch/turn.rs:186-234` |
| **SIGKILL** | No userspace handler; no marker | FORK.md:306 |

Killall is **process exit**, not mid-turn cancel chrome. Resume is **session restart** (`canceled_turn_resume.json`), separate from global pause resume.

---

## 5. Subagent / task kill vs primary turn cancel

| Scope | How | Chrome |
|-------|-----|--------|
| **Primary turn cancel** | Esc Esc / Ctrl+C / `[stop]` → may prompt to also stop subagents | Turn status + cancel panel |
| **Global pause** | Cancels **all** sessions’ running turns with `cancel_subagents: true` | Toast only |
| **One subagent / bg task** | Tasks pane kill (`ballot_x` / `[x]`), or tool `kill_command_or_subagent` | `views/tasks_pane.rs` kill_button_rects ~802+, ~1540-1816; user-guide `16-subagents.md:334-341` |
| **Dashboard agent stop** | Dashboard “stop” action (confirm) | `actions/defaults.rs:1039-1305` |

Primary cancel and per-row kill are different actions; kill does not replace turn cancel.

---

## 6. Related but **not** work pause/stop

| Name | What it is |
|------|------------|
| **Plan soft-park** | After `exit_plan_mode`: plan approval strip/panel + CTAs; modal-free main thread. Not agent work freeze. FORK / user-guide `19-plan-mode` |
| **Permission / ask_user pause** | Agent waits on user; diamond pulse chrome; not operator pause control |
| **Goal “Paused” chips** | Goal harness display states (`agent_status.rs` GoalDisplayStatus::*Paused) |
| **Workflow block Paused** | Workflow timeline status (`scrollback/blocks/workflow.rs`) |
| **Voice stop** | Mic stop hit (`hit_voice_stop_button` in mouse.rs); not turn cancel |
| **Soft interject** | Steer mid-turn without cancel (Enter / interject chord) |

---

## 7. Theming (stop vs pause chrome)

| Token | Typical use for these controls |
|-------|--------------------------------|
| **`theme.accent_error`** | `[stop]` hover; “Cancelling…” label; kill hover in tasks | `turn_status.rs:653-685` |
| **`theme.accent_running`** | Agent activity spinner/tools; bg demote hover | `turn_status.rs:638-644` |
| **`theme.gray`** | `[stop]` at rest; timers | |
| **`theme.accent_user`** | Human green (caret, some panel titles) | cancel panel title uses this, not red stop |

There is **no** dedicated white/neutral “pause” glyph or play button in turn status or status bar today.

---

## 8. Current controls table (operator-facing)

| Control | Discoverable on screen? | Input | Semantic class |
|---------|-------------------------|-------|----------------|
| `[stop]` | Yes (mid-turn, mouse TUI) | Click | **Hard cancel** focused turn |
| Esc:cancel hint | Yes (shortcuts bar) | 2× Esc | Hard cancel (confirm window) |
| Ctrl+C | Cheatsheet / docs; not a painted stop button | 1–2 steps | Clear draft then cancel; or quit path |
| Ctrl+Shift+Space | **No button**; toast after use | Chord | **Global pause** (cancel-all + resume stash) |
| Ctrl+Shift+S | **No button**; toast | Chord | **Soft stop** (no mid-turn cancel) |
| Subagent cancel panel | Yes when armed | 1–4 / click | Cancel turn ± kill children |
| Tasks pane `[x]` | Yes when pane open | Click / keys | Kill one task/subagent |
| Plan soft-park CTAs | Yes when plan awaiting approval | Click / a A ? s q | Plan lifecycle, not run pause |
| SIGTERM / killall | Host shell | Signal | Process quit + optional resume marker |

---

## 9. Semantic gaps (plan-relevant)

1. **Pause exists but is not obvious**
   Product already ships “fearless global pause,” but chrome is toast-only. Matches operator report: no on-screen pause control.

2. **Label collision: `[stop]` means cancel, not soft stop**
   Soft stop is named “soft stop” in registry but has no button. Clickable stop is hard cancel. Adding a red stop + white pause must not make soft-stop and cancel look the same.

3. **Pause ≠ freeze**
   Global pause **cancels** running turns then re-queues. A media-player “pause” (hold stream, no cancel) does **not** exist. Plan must choose: surface existing cancel-and-stash pause, or invent true soft freeze (new product).

4. **No play/resume button**
   Resume is second press of the same chord + toast. No `[▶]` chrome.

5. **Scope mismatch**
   `[stop]` = focused session turn. Global pause = all sessions. Soft stop = process-level queue gate. A single toolbar may need scope labels.

6. **Double-Esc confirm vs one-click `[stop]`**
   Mouse cancel is immediate (unless subagent panel). Keyboard Esc is double-press. Any new pause button needs an explicit confirm policy.

7. **Minimal / keyboard-only**
   `[stop]` suppressed without mouse. Pause already works via Always chord; new chrome must not leave minimal users with only cancel.

8. **Cancel-resume durability**
   Esc/stop/quit write disk marker; global pause does not. UI that looks like “pause” but behaves like stop will surprise on restart.

---

## 10. Plan options (do not implement)

**A. Surface existing global pause (lowest product risk)**
- Add status-row or footer **pause** control (neutral/white) bound to `ToggleGlobalPause`.
- Keep **`[stop]`** red as `CancelTurn`.
- While paused, same control becomes **resume** (play).
- Docs already name the chords; button makes them discoverable.

**B. Soft stop as secondary “finish then stop”**
- Distinct from both red stop and pause (e.g. outline or menu entry).
- Avoid labeling soft stop as “stop” next to `[stop]`.

**C. True freeze-without-cancel (new)**
- Would need sampler/ACP support to suspend generation without cancel + resume semantics.
- Not present today; larger than chrome.

**D. Chrome-only clarity without new semantics**
- Footer hints: `Esc cancel · Ctrl+Shift+Space pause · Ctrl+Shift+S soft stop`.
- Does not fix “button not on screen” fully but cheap.

**E. Split local cancel vs global pause in toolbar**
- Red: cancel this turn. White: pause all sessions.
- Tooltip must say global pause cancels in-flight then resumes queued/stashed work.

**Recommendation for planning:** Prefer **A + clear copy** (pause = existing `ToggleGlobalPause`) unless operator explicitly wants freeze-without-cancel (**C**). Keep red for hard cancel only; never repaint soft stop as the only red control.

---

## 11. Key file index

| Area | Path |
|------|------|
| Turn status `[stop]` | `crates/codegen/xai-grok-pager/src/views/turn_status.rs` |
| Mouse cancel hit | `.../app/mouse.rs` |
| Esc / double-Esc policy | `.../app/agent_view/prompt.rs`, `.../app/mod.rs`, `.../app/app_view.rs` (PendingAction) |
| Cancel dispatch + subagent panel | `.../app/dispatch/turn.rs`, `.../views/modal.rs` |
| Global pause | `.../app/global_work_pause.rs`, `.../app/dispatch/global_pause.rs` |
| Soft stop | `.../app/soft_stop.rs`, `.../app/dispatch/soft_stop.rs` |
| Action defaults | `.../actions/defaults.rs`, `.../actions/mod.rs` |
| Signals / killall path | `.../app/signal_handler.rs`, FORK.md killall/resume notes |
| Tasks kill chrome | `.../views/tasks_pane.rs` |
| User docs | `.../docs/user-guide/03-keyboard-shortcuts.md`, `17-sessions.md`, `16-subagents.md` |
| Shipped product pins | `FORK.md` (Fearless global pause, Soft stop, Resume canceled turn) |

---

*Read-only explore. No product code changed.*
