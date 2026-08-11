# Post-rebuild limits / dual-auth live diagnosis

**When:** 2026-08-02 ~17:48–17:49 -0600
**Cwd:** `/home/hunter/Projects/surmount/grok-build`
**Binary:** `./target/release/grok-oss` (mtime **2026-08-02 17:44:41** -0600; prior multipoll used release mtime 2026-08-01 ~23:55)
**Scope:** diagnose only. No product edits. No git. CLI only (no TUI).

**Artifacts:**
- `/tmp/grok-1000/limits-post-rebuild.json` (poll 1, ~17:48:14)
- `/tmp/grok-1000/limits-post-rebuild-2.json` (poll 2, ~17:49:03, ~49s later)
- `/tmp/grok-1000/limits-post-rebuild.err` / `-2.err` (both empty)
- `/tmp/grok-1000/billing-log-sample-post-rebuild.txt`
- Summary twin: `/tmp/grok-1000/grok-post-rebuild-diagnose-summary.md`

---

## Commands

```bash
timeout 60 ./target/release/grok-oss limits --json \
  2>/tmp/grok-1000/limits-post-rebuild.err \
  | tee /tmp/grok-1000/limits-post-rebuild.json
# sleep ~35s
timeout 60 ./target/release/grok-oss limits --json \
  2>/tmp/grok-1000/limits-post-rebuild-2.err \
  | tee /tmp/grok-1000/limits-post-rebuild-2.json
```

Both polls: **exit 0**, stderr empty.

---

## Live sampling / Design A

| Field | Poll 1 | Poll 2 |
|-------|--------|--------|
| `liveSampling` | `supergrok_session` | same |
| `liveSamplingLabel` | Live sampling: SuperGrok session (business) | same |
| `livePrincipalRole` | `business` | same |
| `console.isLive` | **false** | **false** |
| `console.keyAvailable` | **true** | **true** |

**Design A looks intact:** console API key is available but **not** the live path while a SuperGrok (business) session is sampling. Cold CLI matches that contract on both polls.

---

## SuperGrok meters (distinct)

Business and personal principals report a **shared unified pool** (`sharedUnifiedPool: true`). Same numbers on both principals, both polls:

| Meter | Value | Notes |
|-------|-------|--------|
| Included weekly used % | **66.0** | Flat vs morning multipoll |
| Included remaining % | 34 | |
| Period / next reset | Weekly / August 3, 19:25 | |
| Personal SuperGrok dollar extras (`dollarExtrasUsd`) | **100.29** | `dollarExtrasObserved: true` |
| Build % in CLI JSON | **absent** | Not a top-level field on `limits --json` |
| `flat_poll*` | **absent** | No multi-poll flat-note surface on cold CLI |

**Do not invent SuperGrok included debit.** Included % is still 66 on every cold poll today (morning multipoll + both post-rebuild). That is a **billing poll reading**, not proof that included weekly allowance burned.

### Raw log corroboration (business identity)

Long-lived shell pid `3138654` (and later cold-CLI pid `926219` near poll time) successfully logs:

- `creditUsagePercent`: **66.0**
- `prepaidBalance.val`: **10029** → **$100.29** extras balance
- `productUsage`: GrokBuild **54%**, GrokChat **12%** (Build split exists upstream; CLI JSON still does not surface Build % as its own field)
- `subscriptionTier`: SuperGrok Heavy
- `identity_id`: `61fab250-b2c1-40cf-b5b8-628e673a2eeb`, role **business**
- `onDemandCap` / `onDemandUsed`: 0 / 0 (no on-demand burn signal in these samples)

Recent successful fetches run about every 30s through the afternoon. One transport glitch ~23:40Z, then recovery. Meter values in those successes stay **66 / 10029**.

---

## Console team meters (distinct from SuperGrok)

| Field | Morning multipoll (~10:17) | Post-rebuild poll 1 | Post-rebuild poll 2 |
|-------|----------------------------|---------------------|---------------------|
| `teamPrepaidUsd` | 340.0 | 340.0 | 340.0 |
| `teamPostpaidPeriodTotalUsd` | **absent** | **253.31** | **253.79** (+0.48) |
| `teamPostpaidOauthClassUsd` | **absent** | **247.51** | **247.99** (+0.48) |
| `teamPostpaidApiClassUsd` | **absent** | **5.8** | **5.8** (flat) |

Post-rebuild surface is **richer** than morning multipoll: team postpaid period total + OAuth class + API class now appear on cold `limits --json`.

