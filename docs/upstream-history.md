# Canonical history & xAI monorepo exports

## Principle

**SurmountSystems/grok-oss is the complete, continuous git history** of the
open-source Grok Build tree **plus** fork features.

[xai-org/grok-build](https://github.com/xai-org/grok-build) is treated as a
**series of published snapshots**, not as a linear history we must share
commit hashes with.

## How xAI publishes (observed)

| Behavior | Evidence |
|----------|----------|
| Bot author | `grokkybara[bot]` |
| Message | `Publish harness and TUI open-source` / `initial sync from the monorepo` |
| Shape | **`main` is a single orphan commit** (no parents) |
| Updates | **Force-push a new root** with a new tree; previous export is replaced |
| Tags / GH Releases | Often none on that repo |
| Package versions | May stay at the same `CARGO_PKG_VERSION` while the tree still changes |

GitHub’s “entirely different commit histories” compare is **expected** after
each force-export. It is **not** a Surmount mistake.

## What we never do

| Anti-pattern | Why |
|--------------|-----|
| `git merge xai-org/main` when there is no merge-base | Creates nonsense history or fails |
| GitHub **Sync fork** that resets to upstream | Drops Surmount history and review trail |
| Blind `git reset --hard` to the new export | Loses OpenRouter, branding, rate-limit, etc. |
| “Lazy” bulk accept without reading the delta | Violates **review every contribution** |
| Rewriting Surmount `main` to match xAI SHAs | We are the archive; they are the feed |

## What we always do

1. **Preserve** Surmount history (tags, notes, PR commits).
2. **Detect** a new export (new tip SHA / tree on `xai-org/main`).
3. **Diff** against the **last imported tree**, not against git ancestry.
4. **Review** the delta (human and/or agent skill) file-by-file / area-by-area.
5. **Import** as a **normal commit on Surmount `main`** whose **tree** matches
   the export’s upstream-owned paths, while **keeping** fork-only paths.
6. **Record** the import in `docs/upstream-import-log.md` (SHA, tree, date).
7. **Re-apply / verify** fork seams (branding, OpenRouter, `grok-rate-limit`, …).

Result: `git log` on Surmount stays linear and meaningful. Each upstream
snapshot appears as one or more **reviewed** commits (“Import xAI export
`<shortsha>` …”), not as a disconnected root.

## Mental model

```
xai-org/main (force-push snapshots)
    │  export tree T0     export tree T1     export tree T2
    │       │                  │                  │
    │       ▼                  ▼                  ▼
    │   [orphan]            [orphan]           [orphan]
    │
    │   content-only diffs (git diff T0 T1) — no shared parents
    │
Surmount main (canonical continuous history)
    A──B──C──D──E──F──G──…
              ▲     ▲
              │     └── Import export T1 (reviewed)
              └── Import export T0 (or initial seed)
    + fork commits (OpenRouter, branding, rate-limit, …) interleaved / on top
```

Git may never see a merge-base with `xai-org/main`. **That is fine.** We use
**tree identity** (`git rev-parse <export>^{tree}`) as the upstream pin.

## Put Surmount history on their tip (`put-history-on-xai`)

**This is the script for “our history on theirs”.** Import (below) is the
*opposite* direction (their tree into Surmount).

After each export, rebuild a branch **parented at their tip** that carries
Surmount product commits via **real `git cherry-pick -x`**. When they
force-break history again, re-run with `FORCE=1` — nothing depends on a
stable xAI parent chain.

```
xai-org/main @ export tip
        │
        └── onto-xai/<short>   ← put-history-on-xai.sh (cherry-pick product)
                │
                └── join-main-into-onto.sh  (merge -s ours origin/main)
                          │
                          └── PR base=main ← head=onto-xai/*
```

| Goal | Command |
|------|---------|
| Stack Surmount product on current xAI tip | `SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh` |
| Resume after conflict resolution | `CONTINUE=1 SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh` |
| Rebuild stack from scratch (backs up first) | `FORCE=1 SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh` |
| Join Surmount `main` for a landable PR | `./scripts/join-main-into-onto.sh` then signed merge commit |
| Log | [`docs/upstream-onto-log.md`](upstream-onto-log.md) |

**There is no `MODE=overlay` / commit-tree mode** in the current script. Older
notes that mentioned those modes are obsolete — do not invent them.

`scripts/replay-onto-upstream.sh` is a thin alias of put-history.

**How it works:** cherry-pick each non-merge Surmount commit after the seed
(`docs/upstream-import-log.md` seed / `b189869…`) onto `xai-org/main`. Conflicts
stop for human/agent resolution; each continue is a **signed** commit on a real
TTY. Result: xAI tip is an ancestor of `onto-xai/*`, product sits on top.

**Limits (honest):**

- We **cannot** force-push or rewrite `xai-org/main` (pull-only remote).
- Without **join**, GitHub may still say “entirely different histories” vs
  Surmount `main` (no merge-base). Join records `main` as second parent and
  **keeps the onto tip tree** (`merge -s ours`).
- Mega product PRs on `main` (e.g. “Merge 2”, prior “merge xai 2”) re-touch
  hundreds of files and **will conflict hard** against a newer tip. Resolve
  carefully — never blind `--ours` / `--theirs` across the whole tree.

**Never** reset Surmount `main` to an onto-xai tip to “match” them.

## HITL runbook — put-history + join (compaction-safe)

### When bot issues fire

Detect workflow opens issues labeled `upstream-export` (e.g. #11, #14). Tips
age fast — **always re-fetch** and stack the **current** `xai-org/main`, not an
older issue SHA. Closing older issues as superseded is correct once a newer tip
lands.

Proved path: PR [#12](https://github.com/SurmountSystems/grok-oss/pull/12)
(`onto-xai/3af4d5d39897` → `main` after join).

### Full sequence

```bash
git fetch origin main
git fetch xai-org main --force
./scripts/detect-upstream-export.sh   # record XAI_TIP / XAI_TREE

# Anytime mid-stack / after compaction: live probe (prefer over guessing from docs)
./scripts/recon-status.sh             # or: just recon-status
# → branch, CHERRY_PICK/MERGE, UU count, onto-ish, recommended next human action

# clean worktree preferred
SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh
# on conflict: resolve carefully (see rules below), then:
git add -u
git cherry-pick --continue            # SIGNED on real TTY — agent never commits
CONTINUE=1 SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh

# when stack complete:
./scripts/join-main-into-onto.sh
git commit -S -m "Merge Surmount main into onto-xai (keep tip tree)" \
  -m "Join Surmount archive history so main is an ancestor of this tip." \
  -m "Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main."

just check
git push -u origin HEAD
# PR: base=main head=onto-xai/<short>  — close related export issues
# append docs/upstream-onto-log.md
```

### Scripts missing mid-stack

Early cherry-picks start from bare xAI tip — **`scripts/put-history-on-xai.sh`
does not exist until a later product commit lands**. Fish may run
“find-the-command” and fail. Use a temp copy with fixed `ROOT` until the pick
that adds `scripts/` lands:

```bash
REPO="$(pwd)"   # path to your grok-build / grok-oss clone
git show origin/main:scripts/put-history-on-xai.sh \
  | sed "s|ROOT=\"\$(cd \"\$(dirname \"\${BASH_SOURCE\[0\]}\")/..\" && pwd)\"|ROOT=\"$REPO\"|" \
  > /tmp/put-history-on-xai.sh
chmod +x /tmp/put-history-on-xai.sh
CONTINUE=1 SURMOUNT_REF=origin/main bash /tmp/put-history-on-xai.sh
```

### Conflict resolution rules (fork)

| Prefer | When |
|--------|------|
| **HEAD (onto tip)** | Upstream tip APIs evolved (new fields, ExitInfo, terminal recovery, spawn arity, Doctor absorbing old commands) |
| **Incoming product** | Grok OSS seams: `grok-oss` branding, OpenRouter, `grok-rate-limit`, economic mode, auto-compact thresholds, `oss_update`, updater default-off, `cli_hint_name()`, auto_implement, settings_modal **directory** only |
| **Union** | Import lists / Cargo features / both cancel_token **and** mut config for sampler failover |
| **origin/main as reference** | Product intent when ambiguous — do **not** wholesale overwrite tip-shaped files with older main |

**Hard anti-patterns:**

- No bulk find-and-replace; no thoughtless strip of markers without reading both sides
- No `git checkout --ours/--theirs` across **all** unmerged paths “to finish”
- No updating tests/fixtures to match the wrong side when intent is ambiguous — stop and compare `origin/main` + commit message
- No `just ci` mid-pick with markers left (Cargo fails on `<<<<<<<`)
- Agents **never** `git commit` / never GPG bypass; hand `git cherry-pick --continue` and `git commit -S`
- **No parent-solo conflict marathons** across many UU files (shell + pager + sampler). Use **strategic subagents** (below).
- **No wasteful swarm:** not one agent per file, not overlapping scopes, not N identical explores
- **Spawn first** on multi-file conflict resolve and on post-pick CI red: first tool turn is a tightly scoped child — parent must not pull CI logs, open failing tests, or re-grep the hot path before spawn. Join on short on-disk notes only.
- **Docs can be wrong.** Prefer code, `git show`, both conflict sides, and short child notes over assuming from FORK/AGENTS/research prose. Verify before claim.

### Subagents for conflict resolve (strategic, not wasteful)

**Main/parent thread = HITL only:** goals, spawn/wait, join short on-disk notes,
hand human signed git, brief status. Research and conflict resolve never run in
the parent. Project law: [`AGENTS.md`](../AGENTS.md).

Multi-file cherry-pick resolve is exactly the work global rules say belongs in
children: deep reads of both sides, tip vs product, surgical edits. Parent
holds the goal, the conflict table, and join checks — not full file contents.
**Hard stop:** spawn first; do not parent-marathon diagnose then spawn.

**Good fan-out for #7 (example, ~2–3 agents max):**

| Scope | Paths (disjoint) |
|-------|------------------|
| Shell session / spawn / run_loop | `xai-grok-shell/.../acp_session_impl/*`, `handle_request.rs`, cancel tests |
| Sampler + agent config | `request_task.rs`, `agent/config.rs`, `util/config/*` |
| Pager UI + docs | router, settings ui/modal, slash mod, user-guide md |

Each child gets: conflict rules from this doc, “prefer HEAD tip APIs / product
seams / union imports”, reference `origin/main` when product intent unclear,
**stage resolved files**, write a 5–10 line summary path if useful. Parent
verifies `git diff --name-only --diff-filter=U` is empty and no markers remain,
then hands human `git cherry-pick --continue`.

Same pattern for the **#12 mega-pick** with larger but still **disjoint**
buckets — never tree-wide bulk checkout, never 18 parallel one-file agents.

Detail also in project [`AGENTS.md`](../AGENTS.md) § *Onto / put-history*.
### Live stack (update when tip moves)

**Snapshot: 2026-08-12 — restack onto public Grok Build 1.0.3 (`e5fd4816`).**

| Field | Value |
|-------|--------|
| Branch | `onto-xai/b13fa526f511` (same PR #36 name; old joined tip on `backup/onto-xai-b13fa526f511-0f61ff44-joined-20260812`) |
| xAI tip | `e5fd4816d43260c15ba785f103990c1ed6cea230` (tree `25eefa9bdb3a4748cc065be3fa8200d04bc54493`) |
| Onto first-parent tip | `ee8a80d28cf5df2841b3762396b5921837e15813` (24 first-parent picks from `b13fa526f511..09c407e2`) |
| Onto tip (after join) | `e08e596167538f9e72da0760865340adfa34868f` |
| Compile mop | `4651593a1da1bbaf2831f316791cfb6d69c663e6` (shell + pager + update; tree `42dfccb62b5258ec7d8505f71e7318d89e88746a`) |
| Onto tree | `42dfccb62b5258ec7d8505f71e7318d89e88746a` (after compile mop; join tree was `ae3568e6fa7dcff47a63ca6f87c6c3e8fec18d28`) |
| Join | Done (`e08e5961`, `-s ours` via commit-tree); `origin/main` (`f17e84d8`) is ancestor |
| Cherry-picks | **Done** — no active pick |
| Toolchain | `rust-toolchain.toml` / fenix **1.97.1** |
| Nucleo | `FuzzySearchManager` reuse-per-root; `Nucleo::new(..., Some(2), 1)` |

**Finished on tip:** OpenRouter through `09c407e2 merge upstream` replayed onto 1.0.3, then join current `origin/main`, then compile mop `4651593a`. Report: `.agents/reports/recon-restack-1.0.3-2026-08-12.md`.

**Human next:**

```bash
# already authorized: force-with-lease this PR branch only
git push --force-with-lease origin onto-xai/b13fa526f511
# do not create a new PR; SurmountSystems/grok-oss#36 updates
```

**Historical notes:** Resume from mid-stack at `75a84a52` after 3/9 picks. Recon-unsigned intermediates via `ALLOW_UNSIGNED` + `commit-tree` (PreToolUse blocks `--no-gpg-sign` string). Mega-picks #4/#12 used intentional ours/theirs groups, not bulk tree checkout.

#### #7 — 18 unmerged paths (resolve carefully)

| Path | Intent (summary) |
|------|------------------|
| `xai-grok-shell/.../run_loop.rs` | Prefer **HEAD tip APIs**; port product tokens / economic / failover seams from incoming |
| `xai-grok-shell/.../spawn.rs` | Union: tip `query_params` / `env_http_headers` **and** product `effective_context_window` |
| `xai-grok-shell/.../handle_request.rs` | Prefer HEAD shape; product may pass `None` for tokens where tip differs |
| `xai-grok-shell/.../sampler_turn.rs` | Tip turn APIs + product economic/auto-compact wiring |
| `xai-grok-shell/.../model_switch.rs` | Tip switch path + product model/economic awareness |
| `xai-grok-sampler/.../request_task.rs` | **Union:** mut config **and** `cancel_token` (failover needs both) |
| `xai-grok-shell/.../agent/config.rs` | Tip config fields + product economic/compaction knobs |
| `xai-grok-shell/.../util/config/persist.rs` | Persist product settings without dropping tip keys |
| `xai-grok-shell/.../util/config/resolve/compaction.rs` | Product auto-compact thresholds on tip resolve path |
| `xai-grok-shell/.../cancel_running_task_tests.rs` | Match **resolved** production cancel API — not the wrong side blindly |
| `xai-grok-pager/.../router.rs` | **Union** imports: tip settings routes + economic/auto_compact |
| `xai-grok-pager/.../settings/ui.rs` | Tip UI dispatch + product economic/auto-compact setters |
| `xai-grok-pager/.../slash/commands/mod.rs` | Register product `/economic-mode` (file already **A** staged) |
| `xai-grok-pager/.../settings_modal/state.rs` + `tests.rs` | Tip modal + product economic rows |
| `xai-grok-pager/.../agent_view/render.rs` | Tip render + product indicators if any |
| user-guide `04-slash-commands.md`, `05-configuration.md` | Document `/economic-mode` and compact settings |

**Already staged product-only adds (keep):**

- `xai-grok-pager/src/slash/commands/economic_mode.rs` (**A**)
- `xai-grok-shell/src/util/config/economic_mode.rs` (**A**)
- Plus many **M** staged seams (`compaction_config`, openrouter, setters, actions, …)

**Hard anti-patterns for this pick (and #12):** no bulk `--ours`/`--theirs`; no blind marker strip; compare `origin/main` when product intent is unclear; leave `just ci` until markers are gone.

#### Human commands after stack + join

```bash
# after #7, #12, #13 all cherry-picked cleanly:
test -x scripts/join-main-into-onto.sh \
  || git show origin/main:scripts/join-main-into-onto.sh > scripts/join-main-into-onto.sh
chmod +x scripts/join-main-into-onto.sh
./scripts/join-main-into-onto.sh
git commit -S -m "Merge Surmount main into onto-xai (keep tip tree)" \
  -m "Join Surmount archive history so main is an ancestor of this tip." \
  -m "Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main."
just check
git push -u origin HEAD
# PR base=main head=onto-xai/6e386420825b ; close #11 #14 ; final onto-log row
```

**MSRV / Cargo.lock (rustc 1.92.0):** tip lockfiles may pull crates needing 1.94+.
Regenerate with MSRV-aware resolver, not bare HEAD lock:

```bash
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile
# or start from origin/main lock then the same
```

Never ship `aws-config` 1.9 / `kstring` 2.0.3 while toolchain is 1.92.0.

## Tools

| Tool | Role |
|------|------|
| [`scripts/put-history-on-xai.sh`](../scripts/put-history-on-xai.sh) | **Our history on their tip** → `onto-xai/<short>` (re-run replaces branch) |
| [`scripts/import-upstream-export.sh`](../scripts/import-upstream-export.sh) | **Their tree into Surmount** → `import/*` content-import review branch |
| [`scripts/detect-upstream-export.sh`](../scripts/detect-upstream-export.sh) | Fetch xAI tip; compare to last imported tree; exit codes for CI |
| [`scripts/sync-upstream.sh`](../scripts/sync-upstream.sh) | Detect → print both directions (or `PUT_ON_XAI=1` / `IMPORT_NOW=1`) |
| [`scripts/replay-onto-upstream.sh`](../scripts/replay-onto-upstream.sh) | Alias of `put-history-on-xai.sh` |
| [`.github/workflows/upstream-export.yml`](../.github/workflows/upstream-export.yml) | Scheduled detection; opens issue when a new export appears |
| Agent skill `upstream-export-import` | Checklist for both directions |

### Import safety (in-flight feature work)

| Rule | Behavior |
|------|----------|
| Dirty worktree | **Abort** unless `ALLOW_DIRTY=1` |
| Default base | **`origin/main`**, never the currently checked-out feature tip |
| Feature commits | **Not** included unless you set `BASE_REF=feat/your-branch` |
| After import | Returns to your previous branch (pass `--stay` to remain on `import/…`) |
| Tree apply | `git read-tree -u --reset <xai-tree>` — **not** `git add -A` (that bug once imported only a `result` symlink) |

**Recommended order when you have unmerged features:** finish the feature (merge
`main` *into* the feature with a normal push if the PR is open — **never rebase**
a published PR branch; see [git-workflow.md](git-workflow.md)), land it on
`main`, **or** decide the import should sit on the feature — then run import
with a clean tree.

## Review checklist (every import)

- [ ] Last import tree recorded; new export tip/tree captured
- [ ] `git diff --stat <old-tree> <new-tree>` reviewed (not empty “noise only”)
- [ ] Permission / workspace / shell / pager high-churn areas skimmed for behavior changes
- [ ] Fork-only files still present: branding, OpenRouter, `grok-rate-limit`, AUR, FORK.md, justfile, flake
- [ ] **Process pins still present** (import only restores `FORK_PATHS`; expanded list includes AGENTS/RESIDUAL/join/hermetic/`doc/dev`/assert script — run the assert, do not eyeball alone):
  - [ ] `./scripts/assert-process-pins.sh` or `just upstream-assert-process-pins` (fails if pins missing)
  - [ ] `AGENTS.md`, `RESIDUAL.md`, `FORK.md`
  - [ ] `scripts/join-main-into-onto.sh`, `scripts/with-ci-hermetic-path.sh`, `scripts/assert-process-pins.sh`, `scripts/recon-status.sh`
  - [ ] `scripts/put-history-on-xai.sh` + other import/sync scripts already in `FORK_PATHS`
  - [ ] `docs/upstream-history.md` (+ import/onto logs)
  - [ ] Review `FORK_PATHS` in `scripts/import-upstream-export.sh` only if the assert failed or a new process path is needed
- [ ] **Product regression filters** (assert is path-only; seams inside `xai-grok-*` need cargo). After process-pin assert, run the core block in [`doc/dev/upstream-regression-filters.md`](../doc/dev/upstream-regression-filters.md) (or FORK § *Upstream regression filters*), **or at least `just check` / `just ci`**. Smoke: DOGE default, window titles / `title.enabled`, stuck-retry / StreamResumed, `shell_collision`, dual-auth if those areas churned.
- [ ] **User-guide conflict resolve** — shared path `crates/codegen/xai-grok-pager/docs/user-guide/` is **not** in `FORK_PATHS`. On onto, re-check DOGE default theme, window titles / `title.enabled` vs `hide_header`, and Grok OSS / `grok-oss` branding sections against xAI base; do not drop fork copy for a clean merge alone.
- [ ] `just ci` or at least `just check` (prefer full gate before PR); if skipping full gate, the product filter block above is the minimum besides assert
- [ ] `docs/upstream-import-log.md` updated
- [ ] Signed commit on Surmount (no signing bypass)

## Skills & process pins vs recon (brief)

Skills are **multi-source**: product on this branch owns discovery/load order,
project skill roots, and user-guide; operator skill packs live under
`~/.agents/skills` (host); bundled packs are a network cache under
`~/.grok/bundled/skills`. See [`FORK.md`](../FORK.md) and
`doc/dev/research/where-skills-come-from-2026-07-24.md`.

| Survives recon without special care | At risk unless restored / re-picked |
|-------------------------------------|-------------------------------------|
| Host `~/.agents/skills`, `~/.grok/AGENTS.md` | Paths **not** listed in import `FORK_PATHS` |
| Paths listed in import `FORK_PATHS` (includes AGENTS, RESIDUAL, join/hermetic/assert, `doc/dev`) | Shared user-guide (xAI base on import; conflict on onto) |
| Product commits cherry-picked on onto | Onto tip missing a pin before join (`-s ours` cannot backfill) |

Assert anytime: `./scripts/assert-process-pins.sh` or `just upstream-assert-process-pins`.

**Join does not restore content** — missing process files on the onto tip stay
missing after `merge -s ours`. Chat-only pins do not survive compaction or recon.
Pin durable law in AGENTS / FORK / this doc (and host skills when operator-only).

## Pins

| Pin | Meaning |
|-----|---------|
| `upstream/export/<fullsha>` tag (optional) | Points at a fetched xAI commit for archaeology |
| Log line in `docs/upstream-import-log.md` | Authoritative “we absorbed this tree” record |
| Surmount `main` tip | What users and `grok-oss update --check` care about |

## Related

- Product versioning: [FORK.md](../FORK.md) (upstream package version + Surmount SHA)
- Superset policy: fork features on top; never hollow out upstream behavior
