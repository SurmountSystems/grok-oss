# Process mop: unread config restore

Process mop only. No product edits. No new features. Did not touch `actions.rs`, `views/prompt_widget/**`, or welcome beyond the existing hide_header tests.

Implementer report: `.agents/reports/bug-config-unread-restore-2026-08-13.md`.

## Commands and exit codes

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager -p xai-grok-pager-render -p xai-grok-shell` | 0 |
| `cargo fmt -p … -- --check` (after tests) | 0 |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager-render --lib -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-shell --lib -- -D warnings` (first try) | 101 (`No space left on device` writing rmeta under `/tmp`) |
| Same shell clippy after deleting this mop’s incremental cache | 0 |
| `cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub` (first try, `/tmp`) | 101 (`No space left on device`) |
| Same pager-render filter after moving the target onto home disk | 0 (1 passed, 1117 filtered) |
| `cargo test -p xai-grok-shell --lib -- allow_worktree` | 0 (5 passed, 6565 filtered) |
| `cargo test -p xai-grok-pager --lib -- hide_header always_expand doge plan_approval_soft_park plan_approval_modal_park bubble_copy` | 0 (12 passed, 8813 filtered) |
| `cargo test -p xai-grok-pager --test settings_e2e -- hide_header always_expand scrub allow_worktree bubble plan_approval` | 0 (14 passed, 308 filtered) |

`--all-targets` clippy was not run. Implementer already noted pre-existing red on doctor early dispatch, benches, and settings_e2e harness. Named tests did not need that path.

## Target dir

Preferred `CARGO_TARGET_DIR=/tmp/grok-unread-cfg-mop-target` for fmt, clippy, and the first test wave.

`/tmp` (45G tmpfs) filled while other agent target dirs were also there. First shell clippy and first pager-render test compile died on ENOSPC, not on product warnings or failed asserts. After that, this mop’s incremental cache was deleted, then the whole tree was moved to `/home/hunter/.cache/grok-unread-cfg-mop-target` so remaining tests could finish. That is the same artifact tree, not a second product compile farm.

`CARGO_INCREMENTAL=0` on the retry waves so `/tmp` would not refill.

## Targeted tests (all green)

**xai-grok-pager-render** (1 passed):

- `appearance::cache::tests::prime_applies_scrub_ascii_punct_from_ui`

**xai-grok-shell** (5 passed):

- `config::tests::apply_allow_worktree_policy_false_forces_none`
- `config::tests::subagents_config_allow_worktree_defaults_false`
- `config::tests::resolve_subagents_copies_allow_worktree`
- `config::tests::subagents_config_allow_worktree_false_via_resolve`
- `config::tests::subagents_config_allow_worktree_true_via_resolve`

**xai-grok-pager lib** (12 passed):

- `hide_header_zeros_welcome_top_bar_height`
- `hide_header_zeroes_header_and_header_gap`
- `hide_header_zeroes_status_bar_height`
- `theme_choices_include_doge_and_default_is_doge`
- `bubble_copy_buttons_on_paints_copy_icon`
- `bubble_copy_buttons_off_omits_copy_icon`
- `plan_approval_modal_park_is_fullscreen`
- `plan_approval_soft_park_is_not_fullscreen`
- `always_expand_thinking_keeps_blocks_expanded`
- `always_expand_thinking_hides_ctrl_e_hint`
- plus two DOGE rail accent tests that match the `doge` filter (`agent_message_block_accent_is_magenta_rail_under_doge_while_running`, `user_prompt_block_accent_is_green_rail_under_doge_default`)

**settings_e2e** (14 passed):

- hide_header space + mouse
- always_expand_thinking space + mouse
- scrub_ascii_punct space + mouse
- allow_worktree space + mouse
- bubble_copy_buttons space + mouse
- plan_approval_park choices, picker enter, one-click open, nav without preview

## Fallout

None. Format, `--lib` clippy, and named tests are green. No files changed by this mop.
