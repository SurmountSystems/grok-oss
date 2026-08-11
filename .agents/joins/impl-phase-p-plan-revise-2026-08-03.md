# Join: Phase P — plan revise vs clarify

**Date:** 2026-08-03
**Crate:** `xai-grok-pager` (plus incidental compile unblock in `xai-grok-tools`)

## Goal

Operator revise feedback must rewrite the plan. It must not land as ACP
`"questions"` (answer-only clarify) by mistake.

## Inventory (CTA → wire outcome)

| Operator action | `PlanPromptIntent` | Wire `outcome` | Shell behavior |
|-----------------|--------------------|----------------|----------------|
| Approve (`a` / empty Enter) | n/a | `"approved"` | Leave plan mode, implement |
| Approve w/ comment (`A`) | `ApproveNotes` | `"approved"` + notes Interject | Leave plan mode |
| **Revise** (`s` / footer Revise) | `Revise` (default) | **`"cancelled"`** | Stay in plan mode; inject revise turn (`revise_plan_message`) |
| **Clarify** (`?` / footer Clarify) | `Questions` | **`"questions"`** | Stay in plan mode; answer-only (`questions_plan_message`) |
| Quit (`q`) | n/a | `"abandoned"` | Leave plan mode, no implement |

**Dispatch anchors**

- Soft-park footer paint + hits: `plan_approval_view::paint_soft_park_cta_buttons` / `handle_soft_park_cta_click`
- Side panel footer: `line_viewer` areas → `focus_plan_prompt(Revise|Questions|…)`
- Prompt Enter: `handle_plan_feedback_key` → `prompt_intent` match → `send_plan_feedback` (cancelled) or `send_plan_questions` (questions)
- Wire send helpers: `PlanApprovalViewState::send_cancelled` / `send_questions`
- Shell: `PlanApprovalOutcome::from_response` in `xai-grok-shell` `tool_calls.rs`

No product mis-route was found in the hot path: Revise already defaulted to
`PlanPromptIntent::Revise` and Enter under that intent already called
`send_plan_feedback` → `"cancelled"`. Clarify alone already called
`send_questions` → `"questions"`.

## Work done

### Regression tests (red contract → green observed)

End-to-end via `handle_plan_feedback_key` / soft-park CTA click, not only the
send helpers:

1. `revise_intent_freeform_plus_line_comments_submits_cancelled_not_questions`
   Freeform + saved line comments under Revise → `"cancelled"`, not
   `"questions"`, with `@plan.md:N` + quoted line + freeform.
2. `revise_intent_question_shaped_freeform_still_submits_cancelled`
   Freeform that ends with `?` under Revise still rewrites (intent wins over
   wording).
3. `soft_park_revise_cta_then_enter_submits_cancelled`
4. `soft_park_clarify_cta_then_enter_submits_questions`
5. `soft_park_default_freeform_enter_submits_cancelled_not_questions`
   Default park intent is Revise (typing freeform without a CTA still rewrites).
6. `plan_card_copy_distinguishes_revise_from_clarify`
   Operator-facing card/placeholder copy names rewrite vs answer-only.

**Command (green):**

```bash
cargo test -p xai-grok-pager --lib -- \
  revise_intent_freeform_plus_line_comments \
  soft_park_revise_cta_then_enter \
  soft_park_clarify_cta_then_enter \
  soft_park_default_freeform \
  plan_card_copy_distinguishes \
  revise_intent_question_shaped \
  send_plan_feedback_still \
  send_plan_questions_submits \
  parked_plan_card_has_no
```

Result: **9 passed**.

### UX copy (plain English)

Where operators see it:

- Soft-park transcript card pointer (`PLAN_CARD_CTAS`) and empty-plan placeholder:
  "Revise rewrites the plan" / "Clarify answers without rewriting".
- Enter shortcut hints while drafting: `revise (rewrites plan)` vs
  `clarify (no rewrite)`.
- Toasts after submit: "Revision sent — agent will rewrite the plan." vs
  "Clarify sent — answers without rewriting the plan."
- User-guide `19-plan-mode` shortcut table tightened to the same language.

Footer button short labels stay `clarify` / `revise` (packing / hit-test). The
distinction lives in the card pointer, Enter hint, and toast.

### Incidental compile unblock (not Phase R)

`xai-grok-tools/src/shared_http_rate_limit.rs` had a broken
`pub use DEFAULT_RATE_LIMIT_WAIT` that reimported a private use-import and
blocked `cargo test -p xai-grok-pager`. Fixed to
`pub use grok_rate_limit::DEFAULT_RATE_LIMIT_WAIT`. No rate-limit media wiring
owned here.

## Files touched

- `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` (tests + toasts)
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` (Enter hints)
- `crates/codegen/xai-grok-pager/src/views/plan_approval_view.rs` (card/placeholder copy + copy test)
- `crates/codegen/xai-grok-pager/docs/user-guide/19-plan-mode.md`
- `crates/codegen/xai-grok-tools/src/shared_http_rate_limit.rs` (compile only)

## Not owned

- Residual / AGENTS dual-pin (Phase 0)
- Rate-limit media wiring (Phase R)

## fmt

`cargo fmt -p xai-grok-pager -p xai-grok-tools`
