# Plan: limits-first ideal (included weekly, then credits)

**Date:** 2026-08-02
**Status:** plan only (not approved for implement until product CTAs)
**Board:** `plan:limits-first-ideal`
**Joins used (do not re-invent):**

| Join | Role |
|------|------|
| `.agents/joins/live-auth-path-now-2026-08-02.md` | Live A-class path: SuperGrok business session, console not live |
| `.agents/joins/business-usage-vs-product-path-2026-08-02.md` | License Usage zeros ≠ SuperGrok path |
| `.agents/joins/console-api-usage-547-evidence-2026-08-02.md` | Console team **API** Usage $547.87 proven; not SuperGrok included debit |
| `.agents/joins/console-burn-one-turn-investigation-2026-08-02.md` | Live +$0.01 Usage tick; SuperGrok OAuth → team $ (not live ApiKey hop) |
| `/tmp/grok-join-explore-limits-credits-deep.md` | Flat 65% / $100.29 under heavy dogfood; A≠B |
| `/tmp/grok-join-audit-65pct-flat.md` | Meter timeline; forbidden overclaims |
| `/tmp/grok-join-impl-console-while-supergrok-headroom.md` | Design A strip console; expired multi-slot rank fix |
| `/tmp/grok-join-impl-limits-credits-observability.md` | identity_id + productUsage + prepaid log (Slices 1–2) |
| `RESIDUAL.md` §4 dual-auth halves | Shipped vs open |

---

## 1. Ideal contract (plain American English)

What “limits first, credits as after-burner” means for an operator who can check without a decoder ring.

### Acceptance criteria (checkable)

| # | Check | Pass looks like |
|---|--------|-----------------|
| **C1 Auth path while included has headroom** | `grok-oss limits --json` (or `/limits`) | `liveSampling` = SuperGrok session (personal or business role), `console.isLive` = false. Subagent / main inference logs show `SessionToken` + `cli-chat-proxy`, not `ApiKey` + `api.x.ai`. |
| **C2 Config intent** | `~/.grok/config.toml` | `preferred_method` is SuperGrok login (`oidc` / `oauth`), not `api_key`. `auto_use_included_limits = true` when operator wants limits-before-credits. |
| **C3 No silent console burn by this product while included remaining** | Logs + limits | With included weekly used **&lt; 100%**, this process does not put console keys in primary **or** failover chain (Design A). No dogfood `switching API host` to console unless operator pinned console or included is exhausted / sticky exhaust is real. |
| **C4 Included weekly absorbs session traffic (debit)** | Time series of SuperGrok meters under known load | Over a dogfood window with real SuperGrok session inference, **at least one** of these moves in the same direction as traffic: top-level included % (`creditUsagePercent`), Grok Build `productUsage` %, or SuperGrok $ extras (only after included is full). Flat forever under heavy load = **fail** this criterion (auth path alone does not pass). |
| **C5 After included is full: SuperGrok $ extras before console (ideal after-burner)** | Policy + live | When included weekly reports **≥ 100%** and SuperGrok $ extras remain, product stays on SuperGrok session long enough for extras to be the after-burner **unless** operator pinned console or hop policy explicitly chooses console next. Today’s ExhaustedAll → console-first is a **gap vs this ideal** (see §2). |
| **C6 Honesty** | `/limits`, footer, CLI human text | Never claims “using SuperGrok limits” / “burning included” from a single flat %. Base note: included % is billing poll, not proof of burn. When polls stay flat under load, surface the flat-poll note **from real evidence**, not only hermetic fixtures. |
| **C7 Meters stay distinct** | UI + docs | SuperGrok included weekly ≠ SuperGrok $ extras ≠ console team prepaid ≠ second SuperGrok principal ≠ Grok Business **licenses** Usage chart. |

### What is **not** part of this ideal

- Making console Platform **Grok Business licenses** Usage (messages / active users) move when CLI uses SuperGrok OIDC. That is a different product surface; zeros there do not fail C1–C6.
- Matching browser “Credits remaining” composite (~$1317) to Management prepaid ledger ($340). Different surfaces; prepaid dogfood already validated parse.

### One-sentence ideal

