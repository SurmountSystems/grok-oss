# Skill / reference inventory — subagent orchestration pins

**Date:** 2026-07-24  
**Scope:** files that talk about subagents, parent orchestration, regressions,
CI investigation, token efficiency, or “when to spawn.”  
**Hard rule under test:** `~/.grok/AGENTS.md` § *Hard stop — parent is
coordinator only* (pinned 2026-07-24) + project
[`AGENTS.md`](../../../AGENTS.md) § *Subagents — parent is coordinator only*
— first tool turn after CI fail / regression / multi-file = `spawn_subagent`;
parent must not pull CI logs, open failing tests, or re-do child greps.

## Where things live

| Tree | Role | Lands on grok-build git branch? |
|------|------|----------------------------------|
| `~/Projects/surmount/grok-build/**` | Product + project rules + research notes | **Yes** (if tracked) |
| `~/.agents/skills/**` | Maintained skill home (Zed/Surmount first; own git) | **No** |
| `~/.grok/AGENTS.md` | Global parent runtime pins | **No** |
| `~/.grok/docs/user-guide/**` | Installed TUI user guide (often mirrors product) | **No** (install/runtime) |
| `~/.grok/skills/**` | Sparse/user Grok skill mirrors (often lag) | **No** |
| `~/.grok/bundled/skills/**` | Bundled skill pack shipped with Grok install | **No** (install); source of truth for many of these is in-repo under `crates/…` only if product embeds them differently — skills themselves are home-dir |
| In-repo `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | Product user-guide source | **Yes** |
| In-repo `crates/codegen/xai-grok-agent/templates/subagent_prompt.md` | Product subagent system template | **Yes** |
| Project skills under `.agents/skills/` or `.grok/skills/` on branch | None found for orchestration pins | n/a |

**No** tracked `skills/`, `.agents/skills/`, or agent skill packs inside
`grok-build` on `onto-xai/*` for implement/pr-babysit/etc. Orchestration
policy for agents is **home-dir skills + AGENTS.md**; product only documents
subagent *features*.

---

## Gap summary (Hard stop vs current skill pins)

| Layer | Has older “regressions → subagents / join on disk”? | Has **Hard stop** (spawn-first on CI; parent must not `gh run` / greps)? |
|-------|-----------------------------------------------------|--------------------------------------------------------------------------|
| `~/.grok/AGENTS.md` | Yes (§ Regressions + § Hard stop) | **Yes** (canonical) |
| Project `AGENTS.md` | Yes (hard coordinator + onto conflict subagents) | **Yes** |
| `subagent-token-strategy.md` | Yes (full) | **No** — no “first tool turn” / CI-log ban bullet |
| `_SKILL_RULES-read-first-pls.md` | Yes | **No** Hard stop wording |
| Orchestrator skills (implement, plan, check-work, review) | Partial bullets | **No** explicit Hard stop |
| `pr-babysit` | Fixes in worktree children (practice) | **No** Hard stop pin; skill text still narrates parent-adjacent CI fix commits historically |
| `execute-plan`, `design` | Sub-agents tables only | **No** regression/Hard stop |
| User-guide `16-subagents.md` (home + in-repo) | Product “when to use” only | **Missing** § *Token efficiency* that AGENTS/help still link to |
| `~/.grok/skills/*` mirrors | Stale / thin | **No** |
| `~/.grok/bundled/skills/*` | Peer of agents; no Hard stop string matches | **No** |

---

## Full inventory table

| path | in_repo? | relevance (1 line) | edit needed? (Y/N + why) |
|------|----------|--------------------|--------------------------|
| `~/.grok/AGENTS.md` | N | Canonical parent pins: regressions, **Hard stop**, token economics, strategic parallel max | **N** for content (already has Hard stop); keep as source of truth |
| `$REPO/AGENTS.md` | **Y** | Project hard “parent is coordinator only”; onto multi-file conflict subagent table | **N** unless adding one-line pointer to skill deep guide for non-onto CI |
| `~/.agents/skills/shared/references/subagent-token-strategy.md` | N | Deep “when to spawn”, regressions, anti-patterns, micro-flow, skill-author requirements | **Y** — add Hard stop: first tool = spawn; ban parent `gh run`/CI logs/nextest; link AGENTS § Hard stop |
| `~/.agents/skills/_SKILL_RULES-read-first-pls.md` | N | Author checklist: token efficiency + sub-agent strategy + regressions item 8/12 | **Y** — pin Hard stop in § Sub-agent strategy + Recent reconciliations (2026-07-24) |
| `~/.agents/skills/implement/SKILL.md` | N | Primary implement→review orchestrator; strong regressions + token bullets | **Y** — one Hard stop bullet (CI fail / multi-file → spawn first; parent no `gh`/greps); fix dead link to user-guide § Token efficiency if section still missing |
| `~/.agents/skills/pr-babysit/SKILL.md` | N | CI fail / review / conflict babysit; all fixes in worktree subagents | **Y** — Sub-agents: parent never pulls CI logs or diagnoses in parent; spawn first on red CI; join on disk/JSON only; reinforce no parent marathon before Step 4 |
| `~/.agents/skills/check-work/SKILL.md` | N | Verifier subagent; regression verification stays in child | **Y** — light: forbid parent re-running builds/tests after FAIL summary before re-spawn; point Hard stop |
| `~/.agents/skills/plan/SKILL.md` | N | Explore fan-out; multi-file root-cause plans not in parent | **N** for Hard stop (already solid); optional link to Hard stop for CI-plan cases |
| `~/.agents/skills/review/SKILL.md` | N | One reviewer child; no parent re-author findings | **N**/light — optional Hard stop cross-link |
| `~/.agents/skills/execute-plan/SKILL.md` | N | Mega PR-DAG orchestrator; heavy subagent protocol | **Y** — add Sub-agents regression/Hard stop ownership (CI red mid-stack → child, not parent log dump) |
| `~/.agents/skills/design/SKILL.md` | N | Writer/reviewer loop; spawn protocol | **N**/low — optional “no parent diagnosis marathons” if design starts from a bug report |
| `~/.agents/skills/skill-maintenance/SKILL.md` | N | Quality pass enforces regression ownership in orchestrators | **Y** — Required pins table: add Hard stop row; assert workers check first-tool-turn language |
| `~/.agents/skills/help/SKILL.md` | N | Points users at 16-subagents § Token efficiency + deep guide | **Y** — § Token efficiency **missing** in guide; point at AGENTS Hard stop + deep guide until product section exists |
| `~/.agents/skills/create-skill/SKILL.md` | N | Mandates Sub-agents section + token bar for new skills | **N**/low — optional “if skill can face CI/regression, require Hard stop sentence” |
| `~/.agents/skills/upstream-export-import/SKILL.md` | N | Onto/import scripts; **no** Sub-agents section | **Y** — multi-file conflict/CI after onto: spawn first; defer to project AGENTS onto table + Hard stop (skill is high-risk parent-marathon magnet) |
| `~/.agents/skills/resume-claude/SKILL.md` | N | Explicitly **do not** spawn | **N** |
| `~/.grok/bundled/skills/resume-codex/SKILL.md` | N | Resume other hosts; no spawn policy | **N** |
| `~/.grok/bundled/skills/resume-cursor/SKILL.md` | N | Same | **N** |
| `~/.agents/skills/pptx/SKILL.md` + `editing.md` | N | Parallel slide QA subagents | **N** (unrelated to CI Hard stop) |
| `~/.agents/skills/imagine/SKILL.md` | N | Prefer parallel tools not subagents | **N** |
| `~/.agents/skills/xlsx/SKILL.md` | N | Spawn alias only | **N** |
| `~/.agents/skills/grok-tool-policy/SKILL.md` | N | Do not spawn for policy edits | **N** |
| `~/.agents/skills/check-work/references/verifier-prompt.md` | N | Verifier severity includes regression | **N** |
| `~/.agents/skills/TASKS.md` | N | P1 token-efficiency mega-orchestrator backlog | **N**/note only — track Hard stop as maintenance task if desired |
| `~/.agents/skills/shared/personas/*` | N | No spawn/parent policy text | **N** |
| `~/.grok/docs/user-guide/16-subagents.md` | N | Product “when to use”; **no** Token efficiency section | **Y** — add § Token efficiency (or stop linking it); optionally note agent Hard stop is in AGENTS not product guide |
| `$REPO/crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | **Y** | Same product guide source (ends at “When to Use”; no token/Hard stop) | **Y** — if shipping token guidance to users, add short §; Hard stop is operator/agent policy (AGENTS) more than end-user guide |
| `$REPO/docs/upstream-history.md` | **Y** | Onto conflict subagent fan-out + anti parent-solo marathons | **N**/light — already aligned; optional Hard stop cross-link for post-pick CI |
| `$REPO/crates/codegen/xai-grok-agent/templates/subagent_prompt.md` | **Y** | Child session system prompt template | **N** (product, not orchestrator policy) |
| `$REPO/crates/codegen/xai-grok-shell/README.md` | **Y** | Feature docs for subagents/tools | **N** for Hard stop |
| `~/.grok/skills/check-work/SKILL.md` | N | **Stale** vs agents (long inline verifier; weaker orchestration pins) | **Y** if dual-maintained — prefer agents home; rsync/reconcile on skill-maintenance |
| `~/.grok/skills/help/SKILL.md` | N | Thin doc map; no Hard stop | **Y** only if kept in sync with agents help |
| `~/.grok/skills/upstream-export-import/SKILL.md` | N | Mirror; check vs agents | reconcile via skill-maintenance |
| `~/.grok/bundled/skills/implement/SKILL.md` | N | Bundled peer of agents implement | **Y** after agents edit (skill-maintenance copy) |
| `~/.grok/bundled/skills/pr-babysit/SKILL.md` | N | Bundled peer | **Y** after agents edit |
| `~/.grok/bundled/skills/execute-plan/SKILL.md` | N | Bundled peer | **Y** after agents edit |
| `~/.grok/bundled/skills/review/SKILL.md` | N | Bundled peer | light after agents |
| `~/.grok/bundled/skills/create-workflow/SKILL.md` | N | Rhai workflow agent() orchestration (product skill) | **N** for Hard stop |
| Session/debug under `~/.grok/sessions/**`, `debug/**` | N | Noise / runtime | **Ignore** |

---

## Priority targets (named in request)

| Name | Path(s) | Status vs Hard stop |
|------|---------|---------------------|
| **implement** | `~/.agents/skills/implement/SKILL.md` (+ bundled peer) | Strong regressions pin; missing Hard stop first-tool-turn / CI-log ban |
| **check-work** | `~/.agents/skills/check-work/SKILL.md` | Child owns heavy verify; missing Hard stop |
| **pr-babysit** | `~/.agents/skills/pr-babysit/SKILL.md` | Best *practice* (CI in child) but no explicit Hard stop / “don’t parent-marathon first” |
| **plan** | `~/.agents/skills/plan/SKILL.md` | Good multi-file explore pin |
| **review** | `~/.agents/skills/review/SKILL.md` | Good no-reauthor pin |
| **resume-*** | resume-claude (agents); resume-codex/cursor (bundled) | Correctly no-spawn |
| **upstream-export-import** | agents (+ grok/skills mirror) | **Missing** Sub-agents entirely — high risk for onto/CI parent marathons |
| **_SKILL_RULES** | `~/.agents/skills/_SKILL_RULES-read-first-pls.md` | Author law; needs Hard stop pin |
| **subagent-token-strategy** | `~/.agents/skills/shared/references/subagent-token-strategy.md` | Deep guide; needs Hard stop section mirroring AGENTS |
| **user-guide 16-subagents** | home `~/.grok/docs/...` + **in-repo** `crates/.../16-subagents.md` | Links from AGENTS/help claim § Token efficiency — **section does not exist** |

---

## Top 8 edit targets (ranked)

Ranked for closing the gap to **Hard stop / spawn-first on CI+regression+multi-file**, not for general docs polish.

| Rank | Path | in_repo? | Why |
|------|------|----------|-----|
| 1 | `~/.agents/skills/shared/references/subagent-token-strategy.md` | N | Single deep guide all skills link; must mirror AGENTS Hard stop (first tool turn, banned parent tools, failure mode “grep then spawn”) |
| 2 | `~/.agents/skills/_SKILL_RULES-read-first-pls.md` | N | Forces every create/maintenance pass to require Hard stop language in multi-step diagnosis skills |
| 3 | `~/.agents/skills/pr-babysit/SKILL.md` | N | Primary CI-red skill; parent must not pre-fetch logs before spawn; align Sub-agents with Hard stop |
| 4 | `~/.agents/skills/implement/SKILL.md` | N | Default multi-file implement path; add Hard stop bullet next to existing regressions pin |
| 5 | `~/.agents/skills/upstream-export-import/SKILL.md` | N | Onto/import workflows regularly hit multi-file + CI; currently no Sub-agents section |
| 6 | `~/.agents/skills/execute-plan/SKILL.md` | N | Longest orchestrator; no regression/Hard stop ownership line |
| 7 | In-repo + home `…/user-guide/16-subagents.md` | **Y** + N | Dead link from `~/.grok/AGENTS.md` + help/implement to missing § *Token efficiency*; either add short section or retarget links to deep guide/AGENTS |
| 8 | `~/.agents/skills/skill-maintenance/SKILL.md` + `check-work` / `help` | N | Maintenance assertion + help pointers so Hard stop does not rot; check-work light Hard stop for FAIL loops |

**Already good enough (do not re-rank as primary work):** project `AGENTS.md` Hard stop section; plan/review regression bullets; resume-* no-spawn; product shell README feature docs.

**Do not put Hard stop only in chat.** After skill edits, agents skill git is separate from grok-build; project branch only needs (7) if product guide is updated, plus this research note.

---

## Suggested edit shape (for implementers — not done here)

1. **Deep guide:** new subsection under Regressions, copy Hard stop bullets from `~/.grok/AGENTS.md` (parent may / must not / first tool turn / failure mode).
2. **_SKILL_RULES:** one table row + reconciliation log line dated 2026-07-24.
3. **pr-babysit / implement / execute-plan / upstream-export-import:** 3–6 lines in Sub-agents (not essays).
4. **User-guide:** either add § Token efficiency (operator-facing summary + link to agent deep guide) **or** retarget AGENTS/help/implement links to `subagent-token-strategy.md` + AGENTS Hard stop.
5. **skill-maintenance:** Required pins table row → Hard stop; §4b assert string present on implement/pr-babysit/plan/check-work/review/execute-plan.
6. Reconcile agents → bundled via `/skill-maintenance` (do not hand-edit only bundled).

---

## Out of scope / noise

- Product CHANGELOGs and shell feature docs mentioning “subagent” as a product capability
- Session compaction segments under `~/.grok/sessions/**`
- Shared personas (behavior, not spawn policy)
- Office skills (pptx/docx/xlsx) spawn patterns unrelated to CI

---

## Return

- **This note:** `$REPO/doc/dev/research/skill-subagent-pin-inventory-2026-07-24.md`
- **Top 8:** strategy deep guide → skill rules → pr-babysit → implement → upstream-export-import → execute-plan → user-guide 16-subagents (dead Token efficiency §) → skill-maintenance/help/check-work glue
