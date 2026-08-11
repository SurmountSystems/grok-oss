# C4 xAI ticket package (paste-ready)

**Assembled:** 2026-08-07 / live refresh 2026-08-08 UTC
**Product:** Surmount `grok-oss` (unofficial fork of xAI Grok CLI)
**Purpose:** one copy-paste package for the operator to file with xAI billing / support
**Meters kept distinct:** free SuperGrok period % ≠ SuperGrok $ extras ≠ console team prepaid $ ≠ team postpaid OAuth / Grok Build class $ ≠ team postpaid API class $ ≠ team default credits ≠ Grok Build productUsage %

---

## Operator action (do this once)

1. Copy **Suggested ticket title** + **Body ready to paste** below.
2. File at: **your xAI billing / SuperGrok business support contact** (product has no public one-click ticket URL documented in-tree; use the channel you already use for Grok Business / SuperGrok Heavy billing).
3. Attach multipoll JSON and poll-history tail if available (paths under **Attachments**). **No secrets / tokens / JWTs.**
4. After filing, note the ticket id in residual or chat so agents stop treating C4 as "still need a package."

**Agents cannot close C4.** Client honesty and free-period-first path are already correct. The remaining pass is server absorption of free SuperGrok period against SuperGrok session traffic.

---

## Suggested ticket title

```
SuperGrok free period stuck ~6% (post-reset 2026-08-07) under cli-chat-proxy / SuperGrok primary while team OAuth Grok Build class ~$1014 moves; prior period stuck ~65% (2026-08-02)
```

---

## Body ready to paste

