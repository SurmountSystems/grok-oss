# Fix: false "Plan auto-approved" after exit_plan_mode soft-park

**Date:** 2026-08-10
**Packages:** `xai-grok-tools`, `xai-grok-shell`
**Board:** `bug:plan-auto-approved-false` / `bug:exit-plan-mode-false-approve`
**Dogfood:** dragon-npu ~9:22 AM, agent said "Plan auto-approved again (workflow bug)" while soft-park showed Plan ready; footer `always-approve`

## Root cause: product wire (not always-approve auto-CTA)

| Question | Finding |
|----------|---------|
| Does `exit_plan_mode` tool result claim approval? | **Yes (product lie).** Tool body returned `"Your plan has been approved. You can now start coding."` whenever it ran, even though its own docs say present-only. |
| Does `always-approve` auto-call plan panel Approve? | **No.** YOLO only auto-allows `session/request_permission`. `x.ai/exit_plan_mode` soft-parks and waits on `response_tx`. `require_plan_approval` is loaded on `AppView` but unused for gating. |
| Soft-park under always-approve auto-resolve? | **No.** Soft-park holds the reverse-request; Approve/Revise/Clarify/Quit still require a real CTA. |
| Sticky multi-approve fix? | **Unchanged.** `plan_decision_resolved` path left alone. |

**Architecture (TUI):** shell intercepts `exit_plan_mode` → `request_plan_approval` → soft-park → only on panel **Approve** should the model hear "user approved."

**Bug:** the **tool body** itself always claimed approval. Mid-turn after real Approve, shell **ran that tool**, so the model got "approved / start coding." Same copy also ran on the **no-client** fail-open path (headless/SDK) without any panel click. Host AGENTS already pinned this as `exit_plan_mode` success ≠ operator approve; product still taught the opposite string.

Dogfood agent prose ("auto-approved again") matches that false tool string plus residual/process teaching, not an always-approve plan CTA.

## Product changes

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-tools/.../exit_plan_mode/mod.rs` | Present-only tool message and description. Never "approved" / "start coding". Explicit "NOT operator approval." |
| `crates/codegen/xai-grok-shell/.../tool_calls.rs` | On panel **Approved**: leave plan mode, re-read `plan.md`, synthesize `approved_exit_plan_tool_message` (names plan panel CTAs). **Do not** run tool body for that claim. On **no-client**: leave plan mode with honest no-panel copy (`no_client_exit_plan_tool_message`), not false Approve. |

### Message contracts

| Path | Model hears |
|------|-------------|
| Bare tool run / present | Plan presented; **NOT** operator approval; wait for panel CTAs |
| Shell after panel Approve | User approved **via the plan panel CTAs**; implement; disk re-read body |
| No interactive client | No plan panel; **not** panel Approve; **not** always-approve plan auto-approve |

## TDD

### Red → green (named contracts)

| Test | Contract |
|------|----------|
| `exit_plan_mode_tool_result_does_not_claim_operator_approval` | Bare tool prompt embeds plan body and forbids "has been approved" / "start coding" |
| `exit_with_plan_content` (rewritten) | Present-only language + present-time disk re-read |
| `exit_with_empty_plan_file` / missing | Empty present must not say "exit approved" |
| `prompt_format_includes_plan_content` (rewritten) | Prompt embeds body without approval claim |
| `approved_exit_plan_message_names_panel_cta_and_embeds_body` | Real Approve path names panel CTAs + body |
| `approved_exit_plan_message_empty_plan_still_names_panel` | Empty approve still names panel |
| `no_client_exit_plan_message_does_not_claim_panel_approve` | No-client copy denies panel Approve and always-approve plan auto-approve |

### Commands (exit 0)

```bash
cargo fmt -p xai-grok-tools -p xai-grok-shell
cargo clippy -p xai-grok-tools --lib -- -D warnings
cargo clippy -p xai-grok-shell --lib -- -D warnings

cargo test -p xai-grok-tools --lib -- \
  exit_plan_mode_tool_result_does_not_claim exit_with_plan_content \
  exit_with_empty_plan_file prompt_format_includes_plan

cargo test -p xai-grok-shell --lib -- \
  approved_exit_plan no_client_exit_plan plan_approval_helper
```

Note: `cargo clippy … --all-targets` on tools still hits **pre-existing** test-only clippy in unrelated modules (ENV_LOCK await, etc.). Lib clippy on touched packages is clean.

## always-approve

**Not involved in auto-approving plans.** Permission mode always-approve skips tool permission prompts only. Plan panel Approve is separate. Footer `always-approve` on the dogfood shot does not mean the plan was auto-approved.

## Operator dogfood

1. Rebuild/install this tree; quit old Grok windows.
2. Plan mode → agent calls `exit_plan_mode` → soft-park + side panel.
3. **Before** any panel CTA: model must not get "plan has been approved" / "start coding". Soft-park stays open; agent must not implement from present alone.
4. Click **Approve** once → tool result / turn says user approved **via the plan panel CTAs**; implement may start.
5. Sticky multi-approve: after Approve, no second Approve strip for the same plan until a new present (prior fix).
6. Always-approve on in footer during soft-park must still require a real plan CTA.

## Out of scope

- No git add/commit/push.
- Sticky multi-approve path not reworked here (already fixed separately).
- Dead config field `require_plan_approval` still unused (docs imply YOLO plan gate; not wired; left alone).
