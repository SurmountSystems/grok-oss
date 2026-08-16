# Fork docs fix (independent review must-fix + nits)

**Date:** 2026-08-15  
**Source:** `.agents/reports/fork-docs-review.md`

## What changed

### Must-fix

`doc/dev/upstream-regression-filters.md` operator cheat sheet class 5 now
uses the same cargo already printed in Required land inventory class 5 plus
5b. A land agent who copy-pastes only that minimum block now also runs
after-burner, Business/Team pick and credential order, stale-flock /
never-writes-tokens / billing hub, both combined-remaining tests, and
`active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`.

Required land class 5 / 5b tables were left in place. No new identifiers.
No rustc or empty-cache cargo land.

### Nits

1. Catalog class 3 heading is now
   `### 3. grok-oss SQL extras (Token Economy ledger /spend; not SuperGrok dollar credits)`.
2. After-burner catalog contract now says “out of included SuperGrok period
   limits mark” instead of “out-of-allowance mark.” The `fn` name is unchanged.
3. Land-extras parentheticals now name pause / Clear finished, always-three-layer
   product prompt, and user-guide hop / spend-order in:
   - host `~/.agents/skills/git-recon/SKILL.md` (`recon:land` and the loop
     land step that repeats the same extras list)
   - `justfile` `upstream-land-filters`
   - `docs/upstream-history.md` review checklist item
4. Catalog extra tables (and matching extra cargo / cheat-sheet extra cargo)
   now include the FORK extra names that were missing:
   - Plan: `empty_enter_on_revise_prompt_does_not_approve`,
     `soft_park_empty_ctrl_c_abandons_plan_approval`,
     `exit_plan_mode_shows_overlay_even_in_yolo`
   - SHA: `build_fail_does_not_signal_leaders`
   - Pause: `idle_with_subagents_paints_pause_and_stop_hits`,
     `global_paused_idle_paints_resume_not_stop`

## Assert marker still matches class 3 heading

**Yes.**

`scripts/assert-process-pins.sh` `LAND_CLASS_MARKERS` still has the unchanged
string `### 3. grok-oss SQL extras`. The catalog heading is
`### 3. grok-oss SQL extras (Token Economy ledger /spend; not SuperGrok dollar credits)`.
The assert uses `grep -F` on the catalog body, so the heading still contains
that prefix. `LAND_CLASS_MARKERS` was not edited.

## What was skipped

- `FORK.md` was not edited. After the cheat-sheet expand there is no
  contradiction with the catalog. Residual `show_limits` /
  `format_supergrok_session` / dual `/limits` JSON names stay in the FORK
  hop neighbor block, not as Required land class 5 hop keys.
- Did not add rustc 1.97.1 or empty `models_cache.json` as cargo land.
- Did not enroll honesty leftovers that still have no matching `fn`.
- Did not run cargo or mutate git.
