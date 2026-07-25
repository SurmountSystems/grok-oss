# Tools self-improve + recon survival (workflows + skills)

**Date:** 2026-07-24  
**Workspace:** `/home/hunter/Projects/surmount/grok-build`  
**Mode:** evidence-backed research (product code + host skills + living process docs).  
**Related:**  
`where-skills-come-from-2026-07-24.md`,  
`skills-survive-upstream-recon-2026-07-24.md`,  
`fork-paths-hardening-2026-07-24.md`,  
`host-skills-process-pin-2026-07-24.md`.

---

## Plain answer

Grok Build has **two self-improving tool layers**:

| Layer | What improves | How it improves | Where durable bodies live |
|-------|---------------|-----------------|---------------------------|
| **Skills** | Slash procedures (`SKILL.md`, scripts, refs) | `/create-skill`, `/skill-maintenance` quality pass, peer absorb from bundled | Host `~/.agents/skills` (operator); optional project `.agents/skills` / `.grok/skills`; platform cache `~/.grok/bundled/skills` |
| **Workflows** | Deterministic multi-agent Rhai scripts | `/create-workflow` author → `validate_only` smoke → save → real run; iterate via editable `script_path` | Project `.grok/workflows/<name>.rhai` or user `~/.grok/workflows/<name>.rhai` |

**Recon survival is not automatic for process law.** Chat dies at compaction. Host skill trees sit **outside** the product git tree (safe from import/onto). Product-facing law must also land on the **branch** under paths protected by import `FORK_PATHS` (and on the onto tip before join). That is the **dual-pin**.

```text
  Self-improve loop                    Survive recon
  ─────────────────                    ─────────────
  create / edit skill or workflow  →   pin process law twice when product-facing
  smoke / quality / peer absorb    →   host overlay + branch docs / FORK_PATHS
  residual: open only in RESIDUAL  →   finished truth → FORK / AGENTS / upstream docs
  assert harnesses                 →   assert-process-pins + skill-maintenance pins
```

---

## 1. Dual-pin (branch process + host skills)

### Rule (authoritative wording)

Process law that must survive **product** upstream recon (import / put-history / join) **or** collaborators:

1. **Branch** — project `AGENTS.md`, `FORK.md`, `docs/upstream-*`, and (when research must ride import) `doc/dev/**`.
2. **Host** — `~/.agents/skills/**` (operator skill bodies) and `~/.grok/AGENTS.md` (cross-repo process).

Host skill git alone does **not** ride product history. Branch docs alone do **not** update this machine’s effective slash skills. **Edit both** when both matter (same turn).

| Layer | Survives import/onto? | Role |
|-------|----------------------|------|
| `~/.agents/skills/**` | Yes (outside tree) | Operator skill bodies; this machine’s slash skills |
| `~/.grok/AGENTS.md` | Yes (host) | Cross-repo process law |
| Product `AGENTS.md` / `FORK.md` / `docs/upstream-*` | Only if on tip **and** import-protected (`FORK_PATHS`) | Collaborators + recon land path |
| `~/.grok/bundled/skills/**` | Survives git; **not** durable for edits | Network bundle sync can overwrite |
| Chat / session only | No | Compaction + recon both erase |

Evidence: product `AGENTS.md` § Skills + Survive recon; `FORK.md` § Skills (multi-source);  
`~/.agents/skills/_SKILL_RULES-read-first-pls.md` standing rule 16 + § dual-pin;  
`skill-maintenance` Required pins **C**; global `~/.grok/AGENTS.md` § Skills & process pins.

### What dual-pin is *not*

- **Not** “copy every skill into the repo.” Most orchestrator bodies stay host.
- **Not** agents↔bundled reconcile alone — that is host-only; it does **not** restore product `AGENTS.md` after import.
- **Not** `skill-maintenance` green ⇒ recon-safe product tree. Run `./scripts/assert-process-pins.sh` separately.

