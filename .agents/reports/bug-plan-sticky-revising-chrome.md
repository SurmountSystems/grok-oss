# Restore plan sticky + Revising chrome (1.0.3 restack)

**Date:** 2026-08-13  
**Package:** `xai-grok-pager`  
**Diagnosis:** `.agents/reports/fork-gaps-remaining-seams-2026-08-13.md`

## Result

**Shipped.** After the 1.0.3 restack dropped `plan_decision_resolved` and
`plan_feedback_in_flight`, those identifiers and the Revising status helper
are back.

1. After one decisive Approve or Quit, `plan_decision_resolved` stays true
   until a new `exit_plan_mode` present. `should_arm_plan_decision_chrome()`
   is false, so Approve / idle "Plan written. Click or /view-plan" cannot
   re-arm for the same plan when `CurrentModeUpdate` clears pending while
   plan mode is still active.
2. After decisive Revise or Clarify unparks, `plan_feedback_in_flight` is
   Revising or Clarifying. Idle status helper returns **Revising plan...** or
   **Waiting for updated plan...**, never **Plan written. Click or /view-plan**.
   Busy rewrite yields `None` so real turn status can paint.
3. `handle_exit_plan_mode` clears both flags, then parks. Tool success is
   present for review, not operator Approve.

`exit_plan_mode` present still auto-opens the side panel. Parked turn-row
copy is still **Waiting on plan approval** (this slice did not rewrite
`render.rs`).

## TDD

### Red (observed)

Named tests were restored first. They failed to compile because the
identifiers and status chrome were missing.

```bash
TMPDIR=/home/hunter/.cache/grok-build-tmp cargo test -p xai-grok-pager --lib -- \
  after_approve_current_mode_clears_pending after_revise_status_is_revising \
  after_revise_in_flight new_exit_plan_mode_present after_clarify_status \
  approved_and_implemented after_quit_current_mode re_present_after_revise \
  exit_plan_mode_present_is_not_operator
```

**Fail reason:** `error: could not compile xai-grok-pager (lib test) due to 35 previous errors`.
Unknown fields `plan_decision_resolved` / `plan_feedback_in_flight`. Missing
methods `should_arm_plan_decision_chrome`, `plan_loop_status_label`,
`effectively_in_plan_mode`, `clear_plan_loop_flags_for_new_present`. Unresolved
imports `PlanFeedbackInFlight`, `PLAN_REVISING_STATUS`,
`PLAN_WAITING_UPDATED_STATUS`, `PLAN_IDLE_REVIEW_STATUS`.

### Green (same filters after product)

Same command: **10 passed** (9 in `plan_sticky_and_revising_chrome_tests` +
`new_exit_plan_mode_present_clears_decision_resolved_and_in_flight` on the
real `handle_exit_plan_mode` path).

Nearby five-CTA / empty-Enter / optimistic-mode tests still pass (14 when
combined with those named filters).

## Product changes

| File | Change |
|------|--------|
| `views/plan_approval_view.rs` | `PlanFeedbackInFlight`, `PLAN_REVISING_STATUS`, `PLAN_WAITING_UPDATED_STATUS`, `PLAN_IDLE_REVIEW_STATUS` |
| `app/agent_view/mod.rs` | Fields `plan_decision_resolved`, `plan_feedback_in_flight` |
| `app/agent_view/session.rs` | Init `false` / `None` |
| `app/agent_view/plan.rs` | `should_arm_plan_decision_chrome`, `plan_loop_status_label`, set sticky on Approve/Quit, set in-flight on Revise/Clarify, `/view-plan` stays view-only when chrome must not re-arm; named tests |
| `app/acp_handler/interactions.rs` | New present calls `clear_plan_loop_flags_for_new_present` |
| `app/acp_handler/tests/plan_mode.rs` | Present-path test that the real handler clears both flags |

Did **not** edit `app/agent_view/render.rs` (pause chips writer still live).
Status SoT for tests is `AgentView::plan_loop_status_label`.

## Verify

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
# named tests: 10 passed
```

Clippy `--lib` is clean. Helpers are used from `show_plan_preview` and the
Revise/Clarify paths so they are not dead in the non-test lib.

## Leftover (not this slice)

- **Ctrl+C abandon** is still dropped. Plan overlay still does not call
  `abandon_plan` on empty-composer Ctrl+C. Queued separately; same files.
  Do not restore here.
- **Turn-row paint** of Revising / Plan ready still lives in `render.rs`.
  Until that file can be edited, the live parked label stays **Waiting on
  plan approval**. Idle Revising is the helper, not the painted turn row.
- **Idle local-decision park** (`park_local_idle_plan_decision_if_needed`)
  is still gone. Sticky + `should_arm` are the gate for when that returns.
- **Honest queue toast** while the plan-feedback channel is closed
  (`PLAN_FEEDBACK_QUEUE_TOAST`) is not restored.
- **Revise barren-wait landing** (always push a human line, clear ghost
  composer draft) is not restored.

## Out of scope

- No `git add` / `git commit`.
- No `views/prompt_widget/**`, no `settings/defs.rs`, no `views/agent.rs`.
