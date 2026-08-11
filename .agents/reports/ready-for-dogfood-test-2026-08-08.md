# Ready for dogfood test

**Date:** 2026-08-08
**Verdict:** **YES** — rebuild, contract filters, and live limits all green. Operator can dogfood the installed binary.

---

## 1. Binary

| Field | Value |
|-------|--------|
| Path | `/home/hunter/.cargo/bin/grok-oss` |
| Version | `grok-oss 0.2.111 (c87f66a61d94) [stable]` |
| Install | `just install` (release `xai-grok-pager-bin`, strip, install) exit 0 |

---

## 2. Contract tests (all exit 0)

| Filter / scope | Crate | Result |
|----------------|-------|--------|
| `quit_mid_turn_after_first_activity_writes_cancel_resume` | xai-grok-pager | **pass** (1) |
| `quit_mid_turn_writes_canceled` | xai-grok-pager | **pass** (1) |
| `quit_idle_does_not_write` | xai-grok-pager | **pass** (1) |
| `armed_process_shutdown_writes_cancel_resume` | xai-grok-shell (not pager; filter lives under `canceled_turn_resume`) | **pass** via shell run below |
| `canceled_turn_resume` + `process_shutdown_class` | xai-grok-shell | **pass** 8/8 (includes `armed_process_shutdown_writes_cancel_resume_marker`, `process_shutdown_class_marker_is_auto_resume_eligible`) |
| `rebuild_` | xai-grok-pager | **pass** 18/18 |
| `rebuild::` | xai-grok-update | **pass** 18/18 |
| Design A: `compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars` | xai-grok-pager | **pass** |
| Design A: `active_driver_free_period_headroom_even_with_extras_and_team_prepaid` | xai-grok-pager | **pass** |
| Design A: `status_bar_free_period_headroom_not_console_prepaid_dollars` | xai-grok-pager | **pass** |

**Fails:** none.

Note: pager filter list with `armed_process_shutdown_writes_cancel_resume` matches only the three quit_* tests in pager; the armed process-shutdown marker contract is in `xai-grok-shell` and was green under the shell filter.

---

## 3. Live limits (one-liner)

`activeDriver=supergrok_free_period` · free period **6.0% used / 94% remaining** (business + personal, shared pool, live_poll) · `console.isLive=false` · SuperGrok $ extras **$100.29** · team prepaid **$340** · team postpaid OAuth class **~$1021.67** · `flatPollUnprovenDebit=true` · next free-period reset **August 10, 19:25**.

Meters kept distinct: free SuperGrok period % ≠ SuperGrok dollar extras ≠ console team prepaid ≠ team postpaid OAuth / Grok Build ≠ team default credits.

---

## 4. Ready for operator test checklist

Do these on the **new** binary after a full quit (not an in-place soft reload of an old process):

1. **Restart TUI** — quit fully, relaunch `grok-oss` (or your usual entry). Confirm version `0.2.111 (c87f66a61d94)` if useful.
2. **`/rebuild` once** — expect progress bar, no "Not the..." inherit noise, no footer shred on relaunch.
3. **Mid-implement killall resume** — start a turn that uses tools; while tools are running, `sudo killall grok-oss` (**SIGTERM**, not `-9`); reopen; expect cancel-resume of that turn (process_shutdown class).
4. **Compact status / free period** — compact chrome should show free-period **%** (still ~6% is OK and server-side), with active driver free SuperGrok period, **not** console prepaid dollars as the headline meter.

---

## 5. Still C4 / not client-fixable

| Item | Why not client |
|------|----------------|
| Free SuperGrok period stuck ~**6%** across polls while session work continues | Server absorption / unproven included debit (`flatPollUnprovenDebit`). Product must not invent free-period % climb. |
| Team OAuth / Grok Build class $ can rise under SuperGrok session without free-period % moving | Settlement path ≠ free SuperGrok period burn proof. |
| SuperGrok $ extras flat at ~$100.29 | Side meter when active driver is free period; not a client rank bug. |
| Platforms → Grok Business license message/conversation zeros | Expected; CLI SuperGrok does not drive seat counters. |

Client contracts for killall cancel-resume, `/rebuild` progress/relaunch, and free-period-first limits chrome are what this rebuild verifies. C4 ticket material stays on the xAI / server side.

---

## Commands used (for re-run)

```bash
cd /home/hunter/Projects/surmount/grok-build
just install
~/.cargo/bin/grok-oss --version

cargo test -p xai-grok-pager --lib -- \
  quit_mid_turn_after_first_activity_writes_cancel_resume \
  quit_mid_turn_writes_canceled \
  quit_idle_does_not_write

cargo test -p xai-grok-shell --lib -- \
  canceled_turn_resume process_shutdown_class

cargo test -p xai-grok-pager --lib -- rebuild_
cargo test -p xai-grok-update --lib -- rebuild::

cargo test -p xai-grok-pager --lib -- \
  compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars \
  active_driver_free_period_headroom_even_with_extras_and_team_prepaid \
  status_bar_free_period_headroom_not_console_prepaid_dollars

~/.cargo/bin/grok-oss limits --json
```