```
Subject: SuperGrok free period / included creditUsagePercent not debiting under cli-chat-proxy session load while team OAuth Grok Build Usage $ moves

Hi xAI billing / SuperGrok support,

We run Surmount grok-oss (fork of the Grok CLI) on business SuperGrok Heavy. Under real SuperGrok session traffic the client stays on SessionToken → https://cli-chat-proxy.grok.com/v1 with the console API key NOT live. Free SuperGrok period used percent (billing poll creditUsagePercent → includedUsedPct) does not step with that traffic in a controlled way, while team OAuth / Grok Build settlement dollars keep climbing.

## Two periods of evidence

Period A (2026-08-02, prior weekly window):
- Free SuperGrok period stuck at 65% for multi-hour heavy SuperGrok session / Build load (hundreds of successful billing poll samples at 65.0).
- Weak step later only: 65 → 66 once; not a controlled before/after close.
- Grok Build productUsage stuck at 54.0 for the entire observability window (hundreds of samples).
- SuperGrok $ extras (prepaidBalance) stuck at $100.29 the whole series.
- Same window: team console API Usage ~$547.87 (week Jul 27–Aug 2) with a one-turn +$0.01 tick while SuperGrok path was primary; Management postpaid OAuth / Grok Build class dominated API class.

Period B (2026-08-07…08, current weekly window after reset):
- Free SuperGrok period stuck near 6.0% used (94% remaining) with healthy live polls (pollSucceeded true, includedSource live_poll) on both business and personal principals (shared unified pool).
- activeDriver = supergrok_free_period (client free-period-first / Design A is correct).
- console.isLive = false; console key available but not primary.
- SuperGrok $ extras still $100.29 (side meter; not the live driver).
- Team postpaid OAuth / Grok Build class ~$1013–$1014 and still climbing; multipoll sample (two grok-oss limits --json calls ~35s apart, 2026-08-08 ~05:58–05:59 UTC): free period 6.0% → 6.0% while team postpaid OAuth class $1013.35 → $1013.77 and usage-series OAuth class rose as well.
- Next free SuperGrok period reset: August 10, 19:25 (weekly).
- Durable included poll history for business identity shows a dense multipoll ring of creditUsagePercent = 6.0 only (no Build product % on wire this period in that ring; extras cents 10029 flat).

## Auth path (so this is not a status-bar bug)

- liveSampling: supergrok_session (business)
- activeDriver: supergrok_free_period
- Inference host: https://cli-chat-proxy.grok.com/v1
- Auth: SessionToken / OIDC business (not live console ApiKey)
- Config intent: preferred_method=oidc, auto_use_included_limits=true
- Team / business identity id: 61fab250-b2c1-40cf-b5b8-628e673a2eeb (Surmount; SuperGrok Heavy)
- Client binary (live refresh): grok-oss 0.2.111 (c87f66a61d94) [stable]

## What the client already does (please do not chase a chrome bug)

1. Free SuperGrok period first: console ApiKey is omitted from primary sampling while free period has headroom.
2. Compact chrome shows honest free-period % (e.g. "6%"), not a fake higher %, and does not paint console · $N while free period has headroom.
3. limits --json and human /limits surface activeDriver, pollSucceeded, includedSource, team postpaid OAuth vs API class, and usage series when a management key is set.
4. Honesty notes already say: SuperGrok included % is the billing poll reading, not proof of included-limit burn; SuperGrok session can move team OAuth / Grok Build Usage $ without proving free SuperGrok period moved.
5. Client will not invent free SuperGrok period debit % and will not hop to the console key to "fix" free period while headroom remains.

## What we need from xAI (pass condition)

Please confirm the intended debit order for SessionToken → cli-chat-proxy on this business SuperGrok Heavy identity, and fix or document why free SuperGrok period % and/or Grok Build productUsage stay flat under multi-hour SuperGrok session load while team OAuth Grok Build class $ moves.

Pass condition for us: free SuperGrok period % and/or Grok Build productUsage % and/or SuperGrok $ extras after free period is full move with SuperGrok session traffic in a way that proves the included pool absorbed the work.

## Exact questions

Q1. Intended debit path for SessionToken → cli-chat-proxy on business SuperGrok Heavy / Grok Build traffic for identity 61fab250-b2c1-40cf-b5b8-628e673a2eeb:
    (a) debit free SuperGrok period (included weekly) first, then SuperGrok $ extras, then team console pools; or
    (b) bill only as team OAuth / Grok Build / defaultCredits Usage without moving free SuperGrok period; or
    (c) something else (please document the order)?

Q2. Why creditUsagePercent / free SuperGrok period used can stay flat under multi-hour load with successful live polls (stuck ~65% prior period; stuck ~6% this period after reset). Is that coarse % granularity, settlement lag (how long?), wrong pool/principal, or missing attribution?

Q3. Why Grok Build productUsage can stay flat under Build-heavy session traffic (54.0 for hours on 2026-08-02; often absent/null on wire this period)?

Q4. Confirm SuperGrok $ extras (prepaidBalance / client dollarExtrasUsd) should not move until free SuperGrok period is exhausted (client already after-burns only at ≥100% used). Confirm this field is SuperGrok extras, not console team prepaid $340.

Q5. Is team OAuth / Grok Build Usage settlement supposed to reduce free SuperGrok period % on the same billing poll the CLI uses, or is it intentionally parallel only?

Q6. Authoritative fields for included weekly % and Grok Build product usage % that the CLI should trust. Any known delay, caching, or 1% quantization we should document client-side?

## Reproduction

1. Business SuperGrok session (SessionToken), host cli-chat-proxy.grok.com, console API key not live.
2. Generate SuperGrok / Grok Build session traffic for hours (or at least multi-poll under load).
3. Snapshot meters:
   grok-oss limits --json
   (repeat ≥2 times with ≥30s wall between samples; or use a long-lived process and re-open /limits)
4. Observe: free SuperGrok period includedUsedPct flat; team postpaid OAuth class and/or usage series OAuth class rising; activeDriver remains supergrok_free_period while used < 100%.
5. Durable multipoll ring (operator host): $GROK_HOME/included_poll_history/<identity>.json (creditUsagePercent only; no tokens).

## Attachments (local; redacted; no secrets)

- Multipoll JSON dumps and history tail from the Surmount repo agent reports (paths listed in the package index).
- Prior full evidence package: c4-supergrok-debit-evidence-package-2026-08-02
- 2026-08-07 addendum and this paste-ready merge

Thank you. We need a server contract or server fix; the client will keep displaying the poll honestly and will not invent free SuperGrok period debit.
```

---

## Live numbers table (both periods)

| Window | Free SuperGrok period | Grok Build productUsage | SuperGrok $ extras | Team OAuth / Grok Build settlement | SuperGrok primary? |
|--------|----------------------|-------------------------|--------------------|------------------------------------|--------------------|
| **2026-08-02** (prior period dogfood) | Stuck ~**65%** for hours; weak **65→66** later | **54.0** flat all day of observability | **$100.29** flat | Team Usage ~**$547.87** week; one-turn +$0.01; M3 OAuth class dominated | Yes; SessionToken + cli-chat-proxy; `console.isLive=false` |
| **2026-08-07…08** (this period after reset) | Stuck ~**6.0%** (94% remaining); healthy `live_poll` both principals | Often **null** on wire this period (not invent) | **$100.29** flat | Postpaid OAuth class ~**$1013–$1014** climbing; multipoll +~$0.42 OAuth in ~35s while free period stayed 6.0%; series OAuth ~**$856** | Yes; `activeDriver=supergrok_free_period`; `console.isLive=false` |

