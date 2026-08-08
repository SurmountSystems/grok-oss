# Plan inventory: Grok Business license Usage zeros vs team Grok Build burn

**Date:** 2026-08-07
**Mode:** read-only (code, residual, research, prior reports)
**Operator pain (screenshots ~16:32 same day):**

1. **Image 1:** console.x.ai → Platforms → **Grok Business** → **Usage**
   (`…/grok-business/usage`) for team Surmount, Jul 31–Aug 7 2026:
   Total active users **0**, messages **0**, conversations **0**,
   "No data available for this query".
2. **Image 2:** console.x.ai team **Usage** (`…/usage`, not licenses):
   Spend about **$823.71**, almost all **Grok Build**, ~79k requests,
   ~1.7B tokens, Aug 1–7 2026. Text **$0.00**; Grok Build carries the spend.

**Operator claim:** dual SuperGrok billing honesty did **not** resolve "the problem."
They want a **plan** to fix/address this comprehensively.

---

## Prior SoT (do not reinvent)

| Path | Role |
|------|------|
| [`.agents/reports/plan-grok-business-usage-inventory-2026-08-04.md`](plan-grok-business-usage-inventory-2026-08-04.md) | Full meter taxonomy + A/B/C options (still accurate) |
| [`doc/dev/research/console-team-business-usage-meter-2026-07-30.md`](../../doc/dev/research/console-team-business-usage-meter-2026-07-30.md) | Management API; license API research pin (no public API) |
| [`.agents/reports/impl-supergrok-live-team-usage-2026-08-04.md`](impl-supergrok-live-team-usage-2026-08-04.md) | SuperGrok-live team prepaid + **license honesty note** shipped |
| [`.agents/reports/impl-dual-supergrok-billing-honesty-2026-08-07.md`](impl-dual-supergrok-billing-honesty-2026-08-07.md) | Dual **poll** honesty (AuthFailed, fill provenance). **Different problem.** |
| `RESIDUAL.md` §4 Half B | License charts **non-goal**; Half B = team API prepaid/postpaid/series |
| `FORK.md` billing meters bullet | Same non-goal; meters distinct |
| User-guide `02-authentication` § three surfaces | License page vs team API Usage $ vs TUI meters |
| User-guide `04-slash-commands` `/limits` | Postpaid + series; license note |

---

## One-line answer

**The zeros page is Grok Business *license seat* usage (messages / conversations / active users). grok-oss dogfood does not drive that page and cannot fill those charts. Real burn for this product shows as team Usage dollars under Grok Build (OAuth settlement), which the operator already sees at ~$824. Dual SuperGrok poll honesty fixed a different bug (which SuperGrok login’s free-period % is real).**

---

## 1. Why dual SuperGrok billing honesty did not "fix" this

| Work (2026-08-07) | What it fixed | What it never claimed |
|-------------------|---------------|------------------------|
| Dual SuperGrok billing honesty (Option B) | Per-identity poll Ok / AuthFailed; shared-pool fill labels; active chrome not painted from sibling-only; doctor poll health; rank prefers poll-OK | License charts, console page guidance, Grok Build $ prominence |
| SuperGrok-live team usage (2026-08-04) | Footer / `/usage` team prepaid while SuperGrok live; **always-on** `/limits` note that license page ≠ product meters | Making license page non-zero |

So the operator is correct that dual-poll honesty did not move Image 1. That work was never scoped to Image 1. The remaining gap is **operator path and prominence of the *right* meter**, not missing dual-auth poll plumbing.

---

## 2. Meter map (keep distinct)

| # | Plain name | Browser / wire | Product today |
|---|------------|----------------|---------------|
| **1** | Free SuperGrok period allowance used % | SuperGrok session `creditUsagePercent` | Status bar `%`, `/limits`, `/usage` |
| **2** | SuperGrok dollar extras | Session `prepaidBalance` | `/limits`, footer after included full |
| **3** | Grok Build **product usage %** | Wire `productUsage` on session billing | `/limits` / `/usage` line when observed |
| **4** | Console team **prepaid** remaining | Management `GET …/prepaid/balance` | Footer (console live or SuperGrok live when known); `/limits` Balance |
| **5** | Team **postpaid** OAuth vs API class | Management `GET …/postpaid/invoice/preview` | `/limits` + `limits --json` (`teamPostpaid*`) |
| **6** | Management **usage series** (USD by description) | Management `POST …/usage` analytics | Explicit `grok limits` / TUI `/limits` collect (not every background poll) |
| **7** | Browser **team Usage $** | console.x.ai `…/usage` | Browser only; product mirrors class via 5–6, not full chart UI |
| **8** | **Grok Business licenses** messages / conversations / active users | `…/grok-business/usage` (session cookie) | **Not wired. No client. Not claimed.** |

