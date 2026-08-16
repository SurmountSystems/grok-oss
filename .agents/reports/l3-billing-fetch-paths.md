# SuperGrok billing fetch paths (Slice D map)

Read-only inventory for a flock-backed snapshot hub. SuperGrok is paid. Included SuperGrok period limits, SuperGrok dollar extras, and console team prepaid / postpaid / usage series stay distinct.

There is **no** `limits_snapshot_hub.rs` yet. Credits remember maps are process-local only. Management meters are process-local with a 60s TTL. Existing flock stores (`$GROK_HOME/rate_limits/`, `$GROK_HOME/included_poll_history/`) coordinate 429s and poll history, not a shared meter snapshot.

## 1. Core functions, files, call graph

### `handle_get_billing`

- File: `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/extensions/billing.rs`
- Signature: `async fn handle_get_billing(agent: &MvpAgent) -> ExtResult` (crate-private)
- Entry: `billing::handle` on ACP method `"x.ai/billing"`; routed from `MvpAgent` in `crates/codegen/xai-grok-shell/src/agent/mvp_agent/acp_agent.rs`.
- Does **not** call Management prepaid / postpaid / series.

Active path:

1. `require_xai_auth` for the live SuperGrok session JWT + `user_id`.
2. `agent.cli_chat_proxy_base_url()` then `fetch_credits_config_with_session(base, &auth.key, &auth.user_id)`.
3. Overlay remote-settings `on_demand_enabled` / `subscription_tier`.
4. From `billing.config`:
   - `remember_active_supergrok_included_billing(&grok_home, pct, period_end, period_type)`
   - `apply_billing_usage_to_session_exhaust_with_period`
   - `remember_supergrok_dollar_extras` / `remember_supergrok_build_usage` under `active_supergrok_identity_id` or `billing_log_identity_from_auth`
   - `record_included_poll_history_from_config`
5. `poll_and_remember_non_active_supergrok_included_billing(&grok_home, base)`.
6. Re-apply exhaust after sibling remember; optional `align_to_ranked_free_period_primary`.
7. Return `BillingConfigResponse` (active principal only).

Pager callers (ACP only, no Management HTTP):

- `Effect::FetchBilling` and `Effect::FetchAppBilling` in `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` send `x.ai/billing`.
- Queued from turn end, session start, `/usage`, `/limits` (`dispatch_show_limits`), credit-bar refresh.

### `fetch_credits_config_with_session`

- File: same `billing.rs`
- Signature:

```rust
pub async fn fetch_credits_config_with_session(
    proxy_base: &str,
    access_token: &str,
    user_id: &str,
) -> Result<BillingConfigResponse, String>
```

- Builds `GET {proxy_base}/billing?format=credits`.
- Headers: `Authorization: Bearer {token}`, `X-XAI-Token-Auth`, `x-userid`, `x-grok-client-version`, process client-mode. Timeout 15s.
- Waits / observes `$GROK_HOME/rate_limits/` via `shared_http_rate_limit::billing_provider_key(base, token)`.
- Parses `BillingConfigResponse` (`config.creditUsagePercent`, `currentPeriod`, `prepaidBalance`, `productUsage`, …).

Also called from:

- `poll_and_remember_non_active_supergrok_included_billing` (siblings).
- `collect_limits_report_at` in `crates/codegen/xai-grok-pager/src/limits_cmd.rs` (every pollable principal, including active).

### `remember_supergrok_included_billing`

- File: `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`
- Re-exported from `auth/mod.rs`.
- Signature:

```rust
pub fn remember_supergrok_included_billing(
    identity_id: &str,
    usage_pct: f64,
    period_end_rfc3339: Option<&str>,
    period_type: Option<&str>,
)
```

Writes process mutex `INCLUDED_BILLING_BY_IDENTITY: BTreeMap<String, IncludedBillingFields>` (`supergrok_identity_rank.rs`: `usage_pct`, `reset_at`, `period_type`, `prepaid_balance_cents`, `grok_build_usage_pct`). No tokens. Not durable.

Sibling writers on the same map:

- `remember_supergrok_dollar_extras(identity_id, prepaid_balance_cents)`
- `remember_supergrok_build_usage(identity_id, grok_build_usage_pct)`
- `remember_supergrok_billing_poll_ok` / `remember_supergrok_billing_poll_failed`
- `remember_active_supergrok_included_billing(grok_home, usage_pct, period_end, period_type)` resolves `identity_id` from `$GROK_HOME/auth.json`, then remember + poll-ok.

Readers: `included_billing_fields_snapshot()`, ranking `load_supergrok_session_candidates`, dual `/limits` fill, exhaust / after-burner.

### Call graph (today)

```
ACP "x.ai/billing"
  billing::handle → handle_get_billing
    fetch_credits_config_with_session          # active JWT
    remember_active_supergrok_included_billing
      remember_supergrok_included_billing
      remember_supergrok_billing_poll_ok
    remember_supergrok_dollar_extras / remember_supergrok_build_usage
    record_included_poll_history_from_config   # process + $GROK_HOME/included_poll_history/
    poll_and_remember_non_active_supergrok_included_billing
      load_non_active_supergrok_billing_poll_targets
      ensure_fresh_access_token_for_supergrok_billing_poll  # OIDC if JWT near expiry
      fetch_credits_config_with_session        # sibling JWT
      remember_* + record_included_poll_history_from_config

CLI `grok limits` → collect_limits_report → collect_limits_report_at
    load_supergrok_billing_poll_targets        # all principals
    fetch_credits_config_with_session per target
    remember_* (same maps; does not write exhaust memos)
    clear_console_team_billing_meter_caches    # ForceRefresh when management key present
    fetch_console_team_prepaid_balance_default
    fetch_console_team_postpaid_preview_default
    fetch_console_team_usage_series_default    # POST analytics, 7-day default
```

Management HTTP lives in `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`. Product callers of `fetch_console_team_*_default` are **only** `collect_limits_report_at`. Residual text that says TUI `FetchBilling` live-calls Management postpaid / series is **not** what `handle_get_billing` does now. TUI `/limits` queues silent `FetchBilling` (credits only) and later reads Management **process cache** if a CLI collect already filled it (`dispatch_show_spend` uses `cached_console_team_prepaid_cents_default` / `cached_console_team_postpaid_default`). Policy helpers `should_live_fetch_console_team_*` and `should_queue_silent_billing_on_explicit_limits` exist in `limits_cmd.rs` and are unit-tested; they are not wired into the FetchBilling effect.

## 2. HTTP URLs

### SuperGrok credits (included period + SuperGrok dollar extras)

- `GET {cli_chat_proxy}/billing?format=credits`
- Default proxy: `https://cli-chat-proxy.grok.com/v1` (`CLI_CHAT_PROXY_BASE_URL_DEFAULT` in `crates/codegen/xai-grok-shell/src/agent/config.rs`; override `[endpoints] cli_chat_proxy_base_url` or `GROK_CLI_CHAT_PROXY_BASE_URL` on some pager paths).
- Production URL: `https://cli-chat-proxy.grok.com/v1/billing?format=credits`
- Related, not hub-core: `GET {proxy}/auto-topup-rule` from `handle_get_auto_topup_rule`.

### Management (console team meters; not SuperGrok extras)

Base: `https://management-api.x.ai` (`MANAGEMENT_API_BASE_URL`).

| Meter | Method | Path |
|-------|--------|------|
| Prepaid remaining | GET | `/v1/billing/teams/{team_id}/prepaid/balance` |
| Postpaid invoice preview | GET | `/v1/billing/teams/{team_id}/postpaid/invoice/preview` |
| Usage series | POST | `/v1/billing/teams/{team_id}/usage` (`analyticsRequest`, default 7-day window) |
| Team id discovery | GET | `/auth/management-keys/validation` |

Bearer is `XAI_MANAGEMENT_API_KEY` / config / secret store, not the SuperGrok session JWT. Team id: config / `XAI_MANAGEMENT_TEAM_ID` / validation cache (1h).

## 3. Existing background TTL (60s Management class)

`CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS = 60` in `xai_management.rs`. Alias `CONSOLE_TEAM_PREPAID_CACHE_TTL_SECS` is the same window.

- Process caches for prepaid, postpaid, and usage series each honor 60s.
- Background TUI policy: `ManagementMeterCachePolicy::HonorProcessTtl` (do not clear).
- Explicit `grok limits` collect: `ForceRefresh` → `clear_console_team_billing_meter_caches()` then fetch.
- TUI `/limits` open is **documented** as ForceRefresh; the helper exists (`management_meter_cache_policy_for_explicit_limits_open`) but `dispatch_show_limits` does not call the clear today.

SuperGrok included remember maps have **no** TTL. They last until process exit, `clear_included_billing_cache`, or auth-fail demote (`usage_pct` / `reset_at` cleared; extras and Build % kept).

Shared 429 flock (`grok-rate-limit` under `$GROK_HOME/rate_limits/`) is a cooldown, not a meter snapshot.

## 4. How sibling identities are fetched

1. `load_supergrok_billing_poll_targets(grok_home)` reads `$GROK_HOME/auth.json`. SuperGrok session modes only. Dedupes by `supergrok_identity_id_from_auth` (team_id → user_id → scope). Prefers multi-slot scopes (`::personal` / `::team::`) over a duplicate base. Yields `SupergrokBillingPollTarget { identity_id, access_token, user_id }` (Debug redacts the token).
2. `load_non_active_supergrok_billing_poll_targets` drops the active id (`active_supergrok_identity_id`) and skips identities with `SIBLING_BILLING_AUTH_FAIL_SKIP_THRESHOLD` (3) consecutive auth-class poll fails. Empty when only one principal exists.
3. `poll_and_remember_non_active_supergrok_included_billing` is best-effort: OIDC refresh via `ensure_fresh_access_token_for_supergrok_billing_poll` when the JWT is past the early-invalidation buffer and refresh credentials exist; then the same credits GET; remember included / extras / Build / poll outcome; append poll history. Failures do not fail the active ACP billing response.

CLI collect polls **all** targets (active + siblings) with stored tokens. It does **not** run the sibling OIDC refresh helper. Hub should prefer the ACP sibling path (refresh then poll) for every principal.

## 5. Suggested call sites for `limits_snapshot_hub.rs`

New module (does not exist): `crates/codegen/xai-grok-shell/src/auth/limits_snapshot_hub.rs`. Declare in `auth/mod.rs` next to `included_poll_history` / `xai_management`. Copy flock style from `included_poll_history.rs` (`fs2::FileExt::lock_exclusive`) or `grok-rate-limit` `store.rs`.

Suggested durable path: `$GROK_HOME/limits_snapshot/latest.json` (no secrets: identity ids, percents, cents, period strings, timestamps only).

**Leader (one process fetches):**

1. `handle_get_billing` after auth + proxy resolve: try hub acquire. On win, fetch credits for **active + siblings** (reuse `load_supergrok_billing_poll_targets` + sibling OIDC refresh) and, when a management key exists, prepaid + postpaid + series (`fetch_console_team_*_at` or `_default`). Write flock snapshot, then apply locally.
2. `collect_limits_report_at` should call the same hub entry instead of its own per-target HTTP plus Management trio. Honor ForceRefresh by busting hub freshness (and existing 60s Management process caches) on explicit collect.

**Followers (apply only):**

- Same two call sites on flock miss / snapshot still fresh: deserialize and apply into the **same** remember maps:
  - `remember_supergrok_included_billing`
  - `remember_supergrok_dollar_extras`
  - `remember_supergrok_build_usage`
  - `remember_supergrok_billing_poll_ok` / `_failed`
  - `record_included_poll_history_from_config` or `record_included_poll_sample` when the snapshot carries samples
  - Management: `remember_prepaid` / `remember_postpaid` / `remember_usage_series` are private; either export thin apply helpers from `xai_management.rs` or have the hub write those caches through new `pub` apply functions. Do not invent a second map.

