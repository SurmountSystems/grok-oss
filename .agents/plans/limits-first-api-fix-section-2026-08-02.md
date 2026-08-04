# Practical fix via actual API calls

**Date:** 2026-08-02
**Status:** plan section draft (paste into session plan / limits-first ideal)
**Full research join:** [`.agents/joins/plan-api-fix-specifics-2026-08-02.md`](../joins/plan-api-fix-specifics-2026-08-02.md)

This section is **API-call concrete**: method, path, auth, response fields, when to call, what product does with each field. It does not restate the whole ideal; it is the implementer call graph for limits-first + console Usage pain.

---

## Premise (one paragraph)

Live dogfood is already on **SuperGrok SessionToken → `cli-chat-proxy`**, Design A omits console **ApiKey** while included has headroom, and `console.isLive=false`. Console team **API Usage** still ticks (+$0.01 / ~$548 week) mostly as **Grok Build OAuth** settlement, not live ApiKey hop. Client APIs can **observe** SuperGrok included meters and **attribute** team $ (OAuth vs API); they **cannot** reassign server-side settlement onto included weekly. Implement observe → honesty → rank policy; do not “fix” by forcing more console keys.

---

## Endpoint catalog (product truth)

### Already shipped (use these)

| ID | Method + path | Auth | Key response fields | Product wire-up |
|----|---------------|------|---------------------|-----------------|
| **S1** | `GET {cli-chat-proxy}/billing?format=credits` | SuperGrok JWT Bearer + `X-XAI-Token-Auth` + `x-userid` | `config.creditUsagePercent`, `config.prepaidBalance.val` (SuperGrok $ extras cents), `config.currentPeriod.{type,start,end}`, `config.productUsage[{product,usagePercent}]` (e.g. `PRODUCT_GROK_BUILD`), `config.isUnifiedBillingUser`, `onDemandCap`/`onDemandUsed` | `extensions/billing.rs` `fetch_credits_config_with_session` (~L231); active `x.ai/billing` (~L431); process cache `remember_*` in `allowance_exhaust_from_billing.rs` |
| **S2** | `GET {cli-chat-proxy}/auto-topup-rule` | Same SuperGrok | auto-topup rule amounts/enabled | `billing.rs` ~L518 (policy only) |
| **S3** | `POST {cli-chat-proxy}/…/chat/completions` (stream) | SessionToken | no included % on 200; 402/429 hop | sampler; burn path |
| **M1** | `GET https://management-api.x.ai/auth/management-keys/validation` | Management key Bearer | `teamId` / `scopeId` / `scope` | `xai_management.rs` ~L454 |
| **M2** | `GET https://management-api.x.ai/v1/billing/teams/{team_id}/prepaid/balance` | Management key | `total.val` (cents string, abs = remaining); `changes[]` | `fetch_console_team_prepaid_balance*` ~L665; **60s** process cache |
| **A2** | `POST {issuer token_endpoint}` refresh_token | OIDC client_id + RT | new access/refresh | `auth/oidc/refresh.rs` ~L95 (keeps S1/S3 alive) |

Default proxy base: `https://cli-chat-proxy.grok.com/v1`.
Console inference (not preferred under headroom): `https://api.x.ai/v1` + ApiKey.

### Documented, not wired (add only when a slice needs them)

| ID | Method + path | Auth | Key fields | Why wire |
|----|---------------|------|------------|----------|
| **M3** | `GET …/v1/billing/teams/{team_id}/postpaid/invoice/preview` | Management key | period spend, **`defaultCredits` / `defaultCreditsIssued`**, line items (**Grok Build OAuth** vs **API** class), spending limits | Attribute team $ to OAuth vs ApiKey after dogfood windows |
| **M4** | `GET …/postpaid/spending-limits` | Management key | soft/hard limits | Optional cap display |
| **M5** | `GET …/invoices` | Management key | invoice history (Heavy seats, etc.) | Optional history, not live tick |
| **M6** | `POST …/v1/billing/teams/{team_id}/usage` body `analyticsRequest` | Management key | `timeSeries[]` usd aggregates, groupBy | Charts / hour buckets only if dogfood needs |

Public docs pin: `doc/dev/research/console-team-business-usage-meter-2026-07-30.md`.

---

## Field → decision map

