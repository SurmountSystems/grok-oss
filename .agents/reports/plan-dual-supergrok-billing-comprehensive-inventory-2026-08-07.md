# Comprehensive inventory: dual SuperGrok principal billing honesty

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only inventory for plan (no product edits)
**Prior explain:** `.agents/reports/explain-supergrok-billing-poll-failed-principal-2026-08-07.md`

## Problem (operator cluster)

1. SuperGrok billing poll can fail for **one** principal (expired JWT / `no auth context`) while the other succeeds.
2. Under unified billing, `fill_unified_included_on_empty_slots` copies included % onto empty slots so personal and business both show the same 6% even when one JWT failed.
3. `sharedUnifiedPool: true` plus a soft note in `limits --json` does not make it obvious **which login to re-auth**, or that the second row was **fill-from-sibling**, not a successful poll.
4. Related work already shipped or partial (console-dead recovery, limits chrome on credits, `preferred_method` only `oidc`/`api_key`, SuperGrok-live team prepaid chrome). Operator wants a **full** plan, not only "re-login later."

---

## 1. Dual SuperGrok path map

### 1.1 Store and identity

| Concern | Path / API | Behavior |
|---------|------------|----------|
| Multi-slot store | `auth.json` scopes with `::personal` / `::team::` + base active | Second SuperGrok login does not wipe the first |
| Identity id | `supergrok_identity_id_from_auth` (`model.rs`) | Prefer `team_id`, else `user_id`, else store scope |
| Listings (doctor / limits order) | `list_supergrok_principal_listings` | Dedup by identity; prefer fresher JWT on same id |
| Active principal | `active_supergrok_identity_id` | Base scope first (no multi suffix), else first SuperGrok token |
| Dual-auth status | `collect_dual_auth_status` / `format_human` | Counts + role + fingerprint only; **no poll health** |

### 1.2 Poll targets and HTTP

| Concern | Path / API | Behavior |
|---------|------------|----------|
| All poll targets | `load_supergrok_billing_poll_targets` | One target per identity; multi-slot preferred over base for same id |
| Sibling-only | `load_non_active_supergrok_billing_poll_targets` | Everyone except active identity |
| Credits HTTP | `fetch_credits_config_with_session` (`extensions/billing.rs`) | Bearer = **that** principal’s JWT; `x-userid`; `GET {proxy}/billing?format=credits` |
| Fail body | same | `Billing service error: {upstream error or HTTP status}` |

Polls are **independent**. One dead JWT fails while the other can return included % / prepaid / Build product usage.

### 1.3 Process cache (remember)

Process-local only (not durable across restarts): `INCLUDED_BILLING_BY_IDENTITY` in `allowance_exhaust_from_billing.rs`.

| Writer | What it stores |
|--------|----------------|
| `remember_supergrok_included_billing` | usage %, reset_at, period_type |
| `remember_supergrok_dollar_extras` | prepaid_balance_cents (Extra Usage Credits) |
| `remember_supergrok_build_usage` | Grok Build productUsage % |
| Active path | `remember_active_supergrok_*` + shell `handle_get_billing` + pager FetchBilling |
| Sibling path | `poll_and_remember_non_active_supergrok_included_billing` (debug on fail; active path unchanged) |

Failed poll **does not** write that identity’s cache entry. Stale prior entries for that id can remain until process exit or overwrite.

### 1.4 CLI `grok limits` / `limits --json`

`collect_limits_report_at` in `limits_cmd.rs`:

1. Load **all** poll targets.
2. Per target: fetch credits with that JWT.
3. **Ok** → balance map + remember included/extras/build + poll history.
4. **Err** → `notes.push("SuperGrok billing poll failed for {short_id}: {e}")` where `short_id` is first **12** chars of identity id. **No** balance row for that id.
5. Build `PrincipalLimitsInput` per listing: balance if present, else empty; sibling empty uses `included_billing_only: true`.
6. `LimitsSnapshot::from_principals` → may set `shared_unified_supergrok_pool` and **fill** empty included/extras from a sibling template.
7. CLI exhaust: **does not** write hop memos (“read-only report path”).
8. `report_from_snapshot` adds honesty notes; `sharedUnifiedPool` is a bool on the SuperGrok section.

**Honesty gap:** JSON principals have no `pollOk` / `filledFromSibling` field. Both rows can show the same `includedUsedPct` after fill while only `notes` mentions a failed short id.

### 1.5 TUI `/limits` and status bar