**While SuperGrok included weekly has room, this product talks to the SuperGrok session proxy and that traffic debits the included weekly pool; only after included is full do SuperGrok dollar extras (and then console, if configured) act as the after-burner; UI never pretends debit is proven when meters stay flat.**

---

## 2. Gap analysis (today)

### Proven fixed / working (strong evidence)

| Item | Evidence |
|------|----------|
| Live sampling is SuperGrok session (business / Team Surmount Heavy) | Live `limits --json`: `liveSampling=supergrok_session`, `livePrincipalRole=business`, `console.isLive=false`; unified log SessionToken + proxy |
| Design A: omit console from chain while any SuperGrok has included headroom | `order_credentials_for_preferred_auto` + tests `resolve_auto_omits_console_*`; live no dogfood ApiKey spawns |
| Preemptive hop only at included ≥ 100% | Code + tests; live 65% does not mark exhaust; exhaust store empty |
| Expired personal multi-slot no longer inflates headroom over live Team | Join impl-console-while-supergrok-headroom; tests green |
| Billing observability Slice 1–2 | `identity_id` / role / `grok_build_usage_percent` on success logs; `productUsage` deserialize; Management prepaid info-log; `limits --json` can carry Build % when wire has it |
| Base honesty note (poll ≠ proven burn) | `limits_honesty::NOTE_INCLUDED_PCT_IS_BILLING_POLL` wired into snapshot format when SuperGrok live + included reading |
| Console prepaid meter honesty | $340 matches wire `total.val`; not a cents/100 bug; docs say prepaid ledger ≠ dashboard composite |
| Grok Business licenses zeros expected | Product never drives license seat chart; SuperGrok path uses meters 1–2 |

### Suspected fail modes (ideal gap) with evidence strength

| Fail mode | Strength | Evidence | Ideal criteria hit |
|-----------|----------|----------|--------------------|
| **F1a SuperGrok included weekly debit unproven** (server % coarse, lag, or no debit of this principal’s pool) | **High that we must not claim SuperGrok debit; medium on root cause** | 5–7.5h heavy dogfood, 1000s of polls, included **65.0** and SuperGrok extras **$100.29** flat; productUsage Build **54%** seen on wire in later live dump but top-level still flat. Hypotheses: coarse Heavy %, attribution lag, billing principal ≠ burn principal, Build-only pool split. **Console API Usage page does not close this** (wrong meter). | **C4** (SuperGrok side) |
| **F1b Console team API Usage spend is proven** (operator pain meter for credits-first failure) | **High (browser console.x.ai team Usage)** | 2026-08-02 screenshot: team `61fab250…` **`/usage`** (API Usage, **not** `grok-business/usage`), Jul 27–Aug 2, **Total Spend $547.87**, ~1.14B tokens, ~57k requests; Text $332.63 + Grok Build $214.41 + small image/voice. Big bars Jul 30–Aug 1. Proves heavy **console-side** dollar burn in the dogfood window while SuperGrok still showed ~65% included + flat $100.29 extras. Supports “not at limits-first ideal” **if** that spend is this product and/or any console key on this team while included had headroom. Does **not** prove SuperGrok included moved. | Ideal outcome + operator pain; **C3** if this product caused it; else F5-class external |
| **F2 Flat-poll honesty detector not wired live** | **High (code)** | `flat_poll_unproven_debit` only set in **tests** (`with_flat_poll_unproven_debit`); no production collector sets the flag. Base poll note ships; optional “stayed flat” note does not auto-appear | **C6** |
| **F3 ExhaustedAll prefers console over SuperGrok $ extras** | **High (code vs ideal C5)** | `order_credentials_for_preferred_auto` after no included headroom → console primary; test explicitly: prefer console when included exhausted, **not** SuperGrok $ extras primary. Docs say “before $ extras / console” but hop policy skips staying on session for extras | **C5** |
| **F4 productUsage / Build % not always on CLI JSON surface** | **Medium (earlier live dump)** | Live join noted `grokBuildUsagePct` absent on one CLI dump while logs had Build 54%; code path exists (`apply_grok_build_usage_pcts`) — verify installed binary + collect path | **C4** observability |
| **F5 Other processes burn console while this product is SuperGrok-correct** | **Medium–high (Usage proven; attribution open)** | Console key + `XAI_API_KEY` present; Design A only covers this product’s resolve chain. Live SuperGrok path showed `console.isLive=false` while **API Usage still $547.87** team-wide. Attribution (this binary vs other clients on the same team key) still open. | Not C1 fail for **this** product if path stays SuperGrok; still operator pain + F1b |
| **F6 Wrong SuperGrok principal ranked** | **Low now** after expired-slot fix; residual if personal re-auths with bad billing | Multi-slot + base OIDC; ranking uses included headroom, not Business-first | **C1** edge |
| **F7 Failover hop too eager** | **Low in current window** | No live hop; 429/credit hop intentional; strip removes console mid-turn hop while headroom | **C3** if older binary without Design A |

