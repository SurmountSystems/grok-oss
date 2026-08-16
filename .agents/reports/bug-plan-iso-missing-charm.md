# Isolated plan.md approval chrome (2026-08-14)

**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Agent:** L2 implementer (this host cannot spawn L3)  
**Screenshot:** Fri Aug 14 ~4:46 PM, session `~/Projects/surmount/surmount-server`, file-backed `plan.md` in plan approval  
**Same class as:** `.agents/reports/bug-plan-panel-ui-inconsistent.md`

SuperGrok is a paid product. This report says **included SuperGrok period limits**, never "free SuperGrok."

## Source already correct vs leftover 1.0.3 path

The live TUI painted the Grok Build 1.0.3 placeholder, not a second current paint function.

| Surface in the screenshot | Current source |
|---------------------------|----------------|
| Plan footer `a approve \| s request changes \| c comment \| y copy plan \| q quit plan` | Not a paint string. Approval footer in `crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs` paints `a approve \| A notes \| ? clarify \| s revise \| q quit` when `feedback_active` is true. Casual preview still uses comment / copy plan / send. |
| Composer `c:comment \| y:copy plan \| a:approve \| q:quit plan \| v:select \| Tab:prompt` | Not this mix. `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` uses approve / notes / clarify / revise / quit while `plan_approval_view` is open. |
| Yellow `Waiting on plan approval` | Not a paint string. Parked copy is `Plan ready. Side panel open` from `crates/codegen/xai-grok-pager/src/views/plan_approval_view.rs` (`plan_approval_status_label`). |
| Isolated / file-backed `plan.md` | Same paint as inline CreatePlan. `handle_exit_plan_mode` without a CreatePlan / "Plan: Submit for approval" title sets `PlanReviewSource::FileBacked` (`acp_handler/interactions.rs`), then `show_plan_preview` arms `feedback_active`. Source enum does not pick a second footer. |

`rg` over `*.rs` finds `request changes` and `Waiting on plan approval` only in tests that reject those leftovers, comments, and a research note. There is no product paint of those strings.

This is the same 1.0.3 leftover already restored on 2026-08-13. Live TUIs keep the old binary until a successful `/rebuild` and a full quit/reopen.

## TDD

Named contract: isolated file-backed `plan.md` approval must paint Surmount five-CTA (Approve / Notes / Clarify / Revise / Quit), not the 1.0.3 request-changes / comment / copy-plan / quit-plan bar, and parked status must be **Plan ready. Side panel open**.

Product paint was already correct. There was no honest red on the new assert. I did not invent a failing product state and then "green" it.

**Test names**

- `file_backed_plan_md_approval_draw_uses_five_cta_not_103_placeholder` (new; this isolated path)
- `exit_plan_without_inline_content_uses_file_backed_source` (existing handle path; now also checks `feedback_active` and the parked review label)
- Already present: `plan_approval_footer_paints_five_cta_vocabulary`, `plan_approval_draw_uses_one_five_cta_vocabulary` (inline / viewer unit)

**Command (isolated target; first compile timed out at 300s, retry used cache)**

```bash
CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-plan-charm-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-pager --offline --lib -- \
  file_backed_plan_md_approval_draw_uses_five_cta_not_103_placeholder
```

**First completed run (product already correct, no fake red)**

```
test app::agent_view::render::voice_recording_overlay_tests::file_backed_plan_md_approval_draw_uses_five_cta_not_103_placeholder ... ok
test result: ok. 1 passed; 0 failed
```

**Green sibling filter (same command family)**

```
file_backed_plan_md_approval_draw_uses_five_cta_not_103_placeholder ... ok
plan_approval_draw_uses_one_five_cta_vocabulary ... ok
plan_approval_footer_paints_five_cta_vocabulary ... ok
exit_plan_without_inline_content_uses_file_backed_source ... ok
new_present_turn_row_is_review_park_not_approve ... ok
# 5 passed
```

## What changed (and what did not)

Did **not** rewrite plan footer, composer hints, or titled-composer paint. That chrome already matches the five-CTA contract.

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Named full-draw test for FileBacked isolated `plan.md`. |
| `crates/codegen/xai-grok-pager/src/app/acp_handler/tests/plan_mode.rs` | File-backed `exit_plan_mode` now asserts approval chrome is armed and status is Plan ready. |

No product paint, spend-order, or limits-hub edits.

## Leftovers (not this slice)

- **Live TUI is the old binary** until a successful rebuild/install and a full quit/reopen. The Aug 14 screenshot will keep showing request-changes until then.
- Titled composer: default DOGE frame is already pinned white with a yellow title (`titled_doge_composer_frame_is_prompt_border_not_context_yellow`). During plan mode the composer still uses a 40% `accent_plan` border blend in `render.rs`. That is not the all-yellow 1.0.3 box. Not rewritten here.
- Rails, included SuperGrok period limits chip, and [pause]/[stop] were not the leftover paint function. Left alone.
- Pty e2e for isolated `plan.md` resume was not run.

## Commands + exit codes

| Command | Exit |
|---------|------|
| Isolated `cargo test` first attempt (cold target, 300s) | killed (timeout) while still compiling |
| Isolated `cargo test --lib -- file_backed_plan_md_approval_draw_uses_five_cta_not_103_placeholder` | 0 (1 passed) |
| Isolated sibling filter (5 tests above) | 0 (5 passed) |
| `cargo fmt -p xai-grok-pager` | 0 |
| `cargo clippy -p xai-grok-pager --offline --lib -- -D warnings` | 0 |