| Surface | Path | Dual behavior |
|---------|------|----------------|
| Modal open | `dispatch_show_limits` | Active row = pager `credit_balance`; siblings = process cache only (never copy active balance onto sibling id). Silent FetchBilling if any dual row still empty after build. |
| Snapshot fill | `LimitsSnapshot::from_principals` | If shared pool: fill empty included + dollar extras |
| Human note | `format_limits_detail` | One short line: personal + business share one weekly pool and Extra Usage Credits (not console team prepaid). **Does not** say a row was fill-only or which poll failed. |
| Compact status | `credit_bar` / agent render | Driven by **live** sampling identity + **active** credit cache, not dual fill. Recent Design A: free-period vs SuperGrok $ extras vs console chrome. |
| Sibling background | `poll_and_remember_non_active_…` | Fail = debug log only; **no** operator note in TUI |

### 1.6 Ranking and hop (uses cold/stale principals)

| Step | Path | Risk if dead JWT |
|------|------|------------------|
| Load candidates | `load_supergrok_session_candidates` | Hard wall-clock expired → remaining 0; prefer live/fresher store entry for same identity |
| Enrich from poll cache | `enrich_candidates_with_included_billing` | Missing identity keeps memo 0\|1; present usage % can set headroom. Hard-expired re-zeroed after enrich |
| Rank | `order_credentials_for_preferred_auto` / `pick_supergrok_identity_for_auto` | Prefer included headroom; sooner reset; after-burner extras; ExhaustedAll → console primary + live SuperGrok recovery tail |
| Prefer live | `prefer_live_identity_after_credit_exhaust` | Prunes memoized-exhausted SuperGrok from primary |
| Console dead recovery | `ensure_supergrok_recovery_after_console_credit_exhaust` (shipped 2026-08-07) | Hops to non-hard-expired SuperGrok recovery JWT |

**Important:** billing fill for **display** does not put a failed principal into the balance map, but ranking still loads **every** stored SuperGrok JWT. A dead-but-not-hard-expired JWT with leftover process cache % can look like headroom until wire rejects it. Unified-pool success on the sibling does **not** mean the dead principal’s JWT works for inference.

### 1.7 Token refresh

| Piece | Coverage |
|-------|----------|
| `OidcRefresher` / AuthManager | Primarily **active** / AuthManager primary session; multi-slot kept in lockstep on enrichment write (Heavy routing ship) |
| Poll path | Uses **stored** access token as-is; no refresh-before-billing-poll for siblings |
| Hard expiry | Ranking zeros hard-expired; billing poll may still attempt dead multi-slot if not wall-clock expired but upstream revoked (`no auth context`) |

There is **no** product path today that auto-prunes dead principals from poll targets after repeated auth failures, or auto-refreshes **non-active** JWTs before sibling poll.

### 1.8 Doctor / dual-auth surfaces

| Surface | What operator sees | Poll health? |
|---------|-------------------|--------------|
| `grok doctor` (human) | `DualAuthStatus::format_human`: session count, role, fingerprint, preferred_method, auto_use, failover ready | **No** |
| Doctor JSON | Diagnostic report only (dual-auth block is human-only side effect) | **No** |
| `grok login --list-api-keys` style dual status | Same fingerprints | **No** |
| `limits --json` notes | Failed short id + soft honesty phrases | Partial (short id, not role; no fill provenance) |

---

## 2. Honesty gaps (checklist)

| Gap | Today | Why it hurts |
|-----|--------|--------------|
| **fill_unified_included_on_empty_slots** | Copies known included onto empty dual slots when shared pool | Empty row looks like a successful poll of that principal’s JWT |
| **fill_unified_dollar_extras_on_empty_slots** | Same for Extra Usage Credits | Both rows show $ extras; operator may think both logins observed prepaid |
| **sharedUnifiedPool flag** | True from wire `is_unified_billing_user` **or** matching floored % + reset on two known readings | Matching after fill can reinforce the pool story without both polls OK |
| **CLI note only soft** | `SuperGrok billing poll failed for {12-char id}: …` | No role label (personal/business); no “re-login this fingerprint”; not tied to which JSON principal |
| **No fill provenance on principal** | No `includedSource` / `pollSucceeded` | Machine clients and humans reading meters alone miss the fail |
| **TUI sibling fail silent** | Debug log only | Operator never sees sibling auth death unless they run CLI limits |
| **Doctor dual-auth** | Presence + fingerprints | No “poll last OK / last fail reason” |
| **Status bar** | Active principal cache | Usually OK if live JWT healthy; **does not** warn that a second stored principal is dead |
| **Rank using cold/stale** | Candidates from all stored tokens + optional process cache | Dead non-hard-expired JWT may still be ordered; inference can fail after display looked fine |
| **Active dead + sibling fill** | Display can show healthy shared 6% from sibling | Worst case: live sampling claims SuperGrok while **active** JWT is the dead one (chat `no auth context` while limits look warm) |
| **Prior process cache** | Failed poll leaves old remember | Can enrich rank/display with **stale** % until overwrite |