**Reading:** new period stuck-low is **not** replaying the 65% tape. After weekly reset, free period has headroom but does not absorb SuperGrok session / Build-class traffic into the included % poll. Client chrome correctly shows **6%**.

### Live multipoll delta (2026-08-08 UTC, same binary)

| Field | Poll A (~05:58:45Z) | Poll B (~05:59:21Z) |
|-------|---------------------|---------------------|
| includedUsedPct (business + personal) | **6.0** | **6.0** |
| includedSource | live_poll | live_poll |
| pollSucceeded | true | true |
| activeDriver | supergrok_free_period | supergrok_free_period |
| console.isLive | false | false |
| dollarExtrasUsd | 100.29 | 100.29 |
| teamPostpaidOauthClassUsd | **1013.35** | **1013.77** |
| teamPostpaidPeriodTotalUsd | 1019.15 | 1019.57 |
| teamUsageSeriesOauthClassUsd | ~855.88 | ~856.30 |
| teamPrepaidUsd | 340.0 | 340.0 |

Commands:

```bash
~/.cargo/bin/grok-oss --version
~/.cargo/bin/grok-oss limits --json
# spaced multipoll (no dedicated multipoll subcommand; repeat limits):
~/.cargo/bin/grok-oss limits --json > /tmp/c4-poll-a.json
sleep 35
~/.cargo/bin/grok-oss limits --json > /tmp/c4-poll-b.json
# durable ring (no secrets; creditUsagePercent samples only):
ls ~/.grok/included_poll_history/
```

There is **no** separate `multipoll` CLI subcommand. Multipoll is two or more spaced `limits --json` / `/limits` opens (long-lived process preferred) plus durable history under `$GROK_HOME/included_poll_history/`.

### Addendum (2026-08-08 later multipoll, same day)

Client re-verify (`grok-oss 0.2.111`): free SuperGrok period still **6.0%** both principals; SuperGrok $ extras still **$100.29**; `activeDriver=supergrok_free_period`; `console.isLive=false`; `flatPollUnprovenDebit=true`. Spaced multipoll (~8s): team postpaid OAuth class **$1019.67 → $1019.85**, usage-series OAuth **~$862.20 → ~$862.38**, free period flat. Same-day climb vs morning multipoll OAuth **$1013.35** (free period still 6.0%). Full writeup: [`.agents/reports/verify-limits-still-6pct-2026-08-08.md`](verify-limits-still-6pct-2026-08-08.md).

---

## Environment (live refresh)

| Field | Value |
|-------|--------|
| Client binary | `grok-oss 0.2.111 (c87f66a61d94) [stable]` |
| liveSampling | `supergrok_session` |
| livePrincipalRole | `business` |
| activeDriver | `supergrok_free_period` |
| console.isLive | `false` |
| console.keyAvailable | `true` |
| Proxy / inference host (product path) | `https://cli-chat-proxy.grok.com/v1` |
| Business identity fingerprint | `61fab250-b2c1-40cf-b5b8-628e673a2eeb` |
| Personal SuperGrok principal (shared pool) | `58c5f686-4270-4d6d-9c3b-df44559f8457` (same 6.0% pool) |
| Tier (log / prior package) | SuperGrok Heavy |
| Config intent | `preferred_method=oidc`, `auto_use_included_limits=true` |
| Secrets | **None** in this package |

---

## Q1–Q6 (exact)

1. **Q1:** Intended debit path for SessionToken → cli-chat-proxy on business SuperGrok Heavy / Grok Build for identity `61fab250-…` (included first vs team OAuth only vs other order).
2. **Q2:** Why `creditUsagePercent` stays flat under multi-hour load with successful live polls (~65% prior; ~6% this period). Coarse %, lag, wrong pool, or bug?
3. **Q3:** Why Grok Build `productUsage` can stay flat (or absent) under Build-heavy session traffic.
4. **Q4:** Confirm SuperGrok $ extras should not move until free period is exhausted; field identity vs console prepaid.
5. **Q5:** Is team OAuth / Grok Build settlement supposed to reduce free SuperGrok period % on the same poll, or intentionally parallel only?
6. **Q6:** Authoritative fields for included weekly % and Build product usage %; known delay / 1% quantization / wrong pool.

