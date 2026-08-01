# Plan: structured todos — fib leaves, progress, no casual reset

## Context

Operator wants session todos that are:

1. **Progress-coupled** — progress report reflects real work, not flat row counts
2. **Fibonacci-sized** — atomic leaves only size **1** or **2**; anything larger
   must split into sub-steps; **totals only from leaf sizes** (parents never
   double-count)
3. **Not casually reset** — structured, well-defined, intentional use
4. **Integrated into `/plan` and `/implement`** (and skill rules)

Explore: `/tmp/grok-1000/grok-explore-todo-structure-skills.md`

## Today

| Piece | State |
|-------|--------|
| Progress badge | `done/total` **item counts** (cancelled excluded) |
| Size / effort | **None** |
| Hierarchy | optional `meta.parentId` / `kind` — not enforced |
| Reset | `merge:false` keep-unless-mentioned for protected prefixes; bare/`design:` wipeable |
| Skills | merge-only law; phase scaffolds; no fib / leaf-weighted progress |

## Rules (product + process)

### Fibonacci leaves

| Rule | Detail |
|------|--------|
| Allowed leaf sizes | **1** or **2** only (Fibonacci) |
| Size > 2 | **Reject** at tool (or require decomposition) — agent must split into children |
| Parents / containers | `meta.kind` ∈ `phase` \| residual group; **no size** (or size ignored) |
| Work leaves | `meta.kind` = `work` \| `child` (or no children) with `size` 1\|2 |
| Totals | Σ leaf sizes only; completed = Σ size of completed leaves; cancelled excluded |

### Progress report

| Surface | Formula |
|---------|---------|
| Badge (default) | `completed_points / total_points` leaf fib (fallback to counts if no sizes) |
| Pane header | same + optional `N leaves · M pts` |
| Tool result | include `progress: { completed, total, pct, leaves_done, leaves_total }` |

### No casual reset

| Rule | Detail |
|------|--------|
| Default | **always** `merge: true` upsert |
| `merge: false` | Keep protected prefixes; prefer soft-reject when call would drop unprotected without explicit clear intent |
| Skills | **Never** teach “start clean” wipe; reseed own namespace only |
| Namespaced ids | Prefer `plan:` `impl:` `feat:` `bug:` `residual:` … — discourage bare ids for multi-step work |
| Cleared archive | Keep; intentional prune only |

### Skill integration

| Skill | Change |
|-------|--------|
| `_SKILL_RULES` | Fib leaves 1\|2; progress = leaf pts; never casual wipe; structure meta |
| `/plan` | Steps → work leaves size 1\|2; break >2; handoff `impl:*` with sizes |
| `/implement` | Phase ids = **phase** (unsized); work under `impl:*` or `feat:*` children with sizes; progress from leaves |

## Approach

### Slice A — Schema + validation (TDD)

- First-class optional `size: u8` on `TodoItem` / `TodoUpdate` (1 \| 2 only)
- On write: reject size ∉ {1,2}; reject size on item that has children if we detect graph; parents without size OK
- `compute_progress(state)` → leaf-only points
- Tests: reject 3/5/8; accept 1/2; parent+children total = sum children only

### Slice B — Progress UI

- `TodoCounts` / badge: prefer points when any leaf has size; else legacy counts
- Pane header progress line
- Tool JSON progress block

### Slice C — Anti-reset polish

- Expand soft guidance; optionally protect `design:`
- Tool result warning when merge:false would have wiped unprotected (archive already)
- Prompt.md: intentional structure + fib + merge-only

### Slice D — Host + branch skill/process pins

- Dual-pin `_SKILL_RULES`, plan, implement, product `AGENTS.md` / `RESIDUAL` / research

## Critical files

| Path | Why |
|------|-----|
| `xai-grok-tools/.../todo/mod.rs` | schema, merge, progress |
| `xai-grok-pager/.../todo_pane.rs` | badge + pane |
| `xai-grok-agent/templates/prompt.md` | agent guidance |
| `~/.agents/skills/_SKILL_RULES…` | process law |
| plan + implement SKILL.md | scaffold law |

## Non-goals

- Full PM hierarchy product (epics/sprints)
- Auto-splitting model content into children (agent responsibility; tool enforces max 2)
- OpenCode full-replace API rewrite

## Verification

```bash
cargo test -p xai-grok-tools --lib -- todo size progress fib leaf
cargo test -p xai-grok-pager --lib -- todo badge progress
```

## Research

`doc/dev/research/todo-progress-fib-2026-07-26.md`