### When a skill/workflow change requires dual-pin

| Change type | Host only | Branch pin also |
|-------------|-----------|-----------------|
| Token trim / Zed tool names / routine skill steps | Yes | No |
| Hard stop / HITL parent / never-commit / never-assume | Yes | Yes (AGENTS + often FORK) |
| Import/onto mechanics, `FORK_PATHS`, join rules | Yes (upstream skill) | Yes (`docs/upstream-*`, FORK, scripts) |
| Product-facing user-guide policy | Optional | Yes (pager user-guide; conflict-prone) |
| Personal workflow (this machine only) | Save under `~/.grok/workflows` | Optional research note only |
| Team-shared workflow (repo) | Optional mirror | Yes — commit under `.grok/workflows/` **and** extend `FORK_PATHS` if it must survive import |

---

## 2. Skills — self-improvement loop

### Sources (product multi-source load)

Product owns discovery/precedence/bundle sync (on this branch). Bodies load from:

```text
Local/Repo:  .agents/skills → .grok/skills  (cwd → git root)
User:        ~/.agents/skills → ~/.grok/skills
Config paths / Server inject
Bundled:     ~/.grok/bundled/skills   ← network cache (not pin home)
Plugin
```

On this machine: **no** committed project skill packs under grok-build; effective orchestrators live under **`~/.agents/skills`** and shadow same-named grok/bundled packs.

Detail: `doc/dev/research/where-skills-come-from-2026-07-24.md`.

### Improvement mechanisms

| Mechanism | Skill | What it does |
|-----------|-------|--------------|
| Create | `/create-skill` | Scaffold lean `SKILL.md` under `~/.agents/skills` (default) or project roots; quality bar includes Hard stop / never-assume / dual-pin |
| Maintain | `/skill-maintenance` | Ensure git repo; copy missing **bundled** skills as real dirs (never symlinks); inventory agents vs grok-user vs bundled; surgical peer absorb; §4b quality pass (token / Zed / sub-agents / dual-pin offer); commit **skills repo only** |
| Rules | `_SKILL_RULES-read-first-pls.md` | Standing author law; dual-pin §; token targets |
| Harness | `skill-maintenance/test-required-pins.sh` | Red/green strings for Hard stop, never-assume, dual-pin, regression→subagent |
| Product docs | User-guide `08-skills.md`, `16-subagents.md` | User-facing locations / subagent policy (branch-owned machinery + docs) |

**Sync direction (host):** improvements in Grok/bundled → **offer absorb into agents**. Agents-only skills stay agents-only unless dual-written by request. Never write into bundled as the pin home.

**skill-maintenance may commit** in `~/.agents/skills` git (skills-repo exception). That is **not** product-repo commit policy (product: human-only signed `git commit -S`).

### Self-improvement pattern for skills

1. Detect peer-ahead / missing-in-agents / process pin rot.  
2. `/skill-maintenance` (or surgical edit + lite quality pass).  
3. If process law changed → **same-turn dual-pin** on branch.  
4. Run host pin harness + (for product) `assert-process-pins`.  
5. Finished process truth → FORK/AGENTS; open only → `RESIDUAL.md`.

---

## 3. Workflows — self-improvement loop

### What they are

Workflows are **deterministic Rhai** scripts orchestrating subagents via host API: `agent()`, `parallel()`, `phase()`, `complete()`, `pause` / `await_user`, scratch files, budget. Run by the `workflow` tool; UI in `/workflows`.

Authoring skill: **`create-workflow`** (bundled: `~/.grok/bundled/skills/create-workflow/SKILL.md`). Not an agents-home override on this machine at research time — load path still finds it via bundled unless agents gains a copy through maintenance.

### Where scripts live

