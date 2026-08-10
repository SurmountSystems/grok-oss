# Explore: plan workflow UX + green caret on cursor navigate

**Date:** 2026-08-10
**Tree:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only inventory (no product edits)

Two residual dogfood problems after recent fixes. This report states **current product truth** in code, prior tests/gaps, and concrete implementer targets.

---

## A. Green caret / green char on cursor navigate

### What paints the caret

| Piece | Path | Role |
|-------|------|------|
| Software caret paint | `crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs` | `paint_composer_box_cursor` → `paint_composer_box_cursor_phase` |
| Call site (draw) | same file, `PromptWidget::draw` (~3434–3447) | After textarea + ghost paint; hardware cursor forced `None` |
| Glyph helpers | `crates/codegen/xai-grok-pager-render/src/glyphs.rs` | `cursor_box_filled` (`█`), `cursor_box_hollow` (space), blink phase |
| Buffer nav (Ctrl Home/End/Pg*) | `crates/codegen/xai-ratatui-textarea/src/textarea.rs` | Whole-buffer ends (shipped 2026-08-10) |
| Theme colour | `theme.accent_user` | Human green under DOGE (`Rgb(0,255,0)`); **not** `accent_running` magenta |

Agent-view / plan surfaces do **not** reimplement the box caret; they host `PromptWidget`.

### Mid-buffer styles (current code)

`allow_block_glyph = (cursor == text.len())` only. Then:

| Cell under caret | Solid blink half | Empty blink half |
|------------------|------------------|------------------|
| Blank at **buffer end** (`allow_block_glyph`) | symbol `█`, `fg=accent`, `bg=accent` | space, `fg=canvas`, `bg=canvas` |
| Mid-buffer **space** | keep ` `, reverse plate: `fg=canvas`, `bg=accent` | keep ` `, green ink: `fg=accent`, `bg=canvas` |
| Mid-buffer **letter** (e.g. `T`) | keep `T`, reverse plate: `fg=canvas`, `bg=accent` | keep `T`, **green ink**: `fg=accent`, `bg=canvas` |

There is **no** `Modifier::REVERSED` / DIM. “Reverse” means explicit fg/bg swap onto `accent_user`.

Full-line dirty wipe before textarea paint clears prior caret cells so residue on *left* cells is already tested green.

### Why a letter under the cursor can look solid green while navigating Left/Right

This is **by current design**, not a leftover solid-`█` bug:

1. **Empty blink half on a grapheme** sets `fg = accent_user` (DOGE neon green) on the letter itself. A `T` under the caret becomes a **solid green T** for half the blink cycle. That reads as a second green prompt glyph, not as a classic block caret.
2. **Solid blink half** paints a full-cell **green background plate** with canvas-coloured ink. On bright DOGE green that also reads as a solid green cell with a dark letter punched out.
3. Prior fix (impl-composer-green-char-and-nav) only stopped **mid-buffer solid `█` on spaces**. It did **not** change letter reverse-plate / green-ink styling. Ctrl-Home/End/Pg* only fixed navigation chords.

So operator report “green character (letter T painted green under/as caret)” matches **empty-phase green ink** (and/or solid reverse plate), not residual `█` after the space fix.

### Exact functions for a minimal fix

1. **`paint_composer_box_cursor_phase`** in
   `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs`
   Branches `else if filled_phase` / final `else` for non-blank cells (~2940–2946).
2. Optionally tighten comments + user-guide `06-theming` / `03-keyboard-shortcuts` once visual contract changes.
3. **Tests that currently pin green ink** must change with the product contract (see gap below). Do not green-only reshape asserts without a named caret contract.

**Suggested product direction (for implementer; not decided here):**

- Keep reverse plate only on solid half (`bg=accent`, **keep letter `text_primary` or canvas ink** as today).
- On empty half: **do not** recolor the letter to `accent_user`. Prefer normal text fg + canvas bg, or a quieter cue (underline / dim plate / hollow outline) that does not paint a neon green letter.
- Or: always reverse-plate mid-graphemes without blink recolor of the glyph (blink only at true insertion blank).

