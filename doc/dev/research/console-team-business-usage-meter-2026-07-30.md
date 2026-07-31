# Console team Business Usage meter (Half B) — Management API research

**Date:** 2026-07-30
**Status:** research + shipped-core note (no scrape, no invented endpoints). Half B
core prepaid landed same day; in-tree client status below is post-ship honesty.
**Goal:** TUI picture of **console team Grok Business Usage** class data (team prepaid / spend / usage), not SuperGrok session meters.

## Meters stay distinct

| Meter | What it is | Product path today |
|-------|------------|--------------------|
| SuperGrok **included weekly** | Consumer SuperGrok OAuth included pool | Half A shipped (`GetGrokCreditsConfig` / billing) |
| SuperGrok **dollar extras** | Session prepaid extras on SuperGrok | Half A shipped (session billing only) |
| **Console team prepaid / Business Usage** | Team API spend on console product | **Half B core prepaid shipped** (GET balance + TUI); series open |
| Second SuperGrok OIDC principal | Another SuperGrok login (personal/business) | Half A multi-login; **not** console team prepaid |

## Public docs (xAI)

Sources (2026-07-30):

- [Management API overview](https://docs.x.ai/developers/rest-api-reference/management)
- [Billing Management](https://docs.x.ai/developers/rest-api-reference/management/billing)
- [Using Management API](https://docs.x.ai/developers/management-api-guide)
- [Console billing concepts](https://docs.x.ai/console/billing)

### Auth and host

| Fact | Detail |
|------|--------|
| Base URL | `https://management-api.x.ai` (not `api.x.ai`) |
| Credential | **Management key** (Bearer), separate from inference `XAI_API_KEY` |
| Where to get key | Console → Settings → Management Keys |
| Permission | Account needs Management Keys Read (+ Write if mutating) |
| `team_id` | Path parameter on team-scoped routes |

### Documented billing endpoints (relevant to Half B)

Only endpoints listed in public docs are named here.

#### 1. Prepaid balance (primary for v1)

```
GET /v1/billing/teams/{team_id}/prepaid/balance
Authorization: Bearer <management_key>
```

**Documented response shape (summary):**

| Field | Meaning |
|-------|---------|
| `total.val` | Prepaid balance as **USD cents** (string in examples; often negative convention for remaining credit in examples — parse carefully against live dogfood) |
| `changes[]` | History of balance changes |

Example fields on each change (docs): `teamId`, `changeOrigin` (`PURCHASE` / `SPEND` / `REFUND` / `MANUAL` / `AUTO_PURCHASE`), `amount.val` (USD cents), invoice ids, timestamps, payment processor.

**Residual pin matches this path:**
`GET https://management-api.x.ai/v1/billing/teams/{team_id}/prepaid/balance`

#### 2. Historical usage / series (documented; POST analytics)

```
POST /v1/billing/teams/{team_id}/usage
Authorization: Bearer <management_key>
```

Body: `analyticsRequest` with:

- `timeRange` (`startTime`, `endTime`, `timezone` IANA)
- `timeUnit` (day / hour / month / … / none)
- `values[]` (field name + aggregation, e.g. `usd` + `AGGREGATION_SUM`)
- optional `groupBy`, `filters`

Response: `timeSeries[]` with `group` / `groupLabels` / `dataPoints[]` (`timestamp`, `values[]`), plus `limitReached`.

This is a **real documented series surface**. It is not a GET invent; product may use it for charts **after** prepaid v1 if dogfood needs token/spend series. Do not invent alternate series URLs.

#### 3. Other documented billing surfaces (secondary)

| Method + path | Role for Half B |
|---------------|-----------------|
| `GET …/postpaid/invoice/preview` | Current period postpaid preview; includes prepaid credit fields on core invoice |
| `GET …/postpaid/spending-limits` | Soft/hard postpaid limits |
| `GET …/invoices` | Invoice list (history, not live footer) |
| `GET/POST …/billing-info` | Billing address / tax (not usage meter) |
| `GET …/payment-method` | Payment methods (not usage meter) |
| `POST …/prepaid/top-up` | Mutating top-up (out of scope for read-only TUI meter) |

### Not documented for inference key

- Public **inference** host `api.x.ai` has **no** team prepaid balance endpoint for a console API key in product residual or docs reviewed here.
- OpenRouter has its own `GET /api/v1/credits` (unrelated host).
- SuperGrok session billing remains grok.com session-only.

### External confirmation (non-product)

Third-party notes (e.g. CodexBar docs) also read prepaid balance from the Management API prepaid path. That is corroboration only; product must implement against official docs + hermetic mocks.

## In-tree client status (updated 2026-07-30 after Half B core prepaid ship)

| Piece | Status |
|-------|--------|
| `[endpoints] management_api_key` config field | **Shipped** (config + keyring path) |
| Management key store | **Shipped** — keyring URL `https://management-api.x.ai` (not inference `xai_console`) |
| `[endpoints] management_team_id` | **Shipped** — explicit pin; not SuperGrok OIDC team |
| `GET …/billing/teams/{team_id}/prepaid/balance` | **Shipped** — hermetic client → `ConsoleTeamPrepaidMeter` + ~60s process cache |
| Footer / `/limits` console prepaid | **Shipped** — `Console key · team prepaid: $N` / `Balance (console team prepaid): $N` when console live + cents known; honest `no management key/team id` / `loading team prepaid...` / `team prepaid unavailable` when unknown (soft `no $ meter yet` retired) |
| `POST …/billing/teams/{team_id}/usage` series | **Not wired** (documented; ship only if dogfood needs charts) |
| Soft `/usage` under console live | **Shipped** — names console team prepaid / honest gap; does not sell SuperGrok session billing as live console spend (join `/tmp/grok-join-impl-usage-console-honesty-0c6a7911.md`) |
| Prepaid cache freshness | Known UX: ≤60s process TTL + last-good on fetch miss/error (poll does not bust cache) |
| Live dogfood | **Operator** — real management key + real `team_id` |

Joins (ship evidence):
`/tmp/grok-join-impl-mgmt-key-team-fetch-2026-07-30.md`,
`/tmp/grok-join-impl-console-meter-tui-2026-07-30.md`,
`/tmp/grok-join-impl-usage-console-honesty-0c6a7911.md`.
User-guide: `02-authentication`, `04-slash-commands` `/limits`.

Do **not** re-claim load-only stub / no HTTP / no keyring for management prepaid.

## `team_id` sources (do not assume equality)

| Source | Safe for Management API `team_id`? |
|--------|-------------------------------------|
| Explicit operator config / UX paste | **Yes** (recommended v1) |
| Console Settings UI team id | **Yes** when operator pastes the console team id |
| SuperGrok OIDC `GrokAuth.team_id` (Business SuperGrok) | **Not assumed equal** without evidence; different product surface |
| Enterprise managed-config / `GROK_DEPLOYMENT_KEY` principal | Different purpose; not a substitute for Management team prepaid |
| Inference API key metadata | No documented team prepaid balance on inference key |

**Recommendation:** explicit config field (or interactive set) for Management API team id. Optional warn if SuperGrok OIDC team id differs when both present.

## What v1 shipped vs still open

### Shipped v1 (Half B core prepaid — 2026-07-30)

1. Management key secure store (keyring URL `https://management-api.x.ai`; never argv secrets). Distinct from inference console keys and SuperGrok OIDC.
2. Explicit `[endpoints] management_team_id` for Management API.
3. Hermetic HTTP client: `GET …/prepaid/balance` → cents on `ConsoleTeamPrepaidMeter` (+ process cache).
4. Footer + `/limits` when console live **and** management meter present; plain labels: **console team prepaid**.
5. Half A SuperGrok rows kept; SuperGrok dollar extras are not sold as console live spend.
6. Honest gap when key/team/fetch absent: `no management key/team id` /
   `loading team prepaid...` / `team prepaid unavailable` (soft `no $ meter yet`
   retired — join `/tmp/grok-join-impl-no-dollar-meter-real-0c6a7911.md`).
7. Soft `/usage` under console live: live line + console team prepaid / honest gap (not SuperGrok-as-live). Join: `/tmp/grok-join-impl-usage-console-honesty-0c6a7911.md`.

### Still open (documented, not invented as shipped)

- `POST …/usage` time series for chart-class token/spend rows (group by description, sum `usd`, day buckets). Only if dogfood needs charts.
- Operator live dogfood with real management key + team_id.
- Soft polish: prepaid cents refresh ≤60s TTL + last-good on error (documented known UX).

### Blocked / out of scope for this research

- Scraping console.x.ai HTML
- Inventing GET series endpoints not in docs
- Treating SuperGrok Business OIDC as console team prepaid
- Claiming full Business Usage **charts** done (core prepaid balance is shipped; series is not)

## Suggested product constants (for implementer)

```text
MANAGEMENT_API_BASE = https://management-api.x.ai
PREPAID_BALANCE_PATH = /v1/billing/teams/{team_id}/prepaid/balance
USAGE_ANALYTICS_PATH = /v1/billing/teams/{team_id}/usage   # POST; series later
Auth header = Authorization: Bearer <management_key>
```

Do not hardcode Surmount’s team id in product; operator supplies it.

## Dogfood checklist (operator)

1. Create management key (Console → Management Keys).
2. Note console team id (team UUID used in Management paths).
3. Put key + team id in grok config / keyring (`management_api_key`, `management_team_id`).
4. Refresh billing → `/limits` / footer show console team prepaid.
5. Without key: still `no management key/team id` (no invented $).

## Next agent-doable steps (after core prepaid + `/usage` honesty)

1. Series UI only if operator dogfood asks for charts (`POST …/usage`).
2. Dual-auth failover polish only with observed red.
3. Keep residual / user-guide honest as slices ship.
4. Optional: force-refresh / clear-on-miss prepaid only if dogfood complains about stale ≤60s last-good.

Core store + fetch + TUI wire + soft `/usage` console-live already landed; do not re-implement v1 prepaid.
