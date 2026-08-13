# CI cluster fix: fast-worktree + export_github (hermetic git / host GPG)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Cluster note:** [`.agents/reports/bug-ci-239-test-cluster-2026-08-11.md`](bug-ci-239-test-cluster-2026-08-11.md) (scope 5: git worktree / export)

---

## Status

**Green.** Both clusters fixed by one shared root cause: host global git config (`commit.gpgsign=true` + pinentry needing `/dev/tty`) poisoned fixture and automated-export commits.

| Package | Before | After |
|---------|--------|-------|
| `xai-fast-worktree` (`git::worktree` + `auto_gc` plant) | 12 fails (CI cluster) | **7/7** worktree + **55/55** auto_gc green |
| `xai-grok-workspace` `export_github` | 10 fails (entire suite path) | **16/16** filter green |

---

## Observed red (sample)

### `xai-fast-worktree` `git::worktree::tests::*`

Fixture `init_repo_with_worktrees` ran raw `git commit` without masking global config:

```text
git ["commit", "-m", "init"] failed: error: gpg failed to sign the data:
...
gpg: cannot open '/dev/tty': No such device or address
fatal: failed to write commit object
```

Same pattern in `auto_gc::tests` `plant_stale_git_worktree` (raw `Command::new("git")` + commit).

### `xai-grok-workspace` `export_github`

Product `git()` set author env vars but still inherited host `commit.gpgsign` / hooks. Automated export commits then failed under CI/agent (no pinentry TTY). Non-commit tests (e.g. `missing_repo_and_mapping_is_a_typed_error`) already passed.

---

## Root cause

**One systemic issue, not 22 independent bugs:**

1. Host `~/.gitconfig` has `commit.gpgsign=true` (operator policy).
2. Fixture helpers and automated export commits did not isolate from global/system git config.
3. Under nextest / agent spawn, GPG cannot open `/dev/tty` → every fixture or export commit fails instantly.

Already-correct pattern in-tree: `xai_test_utils::git::{init_git_repo, run_git}` (`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, local `commit.gpgsign=false`). Sibling modules (`sync.rs`, `api.rs` tests) already used it; worktree registration and auto_gc plant did not.

---

## Fix (product + hermetic harness)

### 1. Product: `export_github` automated git path

**File:** `crates/codegen/xai-grok-workspace/src/export_github.rs`
**Change:** In product `git()`, mask host global/system config (same envs as other automated git paths). Local `.git/config` still applies. Author/committer remain forced to export identity.

**Named contract:** Automated GitHub export commits must succeed without interactive GPG/pinentry or host hooks. Not a test-only rewrite of expectations.

### 2. Test harness: worktree registration fixtures

**File:** `crates/codegen/xai-fast-worktree/src/git/worktree.rs`
**Change:** Replace raw `run_git` + manual `user.name` setup with `xai_test_utils::git::{init_git_repo, git_commit_all, run_git}` + `require_git!()`.

### 3. Test harness: auto_gc plant helper

**File:** `crates/codegen/xai-fast-worktree/src/auto_gc.rs`
**Change:** `plant_stale_git_worktree` uses the same hermetic helpers (masks host gpgsign/hooks).

**No test expectation rewrites.** Product behavior for stale registration scrub and export flow unchanged; only commit isolation for automated/fixture git.

---

## Verify commands

```bash
nice -n 19 ionice -c3 cargo test -p xai-fast-worktree --lib 'git::worktree::tests' -- --test-threads=1
# 7 passed

nice -n 19 ionice -c3 cargo test -p xai-fast-worktree --lib --features metadata auto_gc -- --test-threads=2
# 55 passed

nice -n 19 ionice -c3 cargo test -p xai-grok-workspace --lib export_github -- --test-threads=2
# 16 passed
```

**fmt:** `cargo fmt -p xai-fast-worktree -p xai-grok-workspace`
**clippy:** lib clean for touched packages. Pre-existing `clippy::disallowed_methods` on `xai-fast-worktree` test-only `Command::spawn` in `api.rs:3638` (unrelated; not introduced here).

---

## Not changed

- Operator global `commit.gpgsign` policy for real repos (human TTY commits).
- Stale-registration match semantics, export push classification, or mapping file contracts.
- Other CI clusters (pager, signed-policy, external auth) from the 239-fail report.

---

## 5-line summary

1. Host `commit.gpgsign` + no pinentry TTY broke fixture/export commits.
2. Fixed export product `git()` to mask global/system config.
3. Fixed worktree + auto_gc plants to use hermetic `xai_test_utils` git.
4. All cluster filters green (7 + 55 + 16).
5. Shared harness fix, not 22 expect rewrites.
