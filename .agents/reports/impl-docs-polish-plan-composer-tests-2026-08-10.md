# Docs polish + test completeness (plan workflow + composer)

**Date:** 2026-08-10
**Board:** `impl:docs-polish-plan-composer`, `impl:tests-complete-consistent`
**Tree:** `/home/hunter/Projects/surmount/grok-build`

## Goal

Durable docs and tests match the plan-workflow and composer product work already
in tree (sticky decision, present ≠ Approve, always-approve ≠ plan Approve,
composer Ctrl buffer nav, no solid green block mid-draft).

## Docs contracts restated (user-facing)

| Contract | Where taught |
|----------|----------------|
| `exit_plan_mode` / “Plan ready” = **present for review**, not operator Approve | user-guide `19-plan-mode` § Present is not approval; tutorial `07`; FORK; AGENTS |
| Real approval = **plan panel CTAs** (Approve / Notes / Clarify / Revise / Quit); not freeform chat | `19-plan-mode`, tutorial `07`, AGENTS |
| `always-approve` = skip **tool permission** prompts only; still wait for plan CTAs | `19-plan-mode`, `22-permissions-and-safety`, `04-slash-commands`, `01-getting-started` |
| After one decisive Approve or Quit: **no re-arm** of Approve / “Plan ready” until a **new** present | `19-plan-mode` § After one decisive Approve or Quit; FORK sticky bullet |
| Composer: Ctrl+Home/PgUp → buffer start; Ctrl+End/PgDn → buffer end; bare Home/End line-local | `03-keyboard-shortcuts` |
| Composer caret: Human green; no solid `█` mid-draft on spaces | `03-keyboard-shortcuts`, `06-theming`, FORK |

Process pins preserved: FORK plan CTA / soft-park bullets kept; new shipped
bullets added for 2026-08-10 present≠approve, sticky decision, composer nav.
RESIDUAL open honesty updated (product false-approve + multi-approve **shipped**;
agent freeform `plan.md` menus still soft).

Host `~/.grok/docs/user-guide/` is a separate install copy, not the product SoT.
Edits stayed in-repo under `crates/codegen/xai-grok-pager/docs/`.

## Files changed

### User-guide / tutorial

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/docs/user-guide/19-plan-mode.md` | Present ≠ approval table; after-decide sticky; always-approve vs plan CTAs; lifecycle wording |
| `…/22-permissions-and-safety.md` | Always-approve is not plan approval + link to plan mode |
| `…/04-slash-commands.md` | `/always-approve` = tool prompts; not plan CTAs |
| `…/03-keyboard-shortcuts.md` | Ctrl+Home/End/Page buffer nav; caret solid-block rule |
| `…/01-getting-started.md` | Always-approve permission scope note |
| `…/06-theming.md` | Mid-draft space caret (no solid `█`) |
| `…/tutorial/07-plan-and-permissions.md` | Real CTAs; present ≠ approve; always-approve scope |

### Fork / residual / process

| Path | Change |
|------|--------|
| `FORK.md` | Shipped: present≠Approve, sticky `plan_decision_resolved`, composer nav; caret solid-block note |
| `RESIDUAL.md` | Plan UI open item: product multi-approve + false tool-body shipped; freeform soft only |
| `AGENTS.md` | Plan approval bullet + hard-constraint present≠Approve pin |

### Product (consistency with process law)

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-tools/.../exit_plan_mode/mod.rs` | Present message: wait for **plan panel** CTAs; forbid freeform/always-approve as plan approve; test asserts |

## Tests inventory (coverage vs contracts)

| Contract | Existing tests | Package | This pass |
|----------|----------------|---------|-----------|
| Sticky no re-park after Approve | `after_approve_current_mode_clears_pending_still_in_plan_does_not_repark`, `approved_and_implemented_plan_body_does_not_repark_after_decide`, `live_approve_*`, `local_idle_approve_*` | pager | Green (no product gap) |
| New present re-arms | `new_exit_plan_mode_present_clears_decision_resolved_and_parks` | pager | Green |
| Bare tool body ≠ approval | `exit_plan_mode_tool_result_does_not_claim_operator_approval`, `exit_with_plan_content`, `prompt_format_includes_plan_content` | tools | **Strengthened** (panel CTAs, no freeform, always-approve separation) |
| Panel Approve message | `approved_exit_plan_message_names_panel_cta_and_embeds_body`, empty variant | shell | Green |
| No-client honest leave | `no_client_exit_plan_message_does_not_claim_panel_approve`, `real_exit_plan_mode_no_client_executes_tool` | shell | Green |
| Composer Ctrl buffer ends | `ctrl_home_end_page_move_to_buffer_ends`, `ctrl_home_end_page_move_prompt_cursor_to_buffer_ends` | textarea + pager | Green |
| No solid block mid-space | `mid_buffer_space_caret_does_not_paint_solid_block_glyph`, Left prefix residue tests | pager | Green |

No contradictory asserts found against the docs contracts above.

## Commands + exit codes

```text
cargo fmt -p xai-grok-tools -p xai-grok-shell -p xai-grok-pager -p xai-ratatui-textarea
# exit 0

cargo test -p xai-grok-tools --lib -- \
  exit_plan_mode_tool_result_does_not_claim exit_with_plan_content \
  exit_with_empty_plan_file prompt_format_includes_plan
# 4 passed, exit 0

cargo test -p xai-grok-shell --lib -- \
  approved_exit_plan no_client_exit_plan plan_approval_helper \
  real_exit_plan_mode_no_client
# 9 passed, exit 0

cargo test -p xai-grok-pager --lib -- \
  after_approve_current approved_and_implemented new_exit_plan_mode \
  live_approve_does_not local_idle_approve mid_buffer_space_caret \
  ctrl_home_end_page_move_prompt left_arrow_does_not_insert \
  left_arrow_with_chrome
# 9 passed, exit 0

cargo test -p xai-ratatui-textarea --lib -- ctrl_home_end_page_move_to_buffer_ends
# 1 passed, exit 0

cargo clippy -p xai-grok-tools --lib -- -D warnings   # exit 0
cargo clippy -p xai-grok-shell --lib -- -D warnings   # exit 0
cargo clippy -p xai-grok-pager --lib -- -D warnings   # exit 0
cargo clippy -p xai-ratatui-textarea --lib -- -D warnings  # exit 0
```

## Residual still open

- **Agent-written `plan.md` freeform menus** (process/skills; product chrome green).
  Ranked residual row 13; not closed by this pass.
- **Dogfood / install gate** for plan+composer tree (operator rebuild + quit old
  TUIs). Product contracts green in-tree.
- **Host `~/.grok/docs` copy** updates only on product install path; not dual-
  written here.
- Dead config `require_plan_approval` still unused (prior report); not touched.

## Out of scope (honored)

- No git add/commit/push
- No multi-approve product redesign
- No implement-run hex in product docs
