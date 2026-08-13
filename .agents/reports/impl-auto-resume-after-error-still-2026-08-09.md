# Auto-resume after error still idle (dogfood 2026-08-09 evening)

## Verdict

Prior product fix **is in tree** and **is correct** for cold load / `/rebuild`
relaunch when the **new binary** runs. Operator dogfood (bitmagi, surmount-server,
iso idle with yellow 403 history) is explained by **running an old binary on
PATH**, not by a missing history path for durable `stop_reason: error`.

| Binary | Path | Age | Has error-terminal resume? |
|--------|------|-----|------------------------------|
| `grok` (what `which grok` hits) | `~/.grok/bin/grok` → `downloads/grok-1.0.0-linux-x86_64` | 2026-08-08 | **No** (pre-fix) |
| `grok-oss` (just install / `/rebuild` target) | `~/.cargo/bin/grok-oss` | 2026-08-09 19:49 | **Yes** |

`/rebuild` re-execs onto `installed_path` (`~/.cargo/bin/grok-oss`). A plain
shell launch of `grok` still uses the Aug 8 download.

## Live session evidence (read-only)

All three dogfood sessions on disk:

1. Last durable update is `turn_completed` with **`stop_reason: "error"`** and
   agent_result `API error (status 403 Forbidden): unauthenticated:bad-credentials…`
2. **`canceled_turn_resume.json` still present** (eager turn-start / keep path),
   `reason: user_cancel`, prompt text matches the failed implement / user turn
3. No later updates after the error terminal

| Project | Session id | Marker prompt_id matches error turn? |
|---------|------------|--------------------------------------|
| bitmagi | `019fbf4b-69bc-7ed2-bd01-66d51b63b664` | yes (`775d081f-…`) |
| iso | `019f85f6-3971-7363-a8b6-833ed66829c0` | yes (`17c185b3-…`) |
| surmount-server | `019fb3cc-d9dd-7340-a9b0-a9e64eacb300` | yes (`1ecd2e18-…`) |

With the fixed load path, either **marker path A** or **history recovery path B**
(failed flag from wire `turn_completed` error) must SendPrompt. Old binary:
error counted as "completed primary" → stale marker drop + no error history
recovery → idle.

Sessions that **never closed** after the live 403 also stay idle: the contract
is auto-resume on **session load / reopen / rebuild relaunch**, not an infinite
live re-fire loop on the same process (that would 403-loop credentials).

## Load path (hooks)

| Step | Where |
|------|--------|
| Replay `turn_completed` during `loading_replay` | `session_notification.rs`: set `last_primary_user_turn_completed_in_replay` + `last_primary_user_turn_failed_in_replay` when primary-user and `stop_reason == "error"` |
| Evidence | `session_last_turn_ended_in_error` (failed flag **or** scrollback `TurnFailed`) |
| Recover text | `recover_interrupted_turn_from_session` (mid-work **or** error + last user prompt) |
| Apply | `handle_session_loaded`: marker path (not stale if error) **or** history path → enqueue + force drain → `SendPrompt` |
| Keep marker on error | `prompt.rs` / `turn.rs` (rate_limit clears; error keeps / re-arms) |

## This turn's product changes

1. **Wire-path TDD** (no hand-set flags) in
   `acp_handler/tests/turn_completion.rs`:
   - `session_loaded_wire_error_turn_completed_auto_resumes_without_marker`
   - `session_loaded_wire_error_with_marker_still_auto_resumes`
2. **Re-arm marker on error terminal** when prompt text is known:
   - `prompt.rs` failed `PromptResponse` (not rate-limit / credits)
   - `turn.rs` reconcile `stop_reason: error`
   So older clear-on-error leftovers still get a file for path A; path B still
   covers no-marker via durable error flag.
3. Doc fix: `clear_cancel_resume_marker_for_session` comment no longer says
   "error terminal".

## TDD

| Contract | Result |
|----------|--------|
| Wire `turn_completed` error + no marker → SendPrompt | green (new) |
| Wire error + marker → SendPrompt (not stale-drop) | green (new) |
| Prior hand-set flag / TurnFailed contracts | still green |
| Full filter `session_loaded_` | **36 passed** |

## Verify

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
cargo test -p xai-grok-pager --lib session_loaded_    # 36 passed
```

## Dogfood to prove live (operator)

1. Install the tree binary: `just install` (or `/rebuild` in-session).
2. Confirm process: `readlink -f $(which grok)` must not be the Aug 8 download,
   **or** launch `~/.cargo/bin/grok-oss` / the path `/rebuild` re-execs.
3. Reopen each error-ended session (bitmagi / iso / surmount-server). Expect:
   toast **"Continuing interrupted turn..."** and a new turn of the last user
   prompt (may 403 again if OAuth is still bad; that is separate residual).
4. Optional log lines: `canceled_turn_resume: applying marker` or
   `recovering turn from session history` with `error_terminal=true`.

## Residual

- **403 bad-credentials after auto-resume:** re-fire is correct for this
  contract; login / token refresh UX remains separate.
- **PATH / packaging:** default `~/.grok/bin/grok` lag vs `just install`
  `grok-oss` can leave dogfood on pre-fix builds. Product `/rebuild` already
  targets the install path; plain `grok` shell entry is operator install layout.
- **Same-process idle after live error:** by design (no re-fire loop). Reopen
  or rebuild relaunch is the resume moment.

## Supersedes

Strengthens / dogfood-corrects
`.agents/reports/impl-rebuild-auto-resume-after-error-2026-08-09.md`
(product claim stands; install + wire-path proof + error re-arm are the gap).
