# Auto-resume after error: real root cause (2026-08-09)

## Retraction (wrong-binary-as-primary)

The prior report
[`.agents/reports/impl-auto-resume-after-error-still-2026-08-09.md`](impl-auto-resume-after-error-still-2026-08-09.md)
claimed dogfood idle (bitmagi / iso / surmount-server after 403) was because the
operator ran **stock `~/.grok/bin/grok` (Aug 8 download)** without the fix.

**That diagnosis is wrong as primary.** Operator evidence: those sessions show
**our octal emissive Surmount theme**. That is product chrome, not stock xAI
download chrome. Install / PATH skew may still exist as **secondary residual**
only; it is not the explanation for these three idle screenshots.

## Real root cause (code path evidence)

Two product gaps, both in the Surmount tree, both independent of binary age:

### Gap 1: cold `SessionLoaded` treated error as success (stale-marker)

On durable load replay, primary-user `turn_completed` with `stop_reason !=
"cancelled"` set `last_primary_user_turn_completed_in_replay = true`, including
**`stop_reason: "error"`**.

Stale-marker gate (committed shape before the error fix):

```text
stale = completed_in_replay && !mid_work
```

Error is a terminal, so mid-work is false. Result:

1. Marker path **drops** `canceled_turn_resume.json` as "stale after completed"
2. History recovery only covered mid-work, **not** error terminals
3. Session sits idle with yellow 403 history

Contrast: mid-work cancel / killall leaves `completed` false (or unfinished
children), so marker path A still SendPrompt. That is why screensaver /
mid-work cancel-resume worked and error-terminal sessions did not.

**Fix (load path):**

- `last_primary_user_turn_failed_in_replay` set when primary `stop_reason ==
  "error"` during `loading_replay`
- `session_last_turn_ended_in_error` (failed flag **or** scrollback
  `TurnFailed`)
- Stale gate is success-only: `completed && !mid_work && !error`
- History recovery re-queues last user prompt on mid-work **or** error
- Keep / re-arm marker on error terminals (still clear on rate_limit and clean
  success)

### Gap 2: already-open reopen never re-entered resume (dogfood shape)

Live disk for all three dogfood sessions after the 403s:

| Project | Session | Last `turn_completed` | Marker still present | Marker `prompt_id` match |
|---------|---------|----------------------|----------------------|--------------------------|
| bitmagi | `019fbf4b-69bc-7ed2-bd01-66d51b63b664` | `stop_reason: error` + 403 agent_result | yes | yes |
| iso | `019f85f6-3971-7363-a8b6-833ed66829c0` | `stop_reason: error` + 403 | yes | yes |
| surmount-server | `019fb3cc-d9dd-7340-a9b0-a9e64eacb300` | `stop_reason: error` + 403 | yes | yes |

Evidence that **cold SessionLoaded resume never applied after those 403s**:

- Markers still on disk with **old** `canceled_at` (apply would clear; drain-fail
  re-warm would rewrite timestamps)
- No second durable turn after the error terminal (auto-resume SendPrompt would
  append updates / another turn_completed)
- `summary.json` / `chat_history.jsonl` touched later without new primary work

So the live UX was: **same product process** stays idle after live 403
(TurnFailed painted, marker kept). Operator "reopen" from picker / dashboard
hits `focus_if_session_already_open` → focus only → **empty effects** → still
idle. Cold load was not the path those screenshots took; the product still
had to fix reopen-without-reload.

**Fix (already-open path):**

- New `try_auto_resume_error_idle_on_reopen` in
  `dispatch/session/load.rs`
- Called when focus-if-already-open wins (load, remote pick, dashboard attach)
- Only when **error terminal** evidence (`TurnFailed` in scrollback or failed
  load flag) and idle; not cancel-only markers; not clean success
- Enqueue last user text (marker preferred) + force drain + "Continuing
  interrupted turn..." toast

## What product changed

| Path | Change |
|------|--------|
| `agent_view/mod.rs` + `session.rs` | `last_primary_user_turn_failed_in_replay` field + reset |
| `acp_handler/session_notification.rs` | set failed flag on primary `stop_reason == "error"` in load replay |
| `acp_handler/mod.rs` | clear failed flag when a new user prompt arrives in replay |
| `dispatch/session/load.rs` | error evidence, history recovery, success-only stale gate, **already-open error resume** |
| `dispatch/dashboard.rs` | already-open attach runs error-idle resume |
| `dispatch/turn.rs` + `prompt.rs` | keep / re-arm marker on error (not rate_limit) |
| `canceled_turn_resume.rs` | module docs match error keep + load contract |

## TDD

### Red contract (named; dogfood shape)

| Test | Contract |
|------|----------|
| `already_open_error_idle_reopen_auto_resumes_without_session_loaded` | Live TurnFailed + marker + idle; **no** SessionLoaded; reopen must SendPrompt + continue toast. Uses real bitmagi-like UUID session / prompt ids and 403 agent_result text. |
| `already_open_clean_idle_reopen_does_not_auto_resume` | Clean TurnCompleted idle reopen must **not** invent SendPrompt |

Without `try_auto_resume_error_idle_on_reopen`, the first test fails: effects empty,
session stays Idle (focus-only).

### Cold-load contracts (prior + still green)

| Test | Contract |
|------|----------|
| `session_loaded_wire_error_turn_completed_auto_resumes_without_marker` | Wire `turn_completed` error sets failed flag; SessionLoaded SendPrompt |
| `session_loaded_wire_error_with_marker_still_auto_resumes` | Marker not stale-dropped after error |
| `session_loaded_error_terminal_auto_resumes_without_marker` | TurnFailed + failed flag, no marker |
| `session_loaded_durable_error_flag_auto_resumes_without_session_event` | Durable flag only |
| `session_loaded_marker_after_error_terminal_still_resumes` | Marker + completed+failed |
| Clean / cancel / stale success guards | still no false re-fire |

### Commands

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
cargo test -p xai-grok-pager --lib session_loaded_    # 36 passed
cargo test -p xai-grok-pager --lib already_open_      # 2 passed
```

## Dogfood (product binary; theme already proves product)

1. Rebuild / install the **product** binary you already run (theme path is fine):
   e.g. `/rebuild` in-session or `just install` so the running process is this
   tree.
2. For each error-ended session (bitmagi / iso / surmount-server):
   - If still open in the multi-session process: open it again from the picker
     or dashboard (already-open path). Expect toast **"Continuing interrupted
     turn..."** and a new turn of the last user prompt.
   - Or quit and relaunch that session (cold SessionLoaded). Same toast +
     SendPrompt.
3. Credentials may 403 again if OAuth is still bad. That is separate residual;
   the contract is **not silent idle**.
4. Clean successful sessions must **not** re-fire on reopen.

## Secondary residual (not primary)

- PATH / packaging lag of `~/.grok/bin/grok` vs `just install` / `/rebuild`
  target can leave some launches on older trees. Operator theme evidence rules
  that out as the primary story for **these** screenshots.
- Immediate same-turn re-fire on the live 403 event itself is intentionally
  **not** done (would 403-loop credentials). Resume moments: load, reopen
  (including already-open), rebuild relaunch.

## Supersedes

Corrects
[`.agents/reports/impl-auto-resume-after-error-still-2026-08-09.md`](impl-auto-resume-after-error-still-2026-08-09.md)
(wrong-binary primary). Extends
[`.agents/reports/impl-rebuild-auto-resume-after-error-2026-08-09.md`](impl-rebuild-auto-resume-after-error-2026-08-09.md)
with already-open reopen path and dogfood disk evidence.
