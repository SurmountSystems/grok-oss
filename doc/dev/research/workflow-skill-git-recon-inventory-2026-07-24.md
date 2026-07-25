# Workflow / skill inventory — git recon, merge, cherry-pick, commit friction

**Date:** 2026-07-24  
**Mode:** research inventory (no product code change).  
**Workspace:** `/home/hunter/Projects/surmount/grok-build`  
**Also scanned:** host skills `~/.agents/skills/`, global hooks `~/.git-hooks`,  
`~/.grok` deny + PreToolUse, bundled `create-workflow` skill.

**Goal:** ground a future **workflow skill** (or Grok Build Rhai workflow + skill
wrapper) that automates merge / cherry-pick / onto struggles without violating
signing or agent-commit policy.

---

## Plain answer

| Layer | What exists today | Friction |
|-------|-------------------|----------|
| **Repo scripts** | Detect, import, put-history (real cherry-pick), join (`-s ours`), assert pins, sync | Solid mechanics; stop on conflict; human must sign continues / join |
| **Docs** | `docs/upstream-history.md` HITL runbook, onto-log, git-workflow, FORK/AGENTS | Compaction-safe if re-read; long; Live stack drifts |
| **Host skill** | `upstream-export-import` — direction table, hard rules, spawn-first | Orchestration prose, not an executable state machine |
| **PR skill** | `pr-babysit` — merge-not-rebase, stage + hand commit | Some older body lines still show bare `git commit` examples (policy override at top) |
| **GPG guards** | global pre/post-commit, PreToolUse, config.toml deny | Agents cannot create/bypass unsigned tips; good |
| **Rhai workflows** | Product supports them (`create-workflow`); **no** repo/user git-recon `.rhai` yet | Opportunity: fan-out conflict resolve + gates; cannot own GPG |
| **Signing** | Every new object under `commit.gpgsign=true` wants a signature | Upstream tip commits unsigned OK as parents; **our** stack commits + join need sign |

**Signing boundary (recommended, matches current policy):**

1. **Agents never run `git commit` / never unlock GPG / never bypass.**  
2. Agents **may** resolve, stage, run non-commit scripts, spawn conflict children.  
3. **Human signs on a real TTY:** every `git cherry-pick --continue`, import commit, join merge, feature merge, and any amend.  
4. **“Only the final tip needs signing” is false under current machine guards** if intermediate picks create objects with `commit.gpgsign=true` — each successful pick is a commit; post-commit soft-resets unsigned HEAD. If GitHub later **squash-merges** a PR, only the land commit’s signature may matter for *main*, but the **onto branch history and local stack still require signed intermediates** on this machine.  
5. Upstream xAI commits stay unsigned; they are **parents**, not Surmount-authored tips we create.

---

## 1. Existing tooling map

### 1.1 Repo scripts (`/home/hunter/Projects/surmount/grok-build/scripts/`)

| Script | Role | Commits? | Notes |
|--------|------|----------|-------|
| `detect-upstream-export.sh` | Fetch xAI tip; compare tree to import log | No | Exit 0 up-to-date, 2 new export |
| `import-upstream-export.sh` | Their tree → `import/*` + `FORK_PATHS` restore | **Yes** (`git commit -m`) | Suggests gpgsign=false on fail (policy-hostile string); human amend `-S` if needed |
| `put-history-on-xai.sh` | Real `git cherry-pick -x` onto `onto-xai/<12hex>` | Via cherry-pick | Safe no-op if healthy stack; `FORCE=1` rebuild; `CONTINUE=1` resume; exit 2 on conflict |
| `replay-onto-upstream.sh` | Alias → put-history | Same | Compat only |
| `join-main-into-onto.sh` | `merge -s ours` main into onto | Default **no** (`--no-commit`); `DO_COMMIT=1` tries `-S` | Tree identity check; stages for human |
| `sync-upstream.sh` | Detect + print directions; optional `PUT_ON_XAI=1` / `IMPORT_NOW=1` | Delegates | Orchestrator shell, thin |
| `assert-process-pins.sh` | Presence of AGENTS/FORK/scripts/docs | No | Post-import / post-onto gate |
| `with-ci-hermetic-path.sh` | CI PATH scrub | No | Quality, not recon |

