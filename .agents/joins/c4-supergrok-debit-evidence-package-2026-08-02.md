# C4 SuperGrok debit evidence package (server / billing ticket brief)

**Date assembled:** 2026-08-02
**Audience:** xAI / server billing investigation (ticket-ready)
**Client product:** Surmount `grok-oss` (fork of xAI Grok CLI)
**Mode:** evidence synthesis only. No multi-hour re-dogfood. No secrets.
**Meters kept distinct throughout:** SuperGrok **included weekly %** ≠ SuperGrok **$ extras** ≠ console team **prepaid $** ≠ console team **API Usage $** ≠ Management **postpaid OAuth vs API class**.

**Source joins (this package does not re-rank residual):**

| Join | Path |
|------|------|
| Slice 2 dogfood G4 | `.agents/joins/slice2-dogfood-g4-2026-08-02.md` |
| Live limits recheck | `.agents/joins/live-limits-recheck-2026-08-02.md` |
| Slice 1 poll history | `.agents/joins/impl-slice1-poll-history-2026-08-02.md` |
| Branch 2b honesty | `.agents/joins/impl-branch-2b-honesty-2026-08-02.md` |
| Console +$0.01 one turn | `.agents/joins/console-burn-one-turn-investigation-2026-08-02.md` |
| Console Usage $547 | `.agents/joins/console-api-usage-547-evidence-2026-08-02.md` |

**Optional caches / logs consulted:**

- `~/.grok/logs/unified.jsonl` (billing / productUsage series; tokens redacted)
- `/tmp/limits-live.json`, `/tmp/limits-live-now.json`, `/tmp/limits-live-burn.json`
- `/tmp/mgmt-preview.json`
- Tip recheck: `/tmp/grok-1000/limits-live-tip.json` (2026-08-02T15:35Z; polls failed)

---

## One-line ask

Under heavy **SuperGrok session** traffic (SessionToken → `cli-chat-proxy.grok.com`, **not** live console ApiKey), SuperGrok **included weekly** and **Grok Build productUsage** meters stayed flat for many hours while **team console Usage dollars** and **Management OAuth-class postpaid** clearly moved. Please confirm whether session / Grok Build OAuth traffic is supposed to debit this principal’s SuperGrok included pool (and Build sub-meter), and why the client-visible billing poll stayed at 65% (then a weak +1 later) with Build stuck at 54% and extras stuck at $100.29.

---

## 1. What we observed

### 1a. SuperGrok meters (the C4 question)

| Meter | Observed | Window (UTC, 2026-08-02 unless noted) |
|-------|----------|----------------------------------------|
| **Included weekly `creditUsagePercent`** | **65.0** for a long stretch under load | Log samples: **408** polls at 65.0 from **04:08:57Z** through **07:29:11Z** (and earlier dogfood same day / prior audits also reported multi-hour 65.0). CLI dumps same day: **65.0** used / **35%** remaining. |
| **Weak step later** | **65 → 66** | First **66.0** sample: **2026-08-02T13:38:37.561Z**. Then **100** samples at 66.0 through at least **14:34:07.670Z**. About **+1 point** only. **Not** a controlled “before/after one dogfood turn” close. |
| **Grok Build `productUsage`** | **54.0** flat the entire observability window | First logged Build % sample **06:29:40.517Z** at 54.0; last sampled **14:34:07.670Z** still **54.0** (**229** samples at 54.0). Heavy SuperGrok **Build** session traffic did **not** step this meter in-client. |
| **Grok Chat `productUsage`** | **11.0 → 12.0** with the weak included tick | 11.0 earlier; 12.0 co-moving with the 66% window. |
| **SuperGrok $ extras (`prepaidBalance` cents on SuperGrok billing wire)** | **10029** = **$100.29** flat | **508** samples at 10029; never moved in this series. CLI: `dollarExtrasUsd: 100.29`, `dollarExtrasObserved: true`. |
| **Period / reset (CLI)** | Weekly · next reset **August 3, 19:25** | From limits dumps. |

**C4 pass condition (product plan):** any of included %, Build productUsage %, or SuperGrok $ extras (after included full) **moves with SuperGrok session traffic** in a way that proves absorption.
**Result under multi-hour load:** included and Build flat for hours; extras flat always; later a **weak +1%** on included (and Chat) while **Build still 54**. Treat as **unproven / laggy / coarse-%**, **not** C4 closed.

