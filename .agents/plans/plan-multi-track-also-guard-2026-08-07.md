# Plan: Ctrl+C leave plan, resume after kill/restart, multi-track also-guard

**Date:** 2026-08-07 (fourth revise)
**Boards:** `bug:plan-approval-ctrl-c-still`, `bug:killall-no-graceful-resume`, `feat:multi-track-also-guard`
**Explore:**
- `.agents/reports/explore-multi-track-also-guard-2026-08-07.md`
- `.agents/reports/bug-killall-no-graceful-resume-2026-08-07.md`

## Revise notes (this turn)

1. Operator rebuilt. **Watch limits** after rebuild (see section below). Free SuperGrok period still **6%** on live poll; team settlement $ still climbing. That is C4 / dual-bill honesty, not a failed chrome install.
2. **`sudo killall grok-oss` then reopen did not gracefully resume** the conversation. Today cancel-resume only runs after **explicit Esc/stop** writes `canceled_turn_resume.json`. SIGTERM/killall exits without that marker, so reopen never re-queues the mid-turn. Incorporate a real product path for process death / forced restart.
3. Keep plain English (no "untrap").
4. Order of work after this revise: (A) Ctrl+C closes plan + quit path on installed binary, (B) process kill / hard stop leaves durable resume like cancel, (C) multi-track demote guard.

## Limits after rebuild (observed 2026-08-07 ~22:58)

Installed: `grok-oss 0.2.111 (c87f66a61d94) [stable]` via `~/.cargo/bin/grok-oss`.

| Field | Value | Reading |
|-------|--------|---------|
| activeDriver | supergrok_free_period | Free period first chrome still correct |
| free SuperGrok period used | **6.0%** both principals, live_poll OK | Same as before rebuild; **not** past 6% |
| SuperGrok $ extras | ~$100.29 | Unchanged side meter |
| console.isLive | false | Design A: SuperGrok path |
| team prepaid (Management) | $340 | Distinct from browser "Credits" wallet line |
| team postpaid OAuth / Grok Build class | ~$1008 (was ~$944 earlier) | Settlement still moves without free-period % |

**Limits fixes dogfood:** free-period-first / sticky-not-console chrome is doing what we shipped. Free period **% still flat at 6%** is still the open **server C4** residual, not "rebuild failed." Continue watching on later `limits --json` samples if you want multi-sample history.

## Priority

1. **Ctrl+C on plan approval** closes plan approval; then quit TUI without external kill.
2. **Forced process stop** (`killall`, SIGTERM, crash-class exit while a turn is running): on reopen, session loads and **re-queues the interrupted turn once** the same way Esc cancel-resume does (when config allows), with a clear toast. Do not invent finished work.
3. **Multi-track board guard** after 1 to 2 are green (or in parallel if disjoint files and operator still burning less).

## Context

### Problem A: Ctrl+C ignored on plan approval

Empty Ctrl+C should close plan approval (same outcome as panel quit). Source has tests; dogfood after rebuild must prove it. Second empty Ctrl+C or normal quit must exit the process so spend stops.

### Problem B: killall does not resume the conversation

Operator path: mid-work (including plan approval or mid-turn) → `sudo killall grok-oss` → start grok-oss again → expected **graceful resume** (transcript + re-queue interrupted prompt once, like network cancel / Esc stop). Actual: no cancel-resume marker written on SIGTERM, so reopen is cold load only.

**Root gap (explore):** `canceled_turn_resume` is written only on explicit cancel/stop paths. Process signal kill does not write it. SIGKILL cannot run handlers; SIGTERM can if we handle it.

### Problem C: multi-track demote

Second ask demotes first `in_progress` to `pending` while subagent still runs. Product should reject that when `meta.taskId` is bound and still Running.

## Goal

1. Plan approval: empty Ctrl+C closes it; you can quit the TUI.
2. Process death while a turn (or durable interruptible work) is in flight: best-effort write the same durable cancel-resume marker (SIGTERM / controlled shutdown); on session load, re-queue once + toast when enabled.
3. Plan approval state on disk (`plan.md`) still restores after reopen (already file-backed; verify; fix if broken after kill).
4. Multi-track: `meta.taskId` + reject live demote + teach + optional busy toast.

