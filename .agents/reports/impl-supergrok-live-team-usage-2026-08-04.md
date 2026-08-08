# Implement report: SuperGrok-live team Management usage visibility

**Date:** 2026-08-04
**Branch:** fixes-2 (or current)
**Exclusive priority:** SuperGrok-live team prepaid/postpaid/series visibility + honesty about Grok Business **licenses** page zeros.
**Token Economy:** parked (not cancelled; not implemented here).

## Contracts (red → green)

| # | Contract | Red evidence | Green |
|---|----------|--------------|------|
| 1 | SuperGrok live + Management prepaid known → footer includes team prepaid $ | `footer_supergrok_live_with_management_prepaid_shows_team_dollars` failed before merge helper | **Green** |
| 2 | SuperGrok live + no management key → team section not silently omitted | `/limits` `format_supergrok_live_without_mgmt_key_keeps_honest_team_block` | **Green** (Balance: no management key on Console API block) |
| 3 | `/limits` team block when key+team fixture even if `console.isLive=false` | `format_supergrok_live_with_management_prepaid_shows_team_balance` | **Green** |
| 4 | Honesty: license page ≠ SuperGrok / team Management; no license msg counts | `format_limits_honesty_distinguishes_license_page_from_product_meters`, `license_page_note_never_claims_messages_conversations_as_product_meter` | **Green** |

Also: SuperGrok live + Management loading gap surfaces on footer
(`footer_supergrok_live_mgmt_loading_surfaces_team_gap`).

**FetchBilling:** verified already joins prepaid + postpaid into process cache
**regardless of console live** (`effects/mod.rs` `Effect::FetchBilling`). No gate fix needed.

## Product change (minimal)

### Footer / `/usage` (`credit_bar.rs`)

- Extracted `supergrok_session_usage_warning` + `merge_supergrok_warning_with_team_prepaid`.
- SuperGrok live + known cents → always `team prepaid: $N` (append or standalone chip).
- SuperGrok live + gap Loading / MissingTeamId / Unavailable → surface gap (Management path active).
- SuperGrok live + MissingManagementKey → SuperGrok-only footer (team honesty on `/limits` Balance so SuperGrok-only users are not noise-spammed).
- `/usage` SuperGrok path attaches `Console team prepaid: $N` (or active-path gap) as its own line; lag honesty when dollars shown.

### Honesty (`limits_honesty.rs`)

- New `NOTE_LICENSE_PAGE_IS_NOT_PRODUCT_METER` always on `/limits` / `grok limits` honesty stack.
- Does not invent license messages/conversations as product meters.

### Snapshot

- Console API block already always formatted; new tests lock SuperGrok-live + prepaid fixture and missing-key honesty.

## Files changed

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | SuperGrok-live team prepaid footer + `/usage` |
| `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` | License page honesty note |
| `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` | Contract tests (SuperGrok-live team block + license) |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | Surfaces + 3-meter map + oauth free-period checklist |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` | `/limits` SuperGrok-live team + footer + license honesty |
| `FORK.md` | Billing meters + license non-goal; TE options parked note |
| `RESIDUAL.md` | Half B SuperGrok-live ship + TE parked + license non-goal |
| `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` | License API research pin (no public API → non-goal) |

## Test commands (green)

```bash
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib -- \
  footer_supergrok_live \
  format_supergrok_live \
  format_limits_honesty_distinguishes \
  license_page_note \
  branch_2b_stack \
  usage_summary_supergrok_live_keeps

# Broader related suite (134 tests) also green:
cargo test -p xai-grok-pager --lib -- views::limits_honesty views::credit_bar views::limits_snapshot views::limits_modal
```

## Residual

| Item | Status |
|------|--------|
| Token Economy further options plan | **Parked** (not cancelled). Resume later. |
| Grok Business licenses charts non-zero | **Non-goal.** No public license messages API; no scrape; no client invent. |
| Full browser-style series charts UI | Optional later (text series totals already shipped). |
| C4 SuperGrok included debit proof | Still open server-side (unchanged). |

## Operator dogfood

```bash
# Prefer free SuperGrok period after reset (does not fill license charts)
# ~/.grok/config.toml:
# [auth]
# preferred_method = "oauth"
# auto_use_included_limits = true

# Management team meters (separate from SuperGrok session)
grok login --management-key
# pin team id if needed:
# [endpoints] management_team_id = "..."

# Rebuild then:
grok-oss limits
grok-oss limits --json
# Expect: liveSampling supergrok_session, console.isLive false (typical),
# teamPrepaidUsd or teamPrepaidGap, team postpaid/series when key works.
# In TUI: SuperGrok live footer shows team prepaid: $N when Management returns cents.
# /limits Notes include license page ≠ SuperGrok/team Management.
```

See also: `.agents/reports/plan-oauth-after-period-reset-2026-08-04.md`.

## Done checklist

- [x] Red tests for contracts, then same tests green
- [x] SuperGrok-live team Management visibility (footer + `/limits` + `/usage`)
- [x] License honesty + docs meter map
- [x] TE parked in residual/FORK
- [x] No license client invent
- [x] `cargo fmt -p xai-grok-pager`
- [x] Report under `.agents/reports/`
