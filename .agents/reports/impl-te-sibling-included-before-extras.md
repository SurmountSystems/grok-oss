# Implement report: hop to sibling included SuperGrok period limits before SuperGrok dollar credits

**Board parent:** `feat:token-economy-all-plans-ipc`  
**Slice:** `impl:te-sibling-included-before-extras`  
**Plan:** `.agents/plans/token-economy-all-plans-ipc.md` (Slice C, then Slice B)  
**Date:** 2026-08-14  
**Isolated compile:** rustc 1.97.1, `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-sibling-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`

SuperGrok is paid. This report says **included SuperGrok period limits**, never "free SuperGrok."

Slice D (one limits fetcher, flock snapshot hub) is **not** done. Stop here.

---

## What landed (plain English)

When personal SuperGrok Heavy included SuperGrok period limits are full, the product now hops to the next **stored** SuperGrok login that still has included SuperGrok period limits (Business / Team) **before** spending SuperGrok dollar credits that never expire.

Remaining included SuperGrok period limits across **distinct** pools is the real remaining included quota. Compact chrome and the active spend driver stay on included SuperGrok period limits while any distinct pool still has remaining.

A second plan exists only if `auth.json` already has a second SuperGrok multi-slot. No grok.com account-switcher OAuth was added.

Single-identity after-burner is unchanged: included full + extras + no sibling still keeps SuperGrok session live with console failover (`sampling_config_auto_use_extras_keep_session_console_failover`).

---

## Slice C (spend order)

### Red (observed, then product)

**1. `afterburner_does_not_skip_mark_when_sibling_has_included_remaining`**

- Command: `cargo test -p xai-grok-shell --lib -- afterburner_does_not_skip_mark_when_sibling_has_included_remaining -- --test-threads=1`
- Fail (before sibling gate): `afterburner_skips_allowance_mark_with_sibling(..., sibling_has_distinct_included_remaining = true)` returned **true**. Assert: `"sibling included remaining must not skip the out-of-allowance mark"`.
- Product: skip extras **only** when every distinct included pool is exhausted (`&& !sibling_has_distinct_included_remaining`).
- Green: same filter, `ok`.

**2. `apply_billing_marks_personal_full_when_business_sibling_has_included`**

- Command: `cargo test -p xai-grok-shell --lib -- apply_billing_marks_personal_full_when_business_sibling_has_included -- --test-threads=1`
- Fail: `apply_billing_usage_to_session_exhaust(100.0, home)` returned **`None`**, expected **`Marked`**. After-burner skipped the mark because extras were present and the apply path did not ask about a sibling.
- Product: apply loads `any_sibling_has_included_remaining` after remember, then uses `afterburner_skips_allowance_mark_with_sibling`.
- Green: same filter, `ok`.

**3. Rank / sampling hop lock-in (already-green helpers; tests name the contract)**

These helpers already hopped a loaded sibling with included remaining. The named tests lock that as the spend-order contract. First run was green (not a fake red).

- `order_credentials_personal_full_with_extras_hops_to_business_included_before_extras`
- `sampling_config_hops_to_sibling_included_before_extras` (catalog hop name)

First compile of `sampling_config_hops_to_sibling_included_before_extras` failed because the test asserted `SamplerConfig.auth_type` (that field does not exist). That was a test-authoring compile error, not the hop contract. The unused `AuthType` import was dropped. Hop asserts on `api_key` / `failover_api_keys` were green.

**4. `align_after_billing_switches_sticky_personal_full_to_business_included`**

Hermetic. Remember personal 100% + extras, Business 40%, then `align_to_ranked_free_period_primary`. Product in `handle_get_billing` re-applies exhaust after sibling poll, then aligns when `auto_use_included_limits` is on.

**5. `prepare_sampler_for_turn_aligns_to_ranked_included_primary`**

Hermetic (no network). Writes `auth.json` via `upsert_supergrok_session` + `serde_json`. Calls `apply_ranked_auto_turn_credentials` (same rebuild `prepare_sampler_for_turn` / `reconstruct_full_config` use). Primary becomes Business included; console and personal extras stay off failover.

### Product (Slice C)

- `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`  
  `afterburner_skips_allowance_mark_with_sibling`, `any_sibling_has_included_remaining`, apply path sibling gate.
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs`  
  After `poll_and_remember_non_active_supergrok_included_billing`, re-apply exhaust, then `align_to_ranked_free_period_primary` when auto-use is on.
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`  
  `apply_ranked_auto_turn_credentials`; `prepare_sampler_for_turn` aligns first; `reconstruct_full_config` rebuilds primary + failover from `order_credentials_for_preferred_auto`.
- `crates/codegen/xai-grok-shell/src/auth/mod.rs`  
  Exports for the sibling skip helpers.
- Tests: `supergrok_identity_rank.rs`, `allowance_exhaust_from_billing.rs`, `config_tests.rs`, `manager_tests.rs`, `sampler_turn.rs`.

### Green (Slice C, re-run this turn)

