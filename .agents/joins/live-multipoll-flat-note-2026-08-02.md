# Live multi-poll flat note attempt — 2026-08-02

**Goal:** Two spaced `grok-oss limits --json` polls (~35s gap) to observe
flat-poll honesty fields on a live tip binary. No product code. Do **not**
invent C4 SuperGrok included debit. Do **not** claim C5 without
`includedUsedPct >= 100`.

**Binary:** `/home/hunter/Projects/surmount/grok-build/target/release/grok-oss`
(mtime 2026-08-01 23:55 local; release). Also on PATH as
`/home/hunter/.cargo/bin/grok-oss` (not used for these dumps).

**Artifacts:**

| Poll | Path | mtime (local) | exit | bytes |
|------|------|---------------|------|-------|
| 1 | `/tmp/grok-1000/limits-multipoll-1.json` | 2026-08-02 09:57:19 -0600 | **0** | 1268 |
| 2 | `/tmp/grok-1000/limits-multipoll-2.json` | 2026-08-02 09:57:59 -0600 | **0** | 1268 |

Gap: ~40s between dump mtimes (sleep 35s + second poll). stderr both empty.
JSON payloads are **byte-identical**.

**Summary also at:** `/tmp/grok-1000/grok-live-multipoll-summary.md`

---

## Fields observed (both polls identical)

| Field | Value |
|-------|--------|
| exit | 0 |
| `schemaVersion` | `"1"` |
| `liveSampling` | `supergrok_session` |
| `liveSamplingLabel` | Live sampling: SuperGrok session (business) |
| `livePrincipalRole` | `business` |
| SuperGrok business `includedUsedPct` | **66.0** |
| SuperGrok business `includedRemainingPct` | 34 |
| SuperGrok personal `includedUsedPct` | **66.0** |
| SuperGrok personal `includedRemainingPct` | 34 |
| period / nextReset | Weekly / August 3, 19:25 |
| `dollarExtrasUsd` | **100.29** (both principals) |
| `dollarExtrasObserved` | true |
| `sharedUnifiedPool` | true |
| `console.keyAvailable` | true |
| `console.isLive` | **false** |
| `console.teamPrepaidUsd` | 340.0 |
| `flat_poll*` / `flat_poll_unproven_debit` | **absent** |
| `grokBuildUsagePct` / Build % | **absent** |
| team postpaid (`teamPostpaid*`) | **absent** |
| base honesty note | present (see below) |
| dynamic multi-poll flat note | **not** present |

### Notes array (both polls)

1. SuperGrok billing poll **failed** for principal `58c5f686-427`:
   `Billing service error: Invalid or expired credentials`
   (`auth_kind=bearer`, `x_xai_token_auth=xai-grok-cli`,
   `upstream=PermissionDenied`, `reason=no auth context`).
2. Base C6 note: *SuperGrok included % is the billing poll reading, not proof
   of included-limit burn.*

So: process exited 0 and returned partial SuperGrok numbers + extras, but
the product notes say the **billing poll failed** (auth / no auth context),
not a clean live billing read. This is **not** a healthy multi-poll debit
series under load.

---

## What this does / does not prove

| Claim | Status |
|-------|--------|
| Cold CLI `limits --json` exit 0 on tip release | Yes (both) |
| `liveSampling` SuperGrok session; console not live | Yes |
| Included % present at 66% used | Yes (both principals; flat across the two dumps) |
| `flat_poll_unproven_debit` on JSON | **No** (absent; expected for cold one-shot — no long-lived process history) |
| Dynamic flat-poll honesty note from multi-sample evidence | **No** — not on these dumps |
| Build % / postpaid JSON surfaces | **No** on these dumps |
| **C4** SuperGrok included debit under load | **Not claimed.** Do not invent. Flat 66/66 across two cold polls with a failed billing note is **not** C4 pass. |
| **C5** extras-before-console live after included full | **Not claimed.** included used is **66%**, not ≥100%. |

---

## Multi-poll flat note residual

**Still blocked for live proof of the multi-poll flat note surface.**

- These runs are **two independent cold CLI processes**, not one long-lived
  process that appends poll history and can set `flat_poll_unproven_debit`.
