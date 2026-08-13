# PR36 land report (onto-xai/b13fa526f511)

Process-mop lander. Isolation none. Reused branch only.

## fmt / clippy / tests

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-shell -p xai-grok-pager -p xai-grok-pager-bin -p xai-grok-pager-pty-harness -p xai-grok-sampler` | 0 |
| `cargo clippy -p xai-grok-shell -p xai-grok-pager -p xai-grok-pager-bin -p xai-grok-pager-pty-harness -p xai-grok-sampler --lib --bins --locked -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-shell --test team_managed_config --locked -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager-bin --test update_never_blocked_by_config --locked -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-sampler --all-targets --locked -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager-pty-harness --all-targets --locked -- -D warnings` | 0 |
| `cargo test -p xai-grok-sampler --lib --locked cloudflare_edge_range_is_transient` | 0 |
| `cargo test -p xai-grok-sampler --lib --locked classify_cloudflare_525_is_fatal` | 0 |
| `cargo test -p xai-grok-shell --test team_managed_config --locked` (50/50) | 0 |
| shell lib: `resolve_credentials_openrouter_does_not_use_xai_session` | 0 |
| shell lib: `resolve_model_override_api_key_pin_keeps_console_primary` | 0 |
| shell lib: `authenticated_401s_still_exhaust_after_three_retries` | 0 |
| shell lib: `queue_send_now_never_cancels_uncommitted_front` | 0 |
| shell lib: `parse_list_req_forces_kind_under_process_chat_mode_only` | 0 |
| pager lib: `fake_standalone_facts_compose_through_shared_view` | 0 |
| pager lib: `background_block_gutter_uses_block_background_fill` | 0 |
| pager lib: `render_row_needs_input_yellow_blink_no_badge_pending_prefix` | 0 |
| pager lib: `title_renders_on_top_border_with_corners_intact` | 0 |
| `cargo test -p xai-grok-pager-bin --test update_never_blocked_by_config --locked` (`corrupt_config_never_changes_update_outcome`) | 0 |
| `cargo test -p xai-grok-pager-pty-harness --test plan_approval_resume --locked plan_approval_restored_after_resume` | 0 |

`cargo clippy … --all-targets` across shell+pager together exits 101 on **pre-existing** test-only lints in files this land did not touch (`cancel_running_task_tests`, `assistant_ascii_scrub`, `doctor_early_dispatch`, `settings_e2e`, `xai_management`, etc.). Not mopped. Required `--lib --bins` is green.

rustc / rustfmt: 1.97.1.

## 525 / 526 test change

`cloudflare_edge_range_is_transient` no longer expects `RetryWithClientRebuild` for HTTP 525 and 526.

- Transient loop kept: 520–524, 527, 530.
- 525 and 526 now assert `RetryDecision::Fatal`.
- Product retry policy unchanged: `RetryPolicy::edge_client` still marks 525/526 terminal. Did not make product retry 525.
- Sibling SoT `classify_cloudflare_525_is_fatal_even_with_should_retry_true` still green.
- `is_transient_api_status` still lists 520..=527\|530; `classify_error` uses `is_retryable`, which does not retry 525/526.

## Git

- Branch: `onto-xai/b13fa526f511`
- Old HEAD (pre-land): `2174fd75db9a814efbb704b0ae7cf0f7e9326073`
- New HEAD: `71bca1a0c8b0ab3a7ef9eedcdf2a13ab5bd3c527`
- Tree: `219d95ee7484db5b07a4d044053669afc160accc`
- Parent: `2174fd75db9a814efbb704b0ae7cf0f7e9326073`
- `ALLOW_UNSIGNED_COMMIT=1 git commit` failed: GPG `NEED_PASSPHRASE`, no `/dev/tty` (exit 128). Did not disable signing.
- Landed via `git write-tree` + `git commit-tree` + `git update-ref HEAD` (recon exception; unsigned `N`).
- `git fetch origin`: remote still `2174fd75`. Old HEAD is ancestor of new HEAD.
- Push: `git push origin HEAD:onto-xai/b13fa526f511` exit 0, `2174fd75..71bca1a0`, no force.

Staged only listed product paths plus this report. Did not stage other `.agents/reports/*`. `Cargo.lock` was not dirty.

On-disk report after land has this Git section filled; the committed copy still says "filled after land below." Left unstaged on purpose. No second push.

## Residual honesty

None seen beyond the pre-existing `--all-targets` clippy noise on untouched test files.