| Scope | Path | Share / recon |
|-------|------|----------------|
| **Project (default in a git repo)** | `<repo>/.grok/workflows/<name>.rhai` | Teammates via git; **not** currently in import `FORK_PATHS` → **dropped on import** if present and absent from xAI tree |
| **User** | `~/.grok/workflows/<name>.rhai` | All projects; **outside** product tree → **survives** import/onto |
| **Session projection** | Per-run editable `script_path` under session | Ephemeral / debug; not team pin |
| **Built-ins** | Product-registered names | Win over project/user name collisions |

Product discovery (user-guide + tool schema): project `.grok/workflows/`; user `~/.grok/workflows/`; `meta.name` keys discovery; built-in > project > user.

**This workspace today:** no project `.grok/` dir; no `~/.grok/workflows` dir. Workflow self-improvement is available via create-workflow + product machinery, but no saved Surmount workflow packs are on disk yet.

### Author → improve loop (`/create-workflow`)

1. Gather intent (fan-out, verify, final artifact, agent budget comfort).  
2. Pick scope: **project** `.grok/workflows` vs **user** `~/.grok/workflows`.  
3. Author: pure-literal `let meta = #{…}` → schemas → phases.  
4. **Smoke-check:** `workflow` tool with `validate_only: true` (metadata + compile + one canned path — not full branch coverage).  
5. Save to chosen path → invocable as `/<name>` or `/workflow <name>`.  
6. Optional real run; watch `/workflows`.  
7. Iterate: edit projection or saved file → re-smoke → new run (resume uses immutable original script).

**Patterns that improve quality over time:** adversarial verify panels; fail-closed evidence gates; plain-Rhai filters of agent output (prompts do not enforce scope); journal of committed host results for debugging (not exactly-once external effects).

### Workflow scripts in repo vs host (survival)

| Choice | Pros | Cons / recon risk |
|--------|------|-------------------|
| **Repo** `.grok/workflows/*.rhai` | Shared, reviewable, PR’d with product | Import **deletes** unless added to `FORK_PATHS` (or re-picked on onto). Join cannot backfill missing tip files. |
| **Host** `~/.grok/workflows/*.rhai` | Immune to import/onto; personal automation | Not shared with collaborators; not on PR branch |
| **Both** | Dual-pin style: team copy on branch + operator tweaks on host | Name collisions: project wins over user; keep names unique |

**Recommendation for Surmount recon workflows (import/onto/CI babysit):**

- Prefer **host** `~/.grok/workflows/` for operator-only pipelines **or**  
- Prefer **repo** + **extend `FORK_PATHS`** (and `assert-process-pins` REQUIRED list) if the workflow is process infrastructure that must survive import — same discipline as scripts under `scripts/`.  
- Do **not** assume “it’s a workflow so recon keeps it.” Only `FORK_PATHS` + cherry-picks keep Surmount-only trees.

**Note:** GitHub Actions under `.github/workflows/` are **CI YAML**, not Rhai Grok workflows. They are already in `FORK_PATHS` (`ci.yml`, `upstream-export.yml`). Do not confuse the two.

---

## 4. Residual documentation pattern

Living residual is **not** a dump of finished work or a second chat log.

### Pattern (product)

| Artifact | Holds | Does not hold |
|----------|-------|---------------|
| **`RESIDUAL.md`** | **Open** human-intent / unfinished honesty only | Finished process (move to FORK/AGENTS); novels |
| **`FORK.md`** | Hierarchical “what this fork is / what recon keeps” | Ephemeral research tables |
| **`AGENTS.md`** | Standing agent process law for this repo | Full skill bodies |
| **`docs/upstream-history.md`** (+ import/onto logs) | Canonical recon law, HITL runbook, checklists | Chat archaeology |
| **`doc/dev/research/*`** | Dated evidence notes (this file’s class) | “Primary residual ranking” (that’s RESIDUAL + AGENTS) |
| **Chat** | Short join of disk | Authority after compaction |

### Rules that make residual survive recon