- Billing is **not healthy**: notes report **Invalid or expired credentials**
  / PermissionDenied / no auth context (failure class is auth, not only the
  prior timeout class).
- Residual open item (optional live multi-poll flat note when billing works
  again) remains open. Re-run after: valid SuperGrok session token for the
  billing path, then prefer a **long-lived** process (or product path that
  retains poll history) rather than only two cold shells.

**Not done this turn:** product code, residual rewrite, inventing C4, claiming
C5, filing server C4 ticket package.

---

## Commands used

```bash
timeout 45 ./target/release/grok-oss limits --json > /tmp/grok-1000/limits-multipoll-1.json
# sleep ~35s
timeout 45 ./target/release/grok-oss limits --json > /tmp/grok-1000/limits-multipoll-2.json
```

Cwd: `/home/hunter/Projects/surmount/grok-build`.

---

## Retry same day (2026-08-02 ~10:17–10:18 -0600)

Same release binary, same cold multi-poll procedure (~35s sleep between).
No product code. Still do **not** invent C4; still do **not** claim C5
without `includedUsedPct >= 100`.

**Artifacts:**

| Poll | Path | mtime (local) | exit | bytes | sha256 (16) |
|------|------|---------------|------|-------|-------------|
| retry-1 | `/tmp/grok-1000/limits-mp-retry-1.json` | 2026-08-02 10:17:28 -0600 | **0** | 1268 | `bbae6c7621a5974e` |
| retry-2 | `/tmp/grok-1000/limits-mp-retry-2.json` | 2026-08-02 10:18:07 -0600 | **0** | 1268 | `bbae6c7621a5974e` |

Gap: ~39s between dump mtimes (sleep 35s + second poll). stderr both empty.
Retry payloads are **byte-identical** to each other **and** to the morning
dumps (`limits-multipoll-{1,2}.json`).

**Retry summary also at:** `/tmp/grok-1000/grok-live-multipoll-retry-summary.md`

### Fields observed (retry-1 and retry-2 identical)

| Field | Value |
|-------|--------|
| exit | 0 |
| `liveSampling` | `supergrok_session` (business) |
| SuperGrok business / personal `includedUsedPct` | **66.0** |
| SuperGrok business / personal `includedRemainingPct` | 34 |
| period / nextReset | Weekly / August 3, 19:25 |
| `dollarExtrasUsd` | **100.29** (both principals; `dollarExtrasObserved` true) |
| `sharedUnifiedPool` | true |
| `console.keyAvailable` | true |
| `console.isLive` | **false** |
| `console.teamPrepaidUsd` | 340.0 |
| `flat_poll*` / `flat_poll_unproven_debit` | **absent** |
| Build % (`grokBuildUsagePct` etc.) | **absent** |
| team postpaid | **absent** |
| base honesty note | present |
| dynamic multi-poll flat note | **not** present |

### Notes array (retry both)

1. SuperGrok billing poll **failed** for `58c5f686-427`: Invalid or expired
   credentials (`auth_kind=bearer`, `x_xai_token_auth=xai-grok-cli`,
   `upstream=PermissionDenied`, `reason=no auth context`).
2. Base note: SuperGrok included % is the billing poll reading, not proof of
   included-limit burn.

### Claims (retry)

| Claim | Status |
|-------|--------|
| Cold CLI exit 0 again | Yes |
| Included still 66% used (flat across both retries) | Yes |
| Extras still 100.29 | Yes |
| `console.isLive` still false | Yes |
| `flat_poll*` / multi-poll honesty note | **Still absent** |
| Build / postpaid on JSON | **Still absent** |
| **C4** included debit under load | **Not claimed.** Unchanged auth-fail + flat cold dumps. |
| **C5** extras-before-console after included full | **Not claimed.** included used **66%**, not ≥100%. |

### Residual after retry

**Still blocked.** Auth failure class unchanged vs morning; two more cold CLI
shells still cannot surface long-lived `flat_poll_unproven_debit`. Re-run only
after SuperGrok billing credentials work, preferably against a process that
retains poll history.

```bash
timeout 45 ./target/release/grok-oss limits --json > /tmp/grok-1000/limits-mp-retry-1.json
# sleep ~35s
timeout 45 ./target/release/grok-oss limits --json > /tmp/grok-1000/limits-mp-retry-2.json
```
