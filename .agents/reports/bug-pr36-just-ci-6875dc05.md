# PR #36 `just ci` fail on `6875dc05`

Date: 2026-08-13. Branch: `onto-xai/b13fa526f511`. Shared cwd. No worktree. No GitHub write.

## Failed run

- Check: `just ci` (job `quality` / display name `just ci`)
- Run: https://github.com/SurmountSystems/grok-oss/actions/runs/31666531041
- Job: https://github.com/SurmountSystems/grok-oss/actions/runs/31666531041/job/94342184127
- Head: `6875dc05b20b5f7fb7c2938c5ff6e501268454f1`
- Conclusion: **failure**
- Duration: about 30 minutes (04:17:05Z to 04:47:19Z). Step `just ci-prep && just test` failed. Fmt and clippy had already passed. Stopped compiling unit tests.

Noise that was **not** the fail: Node 20 deprecation, Nix cache 400.

## Failing step + first real error

Step: `test-unit` = `cargo nextest run --workspace --locked` (exit 101 via `just cargo-ci` / `cargo test --no-run --workspace --jobs 2 --locked`).

First product error:

```
error[E0603]: function `inject_url_derived_headers` is private
    --> crates/codegen/xai-grok-shell/tests/openrouter_attribution.rs:7:36
```

Compile ended with: `could not compile xai-grok-shell (test "openrouter_attribution") due to 1 previous error`.

No nextest runtime fails in this log. The pager-minimal overlay cursor test was **not** on this CI fail list. Left alone.

## Root cause

The integration test calls `inject_url_derived_headers` as a public crate API. Restack left the helper `pub(crate)` and the body only folded cli-chat-proxy headers. After making it public, the same test failed at runtime because OpenRouter attribution headers were never inserted.

## Local reproduce

```
cargo test -p xai-grok-shell --test openrouter_attribution --locked --no-run
# before: exit 101, E0603 private function

cargo test -p xai-grok-shell --test openrouter_attribution --locked
# after pub only: 1 passed, 1 failed
#   inject_url_derived_headers_sets_openrouter_attribution
#   left: None
#   right: Some("https://github.com/SurmountSystems/grok-oss")
# after header inject: 2 passed; 0 failed (rustc 1.97.1 / 8bab26f4f)

cargo fmt -p xai-grok-shell
# exit 0

cargo clippy -p xai-grok-shell --lib --bins --locked -- -D warnings
# exit 0
```

## TDD

Yes.

- Named contract: `inject_url_derived_headers_sets_openrouter_attribution` (integration test already in tree).
- Red: compile `E0603` first, then after `pub` the same test panicked with missing `HTTP-Referer`.
- Green: OpenRouter base URLs now fold referer, title, and category headers without overwriting existing entries. Same test passes. Asserts were not weakened.

## Files changed

- `crates/codegen/xai-grok-shell/src/agent/config.rs`
  - `inject_url_derived_headers` is `pub`
  - OpenRouter bases get `HTTP-Referer`, `X-OpenRouter-Title`, `X-Title`, `X-OpenRouter-Categories`

Nucleo contracts were not touched. rustc / rustfmt stay **1.97.1**.

## New tip + push

| Item | Value |
|------|--------|
| New tip | `82fa1794a8f1751045da6eb85b3e43d902972a69` |
| Tree | `76b348c9529b07d677a7a3d7654bb4647d5888e3` |
| Parent | `6875dc05b20b5f7fb7c2938c5ff6e501268454f1` |
| Commit path | `git add` the config file, `git write-tree`, `git commit-tree`, `git update-ref HEAD`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`. |
| Push | `git push origin onto-xai/b13fa526f511` ff: `6875dc05..82fa1794`. No force. No new branch. No new PR. |

## What this did not do

- No GitHub writes (`gh pr comment` / `edit` / `create` / reviews / issues).
- Did not implement five-CTA mouse buttons, dual-auth spend-order, or PTY flake.
- Did not mop the large `xai-grok-shell` lib-test compile fallout (not on this CI fail list).
- Did not fix the pre-existing pager-minimal overlay cursor test (not on this CI fail list).
- Did not run full `just ci` (hours). Named step's compile + integration contract is green locally.
- Did not delete untracked `.agents/reports/*`.

## SuperGrok copy

No user-facing billing copy changed. SuperGrok stays a paid product. Say included SuperGrok period limits, never "free SuperGrok".
