# Resume regression remains (2026-08-08) — root cause and fix

## Operator symptoms (both real)

1. **False positive:** reopening a finished session re-sent the last user
   prompt without intent (grok-build: re-fired `Still nothing!!! [Image #1]`).
2. **Iso still idle:** hot iso session stayed on idle composer after
   killall/reopen until a later reopen with a binary that had open-turn
   history recovery.

Prior report
(`.agents/reports/impl-iso-resume-still-idle-2026-08-08.md`) fixed iso
evidence (open turn without unfinished subagents) but **did not** account for
how load replay records turn terminals. That left the false-positive path
wide open and made "open turn" true for almost every completed session.

## Live forensics (verified on disk + logs)

### Grok-build false fire (smoking gun)

Session:
`~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fsurmount%2Fgrok-build/019faf9d-ef93-7d93-b34b-9f19b6345613`

| Time (UTC) | Evidence |
|------------|----------|
| 09:09:32 | Turn completed cleanly (`e31a292b…`) |
| 09:14:27 | `session.load.start` (reopen) |
| 09:14:28 | **Immediate** `prompt.drain` `prompt_len: 27` → auto SendPrompt |
| | User text: **`Still nothing!!! [Image #1]`** (exactly 27 chars) |
| 09:14–09:27 | That re-fire ran as turn 325 and completed |
| 09:29 | Operator: "Oh no. Now it's repeated the last prompt…" |

So: clean completed session + no marker → history recovery still re-queued
the last user prompt.

### Iso true positive (open implement)

Session:
`~/.grok/sessions/%2Fhome%2Fhunter%2FProjects%2Fai%2Fiso/019f85f6-3971-7363-a8b6-833ed66829c0`

| Evidence | Disk |
|----------|------|
| Marker `canceled_turn_resume.json` | **Absent** |
| Subagents | 1297 completed, 6 cancelled, 1 failed, **0 unfinished** |
| Events | `turn_started` 461 with **no** following `turn_ended` |
| Last user | `/implement --effort 2 all remaining residual…` (prompt_461) |
| Last durable `turn_completed` in updates | **Before** prompt_461 (prior turn only) |
| Loads without resume | 07:11 (pid 182664), 08:48 (**pid 1431614**, long-lived process) |
| Load **with** resume | 09:31:02 pid 2008023 → `prompt.drain` len 2429 (implement text) |

Earlier "Still nothing" on iso was largely **old process / pre-open-turn
binary**. Open-turn recovery *did* fire at 09:31 once a process with the
prior fix loaded the session. The remaining product bug was the **false
positive** on completed sessions (and incomplete false-positive tests).

### Binary / process (at investigation)

- Installed: `~/.cargo/bin/grok-oss` (rebuilt this pass; mtime after install).
- Live grok-build pid 1982020: `/proc/…/exe` → current
  `~/.cargo/bin/grok-oss` (not deleted).
- Version string still shows `0.2.111 (c87f66a61d94)` (HEAD tag); tree has
  uncommitted fix until human commit.

## Root cause (why prior fix was insufficient)

On `session/load` replay, durable `TurnCompleted` is handled as:

```text
loading_replay → insert prompt_id into replayed_terminal_prompts
              → do NOT push SessionEvent::TurnCompleted into scrollback
```

(`session_notification.rs` TurnCompleted arm.)

History recovery then used:

```text
scrollback_has_open_turn_without_terminal
  = agent work after last user prompt
    AND no SessionEvent turn-terminal in scrollback
```

After every real load of a **completed** session:

- scrollback has user prompt + agent work
- scrollback has **no** SessionEvent terminal (never replayed)
- → open-turn **true** → re-fire last prompt

Unit test
`session_loaded_clean_completed_does_not_auto_resume_without_marker` only
pushed a manual `SessionEvent::TurnCompleted`, which **never** matches real
load shape. False green.

Iso open implement has no durable primary `turn_completed` for the last
user turn, so the same open-turn signal is a **true** positive there.

## Fix (minimal)

### Gate

`scrollback_has_open_turn_without_terminal` now returns **false** when
`agent.last_primary_user_turn_completed_in_replay` is true.

### Flag maintenance

