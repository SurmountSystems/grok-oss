# Process mop — rebuild + theme packages

Packages: `xai-grok-update`, `xai-grok-pager`, `xai-grok-pager-render`.

CI clippy in this repo is `cargo clippy --workspace --lib --bins -- -D warnings` (`just test-clippy`). `--all-targets` is extra and includes integration tests and benches.

## Commands and exit codes

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-update -p xai-grok-pager -p xai-grok-pager-render` | 0 |
| `cargo clippy -p xai-grok-update --all-targets -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | 101 |
| `cargo clippy -p xai-grok-pager-render --all-targets -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-update --lib --bins -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager --lib --bins -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager-render --lib --bins -- -D warnings` | 0 |
| `cargo test -p xai-grok-update --lib --offline rebuild::` | 0 (27 passed) |
| `cargo test -p xai-grok-pager --lib --offline dispatch::rebuild` | 0 (17 passed) |
| `cargo test -p xai-grok-pager --lib --offline finish_run_carries_rebuild_relaunch_when_armed` | 0 |
| `cargo test -p xai-grok-pager --lib --offline sigusr1_sets_peer_rebuild_flag_once` | 0 |
| `cargo test -p xai-grok-pager --lib --offline -- user_prompt_block_accent user_prompt_prefix_matches agent_message_block_accent recap_accent info_line_model_name_uses_accent_model` | 0 (8 passed) |
| `cargo test -p xai-grok-pager --lib --offline -- render_block_agent_message_accent_color` | 0 |
| `cargo test -p xai-grok-pager-render --lib --offline -- doge_tmtheme_object_property_rules_share_one_foreground doge_markdown_same_role_spans_share_one_token doge_accent_user_is_pure_green doge_accent_model_is_pure_magenta default_theme_is_doge` | 0 (5 passed) |

## What I mopped

Nothing. Fmt was already clean. Clippy `--lib --bins` (the CI gate) is clean on all three packages. Targeted rebuild and theme tests are green.

## Leftovers

`xai-grok-pager` `--all-targets` clippy is red on files that are not rebuild or theme product code. I did not edit those. They are not in the CI `--lib --bins` gate.

- `tests/doctor_early_dispatch.rs:320` — `Path::canonicalize` (`clippy::disallowed_methods`)
- `src/diagnostics/fix_tests.rs:333` — same `canonicalize` lint
- `tests/settings_e2e.rs:1715` — `saturating_sub(1).max(0)` (`clippy::unnecessary_min_or_max`)
- `benches/edit_highlight.rs:152` — `clippy::needless_range_loop`
- `src/app/session_startup.rs:1663` — `assert_eq!(…, false)` in a unit-test helper (`clippy::bool_assert_comparison`; only under `--all-targets` / `--tests`)
