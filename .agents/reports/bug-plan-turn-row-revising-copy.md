# Paint Revising / Waiting on the turn-status row

**Date:** 2026-08-13  
**Package:** `xai-grok-pager`  
**Host L3:** no `spawn_subagent` on this host. Work stayed in this implementer thread.

## Named contract

1. After decisive Revise, the idle turn-status row paints **Revising plan...**, not **Waiting on plan approval**, not idle **Plan written. Click or /view-plan**.
2. After decisive Clarify, the same row paints **Waiting for updated plan...**.
3. After decisive Approve or Quit, do not re-arm Approve or idle Plan written for the same present.
4. Busy rewrite yields to real turn status (`plan_loop_status_label` is `None` when the turn is running).
5. A new `exit_plan_mode` present is review park, not operator Approve.

## Red (observed)

Paint tests were added first. They read the **drawn** buffer, not only the helper.

```bash
CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-plan-turn-row-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-pager --offline --lib -- \
  after_revise_idle_turn_row_paints_revising_plan \
  after_clarify_idle_turn_row_paints_waiting_for_updated_plan \
  after_approve_idle_turn_row_does_not_rearm_plan_written \
  after_quit_idle_turn_row_does_not_rearm_plan_written \
  busy_rewrite_turn_row_yields_to_real_turn_status \
  new_present_turn_row_is_review_park_not_approve
```

**Fail reason:** `after_revise_idle_turn_row_paints_revising_plan` and `after_clarify_idle_turn_row_paints_waiting_for_updated_plan` panicked. Idle after Revise/Clarify left no turn-status row. The screen had only the toast (`Plan revision sent.` / `Clarify sent...`) and the composer. The helper already returned the right strings; `render.rs` only painted `plan_approval_status_label` while parked, and `turn_status_height` was 0 when idle with no watchers.

The other four paint tests were already green (no-re-arm after Approve/Quit, busy yield, parked present).

## Green (same filters after product)

Same command: **6 passed**.

Nearby helper / pause / Clear finished filters (13 tests) still pass, including `pause_button_click_dispatches_global_pause_not_cancel` and `open_todo_with_finished_paints_clear_even_when_unfocused`.

```bash
cargo clippy -p xai-grok-pager --offline --lib -- -D warnings
# CLIPPY_EXIT:0
```

Did not run `cargo fmt -p xai-grok-pager`.

## Product changes

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Allocate the turn-status row when `plan_loop_status_label()` is `Some`. Paint that label (Revising / Waiting for updated / parked wait) instead of only `plan_approval_status_label` while `plan_approval_view` is live. Busy helper `None` still falls through to `render_turn_status` (pause chips unchanged). Named paint tests in `plan_turn_row_revising_copy_tests`. |

Did not edit `plan.rs`, `event_loop.rs`, `views/turn_status.rs`, user-guide, Clear finished selection, settings, caret, spend, or welcome.

## Leftover

- Parked present copy is still **Waiting on plan approval** (that is what `plan_approval_status_label` returns). Soft-park **Plan ready. Side panel open** is not this slice.
- Ctrl+C abandon, idle local-decision park, honest queue toast, and Revise barren-wait landing stay out of this file as before.
- Live TUI still needs a rebuild/install before the operator sees the new row.

## Out of scope

- No `git add` / `git commit`.
- No `docs/user-guide/**`.
