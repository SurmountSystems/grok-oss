# Join: Slice 3 M3 postpaid invoice preview

**Date:** 2026-08-02
**Implementer:** L2
**Plan:** limits-first ideal Slice 3 only
**Also:** `/tmp/grok-$(id -u)/grok-impl-summary-limits-s3.md`

## Outcome

Product can fetch Management postpaid invoice preview, aggregate OAuth vs API class cents, surface them under the console meter family in `limits --json` / human notes, and emit C6 honesty when SuperGrok is live and OAuth postpaid dominates. Design A rank/strip unchanged. Prepaid $ and SuperGrok extras stay distinct meters.

## RED

Named contracts introduced before / with product wire-up:

| Test | Expected fail reason before product |
|------|-------------------------------------|
| `fetch_postpaid_preview_hermetic_parses_oauth_vs_api_totals` | symbol/path missing; no M3 client |
| `postpaid_preview_gap_when_no_management_key` | no gap path for missing key |
| `limits_json_surfaces_postpaid_oauth_vs_api_and_c6_honesty` | no postpaid JSON fields / no C6 note |
| `c6_team_usage_note_when_oauth_postpaid_dominates` | honesty input lacked oauth_postpaid_dominates |

Command (after wire-up, green log):
```bash
cargo test -p xai-grok-shell --lib fetch_postpaid_preview_hermetic_parses_oauth_vs_api_totals
cargo test -p xai-grok-shell --lib postpaid_preview_gap_when_no_management_key
cargo test -p xai-grok-pager --lib limits_json_surfaces_postpaid
cargo test -p xai-grok-pager --lib c6_team_usage
```

## GREEN

```bash
cargo test -p xai-grok-shell --lib xai_management
# 25 passed (incl. named M3 hermetic + gap)

cargo test -p xai-grok-pager --lib limits_cmd
# 16 passed (incl. postpaid JSON + gap)

cargo test -p xai-grok-pager --lib limits_honesty
# 9 passed (incl. C6)

cargo test -p xai-grok-pager --lib limits_snapshot
# 32 passed
```

`cargo fmt -p xai-grok-shell -p xai-grok-pager` applied.

## Product behavior

1. **Fetch:** `GET {management}/v1/billing/teams/{team_id}/postpaid/invoice/preview` with Management Bearer; same key/team resolve as prepaid (discovery when team id cold).
2. **Parse:** Sum line `amount` (cents) by class from `description`/`product`; period total from `totalWithCorr` when present.
3. **JSON** (`console` object): `teamPostpaidPeriodTotalUsd`, `teamPostpaidOauthClassUsd`, `teamPostpaidApiClassUsd`, or `teamPostpaidGap` (`no_management_key` / `no_management_team_id` / `team_postpaid_unavailable`).
4. **Honesty C6:** when live = SuperGrok session and oauth_class > api_class and oauth > 0:
   `Note: SuperGrok session can still move team Usage dollars (OAuth / Grok Build class on the team invoice), even when the console API key is not live.`
5. **Does not:** change Design A, hop to console for Usage $, invent dollars without Management response.

## Files

- `crates/codegen/xai-grok-shell/src/auth/xai_management.rs` (+ exports in `auth/mod.rs`)
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs` (cache attach for TUI)

## Not done (by design)

- Slice 4 ExhaustedAll extras-before-console
- M6 usage series POST
- Expanding `TaskResult::BillingFetched` to live-fetch postpaid in TUI effects (CLI collect does live fetch; TUI reads process cache when warm)

## Review follow-up (2026-08-02)

All 6 open review issues fixed; none wontfix. See `/tmp/grok-1000/grok-review-limits-s3.md`.

| Fix | Test / behavior |
|-----|-----------------|
| Path segment encodes `.` and `/` (no `/../`) | `postpaid_path_encodes_slash_in_team_id`, `postpaid_path_encodes_or_rejects_dotdot_team_id` |
| Postpaid log fields keyless | `management_postpaid_success_log_fields_are_honest_and_keyless` |
| `product=grok-build` → Oauth | `classify_postpaid_line_product_grok_build_is_oauth` |
| Unparseable amounts → None | `postpaid_preview_none_when_line_amounts_unparseable` |
| Gap asserts api class None | `limits_json_postpaid_gap_when_no_management_key` |
| Credential change clears billing caches | `clear_management_billing_process_caches` from store/clear management key |

```
cargo test -p xai-grok-shell --lib xai_management  # 30 passed
cargo test -p xai-grok-pager --lib limits_cmd       # 16 passed
cargo test -p xai-grok-pager --lib limits_honesty   # 9 passed
cargo test -p xai-grok-pager --lib limits_snapshot  # 32 passed
```
