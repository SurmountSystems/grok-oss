# Join: Grok Business license Usage (zeros) vs product SuperGrok / console path

**Date:** 2026-08-02
**Tree:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only (code + prior joins + residual/research; no product edits)
**Screenshot surface:** console.x.ai → team `61fab250-b2c1-40cf-b5b8-628e673a2eeb` → Platform **Grok Business** → **Usage** (`…/grok-business/usage`) — subtitle “Grok Business **licenses**”; Total active users / messages / conversations **0** (Jul 26–Aug 2 2026).

**Prior joins read (not reinvented):**

| Path |
|------|
| `/tmp/grok-join-explore-business-oidc-live.md` |
| `/tmp/grok-join-explore-console-credits-vs-limits.md` |
| `/tmp/grok-join-explore-limits-credits-deep.md` |
| `/tmp/grok-join-live-prepaid-wire-capture.md` |
| `/tmp/grok-join-impl-business-supergrok-heavy-routing.md` |
| `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` |
| `RESIDUAL.md` §4 dual-auth / meters; `FORK.md` billing dual-auth |

---

## One-line answer (operator)

**That Grok Business *licenses* Usage page is a different product surface from SuperGrok OIDC (cli-chat-proxy) and from console API credits; zeros there do not mean the CLI is “not on Business SuperGrok Heavy.”**

---

## Meter taxonomy (plain names; keep distinct)

| # | Meter (plain name) | What it measures | Host / credential | Product surfaces it? |
|---|--------------------|------------------|-------------------|----------------------|
| **1** | SuperGrok **included weekly** | Consumer SuperGrok OAuth included pool (`creditUsagePercent`) | `cli-chat-proxy.grok.com` + OIDC/session JWT | Yes — status `%`, `/limits` SuperGrok rows |
| **2** | SuperGrok **dollar extras** | Session Extra Usage Credits (`prepaidBalance`) | Same session billing | Yes — `/limits` SuperGrok $ extras |
| **3** | **Console team prepaid** | Management prepaid ledger remaining (`total.val` cents) | `management-api.x.ai` + **management** key + team id | Yes — footer / `/limits` when key+team known |
| **4** | **Console API spend / Usage $** | Team inference spend on console keys (e.g. “Grok Build $…”) | `api.x.ai` + **inference** API key | Browser console Usage (API side); TUI has prepaid balance, not full spend charts |
| **5** | **Grok Business license seats / messages** | Team **Grok Business licenses** product: active users, messages, conversations (screenshot subtitle) | console.x.ai Platform → **Grok Business** → Usage | **No** — not wired; product never claims this chart |
| **6** | **Business SuperGrok OIDC principal** | Second SuperGrok login with Team principal + `team_id` (role label `business`) | Still SuperGrok session → proxy; meters **1–2**, not **5** | Yes as SuperGrok `(business)` role on dual-auth `/limits` / doctor |

**Hard law (FORK / RESIDUAL / AGENTS):** personal SuperGrok included ≠ SuperGrok dollar extras ≠ console team prepaid / Business Usage class ≠ second SuperGrok OAuth principal. Business SuperGrok OIDC is **not** console team prepaid. Product comments go further: shared SuperGrok weekly pool is **also not** console Grok Business **license** seat/message usage.

Naming trap: residual/docs sometimes say “console Grok Business Usage **class**” for Half B = team **API** prepaid / spend. The screenshot dropdown **Grok Business** (not “API Usage / Credits”) is the **license** product chart. Same word “Business”; **different meters**.

---

## What the product path does today (code-backed)

### Sampling identity + host

| Intent | Auth | Base URL | Bills which meter? |
|--------|------|----------|--------------------|
| SuperGrok limits / session | `SessionToken` + OIDC JWT | `https://cli-chat-proxy.grok.com/v1` | **1** / **2** (server-side SuperGrok pools) — **not** license seats **5**, **not** console prepaid **3** by itself |
| Console API credits / spend | `ApiKey` + console inference key | `https://api.x.ai/v1` | **4** (and may move **3** when spend posts to prepaid) |
| Read console prepaid only | Management key (no inference) | `https://management-api.x.ai` … `/prepaid/balance` | Reads **3** only |

Defaults:

```48:51:crates/codegen/xai-grok-shell/src/agent/config.rs
/// Default base URL for the cli chat proxy.
pub const CLI_CHAT_PROXY_BASE_URL_DEFAULT: &str = "https://cli-chat-proxy.grok.com/v1";
/// Default base URL for the public xAI API.
pub const XAI_API_BASE_URL_DEFAULT: &str = "https://api.x.ai/v1";
```

Wire prep: `prepare_sampling_config_for_model` → `auth_manager.current_or_expired()` → `resolve_credentials_preferring_with_rank(..., preferred_method, auto_use_included_limits)`
→ `crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs` ~L1231–1251.

### Dual-auth order (when “using SuperGrok limits” vs “using console API”)

