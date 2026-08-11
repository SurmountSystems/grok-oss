# Recon: free SuperGrok period "used to work" vs stuck ~6%

**Date:** 2026-08-08
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only evidence synthesis (poll history, multipoll artifacts, residual, prior C4 packages). No multipoll campaign this turn. No secrets.
**Meters kept distinct:** free SuperGrok period used % ≠ SuperGrok dollar credits ≠ console team prepaid ≠ team postpaid OAuth / Grok Build class ≠ console API credits.

---

## Verdict (plain American English)

**This is not a fresh client bug that invents 6% or stopped selecting free SuperGrok period.** Live multipolls and durable poll history show the xAI billing poll itself returns `creditUsagePercent = 6.0` with `pollSucceeded` / `live_poll`, while SuperGrok session is primary (`activeDriver=supergrok_free_period`, `console.isLive=false`). Team OAuth / Grok Build settlement dollars keep climbing under that same path.

**It is also not “always fine, nothing to see.”** Free SuperGrok period **debit under SuperGrok session load has been unproven / laggy for at least the multi-day window on disk (2026-08-01 evening through 2026-08-08)**. The only **timestamped step** of free SuperGrok period % under observation is a weak **65 → 66** on **2026-08-02**. Earlier in that prior period the meter had already reached **~65%** (so the field can move sometime), but under continuous dogfood observation it sat flat for many hours while team dollars moved. After weekly reset (~2026-08-04), every durable sample on this host is **exactly 6.0%**, with no step, while team OAuth class climbed from roughly **$900s → $1100+**.

**Regression class:** **server-side free SuperGrok period absorption / attribution under cli-chat-proxy SessionToken traffic (open C4)**, observed across **two billing periods**, not a client path hop to console and not a client display invent of 6%. Client work since Aug 2 made free-period-first path **more** sticky on SuperGrok (period-reset memo fix, Design A chrome, omit console under headroom). That cannot force the ledger to step `creditUsagePercent`.

**What “used to work” can honestly mean from disk:**

| Claim | Evidence on this machine |
|-------|---------------------------|
| Free period % used to climb steadily under load | **Not supported.** Observed windows are mostly flat. |
| Free period field can change | **Yes.** Prior period weak **65→66** (2026-08-02T13:38:37Z). Chat productUsage co-moved 11→12. |
| Free period got to mid-period levels | **Yes (prior period).** Stuck observation started already at **65%** (~2026-08-01T20:29Z audits); something had credited that pool earlier in the week. **No timestamped climb series** for 0→65 on disk. |
| This period free period used to step after reset | **No evidence.** First continuous observations (2026-08-07) already **6.0%**; still **6.0%** on 2026-08-08 multipolls and durable ring. |
| Client used to debit free period and a Surmount change broke it | **Disproven for path/display.** Poll is live 6.0 from server; path is SuperGrok free-period-first. A client host/token wrong-path would show `console.isLive=true` or failed polls; multipoll shows neither. |

---

## Last known move of free SuperGrok period %

| Event | When (UTC) | Values | Source |
|-------|------------|--------|--------|
| **Only documented step under load** | **2026-08-02T13:38:37.561Z** (first 66.0 sample) | **65.0 → 66.0** | `.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md` (unified log re-count); 100 samples at 66.0 through ≥14:34:07Z |
| Co-move | Same 66% window | GrokChat productUsage **11.0 → 12.0**; GrokBuild productUsage **still 54.0** | Same package |
| SuperGrok dollar credits | Same windows | **$100.29** flat (10029 cents) | Same |
| **This period (post reset)** | 2026-08-07 evening → 2026-08-08 multipolls | **6.0 → 6.0** (no step) | Reports + multipoll dirs + `~/.grok/included_poll_history/` |

**Last known move = 2026-08-02 afternoon, +1 point (65→66).** No later free SuperGrok period % step is recorded on disk. Period reset then replaced the weekly reading; the new period never shows a multipoll step above 6.0 on this host.

---

## Timeline (stuck windows)

### Prior period (roughly through 2026-08-03 reset)