### 1.2 Just recipes

```text
just upstream-detect
just upstream-import …
just upstream-put-history …
just upstream-join-main …
just upstream-assert-process-pins …
just upstream-sync …
```

### 1.3 Living docs (branch process law)

| Path | Owns |
|------|------|
| `docs/upstream-history.md` | Directions, HITL runbook, conflict rules, Live stack, subagent fan-out |
| `docs/upstream-onto-log.md` | Onto stack history rows + short checklist |
| `docs/upstream-import-log.md` | Import seed + completed rows |
| `docs/git-workflow.md` | Open PR: **merge base in, never rebase/force-push**; agent hand-commit |
| `AGENTS.md` / `FORK.md` / `RESIDUAL.md` | Hard stop parent, recon survival, residual honesty |
| `doc/dev/research/skills-survive-upstream-recon-2026-07-24.md` | Skills × recon survival research |
| `doc/dev/research/fork-paths-hardening-2026-07-24.md` | `FORK_PATHS` + assert |

### 1.4 Host skills (operator overlay — **not** product git history)

| Skill | Path | Recon-relevant? |
|-------|------|-----------------|
| **upstream-export-import** | `~/.agents/skills/upstream-export-import/SKILL.md` | **Primary** — A import / B put-history / join / anti-patterns / spawn-first |
| **pr-babysit** | `~/.agents/skills/pr-babysit/SKILL.md` | Merge conflicts on open PRs; stage + hand `git commit -S` |
| **implement** | `~/.agents/skills/implement/SKILL.md` | Post-pick CI / residual; not onto scripts |
| **skill-maintenance** | `~/.agents/skills/skill-maintenance/SKILL.md` | Dual-pin process; not git labor |
| **create-workflow** (bundled) | `~/.grok/bundled/skills/create-workflow/SKILL.md` | How to author Rhai workflows (`agent`/`parallel`/`await_user`) |

No project `.grok/workflows/*.rhai` and no `~/.grok/workflows/` yet for git recon.

### 1.5 Signing enforcement stack (machine)

| Layer | Location | Behavior |
|-------|----------|----------|
| Permission deny | `~/.grok/config.toml` | Blocks `commit.gpgsign=false`, `--no-gpg-sign`, fake gpg, config flips |
| PreToolUse | `~/.grok/hooks/block-unsigned-git-commit.*` | Same strings before shell runs |
| Global pre-commit | `~/.git-hooks/pre-commit` | Refuse if gpgsign off / bypass on cmdline |
| Global post-commit | `~/.git-hooks/post-commit` | Soft-reset unsigned HEAD (backstop) |
| Policy prose | `~/.grok/AGENTS.md`, project `AGENTS.md` | Agents never `git commit`; hand `-S` for TTY |
| Regression | `~/.git-hooks/test-unsigned-guard.sh` | Must stay green |

Escape **humans only:** `ALLOW_UNSIGNED_COMMIT=1 git commit …` (hooks honor; agents must not use).

---

## 2. End-to-end flows (what a skill would drive)

```text
xai-org/main (unsigned orphan exports; pull-only)
      │
      ├─ detect ──► issue/log notice
      │
      ├─ IMPORT (content absorb)
      │     clean tree → import-upstream-export.sh
      │     → re-apply product seams (OpenRouter, branding, rate-limit, …)
      │     → just check → human signed commit if needed → PR → main
      │
      └─ PUT-HISTORY (product on their tip)   ← usual “histories broken” path
            put-history-on-xai.sh
            loop: conflict? → resolve (subagents) → human cherry-pick --continue
                 → CONTINUE=1 put-history …
            join-main-into-onto.sh  (--no-commit)
            → human git commit -S (join)
            → just check → push → PR base=main → close export issues
            → append upstream-onto-log.md
```

