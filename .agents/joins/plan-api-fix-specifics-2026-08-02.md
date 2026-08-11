# Research join: practical fix via actual API calls (limits-first / console Usage)

**Date:** 2026-08-02
**Mode:** read-only product + docs + prior joins. No product code edits. No secrets.
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Companion plan section:** [`.agents/plans/limits-first-api-fix-section-2026-08-02.md`](../plans/limits-first-api-fix-section-2026-08-02.md)

## One-line

**Limits-first is already SuperGrok-session path (Design A strips ApiKey while included headroom); console team Usage $ still moves because SuperGrok OAuth → Grok Build OAuth class bills team $, and product has no client API that can re-route that settlement onto included weekly. Fixable client work is observe / honesty / rank / postpaid attribution polls — not inventing a server debit switch.**

---

## Prior evidence (do not re-litigate)

| Artifact | Finding |
|----------|---------|
| `live-auth-path-now-2026-08-02.md` | Live: `supergrok_session`, `console.isLive=false`, included 65%, extras $100.29, prepaid $340 |
| `console-burn-one-turn-investigation-2026-08-02.md` | +$0.01 Usage while SessionToken→proxy; postpaid ~$201 OAuth vs ~$6 API key |
| `console-bypass-paths-code-audit-2026-08-02.md` | Design A coverage holes; flag default off; image/voice public API |
| `console-api-usage-547-evidence-2026-08-02.md` | F1b: team Usage $547.87 proven; F1a included debit still unproven |
| `plan-limits-first-ideal-2026-08-02.md` + plan | C1–C7 ideal; ordered slices prove debit before hop rewrite |
| `RESIDUAL.md` §4 | Field maps for SuperGrok credits + Management prepaid; postpaid not wired |
| `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` | Documented Management endpoints (prepaid, usage POST, postpaid, invoices) |

---

## 1. Every HTTP API product already calls (or should call)

### A. SuperGrok session / cli-chat-proxy (included-safe + inference)

Base default: `https://cli-chat-proxy.grok.com/v1`
(`xai-grok-env` `cli_chat_proxy_base_url`; override `GROK_CLI_CHAT_PROXY_BASE_URL` / `[endpoints] cli_chat_proxy_base_url`).

| # | Method + URL template | Auth | Product call site (file:line) | Wire fields that matter | Role in fix |
|---|----------------------|------|-------------------------------|-------------------------|-------------|
| **S1** | `GET {proxy}/billing?format=credits` | `Authorization: Bearer {session JWT}` + `X-XAI-Token-Auth` (token header name from `GrokComConfig`) + `x-userid` + `x-grok-client-version` + client-mode | `extensions/billing.rs` **L231–274** (`fetch_credits_config_with_session`); active handler **L431–516** | Nested under `config` (camelCase): **`creditUsagePercent`** (f64), **`prepaidBalance.val`** (i64 cents = SuperGrok $ extras), **`currentPeriod`** (`type` e.g. `USAGE_PERIOD_TYPE_WEEKLY`, `start`, `end` RFC3339), **`productUsage[]`** (`product` e.g. `PRODUCT_GROK_BUILD`, `usagePercent`), **`onDemandCap`/`onDemandUsed`**, **`isUnifiedBillingUser`**, history. Top response: **`subscriptionTier`** (from remote settings enrich, not always wire), **`onDemandEnabled`**. | **Observe** headroom; **rank** included; **gate** Design A; **prove debit** via time series; **honesty** when flat |
| **S2** | `GET {proxy}/auto-topup-rule` | Same SuperGrok Bearer headers | `billing.rs` **L518–573** | Rule: `enabled`, `minBeforeHittingSl`, `topupAmount`, `maxAmountPerMonth` | Observe auto-topup policy only (not wallet total). Not primary for limits-first debit. |
| **S3** | Inference: `POST {proxy}/chat/completions` (and related stream paths) | Session Bearer (`auth_type=SessionToken` in logs) | Sampler client / `agent/config.rs` resolve → `cli-chat-proxy` when SuperGrok primary; default base **L51** vs proxy env | 200 success does **not** return included %; 402/credit bodies drive hop; 429 drives rate-limit hop | **Burn path.** Does not prove included absorption client-side. Server may settle as team OAuth $ (F1b). |
| **S4** | Sibling poll: same as S1 for non-active SuperGrok principal | Sibling JWT + user_id | `billing.rs` **L280–338** `poll_and_remember_non_active_supergrok_included_billing` | Same S1 fields per `identity_id` | Dual `/limits` + rank multi-principal headroom |
| **S5** | Settings / models / bundle (not meters) | Session | e.g. `GET /v1/settings`, models catalog, subagent bundle | Remote settings may fill `subscription_tier_display` into billing enrich | Tier label only |

