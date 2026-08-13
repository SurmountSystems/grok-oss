# How to fix C4: free SuperGrok period stuck while work burns

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Audience:** operator + parent coordinator (HITL). Not a product invent pass.
**Meters kept distinct:** free SuperGrok period % ≠ SuperGrok $ extras ≠ console team prepaid $ ≠ team Usage / Grok Build OAuth class $ ≠ Grok Build productUsage %.

---

## 1. What is broken

Under real SuperGrok session load (SessionToken → `cli-chat-proxy.grok.com`, console ApiKey **not** live), the free SuperGrok period used percent from the billing poll (`creditUsagePercent` → `includedUsedPct`) does not step in a controlled way with traffic. Dogfood 2026-08-02 sat for hours at **65%** (Build productUsage **54%** flat; SuperGrok $ extras **$100.29** flat), then a weak **65→66** later while Build stayed 54. After period reset, 2026-08-07 dogfood shows free period at about **6% used** with healthy live polls (`pollSucceeded: true`, `includedSource: live_poll`, `activeDriver: supergrok_free_period`) while **team** Grok Build / OAuth class dollars keep climbing. Compact chrome correctly shows **`6%`**. The client is polling honestly. The broken thing is **server-side absorption / settlement of free SuperGrok period against this session path**, not a lying status bar.

**Pass condition (C4):** free SuperGrok period %, and/or Grok Build productUsage %, and/or SuperGrok $ extras after free period is full, **moves with SuperGrok session traffic** in a way that proves the included pool absorbed the work. That pass is still open.

---

## 2. What we cannot fix in this repo alone

| Server / billing concern | Why client cannot close it |
|--------------------------|----------------------------|
| Whether SessionToken → cli-chat-proxy traffic **debits free SuperGrok period** for business SuperGrok Heavy | Ledger lives on xAI billing, not in `grok-oss` |
| Why `creditUsagePercent` stays flat under multi-hour load (coarse %, lag, wrong pool, missing attribution) | Client only **re-reports** the credits poll |
| Why Grok Build `productUsage` can stay flat while Build-heavy traffic runs | Same poll family; product does not invent a higher % |
| Dual settlement: team OAuth / Grok Build **Usage $** rising while free period % stays low/flat | Settlement path is server policy (C6 honesty: team $ can move without free-period proof) |
| Authoritative field contract, cache delay, 1% quantization | Needs xAI answer on poll endpoint semantics |

**Hard residual law:** do **not** invent free SuperGrok period debit in chrome, rank, or tests. Do **not** hop to console ApiKey to "fix" free period while free period still has headroom (that is the wrong fix and violates limits-before-credits / Design A).

**Who owns the real fix:** **xAI server / billing** for ledger and attribution. **Operator** files and escalates with the evidence package. **This product** owns honesty, path correctness, and measurement tools only.

---

## 3. What we can still do (ranked)

### 3.1 File / escalate the ticket with the evidence package (highest leverage)

