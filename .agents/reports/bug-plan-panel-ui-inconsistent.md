# Plan panel UI inconsistent (2026-08-13)

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Agent:** L2 implementer  
**Screenshot:** 2026-08-13 ~11:07, iso session `~/Projects/ai/iso`, plan.md "Slake as Lake-plus-linear"  
**Residual:** `residual:plan-five-cta-after-103` (after 1.0.3 restack, restore five-CTA plan panel)

Mop report `.agents/reports/bug-process-mop-rebuild-theme.md` existed before product edits. Pager files were idle. No same-file race.

## What was inconsistent

The live TUI was waiting on plan approval and showed **two different action bars** for the same decision:

| Surface | What the screenshot painted |
|---------|-----------------------------|
| Plan panel footer | `a approve \| s request changes \| c comment \| y copy plan \| q quit plan` |
| Composer shortcut row | `c:comment \| y:copy plan \| a:approve \| q:quit plan \| v:select \| Tab:prompt` |

Order and set differed. The panel advertised `s request changes`. The composer did not. The composer advertised `v:select` and `Tab:prompt`. The panel did not.

That is the **Grok Build 1.0.3 three-button / key-hint placeholder**, not the Surmount five clickable CTAs: **Approve / Approve with notes / Clarify / Revise / Quit**.

Also visible, and **not** treated as this bug:

1. Plan preview is a cyan/teal titled box (`plan.md`) with a cyan scrollbar.
2. Composer is a yellow titled box (`Build anything`) with yellow "plan approval" on the right.
3. Status is yellow **Waiting on plan approval**.
4. Markdown heading colors inside iso `plan.md` (magenta vs yellow headings). That file is another repo.

## What was intended

FORK + user-guide `19-plan-mode` + AGENTS:

- Plan present is soft-park plus panel. CTAs are real clickable buttons: Approve, Approve with notes, Clarify, Revise, Quit.
- Keys may mirror those buttons when the panel has empty prompt focus (`a` / `A` / `?` / `s` / `q`). Empty freeform Enter never approves.
- Human chrome is green (`accent_user`). Agent activity is magenta (`accent_running`). Model label is `accent_model`. Do not flip the caret to magenta. Do not invent a new theme.
- Panel footer and composer shortcut row share **one** decision vocabulary.

## Theme slice (B): left as intended

Cyan plan preview vs yellow composer is **two different chrome roles**, not a restack drop of one token:

- Plan title / path chrome uses `theme.path` (DOGE cyan/teal).
- Titled composer border uses caption blend; when that fg matches the border token, DOGE steps to `theme.gray` (yellow/gold). FORK already allows that.

No third palette. Iso heading colors inside `plan.md` were not flattened.

## What was actually broken

The 1.0.3 restack dropped the Surmount five-CTA footer paint, hit rects, and key/click map. `line_viewer.rs` still painted the upstream placeholder (`a approve | s request changes | c comment | y copy plan | q quit plan`) while approval was parked. Composer hints used a mixed 1.0.3 set. Shell had no `"questions"` outcome for Clarify.

This is not a hint typo. It is the restack drop named by `residual:plan-five-cta-after-103`.

## What changed (smallest product restore)

