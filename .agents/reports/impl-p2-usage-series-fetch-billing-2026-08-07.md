# Implement report: P2 usage series on FetchBilling / `/limits` warm path

**Date:** 2026-08-07
**Plan:** session `plan.md` (Grok Business license zeros vs team Usage) — complete P2
**Prior:** `.agents/reports/impl-grok-business-license-zeros-vs-team-usage-2026-08-07.md` (P0+P1)

## Outcome

Usage series (Management `POST …/usage` analytics: OAuth / Grok Build class USD,
API class, top descriptions) is no longer a thinner CLI-only collect path. It
uses the same practical path as team prepaid and postpaid:

| Path | Behavior |
|------|----------|
| Background / silent `FetchBilling` | Live-calls series into process cache when management key present; honors ≤60s TTL |
| TUI `/limits` open (and `/limits --json`) | Force-clears prepaid+postpaid+series caches when key present, then silent FetchBilling |
| CLI `grok limits` | Still force-clear + fetch (now remembers into the shared series cache) |
| `/limits` modal rebuild after billing | Attaches warm series from process cache into Console block |

No unbounded spam: same soft 60s process cache as other Management billing meters;
explicit open/collect busts series with prepaid/postpaid.

Non-goals honored: no license chart invent/scrape; dual SuperGrok poll not
re-opened; no C4 invent; no git stage/commit/push.

## RED → GREEN (named contracts)

| Contract | RED evidence (would fail without product) | GREEN |
|----------|-------------------------------------------|-------|
| **1. Series rides FetchBilling gate when management key available** | New pure policy `should_live_fetch_console_team_usage_series_with_billing` did not exist; FetchBilling only joined prepaid+postpaid | `limits_cmd::tests::should_live_fetch_usage_series_with_billing_when_management_key` PASS (same gate as postpaid; false without key) |
| **2. Series requested/stored with TTL; force-clear busts** | Hermetic fetch had no process cache: second call always HTTP; `clear_console_team_billing_meter_caches` did not drop series | `auth::xai_management::tests::fetch_usage_series_hermetic_parses_oauth_vs_api_totals` PASS: 1st HTTP, 2nd cache hit (hits=1), combined clear → re-fetch (hits=2) |
| **3. Series OAuth / Grok Build class in format when known; no mash** | Display already worked if attached; new P2 contract locks non-mash with prepaid Balance and free SuperGrok period % | `views::limits_snapshot::tests::format_console_surfaces_usage_series_oauth_class_when_known` PASS |
| **Keep green** | Existing series JSON/human + postpaid cache + policy suites | `limits_cmd::` 33 ok; `limits_snapshot::` 39 ok; `xai_management::` 40 ok |

Note: wire-up in `Effect::FetchBilling` and `attach_console_postpaid_from_cache`
is covered by pure policy + hermetic cache + format contracts (same pattern as
postpaid into process cache). No invent of series dollars without key.

## Product changes

### Shell (`xai-grok-shell` / `xai_management.rs`)

- Process cache for `ConsoleTeamUsageSeries` keyed by team id + day window
- TTL = `CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS` (60)
- `cached_console_team_usage_series` / `_default`, `clear_console_team_usage_series_cache`
- `fetch_console_team_usage_series_at` honors cache; `remember_usage_series` on success
- `clear_console_team_billing_meter_caches` and `clear_management_billing_process_caches` also clear series

### Pager

| Area | Change |
|------|--------|
| `limits_cmd.rs` | `should_live_fetch_console_team_usage_series_with_billing`; docs: ForceRefresh clears series too |
| `effects/helpers.rs` | `fetch_console_team_usage_series_into_process_cache` |
| `effects/mod.rs` | `FetchBilling` `tokio::join!` includes series fetch (with prepaid/postpaid/OpenRouter) |
| `dispatch/status.rs` | `/limits` rebuild attaches warm series; force-clear comments include series |

### Docs / residual

- User-guide `02-authentication`, `04-slash-commands`: series on `/limits` + background billing, not CLI-only
- `RESIDUAL.md` / `FORK.md`: remove "P2 parked"; mark series cadence shipped
- Project `AGENTS.md` already has hard constraint **5a Complete plan verticals** (do not invent parking on approved plans)

## Meters stay distinct

free SuperGrok period % ≠ SuperGrok dollar extras ≠ team prepaid Balance ≠ team
postpaid OAuth / Grok Build class period $ ≠ **usage series window** OAuth /
Grok Build class USD.

## Files touched

- `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`
- `crates/codegen/xai-grok-shell/src/auth/mod.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`
- `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md`
- `RESIDUAL.md`, `FORK.md`

## Verify (exit codes)

| Step | Command | Result |
|------|---------|--------|
| fmt | `cargo fmt -p xai-grok-pager -p xai-grok-shell` | 0 (`--check` clean) |
| clippy | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | 0 |
| clippy | `cargo clippy -p xai-grok-shell --lib -- -D warnings` | 0 |
| tests | `cargo test -p xai-grok-shell --lib auth::xai_management::` | 40 ok |
| tests | `cargo test -p xai-grok-pager --lib limits_cmd::` | 33 ok (1 ignored live) |
| tests | `cargo test -p xai-grok-pager --lib views::limits_snapshot::` | 39 ok |

## Operator dogfood (after rebuild)

1. Management key set; open TUI `/limits` or wait for background billing: Console
   should show **Team usage series** with OAuth / Grok Build class when Management
   returns data (not only after a special CLI-only path).
2. `grok-oss limits` still force-refreshes; second background poll within ~60s
   should reuse process cache (no series spam).
3. Prepaid Balance, free SuperGrok period %, and series OAuth class remain
   separate lines.
4. Browser licenses Usage may still be zeros (expected; not this work).

## Not done (still open, not P2)

- Full browser-style series **charts** UI (text totals only)
- License Management API (blocked until public docs)
- git stage/commit (human-only)
