# Process mop — plan leftover chrome

**Date:** 2026-08-13
**Tag:** `[process-mop]`
**Package:** `xai-grok-pager`
**Primary:** `.agents/reports/bug-plan-remaining-chrome-leftovers.md`

Backup only. Re-ran clippy and the named leftover filters. SuperGrok is paid. This report never says "free SuperGrok."

## Environment

`/tmp` tmpfs is full. Commands used:

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-plan-leftover-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
```

Cold compiles died at the 300s wrapper. Incremental retries finished.

## Clippy

```bash
cargo --offline clippy -p xai-grok-pager --lib -- -D warnings
```

| Attempt | Result |
|---------|--------|
| 1 (cold) | Wrapper killed at 300s while still compiling deps / `xai-grok-pager` |
| 2 (incremental) | Finished in 22.07s. **CLIPPY_EXIT:0** |

No clippy warnings. No product edits.

Crate-wide `cargo fmt -p xai-grok-pager` was not run (another writer owns dual-auth hop).

## Named tests

```bash
cargo --offline test -p xai-grok-pager --lib -- \
  plan_approval_status_label_distinguishes_empty \
  new_present_turn_row_is_review_park_not_approve \
  exit_plan_mode_present_is_not_operator_approve \
  park_local_idle_plan_decision \
  in_flight_followup_shows_plan_feedback_queue_toast \
  after_revise_empty_always_pushes_human_scrollback_line \
  after_revise_clears_composer_no_ghost_stash_draft \
  line_viewer_empty_ctrl_c_abandons_plan_approval \
  line_viewer_ctrl_c_clears_draft_then_second_abandons
```

| Attempt | Result |
|---------|--------|
| 1 (cold test profile) | Wrapper killed at 300s while compiling the test graph |
| 2 (incremental) | Finished in 2m 26s. **TEST_EXIT:0** |

**11 passed; 0 failed.** 8857 filtered out. Finished in 0.06s.

Matching tests:

- `views::plan_approval_view::tests::plan_approval_status_label_distinguishes_empty`
- `app::agent_view::plan::plan_remaining_chrome_leftover_tests::park_local_idle_plan_decision_skips_when_chrome_must_not_arm`
- `app::agent_view::plan::plan_remaining_chrome_leftover_tests::park_local_idle_plan_decision_parks_five_cta_panel`
- `app::agent_view::plan::plan_remaining_chrome_leftover_tests::park_local_idle_plan_decision_skips_when_already_parked`
- `app::agent_view::plan::plan_remaining_chrome_leftover_tests::after_revise_empty_always_pushes_human_scrollback_line`
- `app::agent_view::plan::plan_sticky_and_revising_chrome_tests::exit_plan_mode_present_is_not_operator_approve`
- `app::agent_view::plan::plan_remaining_chrome_leftover_tests::in_flight_followup_shows_plan_feedback_queue_toast`
- `app::agent_view::plan::plan_remaining_chrome_leftover_tests::after_revise_clears_composer_no_ghost_stash_draft`
- `app::agent_view::viewer::tests::line_viewer_empty_ctrl_c_abandons_plan_approval`
- `app::agent_view::viewer::tests::line_viewer_ctrl_c_clears_draft_then_second_abandons`
- `app::agent_view::render::plan_turn_row_revising_copy_tests::new_present_turn_row_is_review_park_not_approve`

Nine filters, eleven matching tests. All green. No mop of product files.

## Edits

None. Clippy and named tests were already green after incremental retry.

## Leftovers (not this mop)

- Live TUI is still the old binary until a rebuild/install. This mop did not run `/rebuild`.
- PTY e2e was not re-run.

## Stop

Clippy exit 0. Named leftover tests 11/11 pass. Mop done.