| Window (UTC) | Free SuperGrok period | Team settlement (class) | Path |
|--------------|----------------------|-------------------------|------|
| ~2026-08-01T20:29Z → 2026-08-02 morning | **65.0%** flat (~1298+ polls in audits; multi-hour flat) | Team Usage week ~$547; OAuth Build lines rising | SuperGrok session, `console.isLive=false` |
| 2026-08-02 **04:08:57Z → 07:29:11Z** | **408** samples at **65.0** | One-turn Usage +$0.01 ~07:04Z under SuperGrok | Same |
| 2026-08-02 **13:38:37Z → ≥14:34Z** | **66.0** (weak +1) | Build productUsage still **54** | Same |
| 2026-08-02 ~15:35Z tip recheck | Polls failed (no %); do not use as meter truth | — | Sparse |

**Implication:** by the time continuous observation started, free period was already **65%**. Climb to 65% is **not timed** on disk. Under multi-hour SuperGrok load, debit was already **failing the C4 pass** (flat for hours; only a weak +1 later).

### Current period (wire: 2026-08-04 → 2026-08-11 weekly)

| Window | Free SuperGrok period | SuperGrok $ credits | Team postpaid OAuth / Grok Build class | Path / notes |
|--------|----------------------|---------------------|----------------------------------------|--------------|
| Period start (wire) | weekly start **2026-08-04T01:25:32Z** | — | — | From live credits config |
| 2026-08-07 evening dogfood | **6.0%** both principals, shared pool | **$100.29** | ~**$943 → ~$1013** class (reports) | `activeDriver=supergrok_free_period`, live_poll OK |
| 2026-08-08 ~05:58Z polls | **6.0** flat | **$100.29** | **~$1013.35 → $1013.77** | `c4-limits-poll-a/b-2026-08-08T055844Z.json` |
| 2026-08-08 multipoll 10:25Z | **6.0** flat | **$100.29** | (earlier multipoll dir) | `limits-multipoll-20260808T102502Z/` |
| 2026-08-08 multipoll **10:40:42Z → 10:41:13Z** | **6.0 → 6.0** | **$100.29 → $100.29** | **$1068.66 → $1068.82** (+$0.16) | `limits-multipoll-20260808T104042Z/summary.json`; `freePeriodSeries=flat`, `flatPollUnprovenDebit=true` |
| Status check same day | still **6.0** | **$100.29** | ~**$1118** (status report) | `.agents/reports/status-c4-still-6pct-2026-08-08.md` |
| Durable ring (business + personal) | **unique % = [6.0] only** | cents **10029** only | (not in ring) | `~/.grok/included_poll_history/{61fab250…,58c5f686…}.json`; tail export `c4-poll-history-business-tail-2026-08-08.json` |
| Live log `unified.jsonl` (samples read 2026-08-08 ~09:36Z+) | every `billing: fetched credits config` **6.0**; GrokChat productUsage **6.0**; GrokImagine blank % | 10029 | — | Period window 2026-08-04..11; SuperGrok Heavy; identity `61fab250…` business |

**Stuck-at-6% window (this period, on disk):** from **first continuous observation 2026-08-07** through **2026-08-08 multipolls and live logs** (at least **~1–1.5 calendar days** of heavy dogfood evidence; period itself started **2026-08-04**, so unobserved early-period climb 0→6 is **unknown**, not disproven).

---

## Auth / config / binary context (what was different)

| Item | Prior period (Aug 2 package) | This period (Aug 7–8) |
|------|-----------------------------|------------------------|
| Live sampling | SuperGrok session **business** | SuperGrok session **business** |
| Host | `cli-chat-proxy.grok.com` SessionToken | Same |
| `console.isLive` | **false** | **false** |
| Free period driver | Included headroom at 65–66% | `activeDriver=supergrok_free_period` at 6% |
| Config (current host) | free-period-first intent in campaign | `preferred_method = "oidc"`, `auto_use_included_limits = true` (`~/.grok/config.toml`) |
| Flat-poll spend gate | N/A then | `allow_spend_when_free_period_debit_unproven = true` (dogfood allow; **does not invent %**) |
| SuperGrok $ credits | **$100.29** flat | **$100.29** flat |
| Team OAuth class | Rising under session (Usage ~$548 week) | Rising **$1013 → $1068 → ~$1118** under session |
| Identity | business `61fab250-b2c1-40cf-b5b8-628e673a2eeb` | Same business + personal `58c5f686…` shared pool |

