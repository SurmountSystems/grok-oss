# Slice D: one grok-oss process fetches SuperGrok limits

**Board:** `impl:te-limits-one-fetcher` under `feat:token-economy-all-plans-ipc`  
**Plan:** `.agents/plans/token-economy-all-plans-ipc.md` Slice D  
**Workspace:** `/home/hunter/Projects/surmount/grok-build`

One grok-oss process holds an exclusive flock and fetches SuperGrok billing plus Management prepaid / postpaid / series. Other live TUIs wait, read `$GROK_HOME/limits_snapshot.json`, and apply the same process maps `remember_supergrok_included_billing` already fills. No daemon. Rebuild SIGUSR1 is unused (that signal still means fleet relaunch). SuperGrok is paid. This snapshot carries included SuperGrok period used percent, reset, SuperGrok dollar extras cents, identity ids, and poll outcome class. Never JWTs or API keys.

## Red (observed before product)

Isolated env:

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-limits-hub-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-shell --lib -- \
  limits_snapshot_second_process_reads_file_and_does_not_http \
  limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once \
  limits_snapshot_never_writes_access_tokens \
  billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http
```

Stub hub always fetched and did not write a clean snapshot. Fail lines:

```
thread 'auth::limits_snapshot_hub::tests::limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once'
assertion `left == right` failed
  left: Some(10.0)
 right: Some(41.0)

thread 'auth::limits_snapshot_hub::tests::limits_snapshot_never_writes_access_tokens'
leader must write limits_snapshot.json: Os { code: 2, kind: NotFound, message: "No such file or directory" }

thread 'auth::limits_snapshot_hub::tests::limits_snapshot_second_process_reads_file_and_does_not_http'
assertion `left == right` failed: second process must read the flock snapshot, not HTTP
  left: LeaderFetched
 right: FollowerRead

thread 'extensions::billing::tests::billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http'
assertion `left == right` failed
  left: LeaderFetched
 right: FollowerRead
```

`test result: FAILED. 0 passed; 3 failed` then `FAILED. 0 passed; 1 failed`.

## Product files (plain English)

- `crates/codegen/xai-grok-shell/src/auth/limits_snapshot_hub.rs`: flock hub. Exclusive lock on `limits_snapshot.lock`. Atomic JSON at `limits_snapshot.json`. HonorTtl reuses a file younger than 60s (same window as Management process cache). ForceRefresh fetches only when this process got exclusive without waiting; after waiting, reuse the just-written file unless it is still missing or stale. Kill-switch `GROK_DISABLE_SHARED_RATE_LIMIT` skips the shared file so each process fetches. Apply fills included-billing remember maps and optional Management cache seeds.
- `crates/codegen/xai-grok-shell/src/auth/mod.rs`: module + re-exports.
- `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`: seed helpers so a follower can load prepaid / postpaid / series from the snapshot with no Management HTTP.
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs`: `handle_get_billing` goes through the hub (HonorTtl). Leader fetch callback polls active plus siblings and Management. Followers rebuild the ACP billing response from the snapshot. `fetch_credits_config_with_session` stays the raw HTTP primitive. `poll_and_remember_non_active_supergrok_included_billing` stays for existing tests and is no longer called unconditionally from the handler.
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`: `grok-oss limits` collect uses ForceRefresh on the same hub. Balances and Management meters come from the snapshot (or process cache after a leader apply).
- `doc/dev/upstream-regression-filters.md` and `FORK.md`: the four named tests are on the dual-auth / token-economy land cheat sheet.

## Green re-run

Same isolated env. After product + clippy mop (question-mark, redundant closure) + shared test env lock so parallel tests do not wipe remember maps:

```
running 10 tests
... limits_snapshot_never_writes_access_tokens ... ok
... limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once ... ok
... billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http ... ok
... limits_snapshot_second_process_reads_file_and_does_not_http ... ok
... pick_prefers_business_included_before_personal_when_both_have_remaining ... ok
... order_credentials_business_included_before_personal_when_both_have_room ... ok
... order_credentials_personal_full_with_extras_hops_to_business_included_before_extras ... ok
... sampling_config_hops_to_sibling_included_before_extras ... ok
... sampling_config_auto_use_extras_keep_session_console_failover ... ok
... combined_included_remaining_sums_distinct_personal_and_business_pools ... ok
test result: ok. 10 passed; 0 failed
```

Pager `limits_cmd::`: 42 passed, 1 ignored, 0 failed. Spend-order rank is unchanged.

## Verify exits

| Step | Command | Exit |
|------|---------|------|
| FMT_EXIT | `cargo fmt -p xai-grok-shell -p xai-grok-pager` | 0 |
| CLIPPY_SHELL_LIB_EXIT | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | 0 |
| CLIPPY_PAGER_LIB_EXIT | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| CLIPPY_SHELL_ALL_TARGETS | `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` | 101 (unrelated writers: manager_tests field reassign, ascii-scrub await-holding-lock, subprocess items-after-test, existing xai_management single_match). Used `--lib` as allowed. |
| TEST_EXIT | named four + keep-green six | 0 |
| TEST_PAGER_LIMITS_CMD_EXIT | `cargo test -p xai-grok-pager --lib -- limits_cmd::` | 0 |

## Honest leftovers

- Slice A (inventory / user-guide "second `grok login`" pages) was not started.
- `poll_and_remember_non_active_supergrok_included_billing` still exists for older tests. New TUI and CLI collect go through the hub.
- `fetch_credits_config_with_session` still does HTTP when called directly. Leader fetch and isolated tests use it. Multi-process collect must go through the hub.
- Isolated tests that set `GROK_DISABLE_SHARED_RATE_LIMIT` still each fetch. That is the documented kill-switch.
- User-guide pages named in the plan (02 / 04) were not rewritten in this slice.
- No grok.com workspace-switcher OAuth. No daemon. No SIGUSR1 for limits.
- L3 spawn was not available in this host L2 window. Work stayed on L2 with a short file list.

`active_sessions.json` is still only a live-TUI hint. Flock is the authority. Dead leader: OS releases the exclusive flock on process exit; the next waiter becomes leader.
