# Report: plan questionnaire regression (2026-08-09)

## What the screenshot UI actually is

**Most likely tool permission multi-choice, not Plan Questions mode and not plan-approval CTAs.**

Evidence from the operator description:

| Chrome cue | Matches |
|------------|---------|
| Options like always-approve / allow all edits / Yes | Permission option kinds (`permission_view`, always-allow / allow-once style labels) |
| Freeform "Other" + typed process law | Can appear on **both** permission freeform and `ask_user_question` (auto "Other" row) |
| Enter:send / Esc:back | Composer / selection footer language (prompt + multi-choice surfaces), not the plan panel Approve/Revise strip |
| Plan mode active + explore subagents running | Expected during `/plan` explore phase; does not by itself mean a plan quiz |

**Not:** soft-park plan approval (Approve / Notes / Clarify / Revise / Quit). That surface does not present always-approve / allow-all-edits.

**Also real product issue (separate from that screenshot chrome):** even if the shot was only a permission dialog, product plan-mode **prompts still taught** the model to call `ask_user_question` during planning. That made real multi-choice plan questionnaires likely on later turns.

## Product code: regressed (teaching surface)

Host skill and `AGENTS.md` already banned questionnaire plan clarifications. **Product injectors still promoted the opposite.**

### Before (bad)

1. **Plan full + reentry reminders** (`xai-grok-shell` `plan_mode.rs`):
   - "Your turn should only end with either `ask_user_question` to clarify requirements or `exit_plan_mode` …"
2. **`enter_plan_mode` tool result** (`xai-grok-tools` `types/output.rs`):
   - Step 3: "Use `ask_user_question` if you need to clarify the approach"
3. **User guide** `19-plan-mode.md`:
   - "May use `ask_user_question` to clarify specific questions"
4. **`ask_user_question` tool description + module docs**:
   - Described as the plan-mode **interview mechanism**

### Skill / process: no skill regression

Host `~/.agents/skills/plan/SKILL.md` hard rule 6 still bans modal questionnaire unless `--legacy`. Project `AGENTS.md` hard constraint 8 still bans questionnaire while planning. No skill fix required; product was overriding skill intent via injected reminders.

## What we fixed

### Product prompts (default ban)

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-shell/src/session/plan_mode.rs` | Full + reentry reminders: open questions in plan file / freeform chat; **do not use** ask tool multi-choice; end with `exit_plan_mode` |
| `crates/codegen/xai-grok-tools/src/types/output.rs` | Enter-plan prompt: 5 steps; ban questionnaire by name in step 4 |
| `crates/codegen/xai-grok-tools/src/implementations/grok_build/ask_user_question/mod.rs` | Tool description + module docs: ban while planning unless explicit legacy opt-in |
| `crates/codegen/xai-grok-pager/docs/user-guide/19-plan-mode.md` | Matches product: plain bullets, not multi-choice questionnaires |

`EnterPlanModeToolHints.ask_user` remains (serde / name resolution) so the ban can name the real client tool id. Tool stays registered (legacy `/plan --legacy` and non-plan use still possible); we stop **teaching** plan interview by default.

### Tests (contract = ban, not "must use ask")

- `enter_plan_mode_prompt_format_*` expect "do not use … multi-choice questionnaires"
- `enter_plan_mode_prompt_format_contains_five_steps_bans_questionnaire` (replaces six-step "Use ask_user_question")
- Plan reminder tests assert ban wording and reject "to clarify requirements"
- `ask_user_question` description test asserts plan-mode ban + legacy opt-in

### Not changed

- Plan panel CTAs / soft-park approval path
- Permission multi-choice UI (correct for tool consent)
- Host plan skill (already correct)
- Hard strip of `ask_user_question` from plan toolset (soft ban via prompts + tool description; keep tool for non-plan + explicit legacy)

## Verification (operator)

```bash
# Contract tests
cargo test -p xai-grok-tools --lib enter_plan_mode
cargo test -p xai-grok-tools --lib ask_user_question::tests::tool_name
cargo test -p xai-grok-shell --lib full_reminder_resolves
cargo test -p xai-grok-shell --lib reentry_reminder

# Lib clippy on touched crates (pre-existing --all-targets noise elsewhere)
cargo clippy -p xai-grok-tools -p xai-grok-shell --lib -- -D warnings
```

Dogfood after rebuild/install of this tree:

1. `/plan` on a multi-area task.
2. While plan mode is active, agent should **not** open multi-choice questionnaires for design choices; open questions appear in `plan.md` / freeform chat.
3. Permission prompts for `enter_plan_mode` / edits may still show always-approve / allow edits; that is **not** a plan quiz.
4. Present via `exit_plan_mode` → plan panel CTAs only.
5. Optional: `/plan --legacy` still documents the skill exception for intentional modal questions.

## Board

`bug:plan-questionnaire-regression` → complete when this report lands and tests are green.
