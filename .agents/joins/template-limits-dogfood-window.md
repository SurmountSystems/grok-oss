# Dogfood window — limits-first path (template)

Copy this file (or paste the table into a dated join) for each live sampling window.
Meters stay distinct. Do not mash SuperGrok included weekly with SuperGrok dollar
extras, console team prepaid, or team Usage dollars.

**When:** YYYY-MM-DD HH:MM TZ
**Binary / mtime:**
**Config:** `auto_use_included_limits=` · `preferred_method=`
**Join twin (optional):**

## How to capture

```bash
# Hermetic first (always):
just check-limits-first-path

# Live after rebuild (auto_use on, preferred ≠ api_key):
just check-limits-first-live
# or:
timeout 90 ./target/release/grok-oss limits --json | tee /tmp/limits-dogfood-N.json
```

## Path (C1 / C3)

| Field | Value | Expect under limits-first + included used &lt; 100% |
|-------|-------|-----------------------------------------------------|
| `liveSampling` | | `supergrok_session` |
| `liveSamplingLabel` | | SuperGrok session (role…) |
| `livePrincipalRole` | | e.g. business / personal |
| `console.isLive` | | **false** |
| `console.keyAvailable` | | often true (key on file, not live) |

**Pass:** SuperGrok session live, console not live, while any SuperGrok principal
has included weekly used **below 100%**.
**Fail:** `console.isLive=true` or `liveSampling=console_key` under that condition
(unless `preferred_method=api_key`).

## SuperGrok meters

| Meter | Value | Notes |
|-------|-------|--------|
| Included weekly used % (`includedUsedPct`) | | Step 1 usable when **&lt; 100** |
| Included remaining % | | |
| Period / next reset | | |
| SuperGrok dollar extras (`dollarExtrasUsd`) | | Personal SuperGrok prepaid on JWT; **not** console team prepaid |
| `dollarExtrasObserved` | | false → do not claim extras empty |
| Build product % (`grokBuildUsagePct`) if present | | Distinct from included weekly % |
| `sharedUnifiedPool` | | |

## Console team meters (distinct)

| Meter | Value | Notes |
|-------|-------|--------|
| Team prepaid (`teamPrepaidUsd`) | | Management balance; not SuperGrok extras |
| Team postpaid period total | | |
| Team postpaid OAuth / Grok Build class | | Can move on SuperGrok-correct path |
| Team postpaid API key class | | |

## Deltas vs prior window (same binary / config)

| Meter | Prior | Now | Delta |
|-------|-------|-----|-------|
| Included weekly used % | | | |
| SuperGrok dollar extras | | | |
| Postpaid OAuth class $ | | | |
| Postpaid API class $ | | | |
| Team prepaid $ | | | |

## Honesty / flat poll

| Note | Present? |
|------|----------|
| Flat-poll unproven debit (long-lived process) | |
| Session can move team Usage dollars (C6) | |
| Other `notes[]` | |

## Verdict

- [ ] Path certain this window (C1/C3): SuperGrok primary, console not live, included used &lt; 100%
- [ ] Extras-after-full (step 2): N/A until included weekly used ≥ 100%
- [ ] Debit economics: included % and/or Build % stepped with SuperGrok-only load (or honesty only)

**One-line summary:**
