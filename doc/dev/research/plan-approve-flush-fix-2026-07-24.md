# Plan Approve Flush Fix — Red/Green Evidence

Date: 2026-07-24  
Bug note: [`plan-approve-swallows-comment-2026-07-24.md`](./plan-approve-swallows-comment-2026-07-24.md)  
Related UX map: [`plan-approve-comment-related-ux-2026-07-24.md`](./plan-approve-comment-related-ux-2026-07-24.md)

## Summary

`AgentView::approve_plan` now flushes unsaved Commenting drafts and Prompt
freeform into the approve-with-comments Interject **before** taking
`plan_approval_view` / `send_approved`. Casual plan send gets the same
flush-first treatment.

## Files

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` | `flush_plan_composer_before_approve`; `approve_plan` uses freeform + flushed comments; `send_casual_plan_comments` flushes draft; unit tests |
| `doc/dev/research/plan-approve-swallows-comment-2026-07-24.md` | Bug write-up |
| `doc/dev/research/plan-approve-flush-fix-2026-07-24.md` | This join |

## Behavior

1. **Commenting** + non-empty prompt + `commenting_range` → `save_plan_comment` side effects before take.
2. **Commenting** + empty draft → restore `stashed_feedback_prompt` so leftover freeform is not lost.
3. Non-empty composer freeform (Prompt focus or leftover after flush) →
   `format_feedback(Some(freeform))` on the Interject path.
4. Empty Approve (no comments, no freeform) → still `InputOutcome::Changed` only.
5. Enter-on-Prompt → revise (`send_plan_feedback`) **unchanged**.
6. Casual send while drafting → `save_casual_plan_comment` then send.

## Test names

Module: `app::agent_view::plan::approve_plan_flush_tests`

| Test | Asserts |
|------|---------|
| `approve_plan_flushes_commenting_draft_into_interject` | Interject contains draft; outcome `approved`; view cleared; chat restored |
| `approve_plan_includes_prompt_freeform_in_interject` | Saved comment + freeform both in Interject |
| `approve_plan_empty_still_approves_without_interject` | Empty path → `Changed` + approved |
| `send_casual_plan_comments_flushes_in_progress_draft` | `SendPrompt` contains casual draft |

## Commands / evidence

### RED (tests added, fix not yet applied)

```bash
cargo test -p xai-grok-pager --lib approve_plan_flush_tests -- --nocapture
```

Result: **1 passed, 3 failed**

- `approve_plan_flushes_commenting_draft_into_interject` — got `Changed` (draft swallowed)
- `approve_plan_includes_prompt_freeform_in_interject` — Interject had saved comment only
- `send_casual_plan_comments_flushes_in_progress_draft` — got `Changed` ("No comments to send")
- `approve_plan_empty_still_approves_without_interject` — ok (baseline)

### GREEN (after fix)

```bash
cargo test -p xai-grok-pager --lib approve_plan_flush_tests -- --nocapture
```

Result: **4 passed; 0 failed**

### Broader related

```bash
cargo test -p xai-grok-pager --lib plan_approval
cargo test -p xai-grok-pager --lib -- plan::
```

Result: **21** and **22** passed respectively (includes flush tests + plan chip + plan_approval_view).

## Not done here

- `abandon_plan` / quit still does not flush drafts (product choice left open).
- Tab/click-away while Commenting still intentionally discards.
