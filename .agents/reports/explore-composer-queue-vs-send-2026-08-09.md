# Explore: composer Enter — queue vs send (2026-08-09)

Operator pain: not clear before typing whether Enter will **queue** or **submit immediately**.

Scope: read-only map of decision paths, state flags, keyboard, and existing cues. No product edits.

---

## 1. Code paths (anchors)

| Role | Path |
|------|------|
| Enter → `Action::SendPrompt` / empty-Enter interject / Interject chord | `crates/codegen/xai-grok-pager/src/app/agent_view/prompt.rs` ~546–697 |
| Soft-interject top queue row (empty Enter mid-turn) | `…/agent_view/queue.rs` `try_send_now_queued_from_prompt` ~50–72 |
| Soft-interject / idle force-drain a selected row | `…/agent_view/queue.rs` `force_interject_queue_row` ~505–574 |
| Background-subagent hold predicate | `…/agent_view/queue.rs` `holds_queue_for_background` ~303–305 (`watchers().subagents > 0`) |
| Held-count for status suffix | `…/agent_view/queue.rs` `held_queue_count` ~312–317 |
| Parked sendable-wait (Enter may cancel-and-send) | `…/agent_view/queue.rs` `is_parked_on_sendable_wait` ~85–91 |
| Send / enqueue / tip after queue | `…/app/dispatch/prompt.rs` `dispatch_send_prompt*` ~788–989 |
| Server “immediate” mid-turn path (still queue, not new concurrent turn) | same file + `immediate_server_send_eligible` in `…/dispatch/queue.rs` ~62–70 |
| Drain hold / force drain | `…/dispatch/queue.rs` `maybe_drain_queue_with` ~208–253; `dispatch_force_drain_queue` ~1085–1108 |
| Soft Interject effect | `…/dispatch/interject.rs` |
| Footer Enter label (`send` / `queue` / `interject`) | `…/views/agent.rs` `build_hints` ~1084–1102; wired from `…/agent_view/render.rs` `normal_pane_hints` ~407–428 |
| Status: `… still running · N queued — Interject to force` | `…/views/turn_status.rs` ~311–358, tests ~1648–1661 |
| Status: `· N queued — Enter to interject` (sendable wait) | `…/views/turn_status.rs` ~579–591 |
| Ephemeral tip after mid-turn queue | `…/tips/send_now.rs` (“Queued · Enter to interject”, max 3/session) |
| Status-bar queue chip | `…/agent_view/render.rs` ~1424–1442 (`+{queue_len}` when any pending) |
| Queue pane `[Interject]` | `…/views/queue_pane.rs`; shown when `turn_running \|\| holds_queue_for_background` (`render.rs` ~2125–2126) |
| Docs | user-guide `03-keyboard-shortcuts.md` § “During an active turn”; `16-subagents.md` § “Queue hold while subagents run” |
| Prior research | `doc/dev/research/queue-hold-background-subagents-2026-07-24.md` |

Registry: `ActionId::SendPrompt` default label is always `"send"` (`actions/defaults.rs` ~466–467). Footer **overrides** that label in `build_hints`.

---

## 2. Behavior table (state → Enter with non-empty text)