**Not different in a way that explains “server used to debit free period every turn”:** same SuperGrok session primary, same console-not-live, same extras flat. What **is** different is the **period level** (65–66% prior vs 6% after reset) and the **settlement dollar height** this period.

---

## Client changes that could *look like* “used to work” (checked)

| Hypothesis | Verdict from evidence |
|------------|----------------------|
| Client stuck painting stale 6% while server higher | **No.** Multipoll + `live_poll` + durable ring all **6.0** from live credits fetch. Logs show server `creditUsagePercent:6.0`. |
| Client preferred console / api.x.ai so free period never debited | **No for current multipoll windows.** `console.isLive=false`, `liveSampling=supergrok_session`. (Team $ still rises via dual-bill C6 under SuperGrok session.) |
| Sticky exhaust memo after 100% pinned console forever | **Was a real product bug (fixed 2026-08-03).** Join: `.agents/joins/bug-period-reset-flipped-to-console-2026-08-03.md`. That bug burns **console** path while free period has headroom; it does **not** invent free-period %. Live multipoll is **not** on console primary. |
| Free-period-first / Design A “broke” debit | **Opposite.** Keeping SuperGrok primary under headroom is the path that *should* hit free SuperGrok period if the server debits SessionToken traffic. Chrome showing **6%** is honesty, not a failed hop. |
| Sampling label / billing extension stopped recording debit | **No client-side invent path.** Product re-reports poll; residual bans inventing free-period burn. |
| “Always was server C4 / nothing we can do” as dismissive framing | **Reject the dismissive part.** Client already did honesty + multipoll + ticket package. **Server still owns the ledger.** Operator must **file/escalate** with paste-ready package. Client cannot close C4 by more chrome. |

**Net:** no Surmount client change on disk is a good explanation for free SuperGrok period **stopping** under SuperGrok session. The long window shows free period **rarely steps** while **team OAuth does**, across two periods. That is **server dual-bill / missing free-period absorption**, measured honestly by the client.

---

## Answers to the five recon questions

1. **Did free SuperGrok period used % ever step up under SuperGrok session load on this machine/account?**
   **Yes, once with timestamps:** **65 → 66** on **2026-08-02T13:38:37Z**. Also free period was already **65%** before that day (prior accumulation not timed). **This period: no step** above 6.0 in multipoll or durable history.

2. **When was the last time it moved (timestamp + before/after)?**
   **2026-08-02T13:38:37.561Z UTC: 65.0 → 66.0.** After weekly reset, no recorded move; constant **6.0**.

3. **What was different then?**
   Same SuperGrok session / business identity / console-not-live pattern. Prior period sat at high free-period % (65–66) with Build productUsage 54 on wire. This period shows low free-period % (6) with huge team OAuth class. SuperGrok $ credits **$100.29** both eras.

4. **Did our client change something that could stop free SuperGrok period debit?**
   **No evidence of a client debit stop.** Path remains free SuperGrok period first. Client cannot force `creditUsagePercent` to climb. Sticky-console period-reset bug was fixed **toward** SuperGrok under headroom, not away from it.

5. **Or does history show free SuperGrok period flat for a long time while only team OAuth climbed?**
   **Yes.** That is the dominant pattern on disk:
   - Aug 1–2: free period flat 65% for many hours; team Usage / OAuth moved.
   - Aug 2 afternoon: weak +1 only.
   - Aug 7–8: free period flat 6%; team OAuth **hundreds of dollars** higher under SuperGrok session.

---

## What to try next (concrete)

### Operator / xAI (highest leverage — not optional theater)

1. **File the paste-ready ticket if not already filed**
   - Main: `.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`
   - Attach: `.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md` (65→66 window)
   - Attach: `.agents/reports/c4-ticket-addendum-2026-08-08-multipoll.md` + `limits-multipoll-20260808T104042Z/`
   - Attach: this recon (cross-period “flat while team OAuth climbs”)

