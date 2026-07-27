# Host skills process pin — join note (2026-07-24)

**Mode:** operator host edits only (no product branch AGENTS/FORK in this pass).  
**No git commit** (agents never commit).  
**Sources:**  
`where-skills-come-from-2026-07-24.md`,  
`skills-survive-upstream-recon-2026-07-24.md`,  
`process-pin-targets-2026-07-24.md`.

---

## What landed (A/B/C + D harness)

| Pin | Meaning | Where |
|-----|---------|--------|
| **A** HITL UX parent | Parent = human-in-the-loop coordinator only; research/impl in children | `~/.grok/AGENTS.md` Hard stop; strategy HITL synonym; upstream skill |
| **B** Never assume | Docs can lie; verify with tools/subagents before claims | global AGENTS, strategy, skill rules, implement/pr-babysit light, create-skill bar |
| **C** Dual-pin recon | Host `~/.agents` overlay ≠ product history; process law also on branch | global AGENTS, skill rules, skill-maintenance, upstream skill |
| **D** Hard stop | Already present; harness now asserts strings | `test-required-pins.sh` |

Also: **upstream-export-import** skill rewritten off stale `MODE=overlay` /
`FORCE_BRANCH` → real cherry-pick + `FORCE=1` + mandatory join-main + spawn-first.

---

## Files touched

| Path | Edit |
|------|------|
| `~/.grok/AGENTS.md` | HITL UX under Hard stop; § Never assume; § Skills & process pins (host overlay + dual-pin) |
| `~/.agents/skills/shared/references/subagent-token-strategy.md` | Required pins row B; HITL under Hard stop; § Never assume; anti-pattern + author item 7 |
| `~/.agents/skills/_SKILL_RULES-read-first-pls.md` | Standing 15–16; token-efficiency never-assume row; author 9–10; § dual-pin; reconciliations line |
| `~/.agents/skills/skill-maintenance/SKILL.md` | Quality 4c/4d; Required pins A–D table; product pins ≠ agents↔bundled |
| `~/.agents/skills/skill-maintenance/test-required-pins.sh` | Assert Hard stop, HITL, never-assume, dual-pin (+ prior regression pins) |
| `~/.agents/skills/implement/SKILL.md` | Light never-assume under Hard stop |
| `~/.agents/skills/pr-babysit/SKILL.md` | Light never-assume under Hard stop |
| `~/.agents/skills/create-skill/SKILL.md` | Author bar rows D/B/C |
| `~/.agents/skills/upstream-export-import/SKILL.md` | Kill MODE=overlay; cherry-pick + FORCE=1 + join-main; process pin survival; HITL spawn-first |
| `doc/dev/research/host-skills-process-pin-2026-07-24.md` | This join |

---

## Explicitly not edited this pass

| Path | Why |
|------|-----|
| Project `AGENTS.md` / `FORK.md` | User asked host operator skills; product dual-pin **C** still recommended next (process-pin-targets Y rows) |
| Bundled / `~/.grok/skills` | Not durable pin homes; reconcile via `/skill-maintenance` if desired |
| Product user-guide `16-subagents` | Operator law stays host; product already has D summary |

---

## Harness

```bash
~/.agents/skills/skill-maintenance/test-required-pins.sh
```

Expect: `PASS: Hard stop + never-assume + dual-pin + regression→subagent pins green`

---

## Residual honesty

1. **Product dual-pin incomplete until** project `AGENTS.md` + `FORK.md` get the
   same A/B/C short pins (process-pin-targets Y rows). Host pins survive
   recon for the operator; collaborators still need branch pins.
2. **Import `FORK_PATHS`** still omits `AGENTS.md` / residual / join script —
   P0 from skills-survive research; not host-skill work.
3. **skill-maintenance may commit** in `~/.agents/skills` git (skills repo
   exception). Do not confuse with product-repo commit ban.

---

*End join.*
