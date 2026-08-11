# Slice 2 dogfood G4 — SuperGrok included debit evidence

**Date:** 2026-08-02
**Agent:** L2 explore (read-only synthesis; join docs only)
**Workspace:** `/home/hunter/Projects/surmount/grok-build`
**Plan refs:** `.agents/plans/limits-first-ideal-2026-08-02.md` (C1–C7, G4);
`.agents/plans/limits-first-api-fix-section-2026-08-02.md` (Slice 2 dogfood steps)

**Also:** `/tmp/grok-1000/grok-slice2-dogfood-summary.md`

---

## One-line verdict

**C4 / G4 still fail for SuperGrok included debit.** Under heavy SuperGrok session traffic, top-level included %, Build `productUsage`, and SuperGrok $ extras stayed flat. Auth path is SuperGrok (`console.isLive=false`). Team dollars are mostly **Grok Build OAuth** (M3 cache), not proof that included weekly moved. **Do not claim limits-first burn.** Slice 4 after-burner is **code-shipped only**, not live-proved (no included ≥ 100% window).

---

## What was runnable autonomously vs operator-gated

| Check | Status this turn |
|-------|------------------|
| Locate binaries | **Yes.** `target/debug/grok-oss`, `target/release/grok-oss` present |
| Read prior dumps | **Yes.** `/tmp/limits-live.json`, `/tmp/limits-live-now.json`, `/tmp/limits-live-burn.json`, `/tmp/mgmt-preview.json`, `/tmp/mgmt-prepaid.json` |
| Read joins + plan | **Yes.** Slice 1/3/4 joins; live-auth-path; console $547 / +$0.01 joins; audit-65pct / deep explore |
| Read unified log meter series | **Yes.** `~/.grok/logs/unified.jsonl` — billing polls still 65.0 / 10029 / Build 54 |
| Code path for flat poll | **Yes.** Slice 1 wires process history → `flat_poll_unproven_debit` (not test-only) |
| Fresh `grok-oss limits --json` this agent | **No.** This explore session has **no shell tool**; could not re-exec the binary. Baseline from existing dumps + log (same shape as live-auth-path join) |
| Multi-hour controlled SuperGrok-only dogfood | **Operator-gated** (and already done earlier same day; evidence reused) |
| Browser console Usage re-shot | **Operator-gated** |
| Fresh M3 preview after Slice 3 binary | **Partial.** Cache `/tmp/mgmt-preview.json` is prior live wire; product M3 client is green in tests, not re-run live here |

---

## Baseline SuperGrok meters (no movement)

### CLI dumps (same shape across files)

Sources: `/tmp/limits-live.json`, `/tmp/limits-live-now.json`, `/tmp/limits-live-burn.json`
(prior live: `grok-oss limits --json` exit 0 per `live-auth-path-now-2026-08-02.md`)

| Field | Value |
|-------|--------|
| `liveSampling` | `supergrok_session` |
| `livePrincipalRole` | `business` |
| SuperGrok `includedUsedPct` | **65.0** (both business + personal rows; unified pool) |
| SuperGrok `dollarExtrasUsd` | **100.29** (`dollarExtrasObserved: true`) |
| Period | Weekly · next reset **August 3, 19:25** |
| `console.keyAvailable` | `true` |
| `console.isLive` | **`false`** |
| `console.teamPrepaidUsd` | **340.0** |
| `grokBuildUsagePct` on CLI JSON | **absent** on these dumps (wire/log has Build 54%; surface gap on that binary/dump era) |
| M3 postpaid fields on dumps | **absent** (dumps predate or predate-rebuild of Slice 3 surface) |
| Notes | Personal principal poll fail (expired OIDC); base honesty: included % is poll reading, not proof of burn |

### Unified log (B-class series under dogfood)

| Window | Signal | Finding |
|--------|--------|---------|
| ~2026-08-01T20:29Z → 2026-08-02T04:xx (audits) | `creditUsagePercent` | **Only 65.0** (~1298+ polls earlier; extended ~7.5h flat in deep explore) |
| Same | `prepaidBalance.val` | **Only 10029** ($100.29) |
| From ~06:29Z with productUsage observability | `productUsage` GrokBuild | **54.0%** flat across all sampled lines this agent grepped |
| Same | GrokChat | **11.0%** flat |
| Same | `identity_id` | `61fab250-b2c1-40cf-b5b8-628e673a2eeb` (Team Surmount business) |
| Same | tier | SuperGrok Heavy |

