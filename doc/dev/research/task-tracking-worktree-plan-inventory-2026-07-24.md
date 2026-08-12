# Task tracking / worktrees / plan terminology — inventory

**Date:** 2026-07-24  
**Mode:** research inventory (no product code change).  
**Workspace:** `/home/hunter/Projects/surmount/grok-build`  
**Honesty note:** This file was **synthesized from a parent handoff summary** when
the explore agent did not write an inventory. Product code (todo tool, plan
mode, worktree isolation, pending prompts) should be **re-verified** against
live sources before implementing product changes. Treat tables as a working
map, not a signed API contract.

**Related campaign:**
[`doc/dev/campaigns/operator-orchestration-2026-07.md`](../campaigns/operator-orchestration-2026-07.md)

---

## Plain answer

| Area | What exists today | Friction |
|------|-------------------|----------|
| **Todos** | Flat tool surface: `id` / `content` / `status` only | Priority and meta may exist in store but are **not** tool-writable; hierarchy is faked |
| **Hierarchy** | Skills namespace ids (`plan:*`, `impl:*`, `pr-N:*`) | Not real nesting; mid-work typing is still the main channel |
| **Pending prompts** | Queue when a turn is running; hold while waiting on subagents | Confusion when parent looks idle but children still run |
| **Worktrees** | Hints for `/new` / `/fork` | **No** global ban on subagent `isolation: worktree`; operator wants disable default |
| **“Plan”** | Product plan mode vs `/plan` skill vs Rhai `phase()` overload | Same word, three meanings; user-facing jargon leaks |
| **Survival** | Dual-pin (host skills + branch process) | Task model law must not live only in chat |

---

## 1. Todo / task tool surface (as reported)

### Writable via tool (flat)

| Field | Tool-writable? | Notes |
|-------|----------------|-------|
| `id` | Yes | Stable string; skills invent namespace prefixes |
| `content` | Yes | Human-readable task text |
| `status` | Yes | e.g. pending / in_progress / completed / cancelled |

### Present but not tool-writable (as reported)

| Field | Tool-writable? | Notes |
|-------|----------------|-------|
| Priority | Stored, **not** via tool | Operators cannot re-rank through the todo tool alone |
| Meta / extras | Stored, **not** via tool | Hierarchy, parent links, residual pointers not first-class |

**Implication:** multi-level work is encoded in **id namespaces and prose**,
not in a real tree API. Mid-session “what is open?” still collapses to a flat
list plus chat typing.

**Re-verify before product work:** todo tool schema, storage shape, UI
surfaces, any undocumented merge/priority paths.

---

## 2. Fake hierarchy via skill id namespaces

Skills simulate structure by prefixing todo ids:

| Prefix pattern | Typical owner skill / loop | Meaning (convention only) |
|----------------|----------------------------|---------------------------|
| `plan:*` | `/plan` / plan-related flows | Planning slice items |
| `impl:*` | `/implement` | Implementation residual slices |
| `pr-N:*` | `/pr-babysit` or PR loops | Per-PR work items |

| Property | Reality |
|----------|---------|
| Nested children in tool | **No** — still flat |
| Parent/child links | **Convention in id string**, not schema |
| Cross-skill continuity | Fragile; compaction + new session lose chat rationale |
| Disk residual | Separate (`RESIDUAL.md`, research notes) — not auto-linked to todos |

**Implication:** better multi-level tracking cannot be “more prefix cleverness”
alone. Need explicit levels (disk residual vs session todos vs child join notes
vs pending prompts) — see campaign §4.

---

## 3. Pending prompts queue

| State | Behavior (as reported) | Operator confusion |
|-------|------------------------|--------------------|
| Turn running | New user/operator text can **queue** as pending prompts | Expected: “wait, then run” |
| Waiting on subagents | Prompts may **hold** until children finish | Parent UI may look idle while work continues |
| Parent idle, children live | Unclear whether typed text is next-turn intent or noise | Typing becomes the only mid-work channel |

**Implication:** pending prompts should stay **intentional next-turn** only
(L3 in the campaign model), not a substitute for L0 residual docs or L1
session todos. Status UX should make “children still running” visible so
queued text is not misread as “agent ignored me.”

**Re-verify:** pending-prompt enqueue rules, hold conditions, restore order,
interaction with plan approve / interject surfaces.

