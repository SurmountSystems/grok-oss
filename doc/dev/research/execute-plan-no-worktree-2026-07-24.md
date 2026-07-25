# `/execute-plan` shared-cwd when worktrees banned (2026-07-24)

**Slice:** operator orchestration — skill half of `allow_worktree` adaptation.

## Problem

Product already ships `[subagents] allow_worktree = false` → spawn **forces**
`isolation = none`. The host `/execute-plan` skill still described a
worktree-first protocol (push branch into WT, `git fetch <WT>`, reviewer
`cwd=<worktree>`, teardown before stack rewrite). That assumed worktrees always
existed and could break or invent paths when policy forbids them.

## Change (host skill)

File: `~/.agents/skills/execute-plan/SKILL.md`

| Area | Behavior |
|------|----------|
| Default | `isolation_mode = "shared-cwd"` (prefer none) |
| Detection | No fragile TOML parser. Prefer none; if spawn forces none / create fails / operator has `allow_worktree=false` (AGENTS, user, prior spawn) → shared-cwd. Optional note if config already read. |
| Shared-cwd | `isolation: none`; null `worktree_path`; serial or **disjoint** `pr.files` concurrency; `commit_sha = git rev-parse <pr.branch>`; no WT push/fetch/teardown |
| Worktree | Prior protocol unchanged when mode is worktree and create succeeds |
| Reviewers | On-disk review files under `scratch_dir` always; `cwd` only when WT exists |
| State | `isolation_mode` persisted; resume defaults missing field to `shared-cwd` |
| Fall back | Mid-run WT create fail → switch to shared-cwd, continue |

## Dual-pin (branch)

| File | Note |
|------|------|
| `AGENTS.md` | execute-plan honors allow_worktree / shared-cwd |
| `FORK.md` | hierarchical checkbox for execute-plan + allow_worktree |
| `RESIDUAL.md` | skill half closed; OSS default-false still open |
| Campaign | `doc/dev/campaigns/operator-orchestration-2026-07.md` |
| Prior research | `doc/dev/research/task-worktree-pins-2026-07-24.md` |

## Harness

`~/.agents/skills/skill-maintenance/test-required-pins.sh` asserts execute-plan
SKILL mentions `allow_worktree`, `shared-cwd`, `isolation_mode`.

## Not in this slice

- Product default flip `allow_worktree = false` for OSS installs
- Product namespaced-todo API / L2 notes channel
- No git commit (human-only)

## Verify

```bash
~/.agents/skills/skill-maintenance/test-required-pins.sh
rg -n 'allow_worktree|shared-cwd|isolation_mode' ~/.agents/skills/execute-plan/SKILL.md | head
```