**Log line (success S1):** unified `billing: fetched credits config` with nested camelCase config + top-level snake_case `identity_id`, `role`, `grok_build_usage_percent` when Build product present (`billing.rs` **L378–409**, **L505–513**).

**Process cache after S1:**
`remember_active_supergrok_included_billing` / `remember_supergrok_included_billing` / `remember_supergrok_dollar_extras` in `allowance_exhaust_from_billing.rs` **L39–88**, **L95–105**. Ranked via `included_remaining_from_usage_pct` (`usage_pct >= 100` → 0 remaining) in `supergrok_identity_rank.rs` **L70–77**.

---

### B. Management API (team prepaid / attribution class)

Base: `https://management-api.x.ai`
Auth: `Authorization: Bearer {management_key}`
Key resolve: config `[endpoints] management_api_key` → env `XAI_MANAGEMENT_API_KEY` → keyring URL `https://management-api.x.ai`
Team id: config `[endpoints] management_team_id` → env `XAI_MANAGEMENT_TEAM_ID` → validation discovery.

| # | Method + URL | In product? | Call site | Key response fields | Role in fix |
|---|--------------|-------------|-----------|---------------------|-------------|
| **M1** | `GET /auth/management-keys/validation` | **Shipped** | `xai_management.rs` **L45**, **L454–532** | `teamId` / `scopeId` / `scope` / `name` / `apiKeyId` | Discover team id; refuse inference key (401) |
| **M2** | `GET /v1/billing/teams/{team_id}/prepaid/balance` | **Shipped** | `xai_management.rs` **L40–77**, **L665–745**; cache TTL **60s** **L47–48** | `total.val` (string cents, often negative remaining); `changes[]` with `changeOrigin` (`PURCHASE`/`SPEND`/…) | **Observe** console prepaid ledger only. Live dogfood: 0 SPEND while Usage $ large → **not** the pain meter. |
| **M3** | `GET /v1/billing/teams/{team_id}/postpaid/invoice/preview` | **Documented + operator dogfood dumps; NOT wired in product HTTP client** | Docs only: `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` **L81**; residual live fields | Live capture class: period spend / **`defaultCreditsIssued`**, line items distinguishing **Grok Build OAuth** vs **API** models, **`defaultCredits`**, soft spending limit | **Attribution gate:** OAuth vs ApiKey class. Best client API for “did SuperGrok session burn team $?” after a window. |
| **M4** | `GET /v1/billing/teams/{team_id}/postpaid/spending-limits` | **Documented; not wired** | research md **L82** | Soft/hard postpaid limits | Observe caps; not debit proof |
| **M5** | `GET /v1/billing/teams/{team_id}/invoices` | **Documented; operator dump only** | research md **L83** | Invoice list (e.g. SuperGrok Heavy for Orgs $300 seat) | License history; not live Usage tick |
| **M6** | `POST /v1/billing/teams/{team_id}/usage` body `analyticsRequest` | **Documented; not wired** | research md **L59–75**; residual §4 optional series | `timeSeries[]` group/dataPoints for `usd` etc. | **Optional series** / group-by description for OAuth vs API if dogfood needs charts; not required for C4 SuperGrok included |
| **M7** | Mutating: prepaid top-up, billing-info, payment-method | Documented; out of scope | research md | — | No fix path |

