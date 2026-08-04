# Join: Item 5 — spend series + separate team default-credits line

**Date:** 2026-08-03
**Implementer:** L2
**Plan:** finish five open limits/billing gaps § Item 5
**Operator OK:** "Good Grok."

## Outcome

Both **5a** (Management usage series via documented POST) and **5b** (team default credits as its own line) are product-wired with red→green tests. Default credits are never folded into the console team prepaid wallet dollars. Meters keep full plain English names.

## What shipped

### 5b — Team default credits (own line)

- Postpaid preview already parsed `defaultCredits` / `defaultCreditsIssued` (shell `ConsoleTeamPostpaidPreview`).
- Pager `ConsoleTeamPostpaidMeter` now carries `default_credits_cents`.
- Human `/limits` and `grok limits` line:
  - `Team default credits (dashboard allotment; not the prepaid wallet): $N`
- JSON: `console.teamDefaultCreditsUsd`
- Honesty note when present:
  - names that this is **not** the team prepaid wallet, **not** free SuperGrok period allowance, and **not** SuperGrok prepaid top-up dollars

### 5a — Spend / usage series (POST analytics)

- Shell client: documented `POST /v1/billing/teams/{team_id}/usage` with `analyticsRequest` (day-bucketed `usd` sum, group by `description`, `Etc/GMT` window, default 7 calendar days).
- Parse aggregates OAuth / Grok Build class vs API-key class (same classifier as postpaid lines) plus top description rows.
- Explicit `grok limits` collect live-fetches series when management key is available.
- Human: short series block under Console API (window, class totals, top rows).
- JSON: `teamUsageSeriesOauthClassUsd`, `teamUsageSeriesApiClassUsd`, window start/end.
- Empty successful body → zero totals (honest); missing key → no invent; HTTP/JSON fail → gap note.

## RED → GREEN

| Test | Contract |
|------|----------|
| `parse_usage_series_aggregates_oauth_vs_api_usd` | fixture → class USD totals |
| `parse_usage_series_empty_time_series_is_zero_not_none` | empty series is zeros |
| `fetch_usage_series_hermetic_parses_oauth_vs_api_totals` | hermetic POST + Bearer |
| `usage_series_gap_when_no_management_key` | no invent without key |
| `usage_analytics_request_is_day_sum_by_description` | POST body shape |
| `limits_json_surfaces_postpaid_oauth_vs_api_and_c6_honesty` | default credits $1500 distinct from prepaid $340 |
| `limits_json_and_human_surface_usage_series_and_default_credits` | series + default credits labels |
| `default_credits_note_when_reading_present` | honesty copy names exclusions |

```bash
cargo test -p xai-grok-shell --lib -- xai_management
# 38 passed (incl. usage series)

cargo test -p xai-grok-pager --lib -- limits_cmd limits_snapshot limits_honesty
# 81 passed; 1 ignored
```

`cargo fmt -p xai-grok-shell -p xai-grok-pager` applied.

## Files

- `crates/codegen/xai-grok-shell/src/auth/xai_management.rs` — POST usage client, parse, hermetic tests
- `crates/codegen/xai-grok-shell/src/auth/mod.rs` — exports
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` — default credits + series summary + format
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` — default-credits note
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs` — collect fetch + JSON fields + tests
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` — honesty input field
- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` — series + default credits docs

## Not done / residual (honest)

- **Full chart UI** (sparklines / dense day table): skeleton is class totals + top rows on limits; rich charts only if dogfood asks.
- **TUI background FetchBilling** does not live-fetch usage series every poll (CLI / explicit limits collect does). TUI still gets **default credits** via postpaid preview already on the force-refresh path.
- Did **not** change `auto_use_included_limits` default (Item 1).
- Did **not** edit AGENTS process pins (Item 1).
- Did **not** fold default credits into prepaid `$N` or invent a GET series URL.

## Acceptance checklist

| Check | Status |
|-------|--------|
| Series path with management credentials | Yes (POST client + collect wire + tests) |
| Default credits labeled separately from prepaid | Yes |
| Tests green | Yes |
