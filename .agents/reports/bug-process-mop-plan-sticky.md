# Process mop: plan sticky / Revising restore

**Date:** 2026-08-13  
**Role:** `[process-mop]`  
**Primary:** `.agents/reports/bug-plan-sticky-revising-chrome.md`  
**Package:** `xai-grok-pager`  
**Target dir:** `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-plan-sticky-target`  
**TMPDIR:** `/home/hunter/.cache/grok-oss-tmp`

Host L2 has no `spawn_subagent` on this worker. Mop ran here. Did not compact-and-continue.

## Commands

1. **Fmt:** skipped. Instruction: do not run `cargo fmt -p xai-grok-pager`. Primary already ran fmt+clippy clean. This mop did not edit any plan-sticky files.

2. **Clippy**

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-plan-sticky-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo --offline clippy -p xai-grok-pager --lib -- -D warnings
```

First run was killed at the 5m wrapper timeout while compiling a cold isolated target. Incremental resume:

**Exit code: 0.** Finished `dev` profile in 4m 17s. No warnings under `-D warnings`.

3. **Named tests**

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-plan-sticky-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo --offline test -p xai-grok-pager --lib -- \
  after_approve_current_mode_clears_pending after_revise_status_is_revising \
  after_revise_in_flight new_exit_plan_mode_present after_clarify_status \
  approved_and_implemented after_quit_current_mode re_present_after_revise \
  exit_plan_mode_present_is_not_operator
```

First run was killed at the 5m wrapper timeout while compiling the test profile. Incremental resume:

**Exit code: 0.** `10 passed; 0 failed; 0 ignored; 0 measured; 8844 filtered out`.

| Test | Result |
|------|--------|
| `after_approve_current_mode_clears_pending_still_in_plan_does_not_repark` | ok |
| `after_revise_status_is_revising_not_plan_written_click_or_view` | ok |
| `after_revise_in_flight_surface_does_not_rearm_idle_ctas` | ok |
| `new_exit_plan_mode_present_clears_decision_resolved_and_parks` | ok |
| `after_clarify_status_is_waiting_for_updated_plan` | ok |
| `approved_and_implemented_plan_body_does_not_repark_after_decide` | ok |
| `after_quit_current_mode_clears_pending_still_in_plan_does_not_repark` | ok |
| `re_present_after_revise_clears_in_flight_and_arms_ctas` | ok |
| `exit_plan_mode_present_is_not_operator_approve` | ok |
| `new_exit_plan_mode_present_clears_decision_resolved_and_in_flight` | ok |

No half-written Ctrl+C abandon compile failures. Clippy and tests saw a complete tree.

## Edits

None. No mop fallout. Did not touch user-guide, todo pane, Clear finished, `views/turn_status.rs`, settings, prompt_widget, spend, or welcome.

## Leftover (not this mop)

- Live TUI is still the old parked label.
- Turn-row still paints **Waiting on plan approval**. That lives in `render.rs`. Not this job.
- Ctrl+C abandon is still dropped. Queued separately.

Stop.
