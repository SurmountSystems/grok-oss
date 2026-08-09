# Cancel-resume re-fire still live — root cause and real fix (2026-08-08)

## Operator

Session `019faf9d-ef93-7d93-b34b-9f19b6345613` (grok-build). After prior "fix
shipped" claims, reopen / load still auto-sent `??? [Image #1]`.

## Live forensics (this pass)

### Hot process (at investigation)

| Item | Value |
|------|--------|
| PID | **3055710** |
| cmdline | `grok-oss` |
| exe | `/home/hunter/.cargo/bin/grok-oss` (not deleted; SHA matched install at open) |
| version | `0.2.111 (c87f66a61d94)` Surmount product |
| Not | official `grok` 1.0.0 (that was PID 2809478 earlier) |

### Marker on disk

Path:
`~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613/canceled_turn_resume.json`

At open (and after re-fire rewrite):

```json
{
  "prompt_text": "??? [Image #1]",
  "prompt_id": "ca5862b5-cd57-4fb3-a10b-94ddbeb289cc",
  "canceled_at": "2026-08-08T12:07:26.154908654+00:00",
  "reason": "user_cancel"
}
```

`canceled_at` matches the **re-fire** eager write, not only the older 11:56
cancel. Prior marker text was the same prompt; new `prompt_id` after auto-start.

### Smoking gun timeline (UTC)

| Time | Evidence |
|------|----------|
| 12:07:03 | Official 1.0.0 finished primary `5ea3428d…` (`Seems to be very broken right now`) with `stop_reason: end_turn` |
| Disk | Stale marker still held `??? [Image #1]` from earlier user_cancel |
| 12:07:24 | **grok-oss** 0.2.111 `session.load.start` (PID 3055710) |
| 12:07:26 | `session.load.done` then immediate `prompt.drain` **prompt_len=14** (= `??? [Image #1]`) |

So: **completed primary + leftover marker + Surmount binary that already
contained the stale-marker gate** still re-fired. Wrong-binary was not the
12:07 failure (it was earlier). Gate code was present in the running SHA.

### Why the shipped gate failed

Gate in `handle_session_loaded`:

```text
stale_after_completed =
  last_primary_user_turn_completed_in_replay
  && !session_looks_interrupted_mid_work(...)
```

On real load:

1. Replay streams agent message chunks → tracker `current_agent_msg` set,
   scrollback entries left **running**.
2. Durable `TurnCompleted` during `loading_replay` sets
   `last_primary_user_turn_completed_in_replay = true` but **does not**
   call `finish_turn` (by design for terminal / scrollback handling).
3. Mid-work is snapshotted **before** `finish_turn` in `handle_session_loaded`.
4. `session_looks_interrupted_mid_work` treated `has_running_entries` and
   `tracker.has_in_flight_mid_turn_activity()` as interrupted.
5. So after **every** clean completed turn with agent output, mid-work was
   **true** from replay residue alone.
6. `stale_after_completed` was always false → marker path always applied.

Unit test
`session_loaded_stale_marker_after_completed_primary_does_not_resume` set the
completed flag with **no** running residue → false green. Live load always
had residue.

Not the failure modes:

- Gate missing from binary (string and SHA were present).
- History recovery path B alone (drain length 14 is marker text, not last
  user text len 33).
- Incorrect unfinished-child detection for this open (last completed turn’s
  subagent finished in updates before terminal).

## Product fix

In `session_looks_interrupted_mid_work`:

1. **Unfinished subagents still interrupt** (parent completed + live children /
   killall mid-child still resumes).
2. If `last_primary_user_turn_completed_in_replay` is true and no unfinished
   children → **not** mid-work (ignore parent running scrollback / tracker
   residue until `finish_turn`).
3. Only when the primary did **not** complete in replay: keep running
   scrollback, tracker mid-turn, and open-turn signals (true killall /
   Esc / open implement).

Stale-marker gate then drops the file and does **not** SendPrompt after a
completed primary with only replay residue.

True continue paths kept:

| Case | Behavior |
|------|----------|
| Esc cancel marker (`cancelled` stop; completed flag false) | still resume |
| Marker + unfinished child + completed parent flag | still resume |
| No marker + open implement (no durable primary terminal) | history recovery still resumes |
| Completed primary + stale marker + running residue | **idle; clear marker** |

## Tests (red → green)

| Test | Contract |
|------|----------|
| `session_loaded_stale_marker_ignores_replay_running_residue_after_completed_primary` | **Live shape:** completed flag + AgentMessageChunk replay residue + stale `??? [Image #1]` marker → no SendPrompt, marker cleared |
| Existing `session_loaded_stale_marker_after_completed_primary_does_not_resume` | still green |
| Existing cancel / live-child / open-implement suite | still green |

Red proof (pre-fix mid-work body restored temporarily):

```text
cargo test -p xai-grok-pager --lib \
  session_loaded_stale_marker_ignores_replay_running_residue_after_completed_primary
# FAILED: completed primary + only replay residue must not count as mid-work
```

Green (fix restored):

```text
same filter → ok
cargo test -p xai-grok-pager --lib session_loaded_  → 31 passed
```

## Code touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | mid-work ignores replay residue after completed primary; unfinished children still interrupt |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/turn.rs` | new live-residue stale-marker contract |

## Verify commands

```text
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
cargo clippy -p xai-grok-shell --lib -- -D warnings   # clean
cargo test -p xai-grok-pager --lib session_loaded_    # 31 passed
just install
```

## Install

| Item | Value |
|------|--------|
| Path | `/home/hunter/.cargo/bin/grok-oss` |
| Version | `grok-oss 0.2.111 (c87f66a61d94)` |
| mtime | 2026-08-08 ~06:18 local (after this install) |
| SHA256 | `b15708cce2c6618807e36197818832cd655ecdc29d9e01acb8459e418ee31223` |
| Git HEAD tag in binary | still `c87f66a61d94` (uncommitted product fix until human commit) |

## Operator dogfood

1. **Fully quit** the hot TUI (PID 3055710 and any other `grok-oss` still on the
   pre-06:18 inode / `(deleted)` exe). New code only loads in a new process.
2. Confirm install: `~/.cargo/bin/grok-oss --version` and mtime after this
   install; optional `sha256sum` vs table above.
3. Use **`grok-oss`**, not PATH `grok` (still official 1.0.0 under
   `~/.grok/bin/grok`).
4. Optional clean marker before reopen:
   ```bash
   rm -f ~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613/canceled_turn_resume.json
   ```
   Fixed binary should also clear it on load when the last primary completed
   and there is no unfinished child work.
5. Reopen session idle after a finished turn: **no** auto SendPrompt of the
   last (or stale marker) user text; idle composer.
6. True interrupt still works: killall mid-turn, Esc cancel, or parent
   complete with live children + marker → "Resuming…" auto-start.

## Not done

- git commit / stage (forbidden)
- C4 multipoll / free SuperGrok period % work
- Removing eager marker write at turn start (still needed for killall races)
