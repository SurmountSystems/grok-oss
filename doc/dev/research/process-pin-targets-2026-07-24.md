# Process pin targets — A/B/C/D inventory (2026-07-24)

Living files that should **receive** or **already hold** pins for:

| Id | Pin |
|----|-----|
| **A** | Parent/main thread = **HITL UX coordinator only**; research + implementation in subagents |
| **B** | **Never assume** — verify with subagents before claiming (docs can lie: “lies, damned lies, and documentation”) |
| **C** | **Skills change documentation** that **survives upstream recon** (import/onto; host skill trees are not product history) |
| **D** | **Hard stop** — spawn-first on CI (parent must not `gh`/grep/nextest/open failing tests first) |

**Already has?** = which of A–D are present on disk now (partial vs full).  
**Must edit** = still needs a pin write for a missing or partial rule that *belongs* in that file.

Evidence base: live greps + reads of home/project AGENTS, user-guide `16-subagents`, `docs/upstream-history.md`, `~/.agents/skills/**`, prior research under `doc/dev/research/skill-*` and `where-skills-come-from-2026-07-24.md`. Inventory rows for Hard stop in older research notes are **stale** where orchestrators/strategy already gained § Hard stop today.

---

## Table

| path | already has? | must edit Y/N | why |
|------|--------------|---------------|-----|
| `/home/hunter/.grok/AGENTS.md` | **A** partial (parent coordinator / regressions / may–must-not; no “HITL UX” framing). **B** no. **C** no. **D** yes (canonical § *Hard stop*, 2026-07-24). | **Y** | Canonical cross-repo process law. Add **A** HITL-UX reframe (parent = human status + goals only). Add **B** never-assume / docs-lie + verify-via-child before claims. Optional one-line **C** pointer: skill-body pins live under `~/.agents`; product recon survival → project `AGENTS`/`FORK`. **D** already complete. |
| `/home/hunter/Projects/surmount/grok-build/AGENTS.md` | **A** partial (hard § *Subagents — parent is coordinator only*). **B** no. **C** no. **D** yes (spawn-first + onto subagent table). | **Y** | Project law for this repo. Same **A** HITL-UX tightening + **B** never-assume. **C** must land here: operator skills are host overlay; process notes that must survive import/onto live in branch docs (`AGENTS`/`FORK`/upstream-*), not only `~/.agents`. **D** already complete. |
| `/home/hunter/Projects/surmount/grok-build/FORK.md` | **A/B/C/D** none (no skills / subagent process). | **Y** | Living fork inventory. Needs short hierarchical **C** note: host `~/.agents/skills` = operator overlay (not absorbed by xAI recon); durable skill-*process* pins → `AGENTS` + this file; optional one-line pointer to Hard stop / parent-coordinator in `AGENTS`. Not a full Hard stop novel. |
| `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md` | none | **N** | Open residual only. Standing process pins do not belong here once decided; migrate to `AGENTS`/`FORK`. |
| `/home/hunter/Projects/surmount/grok-build/docs/upstream-history.md` | **A** partial (parent holds goal/join; children resolve). **B** no. **C** no. **D** yes (spawn-first anti-pattern + Hard stop line for multi-file conflict / post-pick CI). | **N** | Onto HITL runbook already carries **D** + subagent fan-out for conflict/CI. Not the primary home for general **B** or skills-overlay **C** (link project `AGENTS` instead). |
| `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` | **A** partial (parent coordinates; children heavy work). **B** no. **C** no. **D** partial/product (spawn first on CI/regression; may/should-not table; not operator “Hard stop” law). | **N** | Product user-guide source (ships). **D** product summary already present (§ *Token efficiency*). **A/B/C** operator law stays in AGENTS/skills — do not bloat end-user guide with docs-lie / host-skill recon. |
| `/home/hunter/.grok/docs/user-guide/16-subagents.md` | same as in-repo product guide (runtime mirror) | **N** | Install mirror; edit product source in-repo if product text must change. Not pin home for **B/C**. |
| `/home/hunter/.agents/skills/shared/references/subagent-token-strategy.md` | **A** yes-as-coordinator (Hard stop + regressions). **B** no. **C** no. **D** yes (full § Hard stop + micro-flow). | **Y** | Deep guide all skills link. Add **B** (never claim from docs alone; spawn explore/verify before asserting). Optional **A** “HITL UX only” synonym under Hard stop. **C** not primary here (host-only guide). **D** complete. |
| `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md` | **A** yes-as-coordinator. **B** no. **C** partial (skill-change discipline + reconciliations log; **not** “survives product recon”). **D** yes (Hard stop table + author item 8). | **Y** | Skill-author law. Add **B** (docs can lie → verify with tools/subagents before claims in skill text or quality pass). Add **C**: skill *process* changes that matter for grok-oss must also pin on-branch (`AGENTS`/`FORK`); host skill git alone does not survive upstream recon of the product repo. **D** complete. |
| `/home/hunter/.agents/skills/skill-maintenance/SKILL.md` | **A** partial (parent owns inventory/commit; workers quality pass). **B** no. **C** partial (host roots + Required pins; no recon-survival). **D** partial (worker assert Hard stop on orchestrators; Required pins table still regression-centric). | **Y** | Maintenance workflow. Extend Required pins + §4b for **B** and recon-survival **C** (after editing skills, offer/require branch pin when process law changed). Extend harness pointer for **D** Hard stop strings (not only regressions). |
| `/home/hunter/.agents/skills/skill-maintenance/test-required-pins.sh` | **A** no explicit. **B** no. **C** no. **D** partial (checks regressions / join-on-disk; **no** Hard stop / spawn-first / CI-log ban patterns). | **Y** | Red/green compaction harness. After pins land: assert **D** Hard stop wording in global AGENTS + strategy + skill rules; add **B**/**C** patterns when those pins exist. Project AGENTS pattern still “Regressions…” pointer (keep green). |
| `/home/hunter/.agents/skills/implement/SKILL.md` | **A** yes (Hard stop + implementer child). **B** no. **C** no. **D** yes. | **Y** | Primary implement orchestrator (operator overlay). Light **B**: do not claim root cause from docs/README alone — child verifies code/CI. **C** N (not skill-maintenance). **D** complete. |
| `/home/hunter/.agents/skills/pr-babysit/SKILL.md` | **A** yes (fixes only in worktree children + Hard stop). **B** no. **C** no. **D** yes (CI spawn-first). | **Y** | CI babysit magnet. Light **B** on diagnosis claims. **D** complete. Host skill → **C** via branch docs, not this body. |
| `/home/hunter/.agents/skills/plan/SKILL.md` | **A/D** yes (Hard stop + explore). **B** no. **C** no. | **N** | Optional one-line **B** later; not blocking. **D** already ok. |
| `/home/hunter/.agents/skills/review/SKILL.md` | **A/D** yes (findings only from reviewer child + Hard stop). **B** partial practice (child owns findings) without docs-lie slogan. **C** no. | **N** | Optional **B** cross-link; practice already child-owned findings. |
| `/home/hunter/.agents/skills/check-work/SKILL.md` | **A/D** yes (light Hard stop). **B** no. **C** no. | **N** | Optional **B** if verifier claims; low priority. |
| `/home/hunter/.agents/skills/execute-plan/SKILL.md` | **A/D** yes. **B** no. **C** no. | **N** | Hard stop present; optional **B** later. |
| `/home/hunter/.agents/skills/design/SKILL.md` | **A/D** yes. **B** no. **C** no. | **N** | Low risk for CI marathons. |
| `/home/hunter/.agents/skills/upstream-export-import/SKILL.md` | **A/D** yes (onto/conflict Hard stop). **B** no. **C** no. | **N** | Defers to project onto rules; **D** present. **C** belongs in project `AGENTS`/`FORK`. |
| `/home/hunter/.agents/skills/help/SKILL.md` | **A/D** pointers to AGENTS Hard stop + strategy. **B** no. **C** no. | **N** | Optional pointer to **B** when global AGENTS gains it. |
| `/home/hunter/.agents/skills/create-skill/SKILL.md` | **A/B/C/D** no Hard stop author requirement. | **Y** | Author template. Require: if skill can face CI/regression/multi-file → Hard stop sentence (**D**); if skill teaches claims about code → **B**; note process pins that must hit branch docs (**C**). |
| `/home/hunter/.agents/skills/resume-claude/SKILL.md` | explicit do-not-spawn | **N** | Out of scope. |
| `/home/hunter/.agents/skills/TASKS.md` | backlog only | **N** | May track follow-ups; not living law. |
| `/home/hunter/.grok/bundled/skills/{implement,pr-babysit,execute-plan,review}/SKILL.md` | **D** no Hard stop string (lag vs `~/.agents`). | **N\*** | \*Not direct edit targets for durable pins. Bundled = managed cache/remote archive; agents home wins at User tier. Reconcile via `/skill-maintenance` after agents pins; platform defaults need bundle source, not local cache hacks. See `where-skills-come-from-2026-07-24.md`. |
| `/home/hunter/.grok/skills/**` | stale/sparse vs agents | **N** | Prefer agents home; maintenance may rsync — do not pin law only here. |
| `doc/dev/research/skill-*.md`, `where-skills-come-from-2026-07-24.md` | research / prior Hard stop inventory (some rows stale) | **N** | Join artifacts, not process law. Do not treat as pins. |

---

## Must edit — Y rows only (path + edit list)

| path | edit list |
|------|-----------|
| `/home/hunter/.grok/AGENTS.md` | **A** HITL UX coordinator framing (parent = human status/goals; research+impl in children). **B** never-assume / docs-lie + verify via subagents before claims. Optional **C** one-liner → project branch docs for recon survival. Keep **D** as-is. |
| `/home/hunter/Projects/surmount/grok-build/AGENTS.md` | **A** HITL UX reframe on existing coordinator §. **B** never-assume pin. **C** skills = host overlay; process pins that must survive upstream recon live in `AGENTS`/`FORK`/upstream docs. Keep **D**. |
| `/home/hunter/Projects/surmount/grok-build/FORK.md` | **C** short hierarchical note (operator skills host-layer; not xAI recon content; process pins dual-home). Optional one-line pointer to parent-coordinator / Hard stop in `AGENTS`. |
| `/home/hunter/.agents/skills/shared/references/subagent-token-strategy.md` | **B** section or anti-pattern row (docs can lie → child verify before claim). Optional **A** HITL UX synonym under Hard stop. |
| `/home/hunter/.agents/skills/_SKILL_RULES-read-first-pls.md` | **B** author/standing rule. **C** recon-survival dual-pin (host skill + branch `AGENTS`/`FORK`). Recent reconciliations line when done. |
| `/home/hunter/.agents/skills/skill-maintenance/SKILL.md` | Required pins + §4b: assert **B**; after process-law skill edits, require/offer branch pin (**C**). Point harness at **D** Hard stop patterns. |
| `/home/hunter/.agents/skills/skill-maintenance/test-required-pins.sh` | Assert **D** Hard stop / spawn-first / CI-log ban in global AGENTS + strategy (+ skill rules). Add **B**/**C** patterns after those pins land. |
| `/home/hunter/.agents/skills/implement/SKILL.md` | Light **B** under Hard stop / diagnosis: no claim from docs alone. |
| `/home/hunter/.agents/skills/pr-babysit/SKILL.md` | Light **B** on CI root-cause claims. |
| `/home/hunter/.agents/skills/create-skill/SKILL.md` | Author requirements: **D** Hard stop if CI/regression-facing; **B** if teaching codebase claims; **C** note for process law → branch docs. |