| State | Flags | Plain Enter (text) | Interject chord (Ctrl+Enter / Ctrl+L / Ctrl+O / …) |
|-------|--------|--------------------|-----------------------------------------------------|
| **Idle**, no live subagent hold | `!is_turn_running`, `subagents == 0` | **Send now** (enqueue + `maybe_drain_queue` → `SendPrompt`) | Toast: nothing to interject; use Enter |
| **Idle + live background subagent(s)** | `holds_queue_for_background()` | **Queue and hold** (does not start turn); drains when last holding child ends | Enqueue-front + `ForceDrainQueue` → start despite children; toast *Interject — starting despite background subagents* |
| **Turn running** (thinking / tools / responding; not empty-queue sendable wait) | `is_turn_running` | **Queue** (server-authoritative echo if eligible; else local pending). Does **not** cancel. | Soft **Interject** into current turn (`Action::Interject`) |
| **Turn parked on sendable wait**, **queue empty** | `is_parked_on_sendable_wait`, `!has_held_user_queue` | **Cancel-and-send** / shell send-now (`SendPromptNow` or armed immediate path) — intentional unblock | Soft Interject if turn still “running” path with text |
| **Turn parked on sendable wait**, **already has held queue** | parked + `has_held_user_queue` | **Append queue** (hold behind existing) | Soft Interject top / selected row (plain prompts only) |
| **Empty composer**, mid-turn, queue non-empty | `try_send()` None | **Soft-interject top** queued row (same as empty-Enter “double Enter”) | Same soft-interject top if nothing typed |
| **Empty composer**, idle + hold + queue | | No-op / no drain (still held) | `ForceDrainQueue` of top / selected |
| **Multiline mode** | | Bare Enter = newline (except empty mid-turn + queue → interject); **Shift+Enter / Alt+Enter** submit | Interject chord still works mid-turn |
| Images / local-only cases | not `immediate_server_send_eligible` or images present | Local enqueue + drain rules above | Interject carries drained images |

**Hold who counts:** standalone unfinished background subagents (`watchers().subagents`). Monitors, plain bg commands, loops alone do **not** hold. Workflow-owned children roll into workflow counts, not this hold.

**“Immediate server send”** (`immediate_server_send_eligible`): turn running (or shared queue non-empty), bound session, empty **local** pending, not editing queue. Still a **queue** on the agent shell (`pending_inputs`), not a second concurrent main turn. User-visible: appears in queue pane / `+N`.

---

## 3. Existing user-facing cues

| Cue | When | Says | Strength vs “will my next Enter queue?” |
|-----|------|------|----------------------------------------|
| Footer `Enter` label | Prompt focused, `can_send` | **`send`** if `! (is_turn_running && !renders_parked)`; **`queue`** if turn running and not “parked look”; **`interject`** if empty + mid-turn + has queue | **Best existing pre-type cue for mid-turn** — but **wrong or missing for idle+subagents** (see gaps) |
| Footer Interject chord | Mid-turn + non-empty composer only (`interjection_possible`) | `interject` + chord key | Does **not** show for idle+hold (force path still works) |
| Status still-running | Idle/parked + watchers | e.g. `1 subagent still running` | Visible, but **without** “Enter will queue” until something is already queued |
| Status held-queue suffix | Idle + hold + `held_queue > 0` | `· N queued — Interject to force` | Explains force, **after** first queue; not “Enter queues” while draft is empty |
| Status sendable-wait suffix | Parked sendable wait + held | `· N queued — Enter to interject` | Good for parked + held |
| Ephemeral tip | After mid-turn queue (seen ≤3/session, config `contextual_hints.send_now`) | `Queued · Enter to interject` | **After** queue, not before typing; can stop showing |
| Status-bar chip | Any pending count | `+N` (user green) | Queue depth only; no Enter semantics |
| Queue pane rows + `[Interject]` | Turn running **or** background hold | Row list; button when `show_interject` | After items exist |
| User-guide | Docs | Full rules for turn / wait / hold | Not in chrome |
| Focus table in shortcuts | Docs | “Enter → Send the current prompt” | **Oversimplifies** (always “send”) |

**Parked chrome interaction with footer:** `normal_pane_hints` passes
`is_turn_running = session.is_turn_running() && !renders_parked()`
(`render.rs` ~428). So a **parked** sendable wait (marker shown) re-labels Enter as **`send`**, matching empty-queue cancel-and-send. If the wait still has a held queue, parked marker is suppressed (`maybe_push_parked_marker` skips on `has_held_user_queue`), so footer can show **`queue`** again while status shows “Enter to interject.”

---

## 4. Keyboard summary

