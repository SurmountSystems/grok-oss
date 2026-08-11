# Plan inventory: free SuperGrok period always before SuperGrok dollar extras and console

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only inventory for a **comprehensive** plan (not a half park)
**Trigger:** Operator token-economy rule is free SuperGrok period first; doctor says `auto_use_included_limits` is on; live `limits --json` shows SuperGrok session live at ~6% used, SuperGrok dollar extras ~$100.29, console not live, team Grok Build class postpaid climbing (~$855). Operator feels "back to using credits."

Related same-day reports (do not re-invent):

- `.agents/reports/live-limits-vs-credits-check-2026-08-07.md` (verdict: free period driving; not the chrome-on-credits bug)
- `.agents/reports/bug-limits-chrome-when-on-credits-2026-08-07.md` (Design A status chrome fix for true credits paths)
- `.agents/reports/verify-compact-status-chrome-2026-08-07.md` (unit proof of Design A strings)
- `.agents/reports/explain-supergrok-billing-poll-failed-principal-2026-08-07.md` (dual poll / shared pool)
- Residual open: C4 server included debit; C6 honesty shipped; live extras-after-full not dogfood-proved

---

## 0. Meters (keep distinct forever)

| Meter | What it is | What it is not |
|-------|------------|----------------|
| **Free SuperGrok period allowance** | Included weekly (or period) pool from SuperGrok billing `creditUsagePercent` / `includedUsedPct` | SuperGrok prepaid top-ups; console keys; team invoice |
| **SuperGrok dollar extras** | Session billing `prepaidBalance` / Extra Usage Credits (personal SuperGrok top-ups) | Free period %; console team prepaid; team postpaid OAuth class |
| **Console API key live** | Inference on `api.x.ai` with console ApiKey (`liveSampling=console_key`, `console.isLive=true`) | SuperGrok session on cli-chat-proxy |
| **Console team prepaid** | Management prepaid wallet remaining (ledger) | SuperGrok extras; free period |
| **Team Grok Build class / postpaid OAuth $** | Management postpaid invoice class for OAuth / Grok Build (team settlement dollars that can move while SuperGrok session is live) | Proof that free period % moved; SuperGrok extras debit; console key live |
| **Team postpaid API class $** | Same invoice preview, API key class | OAuth class |
| **Grok Build productUsage %** | Wire `productUsage` on SuperGrok credits poll (product-level %) | Top-level free period %; team settlement $ |
| **Team default credits** | Dashboard allotment from postpaid preview | Prepaid wallet or SuperGrok extras |

"Using credits" in operator speech is ambiguous. In this dogfood snapshot it most often means **team Grok Build / postpaid OAuth settlement dollars climbing**, not SuperGrok dollar extras debit and not free-period chrome lying.

---

## 1. Intended burn order (product law)

Operator + residual + code agree:

1. **Free SuperGrok period first** while any live SuperGrok principal still has included headroom (`usage_pct < 100` → remaining &gt; 0).
2. **Then SuperGrok dollar extras** (after-burner): free period full, known positive SuperGrok `prepaid_balance_cents`, live (not hard-expired) SuperGrok JWT stays primary; console only as failover.
3. **Then console API key** as primary when free period full and extras 0/unknown; live SuperGrok JWT may remain recovery failover (console 403 can hop back).

Config gates:

| Flag | Role |
|------|------|
| `[auth] auto_use_included_limits = true` | Enables free-period-first ranking (default for new installs). **Not** a `preferred_method` value. |
| `preferred_method = api_key` | Pins console first by design; ranking skips free-period-first. |
| `preferred_method` oauth/oidc/unset | SuperGrok-session-first dual-auth when auto_use is on. |

### 1.1 Code map (order of burn)

