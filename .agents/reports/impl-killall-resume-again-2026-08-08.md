# Implement: killall / SIGTERM cancel-resume again (2026-08-08)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Board:** `bug:killall-no-graceful-resume`
**Operator:** dogfood after `sudo killall` mid-implement with a subagent; reopen
looked like cold history, not “Resuming canceled turn...”.

## Root cause

Prior ship (2026-08-07) wired `Action::Quit` → `persist_cancel_resume_on_graceful_quit`
→ `write_cancel_resume_marker_for_session`. That path only read
**`session.in_flight_prompt`**.

Production clears `in_flight_prompt` on **first server activity** (chunk, tool,
subagent, …) so Esc can no longer pristine-rewind the composer:

```text
crates/codegen/xai-grok-pager/src/app/acp_handler/mod.rs
  // Once the server has emitted any activity … clear the stash
  agent.session.in_flight_prompt = None;
```

Mid-implement with a subagent is always past first activity. So SIGTERM /
default `killall grok-oss` reached graceful Quit, but the marker writer saw
**no prompt text** and wrote nothing. Reopen loaded transcript only (composer
empty, no auto re-queue toast).

Not a binary-name mismatch (`grok-oss` is correct). Not SIGKILL (default
killall is SIGTERM). Not missing SIGTERM handler (first signal →
`QUIT_NOTIFY` → `Action::Quit`). The gap was **empty prompt source after first
activity**.

Secondary hardening: first-signal and hard-exit paths now also write from a
process-level **arm** so a wedged event loop or second-signal force-exit still
has a chance to persist the marker without waiting on `Action::Quit`.

## What `sudo killall grok-oss` does vs our handlers

| Step | Behavior |
|------|----------|
| Default killall | Sends **SIGTERM** (not SIGKILL) to every process named `grok-oss` |
| Agent process model | Shell is **in-process** (thread), not a second `grok-oss` binary |
| First SIGTERM | `signal_handler`: write **armed** cancel-resume (if any) → notify event loop |
| Event loop | `Action::Quit` → `persist_cancel_resume_on_graceful_quit` for all agents |
| Second signal | Hard terminal restore + exit; still attempts armed write again |
| SIGKILL (`-9`) | No userspace handler; **cannot** leave a marker |

## Fix

1. **`AgentSession::cancel_resume_prompt_text`** — whole-turn display/user text
   set at drain/shim (including skill display text). Survives first-activity
   clear of `in_flight_prompt`. Cleared on turn start/finish.
2. **`prompt_text_for_cancel_resume()`** — cancel_resume text, then
   `in_flight_prompt`, then `compact_held_prompt`.
3. **`write_cancel_resume_marker_for_session`** uses that resolver.
4. **Process shutdown arm** (`canceled_turn_resume::{arm,write_armed}_…`)
   published when prompt text is noted; written on first SIGTERM and hard-exit.
5. Docs: user-guide `17-sessions.md`; FORK inventory bullet.

## Red / green

| Test | Contract |
|------|----------|
| `quit_mid_turn_after_first_activity_writes_cancel_resume_marker` | TurnRunning + `in_flight_prompt = None` + whole-turn text → Quit writes marker (killall dogfood state) |
| `quit_mid_turn_writes_canceled_turn_resume_marker` | Pristine mid-turn still writes |
| `quit_idle_does_not_write_canceled_turn_resume_marker` | Idle invents nothing |
| `armed_process_shutdown_writes_cancel_resume_marker` | Signal arm writes without AppView |
| `process_shutdown_class_marker_is_auto_resume_eligible` | Marker shape still auto-resume eligible |
| `request_graceful_or_exit_notifies_registered_quit` | Graceful notify still works |

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo test -p xai-grok-shell --lib -- canceled_turn_resume process_shutdown armed_process
cargo test -p xai-grok-pager --lib -- \
  quit_mid_turn_after_first_activity quit_mid_turn_writes_canceled \
  quit_idle_does_not_write request_graceful_or_exit
cargo clippy -p xai-grok-pager -p xai-grok-shell --lib -- -D warnings
```

All listed filters green; clippy clean (`-D warnings`).

## Key paths

| Concern | Path |
|---------|------|
| Whole-turn text | `xai-grok-pager/src/app/agent.rs` (`cancel_resume_prompt_text`) |
| Drain / shim set | `dispatch/queue.rs` (`note_cancel_resume_prompt_text`) |
| Marker write | `dispatch/turn.rs` (`write_cancel_resume_marker_for_session`) |
| First activity clear (unchanged, rewind only) | `acp_handler/mod.rs` |
| Signal arm + write | `xai-grok-shell/.../canceled_turn_resume.rs` |
| SIGTERM first/hard | `signal_handler.rs` (`request_graceful_or_exit`, `shutdown_with_terminal_restore`) |
| Load re-queue | `dispatch/session/load.rs` (unchanged) |

## Remaining limits

- **SIGKILL** (`kill -9` / `killall -9`): no userspace code runs; no marker.
- Multi-session single process: process arm holds one session (last publisher);
  graceful Quit still walks all agents.
- Operator must rebuild/install (`just install` / cargo install path) for the
  running `~/.cargo/bin/grok-oss` to pick this up.

## Forbidden / not done

- No git add / commit / push
- No free SuperGrok period debit invention
- No claim that SIGKILL is graceful
