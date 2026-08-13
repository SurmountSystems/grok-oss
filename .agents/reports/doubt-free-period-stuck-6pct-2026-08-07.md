# Doubt: free SuperGrok period stuck at 6% — honest or wrong?

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only (code + residual + prior joins/reports; no host secret dumps)
**Operator snapshot (evening dogfood, 2026-08-07):**
`liveSampling` SuperGrok session business; `activeDriver: supergrok_free_period`; both SuperGrok principals `includedUsedPct: 6.0`, remaining 94, nextReset August 10, `pollSucceeded: true`, `includedSource: live_poll`, `sharedUnifiedPool: true`; dollarExtras ~$100.29; console not live; teamPrepaid $340; team postpaid OAuth class ~$943.86; team usage series OAuth class ~$786.

---

## Verdict

| Letter | Meaning |
|--------|---------|
| **A** | **6% is the free SuperGrok period used reading per the xAI billing credits poll, and the client is showing it honestly.** Not inventing debit. Not painting dollars as free-period. |

**Confidence:** high (**~0.85**) that the client is not lying, stale-cache-as-live, or wrong-meter for this snapshot.
**Open residual (does not flip to B):** **C4 / branch 2b** still says free SuperGrok period **debit under SuperGrok session load is unproven server-side**. Team Usage / OAuth class $ can rise while free-period % stays flat. That is dual-bill settlement honesty (C6), not a license to invent free-period burn in chrome.

**What “doubt this is right” usually means here:**

1. **“We burned a lot; free period should have moved”** → C4 open residual (server lag / coarse % / dual settlement), not “client stuck inventing 6%.”
2. **“Compact should show dollars”** → Design A free-period-first: while SuperGrok live and used &lt; 100%, compact is bare free-period **%**, even with extras ~$100 and team prepaid $340 on the account. See [`.agents/reports/still-6pct-chrome-2026-08-07.md`](still-6pct-chrome-2026-08-07.md).

Not **B**: this live JSON has `pollSucceeded: true` and `includedSource: live_poll` on both principals. That is not shared-pool fill pretending to be a live poll of a dead JWT, and not cold process_cache alone.

---

## 1. What `includedUsedPct` means on the wire

| Layer | Fact |
|-------|------|
| **Meaning** | Free SuperGrok **period allowance used percent** (included weekly/period pool). 0–100+ as f64. |
| **Not** | SuperGrok $ extras (`prepaidBalance` / `dollarExtrasUsd`); console team prepaid; team postpaid OAuth/API class; team usage series $; Grok Build `productUsage` % (separate field `grokBuildUsagePct`). |
| **Wire source (S1)** | Successful SuperGrok credits poll: `GET {cli-chat-proxy}/billing?format=credits` with session Bearer. Response config field **`creditUsagePercent`** (preferred). Fallback only if missing: `used/monthly_limit`. |
| **Code** | `crates/codegen/xai-grok-shell/src/extensions/billing.rs`: `BillingConfig.credit_usage_percent`, `included_usage_and_period_end`, `fetch_credits_config_with_session`. |
| **JSON field** | `limits --json` → `supergrok.principals[].includedUsedPct` from `PrincipalLimitsSlot.included.used_pct` (`limits_cmd.rs` `principal_cli`). |
| **Compact chrome** | Same included %, floored display: `format!("{pct:.0}%")` when SuperGrok live and used &lt; 100% (`credit_bar.rs` Design A). |
| **Active driver** | `activeDriver: supergrok_free_period` means free period still has headroom and is the active token-economy driver, not that team $ is idle. |

**Provenance flags (dual SuperGrok):**

| `includedSource` | Meaning |
|------------------|---------|
| `live_poll` | This principal’s JWT got a successful credits poll (or active path known OK). |
| `process_cache` | Remembered included fields, not a fill copy. |
| `shared_pool_fill` | Under unified pool, empty dual row filled from sibling; **not** a successful poll of that JWT (`pollSucceeded: false`). |

Tonight’s dump: **both** `live_poll` + `pollSucceeded: true` + `sharedUnifiedPool: true`. Unified pool means both logins share one free-period pool (same 6% is expected), not that the client fabricated the number from a sibling while the active poll failed.