| Layer | Path | Behavior |
|-------|------|----------|
| Config | `crates/codegen/xai-grok-shell/src/auth/config.rs` | `auto_use_included_limits` default **true**; alias `prefer_sooner_reset` |
| Pure rank | `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` | `included_remaining_from_usage_pct`; `order_live_supergrok_for_auto`; `order_credentials_for_preferred_auto` |
| Resolve wire | shell auth resolve + `ModelsManager::sampling_config` + subagent override + `resolve_model_to_sampling_config` | `resolve_credentials_preferring_with_rank` + auto_use (bare-resolve audit closed) |
| Exhaust memo | `xai-grok-sampler/src/exhausted_identity.rs` + shell `allowance_exhaust_from_billing.rs` | Mark SuperGrok out only at **≥ 100%** included (with dual-auth); **afterburner_skips_allowance_mark** when extras remain under auto_use |
| Prefer live | `xai-grok-sampler/src/prefer_live_primary.rs` | Pre-request hop away from memoized-exhausted primary; recovery reinject SuperGrok after console death |
| Request hop | `xai-grok-sampler/src/actor/request_task.rs` | Credit/429 rotation + prefer_live |
| Design A chrome | `xai-grok-pager/src/views/credit_bar.rs` + `agent_view/render.rs` | Compact meter names **active** spend path only |
| Path gate | `xai-grok-pager/src/limits_cmd.rs` `check_limits_first_path_json` | C1/C3: under auto_use, included &lt; 100% ⇒ not console primary |
| Honesty | `xai-grok-pager/src/views/limits_honesty.rs` | C6: SuperGrok live can still move team Usage $ without free-period proof |
| Doctor | `xai-grok-shell/src/auth/dual_auth_status.rs` | Prints "Prefer free SuperGrok period allowance: yes" when auto_use on |

### 1.2 Rank algorithm (included headroom)

From `order_credentials_for_preferred_auto` / `order_live_supergrok_for_auto`:

1. Among SuperGrok candidates with `included_remaining > 0` and **not** last-poll auth-failed: sooner `reset_at`, then stable identity id.
2. **While any such candidate exists:** primary = that SuperGrok JWT; **console keys omitted entirely** from primary and failover (silent 429 must not burn console Grok Build $).
3. When all included pools exhausted:
   - Positive SuperGrok extras on live JWT → SuperGrok primary, console failover (after-burner). Both flags can be true: `primary_is_supergrok_included` (session JWT) + `exhausted_all_supergrok_included`.
   - Extras 0/None or only hard-expired extras sessions → console primary; non-hard-expired SuperGrok as recovery tail.
4. Auth-failed poll demotes free-period "cache headroom" so a dead JWT is not primary while sibling still polls OK (dual SuperGrok honesty, 2026-08-07).

Live billing `usage_pct` wins over stale exhaust memo: `apply_included_billing_to_headroom` + `enrich_candidates_with_included_billing` clear memo tokens when free period has headroom again (period reset).

---

## 2. When product paints "credits" / SuperGrok extras chrome while free period has room

### 2.1 Design A compact status (intended)

| Live path | Compact meter |
|-----------|---------------|
| Console live | `console · $N` or honest gap. Never free-period `%` / `...%` |
| SuperGrok live, free period &lt; 100% | Free-period used `%` (e.g. `6%`). **Ignores** SuperGrok extras balance for compact text |
| SuperGrok live, free period ≥ 100%, extras &gt; 0 | `SuperGrok extras · $N` |
| SuperGrok live, free period ≥ 100%, no extras | `100%` |
| SuperGrok live, cold / active poll AuthFailed | `...%` |

Implementation: `compact_meter_text_for_live_identity_with_active_poll` in `credit_bar.rs`. Free period with room **never** switches compact text to extras solely because extras exist on the account.

### 2.2 Ways operators still "see credits" with free period room

| Mechanism | Explains dogfood? | Notes |
|-----------|-------------------|-------|
| **A. Multi-meter surfaces** (`/limits`, soft `/usage`, footer chips) | Yes | Lists SuperGrok extras $100.29 **and** team prepaid **and** team Grok Build class $ while compact is still `6%`. Correct multi-meter, easy to misread as "on credits." |
| **B. Team Grok Build class chip while SuperGrok live** (shipped 2026-08-07) | Yes | Footer can show `team Grok Build class: $N` climbing. That is settlement visibility, not free-period-first violation. |
| **C. Sticky exhaust memo + prefer_live on console** while free period has room | **Should not** under healthy poll | Live `usage_pct &lt; 100` should clear memo; rank omits console. Bug class if memo stuck + cold poll. |
| **D. Compact paints extras while free period room** | **No** if Design A tree is running | Bug was opposite (free-period chrome on true credits paths); fixed same day. |
| **E. Old installed binary** without Design A / dual honesty | Possible | Tree may differ from dogfood process until rebuild/reinstall. |
| **F. Shared-pool fill + sibling AuthFailed** | Soft | Can show 6% on both rows while one JWT failed; active AuthFailed should paint `...%` not healthy %. |
| **G. Server dual-bill (C4/C6)** | Yes for $ climb | Free period % flat while team Usage $ rises; product cannot invent free-period debit. |

