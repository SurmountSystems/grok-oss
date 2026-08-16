# Report: hierarchically structured subagents (L2 must spawn L3)

**Date:** 2026-08-13  
**Slice:** host + project process law. Did not touch pager/shell last-session-on-start / auto-resume product code.

This Grok Build cloud L2 has **no** `spawn_subagent` / `task` tool, and `workflow` is blocked for nested agents. L3 specialists were required and **could not be launched here**. Diagnosis and edits ran on this L2 with a short, already-known file set (no compact-and-continue).

## Skill files found (verified on disk)

| Path | Role | Load path |
|------|------|-----------|
| `~/.agents/skills/hierarchically-structured-subagents/SKILL.md` | **New** auto-invoke skill. Trigger-rich description. Hard rules: L2 MUST spawn L3; half-window stop; do not compact-and-continue. | Host User tier. Discovery loads `.agents` before `.grok` at the same tier. This file is now in the live skill list. |
| `~/.agents/skills/shared/references/subagent-token-strategy.md` | D3 deep guide. **Not** auto-loaded for L2. Orchestrators link it. | Linked from `_SKILL_RULES`, implement, plan, host AGENTS. L2 that never opens it never sees MUST. |
| `~/.agents/skills/_SKILL_RULES-read-first-pls.md` | Skill-author law. Item 9 + L3 table row now MUST. | Authors / skill-maintenance. Not the L2 system prompt. |
| `~/.agents/skills/implement/SKILL.md` | Orchestrator. L3 row MUST. Implementer prompt now says "You are L2… MUST spawn L3…". | L1 `/implement` reads this. L2 sees the injected prompt only if L1 follows the template. |
| `~/.agents/skills/plan/SKILL.md` | Orchestrator. Explore prompts tell L2 to MUST spawn L3. | L1 `/plan`. |
| `~/.agents/skills/shared/personas/implementer.md` | Persona prepended to implementer L2. Opening is now MUST spawn L3. | Only when `/implement` prepends it. |

**There was no skill named "hierarchically structured subagents" before this slice.** The operator name pointed at D3 + scattered advisory bullets. That is why L2s did not auto-invoke three-layer law.

## Why L2 compacted (verified, not leftover guesses)

The L2 titled "Restore lost auto-resume" hit "Context 98% full. Compacting..." while still walking cancel-resume code. These causes were checked in code and skills:

1. **No dedicated auto-invoke skill.** D3 `subagent-token-strategy.md` is a reference. L2 does not load it unless a parent prompt names it.
2. **Law was advisory.** Text said "L2 may spawn L3 when needed." Hard stop is L1-focused: "L2 owns diagnosis" reads as "L2 solo-walks the tree."
3. **Parent spawn prompts omitted L3.** Implementer persona and `CHILD_TASK_DESCRIPTION` told L2 to prefer doing the work itself. A unit test locked that wording (`child_task_description_is_concise`).
4. **Product default max nesting was 1.** `MAX_SUBAGENT_DEPTH` and `SubagentsConfig::DEFAULT_MAX_DEPTH` were `1`. Spawn is rejected when `depth >= max`, so an L2 at depth 1 **cannot** spawn L3. Test `subagent_cannot_spawn_nested_subagent` encoded that old contract. User-guide `16-subagents.md` said the maximum nesting depth is one.
5. **This host still cannot spawn L3 from an L2** (no spawn tool; nested `workflow` banned). Skill text alone cannot fan out on Grok Build cloud L2.

## What changed

### Host skills

- Created `~/.agents/skills/hierarchically-structured-subagents/SKILL.md`.
- Tightened `_SKILL_RULES`, token-strategy, implement, plan, implementer persona, check-work (including verifier prompt), review (spawn prompt), execute-plan, git-recon, pr-babysit, upstream-export-import, code-review, skill-maintenance.
- Pin harness `skill-maintenance/test-required-pins.sh` now asserts the new skill exists and MUST / do-not-compact-and-continue strings.
- D2 one-liner in `shared/references/skill-reconciliations.md`.

### Dual-pin (short)

- `~/.grok/AGENTS.md`: regressions item 4 + Agent depth paragraph (pinned 2026-08-13).
- Project `AGENTS.md`: L3 table cell MUST + half-window + do not compact-and-continue.
- `FORK.md`: Parent = HITL checkbox, same sentence.

### Parent spawn prompts (product)

- `CHILD_TASK_DESCRIPTION` in `crates/codegen/xai-grok-agent/src/builder.rs` now tells L2: MUST spawn L3; half-window; do not compact-and-continue; no L4.
- Implement `/implement` prompt template already injected the same paragraph.

### Three layers are mechanically allowed

- `MAX_SUBAGENT_DEPTH` (task tool default) `1` → `2`.
- `SubagentsConfig::DEFAULT_MAX_DEPTH` `1` → `2`.
- `clamp_max_depth` floor is literal **1**, not the new default, so `[subagents] max_depth = 1` still means L1-only spawn.
- User-guide `16-subagents.md` (product + `~/.grok/docs`) Depth Limits + short Token efficiency section.

### TDD (named contract changed 2026-08-13)

| Test | Red | Green |
|------|-----|-------|
| `child_task_description_is_concise` | missing `MUST spawn L3` | same test, new description |
| `default_max_allows_l2_to_spawn_l3` (was `subagent_cannot_spawn_nested_subagent`) | depth 1, max 1 rejected spawn | Ok at default max 2 |
| `resolve_max_depth_default_allows_l2_to_spawn_l3` | default was 1 | default is 2 |
| `explicit_max_one_rejects_l2_spawn` | (new, already passed: explicit 1 still rejects) | kept |
| `resolve_max_depth_explicit_one_stays_one` / `_zero_clamps_to_one_not_default` | would have clamped 1→2 if floor used DEFAULT | floor is 1 |

`test-required-pins.sh`: PASS.

`cargo fmt` + `cargo clippy -p xai-grok-agent -p xai-grok-tools --all-targets -- -D warnings` + `cargo clippy -p xai-grok-shell --lib -- -D warnings`: exit 0.

## Leftover

- **This Grok Build cloud L2 still has no `spawn_subagent`.** Nested `workflow` is banned. After rebuild, grok-oss L2s get the task tool at default max 2. This host session does not.
- `~/.agents/skills/design/SKILL.md` L3 row still says Optional (bulk-edit guard blocked the last one-line swap).
- Last-session-on-start / auto-resume product restore is owned by another agent. Not touched.
- D3 research notes that still say "depth is one" were left (not standing law).
- Live grok-oss binary must be rebuilt before operators see default max 2 and the new child task description.