### 1b. Team dollar pain (different meters; still real)

These **do not** prove SuperGrok included debit. They show **team $** moved while SuperGrok path + included headroom were visible.

| Surface | Value | Notes |
|---------|-------|--------|
| Console team **API Usage** (not Grok Business licenses) | **$547.87** week Jul 27–Aug 2 | Text **$332.63** + Grok Build **$214.41** + tiny image/voice. Tokens ~1.14B, ~57k requests. |
| One-turn dogfood tick | **$547.87 → $547.88** (**+$0.01**) ~01:04 local / ~**07:04 UTC** | SuperGrok still ~65% / extras $100.29; path SuperGrok. |
| Management postpaid preview (cached `/tmp/mgmt-preview.json`) | OAuth **Grok Build** lines ~**$201.76**; **API** product lines ~**$5.80**; period totalWithCorr ~**$207.56**; `defaultCredits` **1500** ($1500) | Amounts in cents on wire; OAuth dominates ~35×. |
| Console team **prepaid** | **$340.0** remaining; prior dump **0 SPEND** rows | Usage $ ≠ prepaid SPEND trail in that dump. Open reconciliation (defaultCredits / postpaid-class). |

### 1c. Traffic volume (same process slice; rough)

From the one-turn / dogfood investigation join (unified log slice): on the order of **~1000+** `inference_done` completions and **~100M** tokens in an ~8h slice on this host’s long-lived `grok-oss` (pid noted in joins as **3138654**). Week Usage is team-wide and multi-client; this process is a large contributor, not proven sole source of all $547.

---

## 2. Auth path (client)

| Fact | Value |
|------|--------|
| Live sampling | **`supergrok_session`** |
| Live principal role | **`business`** |
| Label | Live sampling: SuperGrok session (business) |
| Inference host | **`https://cli-chat-proxy.grok.com/v1`** |
| Auth type for spawns | **`SessionToken`** (JWT; `auth_method_id=cached_token`) |
| Console key in resolve chain for primary sampling | **`console.isLive: false`** (key available in store; not live) |
| Live process env | Dogfood `grok-oss` **without** `XAI_API_KEY` in process environ (host fish may still export key for **other** tools) |
| Config intent | `preferred_method=oidc`, `auto_use_included_limits=true` |
| Subagent spawns (dogfood window join) | **56** SessionToken+proxy; **0** live ApiKey spawns; base_url aggregation dominated by cli-chat-proxy |

**Implication for the ticket:** the client is on the **SuperGrok / OAuth / cli-chat-proxy** path with **included headroom**, not secretly primary-sampling with the team console ApiKey. Team Usage $ movement under that path is the pain; it is **not** the same as SuperGrok included % debit.

---

## 3. Identity / roles (redacted; short ids OK)

| Field | Value |
|-------|--------|
| Team / business identity id | **`61fab250-b2c1-40cf-b5b8-628e673a2eeb`** (Surmount; role **business**) |
| Tier (log) | SuperGrok **Heavy** |
| Personal SuperGrok principal | Separate OIDC; often **expired** in dumps (`58c5f686-427…`); dual-principal poll notes appear on CLI |
| Console team id (management / Usage URL) | Same team id as business SuperGrok identity above |
| Secrets | **Not included.** No tokens, API keys, or full JWTs in this package. |

---

## 4. Timestamps / window (UTC)

| Event | When |
|-------|------|
| Heavy dogfood + flat 65 / $100.29 (CLI dumps) | Morning 2026-08-02 and prior same campaign (dumps under `/tmp/limits-live*.json`) |
| Log series flat **65.0** (this package re-count) | **04:08:57Z → 07:29:11Z** (408 samples at 65; prior audits also multi-hour 65) |
| Build productUsage observability | **06:29:40Z → ≥14:34:07Z**, always **54.0** |
| Operator console Usage screenshot ~$547.87 | ~2026-08-02 01:03 local (America/Denver MDT) |
| One-turn Usage +$0.01 | ~**07:04 UTC** (01:04 MDT) aligned with SuperGrok inference |
| First included **66** sample | **2026-08-02T13:38:37.561Z** |
| 66% window sampled through | **≥14:34:07.670Z** (Build still 54; extras still 10029) |
| Tip CLI recheck (polls failed; no meters) | **2026-08-02T15:35Z** — `timeout 45 grok-oss limits --json` exit 0; billing poll timeout / expired personal |