Shared-pool fill is **intentional** product behavior (unified consumer pool dogfood: same % and same Extra Usage Credits). The gap is **labeling**, not the fill math when the pool is truly unified and **at least one** JWT is healthy.

---

## 3. Risks if active principal is the dead JWT while display looks healthy

| Severity | Scenario | Effect |
|----------|----------|--------|
| **High** | Active identity’s token is dead; sibling poll succeeds; unified fill paints both rows | `limits` looks fine (6%/6%, shared pool); chat/inference on SuperGrok can fail with expired credentials; operator may not re-auth the **live** role |
| **High** | Rank picks dead JWT as primary (remaining default 1, no hard expiry, no memo) | First request burns fail path; hop may recover to console or sibling depending on failover chain |
| **Medium** | Active healthy; sibling dead | Soft note only (CLI); ranking may still list dead sibling; after-burner/recovery could try dead token later |
| **Medium** | Stale process cache for dead id | Rank headroom / dual row from **old** poll, not this run |
| **Low (display)** | CLI notes present but ignored | Soft honesty only; product already documents that CLI does not write exhaust memos |
| **Low** | Console live; SuperGrok rows filled from cache | Status chrome (post Design A) should not paint free-period as live spend; dual SuperGrok honesty still matters for `/limits` |

**Mitigation already in tree (partial):** hard wall-clock expiry → remaining 0; prefer fresher store entry; Heavy multi-slot fresher routing; console-dead SuperGrok recovery; hard-expired never recovery/after-burner. **Missing:** fail-loud role + re-login CTA; fill provenance; poll-health on doctor; prune/refresh of dead non-active principals; “prefer active poll for status, never paint sibling-only as active success.”

---

## 4. Related shipped / partial work (do not re-scope as greenfield)

| Item | Report / area | Relevance |
|------|---------------|-----------|
| Console-dead → SuperGrok free period recovery | `impl-console-dead-supergrok-recovery-2026-08-07.md` | Hop/rank recovery when console dies; not dual poll honesty |
| Limits chrome when on credits | `bug-limits-chrome-when-on-credits-2026-08-07.md` | Compact meter Design A (free period vs extras vs console) |
| preferred_method only oidc/api_key | `impl-remove-preferred-method-serde-aliases-2026-08-07.md` | Config wire cleanup; doctor shows oidc |
| SuperGrok-live team prepaid | `impl-supergrok-live-team-usage-2026-08-04.md` | Team $ when SuperGrok live |
| Multi SuperGrok + sibling poll + dual `/limits` | Residual Half A; regression filters | Core dual path shipped; fill + soft notes are residual honesty |
| Soft honesty notes (included = poll reading, flat poll, C6 team Usage) | `limits_honesty.rs` | Does not cover dual poll fail / fill-from-sibling |
| Explain of dogfood fail note | `explain-supergrok-billing-poll-failed-principal-2026-08-07.md` | Root cause of short id + fill |

---

## 5. Product options (2–3) and recommendation

### Option A — Fail-loud surface only (minimal)

- Map failed poll identity → role label + fingerprint (from listings).
- CLI note: `SuperGrok billing poll failed for SuperGrok (personal) (fingerprint abcd…): … Run: grok login` (or role-specific login path if product has one).
- TUI: surface sibling poll failure once (modal note / toast / limits body), not debug-only.
- Doctor human dual-auth: last poll OK/fail per principal (process cache of poll outcome, no secrets).
- **Do not** change fill math.

**Pros:** Small; fixes “which login?”
**Cons:** Both meters still look identical after fill; easy to ignore note.

### Option B — Fail-loud + labeled fill (recommended full-complete)

Everything in A, plus:

1. **Provenance on meters**
   - Per principal: `pollSucceeded: bool`, optional `includedSource: "live_poll" | "process_cache" | "shared_pool_fill"`.
   - Human `/limits`: if a slot was filled, short tag e.g. “included (shared pool; this login not polled OK this run)” or keep one shared-pool note that **names which principal failed**.
