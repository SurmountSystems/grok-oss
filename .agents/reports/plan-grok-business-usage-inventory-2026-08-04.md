# Inventory: console Grok Business Usage vs product meters

**Date:** 2026-08-04
**Mode:** read-only (code, residual, research, prior joins)
**Operator pain:** console.x.ai → team Surmount → Platforms → **Grok Business** → **Usage** (messages / conversations charts) all zeros / "No data available" for Jul 28–Aug 4 2026. After billing/limits work they still do not see that page move.

**Prior SoT (do not reinvent):**

| Path |
|------|
| [`doc/dev/research/console-team-business-usage-meter-2026-07-30.md`](../../doc/dev/research/console-team-business-usage-meter-2026-07-30.md) |
| [`.agents/joins/business-usage-vs-product-path-2026-08-02.md`](../joins/business-usage-vs-product-path-2026-08-02.md) |
| [`.agents/joins/live-auth-path-now-2026-08-02.md`](../joins/live-auth-path-now-2026-08-02.md) |
| [`.agents/joins/console-burn-one-turn-investigation-2026-08-02.md`](../joins/console-burn-one-turn-investigation-2026-08-02.md) |
| [`.agents/joins/console-api-usage-547-evidence-2026-08-02.md`](../joins/console-api-usage-547-evidence-2026-08-02.md) |
| `RESIDUAL.md` §4 (two halves + meter map); `FORK.md` billing meters bullet |

---

## One-line answer

**The screenshot is Grok Business *license* seat usage (messages / conversations / active users). grok-oss does not drive that page. SuperGrok session dogfood and console API spend land on different meters (SuperGrok included % / extras, team API Usage $, Management prepaid/postpaid/series). Zeros on license charts are expected and do not prove Heavy or dual-auth is idle.**

---

## Meter taxonomy (keep distinct)

| # | Plain name | What it is | Credential / host | Product today |
|---|------------|------------|-------------------|---------------|
| **1** | SuperGrok **included weekly** | Free SuperGrok period allowance used % (`creditUsagePercent`) | OIDC JWT → `cli-chat-proxy.grok.com` `GET …/billing?format=credits` | Status bar `%`, `/limits`, `/usage` |
| **2** | SuperGrok **dollar extras** | Extra Usage Credits (`prepaidBalance.val` cents) | Same session billing | `/limits` SuperGrok $ extras; footer when included ≥ 100% |
| **3** | Grok Build **product usage %** | Wire `productUsage` (e.g. GrokBuild 54%) | Same session billing | `/limits` / `/usage` line when observed |
| **4** | **Console team prepaid** | Prepaid ledger remaining | Management key → `management-api.x.ai` `GET …/prepaid/balance` | Footer when **console key live**; `/limits` Balance; honest gaps |
| **5** | **Console team postpaid** OAuth vs API class | Period invoice preview (Usage $ class) | Same Management key `GET …/postpaid/invoice/preview` | `/limits` Team postpaid lines + default credits line |
| **6** | **Management usage series** | Day-window **USD** by description (OAuth vs API class) | Same key `POST …/usage` analytics | Explicit `grok limits` / limits JSON (not license msg counts) |
| **7** | **Console API Usage $** (browser) | Team inference / OAuth settlement dollars | Team-wide browser `…/usage` (not licenses) | Browser only; product surfaces postpaid/series, not full chart UI |
| **8** | **Grok Business licenses** messages / conversations | Seat/license product on Platforms → Grok Business → Usage | Browser `…/grok-business/usage` (session cookie) | **Not wired. No client. Not claimed.** |
| **9** | Second SuperGrok OIDC principal | Personal vs business login roles | Still meters **1–3**, not **8** | Dual `/limits` rows; role label `business` |

**Naming trap:** residual says "console Grok Business Usage **class**" for Half B = team **API** prepaid / postpaid / USD series. The console dropdown **Grok Business** (licenses subtitle, messages/conversations) is meter **8**. Same word "Business"; different products.

---

## What the TUI credit bar / limits chrome shows today

### Status bar (credit bar)

