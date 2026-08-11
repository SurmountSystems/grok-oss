# Skill maintenance report — 2026-08-03

Full `/skill-maintenance` after joins→reports terminology dual-pin (host + product AGENTS). Context: prior terminology pass already edited orchestrators/personas/strategy; this run inventory + reconcile + quality assert + pins harness fix.

## 1. Rules status

| Item | Status |
|------|--------|
| `_SKILL_RULES-read-first-pls.md` | Present; read first. Dirty from terminology pass (report language). |
| `~/.agents/skills` git | Repo OK (`/home/hunter/.agents/skills`) |
| Symlinks under agents skills | **0** |
| Product git | Untouched (no add/commit) |

## 2. Bundled copies

| Metric | Value |
|--------|-------|
| agents skills | 30 |
| grok-bundled skills | 21 |
| grok-user skills | 6 |
| Missing-in-agents (bundled had SKILL.md) | **0** (nothing copied this run) |
| Only-in-agents | check-work, git-recon, grok-tool-policy, help, plan, skill-maintenance, upstream-export-import, xlsx, zed-settings |

## 3. Reconcile (peer-ahead kept intentional)

| Skill | Multi-root | Decision |
|-------|------------|----------|
| code-review | peer longer (bundled/user ~192 vs agents 98) | **Keep agents** — Surmount Hard stop + spawn_agent + trim |
| check-work | grok-user longer (~287 vs 72) | **Keep agents** — intentional slim + L1/L2 depth |
| docx / pdf / pptx | bundled essay + templates | **Keep agents** — token-efficient shells + local refs (pptx templates not absorbed) |
| imagine | peer longer | **Keep agents** — Zed/Surmount + shorter |
| Most orchestrators / game-* / create-* / resume-* | agents-ahead | Keep agents (prior Surmount quality) |

No blind peer→agents clobber. No dual-write to `~/.grok/skills`.

## 4. Quality pass summary (token / Zed / subagents)

Focus: report-on-disk language (not handoff "join"), L1 Hard stop, agent depth L1/L2/L3. Fork-join only for structural subagent depth. Git `join-main` language kept.

| skill | lines | token | zed | subagents | edited? | notes |
|-------|------:|-------|-----|-----------|---------|-------|
| implement | 1154 | needs-human | ok | ok | no (this run) | mega; report language ok; Hard stop + depth |
| execute-plan | 1414 | needs-human | ok | ok | no | mega; allow_worktree pins still present |
| pr-babysit | 954 | needs-human | ok | ok | no | mega |
| review | 614 | needs-human | ok | ok | no | mega |
| plan | 196 | ok | ok | ok | no | report + Hard stop + depth |
| check-work | 72 | ok | ok | ok | no | slim intentional |
| git-recon | 236 | ok | ok | ok | no | HITL + Spawn first + depth; git join kept |
| skill-maintenance | 307 | ok | ok | ok | **yes** | Required pins table + harness |
| design | 175 | ok | ok | ok | no | prior terminology dirty |
| code-review | 98 | ok | ok | ok | no | Surmount slim |
| help | 63 | ok | ok | light | no | |
| create-skill | 85 | ok | ok | ok | no | |
| create-workflow | 220 | ok | ok | ok | no | |
| office (docx/pdf/pptx) | slim | ok | ok | light | no | intentional vs bundled essays |
| game-* / resume-* / build-with-ai | small | ok | ok | light | no | game-* dirty from prior term pass |
| strategy + personas + rules | — | ok | ok | ok | harness only | 0 bad handoff-join jargon |

**This run edits:** `skill-maintenance/test-required-pins.sh` (Join on disk → `report on disk` + `.agents/reports`); `skill-maintenance/SKILL.md` Required pins row for **Reports**.

**Bad handoff-join scan:** no remaining `join on disk` / `join note` / `.agents/joins` as current guidance (log line only). Remaining `\bjoin\b` = git join-main / Python `join` only.

**needs-human residual (token size, not broken pins):** implement, execute-plan, pr-babysit, review — top cuts would be extract more workflow to `references/` (prior known; no mega-diff this run).

## 5. Required pins harness

```text
PASS: Hard stop + agent-depth L1/L2/L3 + never-assume + dual-pin + never-stage + regression→subagent pins green
```

Prior red: harness still required `Join on disk` after terminology rename → fixed.

## 6. Dirty paths (`~/.agents/skills` only)

Unstaged (not staged; operator did not ask to stage):

```
_SKILL_RULES-read-first-pls.md
check-work/SKILL.md
design/SKILL.md
execute-plan/SKILL.md
game-animation-frames/SKILL.md
game-asset-core/SKILL.md
game-character-consistency/SKILL.md
game-tilesets/SKILL.md
git-recon/SKILL.md
git-recon/references/conflict-fanout.md
help/SKILL.md
implement/SKILL.md
plan/SKILL.md
pr-babysit/SKILL.md
review/SKILL.md
shared/personas/implementer.md
shared/personas/reviewer.md
shared/references/skill-reconciliations.md
shared/references/subagent-token-strategy.md
skill-maintenance/SKILL.md
skill-maintenance/test-required-pins.sh
upstream-export-import/SKILL.md
```

## 7. Handed commit commands (host skills repo only)

```bash
cd ~/.agents/skills
git add -A
git status --short
git commit -S -m "skills: joins→reports terminology + pins harness" \
  -m "Canonical handoff path .agents/reports/. Keep git join-main language.
Update test-required-pins for report on disk. Full skill-maintenance 2026-08-03."
```

Do **not** run those from an agent unless the operator explicitly asks to stage/commit. Never disable GPG. Never touch product-repo index for this work.

## 8. Residual / needs-human

1. **Mega-orchestrator token debt** (implement / execute-plan / pr-babysit / review): optional later extract to `references/`; not blocking.
2. **pptx bundled templates** (many under bundled only): absorb only if operator wants demo templates; agents intentionally lean.
3. **Peer longer office/code-review/imagine/check-work:** keep agents unless operator requests surgical absorb of specific peer content.
4. Product dual-pin for reports terminology already landed (see `.agents/reports/impl-joins-to-reports-terminology-2026-08-03.md`); product git commit is human-only when wanted.

## 9. D2 log

Appended one line to `~/.agents/skills/shared/references/skill-reconciliations.md`.