| Config | Primary | Failover |
|--------|---------|----------|
| Default (`preferred_method` oauth/oidc/unset; both creds) | SuperGrok session JWT, proxy host | Console key(s) → `api.x.ai` after hop/exhaust |
| `preferred_method = "api_key"` | Console key, `api.x.ai` | Session JWT last |
| `auto_use_included_limits = true` (and not api_key pin) | Rank SuperGrok JWTs by **included headroom** + sooner reset (**not** Business-first); **omit console** from chain while any SuperGrok has included remaining | After ExhaustedAll included → console primary |

Ranking / strip console while headroom:

```356:388:crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs
/// Build primary/failover for `auto_use_included_limits`.
///
/// Order while any SuperGrok has included headroom: ranked SuperGrok only
/// (console keys **omitted** from the chain — limits-before-credits).
/// If every SuperGrok included pool is exhausted, console keys lead.
pub fn order_credentials_for_preferred_auto(...)
```

Role label `business` = store metadata Team principal + non-empty `team_id` (login JWT peek / listing), **not** a re-decode of the live bearer on every request:

```208:218:crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs
/// Role inferred from auth session fields (team principal → Business).
pub fn role_from_session_fields(
    principal_type: Option<&str>,
    team_id: Option<&str>,
) -> SupergrokAccountRole {
    ...
}
```

```482:488:crates/codegen/xai-grok-shell/src/auth/model.rs
        // Inline role (avoid model ↔ rank cycle): team principal + team_id → business.
        let role_label = if auth.principal_type.as_deref() == Some(TEAM_PRINCIPAL_TYPE)
            && auth.team_id.as_ref().is_some_and(|t| !t.is_empty())
        {
            "business"
```

Live sampling line:

```335:343:crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs
    pub fn live_sampling_line(&self) -> String {
        match (self.live_identity, self.live_principal_label.as_deref()) {
            (SamplingIdentityKind::SuperGrokSession, Some(role)) => {
                format!("Live sampling: SuperGrok session ({role})")
            }
            ...
            (SamplingIdentityKind::ConsoleKey, _) => "Live sampling: console key".into(),
```

Explicit product honesty that dual SuperGrok unified pool ≠ license seats:

```140:145:crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs
    /// Dual SuperGrok OIDC logins share one consumer SuperGrok included pool
    /// ...
    /// pool. Also not console.x.ai Grok Business license seat/message usage.
    pub shared_unified_supergrok_pool: bool,
```

### SuperGrok billing poll (meters 1–2 only)

`GET {cli-chat-proxy}/billing?format=credits` → `creditUsagePercent`, `prepaidBalance`, optional wire `productUsage` (mostly unused in TUI).
`extensions/billing.rs` `fetch_credits_config_with_session` ~L226–241.

### Console prepaid (meter 3 only; not license Usage)

`GET https://management-api.x.ai/v1/billing/teams/{team_id}/prepaid/balance`
`auth/xai_management.rs` `PREPAID_BALANCE_PATH_TEMPLATE` ~L42.
Documented series `POST …/usage` is Management **API analytics**, not the browser Grok Business **licenses** page; series still not required for this join.

**No in-tree client** for console path `/team/.../grok-business/usage` or any “license messages/active users” API. Grep under shell auth / management finds prepaid (+ docs for usage analytics), not license seat charts.

### What would increment **Grok Business license** Usage (meter 5)?

Product tree **does not implement** that path. From surface copy + prior research, zeros are **expected** when traffic is only:

- grok-oss / CLI SuperGrok session → cli-chat-proxy, and/or
- console inference keys → api.x.ai (that hits **API** Usage / prepaid, not the **licenses** subtitle).

**Hypotheses for messages > 0 on that page** (unproven server-side; not in our code):

1. Seated users using **Grok Business chat product** (e.g. grok.com / business workspace licensed seats) under the team’s Grok Business licenses — not CLI proxy.
2. Other first-party clients that xAI attributes specifically to **license** seats (if any exist outside our tree).
3. **Not** proven: Business SuperGrok OIDC JWT on cli-chat-proxy. Prior live dogfood showed SuperGrok session + `(business)` while that license page stayed zero-class.

So: “using my Business SuperGrok Heavy plan” (OIDC Heavy / included %) and “Grok Business licenses Usage > 0” are **not** the same claim.

---

## Hypothesis table: zeros on license Usage while product may still be correct

