# Process mop: Business-before-personal rank + sibling included-before-extras hop

Isolated cargo dirs: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-mop-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-shell -p xai-grok-pager` | **0** |
| clippy shell | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | **0** |
| clippy pager | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | **0** |
| tests | `cargo test -p xai-grok-shell --lib --` (11 named rank/hop filters, `--test-threads=1`) | **0** |

## Tests (11 passed, 0 failed)

- `auth::allowance_exhaust_from_billing::tests::afterburner_does_not_skip_mark_when_sibling_has_included_remaining`
- `auth::supergrok_identity_rank::tests::pick_prefers_business_included_before_personal_when_both_have_remaining`
- `auth::supergrok_identity_rank::tests::combined_included_remaining_does_not_double_count_unified_pool`
- `auth::supergrok_identity_rank::tests::combined_included_remaining_sums_distinct_personal_and_business_pools`
- `auth::supergrok_identity_rank::tests::order_credentials_personal_full_with_extras_hops_to_business_included_before_extras`
- `auth::supergrok_identity_rank::tests::order_credentials_business_included_before_personal_when_both_have_room`
- `agent::config::tests::sampling_config_auto_use_extras_keep_session_console_failover`
- `agent::config::tests::sampling_config_hops_to_sibling_included_before_extras`
- `auth::manager::tests::align_after_billing_switches_sticky_personal_full_to_business_included`
- `auth::allowance_exhaust_from_billing::tests::apply_billing_marks_personal_full_when_business_sibling_has_included`
- `session::acp_session::sampler_turn::ranked_auto_turn_tests::prepare_sampler_for_turn_aligns_to_ranked_included_primary`

`test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 6576 filtered out.`

## Edits

**None.** fmt did not rewrite files. clippy and tests were already green. No mop fallout in rank/hop files.

## Slice D collision

No collision and no edits there.

- `limits_snapshot_hub.rs` was not on disk.
- `limits_cmd.rs` was not on disk.
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs` existed (mtime 2026-08-14 13:20) and was left untouched.

Rank/hop files of interest were present and unchanged:

- `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`
- `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`
