# Operator orchestration (2026-07)

Standing pins for how operators (and skills) track work and spawn children.

## Task levels

| Level | Authority | Examples |
|-------|-----------|----------|
| **L0** | Durable on-disk residual | `RESIDUAL.md`, campaign docs, `doc/dev/research/*` |
| **L1** | Session todos (namespaced) | `plan:*` `impl:*` `pr-N:*` `recon:*` `residual:*` |
| **L2** | Child join notes | `grok-impl-summary-*`, explore maps, short review files |
| **Operator notes** | Session-local annotations (not turns) | `/note` — see `doc/dev/research/notes-channel-2026-07-24.md` |

Session todos are **not** residual authority. Merge only; never wipe foreign
prefixes. Host skill rules:
`~/.agents/skills/_SKILL_RULES-read-first-pls.md` § *Todo namespaces*.
Product (2026-07-24): `todo_write` optional `priority` + `meta` (prefer
`meta.kind`); `merge: false` keep-unless-mentioned for protected prefixes.
Join: [`todo-levels-product-2026-07-24.md`](../research/todo-levels-product-2026-07-24.md).

## Worktrees

- **Prefer** `isolation: none` (shared workspace) for subagents.
- Product default: `[subagents] allow_worktree = false` (empty config
  force-none; set `true` to opt in). Spawn **forces** none when false.
- Skills: `/implement` prefer none; `/execute-plan` defaults to **shared-cwd**
  (`isolation_mode`), serial/disjoint writers, on-disk reviews; worktree only
  when allowed; fall back if spawn forces none / create fails. Join:
  [`execute-plan-no-worktree-2026-07-24.md`](../research/execute-plan-no-worktree-2026-07-24.md).
- User-guide: `05-configuration` (Subagents + migration), `16-subagents`
  (off by default + opt-in).

## Dual-pin

| Layer | Path |
|-------|------|
| Branch process | `AGENTS.md`, `FORK.md`, `RESIDUAL.md` |
| Host skills | `~/.agents/skills` (plan / implement / execute-plan / rules / token strategy) |
| Product code + docs | `SubagentsConfig.allow_worktree`, shell spawn force-none, user-guide |

Join writeup:
[`doc/dev/research/task-worktree-pins-2026-07-24.md`](../research/task-worktree-pins-2026-07-24.md).