**Open feature PR catch-up (orthogonal):**  
`git merge origin/main` → resolve → stage → human `git commit -S` → normal push  
(see `docs/git-workflow.md`; never rebase published PR).

---

## 3. Pain points (observed + structural)

### 3.1 Signing / TTY

- Global `commit.gpgsign=true` + hooks: **every** new commit object needs a
  real GPG/TTY path. Agents correctly stop; humans get interrupt storms mid
  multi-pick stack.
- `put-history` runs `git cherry-pick -x` in a loop — clean picks auto-commit
  (and try to sign) **in-script**. If GPG agent locked, stack dies mid-flight
  even with no conflicts.
- `join-main-into-onto.sh` is well designed: default stages for human (`DO_COMMIT=1` optional).
- `import-upstream-export.sh` still **runs** `git commit` itself and error text
  mentions `commit.gpgsign=false` — conflicts with standing policy and agent
  deny lists (human-only awkwardness).
- Policy tension in user hope “maybe only final staged commit needs signing”:
  **local guards require every tip object signed**. Squash-on-merge at GitHub
  is a **land** concern, not a local stack concern.

### 3.2 Conflict labor

- Mega picks (#4, #12 family) touch shell + pager + sampler + docs at once.
- Rules are good (HEAD tip APIs / product seams / union imports) but **manual**
  and easy for a parent to marathon (forbidden; still happens under stress).
- Mid-stack: `scripts/put-history-on-xai.sh` **absent** on bare xAI tip until a
  product pick lands — needs `/tmp` script + `ROOT` patch (documented, still
  friction).
- `CONTINUE=1` skip logic depends on `cherry picked from commit` trailers;
  messy aborts/amends can confuse “remaining” list.

### 3.3 Process / compaction

- Live stack table in `upstream-history.md` / AGENTS must be re-read after
  compaction; chat memory invents `MODE=overlay` (killed; skill/docs dual-pin
  still needed).
- Join does **not** backfill missing process files from main — tip tree is
  sacred (`-s ours`). Missing pins on tip stay missing until re-applied.
- Dual-pin: host skill survives import/onto; branch docs must too (`FORK_PATHS`
  + assert). A **new skill body** on host alone does not teach collaborators
  or survive operator machine loss.

### 3.4 Skill vs automation gaps

- `upstream-export-import` is excellent **checklist prose**, not a state machine
  with “next command + UU file partition + hand-off blob”.
- No single “where am I?” command that prints: CHERRY_PICK_HEAD / MERGE_HEAD /
  onto tip / unmerged count / next human command.
- `pr-babysit` and upstream skill share commit policy but not conflict-rule
  tables (fork seams vs feature-vs-main).
- Rhai workflows can `await_user` for signed continues — **unused** for recon.

### 3.5 Anti-patterns already documented (must not re-break)

- Agent `git commit` / GPG bypass  
- Parent solo multi-file conflict / CI diagnosis  
- `FORCE=1` rebuild mid healthy stack / `cherry-pick --abort` casually  
- Blind `--ours`/`--theirs` whole tree  
- Invent MODE=overlay / commit-tree  
- Skip join before PR to Surmount main  
- Reset main to onto tip  

---

## 4. What a **skill** should own vs a **Rhai workflow**

### 4.1 Skill (markdown SOP — host + dual-pin branch pointer)

Best for: policy, when-to-use, human gates, hand-command templates, spawn
rules, survival table.

| Owns | Does not own |
|------|----------------|
| Direction choice (import vs put-history) | Running GPG |
| Hard rules + anti-patterns | Silent auto-commit |
| Conflict preference table (pointer to upstream-history) | Bulk replace resolve |
| Subagent fan-out recipe (~2–3 disjoint scopes) | Nested spawn fantasies |
| Exact “hand to human” command blocks | Push without ask |
| Compaction recovery (re-read Live stack, assert pins) | Replacing living docs |
| Dual-pin reminder (branch AGENTS/FORK/upstream docs) | Host-only process law |

**Suggested skill name:** e.g. `git-recon` / `onto-stack` / extend
`upstream-export-import` with a **Commit & continue** section + status probe —
prefer **extend** over a second competing skill unless user wants `/git-recon`
as thin entry that delegates.

### 4.2 Grok Build workflow (Rhai — project `.grok/workflows/` or user)

Best for: multi-phase orchestration with `parallel()` conflict children,
structured status, `await_user` at every sign gate, scratch reports.

| Owns | Does not own |
|------|----------------|
| Phase rail: detect → stack → resolve loop → join stage → verify → PR prep | Creating signed objects |
| Spawning read-write agents with self-contained conflict prompts | Guaranteeing correct seam choice without review |
| Parsing `git status` / UU lists into disjoint buckets (script-side) | Force-push / rewrite main |
| `await_user("user", "Run: git cherry-pick --continue …")` | Unlocking GPG in agent sandbox |
| Writing scratch join notes; `complete(#{…})` summary | Surviving as process law (workflow file may be in `.grok/` — **not** in `FORK_PATHS` today) |

**Capability modes:** conflict agents need `read-write` (or `all` if they run
git stage); status probes `read-only` / execute for git porcelain.

**Survival:** if the workflow is product-shared, put it under a path that
import restores **or** document “host-only workflow” + dual-pin the SOP in
branch docs. Today `.grok/workflows` is **not** in `FORK_PATHS`.

### 4.3 Script improvements (product — optional follow-ups, not this note’s job)

| Idea | Why |
|------|-----|
| `scripts/recon-status.sh` | One-shot CHERRY_PICK/MERGE/onto/unmerged/next cmd |
| import: default `--no-commit` like join | Align with agent-never-commit |
| put-history: optional `STOP_BEFORE_COMMIT=1` / env to pause before each pick sign | Reduce mid-loop GPG failures (design carefully) |
| Remove gpgsign=false suggestion from import error text | Policy hygiene |
| Ensure join + put-history always on early FORK_PATHS / first product picks | Mid-stack script missing |

---

## 5. Signing boundary — decision table

| Action | Who | Signed? |
|--------|-----|---------|
| Fetch / detect / assert pins | Agent or human | N/A |
| Resolve conflict files + `git add` | Agent OK | N/A |
| `git cherry-pick` clean auto-commit inside put-history | Human-run script preferred if GPG locked for agents | **Yes** each pick |
| `git cherry-pick --continue` | **Human only** (policy) | **Yes** |
| `join-main-into-onto.sh` (default) | Agent OK | Stages only |
| Join `git commit -S` | **Human only** | **Yes** |
| Import commit | Script today may commit; policy prefers human `-S` | **Yes** |
| Feature `merge origin/main` conclusion | **Human only** | **Yes** |
| `git push` | Human unless explicitly requested | N/A |
| xAI parent commits on stack | Upstream | Often unsigned — OK as parents |
| Bypass GPG | **Never** (agent); human escape only with `ALLOW_UNSIGNED_COMMIT=1` | Forbidden default |

**Recommendation for skill/workflow copy:**  
> Agents stage and stop. Human signs every continue and the join tip on a real
> TTY. Do not invent “unsigned intermediate stack, sign only land” under current
> hooks — it will soft-reset or refuse.

---

## 6. Survival under onto / import

| Artifact | Import | Put-history | Join |
|----------|--------|-------------|------|
| Host skill `~/.agents/skills/**` | Unaffected | Unaffected | Unaffected |
| `~/.grok/AGENTS.md`, hooks | Unaffected | Unaffected | Unaffected |
| Branch `AGENTS.md`, `FORK.md`, `docs/upstream-*`, scripts | Restored via `FORK_PATHS` + assert | Reappear when picks include them | Tree frozen — no backfill from main |
| Project `.grok/workflows/*.rhai` (if added) | **Not** in `FORK_PATHS` → **drop** on import unless added | Only if cherry-picked | Keeps tip only |
| Research under `doc/dev/research/**` | In `FORK_PATHS` as `doc/dev` | Via stack | Tip only |

**Dual-pin law for any new git-recon skill:**

1. Host skill body (operator).  
2. Branch pointer: short section in `docs/upstream-history.md` and/or
   `AGENTS.md` § onto + link to this research.  
3. If shipping a Rhai workflow for the team: add path to `FORK_PATHS` +
   `assert-process-pins.sh` **or** keep workflow host-only and dual-pin the SOP.

---

## 7. Six capability bullets (new skill / workflow target)

1. **Status probe** — Detect CHERRY_PICK_HEAD / sequencer / MERGE_HEAD /
   onto branch / unmerged paths / whether main is ancestor; emit the **single
   next** human or agent action (no invented modes).

2. **Direction router** — Import vs put-history vs PR-merge-main vs join-only;
   call the right script (`detect` / `put-history` / `join` / `assert`) with
   safe defaults (no `FORCE=1` unless asked).

3. **Conflict fan-out** — Partition UU files into ≤3 disjoint scopes; spawn
   self-contained resolve agents (tip APIs vs product seams vs union);
   join on disk; parent only checks empty UU + no markers.

4. **Human sign gate** — After resolve/stage (or join `--no-commit`), **stop**
   and paste exact `git cherry-pick --continue` / `git commit -S …` lines;
   never agent-commit; never suggest gpgsign bypass.

5. **Stack resume** — Drive `CONTINUE=1` put-history after each signed
   continue; refuse rebuild while mid-pick; refuse casual abort; update or
   point at Live stack / onto-log fields.

6. **Land checklist** — Join staged → human sign → tree/ancestor asserts →
   `just check` → push (if asked) → PR base=main → process-pin assert → log
   append hints; dual-pin survival reminder for any process correction.

---

## 8. Suggested shape (implementation later — not done here)

```text
Host skill:  ~/.agents/skills/git-recon/SKILL.md   (or extend upstream-export-import)
  when-to-use: onto, cherry-pick continue, join, import, merge main into PR
  sections: status, directions, conflict spawn, hand-sign templates, survival

Optional Rhai: .grok/workflows/git-recon.rhai  (project) or ~/.grok/workflows/
  phases: Status → Stack/Resolve loop → Join stage → Verify → Report
  await_user at every commit boundary
  parallel() only for disjoint conflict buckets

Product scripts (optional PRs): recon-status.sh; import --no-commit default;
  scrub gpgsign=false from import error text
```

**Depends on (already shipped):** put-history, join, assert, upstream-history
HITL runbook, global GPG guards, upstream-export-import skill hard rules.

---

## 9. Sources (paths)

- `/home/hunter/Projects/surmount/grok-build/scripts/{put-history-on-xai,join-main-into-onto,import-upstream-export,detect-upstream-export,sync-upstream,assert-process-pins,replay-onto-upstream}.sh`
- `/home/hunter/Projects/surmount/grok-build/docs/{upstream-history,upstream-onto-log,git-workflow}.md`
- `/home/hunter/Projects/surmount/grok-build/AGENTS.md`, `FORK.md`
- `/home/hunter/.agents/skills/upstream-export-import/SKILL.md`
- `/home/hunter/.agents/skills/pr-babysit/SKILL.md`
- `/home/hunter/.git-hooks/{pre-commit,post-commit,README.md}`
- `/home/hunter/.grok/{AGENTS.md,config.toml,hooks/block-unsigned-git-commit.*}`
- `/home/hunter/.grok/bundled/skills/create-workflow/SKILL.md`
- Prior research: `doc/dev/research/skills-survive-upstream-recon-2026-07-24.md`,
  `fork-paths-hardening-2026-07-24.md`

---

## 10. Non-claims

- Does not implement the skill or workflow.  
- Does not change scripts.  
- Live onto stack SHAs may move; re-read `docs/upstream-history.md` § Live stack.  
- Does not claim GitHub branch protection rules without checking the org
  (local GPG guards are verified on this machine).