### 2.3 Memo that "lies"

- Exhaust memo at ≥ 100% is intentional for preemptive console prefer **without** waiting for 402.
- After-burner gate: **do not mark** SuperGrok exhausted when auto_use on, dual-auth ready, and SuperGrok extras known positive (`afterburner_skips_allowance_mark`).
- Memo does **not** mark at 6%. Empty `exhausted_credits/` + `includedUsedPct: 6` + `liveSampling: supergrok_session` is consistent free-period path.
- Prefer_live hop to console with free-period headroom would be a **true TE path bug** (check_limits_first would fail on `limits --json`).

---

## 3. Why team Grok Build $ rises while free SuperGrok period stays ~6%

### 3.1 Known residual: C4 / branch 2b (server-side)

- Free SuperGrok period **debit not proven** under heavy SuperGrok session traffic (historical dogfood: included flat ~65–66%, Build product % flat, extras flat $100.29).
- Product honesty: flat-poll note, `included_debit_unproven`, C6 copy, limits-first path check. **Do not invent** free-period debit in the client.
- Ticket evidence package assembled (human / xAI). Client invent for limits-first is largely exhausted.

### 3.2 Known residual: C6 (shipped honesty)

`NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS`: SuperGrok session can still move **team Usage dollars (OAuth / Grok Build class)** without proving SuperGrok included weekly moved, even when the console API key is **not** live.

So climbing ~$855 team postpaid OAuth class while `includedUsedPct` stays 6% and `console.isLive=false` is **expected settlement dual-bill**, not proof that client rank chose SuperGrok extras or console.

### 3.3 What "using credits" means in this situation

| Interpretation | Matches live snapshot? | Action class |
|----------------|------------------------|--------------|
| SuperGrok **dollar extras** debiting | Unlikely (extras balance reported stable ~$100.29; free period 6%) | Would be true TE violation if extras drop while free period has headroom |
| **Console** live burning team prepaid / API class | No (`console.isLive=false`, liveSampling SuperGrok) | Would fail C1/C3 path check |
| **Team OAuth / Grok Build settlement $** while SuperGrok session samples | **Yes** (C6) | Chrome honesty + operator education + residual C4; product cannot stop server dual-bill alone |
| Free period actually burning but coarse/laggy % | Possible weak C4 | Multi-poll history; server ticket; no client invent |

---

## 4. Root-cause hypotheses (ranked by evidence)

| Rank | Hypothesis | Evidence for | Evidence against | Code / residual |
|------|------------|--------------|------------------|-----------------|
| **1 / C** | **Server dual-bill:** free-period meter lagging or not debiting while team OAuth settlement $ always moves on SuperGrok session traffic | Live: SuperGrok session, 6%, poll OK, console not live, team Grok Build class climbing; residual C4 FAIL + C6 shipped note; historical flat included under load | None for "product chose wrong primary" | `limits_honesty` C6; Management postpaid; residual 2b |
| **2 / A** | **Chrome / multi-meter reading:** looks like credits because extras $ and team $ are listed next to free-period % | Design A compact should be `6%`; `/limits` and footer still show extras + team $; operator "using credits" language | Compact path explicitly ignores extras when free period has room | `credit_bar` Design A; SuperGrok-live team chip |
| **3 / E** | **Stuck wrong path after dual-login / recovery / AuthFailed sibling** | Dual principals; one identity AuthFailed + shared-pool 6% fill; recovery hop code paths exist | Live sampling still SuperGrok business with pollSucceeded live_poll; empty exhaust dir | dual poll honesty; prefer_live recovery |
| **4 / B** | **Rank/memo wrongly treats free period exhausted at 6%** | Would explain console/extras primary | Contradicted by `included_remaining_from_usage_pct(6)` → remaining; liveSampling SuperGrok; check_limits_first OK pattern; exhaust only at ≥100% | `supergrok_identity_rank`; `exhausted_identity` |
| **5 / D** | **Actually burning SuperGrok prepaid extras while free period has headroom** | Would be true TE violation | Extras balance stable; compact and rank do not prefer extras until free period full; no wire proof of extras debit in inventory | After-burner only when included remaining 0 |

