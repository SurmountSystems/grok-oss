# PR #36 agent encrypted prompt templates

Date: 2026-08-13
Branch: `onto-xai/b13fa526f511`
Worktree: `/home/hunter/Projects/surmount/grok-build`
rustc: 1.97.1 (8bab26f4f 2026-07-14)

## Problem

CI run 31673700687 on SHA `a036327e` failed 86 nextest tests. The first hole
in `xai-grok-agent` was stale XOR bytes in
`crates/codegen/xai-grok-agent/src/prompt/prompt_encrypted.rs`.

`templates/prompt.md` already had the Surmount `<planning>` block and
`${{ tools.by_kind.plan }}`. The encrypted copy did not. Decrypt therefore
produced an older prompt with neither `todo_write` nor `<planning>`, so:

1. `prompt::template::tests::test_base_template_contains_resolved_tool_names`
   panicked: default renderer includes Plan tool; prompt should teach
   `todo_write`
2. `prompt::template::tests::test_base_template_plan_present_includes_planning`
3. `prompt::template::tests::test_encrypted_templates_not_stale`
   (`prompt.md encrypted bytes are stale — run scripts/encrypt_templates.py`)

Tests were left as spec. No asserts were loosened.

Scope: **only** `xai-grok-agent` encrypted prompt templates. Did not touch
`xai-grok-shell`, `xai-grok-pager`, `xai-grok-sampler`, or
`xai-grok-pager-minimal` (local nextest mop owns those).

## Commands and exits

| Step | Command | Exit |
|------|---------|------|
| Confirm HEAD | `git rev-parse HEAD` → `a036327e6151398f7c46b79948256b24b2ae1832` on `onto-xai/b13fa526f511` | 0 |
| Locate script | Repo-root `scripts/encrypt_templates.py` is absent. Existing script is `crates/codegen/xai-grok-agent/scripts/encrypt_templates.py` (the command the crate docs and tests name: `python3 scripts/encrypt_templates.py` from the crate dir). | n/a |
| Before hash | `sha256sum src/prompt/prompt_encrypted.rs` → `9d06822af0cd4c128e9dbffe58b3a13ff4a15e5aac51c89cbb8a92a4273a1780` (155122 bytes) | 0 |
| Encrypt | `cd crates/codegen/xai-grok-agent && python3 scripts/encrypt_templates.py` | 0 |
| After hash | `d68f9690915dff06a1d93b0f51e27b734b0a99e2fa49b2d51907a12dc312afa0` (160540 bytes). Git: 1 file, 1 insertion / 1 deletion (single-line byte arrays). | 0 |
| Tests | `cargo nextest run -p xai-grok-agent prompt::template::tests::test_base_template_contains_resolved_tool_names prompt::template::tests::test_base_template_plan_present_includes_planning prompt::template::tests::test_encrypted_templates_not_stale` | 0 (3 passed, 578 skipped) |
| fmt | `cargo fmt -p xai-grok-agent` | 0 |
| clippy | `cargo clippy -p xai-grok-agent --lib --bins --locked -- -D warnings` | 0 |
| Stage | `git add -- crates/codegen/xai-grok-agent/src/prompt/prompt_encrypted.rs` (only this path) | 0 |
| Tree | `git write-tree` → `8ee755f4586244ae743d6d40ef12f236b516e7d6` | 0 |
| Commit | `git commit-tree` parent `a036327e` → `48f0bf1a6307d25cb30561295de6e89aa37d59c5` | 0 |
| Move HEAD | `git update-ref HEAD 48f0bf1a6307d25cb30561295de6e89aa37d59c5` | 0 |
| Fetch | `git fetch origin onto-xai/b13fa526f511` (origin still `a036327e`, ancestor of new HEAD) | 0 |
| Push attempt 1 | `git push --ff-only origin HEAD:onto-xai/b13fa526f511` | 129 (this git has no `push --ff-only`) |
| Push | `git push origin HEAD:onto-xai/b13fa526f511` (no force; origin still ancestor) | 0 (`a036327e..48f0bf1a`) |

Did not run `git commit`, `commit.gpgsign=false`, `--no-gpg-sign`, fake
`gpg.program`, hook disables, or force-push. No GitHub issue/PR writes. No
second PR.

## SHA

Pushed: **`48f0bf1a6307d25cb30561295de6e89aa37d59c5`**
(`ci: regenerate xai-grok-agent encrypted prompt templates`)

Commit tree contains only:

- `crates/codegen/xai-grok-agent/src/prompt/prompt_encrypted.rs`

`templates/prompt.md`, `apply_patch_prompt.md`, and `subagent_prompt.md` were
already current at `a036327e`. Encrypt rewrote all three arrays so the
stale-bytes test matches the sources.

`origin/onto-xai/b13fa526f511` after push: `48f0bf1a` (matches local HEAD).

## Leftover

- The other 83 nextest fails from run 31673700687 are **not** this hole.
  Local mop is already editing shell / pager / sampler / pager-minimal.
  Those working-tree edits were left unstaged and were not part of this
  commit.
- Dirty (other mop / reports), not this change:
  pager, pager-minimal, pager-render, sampler, shell sources; existing
  `.agents/reports/bug-pr36-*` and recon reports.
- This report file is untracked on purpose (not in the template commit).
- `git push --ff-only` is unsupported on this git; the ancestor check plus
  a non-force `git push` was the fast-forward guard.

## Contract check

When the default renderer includes the Plan tool (`todo_write`), the
decrypted base prompt now contains `todo_write` and `<planning>`. Encrypted
bytes match current `templates/prompt.md` (and apply-patch / subagent
siblings). The three named tests were not loosened and are green.
