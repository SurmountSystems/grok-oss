# Implement: killall cancel-resume auto-restart (2026-08-08)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Board:** operator dogfood FAIL — reopen after killall idle (no toast, no turn)
**Binary installed:** `~/.cargo/bin/grok-oss` via `just install` (0.2.111)

## Operator symptom

Iso session after `killall` reopen: history visible, **composer empty**, **nothing running**, no "Resuming canceled turn...", no implement restart.

Contract: marker present → toast + **auto-run** the interrupted prompt (Send path), then consume marker without burning it on a blocked drain.

## Live evidence

| Check | Result |
|-------|--------|
| Iso `canceled_turn_resume.json` | **None** under `~/.grok/sessions/*iso*` |
| Active marker elsewhere | grok-build session had a live eager marker (eager write path works) |
| Config `resume_canceled_turn_on_restart` | Unset → default **on** |
| Installed binary (pre-fix) | 0.2.111 with eager write + prior load re-queue |

Iso having **no marker** after dogfood is consistent with a prior reopen that **cleared the one-shot file while the drain never started a turn** (marker burned, queue only in memory, process exit loses pending). Second open: cold history, empty composer, no toast.

## Root cause

`handle_session_loaded` already enqueued the cancel-resume prompt, showed the toast, and called `maybe_drain_queue`. Two gaps blocked dogfood:

1. **Zombie subagents hold the queue.** After killall mid-subagent, cold load / replay can leave `subagent_sessions` with `finished=false`. `holds_queue_for_background()` is true → `maybe_drain_queue` returns blocked → **no `Effect::SendPrompt`**. Parent looks idle; nothing auto-runs.
2. **Marker cleared before successful start.** Apply cleared `canceled_turn_resume.json` as soon as it enqueued. A blocked drain burned the one-shot file. Next reopen had nothing to resume. Prior unit test accepted `requeued || drained`, so **composer/queue-only green** passed CI.

Eager write at turn start was already correct; the resume **start** path was not.

## Fix

In `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` (`handle_session_loaded`):

1. **Cold load finalize zombies** (`!adopting`): mark unfinished `subagent_sessions` finished (process is dead; children cannot still be live). Clears background hold for normal drains and still-running chrome.
2. **Force-drain on cancel-resume apply:** use `force_drain_queue_past_background` when a marker was just re-queued (defense in depth if residual holds remain).
3. **Re-warm marker if drain fails:** still clear the one-shot on enqueue so a successful drain can re-eager-write for the **new** active turn (second killall). If drain does not emit Send / SendPromptBlocks / SetModeThenPrompt, re-write the marker so a later reopen can retry instead of leaving the session permanently idle.
4. Toast still shows "Resuming canceled turn..." when apply runs.

## TDD

| Test | Contract |
|------|----------|
| `session_loaded_applies_cancel_resume_marker_and_toasts` | **Strengthened:** must emit `Effect::SendPrompt` for the interrupted text, turn running, queue empty, toast present (not `requeued \|\| drained`) |
| `session_loaded_cancel_resume_starts_turn_despite_zombie_subagents` | **New:** unfinished subagent row + marker → SendPrompt + toast + zombie `finished` + no background hold |
| `note_cancel_resume_eagerly_writes_durable_marker_without_quit` | Eager write still on turn-start note (regression) |
| Prior quit / armed / arm_and_persist shell tests | Still green |

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo test -p xai-grok-pager --lib -- \
  session_loaded_applies_cancel_resume session_loaded_cancel_resume_starts_turn \
  note_cancel_resume_eagerly quit_mid_turn_after_first_activity \
  quit_mid_turn_writes_canceled quit_idle_does_not_write
cargo test -p xai-grok-shell --lib -- canceled_turn_resume
cargo clippy -p xai-grok-pager -p xai-grok-shell --lib -- -D warnings
just install
```

All listed filters green; clippy clean (`-D warnings`); install verified.

## What the operator should see after install

1. Start a turn (mid-tool / mid-subagent is fine).
2. Marker while running (eager path):
   ```bash
   find ~/.grok/sessions -path '*iso*' -name 'canceled_turn_resume.json'
   ```
3. `sudo killall grok-oss` (default SIGTERM).
4. Reopen **that** session (last session / resume / project default).
5. Toast **Resuming canceled turn...** and the interrupted prompt **starts as a live turn** (spinner / tool activity), not an empty composer waiting for Enter.

## Key paths

| Concern | Path |
|---------|------|
| Cold-load zombie finalize + force drain + re-warm | `xai-grok-pager/.../dispatch/session/load.rs` |
| Background hold / force drain | `dispatch/queue.rs` (`holds_queue_for_background`, `force_drain_queue_past_background`) |
| Eager arm+persist | `xai-grok-shell/.../canceled_turn_resume.rs` |
| Note at drain / turn start | `xai-grok-pager/.../agent.rs` + `dispatch/queue.rs` |
| Tests | `dispatch/tests/turn.rs` |

## Remaining limits

- **SIGKILL** (`kill -9` / `killall -9`): no userspace; only pre-death eager marker helps.
- **Adopting a live leader turn** (`running_prompt_id` adopt): cancel-resume is skipped (live turn already owns the session).
- If drain fails for other hard gates (plan approval open, model switch pending), marker is re-written for the next reopen; toast still shows once on that attempt.

## Not claimed

- Free SuperGrok period debit invents (out of scope).
- Git commit (forbidden).
