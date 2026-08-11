# Join: network failover not economics-aware

**Date:** 2026-08-03
**Contract:** Network / connection failure must not hop to console API key while free SuperGrok period used percent is still below 100% under `auto_use_included_limits` (limits before credits).

## Root cause

**Same as period-reset → console.** See
[bug-period-reset-flipped-to-console-2026-08-03.md](bug-period-reset-flipped-to-console-2026-08-03.md).

Mid-turn identity rotate only walks `SamplerConfig.failover_api_keys` (credit and plain 429). Transport/network errors retry the **same** identity (client rebuild / backoff). Soft reconnect and next-turn re-resolve use auto rank + prefer_live.

When a **stale exhaust memo** made ranking treat SuperGrok as out of free SuperGrok period allowance, console became primary or sat in the hop chain. Network recovery then continued on console credits even though live free SuperGrok period used percent was still below 100. That is an economics violation, not a separate network-hop bug.

## Fix

Trust live free SuperGrok period used percent below 100 over the exhaust memo for ranking headroom; clear the memo on enrich/load and on shell billing apply. While any SuperGrok has included remaining, auto order still **omits console** from primary and failover (existing Design A). Network re-resolve cannot burn console until included is full (or after-burner rules put SuperGrok extras first).

## Tests

`auto_order_with_included_headroom_omits_console_from_hop_chain` plus period-reset suite in the sibling join. Sampler rate-limit rotate still does not mark credit-exhausted (`rate_limit_rotate_does_not_memoize_credit_exhausted`).

## Operator dogfood

With free SuperGrok period headroom and dual-auth: unplug network during a turn. Expect soft retry / StreamResumed on SuperGrok, not a silent flip to `console · $…`. After reconnect, live identity should remain SuperGrok until free SuperGrok period used percent is full.
