# C4 ticket addendum: multipoll 20260808T104042Z

**Audience:** operator paste into xAI billing / support ticket (append to paste-ready package).
**Not a product code fix for server debit.** Client free-period-first path is OK; free SuperGrok period debit still unproven.
**Source dir:** `.agents/reports/limits-multipoll-20260808T104042Z/`

---

## One-line delta

Across two `limits --json` samples ~31s apart under SuperGrok session primary: free SuperGrok period used % stayed **6.0%**, SuperGrok dollar credits stayed **$100.29**, team postpaid OAuth / Grok Build class rose **$1068.66 → $1068.82** (+$0.16). Path OK; free SuperGrok period flat; `flatPollUnprovenDebit=true`.

---

## Multipoll field table

| Field | Sample 0 (10:40:42Z) | Sample 1 (10:41:13Z) | Delta |
|-------|----------------------|----------------------|-------|
| liveSampling | supergrok_session | supergrok_session | flat |
| activeDriver | supergrok_free_period | supergrok_free_period | flat |
| consoleIsLive | false | false | flat |
| free SuperGrok period used % (business) | 6.0 | 6.0 | **flat** |
| free SuperGrok period used % (personal) | 6.0 | 6.0 | **flat** |
| SuperGrok dollar credits USD | 100.29 | 100.29 | **flat** |
| teamPostpaidOauthClassUsd | 1068.66 | 1068.82 | **+$0.16** |
| teamPostpaidApiClassUsd | 5.8 | 5.8 | flat |
| teamPostpaidPeriodTotalUsd | 1074.46 | 1074.62 | **+$0.16** |
| teamUsageSeriesOauthClassUsd | ~911.20 | ~911.35 | **rose** |
| pathOk | true | true | P1 OK |
| freePeriodSeries | flat | flat | P2 flat |
| flatPollUnprovenDebit | true | true | unproven |

Summary JSON: `pathOk=true`, `freePeriodSeries=flat`, `flatPollUnprovenDebit=true`, `flatPollObservedBuild=false`, `flatPollObservedExtras=true`.

---

## Client product change after this multipoll (2026-08-08)

Default **block** new sampler turns when free SuperGrok period still has room and flat-poll marks free SuperGrok period debit unproven, unless the operator sets:

```toml
[auth]
allow_spend_when_free_period_debit_unproven = true
```

(or env `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=1`). Does **not** invent free SuperGrok period used %. Server debit pass remains open.

Report: `.agents/reports/impl-limits-over-credits-protect-2026-08-08.md`