1. **Same-turn disk pin** for ranking, demotions, stop rules, process corrections.  
2. **When finished:** lasting truth → FORK / AGENTS / upstream docs; remove or demote from `RESIDUAL.md` (*Not residual* section is fine for pointers).  
3. **Research notes** under `doc/dev/research/` ride import via **`doc/dev` in `FORK_PATHS`** — good for survival of analysis; still not a substitute for AGENTS/FORK process pins.  
4. **Do not invent residual** from research alone — fork research may write notes; primary residual ranking stays human-intent open items.  
5. **Assert after recon:** `./scripts/assert-process-pins.sh` — does not prove residual *content* quality, only that pin **paths** exist.

### Self-improve ↔ residual handoff

```text
  Tool learning (skill/workflow fix)
       │
       ├─ open / still fuzzy ──────────► RESIDUAL.md (short)
       │
       └─ settled law ─────────────────► AGENTS / FORK / docs/upstream-*
                                         (+ host skill dual-pin if operator)
                                         research note under doc/dev/research/ if evidence-heavy
```

---

## 5. What must land in `FORK_PATHS`

### Mechanism (import)

1. `read-tree -u --reset <xAI tree>` — whole tree becomes export.  
2. Restore only **`FORK_PATHS`** from `BASE_REF` (`origin/main`).  
3. Post-restore **`assert-process-pins`** fail-closed.  
4. Paths **not** listed and **not** in xAI tree → **deleted**.

Authority: `scripts/import-upstream-export.sh` (`FORK_PATHS` array).  
Onto/join: cherry-pick stack + `merge -s ours` — join **does not** fold missing files from `main`.

### Current `FORK_PATHS` (2026-07-24, from import script)

```text
# product identity / packaging
FORK.md
CONTRIBUTING.md
SECURITY.md
README.md
justfile
flake.nix
flake.lock
packaging

# process pins
AGENTS.md
RESIDUAL.md

# living recon docs + research roots
docs/upstream-history.md
docs/upstream-import-log.md
docs/upstream-onto-log.md
docs/git-workflow.md
docs/dev
doc/dev

# recon + hermeticity scripts
scripts/detect-upstream-export.sh
scripts/import-upstream-export.sh
scripts/sync-upstream.sh
scripts/put-history-on-xai.sh
scripts/replay-onto-upstream.sh
scripts/join-main-into-onto.sh
scripts/with-ci-hermetic-path.sh
scripts/assert-process-pins.sh

# GHA workflows + Surmount-only crates
.github/workflows/upstream-export.yml
.github/workflows/ci.yml
crates/codegen/grok-rate-limit
```

### Must-add candidates (when you create them)

| Path | When to add to `FORK_PATHS` + assert list |
|------|------------------------------------------|
| `.grok/workflows/` or specific `*.rhai` | Team-shared Rhai workflows that are process infrastructure |
| `.agents/skills/` / `.grok/skills/` | Committed project skill packs that must survive import |
| New Surmount-only `scripts/*` | Recon/CI hermeticity helpers |
| New process docs under `docs/` not covered by existing dirs | If not already under `docs/dev` / listed files |
| New Surmount-only crates | Like `grok-rate-limit` |

**Still not restored by design:** seams inside `xai-grok-*` (OpenRouter, branding, sampler) — re-apply via diff on onto/import review. Whole user-guide tree is **not** wholesale-pinned (would freeze upstream doc evolution); product sections re-apply on conflict.

### Assert list stays in sync

When extending `FORK_PATHS`, also update:

- `scripts/assert-process-pins.sh` (`REQUIRED_FILES` / `REQUIRED_DIRS`)  
- Import checklist in `docs/upstream-history.md`  
- Short note in `FORK.md` § recon if the class of pin is new  

---

## 6. How the whole system improves itself (end-to-end)