**Important:** Management polls do **not** create inference Usage $ (console-burn join). They only observe team billing surfaces.

---

### C. Console public inference (`api.x.ai`)

| # | Method + URL | Auth | Call site | Role |
|---|--------------|------|-----------|------|
| **C1** | `POST https://api.x.ai/v1/chat/completions` (etc.) | `Authorization: Bearer {console ApiKey}` | Dual-auth hop when console primary/failover; default base `XAI_API_BASE_URL_DEFAULT` `agent/config.rs` **L51** | Direct **API-class** team Usage burn |
| **C2** | Imagine/edit/video: always `endpoints.xai_api_base_url` (default api.x.ai) | Live sampling bearer (session JWT **or** key) | `agent_ops` image/video prepare (audit join) | Possible Usage Image/Video; week tiny |
| **C3** | Voice STT `api.x.ai` / `wss://api.x.ai/v1/stt` | AuthManager / key | voice auth | Voice bucket tiny |
| **C4** | **No** team prepaid on inference key | — | research md **L89–91** | Cannot observe prepaid with `XAI_API_KEY` |

---

### D. Auth / OIDC token refresh (not billing meters)

| # | Flow | Method + URL | Auth | Call site | Role |
|---|------|--------------|------|-----------|------|
| **A1** | OIDC discovery | `GET {issuer}/.well-known/openid-configuration` (standard) | none | `auth/oidc/protocol.rs` `discover`; refresh uses `token_endpoint` | Locate token endpoint |
| **A2** | Refresh tokens | `POST {token_endpoint}` `grant_type=refresh_token` | client_id + refresh_token body | `auth/oidc/refresh.rs` **L95–122** `refresh_tokens` | Keep SuperGrok JWT live so S1/S3 stay on session path |
| **A3** | Device code login | POST device + token poll | public client | `auth/device_code.rs` | Initial SuperGrok login |
| **A4** | External binary refresh | subprocess | external | `auth/external_auth.rs` | Alternate session refresh |

Issuer in dogfood class: `https://auth.x.ai` (tests/external). Refresh is **necessary** for C1 auth path survival; does **not** move included %.

---

### E. OpenRouter (unrelated to console.x.ai Usage)

`FetchBilling` also fetches OpenRouter credits in parallel (`effects/mod.rs` **L4210–4218**). Different vendor; ignore for console Usage.

---

## 2. Headroom, debit, attribution: exact fields and thresholds

### Included headroom (Design A / rank)

| Source field | Threshold in product | Code |
|--------------|----------------------|------|
| `config.creditUsagePercent` | `< 100` → headroom; `>= 100` → included exhausted | `included_remaining_from_usage_pct` **L70–77** `supergrok_identity_rank.rs` |
| Remaining units for rank | `floor(100 - pct).max(1)` when pct &lt; 100 | same |
| Memo exhaust | sticky fingerprint can force remaining 0 even if % &lt; 100 | `apply_included_billing_to_headroom` memo_exhausted; sampler durable exhaust |
| `productUsage[PRODUCT_GROK_BUILD].usagePercent` | **Not** used for rank today; observability / honesty only | `PRODUCT_GROK_BUILD` constant **L77** `billing.rs` |
| SuperGrok `prepaidBalance.val` | **Ignored by rank** (extras not included) | rank module comment **L44–46** |

**Design A strip console:** `order_credentials_for_preferred_auto` when any SuperGrok `included_remaining > 0` → SuperGrok tokens only, **console omitted** (`supergrok_identity_rank.rs` **L356–388**). Requires `auto_use_included_limits=true` and preferred not `api_key` (`preferred_uses_supergrok_auto_rank` **L438–444**).

### Detect “included debit proven” (client)

Requires **time series of S1** samples per `identity_id`, not a single poll:

