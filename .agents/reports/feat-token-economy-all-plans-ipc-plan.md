# Plan report: included SuperGrok period limits across all stored plans

Plan: `.agents/plans/token-economy-all-plans-ipc.md`

## What exists

The product already knows the three-meter spend order for **one** SuperGrok login: included SuperGrok period limits, then SuperGrok dollar credits, then console. Ranking in `supergrok_identity_rank.rs` will pick a second SuperGrok identity that still has included remaining, and it will omit console while any included pool has room. `auth.json` can keep personal and Business multi-slots after two `grok login`s (`upsert_supergrok_session`, test `team_login_then_personal_keeps_both_principals`). Sibling credits poll exists (`GET {proxy}/billing?format=credits` for the non-active JWT). `/limits` can show two SuperGrok rows. Compact status paints one identity: `included SuperGrok period limits · N%`, then `SuperGrok extras · $N` when that identity is at 100% used.

Shared `grok-rate-limit` flock only spaces retries after a 429. Every live TUI still calls billing on FetchBilling. There is no limits leader and no shared live snapshot. Rebuild SIGUSR1 is for fleet relaunch, not limits. The TUI leader socket is session attach, not a limits hub.

## What is missing

The hop to Business SuperGrok Heavy **before SuperGrok dollar credits** is not wired.

- `afterburner_skips_allowance_mark` looks only at the **active** login's extras. When personal included is full and extras remain, it refuses to mark SuperGrok out of allowance, so `prefer_live` does not hop.
- `align_to_ranked_free_period_primary` runs at AuthManager start and model prepare, not after billing and not in `prepare_sampler_for_turn`.
- Compact chrome and `active_spend_driver` use one JWT's used percent. There is no sum of remaining included SuperGrok period limits across distinct plans.
- Every process fetches limits. There is no one-fetcher snapshot.

The tree cannot see grok.com's Hunter Beast vs Surmount Team switcher. A second SuperGrok plan exists for ranking only if a second `grok login` wrote `{base}::team::{team_id}`.

## Operator decisions

1. **Should Surmount Team SuperGrok Heavy be a second SuperGrok login we already store, or do we still only have one SuperGrok session?** Check `grok login --list-api-keys` / doctor SuperGrok rows. If only one session is stored, hop-to-Team cannot run until a second `grok login`. This plan will not invent a grok.com account-switcher OAuth flow.

2. **When both JWTs report the same included used percent and reset (or `is_unified_billing_user`), is that one shared included pool, or two Heavy plans we must still hop between?** Distinct percents stay separate and are summed. The same-pool case is the only fork.

No other decision is required to start Slice A (list what is stored) or the sibling-before-extras hop tests.