Did not port the whole old soft-park stack. Restored the five-CTA decision surface on the live approval viewer.

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs` | Approval footer paints `a approve \| A notes \| ? clarify \| s revise \| q quit` with clickable hit rects (`approve_button_area`, `approve_notes_button_area`, `questions_button_area`, `send_button_area` = Revise, `abandon_button_area`). Narrow width falls back to key-only. Casual preview still uses comment / copy / send. |
| `crates/codegen/xai-grok-pager/src/views/plan_approval_view.rs` | `PlanPromptIntent` (Revise / Questions / ApproveNotes). `send_questions` emits ACP `"questions"`. Empty-plan placeholder names the five CTAs. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` | Empty-prompt keys: `a` approve, `A` focus notes, `?` focus clarify, `s` decisive revise (ACP `"cancelled"` immediately), `q` quit. Empty Enter never approves (toast). Enter with text follows intent. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` | Mouse hits and preview keys match the same five CTAs. Notes and Clarify are focus-only. Revise is decisive. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Composer shortcut row uses the same five names (`approve` / `notes` / `clarify` / `revise` / `quit`). Preview-only extras stay `y` copy and `Tab` prompt. |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs` | `PlanApprovalOutcome::Questions` → stay in plan mode and answer. Not a rewrite. |
| `crates/codegen/xai-grok-pager/docs/user-guide/19-plan-mode.md` | Table matches the five CTAs. |
| `RESIDUAL.md` | Restack bullet: five-CTA restored in source (2026-08-13); leftovers named. |

`y` copy and `Tab` between preview and prompt remain extras. They are not a second decision vocabulary.

## TDD (red → green)

**Red (tests written / restored first; observed fail before the product restore):**

```
cargo test -p xai-grok-pager --lib --offline -- \
  plan_approval_footer_paints_five_cta_vocabulary \
  plan_approval_draw_uses_one_five_cta_vocabulary \
  s_on_empty_prompt_decisively_revises \
  question_mark_on_empty_prompt_focuses_clarify \
  capital_a_on_empty_prompt_focuses_notes
```

| Test | Fail reason before product edit |
|------|----------------------------------|
| `plan_approval_footer_paints_five_cta_vocabulary` | Footer still contained `request changes`; missing `notes` / `clarify`; notes and questions hit rects were `None`. |
| `plan_approval_draw_uses_one_five_cta_vocabulary` | Full draw reproduced the screenshot: `request changes` on one bar, a different set on the other. |
| `s_on_empty_prompt_decisively_revises` | `s` did not close the park (old request-changes hop). |
| `question_mark_on_empty_prompt_focuses_clarify` / `capital_a_on_empty_prompt_focuses_notes` | `A` / `?` left `prompt_intent` at Revise. |

**Green (same filters after restore, re-run 2026-08-13 this closeout):**

```
cargo test -p xai-grok-pager --lib --offline -- \
  plan_approval_footer_paints_five_cta_vocabulary \
  plan_approval_draw_uses_one_five_cta_vocabulary \
  s_on_empty_prompt_decisively_revises \
  question_mark_on_empty_prompt_focuses_clarify \
  capital_a_on_empty_prompt_focuses_notes \
  a_on_empty_revise_prompt_approves
# 6 passed

cargo test -p xai-grok-shell --lib --offline -- \
  plan_approval_helper questions_plan_message
# 5 passed
```

Earlier in the slice: 49 pager plan-approval lib tests passed; `cargo clippy -p xai-grok-pager --lib --bins -- -D warnings` and `cargo clippy -p xai-grok-shell --lib -- -D warnings` exited 0. `cargo fmt -p xai-grok-pager` was run (also reformatted mop-adjacent dirty files that this slice did not author).

Pty e2e for plan approve/revise was not run here.

## Residual status

**Five-CTA restore is the whole named slice.** `residual:plan-five-cta-after-103` is **satisfied in source**.

The dual action-bar mismatch is the same bug: one 1.0.3 placeholder vocabulary vs one Surmount five-CTA vocabulary. Aligning the composer hints is part of that restore, not a leftover.

## Leftovers (not this slice)

- **Live TUI is the old binary** until a successful rebuild/install. The iso screenshot will keep showing the 1.0.3 placeholder until then.
- Soft-park **strip** paint (the fallback bar when the panel is dismissed or too small) is not restored.
- Idle **local-decision** park (plan mode on, no live `exit_plan_mode` reverse-request) is not restored.
- Status copy is still **Waiting on plan approval**, not **Plan ready. Side panel open**.
- Did not implement a new theme. Did not touch rebuild/install.