---

## 4. Worktrees

| Capability | Status (as reported) |
|------------|----------------------|
| Hints for `/new` | Present (suggest worktree-oriented new sessions) |
| Hints for `/fork` | Present (fork research chat patterns) |
| Global config to disable worktrees | **Missing / not operator-complete** |
| Subagent `isolation: worktree` | **Not globally banned** — can still create/use worktrees |
| Operator preference | Default **off** / none for subagents; avoid wasteful parallel trees |

**Implication:** need config that disables worktree isolation for **subagents
too**, plus skill defaults of “no worktree unless explicitly asked.” See
campaign §7.

**Re-verify:** isolation modes, config keys, `/new` `/fork` hint text, any
skill prose that still defaults to worktrees.

---

## 5. Plan terminology collision

Three different “plans” share language:

| Term in the wild | What it actually is | User-facing? |
|------------------|---------------------|--------------|
| **Product plan mode** | Built-in plan approval / exit-plan surface | Yes (UI) |
| **`/plan` skill** | Host (or multi-source) skill for design-before-implement | Slash skill |
| **Rhai `phase()`** | Workflow orchestration step (“phase” overload) | Internal / workflow author |

Related UX research (approve/comment flush gaps) lives under
`doc/dev/research/plan-approve-*.md` — orthogonal to terminology, but same
word family.

| Problem | Effect |
|---------|--------|
| Same word, three systems | Agents and humans talk past each other |
| Tracks / workstreams / phases in user prose | Opaque jargon (project AGENTS: ban user-facing) |
| Ephemeral plan IDs | Clutter todos; do not survive recon or compaction as residual authority |

**Standard:** campaign §3 terminology table — user-facing plain language;
internal ids namespaced; ban tracks/workstreams in user-facing copy.

---

## 6. Dual-pin / recon survival (task-model law)

Task-tracking **process law** (what levels mean, when to pin residual, parent
= HITL only) must survive:

| Layer | Survives import/onto? | Role for this inventory’s topics |
|-------|----------------------|----------------------------------|
| Chat-only todo rationale | No | Compaction kills it |
| Session todos | Session only | L1 — not residual authority |
| `RESIDUAL.md` / `AGENTS.md` / `FORK.md` | Branch + `FORK_PATHS` | L0 open intent + process law |
| Host skills that encode task hierarchy | Host tree | Operator behavior; dual-pin if product-facing |
| Research under `doc/dev/` | Via `doc/dev` in `FORK_PATHS` | This inventory class |

Full dual-pin mechanics:
[`tools-self-improve-survival-2026-07-24.md`](./tools-self-improve-survival-2026-07-24.md).

---

## 7. Gaps → campaign hooks

| Gap | Campaign section |
|-----|------------------|
| Flat todos + fake id hierarchy | §4 Task model levels |
| Mid-work typing only channel | §4 L1/L2/L3; §8 plan → residual → implement |
| Pending-prompt idle confusion | §4 L3; acceptance criteria |
| Worktree default / no global ban | §7 Worktree policy |
| Plan / phase / skill name collision | §3 Terminology |
| Docs can lie; verify before claim | §2 Principles |
| Tools must improve + survive recon | §9 slices + dual-pin |

---

## 8. Non-claims

- Does **not** assert exact Rust type names or config key strings without a
  later code pass.  
- Does **not** invent product APIs (priority write, nested todos, worktree
  config) as already shipped.  
- Does **not** replace product plan-mode bugfix research (`plan-approve-*`).  
- Live behavior may have moved; **re-verify** before implementation slices
  that touch product code.

---

## 9. Sources

| Source | Use |
|--------|-----|
| Parent task-tracking summary (2026-07-24) | Todo flatness, namespaces, pending prompts, worktrees, plan collision |
| `doc/dev/research/workflow-skill-git-recon-inventory-2026-07-24.md` | Git recon skill/workflow split; signing |
| `doc/dev/research/tools-self-improve-survival-2026-07-24.md` | Dual-pin, residual, FORK_PATHS |
| `doc/dev/research/plan-approve-*.md` | Plan **UI** flush issues (related word “plan”, different bug class) |
| Project `AGENTS.md`, `RESIDUAL.md`, global `~/.grok/AGENTS.md` | HITL parent, residual honesty, plain language |

---

*End inventory.*
