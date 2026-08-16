# Report: in-tree bundled skill text vs three-layer agent law

**Specialist:** L3 bundled-skills walker  
**Date:** 2026-08-15  
**Result:** No owned in-tree skill body to pin. No files edited.

## What I was looking for

Product-bundled skill text in this repo that still teaches L2 (or a "subagent") to do the tool work, or that still teaches the old weaker law: L2 must spawn L3 only when there are many greps, or after about half the window.

## Owned walk (skill bodies only)

| Path | What I found |
| ---- | ------------ |
| `/home/hunter/Projects/surmount/grok-build/.agents/skills/` | Does not exist. No project skill packs on this branch. |
| `/home/hunter/Projects/surmount/grok-build/.grok/skills/` | Does not exist. |
| Any in-tree `SKILL.md` | None. Repo-wide search found no committed skill packs. |
| `crates/codegen/xai-grok-bundle/` | Cache writer for the network subagent bundle (`~/.grok/bundled/skills`). Test fixtures only (`# Commit skill`, `# Implement skill`). No live skill bodies. |
| `crates/codegen/xai-grok-tools/src/implementations/skills/` | Loader and parser. No skill markdown. |
| `crates/codegen/xai-grok-pager/docs/user-guide/` except `16-subagents.md` | No L1/L2/L3 depth teaching. `08-skills.md`, `15-agent-mode.md`, `05-configuration.md`, tutorial pages: no weaker spawn rule. |
| `crates/codegen/xai-grok-pager/docs/tutorial/` | No depth or L2-does-tools teaching. |
| `crates/codegen/xai-grok-shell/src/session/templates/` | Goal/planner templates. No L2/L3 spawn law. |
| `crates/codegen/xai-grok-subagent-resolution/` | Role/persona config and tests. No depth law. |
| `crates/common/xai-tool-types/src/task.rs` | Generic parent spawn-tool description. No "many greps / half the window" rule. |

There is no in-repo vendored or bundled skill markdown that talks about agent depth L1/L2/L3 or "L2 MUST spawn".

I did not invent a skill pack to hold the pin.

## In-tree paths that mention the weaker depth

These are the only in-tree hits for the old "many greps / half the window" wording. None of them are skill bodies this specialist owns.

| Path | Edited? | Why left alone |
| ---- | ------- | -------------- |
| `/home/hunter/Projects/surmount/grok-build/AGENTS.md` | No | Other L3. Already states the old softer law is too weak. On the do-not-edit list. |
| `/home/hunter/Projects/surmount/grok-build/FORK.md` | No | Other L3. Already pins three layers always. On the do-not-edit list. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | No | Other L3. Already has the stronger Token efficiency section and says the older softer rule is replaced. On the do-not-edit list. |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/src/builder.rs` | No | Not a skill body. `CHILD_TASK_DESCRIPTION` (around lines 1242–1258) still teaches L2 the weaker rule: spawn L3 if the work needs many greps, reads, or edits; stop solo after about half the window. Tests around 1545–1561 assert that wording. Product prompt / spawn-tool text, not bundled skill markdown. |

## Related leftover that is not weaker-depth wording

| Path | Edited? | Why left alone |
| ---- | ------- | -------------- |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/templates/subagent_prompt.md` | No | Shared L2 and L3 system template. Tells every subagent it is a focused worker that should use the tools. Not a skill body. Pinning "L2 never does tools" here would also bind L3 specialists. |

## Leftovers that belong to other L3s (not touched)

- `/home/hunter/.grok/AGENTS.md`
- `/home/hunter/Projects/surmount/grok-build/AGENTS.md`
- `/home/hunter/.agents/skills/hierarchically-structured-subagents/SKILL.md`
- `/home/hunter/.agents/skills/shared/references/subagent-token-strategy.md`
- `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md`
- `/home/hunter/.agents/skills/implement/SKILL.md`
- `/home/hunter/.agents/skills/shared/personas/implementer.md`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md`
- `/home/hunter/Projects/surmount/grok-build/FORK.md`
- `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md`

Host `~/.agents/skills/**` and `~/.grok/bundled/skills/**` are outside this repo. They were not walked for edits.

## Bottom line

No product-bundled skill text exists in this tree to pin. The live weaker L2 spawn sentence still sits in `crates/codegen/xai-grok-agent/src/builder.rs`, which is product spawn-tool copy, not a skill body.
