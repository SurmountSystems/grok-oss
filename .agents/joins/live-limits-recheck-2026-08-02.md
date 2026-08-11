# Live limits recheck — 2026-08-02

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 explore (read-only; join docs only)
**When:** 2026-08-02 (this recheck turn)

Also: `/tmp/grok-1000/grok-live-limits-recheck-summary.md`

---

## One-line status

**Shell recheck 2026-08-02T15:35Z:** `timeout 45 ./target/release/grok-oss limits --json` → **exit 0**. Sparse tip JSON: `liveSampling=supergrok_session`, `console.isLive=false`, `teamPrepaidUsd=340.0`. **No** `includedUsedPct`, `dollarExtrasUsd`, `grokBuildUsagePct`, `flat_poll*`, or team postpaid fields (both SuperGrok billing polls failed). **Do not claim C4 closed.** **Do not claim C5 live** (included % not present; not ≥100%). Prior log multi-sample (65→66, Build flat 54) still in sections below; not re-proven on this CLI poll.

---

## What this agent could / could not run

| Check | Result |
|-------|--------|
| `target/debug/grok-oss`, `target/release/grok-oss` present | **Yes** |
| Shell `grok-oss limits --json` (timeout ~30s) | **Not available** — no shell tool in this explore session; exit code **n/a** |
| Auth fail from a live CLI re-exec | **Not observed this turn** (no re-exec) |
| Read prior dumps `/tmp/limits-live*.json` | **Yes** (same shape as morning dogfood) |
| Tail/grep `~/.grok/logs/unified.jsonl` billing | **Yes** — **new vs Slice 2 morning join** |
| Tokens / secrets | None printed |

---

## Stale CLI dump baseline (not re-fetched)

Sources: `/tmp/limits-live.json`, `/tmp/limits-live-now.json`, `/tmp/limits-live-burn.json`
(Prior live exit 0 documented in `.agents/joins/live-auth-path-now-2026-08-02.md`.)

| Field | Value on dump |
|-------|----------------|
| `liveSampling` | `supergrok_session` |
| `liveSamplingLabel` | Live sampling: SuperGrok session (**business**) |
| `livePrincipalRole` | `business` |
| SuperGrok `includedUsedPct` | **65.0** (business + personal rows; unified pool) |
| SuperGrok `dollarExtrasUsd` | **100.29** (`dollarExtrasObserved: true`) |
| Period | Weekly · next reset **August 3, 19:25** |
| `console.keyAvailable` | `true` |
| `console.isLive` | **`false`** |
| `console.teamPrepaidUsd` | **340.0** |
| `grokBuildUsagePct` on principal JSON | **absent** on these dumps |
| M3 `teamPostpaid*Usd` / `flat_poll*` | **absent** on these dumps |
| Notes | Personal principal poll fail (expired OIDC `58c5f686-427…`); base honesty: included % is poll reading, not proof of burn |

**Gap:** dumps predate or predate-rebuild of Slice 3/1 JSON surfaces. Code/tests own `teamPostpaidOauthClassUsd` / `teamPostpaidApiClassUsd` and `flat_poll_unproven_debit` attach (see Slice 1/3 joins). **Not re-proven on a tip binary CLI this turn.**

---

## Live log recheck (multi-sample series)

Source: `~/.grok/logs/unified.jsonl` — `billing: fetched credits config`
Identity: `61fab250-b2c1-40cf-b5b8-628e673a2eeb` · role **business** · **SuperGrok Heavy** · pid **3138654** (long-lived shell).

| Window (UTC) | `creditUsagePercent` | GrokBuild % | GrokChat % | prepaidBalance (extras cents) |
|--------------|----------------------|-------------|------------|-------------------------------|
| ~04:08 → ~07:06+ (and prior dogfood) | **65.0** | **54.0** (once productUsage logged) | **11.0** | **10029** ($100.29) |
| First **66** sample seen | **2026-08-02T13:38:37.561Z** | | | |
| ~13:38 → ≥14:05 (sampled) | **66.0** | **54.0** (still flat) | **12.0** | **10029** (still flat) |