**Naming trap (still active):** residual "console Grok Business Usage **class**" / Half B means meters **4–7** (team API $). The Platforms dropdown **Grok Business** (licenses subtitle) is meter **8**. Same word "Business"; different products.

**Screenshot map:**

| Screenshot | Meter |
|------------|--------|
| Image 1 (zeros) | **8** licenses |
| Image 2 ($823 Grok Build) | **7** (browser view of settlement that **5–6** also attribute as OAuth / Grok Build class) |

---

## 3. What product shows today (code-backed)

### Surfaces that work for dogfood burn

| Surface | Content relevant to Image 2 |
|---------|-----------------------------|
| Status bar | Free SuperGrok period `%` only when SuperGrok live and included has room (Design A). Not team $ series. |
| Footer | SuperGrok "left" / extras; **team prepaid $** when Management cents known (also SuperGrok-live since 2026-08-04) |
| `/limits` + `grok limits` | SuperGrok rows; Console API Balance (prepaid); postpaid OAuth vs API $; usage series OAuth/API USD + top descriptions; Notes stack |
| `limits --json` | `teamPrepaidUsd` / gap; `teamPostpaid*`; `teamUsageSeriesOauthClassUsd` / `ApiClassUsd`; SuperGrok principals + optional `grokBuildUsagePct` |
| Soft `/usage` | SuperGrok meters + team prepaid line when known |
| Local Token Economy `/spend` | Local book vs Management samples (USD class, not license msg counts) |

### Honesty already on `/limits` (always)

From `limits_honesty.rs`:

```text
Note: the console Platforms → Grok Business licenses page (messages / conversations)
is not a SuperGrok or team Management meter in this product. Dogfood burn shows on
SuperGrok included % / extras and on team prepaid / postpaid / usage series when a
management key is set.
```

Plus C6 when SuperGrok live and OAuth postpaid dominates:

```text
Note: SuperGrok session can still move team Usage dollars (OAuth / Grok Build class
on the team invoice) without proving SuperGrok included weekly moved, even when the
console API key is not live.
```

### What product does **not** show

- License messages / conversations / active users (no API, no scrape).
- Full browser-style team Usage charts in the TUI (text series totals only).
- Footer always-on postpaid OAuth class or series window (series is **limits-explicit** collect).
- Doctor dual-auth lines about the **wrong browser page** (doctor has dual **poll** health; no license-page CTA found in shell doctor grep).

### Key code index

| Area | Path |
|------|------|
| License + C6 honesty | `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` |
| Limits snapshot / series | `…/views/limits_snapshot.rs` |
| CLI collect prepaid/postpaid/series | `…/limits_cmd.rs` |
| Management client | `crates/codegen/xai-grok-shell/src/auth/xai_management.rs` |
| SuperGrok credits | `…/extensions/billing.rs` |
| Footer / SuperGrok-live team $ | `…/views/credit_bar.rs` |

---

## 4. Product vs console server ownership

| Question | Answer | Evidence |
|----------|--------|----------|
| Can the client fill Grok Business **license** usage charts? | **No.** No public Management (or other) API for license messages / conversations / active users. Product law: no HTML scrape of console.x.ai. | Research pin 2026-08-04 in `console-team-business-usage-meter-2026-07-30.md`; grep: no license usage client |
| Is dogfood path SuperGrok OAuth / Grok Build billed to **team Usage**? | **Yes** (settlement class). SuperGrok session → cli-chat-proxy; team invoice / browser Usage attributes bulk spend as **Grok Build OAuth**, often while `console.isLive=false`. | C6 note; F1b / console-burn joins; operator Image 2 (~$824 Grok Build, Text $0) |
| Why license zeros while team Usage has ~$823 Grok Build? | **Different products.** CLI SuperGrok dogfood never posts seat message counters. Team Usage $ is OAuth/API settlement for inference. License page is seat/chat-product attribution (or other clients xAI maps to licenses). | Meter 8 vs 7; live Design A omits console ApiKey while free period has room |
| Who owns "fix zeros on Image 1"? | **xAI console / license product**, or seated Grok Business chat clients, not grok-oss. Client can only **stop misleading** and **surface the right meters**. | Product non-goal in residual/FORK |