---

## Already complete enough (no must-edit for these pins)

| path | notes |
|------|--------|
| Project `AGENTS` + global `AGENTS` + strategy + skill rules + orchestrators (`implement`, `pr-babysit`, `plan`, `review`, `check-work`, `execute-plan`, `design`, `upstream-export-import`) + product `16-subagents` § Token efficiency + `docs/upstream-history` conflict/CI bullets | **D** Hard stop / spawn-first largely pinned (2026-07-24 pin campaign). Gaps are **A** framing, **B**, **C**, harness coverage of **D** strings, and create-skill author template. |
| `RESIDUAL.md` | Wrong home for standing pins. |
| Bundled / `~/.grok/skills` | Not durable pin homes; reconcile after agents. |

---

## Suggested pin order

1. Global + project `AGENTS.md` (**A** + **B** + **C** on project; global **A**+**B**).  
2. `FORK.md` one hierarchical **C** bullet.  
3. Strategy + `_SKILL_RULES` + skill-maintenance + harness.  
4. Light **B** on `implement` / `pr-babysit`; author bar on `create-skill`.  
5. Re-run `test-required-pins.sh` green.

---

## Related research (not law)

- `doc/dev/research/skill-subagent-pin-inventory-2026-07-24.md` — early Hard stop gap map (partially superseded).  
- `doc/dev/research/skill-pin-orchestrators-2026-07-24.md` / `skill-pin-core-strategy-2026-07-24.md` / `skill-pin-user-guide-2026-07-24.md` — Hard stop application log.  
- `doc/dev/research/where-skills-come-from-2026-07-24.md` — host vs branch vs bundled (feeds **C**).  
