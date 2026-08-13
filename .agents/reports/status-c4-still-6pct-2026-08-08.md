# Status: free SuperGrok period still ~6% (honest only)

**Date:** 2026-08-08
**Binary (live check):** `grok-oss 0.2.111 (c87f66a61d94)`
**Mandate:** status only. No multipoll campaign. No product code change (no client display invent of 6%).

Meters kept distinct: free SuperGrok period used % ≠ SuperGrok dollar credits ≠ console team prepaid ≠ team postpaid OAuth / Grok Build class ≠ console API credits.

---

## Short answers (operator)

| Question | Answer |
|----------|--------|
| Is 6% still flat vs prior multipoll? | **Yes.** Free SuperGrok period used % is still **6.0%** for business and personal (shared pool). Same number across today’s multipolls and this live check. |
| Expected client vs server? | **Expected client honesty; expected open server C4.** Client reports the billing poll. Server free SuperGrok period debit under SuperGrok session load remains unproven. |
| What client already did | Free-period-first path, honesty notes, flat-poll detector / `flatPollUnprovenDebit`, protect gate (default block turns when debit unproven under headroom), plain language (limits vs credits). |
| What only xAI can fix | Debit free SuperGrok period (`creditUsagePercent` / included pool) when SuperGrok session traffic runs (cli-chat-proxy), so used % steps instead of staying flat while team OAuth / Grok Build settlement climbs. |
| Paste-ready ticket | [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](c4-xai-ticket-paste-ready-2026-08-07.md) plus multipoll addendum [`.agents/reports/c4-ticket-addendum-2026-08-08-multipoll.md`](c4-ticket-addendum-2026-08-08-multipoll.md). |
| Operator next action | **File the ticket with xAI** (if not already filed). Do **not** run multipoll again for proof; evidence is already packaged. |

---

## 1. Is 6% still flat? (numbers)

### Latest multipoll (newest dir)

**Dir:** `.agents/reports/limits-multipoll-20260808T104042Z/`
**Summary:** `pathOk=true`, `freePeriodSeries=flat`, `flatPollUnprovenDebit=true`

| Field | Sample 0 (10:40:42Z) | Sample 1 (10:41:13Z) | Delta |
|-------|----------------------|----------------------|-------|
| free SuperGrok period used % (business + personal) | **6.0** | **6.0** | **flat** |
| SuperGrok dollar credits USD | 100.29 | 100.29 | flat |
| teamPostpaidOauthClassUsd | 1068.66 | 1068.82 | **+$0.16** |
| activeDriver | `supergrok_free_period` | same | path OK |
| consoleIsLive | false | false | not console |
| liveSampling | `supergrok_session` | same | SuperGrok session |

### Same-day earlier multipoll

**Dir:** `.agents/reports/limits-multipoll-20260808T102502Z/`
Same story: free period **6.0** flat both samples; SuperGrok dollar credits **$100.29** flat; team OAuth **$1058.56 → $1058.96**.

### Morning verify multipoll (context)

From [`.agents/reports/verify-limits-still-6pct-2026-08-08.md`](verify-limits-still-6pct-2026-08-08.md) and `c4-limits-poll-*2026-08-08T055844Z.json`: free period already **6.0%** flat while team OAuth was ~**$1013 → $1019**. Hours later multipoll at ~**$1068**. Live now ~**$1118**. Free SuperGrok period did not move.

### Live one-shot (this status turn)

```text
~/.cargo/bin/grok-oss limits --json
```

| Field | Live value |
|-------|------------|
| free SuperGrok period used % | **6.0** business + personal (`includedSource` poll; `pollSucceeded` true) |
| free SuperGrok period remaining | 94% |
| SuperGrok dollar credits | **$100.29** (side meter; not live driver) |
| activeDriver | `supergrok_free_period` (“Active: free SuperGrok period”) |
| liveSampling | `supergrok_session` / business |
| console.isLive | **false** (key available; not primary) |
| console team prepaid | $340.00 |
| team postpaid OAuth / Grok Build class | **$1118.80** |
| team postpaid API class | $5.80 |
| flatPollUnprovenDebit | **true** |

**Conclusion on flatness:** free SuperGrok period used % has been **stuck at 6.0%** all through today’s recorded multipolls and the live check. Team postpaid OAuth / Grok Build class kept climbing under SuperGrok session (~$1013 → ~$1058 → ~$1068 → ~$1118). SuperGrok dollar credits stayed **$100.29**.

