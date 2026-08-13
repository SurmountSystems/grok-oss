# PR #36 `just ci` fail on `1faa8576`

Date: 2026-08-13. Branch: `onto-xai/b13fa526f511`. Shared cwd. No worktree. No GitHub write.

## Failed run

- Check: `just ci` (job `quality` / display name `just ci`)
- Run: https://github.com/SurmountSystems/grok-oss/actions/runs/31663578658
- Job: https://github.com/SurmountSystems/grok-oss/actions/runs/31663578658/job/94333343797
- Head: `1faa857666eff94e3dcf8481b5f915ea72133337`
- Conclusion: **failure**
- Duration: about 36 minutes (03:19:37Z to 03:55:15Z). Step `just ci-prep && just test` failed. Fmt and clippy had already passed. Stopped compiling unit tests.

Noise that was **not** the fail: Node 20 deprecation, Nix cache 400.

## Failing step + first real error

Step: `test-unit` = `cargo nextest run --workspace --locked` (exit 101 via `just cargo-ci` / `cargo test --no-run --workspace --jobs 2 --locked`).

First product error:

```
error[E0432]: unresolved import `pretty_assertions`
  --> crates/codegen/xai-grok-shell/tests/test_session_load_memory.rs:15:5
```

Then the same import at line 399. Compile ended with: `could not compile xai-grok-shell (test "test_session_load_memory") due to 2 previous errors`.

No nextest runtime fails. The pager-minimal overlay cursor test was **not** on this CI fail list. Left alone.

## Root cause

`test_session_load_memory` imports `pretty_assertions::assert_eq`. Other crates already declare that workspace crate as a dev-dependency. `xai-grok-shell` did not, so the integration test could not compile.

## Local reproduce

```
cargo test -p xai-grok-shell --test test_session_load_memory --features test-support --locked --no-run
# before: exit 101, 2 compile errors (unresolved pretty_assertions)
# after:  Finished test profile; executable built (rustc 1.97.1 / 8bab26f4f)

cargo test -p xai-grok-shell --test test_session_load_memory --features test-support --locked
# 1 passed (prepare_replay_lines_borrows_the_transcript); 2 ignored heavy RSS soaks

cargo fmt -p xai-grok-shell
# exit 0

cargo clippy -p xai-grok-shell --lib --bins --locked -- -D warnings
# exit 0
```

## TDD

Not applied as a behavior contract. This is a missing test-only crate link, not a product assert change. Observed red was the same compile error as CI. Green is the same filter compiling and the one non-ignored test passing. Asserts were not weakened.

## Files changed

- `crates/codegen/xai-grok-shell/Cargo.toml`: `pretty_assertions = { workspace = true }` under `[dev-dependencies]`
- `Cargo.lock`: record the new `xai-grok-shell` → `pretty_assertions` edge (crate already in the lock from other packages)

Nucleo contracts were not touched. rustc / rustfmt stay **1.97.1**.

## New tip + push

| Item | Value |
|------|--------|
| New tip | `6875dc05b20b5f7fb7c2938c5ff6e501268454f1` |
| Tree | `b45557a3a3fa61a98e37b31395df33b7fc2034a3` |
| Parent | `1faa857666eff94e3dcf8481b5f915ea72133337` |
| Commit path | `git add` the two files, `git write-tree`, `git commit-tree`, `git update-ref HEAD`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`. |
| Push | `git push origin onto-xai/b13fa526f511` ff: `1faa8576..6875dc05`. No force. No new branch. No new PR. |

## What this did not do

- No GitHub writes (`gh pr comment` / `edit` / `create` / reviews / issues).
- Did not implement five-CTA mouse buttons, dual-auth spend-order, or PTY flake.
- Did not fix the pre-existing pager-minimal overlay cursor test (not on this CI fail list).
- Did not run full `just ci` (hours). Named step's compile contract is green locally.
- Did not delete untracked `.agents/reports/*`.

## SuperGrok copy

No user-facing billing copy changed. SuperGrok stays a paid product. Say included SuperGrok period limits, never "free SuperGrok".
