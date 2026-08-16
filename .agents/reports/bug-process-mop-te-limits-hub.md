# Process mop: flock limits snapshot hub (Slice D)

Isolated cargo dirs: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-hub-mop-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`. rustc 1.97.1.

Clippy used `--lib` as specified (not `--all-targets`).

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-shell -p xai-grok-pager` | **0** |
| clippy shell | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | **0** |
| clippy pager | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | **0** |
| tests shell (first try) | same `cargo test -p xai-grok-shell --lib --` as below | killed at 120s while still compiling |
| tests shell (rerun) | `cargo test -p xai-grok-shell --lib --` six named filters, `-- --test-threads=1` | **0** |
| tests pager | `cargo test -p xai-grok-pager --lib -- limits_cmd:: -- --test-threads=1` | **0** |

## Shell tests (6 passed, 0 failed)

- `auth::supergrok_identity_rank::tests::pick_prefers_business_included_before_personal_when_both_have_remaining`
- `auth::supergrok_identity_rank::tests::order_credentials_personal_full_with_extras_hops_to_business_included_before_extras`
- `extensions::billing::tests::billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http`
- `auth::limits_snapshot_hub::tests::limits_snapshot_never_writes_access_tokens`
- `auth::limits_snapshot_hub::tests::limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once`
- `auth::limits_snapshot_hub::tests::limits_snapshot_second_process_reads_file_and_does_not_http`

`test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 6585 filtered out; finished in 0.01s`

## Pager tests (`limits_cmd::`)

`test result: ok. 42 passed; 0 failed; 1 ignored; 0 measured; 8835 filtered out; finished in 0.07s`

Ignored: `limits_cmd::tests::live_check_limits_first_from_env_json` (live: set `LIMITS_FIRST_JSON` to `limits --json` output path).

## Edits

**None.** fmt did not rewrite files. clippy and tests were already green. No mop fallout in hub, billing, limits_cmd, or xai_management.

No writer collision on those files. Left `docs/user-guide/` alone. Did not change spend-order. Did not `git add`, commit, or push.

Files of interest left untouched:

- `crates/codegen/xai-grok-shell/src/auth/limits_snapshot_hub.rs`
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs`
- `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