### Not a fail mode (wrong meter / expected)

| Claim | Why not a limits-first fail |
|-------|----------------------------|
| Grok Business **licenses** Usage all zeros | Different meter (license seats/messages); not SuperGrok included or console API prepaid |
| Console `isLive: false` while SuperGrok at 65% | **Desired** for limits-first; not “stuck off Heavy” |
| Personal OIDC expired while business live | Explains personal poll fail; business SuperGrok path still live |
| Dashboard Credits remaining ~$1317 vs product prepaid $340 | Composite / defaultCredits vs prepaid ledger; parse already proven |
| Flat 65% “matches grok.com” | Shared poll snapshot, not proof this dogfood burned included |

### Especially: SuperGrok path + console not live, still not ideal?

Yes. **Auth path working is necessary, not sufficient.**

| Candidate | Verdict now |
|-----------|-------------|
| Included % not debiting / coarse / wrong pool | **Open primary SuperGrok gap (F1a)** — still unproven |
| Console API Usage $ burn while included had headroom | **Proven team-wide (F1b)**; attribution to this binary vs other clients open |
| Other processes burning console | Strengthened by F1b + live `console.isLive=false`; does not falsify SuperGrok path for this binary |
| Ranking wrong SuperGrok principal | Mostly fixed; keep dual-row + identity logs |
| Failover hop too eager | Not observed live with current config |
| Honesty UI claims limits-first while path wrong under some configs | Base honesty good; flat-poll auto note missing (F2); doctor text can over-promise extras-before-console vs code (F3 messaging) |
| Build path uses console keys (subagents, tools, image) | Main + subagent resolve share auto rank; BYOK / own-credentials models bypass; env key outside product still burns console |

---

## 2b. Operator evidence 2026-08-02 API Usage (console.x.ai)

**Source:** operator screenshot ~2026-08-02 01:03 local. Page is **console team API Usage**, not Grok Business licenses.

| Field | Value |
|-------|--------|
| URL | `console.x.ai/team/61fab250-b2c1-40cf-b5b8-628e673a2eeb/**usage**` |
| Not | `…/grok-business/usage` (license seats/messages) |
| Range | Jul 27 – Aug 2, 2026 |
| Total Spend | **$547.87** |
| Volume | ~**1.14B** tokens, ~**57k** requests |
| Breakdown | Text **$332.63**, Grok Build **$214.41**, Image & Video $0.83, Voice &lt;$0.01 |
| Shape | Large daily bars Jul 30, Jul 31, Aug 1; Aug 2 small so far |

**Operator read:** this page “proves it’s debiting,” against plan language that included weekly debit under load is **not proven**.

**Required conclusions (plain language):**

1. This page measures **console team API spend / credits** (meter 4 class), **not** SuperGrok weekly included `creditUsagePercent`.
2. Therefore it **does not** prove SuperGrok included debit. F1a stays open.
3. It **does** prove heavy **console-side** dollar burn in the dogfood window (**F1b**). That supports “not at limits-first ideal” when SuperGrok still showed ~**65%** included remaining and flat **$100.29** extras, **if** the spend is from this product and/or any console key on this team while included had headroom.
4. **Open reconciliation** with earlier Management captures (same team `61fab250…`): prepaid ledger **$340** remaining with **0 SPEND** rows in one dump vs **$547** Usage spend. Prior joins also saw period postpaid / `defaultCreditsIssued` (~$207 in one preview) and dashboard composite remaining (~$1317) ≠ prepaid $340. Plausible non-invented buckets: spend against **default/free credits** rather than prepaid ledger; SPEND not listed in prepaid *changes*; prepaid vs Usage chart windows differ. **Do not invent** which bucket ate the $547; keep reconciliation open.
5. Plan gap split: **(a)** SuperGrok included still unproven to move; **(b)** console Usage spend is **proven** and is the operator’s pain meter for credits-first failure.