User-guide: `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` (dual poll + burn order + settlement dual-bill).

---

## 2. RESIDUAL C4 / branch 2b / flat poll — evidence free period flat while team $ rises

### Residual law (do not invent debit)

From [`RESIDUAL.md`](../../RESIDUAL.md) (open still):

- **C4 SuperGrok included debit still FAIL / branch 2b (server-side; product honesty held).**
- Dogfood 2026-08-02 under heavy SuperGrok session: included **65%** flat for hours, Grok Build productUsage **54%** flat, SuperGrok $ extras **$100.29** flat; weak later **65→66** while **Build stayed 54**.
- Team console Usage / Management OAuth class moved in the same campaign windows.
- Soft honesty (flat-poll note, Build %, C6) is **shipped**; it does **not** invent a server debit.
- Ticket evidence package: [`.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`](../joins/c4-supergrok-debit-evidence-package-2026-08-02.md).

### Is rising team Usage while free period flat expected or a client bug?

| Interpretation | Status |
|----------------|--------|
| **Expected dual-bill / settlement honesty (C6)** | Product position: SuperGrok **session** can still move **team Usage $** (OAuth / Grok Build class on the team invoice) **without** proving free SuperGrok period % moved, and **without** console ApiKey being live. Honesty note: `NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS` and, when flat-poll fires, `NOTE_FLAT_FREE_PERIOD_SETTLEMENT_RISE_NOT_EXTRAS` in `limits_honesty.rs`. |
| **Client inventing free-period burn** | Forbidden. Residual + product: do not invent C4 pass. |
| **Client hiding free-period burn** | Only if poll is wrong principal, shared_pool_fill painted as healthy live, or cold cache as live without labels. Tonight’s JSON contradicts that pattern. |

So: team OAuth class ~$943 / usage series ~$786 with free period **6%** is **compatible with shipped C6 dual-bill honesty**, same class as Aug 2 (65% + rising team $). It is **not** by itself proof that compact should show dollars or that `includedUsedPct` is a client lie.

### Period context for 6% vs historical 65%

Prior C4 window: next reset around **August 3**. Live Aug 7 reports: free period window roughly **2026-08-04 → 2026-08-11**, nextReset **August 10**, included **6%**. That is a **new period** reading after reset, not “client stuck replaying 65.” Whether **only 6%** after multi-day dogfood is “fair absorption” is still the C4 question to the billing server, not “wrong JSON field.”

---

## 3. Can `live_poll` return 6% forever while real free period burned?

**Yes, if “burned” means team settlement or true free-period absorption the server does not step on this principal’s `creditUsagePercent`.** The client re-reports the poll. It does not simulate debit.

| Scenario | Client behavior | How to prove |
|----------|-----------------|--------------|
| Server truly has only 6% used this period | Shows 6%; honest A | Multi-sample stays ~6% while dogfood is light; period large |
| Server lags / coarse % (C4 class) | Shows flat 6% for a long time under load | Durable poll history flat + `flat_poll_unproven_debit` + optional `billing: poll_delta` never steps included |
| Server dual-bills team OAuth class only | Free period flat; team $ rises | Same + rising `teamPostpaidOauthClassUsd` / usage series while SuperGrok live |
| Client wrong principal / fill | Would show `shared_pool_fill` / `pollSucceeded: false` or auth-fail cold `...%` | Check provenance flags; doctor dual poll health |
| Free period full, extras after-burner | Compact would leave bare % only if product still thinks used &lt; 100% | If API says 100 and UI says 6 → then B |

**Proof kit (already product-supported):**

1. **Multi-sample history** — process ring + durable `$GROK_HOME/included_poll_history/{identity}.json` (Slice 1 + durable multi-process).
2. **`flat_poll_unproven_debit`** on `/limits` and `limits --json` when detector fires.
3. **`billing: poll_delta` log** when included % / Build % / extras cents **step**.
4. **Build product %** and **extras cents** as co-meters (step clears flat; all flat strengthens unproven debit).
5. **Management** team postpaid / usage series for settlement rise without free-period move.

