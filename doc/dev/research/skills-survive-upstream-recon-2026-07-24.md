# Skills & process pins vs upstream reconciliation (2026-07-24)

**Mode:** read-only research.  
**Workspace:** `/home/hunter/Projects/surmount/grok-build`  
**Sources:** `docs/upstream-history.md`, `docs/upstream-onto-log.md`, `FORK.md`,
`AGENTS.md`, `RESIDUAL.md`, scripts under `scripts/*upstream*` / put-history /
join / import, host skill `upstream-export-import` + `skill-maintenance`,
prior note `doc/dev/research/where-skills-come-from-2026-07-24.md`.

---

## Plain answer

Upstream recon has **two opposite jobs**. Only **import** wholesale-replaces
the tree from an xAI snapshot (then restores a short fork-only list).  
**Put-history** rebuilds product by cherry-pick on a bare tip. **Join** only
rewires history (`merge -s ours`) and **does not fold content** from `main`.

**What survives without special care**

| Layer | Survives recon? | Why |
|-------|-----------------|-----|
| Host operator skills `~/.agents/skills/**` | Yes (git-irrelevant) | Outside the product tree; never touched by import/put-history |
| Global pins `~/.grok/AGENTS.md` | Yes | Host config, not product tree |
| Bundled skill cache `~/.grok/bundled/skills/**` | Survives git; **not** durable for edits | Overwritten by product **bundle sync**, not by onto/import |
| Fork-only paths restored by import (`FORK_PATHS`) | Yes on import | Explicit checkout from `BASE_REF` after `read-tree` |
| Product commits after seed (onto path) | Usually | Reappear via cherry-pick if they were on `SURMOUNT_REF` |

**What gets clobbered or dropped unless you re-apply**

| Layer | Import | Put-history conflict | Join (`-s ours`) |
|-------|--------|----------------------|------------------|
| Paths **not** in `FORK_PATHS` and **not** in xAI tree | **Deleted** from import tree | N/A until a pick adds them | Tree kept = onto tip only — never pulls missing main-only files |
| Shared paths (user-guide, shell/pager) | **xAI version** | Wrong side resolve can drop product | No content merge |
| `AGENTS.md`, `RESIDUAL.md`, `doc/dev/**`, `README` (fork shape) | **Not** in `FORK_PATHS` → **drop / xAI** | Depends on picks + resolve | Keeps whatever is already on tip |
| `scripts/join-main-into-onto.sh`, hermetic PATH script | **Not** in `FORK_PATHS` → **drop** | Comes back only if stacked commits include them | Same |
| Host skill bodies | Unaffected | Unaffected | Unaffected |
| Stale host skill text about MODE=overlay | Unaffected by git; **discipline gap** | Skill lies about current scripts | — |

**Process pins must live on disk in places recon cannot silently delete**
(project `AGENTS.md` + FORK + living upstream docs **and** host
`~/.agents` / `~/.grok/AGENTS.md` for operator skills). Chat-only pins die
at compaction and never ride the onto stack.

---

## 1. Mental model — four workflows

```text
xai-org/main  (force-push orphan snapshots; tree feed only)
      │
      ├─ detect-upstream-export.sh
      │     compare tip^{tree} vs last completed row in docs/upstream-import-log.md
      │
      ├─ IMPORT  (their tree → Surmount)     import-upstream-export.sh
      │     base origin/main → read-tree -u --reset <xai-tree>
      │     → restore FORK_PATHS from base → import/* → PR → main
      │
      └─ PUT-HISTORY  (our commits on their tip)  put-history-on-xai.sh
            checkout -B onto-xai/<short> @ xAI tip
            cherry-pick -x every non-merge Surmount commit after seed
            → join-main-into-onto.sh  (merge -s ours origin/main)
            → PR base=main head=onto-xai/*   (tree = tip + product stack)
```

| Job | Script | Branch | Tree result |
|-----|--------|--------|-------------|
| Detect | `detect-upstream-export.sh` | — | No tree change |
| Import | `import-upstream-export.sh` | `import/xai-export-*` | ≈ xAI tree **+** restored `FORK_PATHS` |
| Stack product | `put-history-on-xai.sh` | `onto-xai/<12hex>` | xAI tip + cherry-picked Surmount product |
| Join for PR | `join-main-into-onto.sh` | same onto | **Identical** onto tip tree; `main` becomes ancestor only |
| Orchestrate | `sync-upstream.sh` | — | Detect; optional `PUT_ON_XAI=1` / `IMPORT_NOW=1` |