| Key | Role |
|-----|------|
| **Enter** | Submit path: send / queue / cancel-and-send (parked empty) / soft-interject top (empty + mid-turn queue). Multiline: newline unless empty+queue interject. |
| **Shift+Enter / Alt+Enter** | Submit in multiline; newline helpers when Shift unavailable |
| **Interject chord** | Soft inject mid-turn; force-drain idle+hold. Terminal family: Ctrl+Enter default; Apple Terminal Ctrl+O; VS Code family Ctrl+L |
| **Esc** | Cancel turn (double Esc default), clear draft, rewind — **not** queue vs send. Cancel panel Esc dismisses only. |
| **Ctrl+C** | Clear draft first mid-turn, then cancel — not the queue path |
| Queue pane **x / e / J K / y** + Interject | Manage rows; Interject soft mid-turn or force when held |

---

## 5. Gaps vs always-visible pre-type cue

1. **Idle + “1 subagent still running” (no items queued yet)**
   - Footer: **`Enter: send`**
   - Reality: **Enter queues and holds** until children finish (or Interject force).
   - Status does **not** say “Enter will queue.” Only after first queue: `· N queued — Interject to force`.
   - **Primary dogfood gap** for the reported pain.

2. **Footer uses only “turn running / parked look,” not `holds_queue_for_background()`**
   - Same binary drives cancel-button interject visibility. Force-interject works while footer still says **send**.

3. **Ephemeral tip is post-queue and seen-capped**
   - Never a stable “before I type” mode indicator.

4. **Mid-turn active generation without sendable wait**
   - Footer correctly says **queue** when text present.
   - No persistent status “N queued — Enter to interject” until parked/hold (`held_queue_count` is 0 unless wait or bg subagents). Rely on tip + `+N` chip.

5. **Parked empty wait**
   - Footer **send** matches fire-now; easy to confuse with idle send vs “unblock wait.”

6. **Docs / registry**
   - ActionDef still “send”; focus table says Enter sends always; product soft-interject vs historical “send-now” tip key names.

7. **No composer placeholder / mode pill** like “Will queue” vs “Will send now” tied to the same predicate as dispatch.

---

## 6. Plan-relevant options (do not implement here)

Ordered by fit to “always-visible cue before typing”:

| Option | Idea | Pros | Cons |
|--------|------|------|------|
| **A. Footer label truth** | Drive `submit_label` from real outcome: idle+hold → `queue` (or `queue (hold)`); parked empty → `send now` / `unblock`; mid-turn → `queue`; idle clean → `send`. Optionally show Interject chord whenever `holds_queue_for_background()`. | Small surface; already where users look for Enter | Label length; need tests next to `prompt_running_submit_hint_is_queue_and_interject` |
| **B. Status line always** | When hold active **even if held_queue == 0**: e.g. `1 subagent still running · Enter queues` (and keep `Interject to force` when `N > 0`) | Matches operator’s still-running cue | Narrow terminals; copy design |
| **C. Composer mode chip** | Left/right of prompt: `SEND` / `QUEUE` / `HOLD` / `INTERJECT` using one pure function of state | Always next to caret; survives footer compact truncation | New chrome; theming |
| **D. Persistent status (not tip)** | Replace or backstop send_now tip with non-ephemeral line while turn running or hold | Survives tip cap | Visual noise mid-tool |
| **E. Docs-only** | Expand focus table + tip | Cheap | Does not fix TUI ambiguity |

**Recommended plan vertical (if product wants this):**
**A + B** together: one shared predicate (mirror of dispatch: running / parked empty / hold / clean idle) feeding footer **and** still-running status suffix, with TDD from existing `build_hints` + `turn_status` tests. Optional later **C** if compact footers hide the label.

**Acceptance sketch:** with parent idle and one live background subagent, empty draft, before any Enter: operator can see without guessing that Enter will **queue/hold**, not start a turn; Interject (or force) is advertised as the fire path. After queue, existing force suffix may remain.

---

## 7. One-line decision tree (implementer cheat sheet)

```
Enter (non-empty text, normal mode):
  if turn running:
    if parked_sendable_wait && !held_queue → cancel-and-send (SendPromptNow / armed)
    else → queue (server or local)
  else if holds_queue_for_background → local enqueue; drain blocked until children done
  else → enqueue + drain → send now

Empty Enter mid-turn + visible queue → soft-interject top
Interject chord → soft mid-turn | force drain idle+hold
```

---

*Report only. No product code changed.*
