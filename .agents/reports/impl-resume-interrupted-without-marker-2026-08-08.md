# Resume interrupted turn without cancel-resume marker (2026-08-08)

## Goal

On session load / resume, auto-restart interrupted mid-work even when
`canceled_turn_resume.json` was never written (old binary, parent success
clear with live children race, killall before eager arm). Operator reopen of
the hot iso session must restart the last implement prompt without hand-planting
a marker.

## Product contract shipped

`handle_session_loaded` now:

1. **A) Marker path** (unchanged priority): if `canceled_turn_resume.json` is
   present and `[ui] resume_canceled_turn_on_restart` is on, re-queue marker
   prompt, toast **"Resuming canceled turn..."**, force-drain / SendPrompt.
2. **B) History recovery** (new): if no marker, but the loaded session looks
   interrupted mid-work, re-queue the last user prompt from scrollback, write
   then clear a one-shot marker for SIGTERM consistency, toast **"Resuming
   interrupted turn..."**, force-drain / SendPrompt.

### Interruption evidence (any of)

- Unfinished subagent records (`finished == false` after replay)
- Parent scrollback still has running entries
- Child scrollback still has running entries

Captured **before** `finish_turn` / zombie finalize clear that state.

### Prompt source

Last non-empty `UserPrompt` block (full text), skipping bash / cron /
interjection. Same send path as re-queue.

### Safety

- Clean completed turn (no unfinished children, no running scrollback) → **no**
  auto SendPrompt even if last history is `/implement …`.
- Marker always wins over scrollback when present.
- One-shot: clear after enqueue; rewarm if drain does not start; successful
  drain re-eager-writes for the new active turn (existing behavior).

## Code

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | `session_looks_interrupted_mid_work`, `last_resumable_user_prompt_text`, `recover_interrupted_turn_from_session`; wire A then B in `handle_session_loaded` |
| `crates/codegen/xai-grok-shell/src/session/canceled_turn_resume.rs` | `auto_resume_interrupted_toast()`; module docs for history recovery |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/turn.rs` | Three new contracts |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/session/load.rs` | Clear host leftover marker in fork first-prompt drain test |

## TDD

Red contracts landed then greened:

1. `session_loaded_recovers_interrupted_turn_without_marker` — no marker,
   unfinished subagent, last user `"implement foo"` → SendPrompt + interrupted
   toast + turn running.
2. `session_loaded_clean_completed_does_not_auto_resume_without_marker` — no
   marker, clean history → no SendPrompt.
3. `session_loaded_marker_wins_over_history_recovery` — marker text wins over
   scrollback; canceled toast (not interrupted).

Existing marker + zombie tests still green. Full filter
`session_loaded_`: **23 passed**.

## Verify

- `cargo fmt -p xai-grok-pager -p xai-grok-shell`
- `cargo clippy -p xai-grok-pager --lib -- -D warnings` (clean)
- `cargo clippy -p xai-grok-shell --lib -- -D warnings` (clean)
- `cargo test -p xai-grok-pager --lib session_loaded_`
- `just install` → `grok-oss 0.2.111 (c87f66a61d94) [stable]`

## Live iso session (read-only)

- Dir: `~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fai%2Fiso/019f85f6-3971-7363-a8b6-833ed66829c0`
- **No** `canceled_turn_resume.json` on disk (matches dogfood)
- Last user prompt length **6887**; implement line first 80 chars:
  `'/implement --effort 2 all remaining residual tasks in priority order according t'`
- Subagent dirs with no `meta.json`: **12** (orphan stubs)
- Events tail was mid-turn (`waiting_for_model` after tools); updates tail had
  incomplete / pending tool work and a cancelled subagent finish

**No hand-planted marker.** Product history recovery is the reopen path.
Reopen that session with the installed binary: expect toast
"Resuming interrupted turn..." and auto-start of the last implement prompt when
replay leaves unfinished children or running scrollback entries.

## Logging

- Marker: `canceled_turn_resume: applying marker on session load (auto-start)`
- History: `canceled_turn_resume: recovering interrupted turn from session history (no marker)`
  (includes `prompt_len`, `interrupted`)
- Drain start / rewarm: existing messages, note "(marker or history)" on start

## Not done / out of scope

- git commit (forbidden)
- Free-period debit invent
- Requiring the operator to write a marker by hand