---

## 2. Expected client vs server?

| Layer | Status |
|-------|--------|
| **Client path** | **OK.** Prefer free SuperGrok period while it has room; stay on SuperGrok session; omit console as primary under free SuperGrok period headroom; show poll % honestly. Live: `activeDriver=supergrok_free_period`, `console.isLive=false`. |
| **Client display invent of 6%** | **No.** Product surfaces billing poll `includedUsedPct` / `creditUsagePercent`. Wire notes say included % is poll reading, not proof of burn; product does not invent free-period debit. No client display bug found this turn. |
| **Spend order (docs + product)** | Free SuperGrok period limits first, then SuperGrok dollar credits, then console team prepaid / console API credits. While free SuperGrok period has room, do not make the console API key primary. |
| **Server (C4)** | **Open.** Free SuperGrok period debit under SuperGrok session load is unproven: multipoll `freePeriodSeries=flat` + `flatPollUnprovenDebit=true` while team OAuth settlement rises. Only xAI billing can fix absorption of free SuperGrok period. |

Not a client rank fail. “6% should climb after dogfood” is server absorption, not “hop to console to fix free period.”

---

## 3. What is already done on the client

| Work | Role |
|------|------|
| Free SuperGrok period first (chrome + resolve / auto order) | Prefer free SuperGrok period as active driver while headroom remains; do not paint console dollars as the live driver under headroom |
| Honesty notes on `/limits` and `limits --json` | Poll ≠ proof of included burn; flat free SuperGrok period with rising team OAuth is called out; does not invent free SuperGrok period used % |
| Multipoll + `flatPollUnprovenDebit` export | Process history marks unproven free SuperGrok period debit for ticket evidence |
| Protect gate (2026-08-08) | Default **block** new sampler turns when free SuperGrok period still has room and flat-poll marks debit unproven; opt-in `[auth] allow_spend_when_free_period_debit_unproven = true` or env `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=1`. Report: [`.agents/reports/impl-limits-over-credits-protect-2026-08-08.md`](impl-limits-over-credits-protect-2026-08-08.md) |
| Language | Limits not bare “allowance”; credits not bare “extras”; meters named in full |

Client cannot force the server to step free SuperGrok period used %.

---

## 4. What only xAI can fix

Debit the free SuperGrok period included pool when SuperGrok session (cli-chat-proxy) traffic runs, so `creditUsagePercent` / free SuperGrok period used % steps under real load instead of staying flat at 6% while team Grok Build / OAuth settlement dollars climb.

---

## 5. Paste-ready ticket path (still current)

1. **Main package:** [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](c4-xai-ticket-paste-ready-2026-08-07.md)
2. **Multipoll addendum (20260808T104042Z):** [`.agents/reports/c4-ticket-addendum-2026-08-08-multipoll.md`](c4-ticket-addendum-2026-08-08-multipoll.md)
3. **Raw multipoll dir:** `.agents/reports/limits-multipoll-20260808T104042Z/` (`summary.json`, `samples.jsonl`, `fields.jsonl`)
4. How-to / diagnosis depth: [`.agents/reports/how-to-fix-c4-free-period-debit-2026-08-07.md`](how-to-fix-c4-free-period-debit-2026-08-07.md)

This live check still matches the package: free SuperGrok period **6.0%** flat, SuperGrok dollar credits **$100.29**, console not live, team OAuth still higher. Ticket package remains current.

---

## 6. Operator next action

**File the xAI ticket** using the paste-ready package (and multipoll addendum) if not already filed.

Do **not** run multipoll again for more proof. Do **not** expect a client rebuild to move free SuperGrok period used % off 6%. Optional: keep the protect gate default (block unproven spend under free SuperGrok period headroom) until C4 closes, or opt in only if you accept team settlement burn while free SuperGrok period stays flat.

---

## Related reports

- [`.agents/reports/verify-limits-still-6pct-2026-08-08.md`](verify-limits-still-6pct-2026-08-08.md)
- [`.agents/reports/impl-limits-over-credits-protect-2026-08-08.md`](impl-limits-over-credits-protect-2026-08-08.md)
- [`.agents/reports/doubt-free-period-stuck-6pct-2026-08-07.md`](doubt-free-period-stuck-6pct-2026-08-07.md)
- Residual C4: project `RESIDUAL.md` (open server free-period debit)
