# Process mop: turn-row Revising / Waiting paint

**Date:** 2026-08-13  
**Role:** process mop only. No new product.  
**Package:** `xai-grok-pager`  
**Target dir:** `/home/hunter/.cache/grok-oss-mop-turn-row-target`  
**TMPDIR:** `/home/hunter/.cache/grok-oss-tmp`

Primary implementer report: `.agents/reports/bug-plan-turn-row-revising-copy.md`

## Commands

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-turn-row-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
```

### 1. Clippy

```
cargo --offline clippy -p xai-grok-pager --lib -- -D warnings
```

- First run: killed at 300s (cold compile of the isolated target).
- Retry incremental: **CLIPPY_EXIT:0** (finished in 2m 06s).

No clippy warnings. No mop edits.

Did not run `cargo fmt -p xai-grok-pager`.

### 2. Named turn-row paint tests

```
cargo --offline test -p xai-grok-pager --lib -- \
  after_revise_idle_turn_row_paints_revising_plan \
  after_clarify_idle_turn_row_paints_waiting_for_updated_plan \
  after_approve_idle_turn_row_does_not_rearm_plan_written \
  after_quit_idle_turn_row_does_not_rearm_plan_written \
  busy_rewrite_turn_row_yields_to_real_turn_status \
  new_present_turn_row_is_review_park_not_approve
```

- First run: killed at 300s (cold test compile).
- Retry incremental: **TEST_EXIT:0** (compile 2m 39s; tests 0.07s).

```
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 8854 filtered out
```

| Test | Result |
|------|--------|
| `after_revise_idle_turn_row_paints_revising_plan` | ok |
| `after_clarify_idle_turn_row_paints_waiting_for_updated_plan` | ok |
| `after_approve_idle_turn_row_does_not_rearm_plan_written` | ok |
| `after_quit_idle_turn_row_does_not_rearm_plan_written` | ok |
| `busy_rewrite_turn_row_yields_to_real_turn_status` | ok |
| `new_present_turn_row_is_review_park_not_approve` | ok |

## Edits

None. Clippy and the six tests were green. No fallout to mop. Did not touch pause chips or Clear finished paint.

## Leftover

Live TUI still needs a rebuild/install before the operator sees the new turn-status row.

Stop.
