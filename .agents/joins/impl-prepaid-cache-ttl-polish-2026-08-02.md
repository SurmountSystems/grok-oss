# Join: soft prepaid cache TTL / force-refresh polish (2026-08-02)

## Outcome

Shipped minimal honesty + force path for console team prepaid process cache.
No new UI button. Did **not** fold prepaid into defaultCredits, flip
`auto_use_included_limits`, or touch Design A / after-burner.

Review follow-up (same day): postpaid combined-clear hermetic lock, pure
FetchBilling-vs-collect policy, softened lag note (app last-good), shared TTL
const + prepaid alias, prepaid-only clear docs.

## Inventory (pre-change)

| Piece | Behavior |
|-------|----------|
| TTL | 60s process-local prepaid (+ postpaid same window) |
| Serve cache | warm entry returned if fresh |
| Bust points | management key clear/rotate; tests |
| Last-good | App state keeps cents on `None`; process cache not past-TTL |
| Explicit limits | fetch could still hit warm ≤60s cache |
| Honesty | residual only; no `/limits` lag note |

## Product change

1. **Public TTL:** `CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS = 60` (shared
   prepaid+postpaid). Alias `CONSOLE_TEAM_PREPAID_CACHE_TTL_SECS` kept equal.
2. **Force path:** `clear_console_team_billing_meter_caches()` (prepaid +
   postpaid; keeps discovered team id). Called from explicit `grok limits`
   collect when management key present, gated by
   `management_meter_cache_policy_for_explicit_limits_collect() == ForceRefresh`.
3. **Background policy:** `management_meter_cache_policy_for_background_billing_poll()
   == HonorProcessTtl`. `fetch_console_team_prepaid_cents` (FetchBilling) does
   **not** clear; doc comment states that.
4. **Honesty:** when prepaid $ shown:
   process cache may lag up to Ns; UI may keep last successful cents until a
   later successful fetch; `grok limits` forces a fresh Management fetch.
5. **prepaid-only clear docs:** building block / tests / key rotate; product
   force path is the combined clear.

## Files

- `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`
- `crates/codegen/xai-grok-shell/src/auth/mod.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs`
- `RESIDUAL.md`
- `doc/dev/research/console-team-business-usage-meter-2026-07-30.md`

## TDD (honest trail)

**Process note:** first wave encoded contracts and landed green without a
logged observed-red fail line before product edit (assert-first / red not
captured). Contracts are real and green; do **not** rewrite asserts to fake a
red story. Future polish: land assert-only red, paste fail line, then product.

| Test | Contract |
|------|----------|
| `fetch_prepaid_balance_hermetic_mock_returns_cents` | warm cache → combined clear → second HTTP (hits=2) |
| `fetch_postpaid_preview_hermetic_parses_oauth_vs_api_totals` | warm postpaid → combined clear → second HTTP (hits=2) |
| `billing_meter_cache_ttl_secs_is_sixty_and_prepaid_alias_matches` | shared TTL=60; prepaid alias equal |
| `management_meter_cache_policy_collect_force_background_honor_ttl` | collect ForceRefresh ≠ background HonorProcessTtl |
| `prepaid_lag_note_when_console_team_prepaid_dollars_shown` | TTL + process cache + last successful + grok limits |
| `no_prepaid_lag_note_without_prepaid_dollars` | no lag claim without $ |
| `format_console_section_distinguishes_...` | Balance $ path includes lag + last-good + force path |

## Verify

```bash
cargo fmt -p xai-grok-shell -p xai-grok-pager
cargo test -p xai-grok-shell --lib -- prepaid postpaid billing_meter_cache -- --test-threads=1
cargo test -p xai-grok-pager --lib -- prepaid_lag management_meter_cache_policy format_console_section_distinguishes limits_honesty -- --test-threads=1
```

Green after review follow-up.

## Residual

Rank **3** soft prepaid TTL polish **shipped** (incl. review polish). Open
ranks 1–2, 4–7 unchanged.

## Review issues (all addressed)

| Issue | Status | Response |
|-------|--------|----------|
| Postpaid force-bust not locked by combined clear | **fixed** | Hermetic postpaid test: warm → `clear_console_team_billing_meter_caches` → cache None → hits=2 |
| No contract that FetchBilling does not clear | **fixed** | Pure `ManagementMeterCachePolicy` + unit test (collect ForceRefresh vs background HonorProcessTtl); collect gates clear on ForceRefresh; FetchBilling helper comment; no clear on poll path |
| TDD red not evidenced | **fixed** (process honesty) | Join/summary state assert-first / red not captured; contracts green without fake red |
| Lag note process-cache only; app last-good can outlive 60s | **fixed** | Softened note: process cache lag + UI last successful cents + force path |
| TTL const prepaid-named but shared with postpaid | **fixed** | `CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS` + prepaid alias |
| prepaid-only clear docs say explicit limits | **fixed** | Docs: building block; product force path is combined clear |

## Implementation Summary (review follow-up)

- Shell: shared billing-meter TTL const + alias; prepaid-only clear docs;
  postpaid hermetic combined-clear → second HTTP.
- Pager: policy enum + collect gate + FetchBilling comment; lag note
  completeness; format/honesty tests updated.
- Join/summary: honest TDD trail; review table closed.
- fmt + focused cargo tests green. No git add/commit.
