# Structured todos: progress, Fibonacci leaves, intentional use

Date: 2026-07-26 · status: **shipped (product slices A–C)**  
Explore: `/tmp/grok-1000/grok-explore-todo-structure-skills.md`  
Plan: [`.agents/plans/plan-todo-progress-fib.md`](../../../.agents/plans/plan-todo-progress-fib.md)

## Operator intent

1. Progress report **coupled** to the todos system (not prose-only)
2. **Fibonacci sizing** — atomic steps size **1 or 2** only; anything larger
   must be broken into sub-steps; **totals only from atomic leaves**
3. Todos **not casually reset** — structured, well-defined, intentional
4. Integrated into **planning and implementing skills**

## Product truth (post-ship)

| Piece | State |
|-------|--------|
| Size field | Optional first-class `size: 1\|2` on `TodoItem` / `TodoUpdate`; `meta.size` fallback normalized onto field |
| Tool progress | `compute_leaf_progress` — leaf-only points when any leaf sized; parents never count; legacy item counts otherwise |
| Badge | `N/M pts` in points mode (leaf sizes only; parents excluded via `parentId` graph + `meta.id` on Plan); else legacy `done/total` |
| Parent size | Reject when write **sets** size on a parent; **clear** retained size when children attach later |
| Hierarchy | Soft `meta.parentId` / `kind` — no full PM enforcement (orphan parentId, one in_progress, …) |
| Reset | `merge:false` keep-unless-mentioned for protected prefixes (`plan:`, `impl:`, `pr-`, `recon:`, `residual:`, `ask:`, `feat:`, `bug:`) + archive warning; bare ids still wipeable |
| OpenCode path | Full-replace adapter; progress on success; no first-class size on its own schema |

### Soft gaps (non-goals left open)

- No hard ban inventing bare unprotected ids
- No full hierarchy product (epics/sprints, indent tree, size chip on rows)
- No auto-split of size>2 into children (agent must split; tool rejects)

## Rules of thumb

| Role | Size | Counts toward progress? |
|------|------|-------------------------|
| Phase / residual container | none | no |
| Work leaf | 1 or 2 | yes |
| Cancelled leaf | — | no |
| Parent with children | none (cleared if set) | no (children only) |

Progress: `Σ size(completed leaves) / Σ size(active leaves)`  
Fallback: legacy counts if no sized leaves exist.

## Skill dual-pin

Host: `_SKILL_RULES`, `plan/SKILL.md`, `implement/SKILL.md`  
Branch: `AGENTS.md` L1 note, this research, `RESIDUAL.md`