**There is no `MODE=overlay` / commit-tree mode** in current put-history
(repo docs + script). Real `git cherry-pick -x` only. Host skill
`upstream-export-import` still describes obsolete MODE=history/overlay —
treat **repo** `docs/upstream-history.md` as law for mechanics.

**Never:** reset Surmount `main` to xAI or to onto tip; GitHub Sync fork
that drops Surmount history; blind `merge xai-org/main` without merge-base;
bulk `--ours`/`--theirs` across an unmerged set.

---

## 2. Import — what overwrites product docs / skills

### Mechanism

1. Clean worktree; base **`origin/main`** (not feature HEAD).
2. `git read-tree -u --reset "$XAI_TREE"` — **entire** index/worktree becomes the export.
3. Restore only paths in **`FORK_PATHS`** via `git checkout "$BASE_REF" -- "$p"`.
4. Stage (`git add -u` + re-add fork paths); create import commit on `import/*`.

### `FORK_PATHS` today (`scripts/import-upstream-export.sh`)

| Restored (survive import) |
|---------------------------|
| `FORK.md` |
| `CONTRIBUTING.md` |
| `SECURITY.md` |
| `justfile`, `flake.nix`, `flake.lock` |
| `docs/upstream-history.md`, `docs/upstream-import-log.md`, `docs/upstream-onto-log.md`, `docs/git-workflow.md` |
| `packaging/` |
| `scripts/detect-upstream-export.sh`, `import-upstream-export.sh`, `sync-upstream.sh`, `put-history-on-xai.sh`, `replay-onto-upstream.sh` |
| `.github/workflows/upstream-export.yml` |
| `crates/codegen/grok-rate-limit` |

### Explicitly **not** restored (import clobber / drop risk)

| Path / area | Risk |
|-------------|------|
| **`AGENTS.md`** | Surmount-only process law → **deleted** if absent from xAI tree |
| **`RESIDUAL.md`** | Open residual tracker → **deleted** |
| **`README.md`** | If present upstream, **xAI text wins**; fork branding lost |
| **`doc/dev/**`, `docs/dev/**`** | Research / RCA / skill-pin notes → **deleted** |
| **`scripts/join-main-into-onto.sh`** | Land path script → **deleted** (not in list) |
| **`scripts/with-ci-hermetic-path.sh`** | CI PATH hermeticity → **deleted** |
| Other `.github/workflows/*` (e.g. `ci.yml`) | Not in list → **xAI or gone** |
| Product seams **inside** `xai-grok-*` | Always from xAI tree; script **prints** re-apply checklist (OpenRouter, branding, sampler) — not auto-restored |
| User-guide under pager | Upstream-owned paths → **xAI** (fork edits must re-apply) |
| Project `.agents/skills` (if ever committed) | **Not** in `FORK_PATHS` → **deleted** on import |
| Host `~/.agents/skills` | Outside repo — **safe** from import |

Import review checklist in `docs/upstream-history.md` says keep FORK / branding /
OpenRouter / rate-limit / justfile / flake — it does **not** name `AGENTS.md`,
`RESIDUAL.md`, join script, or `doc/dev/**`.

### After import

Script notes seams live in `xai-grok-*` and must be reconciled with
`git diff $BASE_REF -- …`. Human/agent must re-apply product, then PR to
`main`. A lazy merge of import **without** restoring missing Surmount-only
files permanently loses those pins on `main`.

---

## 3. Put-history (onto-xai) — what survives / conflicts

### Mechanism

1. Fetch force tip; branch `onto-xai/<short12>` at **bare xAI tip**.
2. Commit list: `git rev-list --reverse --no-merges $SEED_REF..$SURMOUNT_REF`
   (seed from import log “seed” row or hardcoded `b189869…`).
3. `git cherry-pick -x` each commit; stop on conflict.
4. Safe default: existing good stack → exit 0 (no rebuild). `FORCE=1` backs up
   then rebuilds (destructive to that branch only — not `main`).

### Survival rules

| Class | Behavior |
|-------|----------|
| Surmount-only files added after seed | Reappear when their product commit is picked (if not skipped as merge) |
| Shared files changed on tip **and** main | **Conflict** — human/agent resolve using conflict table |
| Early picks | Scripts may be missing until a later product commit lands them (HITL: temp `/tmp` put-history with `ROOT` patch) |
| Merge commits on Surmount | **Skipped** (`--no-merges`) — content must live in non-merge parents or it never stacks |

