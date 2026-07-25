# Plan Approve Swallows Unsaved Comment Draft

Date: 2026-07-24  
Status: bug fixed same day (see `plan-approve-flush-fix-2026-07-24.md`)  
Related: [`plan-approve-comment-related-ux-2026-07-24.md`](./plan-approve-comment-related-ux-2026-07-24.md)

## Symptom

User types a **line comment** (or freeform feedback) while plan approval is
open, then clicks **Approve** or presses **`a`**. Only the approve outcome
fires. The in-progress draft never reaches the agent — it is dropped when
`plan_approval_view` is taken and the stashed chat prompt is restored.

## Desired behavior

On Approve (key or mouse — both call `AgentView::approve_plan`):

1. If **Commenting** with non-empty prompt + `commenting_range` → commit the
   draft into `pav.comments` (same logic as `save_plan_comment`) **before**
   taking the view / `send_approved`.
2. If **Prompt**-focused (or leftover non-empty freeform after the flush) →
   fold that text into the approve Interject via
   `format_feedback(Some(freeform))`.
3. Empty Approve (no comments, no freeform) stays unchanged.
4. Enter-on-Prompt → revise (`send_plan_feedback`) is **not** changed.

## Root cause

`crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` —
`AgentView::approve_plan`:

```rust
let Some(mut pav) = self.plan_approval_view.take() else { ... };
let review_comments = if !pav.comments.is_empty() {
    let formatted = pav.format_feedback(None);  // only already-saved comments
    ...
} else {
    None
};
pav.send_approved();
self.prompt.restore(pav.stashed_prompt);  // discards composer draft
```

- Enter in **Commenting** correctly calls `save_plan_comment`.
- **`a` / mouse Approve** never call it; they only read `pav.comments`.
- Freeform still sitting in the Prompt is also omitted from
  `format_feedback(None)`.

Review comments that *are* approved ride
`InputOutcome::Action(Action::Interject { text, .. })` after
`send_approved()` (outcome `"approved"`, feedback `None` on the ext
response). Unsaved drafts never enter that Interject path.

## Call sites (same method)

| Entry | Path |
|-------|------|
| Key `a` | `viewer.rs` → `approve_plan()` |
| Mouse approve footer | `viewer.rs` → `approve_plan()` |
| Enter on Prompt when empty + no comments | `handle_plan_feedback_key` → `approve_plan()` |

Fixing `approve_plan` covers all of the above.

## Casual plan send (same shape)

`send_casual_plan_comments` only sends `self.plan_comments`. Mouse / `s`
send while `casual_commenting_range` is set ignores the prompt draft
(`save_casual_plan_comment` is Enter-only). Prefer the same flush-first
pattern when easy.

## Not in scope for this fix

- `abandon_plan` / quit (`q`) also drops drafts — separate product choice.
- Tab / click-away while Commenting intentionally **discards**
  (`discard_in_progress_comment`).
- Enter-on-Prompt with freeform remains **revise**, not approve.

## Reference flush pattern

Question view: `submit_question_answers` always calls
`swap_question_freeform()` before taking the view. Plan approve should
mirror that: flush composer into durable comments / freeform slot, then
decide.
