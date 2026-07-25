# Skill pin — hard parent-coordinator / spawn-first (orchestrators)

**Date:** 2026-07-24  
**Scope:** Embed short-form hard stop in orchestrator skills; link canonical rule.  
**Canonical:** `~/.grok/AGENTS.md` § *Hard stop — parent is coordinator only*

## Rule (short form embedded in skills)

- CI fail, regression, multi-file diagnosis, non-trivial fix → **first** action is `spawn_subagent`, not parent grep / `gh run view` / test-file reads / nextest.
- Parent may: goals, spawn/wait, short on-disk join notes, hand signed git cmds, brief status.
- Parent must not: CI log pulls, open failing tests, re-run nextest, product edits (except skill-local exceptions), re-do child greps “to be sure.”
- Failure mode to kill: parent research then spawn. Spawn first; children own fetch/read/fix. Join on disk only.

## Files touched

| File | One-line change |
|------|-----------------|
| `~/.agents/skills/pr-babysit/SKILL.md` | Added **Hard stop** under Sub-agents: spawn first for CI/diagnosis; light parent auth/state only; children own logs/fix. |
| `~/.agents/skills/implement/SKILL.md` | Added **Hard stop** under Sub-agents (above existing regressions bullet); coordinator-only + spawn-first + join on disk. |
| `~/.agents/skills/execute-plan/SKILL.md` | Added **Hard stop** under Sub-agents; parent keeps branch/stack/conflict-as-git-coord; no parent product marathons. |
| `~/.agents/skills/upstream-export-import/SKILL.md` | **New Sub-agents** section with Hard stop (onto/mega-pick/conflict: spawn first; ~2–3 concurrent; join on disk). |
| `~/.agents/skills/check-work/SKILL.md` | Light Hard stop: spawn verifier first; no parent re-verify marathon; FAIL → implementer if multi-file. |
| `~/.agents/skills/help/SKILL.md` | Sub-agents help pointer now leads with AGENTS.md Hard stop (spawn first), then token-efficiency docs. |

## Non-goals (this pass)

- No git commit.
- No bulk find-and-replace.
- Did not rewrite pr-babysit Step 5 / commit-push wording (human-only commit policy already in skill header).
- Did not edit non-orchestrator skills (personas, create-skill, etc.).

## Related

- `doc/dev/research/skill-subagent-pin-inventory-2026-07-24.md`
- `doc/dev/research/skill-subagent-pin-source-text-2026-07-24.md`
- `shared/references/subagent-token-strategy.md` (via `~/.agents/skills/`)