| Surface | Function | Displays |
|---------|----------|----------|
| Compact bar | `credit_bar_line` / `credit_bar_line_for_session` in `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | SuperGrok **included %** only (`65%` or optional linear-burn chip). Loading: `...%`. Not console prepaid. Not license charts. |
| Footer warning strip | `usage_warning_for_session_with_identity_principal_and_gap` | SuperGrok: "% left" near cap, or "SuperGrok extras left: $N" after 100%. **Console live:** `Console key · team prepaid: $N` or honest gap (`no management key`, `no management team id`, `loading team prepaid...`, `team prepaid unavailable`). OpenRouter: separate credits left. |

### `/limits` and `grok limits` / `limits --json`

Built in `limits_snapshot.rs` + `limits_cmd.rs` + honesty notes in `limits_honesty.rs`.

| Block | Content |
|-------|---------|
| Live sampling | SuperGrok session (personal/business) vs console key |
| SuperGrok rows | Included %, period/reset, SuperGrok $ extras, optional Build % |
| Console API | Key status; **Balance** = team prepaid $N or gap; postpaid period + OAuth/API class; team default credits (own line, not prepaid); usage series OAuth/API USD + top description rows (when Management key works) |
| Notes | C6 (session can still move team Usage $); prepaid cache lag; flat-poll unproven debit; etc. |

### Soft `/usage`

- SuperGrok live: included % / reset / pacing / Build % / SuperGrok extras.
- Console live: **console team prepaid** only (not SuperGrok extras as live spend).

### Local Token Economy

- `usage.jsonl` + `/spend` double-entry vs remote Management samples (`token_economy/ledger.rs`, `reconcile.rs`). Still USD class, not license messages.

---

## Is there product UI for console "Grok Business Usage" (messages / conversations)?

**No.**

- No client for `/team/.../grok-business/usage` or any license active-users / messages / conversations API.
- Explicit product comment: shared SuperGrok pool is **also not** "console.x.ai Grok Business license seat/message usage" (`limits_snapshot.rs` `shared_unified_supergrok_pool`).
- Management `POST …/usage` is **USD sum by description** (day buckets), not message/conversation counts. Shown as text totals on `/limits`, not browser-style charts.
- Residual / FORK: do **not** claim full Business Usage **charts** done; series charts UI still optional.

---

## APIs the product actually calls

### SuperGrok (session)

| Call | Path / role |
|------|-------------|
| Credits | `GET {cli-chat-proxy}/billing?format=credits` → `GetGrokCreditsConfig` (`fetch_credits_config_with_session` in `extensions/billing.rs`) |
| Auto-topup | Separate `GET …/auto-topup-rule` (amounts, not wallet invent) |
| Inference | `https://cli-chat-proxy.grok.com/v1` with `SessionToken` |

Key fields: `creditUsagePercent`, `prepaidBalance.val`, `productUsage`, `currentPeriod`, `isUnifiedBillingUser`, `subscriptionTier`.

### Console inference (failover)

| Call | Path / role |
|------|-------------|
| Inference | `https://api.x.ai/v1` with `ApiKey` (`XAI_API_KEY` / store) |
| Team prepaid via this key? | **No** documented balance endpoint on inference host |

### Management (read meters; not inference)

Base: `https://management-api.x.ai`
Module: `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`

| Method | Path | Product type |
|--------|------|--------------|
| GET | `/v1/billing/teams/{team_id}/prepaid/balance` | `ConsoleTeamPrepaidMeter` |
| GET | `/v1/billing/teams/{team_id}/postpaid/invoice/preview` | `ConsoleTeamPostpaidPreview` |
| POST | `/v1/billing/teams/{team_id}/usage` + `analyticsRequest` | `ConsoleTeamUsageSeries` (usd sum, groupBy description, default 7-day window) |
| GET | `/auth/management-keys/validation` | Team id discovery |

Resolve: `[endpoints] management_api_key` / `XAI_MANAGEMENT_API_KEY` / keyring URL `https://management-api.x.ai`; team id config/env/validation. Login: `grok login --management-key`.

**Fetch cadence:** background `FetchBilling` does SuperGrok credits + prepaid + postpaid (TTL ≤60s). **Usage series** is on explicit limits collect (`limits_cmd`), not every background poll.

---

## Could dogfood via grok-oss never populate Grok Business license charts?

**Yes. Strong evidence that is by design, not a missing wire in this tree.**

| Evidence | Source |
|----------|--------|
| No license Usage API client | Grep + `business-usage-vs-product-path` join |
| Live dogfood: SuperGrok business, Heavy, included 65%, Build 54%, console **not** live | `live-auth-path-now-2026-08-02.md` |
| Design A omits console ApiKey while included has headroom | `order_credentials_for_preferred_auto` |
| License zeros called **not a fail mode** for limits-first | `plan-limits-first-ideal`, `plan-api-fix-specifics` ("Make console Grok Business licenses Usage chart move → SuperGrok CLI never drives it") |
| Team **API Usage $** *did* move under SuperGrok OAuth settlement (~$547 week, +$0.01 turn) without license page | `console-burn-one-turn-investigation`, F1b joins |
| Heavy for Orgs $300 is a Management **invoice** entitlement; usage shows on SuperGrok credits, not license message counters | live-auth join §6–7 |

**What *does* move under heavy grok-oss SuperGrok dogfood:**

1. SuperGrok meters 1–3 (if server debits included; C4 still open / flat % pain).
2. Team **postpaid / browser API Usage $** as **Grok Build OAuth** class even when `console.isLive=false`.
3. **Not** Platforms → Grok Business → licenses messages/conversations.

**What would fill license charts (hypotheses only; not in our code):** seated Grok Business **chat product** (or other clients xAI attributes to license seats). Unproven that Business SuperGrok OIDC on cli-chat-proxy ever increments that page.

---

## Gaps vs the operator screenshot (Jul 28–Aug 4, messages/conversations = 0)

