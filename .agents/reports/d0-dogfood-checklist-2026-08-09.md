# D0 dogfood checklist (operator)

**Date:** 2026-08-09
**Why:** Code can be green in the tree while every open TUI still runs an old binary (deleted inode). Do not treat chrome or stale-plan as "live" until this gate passes.

## Steps

1. **Install current tree** into the dogfood binary:
   - From repo: `just install`
   - Or inside TUI after code is built: `/rebuild`
   - Prefer the Surmount binary name **`grok-oss`**, not official `grok`.

2. **Quit every** Grok OSS / Grok TUI window on this machine (including other projects).
   If `ps` / process list shows `(deleted)` next to the binary path, those processes are still the old build.

3. **Reopen only** `grok-oss` for the sessions you care about.

4. **Verify stale plan fix:** in one turn, rewrite session `plan.md` and call `exit_plan_mode` (or use plan mode revise + present). The plan panel body must match the **new** disk content, not an older plan title.

5. **Verify cancel-resume:** finish or cleanly cancel a turn so the session is idle. Reload or reopen. It must **not** re-send an old prompt by itself.

6. **Optional path check:** `grok-oss limits --json` (or multipoll). While free SuperGrok period limits still have room, intent should stay on free SuperGrok period (not console primary). Flat free SuperGrok period % under real SuperGrok work is a **server C4** ticket, not a reason to invent % on the client. Paste-ready notes live under `.agents/reports/c4-*`.

## Agent work in parallel

While you run D0, implementers land Work A (composer Enter cue), then C (meters), B (pause/stop), E (flaky test + naming). Those need another install after they land to dogfood the new chrome.
