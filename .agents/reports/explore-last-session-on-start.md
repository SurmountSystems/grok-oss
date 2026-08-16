# Explore: last session on start (not continue-interrupted-turn)

**Date:** 2026-08-13  
**Mode:** read-only diagnosis. Product restore is a separate implement slice.

## Operator-visible contract

When you start `grok-oss` with a remembered last session, that session opens. No welcome screen. No session picker.

First-ever use, or no last session on disk for this working directory: welcome or picker is fine.

This is **not**:

- Continue interrupted turn (`canceled_turn_resume.json`, re-queue a turn that died mid-work)
- The `/resume` session picker
- Cancel-resume re-fire of a finished prompt

## Verdict: dropped

Source does **not** auto-open the last session on a plain `grok-oss` start. Live binaries being old is not the only problem. The 1.0.3 restack left upstream default: cold start is `NewAuto` and the event loop does nothing, so the welcome screen shows.

FORK.md does **not** currently pin this seam by name. The operator named it as prior Surmount product. Restore it; do not invent a new picker.

## First read of source

| Path | What it does today |
|------|--------------------|
| `session_startup_intent_from_flags` | No flags → `SessionStartupIntent::NewAuto`. `-c` / `--continue` or bare `--resume` → Resume most-recent for cwd. |
| `materialize_startup_for_cwd` | `NewAuto` stays `MaterializedStartup::NewAuto`. Most-recent lookup only on explicit continue/resume. Missing continue target **errors** ("No session found..."). |
| Interactive event loop | `MaterializedStartup::NewAuto => None` → no `LoadSession`. Welcome. |
| Headless `-p` | `NewAuto` opens a **fresh** session. User-guide says that on purpose. Do not change. |
| `pager-bin` `last_session_id` | ACP leader reconnect cache. Not cold-start last session. |
| `ui_config.resume_canceled_turn_on_restart` | Continue interrupted turn. Wrong product. |
| User-guide `17-sessions.md` | "When you launch `grok`, the welcome screen lists recent sessions." Upstream copy. |

Existing test that encodes the dropped default: `intent_default_is_new_auto` (CLI flags only; keep this). The missing contract is **materialize / TUI**: NewAuto plus a last session on disk must become Resume.

`--continue` already implements the lookup (`most_recent_session_id` → `list_summaries(Some(cwd))`, sorted by `last_active_at` else `updated_at`). Restore is: interactive NewAuto uses that lookup and **falls back** to welcome when none exist. Do not make missing last session a hard error (that is `--continue`).

## What the implementer should change

1. Add `open_last_session_on_start` on `MaterializeCtx` (interactive `from_pager_args` true; headless ctx false).
2. On `NewAuto`, if that flag is on, not `--worktree`, not `--chat`: if a most-recent local session exists for cwd, materialize `Resume`. Else stay `NewAuto` (welcome).
3. Red/green tests in `session_startup.rs` (fixture + `GROK_HOME` serial). Do not change headless default.
4. Leave dirty continue-interrupted-turn / rebuild / `--version` edits alone unless they do not compile.

## Half-edits from the previous L2 (wrong product)

Uncommitted tree has large `dispatch/session/load.rs` additions for mid-work / error-class auto-resume (`session_looks_interrupted_mid_work`, `try_auto_resume_error_idle_on_reopen`). Also rebuild relaunch and `--version` without TTY. Those are **not** last-session-on-start. They look structurally complete (function bodies, tests). Do not finish that product in this slice.

## Leftover

After source restore: live TUI stays old until rebuild/install.
