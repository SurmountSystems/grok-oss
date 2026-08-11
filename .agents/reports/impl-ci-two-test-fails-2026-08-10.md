# Fix: two CI test fails (exit_plan_mode no-client + settings inventory)

**Date:** 2026-08-10
**Packages:** `xai-grok-shell`, `xai-grok-pager`
**Operator:** CI summary 28154 run / 2 failed

## Failures (red)

| Test | Red message / cause |
|------|---------------------|
| `xai-grok-shell` `session::acp_session::plan_approval_resume_tests::real_exit_plan_mode_no_client_executes_tool` | Asserted `outcome.is_ok()` ("headless … fall through to execute the tool"). Product no longer fall-throughs; headless intercept completes in prepare with `ToolLoop::Continue` and honest no-panel copy. |
| `xai-grok-pager` `views::settings_modal::tests::rows_contain_categories_and_settings_through_pr_14` | Inventory list omitted registry row `always_expand_thinking` (registered after `show_thinking_blocks`). Legitimate new setting drift, not a missing product row. |

## Contracts (named)

### 1. Headless / no-client `exit_plan_mode`

**Intent (from prior fix report `impl-plan-auto-approved-false-2026-08-10.md`):**
`exit_plan_mode` success is **not** operator plan approval. Panel **Approve** synthesizes `approved_exit_plan_tool_message`. No interactive client uses `no_client_exit_plan_tool_message` and leaves plan mode **without** claiming panel Approve or always-approve plan auto-approve. Bare tool body stays present-only.

**Test update (not product restore of the lie):**

- `prepare_tool_call` → `Err(ToolLoop::Continue)` (completed in intercept; no tool-body prepare).
- Plan mode inactive; `awaiting_plan_approval` clear.
- Tool result text includes "No interactive plan panel", "NOT a plan-panel Approve"; forbids "has been approved" / "start coding"; embeds plan body when present.

File: `crates/codegen/xai-grok-shell/src/session/acp_session_tests/plan_approval_resume_tests.rs`
Function name kept (`real_exit_plan_mode_no_client_executes_tool`) so CI filters stay stable; docstring rewritten to match the real contract.

### 2. Settings modal inventory

**Intent:** `rows_contain_categories_and_settings_through_pr_14` is a full ordered inventory of top-level modal rows. When a legitimate setting is registered, the inventory must list it.

- Added `"always_expand_thinking"` immediately after `"show_thinking_blocks"` (matches `settings/defs.rs` Appearance order).
- No product change.

File: `crates/codegen/xai-grok-pager/src/views/settings_modal/tests.rs`

## Commands (all exit 0)

```bash
# Original two fails — green
cargo test -p xai-grok-shell --lib \
  session::acp_session::plan_approval_resume_tests::real_exit_plan_mode_no_client_executes_tool
cargo test -p xai-grok-pager --lib \
  views::settings_modal::tests::rows_contain_categories_and_settings_through_pr_14

# Nearby plan exit filters — 12 ok
cargo test -p xai-grok-shell --lib -- \
  approved_exit_plan no_client_exit_plan plan_approval_helper \
  exit_plan_mode_empty_plan exit_plan_mode_nonempty real_exit_plan_mode

# Full plan_approval_resume_tests module — 10 ok
cargo test -p xai-grok-shell --lib session::acp_session::plan_approval_resume_tests

# Post-impl
cargo fmt -p xai-grok-shell -p xai-grok-pager
cargo clippy -p xai-grok-shell --lib -- -D warnings
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

## Not done / notes

- Product no-client path left as already shipped (honest leave; no false Approve).
- Did not thrash concurrent `prompt_widget` composer work; a mid-flight syntax error there resolved before final green run.
- `picker_highlights_current_choice` failed once in a bulk settings_modal run mid-compile noise; isolated re-run ok; not part of this fix.
- No git add / commit / push.
