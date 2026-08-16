# Restore leftover plan-approval chrome

**Date:** 2026-08-13  
**Package:** `xai-grok-pager`  
**Host L2:** no `spawn_subagent` on this host. Work stayed in this implementer thread.

SuperGrok is paid. This report says **included SuperGrok period limits**, never "free SuperGrok."

## Named contracts

1. After a new `exit_plan_mode` present, the turn-status / plan-loop label is **Plan ready. Side panel open**, not **Waiting on plan approval**.
2. When plan mode is idle with a plan body that still needs a decision and chrome is not already armed, `park_local_idle_plan_decision_if_needed` parks the five-CTA panel. Already-parked live present and sticky / in-flight chrome are no-ops.
3. If the operator types a follow-up while Revise/Clarify is in flight, show `PLAN_FEEDBACK_QUEUE_TOAST`. Do not pretend the second note was live Revise/Clarify.
4. Clicking Revise lands a human line (`PLAN_REVISE_HUMAN_LINE` when empty), clears the composer (no ghost stash draft), and sets Revising chrome. Local idle / dead channel Interjects a rewrite so the wait is not barren.
5. If the overlay router is skipped, in-approval Ctrl+C in `viewer.rs` abandons when the composer is empty, or clears the draft then abandons. It must not return Changed and swallow the chord.

Already-restored five-CTA, sticky flags, turn-row Revising paint, and overlay Ctrl+C were not redone.

## Red (observed)

Tests were written first. First compile failed because the leftover identifiers and helpers were missing.

```bash
CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-plan-leftover-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-pager --offline --lib -- \
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

**Fail reason:** `error: could not compile xai-grok-pager (lib test) due to 11 previous errors`.

- Missing `PLAN_FEEDBACK_QUEUE_TOAST` / `PLAN_REVISE_HUMAN_LINE`
- Missing `park_local_idle_plan_decision_if_needed` / `maybe_toast_plan_feedback_queue`
- Missing field `is_local_idle_decision`

`plan_approval_status_label_distinguishes_empty` and `new_present_turn_row_is_review_park_not_approve` already encoded the new parked copy before the product string changed.

## Green (same filters after product)

Same command: **11 passed**. Nearby already-restored filters (32 when combined) still pass, including turn-row Revising paint, overlay Ctrl+C abandon, sticky Approve/Quit, empty Enter never approves.

```bash
cargo clippy -p xai-grok-pager --offline --lib -- -D warnings
# CLIPPY_EXIT:0
```

Fmt only the touched files (not crate-wide `cargo fmt -p xai-grok-pager`).

## Product changes

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/plan_approval_view.rs` | Parked label **Plan ready. Side panel open**. Constants `PLAN_FEEDBACK_QUEUE_TOAST`, `PLAN_REVISE_HUMAN_LINE`, `IDLE_PLAN_DECISION_TOOL_CALL_ID`. Field `is_local_idle_decision` + `for_idle_decision`. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` | `park_local_idle_plan_decision_if_needed`, `dismiss_plan_approval_after_turn_if_stale`, `surface_idle_plan_review_if_needed`, `maybe_toast_plan_feedback_queue`. Revise always pushes a human line, clears the composer, Interjects when there is no ACP channel. Named leftover tests. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` | In-approval Ctrl+C routes to `handle_plan_feedback_key` instead of swallowing Changed. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/viewer_tests.rs` | Direct `handle_line_viewer_key` Ctrl+C tests (overlay skipped). |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | New-present paint test expects **Plan ready. Side panel open**. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs` | Keep live park on turn-end; surface idle five-CTA. Honest queue toast when a follow-up is sent while rewrite is in flight. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/turn.rs` | Same stale-dismiss + idle park after viewer turn finalize. |
| `crates/codegen/xai-grok-pager/src/app/turn_completion.rs` | Same stale-dismiss + idle park after viewer terminal apply. |
| PTY wait strings | `plan_revise_empty_enter_does_not_approve.rs`, `plan_scrollbar_grab_zone_pty.rs`, `plan_approval_resume.rs` wait for the new parked copy. Not re-run this turn. |

## Leftovers not finished

- Soft-park footer strip paint and parked plan card (`PLAN_CARD_*`, `PLAN_PARKED_TOAST`) were not this slice.
- Clarify still does not force a human line (only Revise was named).
- PTY e2e was not executed (tmpfs / long harness). Wait strings were updated.
- Live TUI still needs a rebuild/install before the operator sees the new copy.
- User-guide was not edited.

## Out of scope

- No `git add` / `git commit` / `git push`.
- No `/spend`, settings catalog, last-session, dual-auth hop, or `/rebuild`.