| Signal | Field | Pass for C4 |
|--------|-------|-------------|
| Top-level included steps | `creditUsagePercent` delta &gt; 0 under load | Strong |
| Build product steps | `productUsage` Build `usagePercent` delta | Strong (even if top-level flat) |
| Extras drop while included full | `prepaidBalance.val` drop after % ≥ 100 | After-burner path |
| All flat under real SessionToken load | — | **Unproven debit** → set `flat_poll_unproven_debit` |

Today: process cache holds **latest** snapshot only; **no** rolling poll history; `flat_poll_unproven_debit` only test-set (plan F2).

### Detect console burn attribution (Management)

| Class | How to detect with APIs | Product today |
|-------|-------------------------|---------------|
| **OAuth Grok Build $** (session proxy traffic settlement) | M3 postpaid line items ~Grok Build OAuth; M6 series groupBy description; browser Usage “Grok Build” | Operator dumps only; not auto |
| **ApiKey $** | M3 API model lines; live logs `ApiKey` + `api.x.ai`; `console.isLive` | Partial: isLive + logs; not postpaid |
| **Prepaid ledger SPEND** | M2 `changes[]` `changeOrigin=SPEND` | Parsed only for balance cents; history ignored for UI |
| **defaultCredits / free pool** | M3 `defaultCredits` / `defaultCreditsIssued` vs prepaid $ | Documented in residual; not wired |

**Live dogfood truth:** postpaid ~97% OAuth class while Design A + `isLive=false` → **ApiKey strip succeeded; team $ still moved via OAuth.**

---

## 3. Practical call graphs (not vague honesty)

### G1 — Before each main sampling turn (resolve order)

```
1. Load SuperGrok candidates from auth.json (tokens; hard-expire filter)
2. Read process cache INCLUDED_BILLING_BY_IDENTITY (last S1)
   - if cold: treat remaining as memo 0|1 default (do NOT invent %)
3. If auto_use_included_limits && preferred != api_key:
     order_credentials_for_preferred_auto(sessions, console_keys)
   - headroom → SuperGrok primary, console OMITTED
   - ExhaustedAll → console primary (gap vs ideal C5 extras-before-console)
4. Sticky exhaust: prefer_live_identity_after_credit_exhaust may still force console
5. Build SamplerConfig (proxy host vs api.x.ai from primary type)
6. Optional: if last S1 older than TTL (suggest 30–60s) queue silent S1
   - TODAY: S1 mainly via Effect::FetchBilling (session start, turn end, /limits, cold prepaid)
   - NOT strictly before every turn
```

**Recommended product change (smallest):** keep resolve as-is; **do not** add a blocking S1 before every turn (latency). Use process cache from last turn-end poll; optional soft-async refresh if cache &gt; 60s.

### G2 — During turn (inference)

```
POST cli-chat-proxy /chat/completions  (SessionToken)     [desired]
  OR after hop: POST api.x.ai /chat/completions (ApiKey)  [should not under Design A headroom]
Imagine/STT may hit api.x.ai regardless (edge)
```

No client meter update mid-stream from S1.

### G3 — After turn (prove debit + attribution)

```
parallel:
  S1  GET …/billing?format=credits          → creditUsagePercent, productUsage, prepaidBalance
  M2  GET …/prepaid/balance                 → total.val (60s cache; last-good on fail)
  [NEW optional] M3 GET …/postpaid/invoice/preview  → OAuth vs API line totals snapshot
  [NEW optional] if window closed: M6 POST …/usage for hour bucket sum usd by group

pure:
  append poll sample to process history (identity_id, ts, pct, build_pct, extras_cents)
  detector: flat under min_polls + min_window + known inference_done volume?
  if usage_pct >= 100 && dual-auth → mark exhaust (existing preemptive path)
  honesty flag → limits snapshot / doctor
```

**Delta fields for “this turn debited included”:**

