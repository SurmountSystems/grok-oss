# Auto-resume after error on session load / rebuild (2026-08-09)

## Root cause

Error-class turn failures were treated like **successful completes** for resume:

1. **Marker clear on error.** When a turn ended with `stop_reason: "error"` (or failed `PromptResponse`), product **cleared** `canceled_turn_resume.json` so reopen would not re-queue the failed prompt (prior dogfood: flat-poll / image prompts re-fired on every `/rebuild`).

2. **Stale-marker gate treated error as completed.** Load replay sets `last_primary_user_turn_completed_in_replay = true` for any non-`cancelled` stop reason, including `error`. If a marker still existed, the stale gate dropped it: completed + no mid-work → clear file, no SendPrompt.

3. **History recovery only covered mid-work.** `session_looks_interrupted_mid_work` requires unfinished children, running tools, or an **open** turn with **no** terminal. `TurnFailed` / durable `stop_reason: error` **is** a terminal, so history recovery returned false.

4. **Mid-running subagent sessions still worked** because unfinished children or open-turn evidence still fired path B (or live adoption). Error-idle sessions had neither.

Live iso evidence (`019f85f6-…`): last updates line is `turn_completed` with `stop_reason: "error"` and 403 bad-credentials; events end `turn_ended` outcome `error`; session sat idle after rebuild.

## Named product contract

> If the conversation is resumed and the last thing was an error, please auto-resume.

| Last history shape | Auto-resume on load / rebuild relaunch? |
|--------------------|------------------------------------------|
| Error-class failure (`stop_reason: error`, TurnFailed, Internal error, 403 as turn failure) | **Yes** → SendPrompt last user prompt + "Continuing interrupted turn..." |
| Clean success (`TurnCompleted` / end_turn, no mid-work) | **No** (stale marker dropped; no history re-fire) |
| Mid-work killall / open turn / unfinished children | **Yes** (existing path B) |
| User cancel without marker | **No** history resume (marker path still wins when present) |
| Rate limit | Marker cleared; dedicated paywall UX (not this contract) |

If credentials are still bad after resume, the turn may 403 again. That is OK for this contract; re-auth UX is separate residual.

## Fix (minimal)

1. **`last_primary_user_turn_failed_in_replay`** — set during load replay when primary-user `stop_reason == "error"`. Reset with the completed flag on new user prompts / load window start.

2. **`session_last_turn_ended_in_error`** — true if failed flag **or** last scrollback turn-terminal after last user is `TurnFailed`.

3. **History recovery** — re-queue last user prompt when mid-work **or** error terminal.

4. **Stale-marker gate** — drop marker only after **successful** complete (completed && !mid-work && !error). Error keeps marker application.

5. **Keep marker on error** — do not clear cancel-resume on error terminals (still clear on rate_limit and clean success).

## TDD

### Red → green contracts (new)

| Test | Contract |
|------|----------|
| `session_loaded_error_terminal_auto_resumes_without_marker` | TurnFailed + failed flag, no marker → SendPrompt + continue toast |
| `session_loaded_durable_error_flag_auto_resumes_without_session_event` | Durable-only failed flag (no SessionEvent) → SendPrompt |
| `session_loaded_marker_after_error_terminal_still_resumes` | Marker + completed+failed → not stale-dropped; SendPrompt |

### Regression guards (still green)

| Test | Contract |
|------|----------|
| `session_loaded_clean_completed_does_not_auto_resume_without_marker` | Clean TurnCompleted → no re-fire |
| `session_loaded_replay_completed_without_session_event_does_not_auto_resume` | Durable success flag only → no re-fire |
| `session_loaded_stale_marker_after_completed_primary_does_not_resume` | Stale marker after success → drop |
| `session_loaded_user_cancelled_terminal_does_not_history_resume` | Cancel without marker → no history resume |

Full filter `session_loaded_`: **34 passed**.

## Files changed

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | `last_primary_user_turn_failed_in_replay` field |
| `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` | init + begin_replay_window reset |
| `crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs` | set failed flag on error stop_reason |
| `crates/codegen/xai-grok-pager/src/app/acp_handler/mod.rs` | clear failed flag on new user prompt in replay |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | error evidence, recovery, stale gate, logs |
| `crates/codegen/xai-grok-pager/src/app/dispatch/turn.rs` | clear marker on rate_limit only (keep on error) |
| `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs` | stop clearing marker on failed PromptResponse |
| `crates/codegen/xai-grok-shell/src/session/canceled_turn_resume.rs` | module docs |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/turn.rs` | three new contracts |

## Verify commands

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
cargo clippy -p xai-grok-shell --lib -- -D warnings    # clean
cargo test -p xai-grok-pager --lib session_loaded_     # 34 passed
```

## Residual

- **403 / bad credentials after auto-resume:** product will re-fire the prompt; if OAuth is still invalid, the turn fails again. Login / token refresh UX is separate (see OAuth 403 bad-credentials work). Still better than silent idle.
- **Rate-limit terminals** intentionally do not auto-resume via this path (marker cleared; paywall / retry owns next step).
- **Operator install:** rebuild/install binary and reopen error-ended sessions (iso, bitmagi, surmount-server) to dogfood: expect continue toast + SendPrompt of the last user turn.
