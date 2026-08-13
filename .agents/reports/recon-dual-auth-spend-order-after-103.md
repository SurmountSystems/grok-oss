# Recon: dual-auth spend-order after 1.0.3 restack

**Date:** 2026-08-13  
**Branch:** `onto-xai/b13fa526f511`  
**Scope:** read-only code + tests. No implement. Nucleo `Some(2)` out of scope.

Meters named here: **included SuperGrok period limits**, **SuperGrok dollar credits**, **console team prepaid / console API credits**. SuperGrok is paid.

---

## Verdict

Spend-order at the **live sampling wire** is **emptied**. The economics **library** and **AuthManager SuperGrok multi-slot pick** are still present. That is **half-wired**: ranking can pick which SuperGrok JWT is current, but the sampler is not given a hop chain or hop hosts.

The nextest mop’s `upsert_supergrok_session` restore did **not** empty spend-order. It put multi-slot persist back. The empty hop chain comes from 1.0.3-shaped `sampling_config_for_model` / `resolve_credentials`, which never stamp failover.

---

## What the tip actually does

### Live chat sampling

`sampling_config_for_model` (`crates/codegen/xai-grok-shell/src/agent/config.rs`) always builds:

- `failover_api_keys: Vec::new()`
- `failover_base_url: None`
- `session_base_url: None`
- `session_identity_key: None`

`ResolvedCredentials` has only `api_key`, `base_url`, `auth_type`, `auth_scheme`. There is **no** `resolve_credentials_preferring` / `resolve_credentials_preferring_with_rank` in the tree. Stale comment still points at it: `auth/xai_console.rs`.

Callers copy those empty fields into chat-state and later reconstruct:

- `prepare_sampling_config_for_model` (`agent/mvp_agent/agent_ops.rs`)
- `ModelsManager::sampling_config` (`agent/models.rs`)
- `resolve_model_override_to_config` (`agent/subagent/mod.rs`)
- session spawn credentials (`agent_ops.rs` ~4486)
- model switch credentials (`session/acp_session_impl/model_switch.rs`)
- `reconstruct_full_config` copies `creds.failover_*` (`sampler_turn.rs`)

`resolve_credentials` priority: model BYOK > auth-provider token > **session JWT** > `XAI_API_KEY`. With a live SuperGrok session it stays SuperGrok-primary. It does **not** queue console. It does **not** switch host (`cli-chat-proxy` vs `api.x.ai`) on exhaust.

`prepare_sampling_config_for_model` still drops the session when `preferred_method = api_key`, so a console pin can become primary. Failover stays empty, so there is no SuperGrok recovery hop.

### Hop machinery still exists, but has nothing to walk

Sampler still implements identity rotate and pre-request skip:

- `xai-grok-sampler/src/prefer_live_primary.rs` (`prefer_live_identity_after_credit_exhaust`, `rotate_identity_config`)
- `xai-grok-sampler/src/actor/request_task.rs` (credit / 429 hop)
- `xai-grok-sampler/src/exhausted_identity.rs` (`sync_allowance_exhaust_from_usage`)

Those only fire when `failover_api_keys` is non-empty. Empty list: no hop, no host switch. `sync_allowance_exhaust_from_usage` is a no-op when `has_console_failover` is false.

### Spend-order library (not on the sampling path)

`order_credentials_for_preferred_auto` (`auth/supergrok_identity_rank.rs`) still encodes the intended order:

1. Any SuperGrok with included period remaining → SuperGrok JWT primary; **console omitted** from failover.
2. Included full + known positive SuperGrok dollar credits (`prepaid_balance_cents > 0`) → SuperGrok stays primary; console in failover.
3. Included full + extras 0 or unknown → console primary; live SuperGrok JWT as recovery failover + `session_identity_key`.

Tests around that function are still in the same file. Billing memo helpers in `auth/allowance_exhaust_from_billing.rs` still call it (including after-burner skip so SuperGrok is not marked out while dollar credits remain). **Nothing in `sampling_config_for_model` calls this.**

### AuthManager restore (nextest mop)

`upsert_supergrok_session` (`auth/model.rs`) writes `{base}::personal` / `{base}::team::{id}` plus the active base. `AuthManager::persist_auth_into_store` / update / save use that again so two SuperGrok principals can coexist.

When `[auth] auto_use_included_limits` is on (default true), `AuthManager::new` calls `align_to_ranked_free_period_primary`. That can switch the **live SessionToken** among SuperGrok multi-slots to the identity with included-period room. That is **not** console hop, and it does not fill `SamplerConfig` failover.

---

## Spend-order vs the four rules

| Intended rule | On tip |
|---------------|--------|
| Burn included SuperGrok period limits first | **Mostly by staying on SuperGrok session** if AuthManager current is a session JWT. Among two SuperGrok logins, rank-align can pick the one with included room. |
| Then SuperGrok dollar credits | **Stay on SuperGrok** because the process never hops away. After-burner rank/memo exists but is unused at sampling. |
| Then console team prepaid / console API credits | **Not automatic.** Empty failover. No hop to console when included is full. |
| While included period still has room, stay on SuperGrok; do not make console primary | **Yes for the default session path** (empty failover cannot hop). **Broken** if `preferred_method = api_key` (console primary, no SuperGrok recovery). Subagent override also ignores preferred/rank and uses bare `resolve_credentials`. |

---

## Already correct vs still owed

**Already correct (do not re-invent):**

- Rank function + unit tests: `order_credentials_for_preferred_auto`, `preferred_uses_supergrok_auto_rank`
- Multi-slot store + AuthManager persist: `upsert_supergrok_session`, `persist_auth_into_store`
- Included-period align among SuperGrok JWTs: `align_to_ranked_free_period_primary`
- Sampler hop/prefer-live **implementation** when a chain is present
- Chat-state `Credentials` still has failover / hop-host fields

**Still owed (this check only; not a new feature list):**

- Restore the resolve stamp that used to call rank and fill `SamplerConfig` (`resolve_credentials_preferring_with_rank` / equivalent). Residual still names it (`RESIDUAL.md` ~499–504) and the filter `sampling_config_auto_use_omits_console` (that test is **gone**).
- `sampling_config_for_model` must copy primary + failover + `failover_base_url` / `session_base_url` / `session_identity_key` from that resolve, not hardcode empty.
- Wire `prepare_sampling_config_for_model`, `ModelsManager::sampling_config`, aux/web-search, and subagent override through the same rank path (subagent tests still describe rank; `resolve_model_override_to_config` does not use it). `resolve_model_override_api_key_pin_keeps_console_primary` is a known remaining fail for that gap.
- Until that stamp is back, hop-host failover after included SuperGrok period limits are full is **empty**, not “console omitted on purpose while included has room.”

Docs that still say the merge is shipped (`FORK.md` dual-auth bullet; residual “bare resolve landmines closed”) are **stale vs this tip**.