| Feedback | Capture | Durable home |
|----------|---------|--------------|
| Skill peer newer in bundled | `/skill-maintenance` absorb offer | `~/.agents/skills` (+ dual-pin if law) |
| Orchestrator forgot Hard stop | Quality pass 4b + pin harness red | Host skill + `~/.grok/AGENTS.md` + project AGENTS |
| Import dropped a process file | `assert-process-pins` fail | Extend `FORK_PATHS` + re-restore |
| Onto tip missing pin before join | Assert on tip; re-apply from main / pick | Cherry-pick / checkout path onto tip |
| Workflow prompt too terse / bad schema | `validate_only` + real run + journal | Edit saved `.rhai`; re-save |
| Docs lied about MODE/overlay | Never-assume; fix skill + branch docs | Dual-pin same turn |
| Open human-intent unfinished | Residual honesty | `RESIDUAL.md` until closed |

**Anti-patterns**

- Chat-only “we should pin that”  
- Host-only process law for collaborators  
- Committing skill work into the product repo by accident  
- Treating green skill-maintenance as green product recon  
- Saving team workflows only under `~/.grok/workflows` and expecting PRs to carry them  
- Saving team workflows only under `.grok/workflows` without `FORK_PATHS` and expecting import to keep them  
- Parent-marathon skill/workflow research instead of spawn-first (HITL parent)

---

## 7. Survival checklist

### After any skill or process-law change

- [ ] Host skill / rules updated under `~/.agents/skills` (if operator-facing)  
- [ ] If process law: **branch** pin same turn (`AGENTS.md` / `FORK.md` / `docs/upstream-*`)  
- [ ] `~/.agents/skills/skill-maintenance/test-required-pins.sh` green (host pins)  
- [ ] No write to `~/.grok/bundled` as the pin home  
- [ ] Open leftovers only in `RESIDUAL.md`; finished → FORK/AGENTS  

### After any new team workflow (Rhai)

- [ ] Saved under `.grok/workflows/<name>.rhai` (team) and/or `~/.grok/workflows/` (operator)  
- [ ] `validate_only` smoke passed; real run offered  
- [ ] If team + must survive import: path listed in **`FORK_PATHS`** + assert script  
- [ ] Name unique vs built-ins; `meta.name` matches filename convention  

### After import

- [ ] `./scripts/assert-process-pins.sh` (or import’s post-restore assert)  
- [ ] Review `FORK_PATHS` only if assert failed or new pin class needed  
- [ ] Product seams in `xai-grok-*` re-applied via `git diff $BASE_REF`  
- [ ] Host skills untouched (spot-check only if bundle sync also ran)  

### After put-history / before join

- [ ] `./scripts/assert-process-pins.sh HEAD` (or onto tip)  
- [ ] Missing pins re-applied **before** `merge -s ours` (join cannot backfill)  
- [ ] Onto log / upstream-history checklist updated when process changed  

### Anytime (operator)

- [ ] `/skill-maintenance` when peers ahead or pins rot  
- [ ] Never assume — verify load paths / scripts before claiming survival  
- [ ] Parent = HITL only: multi-file recon diagnosis → spawn, join on disk  

---

## 8. Quick reference paths

| Path | Role |
|------|------|
| `scripts/import-upstream-export.sh` | `FORK_PATHS` authority |
| `scripts/assert-process-pins.sh` | Presence assert |
| `AGENTS.md` / `FORK.md` / `RESIDUAL.md` | Branch process + residual |
| `docs/upstream-history.md` | Recon law + import checklist |
| `doc/dev/` | Research that rides import |
| `~/.agents/skills/` | Operator skill home + maintenance |
| `~/.agents/skills/skill-maintenance/` | Reconcile + pin harness |
| `~/.grok/bundled/skills/create-workflow/` | Workflow authoring skill |
| `~/.grok/bundled/skills/` | Platform cache (not pin home) |
| `<repo>/.grok/workflows/*.rhai` | Team Rhai workflows (add to `FORK_PATHS` if import-critical) |
| `~/.grok/workflows/*.rhai` | User Rhai workflows (recon-safe, not shared) |

---

*End research note.*
