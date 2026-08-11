# Bug: killall does not graceful-resume like cancel-on-restart

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Operator report:** `sudo killall grok-oss` then restart did **not** resume the conversation the way network cancel / canceled-turn-on-restart is supposed to.

---

## 1. When does `canceled_turn_resume` / resume fire?

**Write path (marker on disk):** only from interactive cancel with `allow_local_rewind == true`.

| Trigger | Writes `canceled_turn_resume.json`? |
|---------|-------------------------------------|
| Esc / stop / CancelTurn UI (`do_cancel_turn` → `do_cancel_turn_for(..., true)`) | **Yes**, if mid-turn and non-empty `in_flight_prompt` + session id |
| `/rebuild` mid-turn (`rebuild.rs` calls cancel with rewind true) | **Yes** (by design: reopen re-queues once) |
| Fearless global pause (`Ctrl+Shift+Space`, rewind false) | **No** (in-process stash only) |
| Soft stop (`Ctrl+Shift+S`) | **No** |
| Successful turn end | **Clears** marker if present |
| Clean Quit / first SIGTERM graceful quit | **No** cancel marker |

Marker module: `crates/codegen/xai-grok-shell/src/session/canceled_turn_resume.rs`.
Docs and comments are explicit: **only explicit user cancel (Esc / stop)**, reason enum is only `UserCancel`. Not network blips, not fearless pause, not finished work.

**Read path (auto re-queue once):** session hydrate in
`crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs`
when `[ui] resume_canceled_turn_on_restart` is on (default true), marker valid, and the session is **not** already adopting a live running prompt from the leader. Then: enqueue prompt front, clear marker, toast `"Resuming canceled turn..."`.

Config: `UiConfig::resume_canceled_turn_on_restart_enabled()` in `xai-grok-shared` `ui_config.rs`. User-guide: `17-sessions.md`, `05-configuration.md`.

---

## 2. Unclean process death (SIGTERM, SIGKILL, killall)

**Default `killall grok-oss` = SIGTERM** (not SIGKILL unless `-9`).

### SIGTERM / first graceful signal (SIGINT/SIGTERM/SIGHUP)

`signal_handler.rs`: first signal → `QUIT_NOTIFY` → event loop dispatches **`Action::Quit`** (unregister `active_sessions`, `Effect::Quit`, break). Second signal → hard terminal restore + exit (143 for SIGTERM).

What that path does **not** do:

- Does **not** call `do_cancel_turn_for`
- Does **not** write `canceled_turn_resume.json`
- Does **not** soft-cancel the in-flight turn as Esc would

So reopen has **no** cancel-resume marker → **no** auto re-queue, even with the setting on.

### SIGKILL / `killall -9`

No handlers. No unregister (until next launch `collect_crashed` drops dead PIDs from `active_sessions.json`). No marker. Terminal may stay raw/alt-screen (crash handler covers SEGV/BUS, not SIGKILL).

### Session load after death

- Transcript: still from `updates.jsonl` / session dir (durable).
- Unsent composer draft: separate `unsent_prompt_draft` if it was flushed.
- Plan body: on-disk `plan.md` survives; **soft-park UI** (`PlanApprovalViewState`) is **in-process** and dies with the TUI. Reopen does not re-raise the same parked CTAs unless leader replay / a new `exit_plan_mode` arrives.
- Mid-turn agent work: dies with the process (or leader drain path if only client dies and leader still lives). No invent-work from cancel-resume without the marker.
- Live leader still holding a running prompt: load may **adopt** that prompt (`running_prompt_id` / `adopt_running_prompt`); that is attach-to-live, not cancel-resume.

---

## 3. Gap vs operator expectation

| Expectation after killall + reopen same session | Actual product |
|-------------------------------------------------|----------------|
| Re-queue mid-turn once like Esc cancel-resume | **No** — marker never written on SIGTERM/quit |
| Or at least load transcript without inventing work | **Yes** for transcript; idle, no fake resume |

**Gap (one line):** cancel-resume is **Esc/stop only**; process kill / SIGTERM graceful quit restores the terminal and unregisters the session but **never** soft-cancels mid-turn or writes the restart marker, so reopen cannot re-queue.

Operator mental model conflates “network cancel / restart resume” with “kill the binary.” Those are different seams. `/rebuild` is the intentional path that **does** cancel-with-marker then re-exec.

---

## 4. Smallest product fix directions

1. **SIGTERM / first graceful quit while turn running:** before `Action::Quit`, if `is_turn_running`, call the same marker write as interactive cancel (`build_user_cancel_marker` + `write_canceled_turn_resume` from `in_flight_prompt`), optionally send cancel to the agent. Prefer reusing `do_cancel_turn_for(..., true)` only if cancel can finish without hanging the quit path; otherwise write marker first (cheap, sync), then best-effort cancel, then quit.

2. **New `CancelResumeReason` (optional):** e.g. `ProcessShutdown` so toast/docs can distinguish Esc vs kill/rebuild. Not required for re-queue; `should_auto_resume_on_restart` currently requires `UserCancel` only — either keep writing as `UserCancel` for behavior parity or widen the gate + tests.

3. **Do not rely on atexit alone for SIGKILL:** atexit/finalizer never runs on SIGKILL. Accept that `-9` cannot mark; document. SIGTERM is the killall default and is fixable.

4. **Second signal / force exit:** if first signal already wrote the marker, force path is fine. If force fires without graceful arm, consider writing marker in `request_graceful_or_exit` only when app state is reachable (hard from pure signal tail without shared mid-turn snapshot). Practical approach: **snapshot in-flight prompt + session id into statics** on turn start (like `CURRENT_SESSION_ID`), so signal path can write the marker without full `AppView`.

5. **Tests:** unit/integration that “SIGTERM while turn running leaves marker and session load re-queues once”; red first under TDD. Extend `canceled_turn_resume` tests if reason enum grows.

6. **Docs:** user-guide `17-sessions` should say killall/SIGTERM is **not** cancel-resume today (or after fix, that SIGTERM mid-turn is treated like cancel once).

**Out of scope for smallest fix:** inventing resume for turns that finished cleanly; fearless pause; plan soft-park full restore (separate durability if wanted).

---

## Key code pointers

| Concern | Path |
|---------|------|
| Marker write/load/clear | `xai-grok-shell/src/session/canceled_turn_resume.rs` |
| Interactive cancel write | `xai-grok-pager/src/app/dispatch/turn.rs` (`do_cancel_turn_for`) |
| Session open resume | `xai-grok-pager/src/app/dispatch/session/load.rs` (~965–991) |
| SIGTERM → Quit | `xai-grok-pager/src/app/signal_handler.rs` + `event_loop.rs` quit_notify |
| Rebuild cancel+relaunch | `xai-grok-pager/src/app/dispatch/rebuild.rs` |
| Setting default on | `xai-grok-shared/src/ui_config.rs` |

---

## Bottom line

`resume_canceled_turn_on_restart` only re-queues when an **explicit cancel** already left `canceled_turn_resume.json`. **`sudo killall grok-oss` is SIGTERM graceful quit, not Esc cancel**, so no marker and no resume. Fix is to treat mid-turn process shutdown (at least SIGTERM / graceful quit) like cancel for the durable marker, without inventing work for clean idle exits.