2. **Do not claim active success from sibling-only**
   - Status bar / live sampling chrome: only active principal’s poll or active credit cache.
   - If **active** poll failed this collect: do **not** paint included % as if active succeeded; show honest cold/error + re-login for **active** role even if sibling fill could paint dual rows for the shared pool.
3. **Shared pool still fills for dual rows** when unified and **at least one** principal OK, but JSON + human always distinguish observed vs filled.
4. **Doctor /limits dual poll health** table: role · fingerprint · last poll · error class (auth vs network) · re-login CTA.
5. **Ranking hygiene (bounded)**
   - On auth-class billing fail for a principal, do not treat process-cache % as fresh headroom for that identity until a successful poll or re-login (or mark identity “auth_unverified”).
   - Prefer not to primary a principal whose last poll was auth-failed when another SuperGrok principal polled OK (same unified pool).
6. **Optional prune / refresh**
   - Before poll: attempt refresh for multi-slot if near expiry (if product already has refresher for that scope).
   - After N consecutive auth fails: demote from poll targets or show “stale login; remove or re-login” without inventing auto-delete of secrets.

**Pros:** Matches operator ask (“not only re-login later”); keeps useful unified-pool UX; closes active-dead display lie.
**Cons:** More JSON schema / snapshot fields; careful TDD on fill + rank contracts.

### Option C — Strict no-fill (hard honesty)

- Never copy included/extras onto empty slots.
- Failed principal stays “no data yet” / poll failed.
- Shared-pool note only when **both** polled and match (or wire unified flag on a successful poll).

**Pros:** Simplest mental model.
**Cons:** Dogfood already validated unified pool as one consumer pool; dual rows look “broken half” under true shared billing; regresses intentional fill tests (`format_dual_unified_*`).

### Recommendation

**Option B** as the full-complete approach. Keep unified fill for true shared pool, but:

- fail-loud which principal failed + plain re-login CTA;
- label fill-from-sibling / shared pool;
- never paint sibling-only as active success on status/live chrome;
- doctor + limits surface dual poll health;
- bounded rank demotion for auth-failed principals.

Do **not** choose C unless operator rejects fill after seeing B mockups.

---

## 6. Critical files

| Area | Absolute path |
|------|----------------|
| CLI collect + notes + short_id | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/limits_cmd.rs` |
| Dual snapshot + fill + format | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` |
| Honesty phrases | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` |
| TUI limits open + build snapshot | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/dispatch/status.rs` |
| Compact status / Design A chrome | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/credit_bar.rs` |
| Status bar pin | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| Doctor dual-auth dump | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/doctor_cmd/mod.rs` |
| Poll targets + remember + candidates | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` |
| Rank + enrich + after-burner | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` |
| Dual-auth status | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` |
| Listings / identity / expiry | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/model.rs` |
| Credits fetch + sibling poll | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/extensions/billing.rs` |
| Prefer live / recovery hop | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/prefer_live_primary.rs` |
| Residual / filters | `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md`, `/home/hunter/Projects/surmount/grok-build/doc/dev/upstream-regression-filters.md` |

---

## 7. Red contracts to add or extend (TDD)

Named contracts (plain language). Prefer observed red before product edit.

### Display / CLI / JSON

1. **Failed poll names role:** when personal poll fails and business succeeds, note includes `personal` (or principal label) and is not only a 12-char id.
2. **Re-login CTA present:** auth-class fail note includes actionable `grok login` (or documented multi-login path).
3. **Fill provenance:** dual unified fill sets filled principal’s included from sibling **and** marks source shared-pool-fill (JSON and/or human line); successful poll principal is not marked fill.
4. **Active poll fail ≠ active success chrome:** active identity fails poll; sibling succeeds; compact status / live line must not imply active SuperGrok included meter succeeded (honest cold or error for active).
5. **sharedUnifiedPool + one fail:** both may show same % when filled, but notes or fields make fail + fill visible together (no silent identical rows only).

### Sibling / doctor

6. **Sibling auth fail surfaces once** on TUI `/limits` rebuild (not debug-only) when process cache records auth fail for non-active principal.
7. **Doctor dual poll health:** two principals, one fail fixture → human dual-auth (or doctor extension) lists which failed.

### Rank (bounded; careful)