### Settlement story (plain English)

1. Operator runs grok-oss with SuperGrok session (Business role common in dogfood).
2. Inference hits `cli-chat-proxy.grok.com` with SessionToken.
3. Free SuperGrok period % and optional Build product % move (or stay flat: C4 still server-side residual).
4. Separately, **team** billing settles much of that traffic as **Grok Build OAuth** dollars on the **team Usage** page and Management postpaid/series.
5. Platforms → Grok Business → licenses charts are **not** that settlement path. Zeros there do **not** mean Heavy or grok-oss is idle.

---

## 5. Options for a real product-side "fix"

### A) Honesty-only (guidance)

**Do:** Strengthen operator guidance so Image 1 is not the dogfood proof surface.

| Touch | Idea |
|-------|------|
| `/limits` Notes | Keep license note; optionally **name the right browser URL pattern** ("team Usage / spend charts, not Platforms → Grok Business licenses") without inventing data |
| `grok doctor` | One plain line: dogfood burn proof = `/limits` team postpaid/series + browser **team** Usage, not license Usage |
| User-guide | Already has three-surface table; elevate in troubleshooting / limits section with "if you see zeros on licenses…" |
| Residual / chat | Close the alarm: zeros expected |

**Effort:** hours (docs + short honesty/doctor string + tests on copy).
**Does not:** change Image 1; does not make Grok Build $ unavoidable without opening `/limits`.
**Fits:** problem is largely **wrong page**, and honesty already half-exists.

### B) Surface team Usage / Grok Build series more prominently

**Do:** Make the **correct** meter hard to miss while SuperGrok is live (operator should not need the wrong console page).

| Slice | Detail |
|-------|--------|
| B1 | Always show postpaid OAuth / Grok Build class $ on `/limits` Console block when Management works (already largely true; lock prominence + ordering) |
| B2 | On SuperGrok live + OAuth class dominates: footer chip or second line e.g. team postpaid OAuth class period $ (or short "team Usage class: Grok Build $N") without folding into prepaid $N |
| B3 | Optionally fetch usage series on the same cadence as prepaid/postpaid background, or at least on TUI `/limits` open (series today is explicit collect-heavy) |
| B4 | Compact status bar stays free-period % under Design A (do **not** replace % with team $); optional secondary only if dogfood insists |

**Effort:** ~1–3 implementer days with TDD (footer merge, snapshot, FetchBilling policy).
**Does not:** fill license charts.
**Fits:** operator already has Image 2 truth; product should make Image 2-class data the default proof inside the TUI.

### C) Management API for license usage

**Do:** Only if a **documented** public API appears for license messages/conversations/active users.

| Status | Finding |
|--------|---------|
| Public docs (accessed research 2026-08-04) | Prepaid, postpaid, invoices, **POST usage USD analytics**. No license seat counters. |
| Product law | No invent endpoints; no scrape |

**Effort:** blocked until docs exist; then multi-day client + UI.
**Recommend:** park until xAI documents it or operator accepts scrape (product forbids scrape).

### D) Hybrid B + A (recommended)

Ship **A** immediately (close the wrong-page alarm) and a **small B** slice so Grok Build / team OAuth class dollars are visible without hunting.

| Priority | Work |
|----------|------|
| P0 | A: doctor + user-guide troubleshooting + optional sharper `/limits` note pointing to **team Usage**, not licenses |
| P1 | B: SuperGrok-live prominence for **postpaid OAuth / Grok Build class** (and series totals when already fetched) on footer and/or always-on `/limits` top of Console block |
| P2 | Optional series fetch on background FetchBilling or every `/limits` open if P1 still feels thin |
| Out | C until public API; invent license counts; scrape console HTML; re-open dual poll honesty as greenfield |

---

## 6. Recommendation (with evidence)

**Recommend D (hybrid B+A), with A first if effort is tight.**