Between the two post-rebuild polls (~49s), **OAuth-class team Usage moved +$0.48** while SuperGrok included % and dollar extras stayed flat. That matches the new honesty note (see below): a SuperGrok session can move **team Usage dollars** (OAuth / Grok Build class on the team invoice) without proving SuperGrok **included weekly** moved, and without the console API key being live.

Team prepaid stayed **$340** (no observed debit on that meter in this window).

---

## Billing poll success / fail

Still mixed:

1. **Note on both polls:**
   `SuperGrok billing poll failed for 58c5f686-427: … Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)`

2. Despite that note, **included % and dollar extras are populated** for both business and personal labels (shared pool). Business-path billing in the long-lived shell is **succeeding** (hundreds of `billing: fetched credits config` lines with 66% / 10029).

3. Historical log noise (same day, not all from this rebuild window): timeouts, PermissionDenied / no auth context, "User does not have active subscription for this team", and occasional transport failures to `cli-chat-proxy.grok.com/v1/billing?format=credits`. Test-actor 401s around 16:29Z are separate (WebLogin/Oidc test paths).

**Reading:** one principal (id prefix `58c5f686-427`) still fails auth on billing; the business principal that owns the live meters succeeds often enough to keep 66% / $100.29 on the CLI. Partial fail is **not** "all SuperGrok meters dead."

---

## Honesty notes on CLI (post-rebuild)

Post-rebuild JSON has **4 notes** (morning multipoll had 2):

1. SuperGrok billing poll failed for `58c5f686-427` (PermissionDenied / no auth context) — same class as morning.
2. **New:** console team prepaid process cache may lag up to 60s; UI may keep last successful cents; `grok limits` or `/limits` forces a fresh Management fetch.
3. SuperGrok included % is the billing poll reading, not proof of included-limit burn — same base honesty.
4. **New:** SuperGrok session can still move team Usage dollars (OAuth / Grok Build class) without proving SuperGrok included weekly moved, even when the console API key is not live.

Those extra notes line up with observed live behavior (postpaid OAuth moved; included flat; `isLive: false`).

---

## C4 / C5

| Claim | Status |
|-------|--------|
| **C4** SuperGrok included weekly moved / debit proven | **Still unproven.** Included stayed **66%** across morning multipoll and both post-rebuild polls. No observed included % delta. Do not invent SuperGrok debit. |
| **C5** included ≥ 100% and extras after-burner live | **No.** Included is **66%** (&lt; 100%). Dollar extras balance **$100.29** is **observed** as a prepaid extras balance, not evidence that after-burner mode is active. |

Team postpaid OAuth **did** move in this window. That is **console / team invoice Usage**, not SuperGrok included weekly, and not a substitute for C4.

---

## Diff vs last multipoll (66% / $100.29 / billing auth fail)

Baseline: `/tmp/grok-1000/limits-mp-retry-{1,2}.json` (~10:17–10:18), summary `/tmp/grok-1000/grok-live-multipoll-retry-summary.md`.

| Aspect | Morning multipoll | Post-rebuild (~17:48) |
|--------|-------------------|------------------------|
| Binary mtime | 2026-08-01 ~23:55 | **2026-08-02 17:44** |
| Exit / stderr | 0 / empty | 0 / empty |
| liveSampling / role | supergrok_session / business | same |
| included % / extras $ | 66.0 / 100.29 | **same (flat)** |
| console.isLive / keyAvailable | false / true | same (Design A holds) |
| team prepaid | 340.0 | 340.0 |
| team postpaid fields | **absent** | **present** (total/OAuth/API) |
| Postpaid movement across 2 polls | n/a (identical payloads) | OAuth **+0.48** in ~49s |
| Billing fail note (58c5f686) | yes | yes (unchanged class) |
| Extra honesty notes (cache lag, team Usage without included proof) | no | **yes** |
| `flat_poll*` | absent | absent |
| Build % on CLI JSON | absent | absent (54% only in raw billing log) |
| C4 / C5 | not claimed / not claimed | **still not claimed** |

**Bottom line:** rebuild did not unlock C4 or C5. SuperGrok included and extras meters look **flat and honest**. Design A (console not live under SuperGrok session) still holds. New/visible gains vs morning multipoll: **team postpaid breakdown on CLI**, **live OAuth-class Usage movement** under SuperGrok session with console not live, and **clearer honesty notes** about cache lag and team Usage vs included weekly.

---

## TUI / force-refresh

This diagnosis is **CLI only** (`limits --json`). Notes say opening `/limits` or running `grok limits` forces a fresh Management fetch. Cold CLI already returned updated postpaid figures between polls, so CLI path is enough to see team Usage move. TUI force-refresh was **not** exercised here; not required to conclude Design A / C4 / C5 from these dumps.