**Keep HTTP inside existing fetchers.** Hub should orchestrate + flock, not duplicate reqwest. `fetch_credits_config_with_session` stays the only credits GET.

**Do not** apply exhaust memos on the CLI collect path (current comment: read-only report). Hub apply on ACP `handle_get_billing` may still call `apply_billing_usage_to_session_exhaust_with_period` after maps are filled.

**Background vs explicit:** reuse `ManagementMeterCachePolicy`. HonorProcessTtl → serve hub snapshot if younger than 60s (Management class). ForceRefresh → leader refetch.

## 6. How tests mock HTTP today

Pattern is **in-process axum**, not mockito/wiremock, for billing and Management.

- `tokio::net::TcpListener::bind("127.0.0.1:0")` + `axum::serve`.
- `Arc<AtomicUsize>` hit counters (`hits.fetch_add`).
- Routes:
  - Credits: `GET /billing` (query `format=credits` is on the client URL; axum matches path). JSON fixture: `{"config":{"creditUsagePercent":…,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","end":…}}}`. Bearer string selects distinct % per token (see `dual_poll_remembers_distinct_pct_per_token_never_cross_paints` in `allowance_exhaust_from_billing.rs`).
  - Prepaid: `GET /v1/billing/teams/{team_id}/prepaid/balance` with `{"total":{"val":"-12500"}}`.
  - Postpaid / series: same style in `xai_management.rs` tests (`fetch_prepaid_balance_hermetic_mock_returns_cents`, postpaid and series cache tests). Second call asserts `hits == 1` (60s cache); after `clear_console_team_billing_meter_caches`, `hits == 2`.
- Isolation: `TempDir` as grok home + write `auth.json`; `serial_test::serial` because remember maps and Management caches are process-global; `clear_included_billing_cache` / `clear_console_team_*` in setup/teardown.
- Injectable Management base: `fetch_console_team_*_at(base_url, key, team_id)` so tests never hit `management-api.x.ai`.
- Credits tests pass `http://{addr}` as `proxy_base` into `fetch_credits_config_with_session` / sibling poll.

Hub tests should keep this axum + counter style: two “processes” as two hub handles on the same temp `GROK_HOME`; leader increments hits; follower apply fills `included_billing_fields_snapshot()` with **zero** extra hits.

## 7. GROK_HOME / grok home paths

Canonical resolver: `xai_grok_config::grok_home()` in `crates/codegen/xai-grok-config/src/paths.rs`.

- `$GROK_HOME` if set, else `~/.grok` (`default_grok_home`, dunce-canonicalized).
- `OnceLock`; creates the directory.
- `user_grok_home()` is `Some` only when `GROK_HOME` or a home dir exists (no cwd-relative fallback).

Shell alias: `crate::util::grok_home::grok_home()` via `xai_grok_shell_base` re-export. Used by `handle_get_billing` and `collect_limits_report`.

`included_poll_history::grok_home_path()` re-reads `GROK_HOME` every call (not OnceLock). Prefer that pattern in the hub so tests can point `GROK_HOME` at a TempDir.

On-disk homes the hub will sit beside:

| Path | Role |
|------|------|
| `$GROK_HOME/auth.json` | SuperGrok principals / sibling tokens |
| `$GROK_HOME/included_poll_history/{identity}.json` | flock poll-history ring |
| `$GROK_HOME/rate_limits/` | flock 429 cooldowns |
| `$GROK_HOME/exhausted_credits/` | out-of-included-period memo (1h) |
| `$GROK_HOME/config.toml` | proxy URL, management key/team, `auto_use_included_limits` |
| `$GROK_HOME/limits_snapshot/` | **proposed** hub snapshot (new) |

## Slice D insertion summary

Today two independent HTTP storms can run: TUI ACP credits (active + siblings) and CLI `grok limits` (all credits + Management trio). Remember maps and Management caches are per-process. Insert the hub so **one** leader performs those GETs/POSTs, writes a flock snapshot, and every process (including the leader) applies into `remember_supergrok_included_billing` and the Management remember caches. Followers must not hit `billing?format=credits` or `management-api.x.ai` while the snapshot is fresh.
