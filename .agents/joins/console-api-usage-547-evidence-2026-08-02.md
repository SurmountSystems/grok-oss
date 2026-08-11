# Join: console API Usage $547.87 evidence (2026-08-02)

**Mode:** evidence pin only. No product edits.

## One-line diagnosis

**Console team API Usage proves ~$548 of console-side dollar burn (Jul 27–Aug 2); that is not SuperGrok included debit, and it does not close F1a (flat 65% / $100.29). It does prove F1b: credits-side spend is real and is the operator pain meter while SuperGrok still showed included headroom.**

## Source

| Field | Value |
|-------|--------|
| When | ~2026-08-02 01:03 local (operator screenshot) |
| Page | console.x.ai team **API Usage** |
| URL | `console.x.ai/team/61fab250-b2c1-40cf-b5b8-628e673a2eeb/usage` |
| Explicitly not | `…/grok-business/usage` (license seats/messages) |
| Range | Jul 27 – Aug 2, 2026 |

## Numbers

| Metric | Value |
|--------|--------|
| Total Spend | **$547.87** |
| Tokens | ~**1.14B** |
| Requests | ~**57k** |
| Text | **$332.63** |
| Grok Build | **$214.41** |
| Image & Video | $0.83 |
| Voice | &lt;$0.01 |
| Daily shape | Large bars Jul 30, Jul 31, Aug 1; Aug 2 small so far |

## Meter class (required conclusions)

| # | Conclusion |
|---|------------|
| 1 | Page measures **console team API spend / credits**, not SuperGrok weekly included `creditUsagePercent`. |
| 2 | Therefore it **does not** prove SuperGrok included debit. |
| 3 | It **does** prove heavy **console-side** burn in the dogfood window. Supports “not at limits-first ideal” if spend is this product and/or any console key on this team while SuperGrok still showed ~**65%** included remaining and flat **$100.29** extras. |
| 4 | Prepaid vs Usage reconciliation stays **open** (see table). Do not invent the bucket. |
| 5 | Plan gap split: **F1a** SuperGrok included still unproven to move; **F1b** console Usage spend **proven** (operator pain meter for credits-first failure). |

## Reconcile with prior Management / SuperGrok captures

Same team id `61fab250…2eeb`. Prior joins: `live-auth-path-now-2026-08-02.md`, `business-usage-vs-product-path-2026-08-02.md`, prepaid wire cache.

| Surface | What we saw | How it relates to $547 Usage |
|---------|-------------|------------------------------|
| SuperGrok included % | **65.0** flat under heavy dogfood | Different meter; Usage does not move this |
| SuperGrok $ extras | **$100.29** flat | Different meter (session extras) |
| Live sampling | SuperGrok session; `console.isLive: false` | This product’s primary path was SuperGrok at capture; Usage is team-wide API |
| Management prepaid ledger | **$340** remaining; **0 SPEND** rows in one dump | $547 Usage ≠ prepaid SPEND trail in that dump |
| Management postpaid preview (prior) | ~**$207.56** period via `defaultCreditsIssued` (mostly Grok Build OAuth + some API); `defaultCredits` **$1500** | Suggests free/default pool can absorb spend without prepaid SPEND |
| Dashboard “Credits remaining” composite | ~**$1317** vs prepaid **$340** | Composite / defaultCredits vs prepaid ledger (already noted; not a cents bug) |

**Open reconciliation (do not invent):**

- Spend may hit **default/free credits** (or other non-prepaid pools) rather than the prepaid ledger that showed $340 / 0 SPEND.
- Prepaid *changes* history may omit rows that still appear as Usage chart dollars.
- Windows / product filters (Text vs Grok Build vs OAuth vs API key) may not line up 1:1 with prepaid SPEND.
- Attribution of the $547 to **this** grok-oss binary vs other clients on the same team key is **not** proven by the screenshot alone.

## Plan impact

| Plan id | After this evidence |
|---------|---------------------|
| **F1a** | SuperGrok included debit still **unproven** (unchanged) |
| **F1b** (new split) | Console API Usage spend **proven** ($547.87) — operator pain |
| **F5** | Strengthened: team Usage moved while product live path was SuperGrok; who burned is open |
| **C4** | Still fails until SuperGrok meters move or honesty packages unproven debit; console $ does not pass C4 |
| **C3** | Fail only if **this** product put console keys in chain under included headroom; Design A + live path argued otherwise for primary sampling |

Full plan: [`.agents/plans/limits-first-ideal-2026-08-02.md`](../plans/limits-first-ideal-2026-08-02.md) §2b.
