# Remaining-plan wave (approved 2026-08-09) — status

## Short answer

**Not everything is fixed in production until you install and fully quit old TUIs.**
**All agent code packages from the approved plan (A, B, C, E) are in the tree with TDD and docs.** D0 and F stay on you (dogfood + optional C4 ticket).

## Package status

| Work | What | Status |
|------|------|--------|
| **D0** | Install, quit deleted-inode TUIs, reopen `grok-oss` | Checklist: `d0-dogfood-checklist-2026-08-09.md` (operator) |
| **A** | Enter send vs queue when subagents hold | **Shipped** `impl-work-a-composer-enter-cue-2026-08-09.md` |
| **C** | `intent · N%` upper-right; `Team settlement:` footer | **Shipped** `impl-work-c-meters-chrome-2026-08-09.md` |
| **B** | `[pause]`/`[resume]` white; red `[stop]`; subagent-only stop | **Shipped** `impl-work-b-pause-stop-chrome-2026-08-09.md` |
| **E** | Flaky load test fixture; "continue interrupted turn" naming | **Shipped** `impl-work-e-flaky-naming-2026-08-09.md` |
| **F** | Free SuperGrok period flat burn | Operator multipoll + C4 if still flat (not invented %) |
| Stale plan (prior) | Same-batch write + exit_plan_mode | **Shipped earlier** same day; needs install to dogfood |

## How to read the new chrome (after install)

1. **Footer Enter:** says `send` when a new turn would start now; `queue` when primary is busy **or** background subagents hold drain; `interject` when empty Enter would force the top queue row. Idle + "1 subagent still running" should show queue, not send.
2. **Pause vs stop:** gray/white `[pause]` runs global pause (all sessions); red `[stop]` hard-cancels. Soft stop remains `Ctrl+Shift+S` only.
3. **Meters:** upper-right `intent · 15%` is free SuperGrok period **intent** (what Design A wants to spend first). Footer `Team settlement: prepaid $… · Grok Build class $…` is side-channel settlement, not a claim that team $ is the intent driver.
4. **Continue interrupted turn** is not `/resume` (session pick).

## Operator next steps

1. `just install` or `/rebuild`, then **quit every** old Grok window.
2. Reopen `grok-oss` only.
3. Spot-check A/B/C chrome + stale plan same-turn rewrite.
4. Optional: multipoll; if free SuperGrok period still barely steps, file C4 with existing paste-ready reports.