### Interpretation (honest)

1. **Included weekly % moved once** in the logged series: **65 → 66** (about +1 point). **GrokChat** moved **11 → 12**. That is **not** inventing movement; it is multi-hour log evidence, not a one-shot CLI invent.
2. **GrokBuild productUsage stayed 54.0** across the whole productUsage-observability window (from ~06:29 through 14:05 samples). Heavy SuperGrok **Build** session traffic earlier did **not** step the Build % meter in this log.
3. **SuperGrok $ extras** stayed **$100.29** the whole time.
4. **C4 discipline (per task):**
   - **Do not claim C4 debit from a single poll.**
   - Morning Slice 2 dogfood correctly recorded **flat 65** under load → **C4 fail** then.
   - This recheck: **partial meter motion** later in the day (top-level included + Chat), **Build still flat**. Treat as **weak / laggy / coarse-%** evidence that the SuperGrok included reading **can** move, **not** as a clean “limits-first Build burn proven under controlled dogfood” close. Residual branch **2b** (server lag / coarse %) remains reasonable for Build; top-level included is no longer “forever 65.”
5. **C5:** included ~**66% used** (~34% remaining). **Not ≥ 100%.** Slice 4 after-burner **code-only**. **Do not claim C5 live.**
6. **`flat_poll_unproven_debit`:** not visible on dumps; cold one-shot CLI would not invent multi-sample history. Long-lived process has history in principle; **not re-proven** on tip binary this turn.
7. **Console live:** dumps `isLive: false`. No new CLI contradiction. Hermetic **test** management postpaid/prepaid lines appear later in the same log (fake team ids) — ignore for live meters.

---

## Cached M3 postpaid (unchanged attribution picture)

`/tmp/mgmt-preview.json` (prior live wire; not re-fetched):

| Class | ~USD |
|-------|------|
| OAuth (grok-build product lines) | ~**201.76** |
| API product lines | ~**5.80** |
| Period totalWithCorr | ~**207.56** |

OAuth still dominates. Does **not** by itself prove SuperGrok included debit.

---

## Criteria snapshot after this recheck

| Id | Status this recheck | Notes |
|----|---------------------|--------|
| **C1** path SuperGrok while included headroom | **Still pass** (dump + log business session) | included remaining ~34% |
| **C3** no live ApiKey primary while included remaining | **Still pass on dumps** (`console.isLive: false`) | Not re-proved by re-exec |
| **C4** included / Build / post-full extras debit with traffic | **Not closed as full pass** | Multi-sample **65→66** + Chat **11→12**; **Build flat 54**; extras flat. **No single-poll C4 claim.** |
| **C5** extras before console after included ≥ 100% | **Not live** | included ≪ 100% |
| **flat_poll** surface | **Not re-proved live** | Code path remains product-wired per Slice 1 |

---

## Explicit non-claims

- Did **not** re-run `limits --json` or observe a new CLI exit code.
- Did **not** claim **C4** from one poll or invent Build debit (Build **still 54**).
- Did **not** claim **C5** live without included ≥ 100%.
- Did **not** claim tip-binary M3 / `grokBuildUsagePct` / flat_poll JSON fields without a fresh CLI dump.
- Did **not** print tokens or keys.

---

## Operator / parent next (if wanted)

1. Parent or agent **with shell:**
   `timeout 30 /home/hunter/Projects/surmount/grok-build/target/release/grok-oss limits --json`
   (or debug binary) — confirm **66%**, extras, `console.isLive`, and whether tip binary surfaces `teamPostpaid*`, `grokBuildUsagePct`, honesty notes.
2. Optional second poll ≥30s later in **same** process for flat_poll flag (only if series is flat again).
3. Residual: keep **2b** nuance for **Build** flatness; note top-level included **did** tick once in log.

---

## Commands this agent actually ran

