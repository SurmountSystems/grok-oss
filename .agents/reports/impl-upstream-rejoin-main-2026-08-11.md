# Rejoin Surmount main into onto tip

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Agent:** recon implementer (rejoin + mop commit)

---

## Executive status

| Item | State |
|------|--------|
| **Rejoin needed?** | **Yes** after `git fetch origin main` |
| **Join executed?** | **No** (blocked: dirty staged mop; unsigned commit needs operator TTY) |
| **Mop commit** | **Staged only** — recon-unsigned commit failed (GPG passphrase / no `/dev/tty`) |
| **Assert** | **OK** — `./scripts/assert-process-pins.sh HEAD` |
| **`cargo check -p xai-grok-shell --lib`** | **GREEN** (warnings only) |
| **`cargo check -p xai-grok-pager --lib`** | **GREEN** (warnings only) |
| **Named stashes** | **Kept** (not dropped) |
| **Push** | **Not done** |

**Bottom line:** `origin/main` moved past the tip ancestry. Mop shell+pager work is fully staged and compiles. Agent cannot create the recon-unsigned commit without a GPG TTY, so join is held until the operator commits mop then runs join on a real TTY.

---

## SHAs

| Ref | SHA |
|-----|-----|
| **HEAD before (and after this agent turn)** | `241f6f12260d0b977effb54f6f915b55b095d34e` |
| **origin/main before fetch** | `a1515fe11d037308e13db93d7086f78dad675153` (was ancestor of HEAD) |
| **origin/main after fetch** | `f17e84d85fef9aff03c54c7ed0fa5c3345aeb9ad` (**not** ancestor of HEAD) |
| **origin/main tip subject** | `fixes 2 (#31)` |

`git merge-base --is-ancestor origin/main HEAD` exits non-zero after fetch.

---

## What was done this turn

1. **Probe** — `./scripts/recon-status.sh`: onto tip, main was ancestor pre-fetch, dirty worktree (shell staged + pager unstaged).
2. **Stage mop** — one index for shell tests mop + pager lib mop + reports + `Cargo.lock` + shared/telemetry seams (84 paths).
   Reports included:
   - `.agents/reports/impl-upstream-shell-tests-compile-2026-08-11.md`
   - `.agents/reports/impl-upstream-pager-lib-compile-2026-08-11.md`
3. **Fetch** — `git fetch origin main` (and general origin); main advanced `a1515fe1..f17e84d8`.
4. **Commit mop (failed)** — recon-unsigned attempt with the onto Yes-row env var. Failure: gpg could not open `/dev/tty` for passphrase; no commit object written. Hooks honor the recon escape env, but `commit.gpgsign` still invokes gpg. Agent did **not** use banned bypasses (`--no-gpg-sign`, disable `commit.gpgsign`, fake `gpg.program`, hook disable).
5. **Join** — not started. Script requires clean worktree (or dirty override). Mop must land first so join does not mix uncommitted mop into a messy state. Prefer: mop commit, then `./scripts/join-main-into-onto.sh`, then join commit.
6. **Assert** — green on current HEAD.
7. **Compile spot-check** — shell and pager `--lib` both finished green with warnings only (dirty tree = staged mop content).

---

## Stashes (must keep)

```
stash@{0}: On onto-xai/b13fa526f511: recon-resume-local-dirt-2026-08-10
stash@{1}: On fixes-2: recon-temp-work-b-wip-2026-08-10
stash@{2}: On onto-xai/6e386420825b: onto living docs mid-stack 2026-07-24
stash@{3}: On main: wip: upstream docs before onto-xai
```

Neither named recon stash was dropped or applied this turn.

---

## Working tree after agent

- **Branch:** `onto-xai/b13fa526f511` @ `241f6f12`
- **Index:** 84 paths staged (full mop)
- **Worktree dirt:** none beyond staged
- **MERGE_HEAD:** no
- **This report:** on disk; **not** staged (add if you want it in the mop commit)

---

## Operator next steps (real TTY)

### 1. Commit mop (recon-unsigned Yes row on onto-xai/*)

```bash
cd /home/hunter/Projects/surmount/grok-build
# optional: include this report
git add .agents/reports/impl-upstream-rejoin-main-2026-08-11.md

ALLOW_UNSIGNED_COMMIT=1 git commit \
  -m "recon: mop shell tests + pager lib compile on onto tip" \
  -m "Shell --tests and pager --lib compile green after half-merge fixes." \
  -m "Product seams restored (plan mode tool filter, dual-auth rank, delete/title/privacy/deferred switch)." \
  -m "Reports: shell-tests + pager-lib compile 2026-08-11; rejoin report 2026-08-11." \
  -m "Recon intermediate: ALLOW_UNSIGNED_COMMIT under 2026-08-10 recon exception."
```

If gpg still demands a passphrase: unlock the agent on that TTY first. Do **not** leave signing permanently off.

### 2. Join main (strategy ours, stages by default)

```bash
./scripts/join-main-into-onto.sh
# default: merge -s ours origin/main --no-commit, verifies tree == onto tip tree

ALLOW_UNSIGNED_COMMIT=1 git commit \
  -m "Merge Surmount main into onto-xai (keep tip tree)" \
  -m "Join Surmount archive history so main is an ancestor of this tip." \
  -m "Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main." \
  -m "Recon intermediate: ALLOW_UNSIGNED_COMMIT under 2026-08-10 recon exception."
```

Or one-shot if signing works on your TTY: `DO_COMMIT=1 ./scripts/join-main-into-onto.sh`.

### 3. Verify

```bash
git merge-base --is-ancestor origin/main HEAD && echo main_is_ancestor
./scripts/assert-process-pins.sh HEAD
cargo check -p xai-grok-shell --lib
cargo check -p xai-grok-pager --lib
./scripts/recon-status.sh
```

### 4. Push / PR (only if you want)

```bash
# after mop + join commits on onto-xai/b13fa526f511
git push -u origin HEAD
# then open PR onto → main when ready
```

**No push performed by this agent.**

---

## Residual (not this turn)

- Shell **runtime** catalog reds (e.g. stuck-retry / stream_resumed) — product mop later.
- Pager **tests** / `--all-targets` still red (half-merge); lib only green.
- Named stashes still hold local dirt for later optional apply; do not drop blindly.
