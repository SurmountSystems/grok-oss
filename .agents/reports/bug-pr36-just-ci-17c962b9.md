# PR #36 `just ci` fail on `17c962b9`

Date: 2026-08-13. Branch: `onto-xai/b13fa526f511`. Shared cwd. No worktree. No GitHub write.

## Failed run

- Check: `just ci` (job `quality` / display name `just ci`)
- Run: https://github.com/SurmountSystems/grok-oss/actions/runs/31661094816
- Job: https://github.com/SurmountSystems/grok-oss/actions/runs/31661094816/job/94325898933
- Head: `17c962b96f37aaae7c5a8cde2eabaedc1237515a`
- Conclusion: **failure**
- Duration: about 33 minutes (02:31:44Z to 03:04:41Z). `test-fmt` and `test-clippy` passed. Stopped compiling unit tests.

Noise that was **not** the fail: Node 20 deprecation, Nix cache 400.

## Failing step + first real error

Step: `test-unit` = `cargo nextest run --workspace --locked` (exit 101 via `just cargo-ci`).

First product error:

```
error[E0599]: no method named `is_overloaded` found for enum `error::SamplingError`
   --> crates/codegen/xai-grok-sampling-types/src/error.rs:911:14
```

Then more `is_overloaded` calls, then `missing field error_code` on leftover `SamplingError::Api` test fixtures. Compile ended with: `could not compile xai-grok-sampling-types (lib test) due to 25 previous errors`.

The earlier note about `overlay::tests::question_input_mode_editor_grows_and_keeps_row_prefix` was **not** on this CI fail list. Left alone.

## Root cause

Restack left unit tests that name `SamplingError::is_overloaded` and `is_retry_vetoed` contracts, but the methods were missing or incomplete. Some older `Api` fixtures also omitted the later `error_code` field, so the test crate could not compile.

## Local reproduce

```
cargo test -p xai-grok-sampling-types --lib --locked
# before: exit 101, 25 compile errors (is_overloaded missing, error_code missing)
# after:  316 passed; 0 failed (rustc 1.97.1 / 8bab26f4f)

cargo clippy -p xai-grok-sampling-types --lib --bins --locked -- -D warnings
# exit 0
```

## TDD

Yes.

- Named contract: `overloaded_detects_stream_and_api_shapes` and `overloaded_message_matches_backend_variants` (plus `retry_veto_covers_header_and_context_length`).
- Red: same `cargo test -p xai-grok-sampling-types --lib --locked` compile-failed on `is_overloaded` before any product edit.
- Green: implemented `is_overloaded` and expanded `is_retry_vetoed` to the test spec. Same tests pass. Asserts were not weakened. Filling `error_code: None` on fixtures is compile alignment, not a softer assert.

## Files changed

- `crates/codegen/xai-grok-sampling-types/src/error.rs`
  - `SamplingError::is_overloaded`
  - `is_retry_vetoed` also honors `x-should-retry: false` and context-length overflow
  - leftover test `Api` structs get `error_code: None`

Nucleo contracts were not touched. rustc / rustfmt stay **1.97.1**.

## New tip + push

| Item | Value |
|------|--------|
| New tip | `1faa857666eff94e3dcf8481b5f915ea72133337` |
| Tree | `9e3baf1faec02028bc9558210709099b62c00473` |
| Parent | `17c962b96f37aaae7c5a8cde2eabaedc1237515a` |
| Commit path | `git add` the sampling-types file, `git write-tree`, `git commit-tree`, `git update-ref HEAD`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`. |
| Push | `git push origin onto-xai/b13fa526f511` ff: `17c962b9..1faa8576`. No force. No new branch. No new PR. |

## What this did not do

- No GitHub writes (`gh pr comment` / `edit` / `create` / reviews / issues).
- Did not implement five-CTA mouse buttons, dual-auth spend-order, or PTY flake.
- Did not fix the pre-existing pager-minimal overlay cursor test (not on this CI fail list).
- Did not run full `just ci` (hours). Named step's compile contract is green locally.
- Did not delete untracked `.agents/reports/*`.

## SuperGrok copy

No user-facing billing copy changed. SuperGrok stays a paid product. Say included SuperGrok period limits, never "free SuperGrok".
