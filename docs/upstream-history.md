# Canonical history and xAI monorepo exports

## Principle

**Surmount `main` is the continuous product history** (Grok Build tree plus
fork features). [xai-org/grok-build](https://github.com/xai-org/grok-build) is a
**series of published snapshots** — a content feed, not a history we must share
commit hashes with.

GitHub may say the histories are “entirely different.” **Expected.** There is
often **no merge-base**. Do not “Sync fork” or reset Surmount `main` to theirs.

## How xAI publishes (observed)

| Behavior | Notes |
|----------|--------|
| Bot author | `grokkybara[bot]` |
| Messages | e.g. `Publish harness…`, `Synced from monorepo` |
| Shape | Often an **orphan** force-push root; sometimes a **short bot chain** |
| Updates | Force-push replaces the tip; package versions may not bump |

We absorb **trees** (`git rev-parse <export>^{tree}`), not their parent graph.
Whether they stop rewriting history is **unknown** — do not promise stability.

## Two directions (+ join for a landable PR)

| Job | Script | Branch | When |
|-----|--------|--------|------|
| **Their tree → Surmount** | `scripts/import-upstream-export.sh` | `import/*` | Product archive on Surmount `main` after review |
| **Our commits → their tip** | `scripts/put-history-on-xai.sh` | `onto-xai/<short>` | Preferred when histories break: one branch that is a **descendant of their tip** and carries Grok OSS |
| **Join `main` into onto** | `scripts/join-main-into-onto.sh` | same `onto-xai/*` | After put-history: make the tip also a **descendant of Surmount `main`** so GitHub compare / PR works |

```
xai-org/main  (force-pushed snapshots)
      │
      ├── import/*     ← their tree into Surmount base + fork paths
      │
      └── onto-xai/*   ← cherry-pick product onto their tip
              │
              └── join main (-s ours)  ← main becomes ancestor; tip tree unchanged
                        │
                        └── PR → Surmount main
```

**Preferred HITL when they rewrite history:**

1. **Put** our product commits on their current tip (`put-history`).
2. **Join** Surmount `main` into that tip (`join-main-into-onto`, strategy
   **ours**) so the branch is in our history graph and is PR-able.
3. Open a normal PR: **base `main` ← head `onto-xai/*`**.

Import remains the way to record a reviewed **content absorption** into
Surmount’s archive under Surmount-first parents (different job from the join).

Without step 2, GitHub often says **“entirely different commit histories”**
(no merge-base). That is expected until you join.

Detect: `./scripts/detect-upstream-export.sh` or `just upstream-detect`.

## Put history on their tip (cherry-pick)

`scripts/put-history-on-xai.sh` runs **real `git cherry-pick -x`** of Surmount
product commits (after the seed) onto the current `xai-org/main` tip.

There is **no** `MODE=overlay` / commit-tree mode in the current script. Older
docs that mentioned those modes are obsolete.

```bash
git fetch xai-org main --force
# clean worktree preferred
./scripts/put-history-on-xai.sh
# FORCE=1 SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh   # rebuild

# on conflict:
git add -u
git cherry-pick --continue    # signed on a real TTY when commit.gpgsign=true
CONTINUE=1 ./scripts/put-history-on-xai.sh
```

Does not push. Does not rewrite Surmount `main`. Does not touch xAI (pull-only).

## Join Surmount `main` into an onto tip (landable graph)

After put-history, `onto-xai/*` sits on an xAI root and usually shares **no**
merge-base with Surmount `main`. To open a normal PR you must **join** our
archive history into the tip **without** replacing the tip-aligned tree.

```bash
# on onto-xai/<short>, clean worktree
./scripts/join-main-into-onto.sh
# stages merge -s ours --allow-unrelated-histories; tree identity checked
# human TTY:
git commit -S -m "Merge Surmount main into onto-xai (keep tip tree)" \
  -m "Join Surmount archive history so main is an ancestor of this tip." \
  -m "Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main."

# or try commit in-script when GPG/TTY works:
# DO_COMMIT=1 ./scripts/join-main-into-onto.sh

just check
git push -u origin HEAD
# PR base=main head=onto-xai/<short>
```

| Check | Expect |
|-------|--------|
| `git merge-base --is-ancestor origin/main HEAD` | true |
| `git rev-parse HEAD^{tree}` | same as pre-join onto tree |
| GitHub `main...onto` | renders a real compare (not “entirely different histories”) |

**Strategy `ours`:** second parent is `main`; **tree stays the onto tip**
(current export + product). This is not a content merge of older `main` over a
newer xAI tree. Main-only obsolete paths remain reachable via the second parent.

Just recipes: `just upstream-put-history`, `just upstream-join-main`.

Does not push. Does not rewrite `main`. Does not touch xAI.

## Import their tree into Surmount

```bash
./scripts/import-upstream-export.sh           # stages import/* from origin/main
./scripts/import-upstream-export.sh --stay
```

Uses `git read-tree` of the xAI tree, restores fork-only paths, then a **signed**
content-import commit (or leaves staged for a human TTY). Re-apply OpenRouter,
branding, rate-limit seams; run `just check`; append the import log; PR to `main`.

## Never do

| Don’t | Do |
|-------|-----|
| `git merge xai-org/main` with no merge-base (content) | Content **import** or **put-history** |
| Content-merge older Surmount `main` over a tip-aligned onto tree | **`join-main-into-onto`** (`-s ours`) then PR |
| GitHub Sync fork that drops Surmount | Branch from Surmount `main` |
| Blind `reset --hard` to export | Review + re-apply seams |
| Disable GPG for import/onto/join commits | Human signs on a real TTY |
| Reset Surmount `main` to an onto-xai tip “to match” them | PR onto → `main` after join |

## Signed commits

Agents do not bypass GPG. Prefer multi `-m` flags (not heredocs) for commands
handed to humans. See project `AGENTS.md` and global GPG rules.

## Logs

| File | Meaning |
|------|---------|
| [`upstream-import-log.md`](upstream-import-log.md) | Reviewed trees absorbed into Surmount |
| [`upstream-onto-log.md`](upstream-onto-log.md) | Stacks parented at an xAI tip |

## Related

- Product divergences: [`FORK.md`](../FORK.md)
- Open PR workflow: [`git-workflow.md`](git-workflow.md)
