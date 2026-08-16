# User-guide leftover operator commands use grok-oss

**Date:** 2026-08-14  
**Tree:** `/home/hunter/Projects/surmount/grok-build`  
**Board:** `bug:user-guide-grok-command-leftovers`

Surmount's operator command is `grok-oss`. The branding slice already owned `--resume`, `--version`, `--yolo`, and `--continue`. Leftover pages still told operators to run bare `grok sessions`, `grok login`, `grok mcp add`, and similar CLI examples. Official xAI `grok` product mentions and `~/.grok` paths were left alone.

## TDD

Named contract added in `crates/codegen/xai-grok-pager/src/docs.rs`:

`docs::tests::user_guide_operator_cli_examples_use_grok_oss`

It forbids leftover operator stems (`grok sessions`, `grok login`, `grok logout`, `grok mcp`, `grok inspect`, `grok doctor`, `grok plugin`, `grok memory`, `grok dashboard`, `grok wrap`, `grok agent`, `grok models`, `grok workspace`, `grok worktree`, `grok setup`, `grok du`, `grok disk-usage`, `grok -p`, `grok -w`, `grok --`) in every embedded `USER_GUIDE` page. It also requires `grok-oss sessions` on `17-sessions.md`, `grok-oss login` on `02-authentication.md`, and `grok-oss mcp add` on `07-mcp-servers.md`.

### Red (before product markdown edits)

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ug-cmd-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-pager --lib user_guide_operator_cli_examples_use_grok_oss --offline
```

**Exit code: 101**

```
test docs::tests::user_guide_operator_cli_examples_use_grok_oss ... FAILED

02-authentication.md must not tell operators to run `grok login`; use grok-oss for this tree
```

## Files changed this slice

- `crates/codegen/xai-grok-pager/src/docs.rs` (named contract)
- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/06-theming.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/07-mcp-servers.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/09-plugins.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/10-hooks.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/12-project-rules.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/13-memory.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/14-headless-mode.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/15-agent-mode.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/18-sandbox.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/21-terminal-support.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/22-permissions-and-safety.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/23-dashboard.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/24-monitoring-usage.md`

Operator command examples only. Did not rename official xAI `grok` (01 still says the SpaceXAI install script installs upstream `grok`). Did not rewrite `~/.grok` paths, model ids, or the window-title slot named `grok`.

## Green (same test)

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ug-cmd-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib user_guide --offline
```

**Exit code: 0**

```
test docs::tests::user_guide_entries_are_valid ... ok
test docs::tests::user_guide_entries_have_no_duplicates ... ok
test docs::tests::default_howto_entries_includes_all_user_guide_docs ... ok
test docs::tests::user_guide_does_not_claim_automatic_host_hop_is_unshipped ... ok
test docs::tests::user_guide_resume_and_version_examples_use_grok_oss ... ok
test docs::tests::user_guide_operator_cli_examples_use_grok_oss ... ok

test result: ok. 6 passed; 0 failed
```

## Honest leftovers

`~/.grok/docs/user-guide/` is the extract target (`extract_user_guide_docs`). That host copy stays stale until the next product launch that writes the embedded guide to disk. Do not treat the host extract as a second source of truth.

Did not edit `FORK.md` or `AGENTS.md`. Did not rebuild. Did not `git add` or commit.

Stop.