| When | Action |
|------|--------|
| `begin_replay_window` | clear flag |
| `TurnCompleted` during `loading_replay` for **primary user** prompt_id (`PromptOrigin::User`) | set flag **true** |
| New resumable `UserPrompt` applied during load (`handle_update` path) | set flag **false** |
| Synthetic terminals (`subagent-completed-*`, `task-completed-*`, …) | **do not** set true (parent may still be open) |

Marker path unchanged (still wins). Unfinished subagents / running scrollback /
tracker mid-turn activity unchanged.

### Exact auto-resume gates now

**MUST resume (true positive)** when resume setting on and not adopting a live
running prompt, and either:

1. **Marker path:** valid `canceled_turn_resume.json` → SendPrompt marker text +
   "Resuming canceled turn..."
2. **History path (no marker):** `session_looks_interrupted_mid_work` **and**
   last resumable user prompt text, where interrupted means any of:
   - unfinished subagent rows (pre-finalize capture)
   - parent/child running scrollback
   - tracker mid-turn activity (suppressed waits, pending tools, open
     thinking/agent message)
   - open turn: agent work after last user **and** no SessionEvent terminal
     **and** `last_primary_user_turn_completed_in_replay == false`

Then force-drain past background holds; toast
"Resuming interrupted turn..."; loud failure toast if evidence but no prompt
or drain blocked.

**MUST NOT resume (false positive):**

- Clean completed turn with SessionEvent terminal in scrollback
- Clean completed turn with **only** replay durable terminal
  (`last_primary_user_turn_completed_in_replay`) and no SessionEvent
- User-cancelled turn with `TurnCancelled` terminal and no marker
- Setting off / no prompt / adopting live running prompt

## Tests

| Test | Contract |
|------|----------|
| `session_loaded_replay_completed_without_session_event_does_not_auto_resume` | **False-positive dogfood:** user + agent work, no SessionEvent, flag true → no SendPrompt |
| `session_loaded_user_cancelled_terminal_does_not_history_resume` | TurnCancelled without marker → no history resume |
| `session_loaded_clean_completed_does_not_auto_resume_without_marker` | SessionEvent terminal path (unchanged) |
| `session_loaded_recovers_open_implement_turn_without_unfinished_subagent` | Iso open implement still resumes |
| Other existing `session_loaded_*` resume tests | Still green |

Filter: `cargo test -p xai-grok-pager --lib session_loaded_` → **27 passed**.

## Code touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | field `last_primary_user_turn_completed_in_replay` |
| `…/agent_view/session.rs` | init + clear on `begin_replay_window` |
| `…/acp_handler/session_notification.rs` | set flag on primary-user TurnCompleted during replay |
| `…/acp_handler/mod.rs` | clear flag when resumable UserPrompt applied during load |
| `…/dispatch/session/load.rs` | open-turn gate consults flag |
| `…/dispatch/tests/turn.rs` | false-positive + cancelled-terminal tests |

## Commands

```text
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
cargo clippy -p xai-grok-shell --lib -- -D warnings   # clean
cargo test -p xai-grok-pager --lib session_loaded_    # 27 passed
just install
# grok-oss 0.2.111 (c87f66a61d94) [stable] under ~/.cargo/bin/grok-oss
# binary mtime: after this install
```

## Operator dogfood

1. **Fully quit** any live `grok-oss` (including this chat if it still holds an
   old mapping). If `/proc/<pid>/exe` shows `grok-oss (deleted)`, that process
   cannot pick up this binary. Quit the TUI completely, do not only reload a
   session inside the same process.
2. Confirm installed binary: `~/.cargo/bin/grok-oss --version` and
   `ls -l ~/.cargo/bin/grok-oss` (mtime after this install).
3. **False positive check:** reopen a session that already finished cleanly
   (e.g. grok-build after idle composer). Must **not** re-send last prompt;
   idle "What do you want to get done?" is correct.
4. **Iso true positive:** from `/home/hunter/Projects/ai/iso`, reopen
   `019f85f6-3971-7363-a8b6-833ed66829c0` **only if** that session still has
   no durable terminal after the last `/implement` (or killall mid-implement
   again). Must toast **Resuming interrupted turn...** and auto-start that
   implement text.
5. If mid-work evidence exists but resume fails, toast must be
   `Interrupted work found but resume failed: {reason}` (not silent idle).

## Not done

- git commit / stage (forbidden)
- Killing operator live processes