**Ticket brief (already assembled):**
[`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](../joins/c4-supergrok-debit-evidence-package-2026-08-02.md)

**3-line summary of the ask:**

1. Under heavy SuperGrok **session** traffic (business SessionToken → cli-chat-proxy, **not** live console key), free SuperGrok period and Grok Build productUsage stayed flat for hours while team console Usage $ and Management OAuth-class postpaid clearly moved.
2. Please confirm the intended debit order for this identity (included first vs team OAuth only vs something else) and whether lag, coarse %, wrong pool, or missing attribution explains flat `creditUsagePercent` / Build %.
3. Client already surfaces flat-poll and dual-bill honesty; it needs a **server contract**, not a client fake %.

**Suggested title (from package):**
`SuperGrok included / Build productUsage flat under cli-chat-proxy load while team OAuth Usage $ moves (identity 61fab250…, 2026-08-02)`

**Augment for 2026-08-07 before filing (operator):** add this period window: free period ~**6%**, next reset ~August 10, team postpaid OAuth class hundreds of dollars while SuperGrok still primary and free period has ~94% remaining. That is a **new period** reading after reset (not "stuck replaying 65"), and it strengthens "still unproven absorption."

**Exact questions for xAI (from package Q1–Q6, short form):**

- Q1: Intended debit path for SessionToken → cli-chat-proxy on business SuperGrok Heavy.
- Q2: Why included poll stayed flat under load (coarse %, lag, wrong pool, bug?).
- Q3: Why Grok Build productUsage can stay flat under Build-heavy session traffic.
- Q4: Confirm SuperGrok $ extras should not move until free period is exhausted.
- Q5: Is OAuth settlement supposed to reduce free SuperGrok period % on the same poll, or intentionally parallel team Usage?
- Q6: Authoritative fields for included weekly % and Build product usage %; any known delay / quantization.

**Channels:** operator / human-facing xAI support or billing contact. Product residual ranks this as **Item 2 / server-side C4** (human/xAI). Agents do not invent the ticket as closed by more client code.

### 3.2 Client soft honesty already shipped (do not re-implement)

| What | Where / notes |
|------|----------------|
| Design A: free SuperGrok period first; omit console primary while free period has headroom | Rank + resolve; default `auto_use_included_limits=true` |
| Compact chrome free-period **%** when SuperGrok live and used &lt; 100% | e.g. honest **`6%`**; not invent dollars as free-period burn |
| Sticky exhaust must not paint `console · $N` while free period has headroom | Smoking gun fix 2026-08-07 |
| `activeDriver` / **Active:** free SuperGrok period \| SuperGrok extras \| console key | `limits --json` + human `/limits` |
| Slice 1: poll history + `flat_poll_unproven_debit` + optional `billing: poll_delta` | Process ring + durable `$GROK_HOME/included_poll_history/` |
| Branch 2b: dynamic flat note; Grok Build % when on wire; C6 dual-bill note | Flat ≠ proven burn; team $ can move without free-period proof |
| Slice 3: Management postpaid OAuth vs API class | Attribution for team $, not free-period invent |
| Slice 4: SuperGrok $ extras before console after free period full (C5 **code**; not live-proved at 100%) | After-burner policy only when included ≥ 100% |
| Dual SuperGrok poll honesty: `pollSucceeded`, `includedSource` (live_poll \| process_cache \| shared_pool_fill) | No healthy paint from sibling-only when active poll auth-failed |
| SuperGrok-live team prepaid / postpaid / usage series visibility | Side meters while SuperGrok path is live |
| Bare-resolve / console-edge audit | No accidental console primary under free-period headroom |
| License-page zeros honesty | Grok Business licenses messages/conversations ≠ CLI SuperGrok burn proof |

**Reading the chrome correctly:** still seeing **`6%`** after limits-before-credits is **success for free-period-first chrome**, not a remaining status bug. The remaining anger is "work should have burned free period harder." That is C4 server residual.

### 3.3 Remaining client levers (none invent debit; limited intentional burn)

| Lever | Helps intentional free-period burn? | Notes |
|-------|-------------------------------------|--------|
| Stay on SuperGrok session + proxy with free-period headroom | **Yes (path)** | Design A is correct: this **is** the wire path that *should* debit included if server does. Confirm `liveSampling` SuperGrok, `console.isLive: false`, SessionToken + `cli-chat-proxy`. |
| Wrong identity / dead personal JWT | **Maybe for path health** | Prefer business poll OK; dual honesty demotes auth-failed free-period cache. Re-login stale personal if notes say auth failed. Does **not** invent %. |
| Wrong host / console primary | **Opposite of free period** | Console ApiKey / `api.x.ai` burns team API path, not free SuperGrok period. Do not hop console to "fix" free period. |
| Heavy routing fresher-slot | **Yes for path** | Prefer live/fresher SuperGrok token over stale multi-slot "out of allowance" while session is usable. Already shipped. |
| Force free period to 100% client-side | **Forbidden** | Invent debit. Residual bans it. |
| Mash team Usage $ into free-period chrome | **Forbidden** | C6: team $ rise ≠ free-period burn proof. |
| Prefer personal SuperGrok if pool differs | **Unproven** | Unified pool often shows same % on both principals; not a proven debit fix. Only useful if xAI says business pool is wrong. |
| Wait for free period full → SuperGrok extras → console | **Correct order** | Does not fix C4 flat under headroom; only correct after-burner once free period actually hits 100% on the poll. |

**Bottom line on levers:** the intentional free-period path is already Design A SuperGrok session. There is no secret client wire that debits included while the poll stays flat. If the server dual-bills team OAuth only, **no client lever burns free period** until billing changes.

### 3.4 Dogfood experiments to prove or disprove (operator + long-lived process)

Use a **long-lived** `grok-oss` process (not only cold one-shot CLI) so poll history and flat note can fire.

1. **Baseline snapshot (healthy session billing):**
   ```bash
   grok-oss limits --json
   ```
   Record: `liveSampling`, `activeDriver`, both principals `includedUsedPct`, `pollSucceeded`, `includedSource`, `grokBuildUsagePct` if present, `dollarExtrasUsd`, `console.isLive`, team postpaid OAuth class / usage series if warm, `flatPollUnprovenDebit` / notes.

2. **Multi-poll under load (≥2 samples, ≥30s wall, ideally hours of dogfood):**
   - Same process; re-open `/limits` or re-run `limits --json`.
   - Watch durable history: `$GROK_HOME/included_poll_history/<identity>.json`.
   - Expect: flat free period → `flat_poll_unproven_debit` + honesty notes when detector fires.
   - Optional: unified log `billing: poll_delta` when % / Build / extras **step**.

3. **Settlement co-series (same window):**
   - Team postpaid OAuth / Grok Build class $ (Management) and browser team Usage if available.
   - Rising team $ with flat free period = **C6 dual-bill evidence**, not client invent of free-period burn.

4. **New period / after reset:**
   - Confirm free period is a **new** window (e.g. Aug 4–11, 6% used), not stuck replaying old 65%.
   - If multi-day heavy SuperGrok dogfood still lands near **6%**, that is strong ticket evidence for this period, same class as Aug 2 flat 65%.

5. **Auth health gate:**
   - If billing notes say invalid/expired credentials, fix login first. Cold multipoll with auth fail does **not** prove debit or flat honesty live surface.

6. **What would disprove "client stuck at 6%":**
   - `includedSource: shared_pool_fill` or `pollSucceeded: false` painted as healthy free period → dual honesty bug (product fix).
   - API says 100% used but chrome still 6% → poll/cache/principal bug (product fix).
   - Tonight's shape (`live_poll` + both OK + 6%) is **not** that class.

---

## 4. Concrete next actions (do-this-week)

### Operator

1. **File or escalate the C4 ticket this week** using
   [`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](../joins/c4-supergrok-debit-evidence-package-2026-08-02.md)
   plus a short 2026-08-07 addendum (free period ~6%, team OAuth class high, SuperGrok primary, console not live). Attach redacted `limits --json` dumps and screenshots of team Usage if available. **No secrets.**