| Decision | Field(s) | Threshold / rule | Code today |
|----------|----------|------------------|------------|
| SuperGrok has **included headroom** | `creditUsagePercent` | **&lt; 100** → headroom; **≥ 100** → included exhausted | `included_remaining_from_usage_pct` `supergrok_identity_rank.rs` ~L70 |
| Design A omit console keys | any SuperGrok `included_remaining > 0` | failover = SuperGrok only | `order_credentials_for_preferred_auto` ~L356–388; needs `auto_use_included_limits=true` |
| Preemptive exhaust → prefer console | same % ≥ 100 + dual-auth ready | mark session fingerprint exhausted | `allowance_exhaust_from_billing` + pager effects helpers |
| SuperGrok $ extras remaining | `prepaidBalance.val` | cents &gt; 0 | process cache; **rank ignores** today |
| Build-specific included | `productUsage` entry `PRODUCT_GROK_BUILD` | independent of top-level % | deserialize + log hoist; not rank |
| Console prepaid remaining | M2 `total.val` abs | display only | footer / limits when console live |
| Team $ OAuth class | M3 OAuth line totals (dogfood ~$201) | delta after SuperGrok-only window | **not wired** |
| Team $ ApiKey class | M3 API lines (~$6 dogfood) + logs ApiKey+api.x.ai | delta | partial (logs / isLive) |
| Included debit **proven** | time series of S1 % / Build % / extras | any upward % or extras drop under load | **history not built**; F2 flat flag test-only |
| Included debit **unproven** | flat series under real SessionToken load | min_polls + min_window | set `flat_poll_unproven_debit` |

---

## Call graphs

### Before main sampling turn (resolve)

```
auth.json SuperGrok candidates
  → process cache from last S1 (usage_pct, reset_at, prepaid extras)
  → if auto_use_included_limits && preferred != api_key:
       order_credentials_for_preferred_auto
         headroom  → primary SuperGrok JWT, console OMITTED
         ExhaustedAll → console primary  [C5 gap: should prefer extras if prepaid > 0]
  → sticky exhaust memo may still force console
  → SamplerConfig host = cli-chat-proxy | api.x.ai
```

**Do not** block every turn on a fresh S1 (latency). Prefer last turn-end poll; optional async refresh if cache older than ~60s.

### After turn / billing refresh (already parallel in `Effect::FetchBilling`)

```
parallel:
  S1  GET …/billing?format=credits
  M2  GET …/prepaid/balance   (60s cache)
  OpenRouter credits (unrelated)

then:
  remember included + extras into process cache
  if usage_pct >= 100 → preemptive exhaust path
  sibling SuperGrok S1 for non-active principal
  [NEW] append poll sample; compute flat_poll / poll_delta log
  [NEW optional] M3 postpaid preview snapshot for OAuth vs API
```

### Prove debit window (dogfood acceptance for C4)

```
t0: S1 + (optional M3) snapshot
run controlled SuperGrok turns only (Design A on; no other clients on team key if possible)
t1: S1 + M3 again

PASS C4 if any of:
  creditUsagePercent increased, OR
  PRODUCT_GROK_BUILD usagePercent increased, OR
  prepaidBalance dropped after included full

FAIL C4 (honest) if all SuperGrok fields flat under load:
  surface flat_poll_unproven_debit; do not claim limits-first burn

Attribution (F1b):
  M3 OAuth total ↑ with SuperGrok-only traffic → OAuth settlement (expected pain)
  M3 API total ↑ with isLive=false → other clients or edge public-API tools
```

---

## Implementation slices (ordered)

### Slice 1 — Poll history + flat honesty (S1 only; no new endpoints)

**Goal:** Make C4 measurable; close F2 (flat note only in tests today).

| Step | Work |
|------|------|
| 1 | Process ring buffer per `identity_id`: `{ts, creditUsagePercent, buildUsagePercent?, prepaidBalanceCents?}` filled on every successful S1 (`handle_get_billing` / limits collect). |
| 2 | Pure detector `included_debit_unproven(samples, min_polls, min_window)` when included % and extras (and optionally Build %) unchanged. |
| 3 | Wire detector → `LimitsSnapshot.flat_poll_unproven_debit` on `/limits` and `limits --json`. |
| 4 | Optional unified log `poll_delta` when any field steps. |

**Acceptance:**

- Heavy SuperGrok dogfood with flat S1 → honesty note appears **without** test-only setter.
- If Build % steps while top-level flat → unproven **false**.
- Design A / resolve order unchanged.

**Red tests (fixtures, no host):**

- `poll_history_marks_flat_when_included_and_extras_unchanged`
- `poll_history_clears_flat_when_included_pct_steps`
- `poll_history_clears_flat_when_build_product_usage_steps`
- `poll_history_clears_flat_when_extras_cents_drop`
- `limits_snapshot_sets_flat_poll_from_history_not_only_tests`

**Files (likely):** `extensions/billing.rs`, `auth/allowance_exhaust_from_billing.rs` (or small pure `included_poll_history` module), `pager/src/limits_cmd.rs`, `views/limits_honesty.rs`.

---

### Slice 2 — Dogfood G4 with live S1 + operator M3 (no code required for M3)

**Goal:** Branch on evidence before hop policy rewrite.

| Step | Work |
|------|------|
| 1 | Rebuild binary with Slice 1. Baseline: `grok-oss limits --json` + log `billing: fetched credits config`. |
| 2 | Window: N SuperGrok turns; capture S1 before/after. |
| 3 | Operator (or later Slice 3 client): `GET …/postpaid/invoice/preview` before/after; compare OAuth vs API lines. |
| 4 | Record: C4 pass/fail; OAuth delta; confirm `console.isLive=false` and SessionToken logs. |

**Acceptance:** Written evidence in a join; plan next branch (server debit lag vs OAuth-only vs extras-early).

