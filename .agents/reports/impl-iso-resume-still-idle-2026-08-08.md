# Iso resume still idle (2026-08-08) — root cause and fix

## Why it failed (one root cause)

**History recovery evidence was too narrow for the real iso session shape.**

Prior fix only treated a session as interrupted when:

1. unfinished subagent rows (`finished == false`), or
2. parent/child scrollback still had **running** entries.

The hot iso session
`~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fai%2Fiso/019f85f6-3971-7363-a8b6-833ed66829c0`
matches **neither**:

| Evidence | Live disk |
|----------|-----------|
| Marker `canceled_turn_resume.json` | **Absent** |
| Subagent `meta.json` statuses | 1297 completed, 6 cancelled, 1 failed, **0 unfinished** |
| Unpaired spawn/finish in `updates.jsonl` | **0** unfinished (last child `cancelled`) |
| Last parent tool | `get_command_or_subagent_output` with `timeout_ms: 900000`, **no completion update** |
| Events | `turn_started` 461 with **no** following `turn_ended`; ends `waiting_for_model` |
| Last user | `/implement --effort 2 all remaining residual tasks…` (prompt_461 / chat_history) |

`get_command_or_subagent_output` is a **suppressed** bg-plumbing tool: the ACP
tracker puts it in `blocking_waits` and **never** creates a running scrollback
entry. After killall mid-wait:

- all children finished → no unfinished subagent rows after replay
- no running scrollback entries (wait tool suppressed)
- open turn never got `TurnCompleted`

→ `session_looks_interrupted_mid_work` returned **false** → no SendPrompt, silent idle.

Secondary dogfood note (not the product bug): process **1431614** was still
running the **pre-install** binary (`/proc/…/exe` → `grok-oss (deleted)`, cwd
`/home/hunter/Projects/ai/iso`, started 02:48; install was 03:14 then this
fix). Reopen inside that process cannot pick up a new binary. Full quit +
reopen with the installed binary is required.

## Exact fix

### 1. Broader interruption evidence (`load.rs` + tracker)

`session_looks_interrupted_mid_work` now also true when:

- **Tracker mid-turn activity** before `finish_turn`:
  `blocking_waits`, `pending_tools`, open thinking, or open agent message
  (`AcpUpdateTracker::has_in_flight_mid_turn_activity`)
- **Open turn without terminal**: after the last resumable user prompt there is
  agent work (message / thinking / tool / subagent / bg task) and **no**
  turn-terminal session event (`TurnCompleted` / `TurnCancelled` /
  `TurnHalted` / `TurnFailed`)

Capture still runs **before** `finish_turn` / zombie finalize (order unchanged).

### 2. Loud failure toasts

When mid-work evidence exists but resume does not start:

- no user prompt → `"Interrupted work found but resume failed: no user prompt to re-queue"`
- setting off → `"… resume on restart is off in settings"`
- drain blocked after enqueue → `"… queue drain did not start a turn"`

Helper: `canceled_turn_resume::interrupted_resume_failed_toast(reason)`.

### 3. Tests (red→green contracts)

| Test | Contract |
|------|----------|
| `session_loaded_recovers_open_implement_turn_without_unfinished_subagent` | **Iso shape**: no marker, no unfinished subagent, no running scrollback, `/implement …` + agent work without terminal → SendPrompt + interrupted toast |
| `session_loaded_interrupted_without_prompt_toasts_failure` | Evidence without prompt → failure toast, stay idle |
| `session_loaded_clean_completed_does_not_auto_resume_without_marker` | Updated: clean history includes `TurnCompleted` so open-turn signal stays false |

Full filter `session_loaded_`: **25 passed**.

### Code

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/acp/tracker.rs` | `has_in_flight_mid_turn_activity` |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | open-turn scan; expand evidence; failure toasts |
| `crates/codegen/xai-grok-shell/src/session/canceled_turn_resume.rs` | `interrupted_resume_failed_toast` |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/turn.rs` | iso + failure toast tests; clean TurnCompleted |

## Verify (operator)

1. **Fully quit** any `grok-oss` still attached to iso (especially pid using
   `(deleted)` binary). New binary is installed:
   `grok-oss 0.2.111 (c87f66a61d94) [stable]` under `~/.cargo/bin/grok-oss`.
2. From `/home/hunter/Projects/ai/iso` (or picker): reopen session
   `019f85f6-3971-7363-a8b6-833ed66829c0`.
3. **Must see toast:** `Resuming interrupted turn...`
4. Turn must auto-start the last `/implement --effort 2 all remaining residual…`
   prompt (not idle composer-only).

If resume is skipped despite mid-work, toast must say
`Interrupted work found but resume failed: {reason}` instead of silent idle.

## Commands run

```text
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
cargo clippy -p xai-grok-shell --lib -- -D warnings   # clean
cargo test -p xai-grok-pager --lib session_loaded_    # 25 passed
just install → grok-oss 0.2.111 (c87f66a61d94) [stable]
```

## Not done

- git commit (forbidden)
- Killing the operator’s live iso process (operator quit)
- Requiring a hand-planted marker