Join: [`.agents/joins/console-api-usage-547-evidence-2026-08-02.md`](../joins/console-api-usage-547-evidence-2026-08-02.md).

### 2c. Operator live +$0.01 console Usage (same night)

**Source:** operator screenshots, same team API Usage page: **$547.87 → $547.88** (+**$0.01**) during product dogfood ~01:04 AM local (≈07:04 UTC MDT), while SuperGrok included still ~65% / extras $100.29.

**Investigation join:** [`.agents/joins/console-burn-one-turn-investigation-2026-08-02.md`](../joins/console-burn-one-turn-investigation-2026-08-02.md).

| Finding | Detail |
|---------|--------|
| Live sampling | Still SuperGrok session; `console.isLive=false`; all live subagent spawns `SessionToken` + `cli-chat-proxy` |
| No live ApiKey hop | No successful `api.x.ai` + ApiKey sampling in unified log for this window (only test kill-switch noise) |
| Most likely +$0.01 cause | **This product SuperGrok OAuth** settling as team **Grok Build OAuth** dollars on console Usage (not a secret console-key primary path) |
| Week $ mechanism | Management postpaid (same team): OAuth Grok Build ~**$201** vs API key class ~**$6** of one ~$207 period snapshot → OAuth path is the bulk team $ class |
| Pain meter | Console Usage movement **is** valid credits-burn signal even when Design A keeps the console **key** out of the chain |
| C1 vs C4 | C1 still pass; C4 still fail/unproven; C3 pass only if scoped to “console key not live,” **not** if scoped to “Usage $ must not move” |

**Do not** re-label this as “wrong meter so ignore.” F1b + one-turn tick strengthen operator pain; attribution is OAuth-on-Usage, not “included % moved.”

---

## 3. Solution plan (ordered slices)

**Rule:** prove debit / observability first. No large hop rewrites until C4 has a red/green story. No Grok Business licenses chart work unless a real fix needs it (it does not for this ideal).

### Slice 0 — Dogfood baseline (operator + agent, no product rewrite)

**Goal:** Freeze a honest “now” baseline after rebuild so later slices compare apples to apples.

**Steps:**

1. Rebuild/install so binary includes Design A + billing observability.
2. `grok-oss limits --json` → save snapshot (included %, Build %, extras $, liveSampling, console).
3. Tail `billing: fetched credits config` for `identity_id`, `role`, `grok_build_usage_percent`, nested `creditUsagePercent` / `prepaidBalance`.
4. Note personal vs business poll failures.

**Live verify:** same as live join 2026-08-02 shape; confirm Build % field present when wire has it.

**Effort / risk:** Low / none.

---

### Slice 1 — Prove or disprove included debit (observability + dogfood contract)

**Goal:** Make C4 measurable. Prefer product tests + process-local poll history over host archaeology.

**Likely files:**

