# CI cluster: worktree GC + export_github (TRY 2)

**Date:** 2026-08-12
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Prior report (same cause, not present in this tree):** [`.agents/reports/bug-worktree-export-github-cluster-2026-08-11.md`](bug-worktree-export-github-cluster-2026-08-11.md)

---

## Status

**Green.** One shared root cause: host `commit.gpgsign=true` plus no pinentry TTY. Fixture plants and automated export commits inherited global git config and died on GPG.

| Package | Filter | After |
|---------|--------|-------|
| `xai-fast-worktree` | `auto_gc::tests` (`--features metadata`) | **55/55** |
| `xai-fast-worktree` | `git::worktree::tests` | **7/7** |
| `xai-grok-workspace` | `export_github::tests` | **15/15** (all listed names) |

`auto_gc` is compiled only with the `metadata` feature. Bare `cargo test -p xai-fast-worktree --lib auto_gc::tests` matches **0** tests.

---

## Observed red

### `git::worktree::tests::keeps_locked_registration`

Ran first, before any edit:

```text
cargo test -p xai-fast-worktree --lib --features metadata \
  git::worktree::tests::keeps_locked_registration -- --nocapture --test-threads=1
```

```text
git ["commit", "-m", "init"] failed: error: gpg failed to sign the data:
...
gpg: cannot open '/dev/tty': No such device or address
fatal: failed to write commit object
```

`init_repo_with_worktrees` used raw `Command::new("git")` for `commit`. Host `~/.gitconfig` has `commit.gpgsign=true`. Agent/CI has no pinentry TTY.

Same plant path in `auto_gc::tests::plant_stale_git_worktree` (raw `git commit -m i`).

### `export_github`

Product `git()` set author env vars but still inherited host `commit.gpgsign` / hooks. Automated export `commit` hits the same GPG failure. Classification-only tests (no commit) were already fine.

---

## Root cause

Not 22 independent product bugs.

1. Host `commit.gpgsign=true` (operator policy).
2. Worktree / auto_gc fixtures used raw `git` without masking global/system config.
3. Product `export_github::git()` did the same, so automated export commits required interactive GPG.
4. The 2026-08-11 hermetic fix was **not** in this tree (reverted or never landed). TRY 2 re-applied it.

Already-correct in-tree pattern: `xai_test_utils::git::{init_git_repo, git_commit_all, run_git}` (`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, local `commit.gpgsign=false`). Sibling modules (`sync.rs`, `api.rs` tests) already use it.

---

## Fix

No test expectation rewrites. Named contracts (stale-registration prune, export mapping/push) unchanged.

### 1. Product: `export_github` automated git

**File:** `crates/codegen/xai-grok-workspace/src/export_github.rs`

In product `git()`, mask host global/system config. Local `.git/config` still applies. Author/committer stay forced to the export identity.

**Named contract:** Automated GitHub export commits must succeed without interactive GPG/pinentry or host hooks.

### 2. Harness: worktree registration fixtures

**File:** `crates/codegen/xai-fast-worktree/src/git/worktree.rs`

`init_repo_with_worktrees` / `run_git` now use `xai_test_utils::git::{init_git_repo, git_commit_all, run_git}` plus `require_git!()`.

### 3. Harness: auto_gc plant helper

**File:** `crates/codegen/xai-fast-worktree/src/auto_gc.rs`

`plant_stale_git_worktree` uses the same hermetic helpers.

---

## Red + green commands

**Red (before edit):**

```bash
cargo test -p xai-fast-worktree --lib --features metadata \
  git::worktree::tests::keeps_locked_registration -- --nocapture --test-threads=1
# FAIL: gpg failed to sign / cannot open '/dev/tty'
```

**Green (same filter after edit):**

```bash
cargo test -p xai-fast-worktree --lib --features metadata \
  git::worktree::tests::keeps_locked_registration -- --test-threads=1
# ok
```

**Verify (all listed names):**

```bash
cargo test -p xai-fast-worktree --lib --features metadata auto_gc::tests -- --test-threads=1
# 55 passed

cargo test -p xai-fast-worktree --lib git::worktree::tests -- --test-threads=1
# 7 passed

cargo test -p xai-grok-workspace --lib export_github::tests -- --test-threads=1
# 15 passed
```

**fmt:** `cargo fmt -p xai-fast-worktree -p xai-grok-workspace` (check clean)

**clippy:**
- `cargo clippy -p xai-fast-worktree --lib --features metadata -- -D warnings` clean
- `cargo clippy -p xai-grok-workspace --all-targets -- -D warnings` clean
- `--all-targets` on `xai-fast-worktree` still hits pre-existing `clippy::disallowed_methods` at `api.rs:3638` (`Command::spawn` in tests). Not introduced here. Did not edit `api.rs`.

---

## Not changed

- Operator global `commit.gpgsign` for real TTY commits
- Stale-registration match semantics, export push classification, mapping file contracts
- `xai-grok-shell` / pager
- Test assertions

---

## 5-line summary

1. Host `commit.gpgsign` + no pinentry TTY broke fixture and export commits.
2. Product `export_github::git()` now masks global/system git config.
3. Worktree + auto_gc plants now use hermetic `xai_test_utils` git.
4. All listed filters green (55 + 7 + 15).
5. Shared isolation fix, not 22 expect rewrites.