Defaults: ≥2 successful samples spanning ≥30s wall time, included % unchanged; optional Build/extras also flat if present on every sample in the window (`included_debit_unproven` in `included_poll_history.rs`).

---

## 4. Does product flag stuck 6%? (`flat_poll_unproven_debit`)

**Yes.**

| Piece | Where |
|-------|--------|
| Pure detector | `xai_grok_shell::auth::included_debit_unproven` |
| Evidence struct | `FlatPollEvidence { unproven, observed_build, observed_extras }` |
| Wire on snapshot | `LimitsSnapshot.flat_poll_unproven_debit` via `attach_flat_poll_from_history` in `limits_cmd.rs` |
| Human note | `flat_poll_unproven_debit_note(...)` — names SuperGrok included % always; Build / SuperGrok $ extras only if observed flat |
| Storage | Process ring + `$GROK_HOME/included_poll_history/` (no secrets; meter samples only) |
| Record path | Every successful S1 credits config → `record_included_poll_history_from_config` |

**What it does not do:** invent a higher free-period %, hop to console, or claim “you are burning included” from flat % alone (forbidden burn claims in `FORBIDDEN_INCLUDED_BURN_CLAIMS`).

**Tonight’s operator paste** did not list `flatPollUnprovenDebit` / notes. For a single evening dump, flat flag may be true if history already has multi-sample 6.0, or false if only one sample / window too short. Re-check:

```bash
grok-oss limits --json
# look for flatPollUnprovenDebit (or notes mentioning "included debit is unproven")
# optional: two polls ≥30s apart under load
```

If both principals stay 6.0 across a long dogfood window while team OAuth class climbs, expect flat + C6 notes when SuperGrok is live and OAuth dominates postpaid. That is **product honesty surfacing C4**, not a broken 6%.

---

## 5. Tonight’s snapshot vs doubt modes

| Hypothesis | Fits live dump? |
|------------|-----------------|
| Stale process_cache as healthy free period | **No** — `live_poll` + `pollSucceeded: true` |
| Wrong principal / only sibling fill | **No** — both live_poll; unified pool explains **same** 6% |
| Compact wrong for free-period-first | **No** — Design A wants `6%` when SuperGrok live and used &lt; 100% |
| Client inventing debit past API | **No** — residual forbids; chrome shows API % |
| Free period full but UI 6% | **No evidence** — API says 6; remaining 94; nextReset Aug 10 |
| Server free-period debit lag / dual-bill (C4/C6) | **Still open residual** — team $ high, free period low/flat class of observation |
| Expect dollars because extras/prepaid exist | **Policy mismatch**, not meter bug — extras after free period full; console when console live |

Extras **$100.29** matching Aug 2 series is consistent with “extras never stepped” in that campaign; not proof free period is full.

---

## Honest one-line for the operator

**A — Compact 6% is an honest live billing-poll reading of free SuperGrok period used; the client is not inventing debit; if dogfood “should have” burned free period harder, that is open server-side C4 dual-bill residual (team Usage can rise without free-period %), not a stale or lying client meter.**

**Confidence:** ~0.85 on client honesty / API fidelity for this JSON shape. Absorption vs settlement remains residual until multi-sample flat + settlement series close a ticket with xAI (evidence package already assembled 2026-08-02; re-dogfood this period at 6% strengthens a new ticket window, does not rewrite Design A).

---

## Pointers

| Doc / code | Role |
|------------|------|
| `RESIDUAL.md` C4 / 2b / flat poll | Open server debit; honesty held |
| `.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md` | Ticket brief (65% era) |
| `.agents/reports/still-6pct-chrome-2026-08-07.md` | Compact Design A: 6% is success chrome |
| `.agents/reports/live-limits-vs-credits-check-2026-08-07.md` | Same-day live 6% free-period path |
| `billing.rs` `credit_usage_percent` + `GET …/billing?format=credits` | Wire fill |
| `included_poll_history.rs` | Flat detector + durable history |
| `limits_honesty.rs` C6 + flat notes | Dual-bill / unproven debit copy |

**No limits binary re-run in this explore agent** (no shell). Operator paste + on-disk residual/reports/code were sufficient for the verdict.