---

## What client already does

| Surface | Behavior |
|---------|----------|
| Design A / free-period-first | Console key omitted from primary while free period has headroom |
| Compact chrome | Honest free-period **%** (e.g. `6%`); not invent; not false `console · $N` under headroom |
| `activeDriver` | free SuperGrok period \| SuperGrok extras \| console key |
| Poll provenance | `pollSucceeded`, `includedSource` (`live_poll` \| `process_cache` \| `shared_pool_fill`) |
| Dual-bill honesty (C6) | Team OAuth $ can move without free-period proof |
| Flat-poll measurement | Process + durable `$GROK_HOME/included_poll_history/`; `flat_poll_unproven_debit` on snapshot / (after 2026-08-07 tree fix) `limits --json` fields + honesty notes |
| M3 class split | Team postpaid OAuth vs API class on limits |
| Slice 4 | SuperGrok $ extras after free period full (code; not live-proved at 100%) |

---

## What we need from xAI

**Pass condition:** free SuperGrok period % and/or Grok Build productUsage % and/or SuperGrok $ extras after free period is full **moves with SuperGrok session traffic** so included absorption is proven.

Not acceptable as "client fixed": inventing a higher free-period %, mashing team OAuth $ into free-period chrome, or hopping to console ApiKey while free period has headroom.

---

## Attachments list (paths)

| Path | What |
|------|------|
| [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](c4-xai-ticket-paste-ready-2026-08-07.md) | **This file** (title + body + tables) |
| [`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](../joins/c4-supergrok-debit-evidence-package-2026-08-02.md) | Full prior-period evidence (Q1–Q7 detail, log series) |
| [`.agents/reports/c4-ticket-addendum-2026-08-07.md`](c4-ticket-addendum-2026-08-07.md) | Post-reset ~6% addendum |
| [`.agents/reports/how-to-fix-c4-free-period-debit-2026-08-07.md`](how-to-fix-c4-free-period-debit-2026-08-07.md) | Operator how-to (ownership, levers) |
| [`.agents/reports/c4-limits-poll-a-2026-08-08T055844Z.json`](c4-limits-poll-a-2026-08-08T055844Z.json) | Multipoll A (redacted limits --json) |
| [`.agents/reports/c4-limits-poll-b-2026-08-08T055844Z.json`](c4-limits-poll-b-2026-08-08T055844Z.json) | Multipoll B (~35s later; OAuth $ up, free period flat) |
| [`.agents/reports/c4-poll-history-business-tail-2026-08-08.json`](c4-poll-history-business-tail-2026-08-08.json) | Durable ring tail summary (6.0% only; no secrets) |
| [`.agents/reports/doubt-free-period-stuck-6pct-2026-08-07.md`](doubt-free-period-stuck-6pct-2026-08-07.md) | Client honesty verdict (6% not a chrome lie) |
| [`.agents/reports/still-6pct-chrome-2026-08-07.md`](still-6pct-chrome-2026-08-07.md) | Compact 6% = Design A success |
| Host (optional attach, no secrets) | `~/.grok/included_poll_history/61fab250-b2c1-40cf-b5b8-628e673a2eeb.json` |

Operator may also attach redacted browser team Usage screenshots if available.

---

## Client measurement fix shipped in-tree (2026-08-07, not invent)

Dense multipoll used to miss `flat_poll_unproven_debit` because the detector only looked at the last two samples (often ~2s apart under load). Product now uses the most recent window that spans ≥30s wall, so high-frequency flat free-period series surfaces honesty notes and (after rebuild) `flatPollUnprovenDebit` on `limits --json`. That improves **ticket evidence**, not free-period %.

Rebuild/install required for the live binary to export the new JSON fields. Evidence in this package already shows flat free period + rising team OAuth without inventing %.

---

## Ownership one-liner

| Owner | Owns |
|-------|------|
| **xAI billing / server** | Free SuperGrok period and Build productUsage debit/settlement under cli-chat-proxy |
| **Operator** | File this ticket once; attach multipoll JSON; track ticket id |
| **This repo** | Honest poll display, free-period-first path, flat/dual-bill notes, multipoll measurement. **Not** the ledger. |
