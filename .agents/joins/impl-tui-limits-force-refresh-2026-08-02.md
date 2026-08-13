# Join: TUI `/limits` force-refresh parity with CLI (2026-08-02)

## Outcome

TUI explicit `/limits` open (slash, status click, `/limits --json`) now
force-busts Management prepaid+postpaid process caches before silent
`FetchBilling`, same class as CLI `grok limits` collect. Background
`FetchBilling` still honors ≤60s process TTL (no clear). `FetchBilling` also
live-calls postpaid preview into process cache so explicit open is not
cache-only for M3.

Did **not** flip `auto_use_included_limits`, fold meters, or change Design A /
after-burner.

## Product change

1. **Policy helpers** (`limits_cmd`):
   - `management_meter_cache_policy_for_explicit_limits_open() → ForceRefresh`
   - `should_clear_management_meter_caches(policy, has_key)`
   - `should_queue_silent_billing_on_explicit_limits(has_key, needs_sibling)`
   - `should_live_fetch_console_team_postpaid_with_billing(has_key)`
2. **`dispatch_show_limits` / `dispatch_show_limits_json`:** clear when
   ForceRefresh + management key; queue silent FetchBilling when key **or**
   sibling SuperGrok included still empty (always after clear when key present).
3. **`FetchBilling` effect:** parallel `fetch_console_team_postpaid_into_process_cache`
   (TTL honored unless open/collect cleared). Modal rebuild still attaches
   postpaid from process cache.
4. **Honesty note:** names `grok limits` **or** opening `/limits` as force path.

## Files

- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` (assert only)
- `RESIDUAL.md`

## TDD

| Step | Evidence |
|------|----------|
| **Red** | Stubbed `management_meter_cache_policy_for_explicit_limits_open` → `HonorProcessTtl`; test failed: `left: HonorProcessTtl right: ForceRefresh` ("TUI explicit /limits open must force-refresh") |
| **Green** | Open policy → `ForceRefresh`; status dispatch + FetchBilling postpaid path; honesty note; pure helper contracts green |

| Test | Contract |
|------|----------|
| `management_meter_cache_policy_collect_force_background_honor_ttl` | collect + open ForceRefresh; background HonorProcessTtl |
| `should_clear_management_meter_caches_force_with_key_only` | clear only ForceRefresh + key |
| `should_queue_silent_billing_on_explicit_limits_when_key_or_sibling` | key or sibling → queue; neither → no |
| `should_live_fetch_postpaid_with_billing_when_management_key` | postpaid with billing iff key |
| `prepaid_lag_note_when_console_team_prepaid_dollars_shown` | note names CLI + `/limits` force paths |
| `format_console_section_distinguishes_...` | detail includes both force paths |

## Verify

```bash
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib -- management_meter_cache_policy should_clear_management_meter should_queue_silent_billing should_live_fetch_postpaid prepaid_lag format_console_section_distinguishes show_limits -- --test-threads=1
```

Green.

## Residual

Rank-2 TUI force-refresh parity + TUI live postpaid (explicit path) **shipped**.
Background polls still HonorProcessTtl. Open ranks elsewhere unchanged.

## Not done / intentional

- Modal zero-countdown refresh stays HonorProcessTtl (not operator explicit).
- Postpaid still surfaces via process cache on rebuild (live-fill is the fetch
  into that cache after clear on explicit open).