**Working diagnosis for the 2026-08-07 dogfood snapshot:** primarily **C (settlement dual-bill / C4 lag)** plus **A (operator-visible multi-meter "credits" language)**; not **B** or **D** on the live path reported (`liveSampling` SuperGrok, 6%, console false).

---

## 5. Tests that enforce free-period-first; gaps

### 5.1 Present (shipped contracts)

| Area | Examples / filters |
|------|-------------------|
| Rank omits console with headroom | `auto_order_omits_console_while_any_supergrok_included_headroom`, `auto_with_included_headroom_still_omits_console`, `auto_order_with_included_headroom_omits_console_from_hop_chain` |
| After-burner extras before console | `auto_order_keeps_supergrok_when_included_full_but_extras_remain`, hard-expired skips, dual extras |
| Exhausted both → console | `auto_both_included_exhausted`, resolve_auto_* filters in residual |
| Resolve + ModelsManager omit console | `sampling_config_auto_use_omits_console_while_supergrok_included_headroom`, bare-resolve filters |
| Memo / after-burner skip mark | `afterburner_skips_allowance_mark` / allowance_exhaust_from_billing tests; clear on headroom |
| Prefer live | sampler `prefer_live_*` tests |
| Limits-first path JSON (C1/C3) | `check_limits_first_*` in `limits_cmd.rs` |
| Design A compact chrome | `compact_status_supergrok_free_period_room_shows_pct_not_extras`, extras-at-100%, console-live |
| C6 honesty | `c6_team_usage_note_when_oauth_postpaid_dominates` |
| Flat poll / no invent debit | `flat_poll_*`, `FORBIDDEN_INCLUDED_BURN_CLAIMS` |
| Dual poll / AuthFailed demote | dual SuperGrok honesty filters (`auth_failed_poll`, `order_live_prefers_poll_ok`) |

Canonical residual command block: `RESIDUAL.md` § Validate honesty items 2–2k.

### 5.2 Gaps (plan-worthy)

| Gap | Why it matters |
|-----|----------------|
| **No hermetic assert that SuperGrok extras balance does not decrease while free period has headroom** | True TE violation (hypothesis D) needs wire or ledger observation, not only rank order |
| **No automated assert that team postpaid OAuth class must not climb without free-period move** | Server may dual-bill; product can only honesty-note (C6). Optional "settlement moved, free period flat" alert strength |
| **Live extras-after-full (C5) never dogfood-proved** at included ≥ 100% | Code path exists; no live window |
| **C4 free-period debit still unproven** | Server; multi-poll history soft |
| **Installed binary vs tree** | Unit green in tree ≠ operator process chrome |
| **Compact vs multi-surface "active driver" label** | `/usage` multi-meter can still lead with free period then extras without a single "active burn" line |
| **End-to-end spawn credential proof in CI** | Subagent spawn logs show SessionToken + proxy in dogfood; no always-on CI live account |

---

## 6. Comprehensive plan options (complete verticals; no soft park of in-scope work)

A plan that addresses **everything related** should cover all of the following verticals. Only operator-explicit deferral parks a slice.

### Vertical 1 — Prove which meter is actually burning (client observability)

1. **Live driver line** on `limits --json` / `/limits` human: one sentence naming active path (free SuperGrok period | SuperGrok extras after-burner | console key) from the same inputs as Design A + liveSampling.
2. **Delta panel / series:** free-period %, SuperGrok extras cents, team postpaid OAuth class, team postpaid API class, team prepaid, over a short process or durable history window (reuse included poll history + Management process cache patterns).
3. **Optional operator dogfood checklist** after rebuild: `grok-oss limits --json` fields (`liveSampling`, `includedUsedPct`, `dollarExtrasUsd` / prepaid, `teamPostpaid*`, `console.isLive`, flat_poll notes) + two spaced samples under load.
4. **Subagent / sampling spawn log** already attributes SessionToken vs ApiKey; surface a non-secret "last request identity" on `/limits` if not already obvious.