| Why not A alone | Why not B alone | Why not C |
|-----------------|-----------------|-----------|
| Honesty note already ships and operator still opened the wrong page; guidance alone may be weak without always-visible Grok Build $ | Without explicit "wrong page" copy, someone will keep bookmarking licenses | No public license API; invent/scrape forbidden |

| Why dual honesty is out of scope for this plan | Evidence |
|-----------------------------------------------|----------|
| Different contract (which SuperGrok JWT’s free-period % is live) | `impl-dual-supergrok-billing-honesty-2026-08-07.md` |
| Does not mention licenses or team Usage browser pages | Same report |

| Live operator evidence | Interpretation |
|------------------------|----------------|
| Image 1 all zeros | Expected for meter 8 under CLI SuperGrok dogfood |
| Image 2 ~$824 Grok Build | Expected settlement for heavy Build/OAuth dogfood; proves product/team path is **not** idle |
| Earlier week ~$547 team Usage (F1b) same pattern | Stable multi-week story |

**Success criteria for this plan (product):**

1. Operator can answer "is dogfood burning team money?" from `grok limits` / `/limits` without opening console.
2. Operator is steered away from Platforms → Grok Business → licenses as the dogfood proof page.
3. No fake license message/conversation numbers.
4. Dual SuperGrok poll honesty remains shipped residual, not re-lit as greenfield.

**Non-goals (explicit):**

- Invent fake license chart data.
- Scrape console HTML.
- Re-open dual poll honesty as a new feature if already on branch.
- Make license charts non-zero via CLI SuperGrok traffic (server/product ownership elsewhere).
- Merge free SuperGrok period %, SuperGrok dollar extras, team prepaid, and team postpaid into one number.

---

## 7. Suggested implement slices (for a later plan approve)

| Slice | Outcome | TDD anchors (existing or new) |
|-------|---------|--------------------------------|
| **A1** | Doctor one-liner: license zeros ≠ idle; use team Usage / `/limits` | New doctor format test; no secret dumps |
| **A2** | User-guide troubleshooting short section | Doc only |
| **A3** | Optional honesty string names "team Usage spend charts" explicitly | Extend `limits_honesty` contract tests |
| **B1** | SuperGrok live + postpaid OAuth > 0 → footer or limits-first line with Grok Build class $ | Red: footer merge test; green: same |
| **B2** | Series OAuth class on SuperGrok-live `/limits` without requiring operator to know collect path | Snapshot + limits_cmd tests already for series JSON |

Order: **A1+A2** → **B1** → **A3/B2** as polish.

---

## 8. Operator dogfood (no secrets)

```bash
# Right product surfaces (not Platforms → Grok Business → licenses)
grok-oss limits
grok-oss limits --json
# Expect when Management key works:
#   teamPrepaidUsd or teamPrepaidGap
#   teamPostpaidOauthClassUsd / teamPostpaidApiClassUsd
#   teamUsageSeriesOauthClassUsd / teamUsageSeriesApiClassUsd
# Notes: license page ≠ SuperGrok/team Management; C6 if OAuth dominates

# Browser proof of burn (same class as Image 2):
#   console.x.ai → team → Usage (spend / Grok Build)
# Not:
#   Platforms → Grok Business → Usage (messages / conversations)
```

Management setup if gaps: `grok login --management-key` and optional
`[endpoints] management_team_id`.

---

## 9. Bottom line

| Question | Answer |
|----------|--------|
| Why Image 1 is zero? | License seat page; CLI SuperGrok dogfood does not drive it. |
| Why Image 2 has ~$823 Grok Build? | Team OAuth / Grok Build settlement of dogfood (and related) traffic. |
| Did dual SuperGrok honesty fix this? | **No** (different bug). |
| Can product fill license charts? | **No** (no API; no scrape). |
| Best product response? | **Hybrid D:** wrong-page honesty (A) + prominent team / Grok Build $ while SuperGrok live (B). |
| Ship or park C? | **Park** until public license-usage API exists. |

---

## 10. Residual / board language (for parent after plan)

Suggested board titles (complete thoughts, plain English):

- `feat:point-dogfood-away-from-grok-business-license-usage-page`
- `feat:show-team-grok-build-oauth-dollars-while-supergrok-live`
- Non-goal pin already in residual/FORK: license charts non-zero.

Do **not** file as "fix dual SuperGrok poll" or "fill license Usage zeros in product."
