# PR #36 disjoint small-crate nextest fails

Date: 2026-08-13
Branch: `onto-xai/b13fa526f511`
Worktree: `/home/hunter/Projects/surmount/grok-build`
rustc: 1.97.1 (8bab26f4f 2026-07-14)

## Scope

CI run [31673700687](https://github.com/SurmountSystems/grok-oss/actions/runs/31673700687)
on SHA `a036327e` failed 86 nextest tests. This slice owned only:

84. `xai-grok-tools computer::local::shell_state::tests::test_user_cmd_var_not_exported_under_allexport_bash`
85. `xai-grok-workspace session::git::restore_code_tests::ensure_binding_forks_conv_branch_off_base_and_is_idempotent`
86. `xai-ratatui-textarea textarea::tests::home_end_use_logical_line_when_soft_wrapped`

Did **not** touch `xai-grok-agent` templates, `xai-grok-shell` (including
`team_managed_config` / ABRT), `xai-grok-pager` / pager-bin / pager-minimal /
pager-pty-harness, or `xai-grok-sampler`. Tests stayed spec. No asserts
loosened.

## Red (observed)

| Test | Local / CI | Fail |
|------|------------|------|
| allexport bash | Local **pass** on `a036327e`. CI **red** (both tries). | `shell_state.rs:1239` `assert_eq!(code, 0)` on `set -a`: **left 126, right 0**. Not the later leak assert. |
| ensure_binding fork | Local **red** (both tries). CI same assert. | `git_restore_code_tests.rs:1425` `Some(main_sha) != res.head_sha`. Fresh fork committed a seeded `.gitignore`, so HEAD was not `base_ref`. |
| Home/End wrap | Local **red** (both tries). CI same assert. | `textarea_tests.rs:2197` Home on a width-4 wrap of `abcdefghij` at cursor 6 went to visual-row start **4**, spec wants logical-line start **0**. |

## Product fixes

### 1. `xai-grok-tools` dump after `set -a` (exit 126)

Named contract: after the model runs `set -a`, the wrapper temp
`__grok_user_cmd` must not be exported to child processes, and `set -a`
itself must succeed (exit 0) so the option is dumped and replayed.

CI 126 was the dump helper replacing the user exit code.
`dump_bash_state` starts `set -euo pipefail` (shell-global in bash). With
allexport on, `_emit_encoded` exported its `content` local (the full
`export -p` payload). Helper `exec` of `base64`/`tr`/`grep` in a large
CI/Nix environment then failed with 126; `set -e` skipped
`builtin exit $COMMAND_EXIT_CODE`.

Fix:

- Capture `shopt -po` (including allexport) **before** isolating the dump.
- `set +o allexport` for the rest of the dump so temps are not exported.
- Filter `__grok_user_cmd` out of `export -p`.
- Wrapper: briefly clear allexport around the temp assignment, restore it
  if the user had it, `declare +x` as backstop, `{dump_fn} >&4 || true`.

Same tests, no assert change.

### 2. `xai-grok-workspace` `ensure_binding` fork SHA

Named contract: a missing local/remote conv branch is forked off `base_ref`
and checked out. `created` is true. HEAD equals `base_ref`. A second call
is a no-op checkout (`created` false). The base is never written.

`ensure_binding` was committing a default `.gitignore` on a genuine fresh
fork, so `res.head_sha` was the seed commit, not `main`.

Fix: do not seed or commit on this path. Local-only `info/exclude` seeding
stays on `git_commit` via `seed_default_excludes`. Removed unused
`seed_default_gitignore`.

### 3. `xai-ratatui-textarea` Home/End on a soft-wrapped line

Named contract (the test):

- Bare Home/End stay on **this logical line** (not wrap-row, not buffer).
- Super+Left/Right stay on the **visual wrap row**.
- Ctrl+A/E stay logical and still chain across lines.
- Ctrl+Home/End stay document-level.

Adapter `input` was sending bare Home/End through
`move_cursor_to_beginning_of_line(false)` (visual row).

Fix: Home/End call `beginning_of_current_line` / `end_of_current_line`.
Super+Left/Right and Ctrl+A/E are unchanged.

## Commands + exits

| Step | Command | Exit |
|------|---------|------|
| HEAD at start | `onto-xai/b13fa526f511` @ `a036327e` | 0 |
| Red: allexport | `cargo nextest run -p xai-grok-tools --lib …test_user_cmd_var_not_exported_under_allexport_bash` | 0 local (CI 126) |
| Red: ensure_binding | `cargo nextest run -p xai-grok-workspace --lib …ensure_binding_forks_conv_branch_off_base_and_is_idempotent` | 100 |
| Red: Home/End | `cargo nextest run -p xai-ratatui-textarea --lib …home_end_use_logical_line_when_soft_wrapped` | 100 (left 4, right 0) |
| fmt | `cargo fmt -p xai-grok-tools -p xai-grok-workspace -p xai-ratatui-textarea` | 0 |
| Green: three named | same three nextest filters | 0 (1 passed each) |
| Neighbors | workspace both `ensure_binding_*`; tools `shell_state::tests::test_*` (14); textarea both `home_end_*` | 0 |
| clippy | `cargo clippy -p xai-grok-tools -p xai-grok-workspace -p xai-ratatui-textarea --lib --bins --locked -- -D warnings` | 0 |

## Files

- `crates/codegen/xai-grok-tools/src/computer/local/shell_state.rs`
- `crates/codegen/xai-grok-workspace/src/session/git.rs`
- `crates/codegen/xai-ratatui-textarea/src/textarea.rs`
- this report

## Git

| Item | Value |
|------|--------|
| Parent | `a10f9aa7fa74d4a47e74518fbeba648aef2a3205` |
| Tree | `71d99dc5b2c3456545a771e0ff9d688485c3ba5f` |
| New tip | `2174fd75db9a814efbb704b0ae7cf0f7e9326073` |
| Commit path | `git add` the four paths, `git write-tree`, `git commit-tree`, `git update-ref HEAD`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`. |
| Fetch | origin still `a10f9aa7` (ancestor of new HEAD) |
| Push | `git push origin HEAD:onto-xai/b13fa526f511` ff `a10f9aa7..2174fd75`. No force. No new branch. No new PR. |

No GitHub issue/PR writes. No `git commit`.

## SuperGrok copy

No user-facing billing copy changed. SuperGrok is a paid product. Say
included SuperGrok period limits, never "free SuperGrok".
