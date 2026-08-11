# Rebuild must not re-fire a completed last prompt (2026-08-08)

## Operator symptom

After `/rebuild` (install + self re-exec into the same session), the product
auto-sent a **previous user prompt** that had already finished cleanly
(dogfood: `??? [Image #1]`, earlier `Still nothing!!! [Image #1]`). Same class
as history-recovery false positives, but the live loop was driven by a
**stale `canceled_turn_resume.json`** surviving idle rebuild relaunch.

## Live forensics (session `019faf9d-…`)

| Time (UTC) | Evidence |
|------------|----------|
| 11:10:27 | `prompt.drain` len 171 (user turn starts; eager marker write) |
| 11:10:40 | `turn_ended` **completed** |
| 11:11:09 | `session.load.start` (rebuild relaunch) |
| 11:11:10 | Immediate `prompt.drain` len **171** (false re-fire of completed turn) |
| 11:13–11:14 | More rebuild loads re-fire len 14 (`??? [Image #1]`) |
| Disk | `canceled_turn_resume.json` still present with that prompt text |

So: clean completed turn + leftover marker → every reopen/`/rebuild` re-queued
the last prompt via the **marker path** (not only open-turn history recovery).

### Why the marker stuck

1. **Eager write at turn start** always leaves `canceled_turn_resume.json`.
2. Clear on success is skipped when **live background subagents** still hold
   work (`finalize_cancel_resume_after_successful_turn` keeps the marker).
3. When the **last** child finished, nothing cleared that kept marker.
4. **Error / rate-limit** terminals also left the eager marker (flat-poll block
   loop re-fired `??? [Image #1]` on every rebuild).
5. Load always applied a present marker, with **no** stale-after-completed gate.

History-recovery false-positive guards (`last_primary_user_turn_completed_in_replay`)
were necessary but **not sufficient** while a stale marker still won path A.

## Fix

### 1. Stale-marker gate on session load (rebuild relaunch uses same path)

In `handle_session_loaded`, marker auto-resume is **refused** when:

- `last_primary_user_turn_completed_in_replay` is true (primary finished with a
  non-`cancelled` stop reason in this load's replay), **and**
- `session_looks_interrupted_mid_work` is false (no unfinished children,
  running scrollback, tracker mid-turn, or open turn).

Then: clear the file, stay idle, no SendPrompt.

Still **resume** when:

- Marker + mid-work evidence (e.g. parent completed, children still live, killall)
- Marker + primary **not** marked completed (Esc cancel / mid-turn rebuild cancel;
  `cancelled` stop reason does not set the completed flag)
- No marker + true interruption → history recovery (unchanged)

### 2. Completed flag ignores `cancelled`

`TurnCompleted` during load replay sets
`last_primary_user_turn_completed_in_replay` only when
`stop_reason != "cancelled"`. That keeps Esc cancel-resume markers valid while
allowing the stale gate for success/error/end_turn.

### 3. Clear leaks

| Path | Action |
|------|--------|
| Last background subagent finishes, parent idle, no hold | clear marker |
| Error / rate-limit terminal (not user-cancelling, no hold) | clear marker |
| Failed `PromptResponse` (same conditions) | clear marker |
| Clean success (existing) | clear unless live children (keep) |

### 4. Mid-turn rebuild continue

Unchanged: if turn is running, `/rebuild` still cancels with cancel-resume so
relaunch re-queues the interrupted work.

## Tests (red → green)

| Test | Contract |
|------|----------|
| `session_loaded_stale_marker_after_completed_primary_does_not_resume` | Stale marker + completed primary + no mid-work → no SendPrompt, marker cleared |
| `session_loaded_cancel_marker_without_completed_primary_still_resumes` | Esc-style marker (flag false) still SendPrompt |
| `session_loaded_marker_with_unfinished_child_resumes_despite_completed_primary_flag` | Parent completed flag + live child + marker still resumes |
| Existing `session_loaded_*` resume suite | Still green (30 passed) |

## Code

| Path | Change |
|------|--------|
| `…/dispatch/session/load.rs` | Stale-marker gate on path A |
| `…/acp_handler/session_notification.rs` | Non-cancelled completed flag; clear on last child finish |
| `…/dispatch/turn.rs` | `clear_cancel_resume_marker_for_session`; clear on error terminal |
| `…/dispatch/prompt.rs` | Clear on failed PromptResponse |
| `…/agent_view/mod.rs` | Field docs |
| `…/canceled_turn_resume.rs` | Module docs |
| `…/dispatch/tests/turn.rs` | Three new contracts |

## Verify

```text
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
cargo clippy -p xai-grok-shell --lib -- -D warnings   # clean
cargo test -p xai-grok-pager --lib session_loaded_    # 30 passed
just install
```

## Operator dogfood

1. Fully quit any `grok-oss` still on an old binary (`/proc/…/exe` → deleted).
2. Confirm new install under `~/.cargo/bin/grok-oss`.
3. Idle session whose last turn finished cleanly: `/rebuild` → relaunch must
   show **idle composer**, **no** auto SendPrompt of the last user message.
4. Mid-turn `/rebuild` (or reopen after killall mid-work) must still
   auto-continue interrupted work (marker or history recovery).
5. Esc cancel then reopen (no completed primary in replay for that turn) must
   still "Resuming canceled turn...".

## Not done

- git commit (forbidden)
- Removing eager write at turn start (still required for killall races)
