# Report: strengthen billing tests + SuperGrok keep-last-good

**Date:** 2026-08-03
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Critic SoT:** `.agents/reports/critic-billing-tests-and-xai-api-2026-08-03.md`
**Prior green:** `.agents/reports/bug-ci-two-unit-fails-2026-08-03.md` (None→poll false, `/spend` reserved)

## Product fix (highest client risk)

**Problem:** SuperGrok ACP/parse failure while OpenRouter or console team prepaid succeeded emitted `BillingFetched { balance: None }`, which **wiped** SuperGrok last-known balance. Pure SuperGrok failure used `BillingError` and kept cache. Asymmetry lied when another meter was warm.

**Fix:** three-state SuperGrok fetch, mirroring auto-topup:

| Outcome | Meaning |
|---------|---------|
| `CreditBalanceFetch::Resolved(Some(bal))` | Apply SuperGrok balance |
| `CreditBalanceFetch::Resolved(None)` | Successful response, no config → **clear** SuperGrok cache + SuperGrok-only poll |
| `CreditBalanceFetch::Unchanged` | SuperGrok transport/parse fail → **keep** last SuperGrok; side meters may still update |

Effects (`FetchBilling` / `FetchAppBilling`): fail paths with side meters now emit `Unchanged`, never `Resolved(None)`.

## Ranking / exhaust gate

`should_apply_included_usage_side_effects(bal)` requires `included_usage_known`.

- Effects: `remember_active_supergrok_included_billing` only when known.
- `handle_billing_fetched` and `AppBillingFetched`: `apply_billing_usage_to_session_exhaust` only when known.

Placeholder `usage_pct: 0.0` + unknown no longer poisons ranking or clears a Marked exhaust memo as if free SuperGrok period reset.

**Did not invent free SuperGrok period debit.**

## Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | `CreditBalanceFetch`, pure policy helpers |
| `crates/codegen/xai-grok-pager/src/app/actions.rs` | TaskResult uses three-state |
| `crates/codegen/xai-grok-pager/src/app/dispatch/billing.rs` | handle three-state + gate exhaust |
| `crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs` | AppBillingFetched same policy |
| `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` | Unchanged on SuperGrok fail; remember gated |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/billing.rs` | Stronger contracts |

## Critic §2 table → landed / parked

| Contract | Status | Test / evidence |
|----------|--------|-----------------|
| None clears agent **and** app | **Landed** | `billing_fetched_none_clears_agent_and_app_credit_balance` |
| Unknown included keeps poll + honest chrome | **Landed** | `billing_fetched_unknown_included_keeps_poll_and_honest_placeholder` |
| True zero known vs unknown | **Landed** | `billing_fetched_true_zero_included_known_does_not_force_poll` + unknown test |
| SuperGrok fail + console/OR keeps SuperGrok | **Landed (product + test)** | `fetch_billing_supergrok_error_with_console_prepaid_keeps_prior_supergrok_balance`, `…_with_openrouter…`, pure `credit_balance_fetch_from_supergrok_path` |
| Silent BillingError preserves cache/poll | **Landed** | `billing_error_silent_preserves_cached_balance_and_poll` |
| None + console/OR keeps poll | **Landed** | `billing_fetched_none_with_console_prepaid_keeps_poll`, `…_with_openrouter…` |
| None + Cleared autotopup | **Landed** | `billing_fetched_none_with_cleared_autotopup_resets_rule` |
| Exhaust not on unknown included | **Landed (gate)** | `remember_active_skipped_when_included_usage_unknown` + product gate; full hermetic memo integration **parked** |
| Ranking remember skipped when unknown | **Landed** | same pure gate + effects call sites |
| AppBillingFetched None clear / Unchanged keep / unknown | **Landed** | `app_billing_fetched_none_…`, `…_unchanged_…`, `…_unknown_…` |
| Meters stay distinct | **Landed** | `billing_fetched_console_prepaid_does_not_mutate_supergrok_prepaid_field` |
| None + tier clear + tier update | **Landed** | `billing_fetched_none_with_tier_updates_tier_and_clears_balance` |
| `test_bal_unknown()` helper | **Landed** | dispatch tests helper |
| Token Economy no invent free SuperGrok debit | **Parked** | shell/token_economy slice; not this PR |
| Flat multi-sample honesty dispatch | **Parked** | needs history injection hooks |
| Identity + exhaust hermetic integration | **Parked** | needs temp grok_home exhaust suite |
| Cold-start "no config yet keep poll" join-note tension | **Parked** | explicit policy still Resolved(None)→poll off unless side meters; residual if cold chrome freezes |

## Red/green notes

- **Product bug was real** (wipe on side-meter success). Fixed first; new tests lock keep-last-good.
- Many asserts are **assert-only on already-correct paths** (silent error preserve, true zero poll, meter slots) — **no fake red**.
- Gate for ranking/exhaust is product green with pure helper tests (no filesystem exhaust red).

## Proof (green)

```bash
cargo fmt -p xai-grok-pager

cargo test -p xai-grok-pager --lib -- billing_fetched
# 25 passed (includes strengthened contracts)

cargo test -p xai-grok-pager --lib -- shell_collision
# 1 passed

cargo test -p xai-grok-pager --lib -- billing_error
# 3 passed

cargo test -p xai-grok-pager --lib -- fetch_billing_supergrok
# 1 passed

cargo test -p xai-grok-pager --lib -- remember_active_skipped
# 1 passed

cargo test -p xai-grok-pager --lib -- app_billing_fetched
# 4 passed

cargo test -p xai-grok-pager --lib -- credit_balance_
# 18 passed (mapping honesty incl. empty/zero known)

cargo test -p xai-grok-pager --lib -- unknown_included
# 4 passed

cargo test -p xai-grok-pager --lib -- true_zero
# 2 passed
```

No git add/commit.

## Empty states (named)

1. **`config` absent** (`Resolved(None)`): clear SuperGrok UI cache; "No billing data"; SuperGrok poll off unless OR/console prepaid present.
2. **`config` present, included unknown** (`Resolved(Some)` + `included_usage_known: false`): keep `Some(bal)`, `...%` / "not yet available", poll on, no ranking/exhaust from placeholder 0.
3. **SuperGrok path fail** (`Unchanged` or `BillingError`): keep last SuperGrok; side meters update only on `Unchanged` path with OR/console data.

*End of implementer report.*
