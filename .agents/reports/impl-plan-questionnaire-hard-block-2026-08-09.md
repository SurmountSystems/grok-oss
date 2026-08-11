# Report: plan questionnaire hard block (2026-08-09)

## Problem

Soft prompt bans from the earlier plan-questionnaire regression fix were **not
enough**. Colibri sessions still opened full multi-choice
`ask_user_question` questionnaires during plan mode because the tool remained
in the advertised toolset. Models keep calling tools that are listed.

Prior soft-only report:
[`.agents/reports/impl-plan-questionnaire-regression-2026-08-09.md`](impl-plan-questionnaire-regression-2026-08-09.md)

## Product fix (hard)

Two layers, same pattern as the plan-mode edit gate:

| Layer | Where | Behavior |
|-------|--------|----------|
| **Tool list** | `filter_cursor_tools_by_plan_mode` in `session_mode.rs` | When plan mode is **Active**, strip `ask_user_question` / `AskUserQuestion` / `AskUser` from tools advertised to the model. Outside plan mode, unchanged. |
| **Call reject** | `plan_mode_ask_user_gate` in `tool_calls.rs`, run in `prepare_tool_call` | If plan mode is **Active** and the call is `ToolInput::AskUserQuestion`, fail closed with a model-facing rejection **before** the questionnaire UI path. |

### Rejection text (model-facing)

Names the blocked tool, steers to plan file / freeform chat / `exit_plan_mode`,
and mentions legacy `/plan --legacy` only as documentation (not an automatic
re-enable).

### Not changed

- Non-plan use of `ask_user_question` (still available when plan mode is
  Inactive / Pending).
- Soft reminder / enter-plan prompt bans (still useful teaching surface).
- Permission multi-choice UI (tool consent is a different surface).
- Plan approval CTAs / soft-park path.
- Explicit product flag for `/plan --legacy` is **not** wired yet. Default plan
  mode always hard-blocks. Legacy re-enable would need an explicit product flag
  later; skill-only opt-in does not restore the tool today.

## Tests (TDD contract)

| Test | Contract |
|------|----------|
| `plan_mode_tool_list_omits_ask_user_question` | Active plan mode drops questionnaire names from tool defs; inactive keeps them |
| `plan_mode_blocked_ask_user_name_matcher` | Name matcher covers aliases; does not strip Cursor `AskQuestion` |
| `active_plan_mode_rejects_ask_user_question` | Unit gate rejects `AskUserQuestion` when Active |
| `inactive_or_pending_allows_ask_user_question` | Unit gate allows when Inactive / Pending |
| `ask_user_gate_does_not_block_other_tools` | Gate is questionnaire-only |
| `plan_mode_rejects_ask_user_question_before_ui` | Real `prepare_tool_call` rejects while Active |
| `inactive_plan_mode_allows_ask_user_question_prepare` | Real `prepare_tool_call` allows outside plan mode |

Existing edit-gate integration tests still green.

## Verification

```bash
cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --lib -- -D warnings
cargo test -p xai-grok-shell --lib -- \
  plan_mode_tool_list_omits_ask_user_question \
  plan_mode_blocked_ask_user_name_matcher \
  active_plan_mode_rejects_ask_user_question \
  inactive_or_pending_allows_ask_user_question \
  ask_user_gate_does_not_block_other_tools \
  plan_mode_rejects_ask_user_question_before_ui \
  inactive_plan_mode_allows_ask_user_question_prepare \
  plan_mode_rejects_grok_edit \
  plan_mode_allows_plan_file \
  inactive_plan_mode_does_not_gate
```

All listed tests passed. Clippy `--lib -D warnings` clean for `xai-grok-shell`.

## Files

- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/session_mode.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_tests/prompt_mode_transition_tests.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_tests/plan_mode_edit_gate_tests.rs`

## Dogfood after rebuild

1. `/plan` on a multi-area task.
2. While plan mode is Active, model must **not** open multi-choice questionnaires.
3. If a stale call still arrives, tool result is the hard rejection string (no UI).
4. Permission prompts and plan-panel CTAs still work as before.
5. Outside plan mode, `ask_user_question` still available when enabled.

## Board

`bug:plan-questionnaire-still-fires` → complete with this hard block + TDD green.