**No evidence of meter step** (included %, Build %, extras) under heavy SessionToken → `cli-chat-proxy` load. Prior joins: `/tmp/grok-join-audit-65pct-flat.md`, `/tmp/grok-join-explore-limits-credits-deep.md`, `.agents/joins/live-auth-path-now-2026-08-02.md`.

**C4 pass condition (plan):** any of included %, Build productUsage %, or extras (after included full) **moves with traffic.**
**Result:** none moved → **C4 fail**.

---

## OAuth vs API attribution (M3 / team dollars)

### Cached Management postpaid preview (`/tmp/mgmt-preview.json`)

| Class | Approx period total | Source |
|-------|---------------------|--------|
| **Grok Build OAuth** (product `grok-build` lines) | ~**$201.76** | line `amount` cents sum (prior live-auth join) |
| **API** (product `api` lines) | ~**$5.80** | same |
| Period | `totalWithCorr` / `defaultCreditsIssued` | **~207.56** USD; `defaultCredits` **1500** ($1500) |

OAuth **dominates** API class by ~35×. Aligns with “SuperGrok session / Grok Build OAuth settles as team Usage $ even when ApiKey is not live.”

### Console API Usage (operator pain; not SuperGrok included)

| Evidence | Value |
|----------|--------|
| Team Usage screenshot | **$547.87** week (Jul 27–Aug 2); Text + Grok Build bulk |
| One-turn dogfood | **$547.87 → $547.88** (+$0.01) while SuperGrok still 65% / $100.29 |
| Live path during that turn | SessionToken + proxy; **0** live ApiKey subagent spawns in log |

Joins: `.agents/joins/console-api-usage-547-evidence-2026-08-02.md`,
`.agents/joins/console-burn-one-turn-investigation-2026-08-02.md`.

**Interpretation for residual:** team $ moved (OAuth-class settlement / Usage chart) while SuperGrok **included weekly did not**. That is **not** C4 pass. It is F1b pain + C6 honesty fuel.

---

## Criteria status (evidence-based)

| Id | Criterion (short) | Status | Evidence |
|----|-------------------|--------|----------|
| **C1** | SuperGrok session path while included has headroom | **Pass** | `liveSampling=supergrok_session`, business role; logs SessionToken + cli-chat-proxy; config oidc + `auto_use_included_limits=true` |
| **C3** | No silent console **ApiKey chain** burn by this product while included remaining | **Pass for primary sampling** (with caveats) | Design A + live `console.isLive=false`; no dogfood ApiKey spawns. **Caveat:** team Usage $ and OAuth settlement can still move; image/voice edge paths exist; other clients on team key (F5) open. C3 is “this product’s resolve chain,” not “team Usage flat.” |
| **C4** | Included weekly (or Build productUsage / post-full extras) **debits** under session traffic | **Fail** | Flat 65.0 / Build 54 / $100.29 across multi-hour heavy dogfood |
| **C5** | After included ≥ 100%, SuperGrok $ extras before console | **Unknown / not live-proved** | Slice 4 **code + unit tests** shipped (`impl-slice4-extras-before-console-2026-08-02.md`). Included never reached 100% in dogfood → **no live after-burner proof**. Do not claim live-proved. |
| **C6** (related) | Honesty when debit unproven | **Code pass; live surface partial** | Slice 1: `attach_flat_poll_from_history` + detector from process ring. Base poll note present on dumps. Flat-poll “stayed flat” note needs **in-process** multi-sample history (≥2 polls, ≥30s); one-shot cold CLI may not light it; long-lived TUI / multi-poll process should. **Not re-proven live this turn** with post-Slice-1 binary. |
| **G4** | Dogfood branch decision for residual | **Closed as 2b** (see below) | |

---

## Slice code status (not live G4 pass)

| Slice | What shipped | Live G4 impact |
|-------|--------------|----------------|
| **1** Poll history + flat honesty | `included_poll_history.rs`; wire on billing + limits collect; tests green | Enables C4 measurement / C6 note; **does not create debit** |
| **3** M3 postpaid OAuth vs API | Management preview client; JSON `teamPostpaid*Usd`; C6 OAuth-dominates note | Attribution tool; hermetic tests green; **live re-fetch not re-run this turn** |
| **4** Extras before console | Rank/order after included full if extras > 0 | **Policy only until included ≥ 100%** |