---

### Slice 3 — Management postpaid preview client (M3) for attribution

**Goal:** Product can show team **OAuth vs API** dollar class without scraping console HTML.

| Step | Work |
|------|------|
| 1 | Hermetic fetch sibling to prepaid: `GET /v1/billing/teams/{team_id}/postpaid/invoice/preview`, same management key + team_id resolve. |
| 2 | Parse only fields needed for honesty (period totals + OAuth vs API line aggregates). Never invent if body shape differs. |
| 3 | Surface under **console** meter family in `limits --json` / notes — **distinct** from prepaid `$N` and SuperGrok extras. |
| 4 | Doctor / honesty: plain English when SuperGrok live + OAuth postpaid dominates (“session can still move team Usage dollars”). |

**Acceptance:**

- Hermetic mock returns OAuth-heavy fixture → JSON fields match.
- Missing management key → honest gap (same family as prepaid gaps).
- **Does not** change Design A rank (OAuth $ ≠ switch to ApiKey).

**Red tests:**

- `fetch_postpaid_preview_hermetic_parses_oauth_vs_api_totals`
- `postpaid_preview_gap_when_no_management_key`

**Files:** `auth/xai_management.rs` (+ exports), limits collect / honesty copy.

---

### Slice 4 — After-burner order (C5): extras before console (S1 field already cached)

**Goal:** When included exhausted but SuperGrok `prepaidBalance` &gt; 0, stay on SuperGrok session before console.

| Step | Work |
|------|------|
| 1 | Change `order_credentials_for_preferred_auto` ExhaustedAll branch (or adjacent): if any session has `prepaid_balance_cents > 0` (from process cache / candidate fields), primary = SuperGrok session, console failover. |
| 2 | Align doctor/docs: stop claiming extras-before-console if code disagreed (today prefers console). |
| 3 | Operator may want a config kill-switch if extras burn is unwanted — only if dogfood asks. |

**Acceptance:**

- Pure test: included remaining 0, extras 10029 → primary SuperGrok, console in failover not primary.
- Live: included ≥ 100%, extras &gt; 0 → still SessionToken + proxy until extras go or hop policy fires.

**Red tests:**

- `auto_order_keeps_supergrok_when_included_full_but_extras_remain`
- Existing Design A headroom tests stay green.

**Depends on:** operator OK to burn SuperGrok $ extras as after-burner (ideal C5). Park if ambiguous.

---

### Slice 5 — Optional M6 series / Imagine public-API policy

| Item | When |
|------|------|
| **M6** `POST …/usage` charts | Only if dogfood needs token/spend series beyond M3 totals |
| Image/voice always `api.x.ai` | Separate audit; small week $; route only if operator wants zero public API under headroom |

Do not block Slices 1–4 on these.

---

## Explicit non-goals (client cannot)

1. Force SuperGrok included `%` to rise when server keeps it flat under OAuth proxy load.
2. Stop console Usage $ that is **OAuth Grok Build** settlement of SuperGrok session traffic.
3. Move Grok Business **licenses** Usage chart.
4. Read team prepaid with inference `XAI_API_KEY`.
5. Control other processes / host-exported `XAI_API_KEY` burners.

Upstream billing attribution is the only fix for (1)–(2) if product path stays SuperGrok-correct.

---

## Acceptance checklist (operator + CI)

| # | Check | How |
|---|-------|-----|
| A1 | Design A still omits console under headroom | Existing rank unit tests + live `console.isLive=false` |
| A2 | Flat-poll honesty live | Slice 1 + dogfood flat window → note on limits |
| A3 | Debit proven or honestly unproven | S1 series deltas or flat flag |
| A4 | OAuth vs API attribution | M3 (Slice 3) or operator curl; OAuth-dominant under SuperGrok path matches dogfood |
| A5 | After included full, extras before console | Slice 4 tests + live path |
| A6 | Meters stay distinct | SuperGrok included ≠ SuperGrok extras ≠ console prepaid ≠ postpaid OAuth line ≠ licenses |
| A7 | No live secrets in tests | Hermetic mocks / fixtures only |

---

## Suggested next implement prompt (agent)

`/implement` Slice 1 only: process poll history from existing S1 results, pure flat detector, wire `flat_poll_unproven_debit` on limits surfaces, red/green TDD with fixture samples. Do not add M3 or reorder ExhaustedAll until Slice 1 green and dogfood G4 evidence is written.

---

## Critical files (implementer)

| Path | Why |
|------|-----|
| `crates/codegen/xai-grok-shell/src/extensions/billing.rs` | S1/S2 HTTP, remember hooks, log ctx |
| `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` | Included/extras process cache; exhaust |
| `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` | Design A order; C5 after-burner change site |
| `crates/codegen/xai-grok-shell/src/auth/xai_management.rs` | M1/M2 shipped; M3/M6 add site |
| `crates/codegen/xai-grok-pager/src/limits_cmd.rs` + `views/limits_honesty.rs` | CLI/snapshot honesty flag surface |