| Hyp | Why screenshot is zeros | Fits product? | Evidence |
|-----|-------------------------|---------------|----------|
| **H1 (strong)** | License Usage only counts Grok Business **seat/message** product traffic; CLI SuperGrok + console API never post there | Yes by design | Subtitle “licenses”; product comment L144–145 limits_snapshot; no client for that page; prior OIDC explore join |
| **H2 (strong)** | Live sampling is SuperGrok session → meters **1–2**, not **5** | Yes when `/limits` says SuperGrok | Live join 2026-08-02: `liveSampling: supergrok_session`, `livePrincipalRole: business`, `console.isLive: false` (`/tmp/grok-join-live-prepaid-wire-capture.md`) |
| **H3 (strong)** | Console API $ / prepaid movement is meters **3–4** (Platform API Usage / Management), wrong dropdown for this screenshot | Yes | Explore console-credits join; Management prepaid $340 live; postpaid lines can show `grok-build` spend without license messages |
| **H4 (medium)** | Business SuperGrok OIDC is stored and labeled, but included % debit unproven (flat 65% under heavy dogfood) | Orthogonal to license zeros | Deep limits join: SessionToken path proven; included burn **not** proven from flat poll |
| **H5 (medium)** | `auto_use_included_limits` may rank personal vs business JWT; UI role tracks **active base** store role, not always ranked wire token | Can mislead which SuperGrok principal, **not** whether license seats count | OIDC explore join §2, §4–5 |
| **H6 (weak for this screenshot)** | Product stuck on console key only → would zero SuperGrok burn but should raise **API** Usage $, still not necessarily license messages | Check live sampling | Design A strips console while included headroom; dogfood logs SessionToken |
| **H7 (weak)** | Wrong team id in URL | Possible but team id matches management pin / SuperGrok team scope in dogfood | Config `management_team_id` = `61fab250…` same family |

**Conclusion for the operator alarm** (“still not using Business SuperGrok Heavy despite all we built”):

- **Unsupported** by license Usage zeros alone.
- **Supported** for SuperGrok **session auth path** when live sampling is SuperGrok (recent live JSON: business role, 65% included, $100.29 extras, prepaid $340 console ledger separate).
- **Open** whether included Heavy pool is **debited** (flat % under load). That is a SuperGrok billing question, not a Grok Business **licenses** chart question.

---

## What still unproven / next live checks

| Check | How | Proves |
|-------|-----|--------|
| Live sampling principal | `grok limits` / `grok-oss limits --json` → `liveSampling`, `livePrincipalRole` | SuperGrok session vs console key **now** |
| Dual principals + fingerprints | `/doctor` dual-auth block | Business principal in store |
| SuperGrok included / extras movement | Same limits JSON over time; do not claim burn from a single flat % | Debit of meters **1–2** |
| Console inference burn by this product | Spawn / logs: `ApiKey` + `api.x.ai` vs `SessionToken` + proxy | Meter **4** by this process |
| Console prepaid | Management `total.val` ↔ `console.teamPrepaidUsd` (already live-matched $340) | Meter **3** honesty |
| License Usage non-zero | Only by using whatever product xAI ties to **Grok Business licenses** (likely seated business chat); **not** expected from CLI SuperGrok alone | Meter **5** |
| Wire JWT business vs personal | Offline JWT claims / token suffix vs store scopes (secrets careful) | Absolute personal vs business SuperGrok on wire |

Do **not** treat console Platform **Grok Business → Usage** zeros as a red test for dual-auth routing or SuperGrok Heavy implement work.

---

## File:line citations (key seams)

| Seam | Path |
|------|------|
| Default proxy / API hosts | `crates/codegen/xai-grok-shell/src/agent/config.rs:48-51` |
| Resolve + auto rank wire-up | `…/agent/mvp_agent/agent_ops.rs:1231-1251` |
| Auto order / omit console while included | `…/auth/supergrok_identity_rank.rs:1-8, 356-405` |
| Business role from Team + team_id | `…/auth/supergrok_identity_rank.rs:208-218`; `…/auth/model.rs:482-488` |
| SuperGrok credits poll | `…/extensions/billing.rs:226-241` |
| Management prepaid only | `…/auth/xai_management.rs:29-42` |
| License seats ≠ unified SuperGrok pool | `…/pager/src/views/limits_snapshot.rs:140-145, 335-343, 580-588` |
| User-guide dual SuperGrok + console prepaid | `…/pager/docs/user-guide/02-authentication.md:100-196` |
| Residual meters law + Half B | `RESIDUAL.md` ~257–407 |
| FORK dual-auth + billing halves | `FORK.md` dual-auth + billing meters bullets |
| Research Half B (API prepaid, not license chart) | `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` |

---

## Bottom line for parent / chat

1. **Screenshot is meter 5 (Grok Business licenses).** Product does not drive it.
2. **Business SuperGrok Heavy work** targets SuperGrok session (meters 1–2) + dual-auth routing, optional console prepaid (3) and API hop (4).
3. **Zeros on licenses Usage are consistent with successful SuperGrok CLI dogfood**, not a refutation.
4. If the fear is “not burning SuperGrok included,” check **flat included % / productUsage / independent SuperGrok Usage**, not this licenses page.
5. If the fear is “burning console $ instead,” check live sampling line and `api.x.ai` vs proxy, not Grok Business licenses zeros.

**Join path:** `/home/hunter/Projects/surmount/grok-build/.agents/joins/business-usage-vs-product-path-2026-08-02.md`
