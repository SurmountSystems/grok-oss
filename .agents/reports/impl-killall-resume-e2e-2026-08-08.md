# Implement: killall cancel-resume end-to-end (2026-08-08)

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Board:** `bug:killall-no-graceful-resume` / dogfood FAIL after prior partial fix  
**Binary installed:** `~/.cargo/bin/grok-oss` via `just install` (0.2.111)

## Live evidence (pre-fix)

| Check | Result |
|-------|--------|
| Markers under iso sessions (`~/Projects/ai/iso`) | **None** (`find` found zero `canceled_turn_resume*`) |
| Markers under `~/.grok` | Only `/tmp` test-session leftovers |
| Config `resume_canceled_turn_on_restart` | Unset → default **on** |
| Prior unit path (Quit + whole-turn text) | Green in source; **not enough** for dogfood |
| Process model | Client TUI + leader subprocess both named **`grok-oss`** (`agent leader`); default `killall` SIGTERM hits both |
| Default killall | SIGTERM (not SIGKILL); `sudo` does not change signal kind |
| SpaceXAI rebuild scrollback string | **Absent** from source and installed binary |

## Root cause (write path race, not resume eligibility alone)

Prior fix (2026-08-08 morning) added whole-turn `cancel_resume_prompt_text` and a
**process-level arm** written only when:

1. SIGTERM async task runs → `write_armed_process_shutdown_cancel_resume`, and/or
2. Event loop reaches `Action::Quit` → `persist_cancel_resume_on_graceful_quit`.

Both still **defer durable disk write until death**. Under real dogfood
(`sudo killall grok-oss` mid-implement with a subagent):

- Client and leader share the binary name; both die on killall.
- Marker is **only in memory** until the client's async signal task / Quit path
  runs. Tight races leave **no file**.
- Reopen of the iso session therefore loads **cold history only**: empty
  composer, no “Resuming canceled turn...”.

Resume path itself was already wired in `handle_session_loaded` (enqueue + toast
+ clear). Without a marker on disk it never fires. Config default is on.

fsync of the marker file already existed; parent-dir fsync after rename was
added as hardening.

## Fix (write + resume vertical)

1. **Eager active-turn sidecar** — `note_cancel_resume_prompt_text` →
   `publish_process_shutdown_cancel_resume_arm` now calls
   `arm_and_persist_process_shutdown_cancel_resume`, which **writes**
   `canceled_turn_resume.json` (with `sync_all` + parent dir fsync) as soon as
   the turn prompt is known.
2. **Signal / Quit paths** still re-write / arm (defense in depth).
3. **Successful turn finish** still **clears** the marker (no invent after
   success).
4. **Session load** still auto-applies when
   `[ui] resume_canceled_turn_on_restart` is on: re-queue once, toast
   “Resuming canceled turn...”, clear the one-shot marker (drain may immediately
   re-eager-write for the new active turn — correct for a second killall).
5. Load path uses `session_id.0` string consistently with the write side.
6. Docs: user-guide `17-sessions.md`, FORK inventory bullet.

### Key paths

| Concern | Path |
|---------|------|
| Eager arm+persist | `xai-grok-shell/.../canceled_turn_resume.rs` (`arm_and_persist_…`) |
| Note / publish | `xai-grok-pager/.../agent.rs` |
| Quit / Esc write | `dispatch/turn.rs` |
| SIGTERM write | `signal_handler.rs` + armed write |
| Resume apply | `dispatch/session/load.rs` |

## What the operator should see after install

1. Start a turn in a session (mid-tool / mid-subagent is fine).
2. Confirm marker appears while the turn is running:
   ```bash
   ls ~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fai%2Fiso/<session-id>/canceled_turn_resume.json
   ```
3. `sudo killall grok-oss` (default SIGTERM).
4. Reopen **that** session (last session / resume picker).
5. Toast: **Resuming canceled turn...** and the interrupted prompt re-queues /
   starts again (not cold empty composer).

## Tests (red → green contracts)

| Test | Contract |
|------|----------|
| `arm_and_persist_writes_cancel_resume_marker_eagerly` | Turn-start persist without SIGTERM |
| `note_cancel_resume_eagerly_writes_durable_marker_without_quit` | Note alone writes disk (no Quit) |
| `session_loaded_applies_cancel_resume_marker_and_toasts` | Load re-queues + toast |
| `quit_mid_turn_after_first_activity_writes_cancel_resume_marker` | Post first-activity Quit still writes |
| `quit_mid_turn_writes_canceled_turn_resume_marker` | Pristine mid-turn Quit |
| `quit_idle_does_not_write_canceled_turn_resume_marker` | Idle invents nothing |
| `armed_process_shutdown_writes_cancel_resume_marker` | Signal arm path |
| `process_shutdown_class_marker_is_auto_resume_eligible` | Marker shape eligible |
| `request_graceful_or_exit_notifies_registered_quit` | Graceful notify |

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo test -p xai-grok-shell --lib -- canceled_turn_resume
cargo test -p xai-grok-pager --lib -- \
  note_cancel_resume_eagerly session_loaded_applies_cancel_resume \
  quit_mid_turn_after_first_activity quit_mid_turn_writes_canceled \
  quit_idle_does_not_write request_graceful_or_exit
cargo clippy -p xai-grok-pager -p xai-grok-shell --lib -- -D warnings
just install
```

All listed filters green; clippy `-D warnings` clean; `just install` succeeded.

## Remaining limits

- **SIGKILL before any turn-start write** still cannot leave a marker (no
  userspace). After the turn has started, the eager file survives SIGKILL.
- Multi-session single process: process arm is still one slot (last publisher);
  each session still gets its own on-disk eager marker.
- No free SuperGrok period debit invention; no git commit.

## Forbidden / not claimed

- Not claiming SIGKILL is fully graceful without a prior turn-start write.
- Not claiming fixed without both write **and** resume (both wired + tested).