### Vertical 2 — Enforce rank never prefers extras/console when free period has headroom

1. Keep / strengthen existing pure rank + resolve tests (already strong).
2. **Fail-loud doctor / limits path check** when auto_use on and console primary with any principal &lt; 100% (already pure; wire to doctor exit or CLI status if not already).
3. Audit remaining non-ranked credential paths (Imagine / STT / BYOK / OpenRouter / explicit api_key pin) and **document** as credential-host exceptions, not free-period-first exceptions (residual Phase R separate).
4. Dual SuperGrok: auth-failed demotion + multi-slot refresh so recovery does not demote free-period headroom to console.
5. Prefer_live / exhaust memo: regression that memo cannot force console while live poll shows free-period headroom (enrich clear path already exists; pin integration if flaky).

### Vertical 3 — Chrome honesty when settlement $ moves without free-period move

1. **C6 always visible** when SuperGrok live and OAuth postpaid dominates / rising (already note; maybe promote to status-adjacent one-liner when OAuth class delta positive and free period flat).
2. **Never rename** free-period compact meter to "credits" while free period has room.
3. Footer team Grok Build class chip: keep distinct label ("team Grok Build class", not "credits" / not SuperGrok extras).
4. Soft `/usage`: lead with **active driver**, then other meters (optional UX polish so "using credits" is not the misread of a multi-meter dump).
5. Design A status sticky pin (console vs SuperGrok extras) already fixed for true credits paths; keep regression filters.

### Vertical 4 — Operator controls and doctor

1. Doctor continues to state free-period-first on/off and after-burner order (shipped dual_auth_status lines).
2. Optional: doctor warn when free period has headroom **and** team postpaid OAuth class rose since last sample (settlement awareness, not rank rewrite).
3. Optional: config already has auto_use kill-switch; do **not** invent a second parallel TE flag without need.
4. User-guide: one short "four meters" map (free period, SuperGrok extras, console team prepaid, team Grok Build class) with Design A table.

### Vertical 5 — Residual honesty for server dual-bill (unfixable client-side)

1. Keep C4 evidence package path; human/xAI ticket for free SuperGrok period debit under load.
2. Do **not** hop to console to "fix" flat free-period % or rising team Usage $.
3. Do **not** invent free-period debit or force extras primary while free period has room.
4. Residual stays open until server proves debit or vendor documents dual-bill as intended.

### Vertical 6 — Rebuild / dogfood closeout

1. Rebuild install dogfood binary from this tree (large uncommitted chrome/auth work may not be in older `grok` / `0.2.118` path).
2. Confirm: compact `N%`, liveSampling SuperGrok, check_limits_first OK, C6 note if OAuth class dominates, exhaust dir empty under 6%.
3. Live multi-poll only when session billing healthy (not AuthFailed cold process).

---

## 7. Critical files, red contracts, non-goals

### 7.1 Critical files

| File | Why |
|------|-----|
| `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` | Free-period-first rank + after-burner order |
| `crates/codegen/xai-grok-shell/src/auth/config.rs` | `auto_use_included_limits` default |
| `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` | Memo sync; after-burner skip mark |
| `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` | Doctor free-period-first copy |
| `crates/codegen/xai-grok-shell/src/agent/models.rs` | sampling_config rank wire |
| `crates/codegen/xai-grok-sampler/src/exhausted_identity.rs` | Exhaust memo + 100% floor |
| `crates/codegen/xai-grok-sampler/src/prefer_live_primary.rs` | Pre-request identity prefer |
| `crates/codegen/xai-grok-sampler/src/actor/request_task.rs` | Live hop / prefer_live call sites |
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Design A compact + multi-meter honesty helpers |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Status bar sticky pin + paint |
| `crates/codegen/xai-grok-pager/src/limits_cmd.rs` | `limits --json`, path check C1/C3 |
| `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` | C6 + flat-poll + license notes |
| `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` | `/limits` sections |
| `RESIDUAL.md` §4 OAuth dual-auth / limits-first | Open C4/C6/C5 context |
| `FORK.md` dual-auth + TE bullets | Shipped product law |

### 7.2 Red contracts to keep or add in plan implement