8. **Auth-failed principal not preferred over healthy sibling** under auto_use when last poll for A was auth error and B has included headroom (same unified pool OK to use B’s token).
9. **Hard-expired still zero** (existing); do not weaken.
10. **Recovery never hard-expired** (existing console-dead contracts stay green).

### Regression keep-green (do not regress)

From residual / filters:

```text
cargo test -p xai-grok-shell --lib -- upsert_personal_then_business team_login_then_personal_keeps dual_supergrok load_supergrok_candidates two_principals_billing enrich_candidates principal_limits_label non_active_poll_targets remember_both_principals included_usage poll_non_active_remembers
cargo test -p xai-grok-pager --lib -- format_dual_principals live_console_omits extra_principals_hook show_limits format_supergrok_session footer_names_live_principal format_dual_unified fill_unified limits_honesty
```

Plus console-dead recovery and limits-chrome-on-credits filters already shipped.

---

## 8. Ordered implementation steps (for plan body)

1. **Inventory lock** (this report) + operator accept Option B vs A/C.
2. **Poll outcome model** (process-local): per identity_id last result enum (`Ok`, `AuthFailed`, `OtherFailed`, `Never`), role, short error class; written by CLI collect + sibling poll + active FetchBilling. No tokens.
3. **CLI note + JSON fields** (smallest user-visible win): role + re-login CTA; principal `pollSucceeded` / `includedSource`; tests red→green.
4. **Snapshot fill labeling** (`limits_snapshot` + human format + honesty module): keep fill; add human one-liner when any slot filled from sibling or any principal auth-failed.
5. **Active-poll honesty for status** (credit_bar / render): if active last poll AuthFailed, do not paint free-period % from sibling fill as live success.
6. **TUI sibling fail surface** on `/limits` open rebuild from poll outcome map.
7. **Doctor human dual-auth** append poll health lines from same map.
8. **Rank demotion** (optional same PR or follow-up): auth-failed identity not primary when another SuperGrok candidate is live; keep hard-expired / recovery contracts.
9. **Optional refresh-before-sibling-poll** only if existing OIDC refresher can target multi-slot without large AuthManager rewrite; else park as residual.
10. **User-guide** (`02-authentication` / limits): dual login poll fail + shared pool fill + re-login which role. Citation only if external vendor policy claimed.
11. **Regression filter** line in `doc/dev/upstream-regression-filters.md` for new dual-poll honesty names.
12. **Process mop:** fmt + clippy + targeted tests on touched packages.

---

## 9. Non-goals

| Non-goal | Why |
|----------|-----|
| **Invent C4 SuperGrok included debit** | Server-side; residual still FAIL; honesty is poll reading not burn proof |
| **License / Platforms → Grok Business charts** | Explicit non-goal; license page ≠ product meter |
| **Token Economy park** | Separate residual/plan; do not fold into dual poll honesty |
| **Scrape console.x.ai HTML** | Forbidden |
| **Merge SuperGrok pools with console team prepaid** | Meters stay distinct |
| **Auto-delete principals from auth.json without operator** | Security/trust; demote/warn only unless operator asks prune UX |
| **Re-open preferred_method aliases** | Shipped reject oauth aliases |
| **Re-litigate console-dead recovery or Design A chrome** | Shipped; only wire dual poll outcomes into them |
| **Unsigned commit / agent commit** | Process law |

---

## 10. Acceptance sketch (Option B done)

- Operator with dual personal/business can see **which** principal’s poll failed in plain English (role + fingerprint or full short identity) and a **re-login** action.
- Dual rows under unified pool may still share % / Extra Usage Credits, but **fill is labeled** (not silent clone).
- Active principal auth failure cannot leave compact status looking like a healthy free-period read from the sibling alone.
- Doctor (human) and `/limits` / `limits --json` agree on dual poll health without secrets.
- Existing dual SuperGrok, Heavy fresher-slot, console-dead recovery, and limits-chrome-on-credits contracts stay green.
- No C4 invent, no license charts, no Token Economy scope creep.

---

## 11. One-line parent summary

Dual SuperGrok polls each principal’s own JWT; shared-pool fill intentionally copies included % and Extra Usage Credits onto empty slots; fail notes are soft (12-char id, no role/CTA) and TUI sibling fails are debug-only; doctor has no poll health; ranking can still load cold JWTs; **recommended plan = Option B**: fail-loud role + re-login, label fill provenance, never paint sibling-only as active success, doctor/limits poll health, bounded rank demotion; keep unified fill; non-goals C4 invent, license charts, Token Economy.
