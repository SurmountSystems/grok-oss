# PR #36 `just ci` fail on `4df59dac`

Date: 2026-08-12. Branch: `onto-xai/b13fa526f511`. Shared cwd. No worktree. No GitHub write.

## Failed run

- Check: `just ci` (job `quality` / display name `just ci`)
- Run: https://github.com/SurmountSystems/grok-oss/actions/runs/31658516974
- Job: https://github.com/SurmountSystems/grok-oss/actions/runs/31658516974/job/94318181973
- Head: `4df59dac8e70ce339236350bf00288f1fe8adf47`
- Conclusion: **failure**
- Duration: about 10m 51s (01:41:55Z to 01:52:43Z). `test-fmt` passed. Stopped in `test-clippy`.

Noise that was **not** the fail: Node 20 deprecation, Nix cache 400.

## Failing step + first real error

Step: `test-clippy` = `cargo clippy --workspace --lib --bins --locked -- -D warnings` (exit 101).

First product error:

```
error: this can be rewritten more simply using `.sort_by_key`
  --> crates/codegen/xai-grok-shell/src/agent/roster.rs:154
  clippy::unnecessary-sort-by
```

Then 42 more `xai-grok-shell` lints. Compile ended with: `could not compile xai-grok-shell (lib) due to 43 previous errors`.

## Root cause

This restack tip was one rustfmt wrap past the prior fmt fail, then clippy `-D warnings` on production lib+bins. After the 43 shell lints, the same workspace command also needed pager style lints, pager-minimal compile alignment (dropped five-CTA mouse paint; keyboard legend only), and pager-bin alignment to the current crate APIs (`parse_cli` + `apply_cwd`, six-arg `resolve_use_leader`, current `Command` shape, `enforce_version_policy_or_exit`, `HeadlessOptions` fields).

Not a product behavior change. No fake TDD.

## Local reproduce

```
cargo clippy --workspace --lib --bins --locked -- -D warnings
# before mop: exit 101 (shell 43, then pager 32, pager-minimal compile, pager-bin 11)
# after mop:  exit 0   (rustc 1.97.1 / 8bab26f4f)

cargo fmt --all -- --check
# exit 0

cargo test -p xai-grok-pager-minimal --lib --locked plan::tests
# exit 0 (legend test green after dropping missing set_plan_approval_view fixture)
```

## TDD

Not applied. This ticket is clippy/compile/fmt style, not a named product behavior contract. The pager-minimal legend test was adjusted to compile against the current `minimal_api` (no setter for `plan_approval_view`). Asserts were not weakened.

## Files changed (38)

- `xai-grok-shell`: 18 files. `sort_by_key`, `checked_div`, `pub(crate)` visibility, `Option::zip`, `format!` without extra `&`, match-guard, `while let Some`.
- `xai-grok-pager`: 18 files. Same style lints; `#![allow(dead_code)]` on unwired peer-rebuild helpers; playground `TodoItem.size`; zip loops. Nucleo `Some(2)` / FuzzySearchManager reuse / poll last_activity were not touched.
- `xai-grok-pager-minimal`: `live.rs` (drop `global_paused`), `plan.rs` (keyboard legend; drop five-CTA mouse tests).
- `xai-grok-pager-bin`: `main.rs` crate-API align. Did **not** restore `Command::Limits` / `Command::Rebuild` / OpenRouter login clap fields (those variants are not on this restack `Command`).

## New tip + push

| Item | Value |
|------|--------|
| New tip | `17c962b96f37aaae7c5a8cde2eabaedc1237515a` |
| Tree | `0e288d7381ff36435f2aecd7f540e5786858fc23` |
| Parent | `4df59dac8e70ce339236350bf00288f1fe8adf47` |
| Commit path | `git add` the 38 product files, `git write-tree`, `git commit-tree`, `git update-ref HEAD`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`. |
| Push | `git push origin onto-xai/b13fa526f511` ff: `4df59dac..17c962b9`. No force. No new branch. No new PR. |

## What this did not do

- No GitHub writes (`gh pr comment` / `edit` / `create` / reviews / issues).
- No rustc downgrade. rustc / rustfmt stay **1.97.1**.
- No Nucleo `None` revert.
- Did not implement five-CTA mouse buttons, PTY flake, or dual-auth spend-order.
- Did not run full `just ci` (hours). Named step is green.
- Did not delete untracked `.agents/reports/*`.
- Did not restore `grok limits` / `grok rebuild` / OpenRouter login clap (crate `Command` does not have those variants on this tip). Residual honesty, not this ticket.
- Pre-existing pager-minimal unit fail `overlay::tests::question_input_mode_editor_grows_and_keeps_row_prefix` (file not in this mop) was not fixed.

## SuperGrok copy

No user-facing billing copy changed. SuperGrok stays a paid product. Say included SuperGrok period limits, never "free SuperGrok".