### Tests from impl-composer-green-char-and-nav; gap for non-space mid-buffer

| Test | Package | What it covers |
|------|---------|----------------|
| `mid_buffer_space_caret_does_not_paint_solid_block_glyph` | pager | Mid space keeps ` `, never `█`; solid reverse plate OK |
| `left_arrow_with_chrome_prefix_clears_caret_residue` | pager | Left through draft: no solid `█` in textarea body |
| `left_arrow_does_not_insert_prompt_prefix_into_buffer` | pager | Prefix never enters buffer |
| `ctrl_home_end_page_move_to_buffer_ends` | textarea | Ctrl chords → buffer ends |
| `ctrl_home_end_page_move_prompt_cursor_to_buffer_ends` | pager | Same via `PromptWidget::handle_key` |
| `caret_move_clears_previous_cell_caret_styling` | pager | Prior cell loses green after move |
| `paint_composer_box_cursor_grapheme_phases_keep_letter` | pager | **Encodes** empty half `fg=accent` on letter as correct |
| `paint_composer_box_cursor_*` blank phases | pager | End-of-buffer solid `█` vs hollow |

**Gap:** no contract that “mid-buffer letter under caret must not look like a solid green glyph / second prompt character.” Existing grapheme tests **require** green ink on empty half. Any UX fix that stops green letters will need a **named contract rewrite** (TDD: red against new intent, then paint change), not only `mid_buffer_space_*`.

Prior report:
`.agents/reports/impl-composer-green-char-and-nav-2026-08-10.md`

---

## B. Plan workflow UX inventory (current product truth)

### Surfaces after present

| Surface | How it appears | CTAs |
|---------|----------------|------|
| **Side panel** | Soft-park auto-open (`handle_exit_plan_mode` → `show_plan_preview_if_available`); `/view-plan`, status click, `ShowPlan` reopen | Panel footer: Approve / Notes / Clarify / Revise / Quit (mouse primary). Empty-prompt keys `a`/`A`/`?`/`s`/`q` when panel has focus rules. |
| **Soft-park strip** | Panel dismissed **or** panel too small; `paint_soft_park_cta_buttons` in shortcuts row | Same five hit-tested buttons |
| **Transcript plan card** | `commit_parked_plan_card` | Preview + plain pointer only (**not** buttons) |
| **Status chip** | Live park: `plan_approval_status_label` → **“Plan ready. Side panel open”**; idle arm (no park yet): **“Plan written. Click or /view-plan”** | Click opens/reopens preview path |
| **Toast** | Live present: `PLAN_PARKED_TOAST`; idle surface: `PLAN_IDLE_REVIEW_TOAST` | Non-blocking |

Config: `[ui] plan_approval_park = "soft"` (default) vs `"modal"` (fullscreen stash).

### Soft-park vs side panel vs Enter:approve

- Soft-park **parks** `PlanApprovalViewState` with optional live `response_tx` (shell reverse-request). Default soft path **auto-opens** side panel and sets **Prompt focus** (typing goes to composer; L1 modal-free).
- With panel open + Prompt focus + **empty** prompt: shortcuts show **`Enter:approve`**. Empty Enter on panel Prompt **approves** (screenshots-only path handled). Soft-park **without** panel: empty Enter is a **no-op** (not approve); mouse strip CTAs decide.
- Freeform Enter with text defaults by `PlanPromptIntent` (Revise / Clarify / ApproveNotes). Default intent is **Revise**.

### When CTAs appear / re-appear (`plan_decision_resolved`)

Gate: `should_arm_plan_decision_chrome()` =
`effectively_in_plan_mode() && !plan_decision_resolved`