| Field | Before turn T | After turn T | Meaning |
|-------|---------------|--------------|---------|
| `creditUsagePercent` | 65.0 | 65.x+ | Top-level included moved |
| Build `usagePercent` | 54.0 | 54.x+ | Build slice moved |
| SuperGrok `prepaidBalance.val` | 10029 | lower | Extras burn (usually after included full) |
| M2 prepaid | 34000 cents abs | lower + SPEND row | Console prepaid ledger (rare vs OAuth) |
| M3 OAuth line total | X | X+δ | Team $ via OAuth class (proves F1b-class) |
| M3 API line total | Y | Y+δ | True ApiKey / public API key burn |

### G4 — Controlled dogfood window (operator + product)

```
t0: S1 + M2 + M3 snapshot → save
run N SuperGrok turns only (Design A on, unset host XAI_API_KEY for other tools)
t1: S1 + M3 again
compare deltas → C4 / F1a / OAuth attribution
optional freeze product 30–60m + M3 again → other clients?
```

---

## 4. Smallest product changes that wire API results into resolve + honesty

Ordered by leverage; each is API-backed, hermetic-testable.

### Slice A — Poll history + flat-poll honesty (no new HTTP)

| Piece | Detail |
|-------|--------|
| What | Ring buffer of S1 fields already fetched |
| Where | New pure helper near `allowance_exhaust_from_billing` or `billing.rs`; wire from `handle_get_billing` after remember |
| Wire | Set `LimitsSnapshot.flat_poll_unproven_debit` from detector (closes F2) |
| Rank | Unchanged |
| Tests | Mock in-memory samples; no network |

### Slice B — Log poll_delta on S1 success

| Piece | Detail |
|-------|--------|
| What | Compare last vs current `creditUsagePercent` / Build % / extras; unified log `poll_delta` |
| Where | `billing_fetched_credits_log_ctx` or adjacent |
| Tests | Fixture two BillingConfigResponse bodies |

### Slice C — Wire Management postpaid preview (M3) as **attribution meter**

| Piece | Detail |
|-------|--------|
| What | Hermetic client sibling of prepaid: `GET …/postpaid/invoice/preview` |
| Auth | Same management key + team_id |
| Surface | Optional `limits --json` fields e.g. `console.postpaidOauthUsd` / `console.postpaidApiUsd` (plain names) — **distinct** from prepaid $N |
| Use | Doctor note: “team postpaid is mostly Grok Build OAuth while SuperGrok live” when OAuth >> API under session path |
| Rank | **Do not** fold into Design A remaining; OAuth $ does not mean switch to ApiKey |
| Tests | Mock JSON bodies with line items |

### Slice D — After-burner order (C5) — policy, not new API

| Piece | Detail |
|-------|--------|
| What | When ExhaustedAll **but** SuperGrok `prepaid_balance_cents > 0`, keep SuperGrok session primary before console |
| APIs used | S1 `prepaidBalance` already in process cache |
| Risk | May burn SuperGrok $ extras (intended after-burner); operator may prefer console instead — flag or keep config |
| Tests | Pure `order_credentials_for_preferred_auto` with exhausted included + prepaid &gt; 0 |

### Slice E — Optional M6 series (only if dogfood needs charts)

POST analytics; residual already parks this behind need.

### Slice F — Do **not** claim client fix for OAuth→Usage coupling

No HTTP call in this client reassigns server settlement from “team defaultCredits / OAuth line” to “SuperGrok included weekly %”. Upstream billing attribution.

---

## 5. Red tests (mock HTTP bodies / pure fixtures)

Prefer existing patterns: axum hermetic mocks in `xai_management.rs` tests; `BillingConfig` serde fixtures in `billing.rs` tests.