| Expectation | Reality in product |
|-------------|-------------------|
| See messages/conversations usage for Grok Business | Product never reads or mirrors that chart |
| "Business" SuperGrok Heavy work should tick that page | Heavy entitlement → SuperGrok credits path; different surface |
| Billing/limits work should fix zeros | Limits work fixed **meters 1–7 honesty and dual-auth**; intentionally not meter **8** |
| TUI shows "same data" as that page | TUI shows SuperGrok % / extras / Management $; not seat message series |

If the operator meant **team API Usage $** (not licenses), product already has postpaid + series text on `/limits` when Management key is set; full browser chart parity is still optional UI work.

---

## Key file / function index

| Area | Path | Names |
|------|------|-------|
| Management client | `crates/codegen/xai-grok-shell/src/auth/xai_management.rs` | `fetch_console_team_prepaid_balance*`, `fetch_console_team_postpaid_preview*`, `fetch_console_team_usage_series*`, `usage_analytics_day_sum_by_description_request`, `classify_postpaid_line` |
| SuperGrok billing | `crates/codegen/xai-grok-shell/src/extensions/billing.rs` | `fetch_credits_config_with_session`, `x.ai/billing` handler |
| Credit bar / footer | `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | `CreditBalance`, `credit_bar_line*`, `usage_warning_*`, `format_usage_summary_*`, `ConsoleTeamPrepaidGap` |
| Limits snapshot | `…/views/limits_snapshot.rs` | `LimitsSnapshot`, `ConsoleMeter`, `format_console`, `ConsoleTeamUsageSeriesSummary` |
| Limits CLI | `…/limits_cmd.rs` | collect prepaid/postpaid/series; JSON fields `teamPrepaidUsd`, `teamPostpaid*`, `teamUsageSeries*` |
| Honesty | `…/views/limits_honesty.rs` | C6 team Usage note, prepaid lag note, Build product line |
| FetchBilling | `…/app/effects/mod.rs`, `effects/helpers.rs` | SuperGrok + prepaid + postpaid into app state / process cache |
| Dual-auth order | `…/shell/src/auth/supergrok_identity_rank.rs` | `order_credentials_for_preferred_auto` |
| Docs | `…/pager/docs/user-guide/02-authentication.md`, `04-slash-commands.md` | Management setup + `/limits` surfaces |
| Research | `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` | Half B endpoints (series was open; **USD series later shipped** Item 5) |

---

## Recommended product directions (2–3 options)

### Option A — Operator education + verify the *right* surfaces (lowest effort)

**Do:** Point dogfood at `grok-oss limits --json` / `/limits` / team **API Usage** (not Grok Business licenses). Confirm Management key + team id. Treat license zeros as non-goals.

**Effort:** docs / chat / residual honesty only (hours).
**Fits:** current evidence that screenshot is meter 8 and product already surfaces 1–7.
**Does not:** make messages/conversations charts non-zero.

### Option B — Stronger TUI mirror of team **API / OAuth Usage $** (medium)

**Do:** Enrich footer or `/limits` with always-visible postpaid OAuth vs API class (and series window) even when SuperGrok is live (today series is limits-explicit; postpaid is cached from billing). Optional sparkline/text day series from existing `POST …/usage` (usd only). Never fold into prepaid $N. Never invent license message counts.

**Effort:** ~1–3 implementer days (TDD on snapshot + FetchBilling policy).
**Fits:** operator pain about **dollars** moving while included % looks flat (F1b).
**Does not:** populate Platforms → Grok Business → licenses charts.

### Option C — Wire license seat Usage into product (high / uncertain)

**Do:** Discover whether xAI documents a Management or other API for Grok Business **license** messages/conversations/active users; only then hermetic client + TUI. No HTML scrape of console.x.ai.

**Effort:** research first (may be blocked: no public API); if exists, multi-day client + UI; if not, **cannot ship**.
**Risk:** inventing endpoints; conflating license seats with SuperGrok Heavy (already bought via invoice HAHH-YYA9-UQ6Q class).
**Only if:** operator explicitly wants that page's data in-product, not "prove Heavy works."

---

## Suggested default for exclusive priority

1. **Close the alarm:** license Usage zeros ≠ SuperGrok Heavy unused (Option A).
2. If they still want "I see team burn in the TUI without opening console," ship Option B (postpaid + series always on SuperGrok-live honesty).
3. Park Option C until a documented license-usage API exists or operator accepts scrape (product law: no scrape).

**Live check commands (no secrets):**

```bash
grok-oss limits --json
# liveSampling, livePrincipalRole, console.isLive, teamPrepaidUsd, teamPostpaid*, teamUsageSeries*
```

---

## Bottom line

| Question | Answer |
|----------|--------|
| What does product show? | SuperGrok included % / extras / Build %; console prepaid when Management works; postpaid OAuth vs API $; USD usage series on limits. |
| Mirror of Grok Business license messages/conversations? | **No.** |
| Why screenshot is still zero after limits work? | Wrong meter for CLI SuperGrok dogfood; product never posts to that surface. |
| Where real dogfood burn shows? | SuperGrok credits; team **API Usage $** / Management postpaid (OAuth Grok Build); not license charts. |
| Next product value | Honesty/docs (A) or SuperGrok-live team $ visibility (B); not inventing license charts (C) without a public API. |