- `crates/codegen/xai-grok-shell/src/extensions/billing.rs` (poll history helper, productUsage already present)
- `crates/codegen/xai-grok-shell/src/auth/` (process cache for last N samples per identity, if not only in pager)
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs` / `views/limits_snapshot.rs` / `views/limits_honesty.rs`
- Optional tiny pure module e.g. `included_poll_delta.rs` (name plain: poll history / debit evidence)

**What to build (smallest):**

1. **Process poll samples** per SuperGrok `identity_id`: timestamp, `creditUsagePercent`, SuperGrok extras cents, optional Grok Build %.
2. **Pure detector:** `included_debit_unproven(samples, min_polls, min_window)` when both included % and extras cents flat across window (and optionally Build % also flat).
3. **Log** on each success: already has identity + Build %; add optional `poll_delta` debug/info when % or Build moves.
4. **Wire** detector → `LimitsSnapshot.flat_poll_unproven_debit` on `/limits` and `limits --json` collect (closes F2).

**Red/green TDD (named contracts):**

| Test name (suggested) | Contract |
|------------------------|----------|
| `poll_history_marks_flat_when_included_and_extras_unchanged` | N identical samples → unproven debit true |
| `poll_history_clears_flat_when_included_pct_steps` | 65.0 then 66.0 → unproven false |
| `poll_history_clears_flat_when_build_product_usage_steps` | Top-level flat but Build 54→55 → unproven false (debit signal elsewhere) |
| `poll_history_clears_flat_when_extras_cents_drop` | Extras drop with flat % → unproven false (burn is $ extras) |
| `limits_snapshot_sets_flat_poll_from_history_not_only_tests` | Collect/build snapshot with history → honesty note appears without manual flag |
| Existing: `flat_poll_note_when_evidence_flag_set`, `billing_fetched_credits_log_ctx_includes_identity_and_build_product_usage` | Stay green |

**Live verify:**

1. Heavy dogfood session ≥ 30–60 min on SuperGrok path.
2. `rg identity_id|grok_build_usage_percent|creditUsagePercent ~/.grok/logs/unified.jsonl`
3. `grok-oss limits --json` / human: flat-poll note appears only when samples truly flat; disappears if Build % or top-level steps.
4. If Build % steps while top-level 65 stuck: treat top-level as coarse (C4 pass via productUsage); document that.

**Effort / risk:** Small–medium / low (pure history + wire existing honesty flag). Highest value.

---

### Slice 2 — Debit decision tree from Slice 1 evidence

**Goal:** After Slice 1 dogfood, pick one branch. Do not implement all branches.

| Outcome | Next action |
|---------|-------------|
| **2a** Top-level or Build % moves under load | C4 pass for “server debits something” on SuperGrok side. Keep honesty base note. Optional: surface Build % more prominently in footer/limits. Stop treating **F1a** as product bug. F1b (console Usage) remains a separate pain/attribution track. |
| **2b** All SuperGrok meters flat (included + Build + extras) under proven SessionToken load | Package evidence (identity_id, JWT principal family redacted, request volume, flat series). Product cannot invent SuperGrok debit. Options: escalate to xAI / treat as server attribution; optional product warning stronger than honesty note. **No** fake “using limits” copy. Console $547 Usage does not substitute for SuperGrok debit proof. |
| **2c** Extras drop while included flat and &lt; 100% | Server burning SuperGrok $ extras before included (wrong order server-side). Product still on session path; honesty + residual pin; do not hop to console to “fix.” |

**Files:** residual/FORK pin + maybe honesty copy strength; no hop rewrite in 2b/2c.

**TDD:** only if stronger honesty strings change (named forbidden phrases already in `limits_honesty`).

**Effort / risk:** Low product code / judgment risk on 2b.

---

### Slice 3 — After-burner policy: SuperGrok $ extras before console (C5)

**Goal:** Align ExhaustedAll with ideal: included full → stay SuperGrok session while SuperGrok $ extras remain → then console.

**Only start after Slice 1–2** so we do not reorder hop while debit is unknown.

**Likely files:**

- `supergrok_identity_rank.rs` (`order_credentials_for_preferred_auto`, ExhaustedAll path)
- `agent/config.rs` resolve + tests (~`resolve_enforced_auto_use_included_limits_prefers_console_when_supergrok_included_exhausted`)
- `dual_auth_status.rs` / user-guide `02-authentication` (plain language order)
- Hop / host switch call sites if they assume console-after-included always

**Design choices to lock in implement plan (park freeform questions if needed):**

1. When included ≥ 100% and extras &gt; 0: primary stays SuperGrok session; console failover only.
2. When included ≥ 100% and extras = 0 (or unobserved): console primary as today.
3. Sticky exhaust memo: must mean “included out,” not “leave SuperGrok forever if extras remain” (unless operator wants hard leave).
4. `preferred_method = api_key` still pins console first.

**Red/green TDD:**

| Test name (suggested) | Contract |
|------------------------|----------|
| `auto_after_included_exhausted_keeps_session_while_extras_positive` | included remaining 0, prepaid cents &gt; 0 → primary SessionToken, console in failover only |
| `auto_after_included_and_extras_gone_console_primary` | remaining 0, extras 0/None → console primary |
| `auto_with_included_headroom_still_omits_console` | regression Design A |
| Update/replace: `resolve_enforced_auto_use_included_limits_prefers_console_when_supergrok_included_exhausted` | Either rename contract to “when extras also gone” or split cases |

**Live verify:** Temporarily mark exhaust or wait for real 100% (careful with dogfood spend). Prefer hermetic first; live only with operator OK.

**Effort / risk:** Medium / medium (behavior change on money path; needs clear extras observation in ranking).

---

### Slice 4 — Remaining console burn hygiene (product edges only)

**Goal:** Inventory and close **this product’s** paths that can still hit `api.x.ai` while SuperGrok has included headroom.

**Scope:**

- BYOK / `has_own_credentials` models (document or gate)
- Aux summary clients (already pass `auto_use_included_limits`; re-verify tests)
- Image / embeddings / non-chat first-party if any use raw env key
- Subagent spawn credentials (already SessionToken in dogfood)

**Out of scope:** other hosts’ tools using the same team console key (operator audit).

**TDD:** one test per surprising path found (named: path must use resolve rank, not raw `XAI_API_KEY`, when auto limits on).

**Effort / risk:** Medium if many paths; low if audit is clean.

---

### Slice 5 — Surface polish (only if C4/C5 need it)

**Goal:** Operator-facing clarity without taxonomy lectures.

- Always show Grok Build % on `/limits` and JSON when wire has `productUsage`
- Footer: optional short “included debit unproven” when flat flag set (not only modal)
- Doctor dual-auth line: order matches Slice 3 policy (extras before console)

**TDD:** format/snapshot string asserts already pattern in `limits_cmd` / `limits_snapshot`.

**Effort / risk:** Small / low.

---

### Explicitly deferred (not this plan)

- Grok Business **licenses** Usage charts / seat APIs
- Console Management **series** charts (Half B optional residual)
- Folding dashboard $1317 into prepaid meter
- Business-first ranking (rejected earlier; headroom + sooner reset stays)

---

## 4. Sequencing summary

```
Slice 0  baseline dogfood
   ↓