```bash
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-te-sibling-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo test -p xai-grok-shell --lib -- \
  order_credentials_personal_full_with_extras_hops_to_business_included_before_extras \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  apply_billing_marks_personal_full_when_business_sibling_has_included \
  sampling_config_hops_to_sibling_included_before_extras \
  sampling_config_auto_use_extras_keep_session_console_failover \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
  -- --test-threads=1
```

Result: **7 passed**. Keep-green single-identity extras: `sampling_config_auto_use_extras_keep_session_console_failover` still `ok`.

---

## Slice B (sum + chrome)

### TDD honesty

`combined_included_remaining` and chrome wiring were written in the same implementer session as the named tests. This continuation's **first compile and run** of the four named Slice B tests was green. There is no separate observed-red log for Slice B (product was already in the tree when these filters first compiled). That is not a fake red. It is also not a full red-then-green log.

### Product (Slice B)

- `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`  
  `IncludedPoolReading`, `CombinedIncludedRemaining`, `combined_included_remaining`, `chrome_included_usage_from_combined`.  
  Unknown usage does not add. Unified pool (`is_unified_billing_user == Some(true)`, or same floored used percent + same reset) counts once (max remaining). Combined chrome percent is `100 - floor(sum_remaining / (100 * distinct_pool_count) * 100)`.
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs`  
  `combined_included_from_active_and_process_cache` (process included cache + active; applies the active `is_unified_billing_user` flag). Compact meter uses combined chrome so personal 100% + extras still paints included SuperGrok period limits when a sibling pool has remaining.
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`  
  `combined_included_from_limits_snapshot`, `chrome_included_from_limits_snapshot`, `active_driver_line_for_snapshot`.
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`  
  `active_spend_driver_from_snapshot` uses combined chrome.
- `crates/codegen/xai-grok-shell/src/auth/mod.rs`  
  Exports `CombinedIncludedRemaining`, `IncludedPoolReading`, `combined_included_remaining`, `chrome_included_usage_from_combined`.

### Green (Slice B)

```bash
cargo test -p xai-grok-shell --lib -- \
  combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool \
  -- --test-threads=1
# 2 passed

cargo test -p xai-grok-pager --lib -- \
  compact_meter_stays_included_while_sibling_pool_has_remaining \
  active_spend_driver_stays_included_while_any_distinct_pool_has_remaining \
  compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct \
  active_driver_afterburner_extras_when_free_period_full \
  -- --test-threads=1
# 4 passed (including single-identity extras keep-green)
```

Combined used percent for personal 100% + Business 24%: remaining 76, two pools, chrome used percent 62. Unified flag or same floor percent + same reset: one pool.

---

## fmt / clippy

```bash
cargo fmt -p xai-grok-shell -p xai-grok-pager
# FMT_EXIT:0

cargo clippy -p xai-grok-shell -p xai-grok-pager --lib -- -D warnings
# Finished `dev` profile, exit 0
```

Sampler crate was not edited.

---

## Catalog / FORK

Added existing `fn` names only (no invented names, no Slice D `limits_snapshot_second_process`):

- `doc/dev/upstream-regression-filters.md` class 5 hop: `sampling_config_hops_to_sibling_included_before_extras`, `afterburner_does_not_skip_mark_when_sibling_has_included_remaining`, `align_after_billing_switches_sticky_personal_full_to_business_included`, `prepare_sampler_for_turn_aligns_to_ranked_included_primary`
- Same file class 5b (sum + chrome, not hop keys): `combined_included_remaining_sums_distinct_personal_and_business_pools`, `combined_included_remaining_does_not_double_count_unified_pool`, `compact_meter_stays_included_while_sibling_pool_has_remaining`, `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`
- `FORK.md` land cheat sheet dual-auth block lists the hop + combined + compact names

---

## Honest leftovers

1. **Slice D is not done.** There is still no flock-backed limits snapshot hub. Every `grok-oss` process still calls the limits APIs. Do not start that here.
2. **Second SuperGrok login is operator-gated.** Ranking only sees what is already in `auth.json`. grok.com Hunter Beast vs Surmount Team is a different product. If only one SuperGrok session is stored, there is no sibling to hop to.
3. **Live TUI stays old until rebuild / re-exec.** This tree change is not the running binary.
4. **Optional `/limits` combined remaining sentence** from the plan (remaining included SuperGrok period limits across N plans) was not added.
5. **`IncludedBillingFields` still does not store `is_unified_billing_user`.** Compact chrome applies the flag from the active `CreditBalance` onto process-cache readings. `/limits` uses `shared_unified_supergrok_pool`. Same floored percent + same reset still dedupes without the flag.
6. **Unknown sibling remaining defaults to 1** in `load_supergrok_session_candidates` when the JWT is not hard-expired and not memoized exhausted. Combined chrome does **not** invent a percent for unknown identities (those rows do not add to the sum). Hop can still try a sibling whose included percent has not been polled yet.
7. **Slice A** (limits JSON discovered-identities honesty) was not this slice.

No next `/implement` prompt. Slice D belongs to the next implementer.