| Action | Result |
|--------|--------|
| `list_dir` target/debug, target/release, .agents/joins, /tmp, /tmp/grok-1000 | Binaries + dumps present |
| `read_file` limits dumps, prior joins, mgmt-preview head | OK |
| `grep` unified.jsonl billing / productUsage / 65 vs 66 | 65 long flat; **66 from 13:38Z** |
| Shell limits | **Not run** |
| Product edits / git | **None** |

---

## Shell live recheck (2026-08-02T15:35:56Z)

**Binary:** `/home/hunter/Projects/surmount/grok-build/target/release/grok-oss`  
**Command:** `timeout 45 ./target/release/grok-oss limits --json 2>/tmp/grok-1000/limits-stderr.txt | tee /tmp/grok-1000/limits-live-tip.json`  
**Exit code:** **0**  
**stderr:** empty (0 bytes)  
**Artifact:** `/tmp/grok-1000/limits-live-tip.json` (913 bytes; secrets redacted below — no tokens/keys printed)

### Key fields (actual tip JSON)

| Field | Value this poll |
|-------|-----------------|
| `schemaVersion` | `1` |
| `liveSampling` | `supergrok_session` |
| `liveSamplingLabel` | Live sampling: SuperGrok session (business) |
| `livePrincipalRole` | `business` |
| `console.keyAvailable` | `true` |
| `console.isLive` | **`false`** |
| `console.teamPrepaidUsd` | **340.0** |
| SuperGrok principals | business: `dollarExtrasObserved: true`; personal: `dollarExtrasObserved: false` |
| `supergrok.sharedUnifiedPool` | `false` |
| **`includedUsedPct`** | **absent** (polls failed) |
| **`dollarExtrasUsd`** | **absent** (only boolean `dollarExtrasObserved` on principals) |
| **`grokBuildUsagePct`** | **absent** |
| **`flat_poll*` / `flat_poll_unproven_debit`** | **absent** |
| **team postpaid OAuth/API** (`teamPostpaid*Usd` etc.) | **absent** |
| Notes | (1) SuperGrok billing poll failed for `58c5f686-427…`: Invalid or expired credentials (`auth_kind=bearer`, `x_xai_token_auth=xai-grok-cli`, upstream=PermissionDenied, reason=no auth context). (2) SuperGrok billing poll failed for `61fab250-b2c…`: Timeout expired. |

### Criteria (this poll only)

| Id | This poll | Notes |
|----|-----------|--------|
| Path / sampling | SuperGrok session (business) | matches prior dumps |
| Console live | `isLive: false` | key available; prepaid 340 |
| **C4** | **Not closed** | No included/Build/extras meters on JSON; failed polls ≠ debit proof. Single poll would not close C4 anyway. |
| **C5** | **Not live** | `includedUsedPct` missing; cannot show ≥100% |
| flat_poll surface | **Not re-proved** | field absent on this dump |

### Explicit non-claims

- Did **not** claim **C4** closed from this poll (or at all).
- Did **not** claim **C5** live (included not ≥100%; field absent).
- Did **not** invent missing tip fields (`includedUsedPct`, extras USD, postpaid, flat_poll, `grokBuildUsagePct`).
- Did **not** print full tokens or secrets (principal ids truncated as in CLI notes).
- Product code edits / git: **none**.

### Redacted raw shape (no secrets)

```json
{
  "schemaVersion": "1",
  "liveSampling": "supergrok_session",
  "liveSamplingLabel": "Live sampling: SuperGrok session (business)",
  "livePrincipalRole": "business",
  "supergrok": {
    "principals": [
      { "label": "SuperGrok (business)", "role": "business", "dollarExtrasObserved": true },
      { "label": "SuperGrok (personal)", "role": "personal", "dollarExtrasObserved": false }
    ],
    "sharedUnifiedPool": false
  },
  "console": { "keyAvailable": true, "isLive": false, "teamPrepaidUsd": 340.0 },
  "notes": [
    "SuperGrok billing poll failed for 58c5f686-427…: Invalid or expired credentials …",
    "SuperGrok billing poll failed for 61fab250-b2c…: Timeout expired"
  ]
}
```