**Keep green (regression):**

```text
cargo test -p xai-grok-shell --lib -- auto_order_keeps_supergrok auto_after_included_and_extras auto_with_included_headroom auto_order_omits_console auto_both_included_exhausted resolve_auto_after_included_exhausted resolve_enforced_auto_use_included_limits resolve_auto_both_supergrok_exhausted
cargo test -p xai-grok-shell --lib -- allowance_exhaust_from_billing sampling_config_auto_use_omits_console
cargo test -p xai-grok-pager --lib -- check_limits_first compact_status_ c6_team_usage flat_poll
cargo test -p xai-grok-sampler --lib -- prefer_live rotate_ exhausted
```

**Add when implementing observability / honesty slices (named contracts):**

1. Free period room + positive SuperGrok extras + SuperGrok live → compact meter is free-period `%` only (already exists; keep).
2. Free period room + auto_use → credential chain has **zero** console keys (exists).
3. Free period room + auto_use → `check_limits_first` fails if console primary (exists).
4. **New:** `limits --json` / snapshot exposes an explicit `activeBurnMeter` (or plain label) matching Design A inputs.
5. **New:** when free period flat across ≥N samples and team postpaid OAuth class rises, honesty note fires (even without inventing free-period debit).
6. **New (optional):** hermetic ledger mock: extras cents must not be selected as primary while `usage_pct < 100`.

### 7.3 Non-goals

- Invent free SuperGrok period debit (C4) in the client.
- Hop to console to "fix" team Usage $ or flat free-period %.
- Scrape console.x.ai HTML or treat Grok Business **licenses** page as dogfood burn proof.
- Merge SuperGrok extras, team prepaid, and team Grok Build class into one "credits" meter.
- Claim C5 after-burner live-proved without a free-period ≥ 100% dogfood window.
- Full Business Usage chart UI (text totals / series already partial; charts optional later only if operator re-asks).
- Token Economy effort-cap pillars rework (parked TE options 2026-08-04 stay separate unless operator reopens).
- Git commit / push (human-only).

---

## 8. Suggested plan structure (for plan.md when operator enters plan mode)

Complete verticals, not optional-feeling leftovers:

1. **Diagnose live path** (observability vertical): active-burn field + delta samples; confirm C vs A vs D with evidence.
2. **Rank enforcement** (already mostly shipped): doctor/CLI fail-loud; dual-login recovery; memo/headroom integration tests.
3. **Chrome honesty** for settlement rise under free-period room (C6 promote + multi-meter "active driver" lead).
4. **Server residual** C4 ticket status + permanent product honesty (no invent debit).
5. **Dogfood rebuild** + live checklist + residual honesty update.
6. **Regression filters** catalog update in `doc/dev/upstream-regression-filters.md` / residual validate block for any new contracts.

Acceptance sketch:

- With `auto_use_included_limits=true` and free SuperGrok period used &lt; 100%: live sampling SuperGrok session only; console not in hop chain; compact free-period `%`; no SuperGrok extras primary.
- Team Grok Build class $ may still rise: product names settlement dual-bill (C6), never sells it as free-period burn or SuperGrok extras burn without evidence.
- True extras chrome only when free period full and extras positive.
- Operator can answer "which meter is burning?" from one glance at `/limits` or status without conflating meters.

---

## 9. Bottom line

| Question | Answer |
|----------|--------|
| Intended order | Free SuperGrok period → SuperGrok $ extras → console API key |
| Live dogfood (operator snapshot) | SuperGrok session, ~6% free period, extras on account, console not live, team Grok Build $ climbing |
| Free-period-first rank broken? | **Not indicated** by liveSampling + path invariants |
| "Using credits"? | Most consistent with **team OAuth/Grok Build settlement (C6)** and/or multi-meter chrome, **not** SuperGrok extras after-burner or console primary |
| True TE violation (extras/console preferred with free-period room)? | **Not supported** by current live path evidence |
| Server dual-bill / flat free period (C4)? | **Still open**; product honesty held |
| Plan scope | Observability + enforce/regression + chrome honesty + doctor + residual C4 + rebuild dogfood; no half park of those verticals |

This inventory is ready to become a full plan.md under plan mode without inventing "optional later" for the verticals above unless the operator explicitly defers one.