| Test (suggested name) | Contract | Fixture |
|-----------------------|----------|---------|
| `poll_history_marks_flat_when_included_and_extras_unchanged` | N identical S1 samples → unproven true | Pure samples 65.0 / 10029 |
| `poll_history_clears_flat_when_included_pct_steps` | 65.0→66.0 → unproven false | Pure |
| `poll_history_clears_flat_when_build_product_usage_steps` | Top flat, Build 54→55 → unproven false | `productUsage` entries |
| `poll_history_clears_flat_when_extras_cents_drop` | extras drop → unproven false | prepaidBalance |
| `limits_snapshot_sets_flat_poll_from_history_not_only_tests` | Collect path sets flag without test-only setter | Unit |
| `fetch_prepaid_balance_hermetic_mock_returns_cents` | **Existing** M2 mock | `xai_management.rs` **L1332+** |
| `fetch_postpaid_preview_parses_oauth_vs_api_line_totals` | M3 mock splits OAuth vs API | New mock body |
| `auto_order_omits_console_while_any_supergrok_included_headroom` | **Existing** Design A | rank tests ~L834 |
| `auto_order_keeps_supergrok_when_included_full_but_extras_remain` | C5 after-burner | New pure order test |
| `billing_fetched_credits_log_ctx_includes_identity_and_build_product_usage` | **Existing** | billing.rs tests |
| `included_remaining_from_usage_pct_100_is_zero` | Threshold | pure |

**Ban:** host D-Bus, live management key in CI, rewriting asserts to match flat server without named contract.

---

## 6. What client APIs **cannot** fix

| Problem | Why client cannot fix |
|---------|----------------------|
| SuperGrok OAuth session traffic posts **team Usage $** (Grok Build OAuth / Text aggregation) while included % flat | Settlement is server-side on xAI billing. S1 only **reports** included %; it does not control which ledger absorbs proxy tokens. |
| Force included weekly to move when server pool is coarse / lagging / wrong principal | Only S1 can observe; no PATCH “debit included”. |
| Make console Grok Business **licenses** Usage chart move | Different product surface (seat messages); SuperGrok CLI never drives it. |
| Read prepaid remaining with inference `XAI_API_KEY` | No endpoint on `api.x.ai` for team prepaid. |
| Stop **other** machines/tools on same team key | Client only controls this process’s resolve chain. |
| Merge ~$1317 dashboard composite into prepaid $340 | Different surfaces (defaultCredits composite vs prepaid ledger). |
| Design A alone stop Usage $ | Design A only strips **ApiKey** from chain; OAuth class continues. |

**Honest product contract after API wiring:**

1. While included headroom: stay on SuperGrok proxy; **no ApiKey** in chain (Design A).
2. UI states: included % is poll, not proven burn; optional flat-window note from poll history.
3. Optional: team postpaid OAuth vs API split so operator sees **where dollars land**.
4. After included full: prefer SuperGrok $ extras before console (if C5 accepted).
5. Upstream ticket if OAuth→team $ with unused included is a billing bug.

---

## 7. Call-site index (quick)

| Concern | Primary files |
|---------|---------------|
| S1 / S2 SuperGrok billing | `crates/codegen/xai-grok-shell/src/extensions/billing.rs` |
| M1 / M2 management | `crates/codegen/xai-grok-shell/src/auth/xai_management.rs` |
| Included cache + preemptive exhaust | `…/auth/allowance_exhaust_from_billing.rs` |
| Design A order | `…/auth/supergrok_identity_rank.rs` |
| Resolve dual-auth | `…/agent/config.rs` (~L5097–5344) |
| FetchBilling effect (S1+M2 parallel) | `…/pager/src/app/effects/mod.rs` ~L4205+ |
| limits CLI / honesty flag | `…/pager/src/limits_cmd.rs`, `views/limits_honesty.rs` |
| OIDC refresh | `…/auth/oidc/refresh.rs`, `protocol.rs` |
| Documented M3–M6 | `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` |

---

## 8. Recommended implement order (API-backed)

1. **Slice A+B** (poll history + log delta + wire flat flag) — uses S1 only; proves C4 measurability / F2.
2. **Dogfood G4** with live S1 + optional operator M3 curl dumps.
3. **Slice C** (M3 postpaid preview) if operator wants automated OAuth vs API attribution.
4. **Slice D** (extras before console) only after operator accepts C5 burn-extras policy.
5. **Slice E** M6 only if charts needed.
6. **Never** “fix” by forcing more ApiKey traffic while included has headroom.

---

## Status

Research complete for plan merge. No product edits in this join.
