# Fix: plan_panel_click_clarify_revise_quit_buttons after decisive Revise

**Date:** 2026-08-09
**Package:** `xai-grok-pager`
**File:** `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs`
**Related:** `.agents/reports/impl-plan-revise-stuck-2026-08-09.md`

## Result

**Fixed.** Test expectations updated to the named product contract (Revise is decisive). No product revert.

## Failure

```
plan_panel_click_clarify_revise_quit_buttons ... FAILED
plan.rs:4307: called `Option::unwrap()` on a `None` value
```

Revise click cleared `plan_approval_view` via `request_plan_revise` → `send_plan_feedback`. The test still unwrapped park and asserted focus-only:

```rust
let pav = agent.plan_approval_view.as_ref().unwrap();
assert_eq!(pav.focus, PlanApprovalFocus::Prompt);
assert_eq!(pav.prompt_intent, PlanPromptIntent::Revise);
```

## Named contract (unchanged product)

| CTA | Behavior |
|-----|----------|
| **Clarify** | Focus Prompt + `PlanPromptIntent::Questions` (still focus-only) |
| **Revise** | Immediate `request_plan_revise` → ACP `cancelled`, clear park, close panel |
| **Quit** | Abandon → ACP `abandoned` |

Evidence: `viewer.rs` panel footer `send_area` → `request_plan_revise()`; soft-park/panel-s tests already green for decisive revise.

## Test change

Revise branch now asserts (stronger or equal to old focus check):

1. `plan_approval_view.is_none()` — park cleared
2. `line_viewer.is_none()` — panel closed
3. ACP outcome `"cancelled"` via `parse_outcome(rx)`

Clarify and Quit branches unchanged. Doc comment updated to name the contract.

**Did not:** revert product to focus-only; delete the test; weaken Clarify/Quit.

## Commands

```bash
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib -- plan_panel_click_clarify_revise_quit_buttons
# ok

cargo test -p xai-grok-pager --lib -- approve_plan_flush_tests
# 97 passed; 0 failed
```

## Out of scope

- No git add/commit/push
- No product code change
- No clippy package-wide (test-only assert edit; fmt applied)
