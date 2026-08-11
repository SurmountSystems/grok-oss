# Report: process jargon "join" → "reports" (2026-08-03)

## Intent

On-disk handoff artifacts are **reports**, not "joins" / "join notes" / "join
artifacts." Fork-join parallelism may still be named when explaining
hierarchically structured subagent work (agent depth L1 main / L2 subagents /
L3 specialists max). That is different from naming the summary file a "join."

Canonical new path: **`.agents/reports/`**. Legacy **`.agents/joins/`** keeps
historical files; see `.agents/joins/README.md`.

## Files changed

### Dual-pin (recon survival)

| Path | Change |
|------|--------|
| `/home/hunter/.grok/AGENTS.md` | Handoff language → report; path guidance `.agents/reports/`; legacy joins noted; ban "join notes/artifacts" as jargon |
| `/home/hunter/Projects/surmount/grok-build/AGENTS.md` | Same process language; session-board L2 = reports; explicit pin *Reports, not "joins"* |
| `/home/hunter/Projects/surmount/grok-build/FORK.md` | Process HITL wording (goals/spawn/reports); "Report:" research links where process handoff was meant |
| `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md` | Standing process prose (short reports on disk); "Report:" labels; **paths** to existing `.agents/joins/*.md` left as real historical paths |

### Host skills

| Path |
|------|
| `~/.agents/skills/_SKILL_RULES-read-first-pls.md` |
| `~/.agents/skills/shared/references/subagent-token-strategy.md` |
| `~/.agents/skills/shared/references/skill-reconciliations.md` (log line wording) |
| `~/.agents/skills/shared/personas/implementer.md` |
| `~/.agents/skills/shared/personas/reviewer.md` |
| `~/.agents/skills/implement/SKILL.md` |
| `~/.agents/skills/plan/SKILL.md` |
| `~/.agents/skills/review/SKILL.md` |
| `~/.agents/skills/design/SKILL.md` |
| `~/.agents/skills/execute-plan/SKILL.md` |
| `~/.agents/skills/pr-babysit/SKILL.md` |
| `~/.agents/skills/check-work/SKILL.md` |
| `~/.agents/skills/help/SKILL.md` |
| `~/.agents/skills/skill-maintenance/SKILL.md` |
| `~/.agents/skills/git-recon/SKILL.md` (process handoff only; git join-main kept) |
| `~/.agents/skills/git-recon/references/conflict-fanout.md` |
| `~/.agents/skills/upstream-export-import/SKILL.md` (process handoff only) |
| `~/.agents/skills/game-animation-frames/SKILL.md` |
| `~/.agents/skills/game-tilesets/SKILL.md` |
| `~/.agents/skills/game-character-consistency/SKILL.md` |
| `~/.agents/skills/game-asset-core/SKILL.md` |

### Product user-guide + one comment

| Path |
|------|
| `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md` |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` |
| host copies under `~/.grok/docs/user-guide/` (same sections) |
| `crates/codegen/xai-grok-pager/src/app/agent.rs` (session notes comment) |

### New tree guidance

| Path | Role |
|------|------|
| `.agents/reports/` | Canonical directory for new reports |
| `.agents/joins/README.md` | Legacy pointer to reports |
| This file | Implementer report for the terminology change |

## Sample before → after

| Before (jargon) | After |
|-----------------|-------|
| join note / join artifact | report / short report |
| "Join on disk." | "Write a short report on disk." (L2) / "Read the short report on disk." (L1) |
| L2 child joins (session board) | L2 reports |
| `.agents/joins/foo.md` (guidance) | `.agents/reports/foo.md` |
| short on-disk join notes | short on-disk reports |

## Intentionally unchanged

- **Git recon "join main"** / `join-main-into-onto.sh` / `merge -s ours` join
- Code `.join()`, `os.path.join`, string join
- Tile "seam lines at the joins"
- Historical campaign research under `doc/dev/research/*` (not standing D1)
- Historical residual **paths** that still point at real files under
  `.agents/joins/` (files not mass-moved)
- Session compaction logs under `~/.grok/sessions/`
- No product Rust bulk rename of the word "join"

## Residual

1. Full `/skill-maintenance` not run (parent may spawn next if desired).
2. Bundled skills under `~/.grok/bundled/skills` not synced this pass; host
   `~/.agents/skills` is the maintained body. Maintenance can rsync later.
3. Historical notes still under `.agents/joins/`; new work uses
   `.agents/reports/`.
4. Some D3 research docs still say "join on disk" historically; not D1 law.

## Proof

- Grep for process jargon (`join on disk`, `join note`, `join artifact`,
  `child join`, `.agents/joins` as guidance) in host skills + project AGENTS:
  cleared for standing guidance (ban lines that mention the old words on
  purpose remain).
- Git join-main language preserved in project AGENTS and git-recon skill.
