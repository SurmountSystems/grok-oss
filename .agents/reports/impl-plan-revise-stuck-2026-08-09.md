# Fix: plan approval Revise CTA felt stuck

**Date:** 2026-08-09
**Packages:** `xai-grok-pager`, `xai-grok-shell`
**Board:** `bug:plan-revise-stuck`

## Result

**Fixed.** Clicking **Revise** (soft-park footer, side-panel footer, or empty-prompt panel `s`) now **immediately** submits ACP `cancelled`, clears `plan_approval_view`, closes the plan panel, and toasts **"Revision sent — agent will rewrite the plan."** It is no longer a silent re-set of the default Revise intent while the UI stays parked with `Enter:approve`.

## Root cause

Revise was implemented as **focus-only**:

- Soft-park mouse / panel mouse / empty-prompt `s` called `focus_plan_prompt(PlanPromptIntent::Revise)`.
- Constructor **already** defaults `prompt_intent` to `Revise`.
- So a bare Revise click did nothing visible when park was already open (common dogfood path: side panel open, "Plan ready. Side panel open", composer `Enter:approve`).
- Operator still had to type freeform and press Enter; empty Enter on the panel still **approves**. That felt stuck.

Hit-testing was fine. The agent was never notified.

## Named product contract

| Action | Behavior |
|--------|----------|
| **Revise** mouse CTA / empty-prompt panel `s` | Immediate `request_plan_revise()` → `send_plan_feedback` → ACP `cancelled`; clear park + panel; toast "Revision sent…" |
| Freeform already in composer | Attached as revise feedback |
| Empty freeform | Still unparks; shell injects a revise turn that rewrites `plan.md` from conversation (or one short question if unclear), then `exit_plan_mode` again |
| Freeform Enter (default intent) | Unchanged: still revises when prompt has text |
| Empty Enter on panel Prompt | Unchanged: still **approves** |
| **Notes** / **Clarify** | Unchanged: still focus prompt for typed input |
| Soft-park bare `s` (no panel) | Unchanged: types into composer (modal-free L1) |

No freeform chat menus invented.

## Product changes

| File | Change |
|------|--------|
| `xai-grok-pager/.../plan.rs` | `request_plan_revise()`; soft-park Revise click calls it |
| `xai-grok-pager/.../viewer.rs` | Panel empty-prompt `s` + panel footer Revise call `request_plan_revise()` |
| `xai-grok-shell/.../tool_calls.rs` | Empty revise agent message pushes rewrite + re-`exit_plan_mode` (not stall-only "ask what to change") |
| `docs/user-guide/19-plan-mode.md` | Revise documented as decisive |
| `FORK.md` | Plan CTA ship note updated |

## TDD

**Red (observed):**

- `soft_park_revise_cta_click_submits_cancelled_immediately` — failed: park still present
- `soft_park_revise_cta_click_includes_existing_freeform` — failed: no ACP response
- `panel_empty_prompt_s_submits_cancelled_immediately` — failed: park still present

**Green:** same tests pass after `request_plan_revise` wire-up.

### Commands

```bash
cargo test -p xai-grok-pager --lib -- \
  soft_park_revise_cta_click_submits_cancelled_immediately \
  soft_park_revise_cta_click_includes_existing_freeform \
  panel_empty_prompt_s_submits_cancelled_immediately \
  soft_park_revise_cta_click_after_paint \
  soft_park_all_cta_clicks_after_paint \
  soft_park_default_freeform_enter \
  soft_park_clarify_cta_then_enter \
  focus_plan_prompt_sets_intent \
  send_plan_feedback_still_submits_cancelled \
  soft_park_cta_s_types_into_composer \
  revise_intent

cargo test -p xai-grok-shell --lib -- \
  revise_plan_message_includes_feedback_when_present

cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo clippy -p xai-grok-pager --lib -- -D warnings
cargo clippy -p xai-grok-shell --lib -- -D warnings
```

All of the above: **exit 0**.

## Operator dogfood

1. `just install` (or `/rebuild`) and **quit every** old Grok window.
2. Reopen `grok-oss` only.
3. Enter plan mode → agent calls `exit_plan_mode` → side panel open.
4. Click **Revise** (or empty-prompt `s`): panel/park should clear, toast "Revision sent…", agent continues in plan mode to rewrite and re-present.

## Out of scope

- No git add/commit/push.
- Did not change empty Enter → approve on panel Prompt.
- Did not touch `live.rs` / pager-minimal turn status (`bug:pager-minimal-turn-status-api` is a separate track).
- Notes / Clarify remain focus-then-type (they need freeform).