Host local zone for operator screenshots: **America/Denver (MDT, UTC−6)**.

---

## 5. M3 OAuth vs API class (team $ pain ≠ included debit)

Management postpaid invoice preview (cached live wire, same team):

| Class | ~USD (period) | Wire product |
|-------|---------------|--------------|
| **Grok Build OAuth** | ~**201.76** | product `grok-build`, descriptions like “Grok Build OAuth grok-4.5-build” |
| **API** | ~**5.80** | product `api`, “API grok-4.5”, STT, storage |
| Period totalWithCorr | ~**207.56** | `defaultCreditsIssued` / totalWithCorr cents 20756 |
| defaultCredits pool | **$1500** | free/default credits envelope |

**Client reading we need confirmed or denied by server:**

1. SuperGrok CLI / SessionToken / cli-chat-proxy traffic appears to settle as **team OAuth / Grok Build** dollars on console Usage and Management postpaid.
2. That **team $ movement is not evidence** that SuperGrok **included weekly** debited.
3. While included still shows headroom (~35% remaining at 65%, ~34% at 66%), team Usage still climbed (week $547 + one-cent tick). That is the limits-first gap operators feel.

**Do not invent SuperGrok included debit from OAuth postpaid totals.**

---

## 6. Product client honesty already in place

Client work does **not** invent server debit. It measures and labels unproven debit.

| Slice / branch | What shipped | Ticket relevance |
|----------------|--------------|------------------|
| **Slice 1** poll history | Process ring per `identity_id`; `included_debit_unproven` (default ≥2 polls, ≥30s window); `flat_poll_unproven_debit` on limits snapshot; optional `billing: poll_delta` when %/extras step | Client can detect multi-sample flat series; **cannot** force ledger movement |
| **Branch 2b honesty** | `/limits` and `/usage` notes: base poll note; **conditional** flat note (only meters observed flat); Build % when on wire; C6 when OAuth postpaid dominates without proving included moved | Operators are told flat polls ≠ proven burn |
| **Slice 3** M3 postpaid | Surfaces OAuth vs API class totals when Management preview works | Attribution tool for team $, not included proof |
| **Slice 4** extras before console | Policy when included ≥ 100% and extras remain | **Code-only** for after-burner; **not live-proved** (included never hit 100% in dogfood) |
| Design A / limits-first | Console ApiKey omitted from primary chain while included has headroom | Live path matched: `console.isLive=false` |

**Honesty note examples already used with operators:**

- SuperGrok included % is the billing poll reading, not proof of included-limit burn.
- Team Usage $ / OAuth postpaid can move without proving included weekly moved.
- Flat note names included always; Build / SuperGrok $ extras only if those were observed flat on the window.

Cold one-shot `limits --json` may not light multi-sample flat_poll (needs in-process history). Long-lived process + log series are the C4 evidence, not a single poll.

---

## 7. What we need from server / billing (ticket questions)

Please answer in product terms if possible (not only internal codenames).

### Q1. Intended debit path for SessionToken → cli-chat-proxy

For identity **`61fab250-b2c1-40cf-b5b8-628e673a2eeb`** (business SuperGrok Heavy), does **Grok Build / CLI session** traffic:

- (a) debit SuperGrok **included weekly** first, then SuperGrok **$ extras**, then team console pools; or
- (b) bill **only** as team OAuth / Grok Build / defaultCredits Usage without moving included weekly; or
- (c) something else (document the order)?

### Q2. Why included poll stayed flat under load

During multi-hour heavy session traffic on 2026-08-02, client polls reported **`creditUsagePercent = 65.0`** hundreds of times, then a **single-point** step to **66.0** hours later. Is that:

- expected **coarse %** granularity,
- **settlement lag** (how long?),
- wrong **pool / principal** for this identity,
- or a **bug** (traffic not attributed to included)?

### Q3. Why Grok Build productUsage stayed 54.0

`productUsage` for **GrokBuild** stayed **54.0** from first observability (~06:29Z) through afternoon samples while the session was Build-heavy SuperGrok. Is Build %:

- a different quota than top-level included,
- lagging more than included,
- not updated for cli-chat-proxy / Build model,
- or broken for this tier/team?