**Out of scope:** invent free SuperGrok period debit; SIGKILL magic (no userspace handler); full multi-agent ownership UI.

## Approach

### Step 0: Ctrl+C and quit (installed binary)

- Prove unit tests green; dogfood after operator rebuild.
- If still broken: fix remaining swallow path + TDD.
- Quit after close works.

### Step 1: Resume after process kill / forced stop

| Piece | Intent |
|-------|--------|
| On SIGTERM (and clean quit paths that abort mid-turn) | If a top-level turn is running and would have been cancel-resumable on Esc, **write `canceled_turn_resume.json`** (same shape as Esc) before exit when possible. |
| On session load | Existing `resume_canceled_turn_on_restart` path re-queues once + toast. Keep that. Do not invent success. |
| killall | Default `killall` is SIGTERM: should hit the handler. Document that `kill -9` cannot run handlers. |
| Plan approval open | Closing process: durable `plan.md` remains; on reopen, plan approval can reappear if park was durable. Prefer: leave plan park restorable; if mid-turn also running, cancel-resume wins for the turn without inventing approve. |
| Tests | Hermetic: "mid-turn + simulated SIGTERM path writes marker"; "load re-queues once." Red then green. |
| `/rebuild` relaunch | Reuse same marker path so rebuild self re-exec and killall restart share one resume story. |

### Step 2: Multi-track also-guard

- `meta.taskId` bind; reject `in_progress` → `pending` while Running.
- Allow completed/cancelled; teach prompt; optional sticky.

## Critical files

| Path | Why |
|------|-----|
| `agent_view/plan.rs`, `viewer.rs` | Ctrl+C closes plan |
| Global Ctrl+C quit | Quit after plan closed |
| `canceled_turn_resume` + session load | Resume after restart |
| Signal / shutdown hooks (pager or shell) | SIGTERM → write marker |
| `/rebuild` self re-exec path | Same resume class |
| todo `apply_merge` | Multi-track demote reject |
| Subagent live status | Running check |

## Steps after this revise

0. Ctrl+C plan approval + quit on installed binary (dogfood).
1. SIGTERM / forced stop writes cancel-resume when mid-turn; tests; document kill -9 limit.
2. Verify plan.md park survives killall reopen.
3. Multi-track bind + demote reject + teach + optional toast.
4. Residual + FORK honesty (resume after kill; multi-track; Ctrl+C).
5. Re-check `limits --json` once after install work (note free period vs team $).

## Done when

1. Empty Ctrl+C closes plan approval on installed binary; quit works.
2. Mid-turn + SIGTERM (or product shutdown that aborts turn) → reopen re-queues once with existing toast when config on.
3. Documented: `kill -9` / SIGKILL cannot write marker.
4. Bound live demote to pending rejected.
5. Limits watch: free-period-first still honest; free period % still not invented past poll.

## Risks

| Risk | Mitigation |
|------|------------|
| SIGTERM during half tool | Same as Esc cancel: no invent success; marker only for resumable user turn |
| Double resume | Existing one-shot re-queue rules |
| kill -9 | Honest docs only |
| Free period still 6% | Not a client fail; C4 ticket |

## Verification

- Plan Ctrl+C unit tests + dogfood.
- New: mid-turn + write marker on simulated term path; load re-queues.
- Multi-track demote reject unit test.
- `grok-oss limits --json`: activeDriver free period while used &lt; 100%; note included %.

## Defaults

- First empty Ctrl+C: close plan approval.
- SIGTERM mid-turn: write cancel-resume when safe (same as Esc class).
- Multi-track after or parallel to resume work if files disjoint.
- Unbound demote still allowed in first multi-track cut.

## Critical files for implementation

- Plan key paths + global quit
- `canceled_turn_resume` + session load
- Signal/shutdown wiring
- todo merge + coordinator
- Residual / FORK when shipped

## References

- `.agents/reports/bug-plan-approval-ctrl-c-2026-08-07.md`
- `.agents/reports/bug-killall-no-graceful-resume-2026-08-07.md`
- `.agents/reports/explore-multi-track-also-guard-2026-08-07.md`
- Live limits sample after rebuild (free period 6%, team postpaid OAuth ~$1008)