| Event | Sticky / park |
|-------|----------------|
| New `exit_plan_mode` soft-park | `plan_decision_resolved = false`; parks live reverse-request |
| Approve or Quit | `plan_mode_pending = Some(false)`; **`plan_decision_resolved = true`**; park cleared |
| Revise / Clarify | Park cleared; sticky **not** set; idle may re-arm CTAs |
| Turn-end / draw self-heal / `/view-plan` | May park **local idle** only if `should_arm_plan_decision_chrome()` |
| Shell `CurrentModeUpdate` clears pending | Sticky still blocks re-arm until new present |

Helpers:
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs`
(`effectively_in_plan_mode`, `should_arm_plan_decision_chrome`, `park_local_idle_plan_decision_if_needed`, `surface_idle_plan_review_if_needed`, `sync_plan_viewer_approval_chrome`, `approve_plan`, `abandon_plan`)

Present entry:
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/acp_handler/interactions.rs` → `handle_exit_plan_mode`

### What agent/model hears

| Path | Message source | Model hears |
|------|----------------|-------------|
| Bare `exit_plan_mode` tool body | `xai-grok-tools` … `exit_plan_mode/mod.rs` | Present only: **“NOT operator approval”**; wait for plan panel CTAs; forbid freeform/always-approve as plan approve |
| Panel **Approve** (live reverse-request) | Shell synthesizes `approved_exit_plan_tool_message`; **does not** run tool body | “approved via the plan panel CTAs” + disk re-read body |
| **No interactive client** | `no_client_exit_plan_tool_message` | Honest leave; **not** panel Approve |
| Local idle Approve | Pager Interject + `SetPlanMode(Off)` | Implement wording; no ACP waiter |
| Revise / Clarify | Shell `revise_plan_message` / `questions_plan_message` | Stay in plan mode; re-`exit_plan_mode` |

Shell wire:
`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`

### always-approve interaction

- Permission mode **always-approve** skips **tool permission** prompts only.
- Does **not** auto-click plan panel Approve; soft-park still waits on `response_tx`.
- Footer label `always-approve` next to soft-park is **permission chrome**, not plan decision.
- Shift+Tab cycles Normal → Plan → Always-approve (easy mental mix-up with plan Approve).
- Config `require_plan_approval` is **loaded** on `AppView` but **unused** for gating (dead field; docs must not imply YOLO plan auto-gate).

### View-plan / status click paths

1. Status chip hit → `show_plan_preview` (or idle cue path).
2. `/view-plan` (aliases) / ShowPlan → same.
3. `show_plan_preview` re-reads FileBacked `plan.md`, then `park_local_idle_plan_decision_if_needed` **if** decision chrome still owed, then opens side panel with `feedback_active` when parked.
4. Draw: `sync_plan_viewer_approval_chrome` self-heals casual open panel → park CTAs when idle + still armable.

Casual `/view-plan` when **not** in plan mode (or sticky resolved) stays view-only `c comment`.

### Known residual from reports / docs polish

| Report | Shipped truth | Still soft / residual |
|--------|---------------|------------------------|
| `impl-plan-workflow-broken-2026-08-10.md` | B2 race: pending leave on Approve | Superseded by sticky for CurrentModeUpdate |
| `impl-plan-multi-approve-still-broken-2026-08-10.md` | `plan_decision_resolved` sticky | Needs dogfood on **rebuilt** binary |
| `impl-plan-auto-approved-false-2026-08-10.md` | Tool body present-only; shell Approve message | Agent prose can still invent “auto-approved” from habits |
| `impl-plan-approval-ctas-still-missing-2026-08-10.md` | Idle park + draw self-heal + view-plan parks CTAs | — |
| `impl-docs-polish-plan-composer-tests-2026-08-10.md` | Docs + tests aligned | Agent-written freeform menus in `plan.md`; dead `require_plan_approval` |
| RESIDUAL / FORK | Soft-park three surfaces, sticky, present≠approve shipped | Freeform plan.md menus; toast can still *feel* modal |

### Concrete product UX pain points still in code (implementable)

Not process/skills. Things an implementer could change:

1. **Three concurrent “plan ready” signals** (toast + status + side panel + card + strip). Operators still parse “Plan ready” as done/approved. Copy or progressive disclosure could lower false-approve feel without changing wire truth.
2. **`Enter:approve` on empty panel Prompt** remains a high-risk accidental approve path next to free typing. Soft-park deliberately disabled empty-Enter approve without panel; panel still has it.
3. **Default soft-park focus is Prompt**, not Preview. Key CTA accelerators only when empty; operators looking for keyboard `a` while drafting hit composer. Mouse is primary, but dogfood still hunts keys.
4. **Dual park models** (live reverse-request vs local idle Interject) share chrome but different model/shell outcomes. Edge cases after freeform finish without `exit_plan_mode` still feel like a second workflow.
5. **Idle status string still says “Plan written. Click or /view-plan”** when armable but not yet parked (brief window before surface/self-heal). Prior dogfood path to view-only if open raced park (mostly fixed; copy still invites click ceremony).
6. **Sticky is session-flag only** until new present; no disk “workflow status” gate. Bodies saying “approved and implemented” re-park only if sticky false (tests cover sticky true). If sticky fails to set on some path, multi-approve returns.
7. **Dead `require_plan_approval`** still loaded; invents false hope of a plan YOLO gate.
8. **Mouse Approve vs freeform Enter revise** historical inconsistency risk when draft text sits in prompt (flush/approve paths exist; still easy to mis-click Approve and carry or drop notes).
9. **Agent freeform menus inside `plan.md` body** are not product chrome but still paint in the panel body (process residual; product could scrub or warn).
10. **always-approve footer next to plan strip** remains a visual false friend (docs fixed; chrome adjacency still confuses).

Shipped and green in-tree (do not re-diagnose as missing without dogfood rebuild): sticky multi-approve, present-only tool body, always-approve ≠ plan Approve, FileBacked re-read, decisive Revise, same-batch write before exit.

---

## C. Implementer-facing minimal targets

### Green caret residual

- **File:** `prompt_widget/mod.rs` → `paint_composer_box_cursor_phase` grapheme branch.
- **New contract:** mid-buffer letter under caret must not paint solid neon green ink that reads as a second green glyph; keep reverse plate or quieter cue only on current cell.
- **Tests to rewrite/add:** extend beyond `mid_buffer_space_*`; change `paint_composer_box_cursor_grapheme_phases_keep_letter` empty-half assert; full-draw Left through letters with chrome prefix (assert no cell has `fg=accent` except intentional plate rule).

### Plan workflow residual

- Prefer **one primary decision surface** after present (panel already auto-opens; reduce toast/status/card redundancy or unify copy).
- Revisit **empty Enter:approve** on panel Prompt (require explicit `a` / mouse Approve when freeform is empty, or move default focus to Preview).
- Wire or remove **`require_plan_approval`**.
- Keep sticky + present-only messages; dogfood only after rebuild/install.

### Key absolute paths

```
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/prompt_widget/tests.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-ratatui-textarea/src/textarea.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/render.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/acp_handler/interactions.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/plan_approval_view.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-tools/src/implementations/grok_build/exit_plan_mode/mod.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs
/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/19-plan-mode.md
```

### Prior reports (read first if implementing)

```
.agents/reports/impl-composer-green-char-and-nav-2026-08-10.md
.agents/reports/impl-plan-multi-approve-still-broken-2026-08-10.md
.agents/reports/impl-plan-auto-approved-false-2026-08-10.md
.agents/reports/impl-plan-workflow-broken-2026-08-10.md
.agents/reports/impl-plan-approval-ctas-still-missing-2026-08-10.md
.agents/reports/impl-docs-polish-plan-composer-tests-2026-08-10.md
```

---

## Out of scope (this explore)

- No product edits, no git, no rebuild/dogfood.
- Screenshot of awkward plan flow not attached at explore time; inventory is from code + prior dogfood reports.