2. **Ask xAI explicitly (Q set already in ticket):**
   - Intended debit for SessionToken → cli-chat-proxy on SuperGrok Heavy / Grok Build.
   - Why `creditUsagePercent` can stay flat for hours/days with successful polls.
   - Whether OAuth Grok Build settlement is **supposed** to ignore free SuperGrok period (parallel dual-bill) or is missing attribution.
   - Coarse % / lag / wrong pool.

3. **Do not re-run multipoll for more of the same proof.** Evidence is already packaged. Optional: one `grok-oss limits --json` after any xAI claim of a server fix, to see if free period **steps**.

### Product (only if new red evidence appears)

| If you see… | Then it is a product bug | Else |
|-------------|--------------------------|------|
| `console.isLive=true` while free period &lt; 100% under free-period-first | Yes (path) | Current multipoll: **no** |
| Chrome shows free-period % ≠ live poll `creditUsagePercent` | Yes (display) | Current: **matches** |
| `shared_pool_fill` / failed poll painted as healthy free period | Yes (honesty) | Current: live_poll OK |
| Free period still flat after xAI says debit fixed | Re-open ticket with new multipoll | — |

**No product invent of free SuperGrok period used %.** No hop to console “to fix free period.” No claim that more Surmount rank work closes C4.

### Optional host hygiene (not a C4 close)

- Prefer dogfood binary `grok-oss` with current install after killall of stale TUI (forensic report: old processes vs new defaults).
- Keep `preferred_method=oidc` and `auto_use_included_limits=true` for free-period path proof.
- Flat-poll allow flag only controls **whether dogfood continues under unproven debit**; it does not change the ledger.

---

## Evidence index (absolute paths)

| Artifact | Path |
|----------|------|
| This report | `/home/hunter/Projects/surmount/grok-build/.agents/reports/recon-free-period-used-to-work-2026-08-08.md` |
| Durable poll history (business) | `/home/hunter/.grok/included_poll_history/61fab250-b2c1-40cf-b5b8-628e673a2eeb.json` |
| Durable poll history (personal) | `/home/hunter/.grok/included_poll_history/58c5f686-4270-4d6d-9c3b-df44559f8457.json` |
| Ring uniqueness export | `/home/hunter/Projects/surmount/grok-build/.agents/reports/c4-poll-history-business-tail-2026-08-08.json` |
| Multipoll summary (flat 6%) | `/home/hunter/Projects/surmount/grok-build/.agents/reports/limits-multipoll-20260808T104042Z/summary.json` |
| Status still 6% | `/home/hunter/Projects/surmount/grok-build/.agents/reports/status-c4-still-6pct-2026-08-08.md` |
| Verify still 6% | `/home/hunter/Projects/surmount/grok-build/.agents/reports/verify-limits-still-6pct-2026-08-08.md` |
| Doubt / honesty | `/home/hunter/Projects/surmount/grok-build/.agents/reports/doubt-free-period-stuck-6pct-2026-08-07.md` |
| How to fix C4 (ownership) | `/home/hunter/Projects/surmount/grok-build/.agents/reports/how-to-fix-c4-free-period-debit-2026-08-07.md` |
| Aug 2 65→66 package | `/home/hunter/Projects/surmount/grok-build/.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md` |
| Residual C4 open | `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md` (C4 free-period debit) |
| Config | `/home/hunter/.grok/config.toml` (`preferred_method=oidc`, `auto_use_included_limits=true`) |
| Live billing logs | `/home/hunter/.grok/logs/unified.jsonl` (`billing: fetched credits config`, 6.0) |

---

## One-line for the angry operator

**Free SuperGrok period debit did not “always work under load” in our multipoll windows; the only timed climb is a weak 65→66 on 2026-08-02, and this period is stuck at live server poll 6% while team OAuth keeps climbing under SuperGrok session. That is a real billing absorption problem for xAI, not a Surmount chrome lie and not something a client rebuild will invent away.**