### Can `flat_poll_unproven_debit` fire from process history (not test-only)?

**Yes (code).** Product path:

1. `record_included_poll_history_from_config` on successful S1 (billing extension + limits collect).
2. `flat_poll_unproven_debit_from_history()` → pure `included_debit_unproven` (defaults: min 2 polls, 30s window).
3. `attach_flat_poll_from_history` sets `LimitsSnapshot.flat_poll_unproven_debit`.
4. Honesty note when flag set (`limits_honesty.rs` “stayed flat” class).

**Not durable across process restart.** Cold single `limits --json` with one poll → flag false (honest: no invented multi-sample window). Long session with flat series → can fire.

---

## Branch recommendation (residual)

Plan branches after G4:

| Branch | Meaning | Recommend? |
|--------|---------|------------|
| **2a** Pass debit | SuperGrok meters moved with traffic | **No.** Evidence contradicts. |
| **2b** Server lag / coarse % / no debit of this principal’s pool | Flat under load; auth path correct | **Yes — primary residual branch for C4/F1a.** Keep honesty; do not hop to console to “fix” included. Optional: longer wait after load, grok.com Build chart cross-check, server ticket with identity_id + timestamps. |
| **2c** Extras-early | After-burner when included full | **Code already shipped as Slice 4.** Do **not** treat as G4 live pass. Revisit live only if included hits ≥ 100% with extras still positive. |

**Secondary product residual (not 2a):** team OAuth Usage $ while SuperGrok primary (F1b / C6 honesty) — Slice 3 surfaces this; operator may still want Usage attribution hygiene, not more console keys in chain.

---

## Highest-value next steps

### Agent-doable

1. Rebuild/install tip binary with Slices 1+3+4; one-shot `grok-oss limits --json` → confirm `teamPostpaidOauthClassUsd` / `teamPostpaidApiClassUsd` (or gap) and `grokBuildUsagePct` when wire has productUsage.
2. In a **long-lived** process (or scripted double-poll ≥30s apart without restart), verify flat-poll honesty note appears while meters still 65/54/10029.
3. Optional: log scrape script summarizing distinct `(creditUsagePercent, GrokBuild%, prepaidBalance)` over a window (no secrets).
4. Keep Design A / limits-before-credits; residual pin 2b wording if not already.

### Operator-only

1. Controlled SuperGrok-only window (minimize other team key clients); capture S1 before/after + browser Usage + optional M3 re-fetch.
2. Wait / re-poll after heavy load for **lag** hypothesis (included or Build % step delayed).
3. Confirm on grok.com / SuperGrok usage UI whether Build pool matches 54% flat.
4. Live C5 only if included weekly actually hits 100% with extras remaining (expensive; optional).
5. Personal OIDC re-login if dual-principal surface matters (personal currently expires).

---

## Commands / checks this agent actually ran

| Action | Result |
|--------|--------|
| `list_dir` `.agents/joins`, `target/debug`, `target/release`, `/tmp`, `/tmp/grok-1000` | Binaries and joins present |
| `read_file` Slice 1/3/4 joins, live-auth-path, console $547 / burn joins, plan sections | OK |
| `read_file` `/tmp/limits-live*.json`, `/tmp/mgmt-preview.json` | OK (redacted meters only) |
| `grep` unified.jsonl for billing / productUsage / identity | Many hits; all 65.0 / 10029 / Build 54.0 in samples |
| `grep` code `attach_flat_poll`, `included_poll_history`, M3 JSON fields | Slice 1+3 paths confirmed |
| Shell `grok-oss limits --json` | **Not run** (no shell in this agent) — exit code **n/a** |
| `cargo test` | **Not run** this turn (prior joins already green for S1/S3/S4) |

---

## Explicit non-claims

- Do **not** claim F1a / C4 fixed without meter movement.
- Do **not** claim Slice 4 live after-burner without included ≥ 100% window.
- Do **not** treat console Usage $ or OAuth postpaid as SuperGrok included debit.
- Do **not** use console ApiKey hop to “make Usage honest” while included has headroom (limits before credits).

**No product source edits. No git add/commit.**