Slice 1  poll history + wire flat_poll + prove/disprove debit   ← do first
   ↓
Slice 2  branch on evidence (2a pass / 2b server / 2c extras-early)
   ↓
Slice 3  after-burner: extras before console (only if still wanted)
   ↓
Slice 4  product console-edge audit
   ↓
Slice 5  UI polish
```

**Do not** start Slice 3 hop policy rewrites before Slice 1 green and Slice 2 branch chosen.

---

## 5. Regression filters (when implementing)

Prefer existing dual-auth / limits filters plus new names above:

```bash
cargo test -p xai-grok-shell --lib -- \
  extensions::billing:: \
  auth::supergrok_identity_rank:: \
  resolve_auto_omits_console \
  load_candidates_expired_personal

cargo test -p xai-grok-pager --lib -- \
  limits_honesty:: \
  limits_cmd:: \
  limits_snapshot::
```

Plus any new `poll_history_*` / `auto_after_included_*` filters.

---

## 6. Success definition (campaign done)

| Criterion | Done when |
|-----------|-----------|
| C1–C3 | Live + tests: SuperGrok session while headroom; Design A strip; no eager console |
| C4 | Dogfood series shows debit **or** product surfaces unproven-debit honestly with evidence package if server flat |
| C5 | Policy matches operator ideal **or** operator explicitly accepts console-after-included (document that acceptance) |
| C6 | Flat-poll note from live history, not fixture-only |
| C7 | No meter mash; no licenses-chart scope creep |

---

## Critical files for implementation

- `crates/codegen/xai-grok-shell/src/extensions/billing.rs` — poll, productUsage, identity log
- `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` — order, strip console, ExhaustedAll
- `crates/codegen/xai-grok-shell/src/agent/config.rs` — resolve wire-up + resolve tests
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` + `limits_snapshot.rs` — honesty + flat flag
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs` — CLI collect / JSON surface