2. **Capture one long-lived multi-poll window** under normal SuperGrok dogfood (hours if possible): start process → work → two or more `limits --json` / `/limits` opens with time gaps → save JSON + note whether free period stepped and whether team $ stepped.

3. **Confirm path once:** `preferred_method=oidc`, `auto_use_included_limits=true`, compact shows free-period `%` (not false `console · $N`), `activeDriver=supergrok_free_period` while used &lt; 100%. If any of that is wrong, product bug first; else stay on C4 ticket.

4. **Do not** pin console primary or disable free-period-first to "make meters move." That burns the wrong pool and confuses the ticket.

5. **Track status in residual / board as server-owned Item 2** until xAI answers Q1–Q6 or free period / Build % clearly steps under controlled SuperGrok load.

### Agent (product / coordinator)

1. **Do not invent free SuperGrok period debit** in chrome, tests, rank, or docs. Client invent for limits-first is largely exhausted (residual).

2. **Help package the ticket only when asked:** redacted dumps, title/body from the evidence package, 2026-08-07 period addendum. No fake "fixed in client" claim.

3. **Keep honesty surfaces green** on regression filters (flat poll, C6, dual poll provenance, Design A compact). Fix only real client bugs (false console paint, auth-fail painted healthy, wrong principal fill).

4. **If operator pastes a new multipoll series** where free period still flat and team $ up: append evidence to residual / ticket package; do not rewrite Design A.

5. **Item 3 (extras after free period full)** remains separate: only dogfood when poll actually hits ≥ 100% used. Not a substitute for C4.

---

## 5. Ticket text already exists

| Item | Path |
|------|------|
| Full evidence package + Q1–Q7 + title/body sketch | [`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](../joins/c4-supergrok-debit-evidence-package-2026-08-02.md) |
| Residual open pin (C4 / branch 2b / Item 2) | [`RESIDUAL.md`](../../RESIDUAL.md) § limits residual + rank table row 7 |
| 6% client honesty verdict | [`.agents/reports/doubt-free-period-stuck-6pct-2026-08-07.md`](doubt-free-period-stuck-6pct-2026-08-07.md) |
| Compact 6% is Design A success | [`.agents/reports/still-6pct-chrome-2026-08-07.md`](still-6pct-chrome-2026-08-07.md) |
| Limits-before-credits shipped; C4 still server | [`.agents/reports/impl-limits-before-credits-2026-08-07.md`](impl-limits-before-credits-2026-08-07.md) |
| Dual poll honesty shipped | [`.agents/reports/impl-dual-supergrok-billing-honesty-2026-08-07.md`](impl-dual-supergrok-billing-honesty-2026-08-07.md) |

**3-line ask (repeat):** SuperGrok session path under load did not cleanly debit free SuperGrok period / Build productUsage while team OAuth Usage $ moved; please confirm intended debit order and whether lag, coarse %, wrong pool, or bug; client needs server contract, not invent %.

---

## 6. Ownership one-liner

| Owner | Owns |
|-------|------|
| **xAI billing / server** | Free SuperGrok period and Build productUsage **debit and settlement** under cli-chat-proxy / SessionToken |
| **Operator** | File ticket, escalate, dogfood multipoll, period addendum |
| **This repo (grok-oss)** | Honest poll display, free-period-first path, flat/dual-bill notes, rank that keeps SuperGrok primary under headroom. **Not** the ledger. |

**Fix path:** escalate server with evidence → get contract or server fix → then client only documents lag/quantization if xAI names it. There is no honest "park forever" without naming **xAI** as the owner of the remaining debit bug.
