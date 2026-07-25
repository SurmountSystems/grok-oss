# Operator orchestration (2026-07)

Standing pins for how operators (and skills) track work and spawn children.

## Task levels

| Level | Authority | Examples |
|-------|-----------|----------|
| **L0** | Durable on-disk residual | `RESIDUAL.md`, campaign docs, `doc/dev/research/*` |
| **L1** | Session todos (namespaced) | `plan:*` `impl:*` `pr-N:*` `recon:*` `residual:*` |
| **L2** | Child join notes | `grok-impl-summary-*`, explore maps, short review files |

Session todos are **not** residual authority. Merge only; never wipe foreign
prefixes. Host skill rules:
`~/.agents/skills/_SKILL_RULES-read-first-pls.md` § *Todo namespaces*.

## Worktrees

- **Prefer** `isolation: none` (shared workspace) for subagents.
- Config: `[subagents] allow_worktree = false` → spawn **forces** none.
- Skills: `/implement` prefer none; `/execute-plan` defaults to **shared-cwd**
  (`isolation_mode`), serial/disjoint writers, on-disk reviews; worktree only
  when allowed; fall back if spawn forces none / create fails. Join:
  [`execute-plan-no-worktree-2026-07-24.md`](../research/execute-plan-no-worktree-2026-07-24.md).
- User-guide: `05-configuration` (Subagents), `16-subagents` (Isolation).

## Dual-pin

| Layer | Path |
|-------|------|
| Branch process | `AGENTS.md`, `FORK.md`, `RESIDUAL.md` |
| Host skills | `~/.agents/skills` (plan / implement / execute-plan / rules / token strategy) |
| Product code + docs | `SubagentsConfig.allow_worktree`, shell spawn force-none, user-guide |

Join writeup:
[`doc/dev/research/task-worktree-pins-2026-07-24.md`](../research/task-worktree-pins-2026-07-24.md).