### Q4. SuperGrok $ extras never moved

`prepaidBalance` / extras stayed **$100.29** the entire series (included never full). Confirm extras should **not** move until included is exhausted (expected), and that this field is the correct SuperGrok extras meter (not console prepaid $340).

### Q5. OAuth settlement vs included

Management shows **~97%** of one postpaid snapshot as **Grok Build OAuth**. Console Usage **Grok Build** line is large. Is OAuth settlement:

- **supposed** to reduce SuperGrok included % on the same billing poll the CLI uses, and if so why didn’t it under load; or
- intentionally **parallel** team Usage that does **not** touch SuperGrok included?

### Q6. Poll endpoint contract for the CLI

What is the authoritative field/path for “included weekly used %” and “Grok Build product usage %” that `xai-grok-cli` / billing config should trust? Any known delay, caching, or 1% quantization we should document client-side?

### Q7. Reproduction hints for server side

If useful for server logs:

- Identity: `61fab250-b2c1-40cf-b5b8-628e673a2eeb`
- Auth: SessionToken / OIDC business, host `cli-chat-proxy.grok.com`
- Window: 2026-08-02 ~04:00–15:00 UTC (flat 65, then 66; Build 54)
- Client long-lived pid (host-local): 3138654 (may not appear server-side)
- No claim that this host is the only team consumer of Usage $547

---

## 8. Explicit non-claims

Do **not** treat this package as proving any of the following:

1. **C4 closed** or SuperGrok included debit fully proven (weak 65→66 is not a clean controlled pass; Build still flat).
2. **SuperGrok included debit invented** from console Usage $547, +$0.01, or OAuth postpaid ~$202.
3. **Included weekly = SuperGrok $ extras = console prepaid $340 = console Usage $** (four different meters).
4. **Live console ApiKey primary sampling** during the dogfood window (`console.isLive=false`; SessionToken path).
5. **C5 live** (extras before console after included ≥ 100%) — included never reached 100% in evidence.
6. **All team Usage $547** is this single process or this product alone (team-wide page; other clients open).
7. **Prepaid ledger SPEND** equals Usage chart (open reconciliation with defaultCredits / postpaid-class).
8. **Tip binary 15:35Z recheck** as meter proof (billing polls failed; meters absent).
9. Product can fix ledger lag by hopping to console ApiKey while included has headroom (limits-before-credits; wrong fix).
10. Secrets, tokens, or full credentials (none shipped here).

---

## 9. Suggested ticket title + body sketch

**Title:** SuperGrok included / Build productUsage flat under cli-chat-proxy load while team OAuth Usage $ moves (identity 61fab250…, 2026-08-02)

**Body (short):**
Client on SuperGrok business SessionToken → cli-chat-proxy, console key not live. Multi-hour heavy Build traffic: included `creditUsagePercent` stuck at 65 then weak step to 66; GrokBuild productUsage stuck at 54; SuperGrok extras $100.29 flat. Same window: team console API Usage ~$548 week and OAuth-dominant Management postpaid. Please confirm intended debit order and whether lag, coarse %, wrong pool, or missing attribution. Client already surfaces flat-poll honesty; needs server contract.

**Attachments for filer (local, not in repo):** redacted limits JSON, redacted log excerpts of `billing: fetched credits config` series, Management preview class totals, console Usage screenshots (operator).

---

## 10. Criteria snapshot (evidence only; not residual re-rank)

| Id | Status from evidence | Note |
|----|----------------------|------|
| **C1** SuperGrok path while included headroom | **Pass** | SessionToken + proxy; business |
| **C3** no live ApiKey primary under headroom | **Pass for primary sampling** | `isLive=false`; OAuth team $ still can move |
| **C4** included / Build / post-full extras debit with traffic | **Not closed** | Flat under load; weak +1 later; Build flat; extras flat |
| **C5** extras before console after included full | **Not live** | included ≪ 100% |
| **F1a** SuperGrok included moves | **Unproven / weak lag signal only** | |
| **F1b** console Usage $ real while SuperGrok primary | **Proven as team $ pain** | ≠ included debit |

---

## 11. Package hygiene

- **No product code edits** for this package.
- **No git add / commit.**
- **No secrets** printed.
- Join path: `.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`
- Short summary: `/tmp/grok-1000/grok-impl-summary-c4-evidence.md`