### Conflict discipline (docs + `AGENTS.md`) — product docs risk

| Prefer | When |
|--------|------|
| **HEAD (onto tip)** | Upstream tip APIs evolved |
| **Incoming product** | Grok OSS seams: branding, OpenRouter, rate-limit, economic mode, auto-compact, oss_update, updater default-off, etc. |
| **Union** | Import lists, Cargo features, dual cancel_token + mut config |
| **`origin/main` as reference** | Ambiguous product intent — **not** wholesale overwrite tip-shaped files |

**Doc / skill-adjacent conflict surfaces already seen in HITL (#7):**  
user-guide `04-slash-commands.md`, `05-configuration.md`, pager settings UI,
shell config/compaction paths. Wrong-side resolve **drops** fork user-facing
docs even though host skills are fine.

**Anti-patterns that clobber product:** blind `--ours`/`--theirs` on whole
unmerged set; strip markers without reading; “fix tests to match whichever
side compiles”; parent-solo marathons across shell+pager+sampler.

---

## 4. Join main into onto — does **not** restore content

`join-main-into-onto.sh`:

- `git merge -s ours $MAIN_REF --allow-unrelated-histories --no-commit`
- Verifies `write-tree` **equals** pre-merge onto tree; aborts if not
- Default: leave staged for **human** `git commit -S` (agents never commit)

**Implications for docs/skills:**

- Join **cannot** heal an onto tip that never cherry-picked `AGENTS.md` /
  `join-main` script / research docs — those stay missing until re-applied
  on the tip or fixed on `main` after land.
- Join **cannot** overwrite tip with older main docs (by design).
- PR onto → `main` lands the **onto tip tree**. Main’s pre-join file set is
  not a content union.

---

## 5. Where process pins **must** live to survive

Ranked by survival under recon + compaction:

### A. Must live in product git (survive collaborators + onto land)

| Pin home | Role | Import safety today |
|----------|------|---------------------|
| **`FORK.md`** | Product divergence inventory; sync job table | **In `FORK_PATHS`** |
| **`docs/upstream-history.md`** | Canonical recon law + HITL + conflict rules | **In `FORK_PATHS`** |
| **`docs/upstream-onto-log.md`**, **`docs/upstream-import-log.md`** | Append-only ledgers | **In `FORK_PATHS`** |
| **`docs/git-workflow.md`** | PR merge-not-rebase | **In `FORK_PATHS`** |
| **`AGENTS.md`** (repo) | Project Hard stop, onto recovery, residual pointer | **NOT in `FORK_PATHS` — fragile** |
| **`RESIDUAL.md`** | Open human-intent only | **NOT in `FORK_PATHS` — fragile** |
| User-guide under pager (product-facing skills/subagents prose) | Shipped docs | Upstream path — **re-apply on conflict/import** |
| Optional project `.agents/skills/**` | Collaborator skill packs | Supported by product loader; **not** import-protected |

### B. Must live on host (operator skill bodies + cross-repo law)

| Pin home | Role | vs product recon |
|----------|------|------------------|
| **`~/.grok/AGENTS.md`** | Global: never agent-commit, Hard stop, subagent strategy | Outside tree — **survives** all recon |
| **`~/.agents/skills/**`** | Maintained skill home (`implement`, `pr-babysit`, `upstream-export-import`, …) | Outside tree — **survives**; skill-maintenance reconciles vs bundled |
| **`~/.agents/skills/_SKILL_RULES-…`**, `shared/references/subagent-token-strategy.md` | Author law + deep strategy | Host-only |
| **`~/.grok/bundled/skills/**`** | Platform defaults cache | Survives git; **sync overwrites** local edits |

### C. Does **not** survive as authority

| Location | Failure |
|----------|---------|
| Chat-only process corrections | Compaction erases; never on onto tip |
| Only `~/.grok/skills` lag mirrors | Shadowed by `~/.agents`; easy to drift |
| Only editing `~/.grok/bundled` | Bundle sync restores managed tree |
| Assuming “skills are off-branch so recon never matters” | Product **user-guide** + optional project skills + loader code **are** on-branch; import/`FORK_PATHS` gaps still kill **`AGENTS.md` / residual / research** |

**Same-turn disk pin rule** (global + project AGENTS): operator corrections that
change “what is left / how recon works” must land in living files **in the same
turn** — FORK / AGENTS / upstream-history / residual / host skill bodies as
appropriate — not “I’ll remember.”

---

## 6. Gaps in recon discipline for skills & process pins

| # | Gap | Evidence / effect |
|---|-----|-------------------|
| 1 | **`FORK_PATHS` incomplete for process** | `AGENTS.md`, `RESIDUAL.md`, `doc/dev/**`, `join-main-into-onto.sh`, hermetic PATH script, README not restored → import **silently drops** Surmount process law |
| 2 | **Import checklist under-names process files** | `upstream-history.md` review list: branding/OpenRouter/rate-limit/FORK/justfile/flake — not AGENTS/residual/research/join script |
| 3 | **Host skill `upstream-export-import` is stale** | Still documents `MODE=history/overlay`, `FORCE_BRANCH=1`, no join-main step as first-class land path; conflicts with repo cherry-pick + `join-main-into-onto.sh` truth |
| 4 | **No skill-pack protection story** | If project ever commits `.agents/skills`, import will wipe them unless list grows; no FORK residual for “skills on branch” |
| 5 | **User-guide is conflict surface without fork-only restore** | Import takes xAI user-guide; onto conflicts require manual product re-apply (`economic-mode`, token-efficiency, skills docs) |
| 6 | **Join does not backfill** | Onto tip missing a process file → PR lands without it; join cannot fix |
| 7 | **Merge commits skipped in put-history** | `--no-merges` — if lasting pins only exist as merge commit trees, they may not stack (unusual but real) |
| 8 | **Skill-maintenance vs product recon are separate** | skill-maintenance keeps `~/.agents` ↔ bundled; **does not** assert product `AGENTS.md` / FORK / import `FORK_PATHS` completeness after recon |
| 9 | **Dual residual honesty** | RESIDUAL can say “finish join” while import ledger still *pending* — different jobs; easy to confuse “landed onto PR” with “import absorbed tree” |
| 10 | **FORK.md silent on skills** | Divergence inventory has no skills / process-pin survival section; agents must invent policy from scattered docs |

---

## 7. Hardening recommendations (ranked checklist)

Priority: **P0** = prevent silent loss of law; **P1** = make recon hard to misuse; **P2** = polish.

### P0 — stop silent clobber of process pins

1. **Extend `FORK_PATHS`** in `import-upstream-export.sh` to restore at least:
   - `AGENTS.md`
   - `RESIDUAL.md`
   - `README.md` (or document deliberate xAI README + re-apply branding)
   - `scripts/join-main-into-onto.sh`
   - `scripts/with-ci-hermetic-path.sh`
   - `doc/dev/` and/or `docs/dev/` (research that must survive)
   - any other Surmount-only scripts/workflows not upstream (`ci.yml` deltas if fork-owned)
2. **Update import review checklist** in `docs/upstream-history.md` with an
   explicit “process pins still present” block (AGENTS, RESIDUAL, join script,
   FORK, upstream logs, research dirs).
3. **Post-import assertion script** (or just recipe): fail if
   `test -f AGENTS.md && test -f FORK.md && test -f scripts/join-main-into-onto.sh`
   (and selected paths) after import commit; print missing list.
4. **Same-turn pin rule already written** — enforce for recon: after any
   import/onto land, update FORK hierarchical note + append onto/import logs
   **before** calling residual “done.”

### P1 — recon skill & doc honesty

5. **Rewrite host skill** `~/.agents/skills/upstream-export-import/SKILL.md` to
   match current scripts: cherry-pick only; `FORCE=1` not `FORCE_BRANCH`;
   mandatory **join-main** before PR; conflict table; **no MODE=overlay**.
6. **Add FORK.md § “What recon keeps”** — short table: import restores X;
   put-history cherry-picks product; join keeps tip tree; host skills outside.
7. **Conflict resolve prompt template** for subagents must list **docs paths**
   (user-guide, AGENTS if conflicted) under “prefer product seams,” not only
   Rust seams.
8. **After every onto PR lands:** verify tip tree contains `AGENTS.md`,
   `FORK.md`, join/put-history scripts, residual policy files — not only
   `just check` green.

### P2 — skills product surface & maintenance

9. **skill-maintenance optional hook:** after recon or on `/skill-maintenance`,
   offer “product pin check” against grok-build `AGENTS.md` / FORK existence
   (without committing skill work into product repo).
10. **If shipping project skills:** commit under `.agents/skills/`, add that
    root to `FORK_PATHS`, and document in FORK; else keep operator skills host-only
    and treat user-guide + AGENTS as the branch-facing process surface.
11. **Do not edit `~/.grok/bundled` for pins** — only agents home + bundle
    source pipeline; recon does not replace this rule.
12. **Keep residual clean:** finished recon process truth → FORK or
    upstream-history; residual only open import-vs-onto dual-path decisions.

### Operator checklist (run after any recon)

```text
[ ] Detect recorded XAI_TIP / XAI_TREE
[ ] Direction chosen deliberately (import vs put-history — not both confused)
[ ] Import: FORK_PATHS review + git diff base for seams + process files present
[ ] Put-history: conflicts resolved with tip-API / product-seam / union rules
[ ] Join: tree identity == pre-join onto tip; signed human commit
[ ] just check
[ ] AGENTS.md + FORK.md + RESIDUAL.md + join script still on tip
[ ] User-guide product sections re-checked if those paths changed
[ ] Append import log and/or onto log
[ ] Host upstream-export-import skill still matches scripts (or fix skill)
[ ] No chat-only “we should remember” process rules
```

---

## 8. Workflow × artifact matrix (summary)

| Artifact | Import | Put-history | Join | Host skill-maintenance |
|----------|--------|-------------|------|------------------------|
| `FORK.md` | Restored | Via picks | Keep tip | No |
| `docs/upstream-*` | Restored | Via picks | Keep tip | No |
| `AGENTS.md` | **At risk** | Via picks / resolve | Keep tip | Points at it; does not restore |
| `RESIDUAL.md` | **At risk** | Via picks | Keep tip | No |
| `doc/dev/research/**` | **At risk** | Via picks | Keep tip | No |
| User-guide skills/subagents | xAI base | Conflict-prone | Keep tip | No |
| `~/.agents/skills/**` | Untouched | Untouched | Untouched | **Primary** |
| `~/.grok/AGENTS.md` | Untouched | Untouched | Untouched | Assert pins |
| `~/.grok/bundled/skills` | Untouched by recon | Untouched | Untouched | Copy **from** → agents; never write durable pins only here |
| OpenRouter / branding seams | Clobbered in crates; re-apply | Conflict resolve | Keep tip | No |

---

## 9. Ranked hardening list (return surface)

1. **P0** Expand `FORK_PATHS` (AGENTS, RESIDUAL, join script, hermetic PATH, research dirs, README policy).  
2. **P0** Post-import / post-onto “process files present” assertion.  
3. **P0** Extend import review checklist in `docs/upstream-history.md`.  
4. **P1** Fix stale `upstream-export-import` skill (cherry-pick + join-main; kill MODE=overlay).  
5. **P1** FORK.md short “what recon keeps / clobbers” table.  
6. **P1** Onto conflict prompts include user-guide + process docs as product seams.  
7. **P1** Land verification: tip has AGENTS/FORK/scripts, not only CI green.  
8. **P2** skill-maintenance optional product-pin probe; never treat bundled as pin home.  
9. **P2** If project skills ever commit: protect `.agents/skills` in `FORK_PATHS`.  
10. **P2** Residual hygiene: dual import-vs-onto status explicit; finished rules leave residual.

---

## Related paths

| Path | Role |
|------|------|
| `/home/hunter/Projects/surmount/grok-build/docs/upstream-history.md` | Canonical recon + HITL |
| `/home/hunter/Projects/surmount/grok-build/docs/upstream-onto-log.md` | Onto stack ledger |
| `/home/hunter/Projects/surmount/grok-build/docs/upstream-import-log.md` | Import ledger |
| `/home/hunter/Projects/surmount/grok-build/scripts/import-upstream-export.sh` | `FORK_PATHS` authority |
| `/home/hunter/Projects/surmount/grok-build/scripts/put-history-on-xai.sh` | Cherry-pick stack |
| `/home/hunter/Projects/surmount/grok-build/scripts/join-main-into-onto.sh` | History join, tree kept |
| `/home/hunter/Projects/surmount/grok-build/FORK.md` | Product divergence + sync jobs |
| `/home/hunter/Projects/surmount/grok-build/AGENTS.md` | Project process pins |
| `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md` | Open residuals only |
| `/home/hunter/Projects/surmount/grok-build/doc/dev/research/where-skills-come-from-2026-07-24.md` | Skill load order / host vs branch |
| `~/.agents/skills/upstream-export-import/SKILL.md` | Host recon skill (stale sections) |
| `~/.agents/skills/skill-maintenance/SKILL.md` | Host skill reconcile (not product recon) |
| `~/.grok/AGENTS.md` | Global Hard stop / never commit |

---

*End of note. Implementation of FORK_PATHS / skill rewrites is out of scope for this research file.*
