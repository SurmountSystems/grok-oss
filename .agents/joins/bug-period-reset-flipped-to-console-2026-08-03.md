# Join: period reset flipped to console; network failover not economics-aware

**Date:** 2026-08-03
**Contracts:** both (same root cause)

1. After free SuperGrok period allowance resets (used percent drops below 100), with `auto_use_included_limits` on and preferred not `api_key`, live sampling must prefer SuperGrok session again; must not stick on console primary solely because of last period's exhaust memo.
2. Network / connection failure failover must not hop to console while free SuperGrok period used percent is still below 100 under auto_use (limits before credits).

## Root cause (one)

Stale **out-of-allowance fingerprint memo** + enrich policy that **preferred memo over live billing**.

1. When free SuperGrok period used percent hit 100%, product marked the session JWT exhausted (process + `$GROK_HOME/exhausted_credits/`, 1h) and prefer_live / ranking put **console** primary (expected).
2. After period reset, billing could show low free SuperGrok period used percent, but:
   - Shell ACP billing path **remembered** usage for ranking **without** calling `apply_billing_usage_to_session_exhaust` (memo never cleared on that path).
   - Enrich ranked with `memo_exhausted → remaining 0` **even when** live `usage_pct` was e.g. 8% (intentional old anti-race that broke period reset).
3. Result: auto rank still saw ExhaustedAll → console primary; prefer_live still hopped SuperGrok → console; status chrome stayed `console · $N`.
4. Network / soft reconnect does not invent a separate economics path: mid-turn identity hop only walks `failover_api_keys`. When ranking wrongly left console in the chain (or console already primary from sticky memo), recovery stayed on console credits while free SuperGrok period still had headroom. Fixing ranking + memo clear also fixes network re-resolve economics (console **omitted** from hop chain while included headroom remains).

## Product fix

| Area | Change |
|------|--------|
| `apply_included_billing_to_headroom` | Live `usage_pct` always drives remaining; memo forces 0 only when usage is **absent**. Period reset (used &lt; 100) restores SuperGrok headroom. |
| `enrich_candidates_with_included_billing` | Returns tokens whose exhaust memo should clear when live free SuperGrok period used percent is below 100. |
| `load_supergrok_session_candidates` | Clears those memos (skip hard-expired JWTs); re-zeros hard-expired after enrich. |
| Shell `extensions/billing` | After remember, call `apply_billing_usage_to_session_exhaust_with_period` so shell polls mark/clear like the pager. |
| Pager `AppBillingFetched` | Same meter identity update as agent `BillingFetched` on Marked/Cleared (period reset re-labels SuperGrok when not api_key pin). |

No invent of free SuperGrok period debit. Meters stay distinct. Limits before credits preserved (console still omitted while any SuperGrok has included remaining).

## Tests (green)

```bash
cargo test -p xai-grok-shell --lib -- \
  period_reset enrich_period_reset enrich_full_usage \
  auto_order_with_included_headroom load_candidates_period_reset \
  auto_order_omits_console auto_with_included_headroom apply_billing_

cargo test -p xai-grok-sampler --lib -- prefer_live allowance_exhaust rotate_ exhausted
```

Named new tests:

- `enrich_period_reset_billing_headroom_beats_stale_exhaust_memo`
- `auto_order_with_included_headroom_omits_console_from_hop_chain`
- `period_reset_clears_memo_and_ranks_supergrok_primary_without_console`
- `load_candidates_period_reset_billing_clears_stale_memo_without_apply`

## Operator dogfood

Rebuild and run a long dual-auth session where free SuperGrok period was full (console sticky), wait for period reset or force a billing poll after used percent drops. Status should return to SuperGrok (not `console · $…`) while free SuperGrok period used percent is below 100 and `auto_use_included_limits` is on. Unplug/replug network: retries stay on SuperGrok until included is full (or after-burner rules apply).

## Sibling join

Same root cause covers network economics:
[bug-network-failover-not-economics-2026-08-03.md](bug-network-failover-not-economics-2026-08-03.md)
