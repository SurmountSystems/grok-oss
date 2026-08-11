# Critic: billing tests + xAI (Grok) API usage

**Date:** 2026-08-03
**Scope:** Read-only critic pass. No product code edits.
**Primary paths:**
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/billing.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/billing.rs` (`handle_billing_fetched`)
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` (`Effect::FetchBilling` / `FetchAppBilling`)
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs` (`credit_balance_from_config`)
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs`
- `crates/codegen/xai-grok-shell/src/auth/xai_management.rs`
- `crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs`
- `crates/codegen/xai-grok-shell/src/token_economy/`

**Race note:** If another agent is greening the two CI unit fails around `billing_fetched_none_balance_clears_cached` (or sibling dispatch asserts), treat this report as **strengthen-after-green**. Do not race product edits in `billing.rs` / `handle_billing_fetched`. Prefer additive red tests once the tree is stable.

**Stance:** Never assume malice; never assume competence (ours or xAI's). Evidence from code paths and public docs; docs can lie.

---

## 1. What the failing / cleared-cache contract was trying to guarantee

Plain English:

1. **Successful SuperGrok credits parse with no `config` object** is not a transport error. It means "this response has no billing configuration for this principal."
2. In that case the TUI must **drop stale SuperGrok included / extras state** on both the app and the agent so the status bar and chrome do not keep painting last week's 80% while scrollback says **"No billing data available."**
3. **Polling for SuperGrok-only near-exhaust** must also stop when there is no config, so we do not spin silent `FetchBilling` forever on a principal that has nothing to show. (OpenRouter account cents and console team prepaid are separate meters; they can still keep polling when present.)
4. **Real parse/transport failures** are supposed to go to `BillingError`, which today does **not** clear the cache (last-known-good). The None-balance path is intentionally the "honest empty config" path, not the "network blip" path.

The named test:

```text
billing_fetched_none_balance_clears_cached
```

Seeds a known balance + forces `billing_poll_wanted = true`, dispatches `BillingFetched { balance: None, … }`, then asserts:
- `app.credit_balance.is_none()`
- `!app.billing_poll_wanted`

Sibling: `billing_fetched_none_balance_shows_no_data_message` only checks that non-silent None pushes one scrollback line (the "No billing data available." body via `format_usage_summary_*`).

Product context that raised the temperature:
- `included_usage_known` / false-0% chrome (empty config → `Some(CreditBalance)` with `included_usage_known: false`, not invent a known 0%).
- Period-reset exhaust memo (live free SuperGrok period % mark/clear).
- Console team prepaid / postpaid (Management) kept distinct from SuperGrok prepaid extras.
- Token Economy double-entry (local book vs remote samples; no invented free SuperGrok period debit).

There is a **product-intent tension** already written in a same-day join note (`.agents/joins/feat-startup-session-and-limits-chrome-2026-08-03.md`): that note says poll should stay true for "included unknown **or no config yet**," while `handle_billing_fetched` and the clear-cache test treat **None balance** as "stop SuperGrok polling." Those are different states and need explicit contracts (see gaps table).

---

## 2. Gaps in the test suite (prefer stronger contracts)

| Missing / weak contract | Why it matters | Suggested red test name + assert shape |
|-------------------------|----------------|----------------------------------------|
| **None balance clears agent cache, not only app** | Footer/status often read `agent.credit_balance` first; app-only clear leaves dual chrome disagreeing. | `billing_fetched_none_clears_agent_and_app_credit_balance` — seed app+agent with `test_bal(80)`, dispatch None, assert **both** `app.credit_balance` and `agent.credit_balance` are `None`. |
| **None balance vs empty-config `Some(bal)` with unknown included** | Mapping path turns empty `BillingConfig` into `Some(CreditBalance { usage_pct: 0.0, included_usage_known: false })`, not `None`. Different chrome, poll, and exhaust semantics. | `billing_fetched_unknown_included_keeps_poll_and_honest_placeholder` — dispatch bal with `included_usage_known: false`, `usage_pct: 0.0`; assert `billing_poll_wanted`; assert scrollback/chrome path would not claim "0%" as known (via format helpers or snapshot fields). |
| **True wire 0% vs unknown 0 placeholder** | Product goal: true zero paints `0%`; unknown paints `...%` / "not yet available." Dispatch tests almost always use `test_bal` with `included_usage_known: true`. | `billing_fetched_true_zero_included_known_does_not_force_poll` (usage 0, known) vs `…_unknown_forces_poll` (usage 0, unknown). Assert poll and any summary string diverge. |
| **Transport fail + Management/OR success must not clear SuperGrok** | In `Effect::FetchBilling`, ACP error **or** SuperGrok parse error with OR/console prepaid present returns `BillingFetched { balance: None, … }`, which **clears** SuperGrok cache. Pure SuperGrok failure goes to `BillingError` and keeps cache. Asymmetry is a fail-open that **lies** about SuperGrok when another meter is warm. | `fetch_billing_supergrok_error_with_console_prepaid_keeps_prior_supergrok_balance` — unit at effects or a pure policy helper: when SuperGrok path fails and prior bal exists, result must be keep-last-good (or `BillingError`), never None that wipes SuperGrok. Prefer a small pure function so the test does not need full ACP. |
| **BillingError never clears; silent error leaves stale % forever** | Last-good is correct for blips, but there is no age / "stale" flag and no test that silent error leaves chrome unchanged. | `billing_error_silent_preserves_cached_balance_and_poll` — seed bal + poll; silent error; assert bal and poll unchanged; assert scrollback length unchanged. |
| **OpenRouter / console prepaid keep poll when SuperGrok None** | Current code: `unwrap_or(false) \|\| openrouter.is_some() \|\| console_prepaid.is_some()`. Test only checks SuperGrok-only path. | `billing_fetched_none_with_console_prepaid_keeps_poll` and `…_with_openrouter_keeps_poll` — seed OR or console cents, dispatch SuperGrok None; assert SuperGrok bal cleared **and** `billing_poll_wanted` still true. |
| **Autotopup Cleared on None / no prepaid** | Effects clear autotopup when no prepaid credits; dispatch tests cover Cleared when credits still exist at 50%. None balance path uses `Unchanged` in helper `dispatch_billing`. | `billing_fetched_none_with_cleared_autotopup_resets_rule` — None + `AutoTopupFetch::Cleared` after a seeded rule; assert app+agent `auto_topup` None. |
| **Exhaust memo not applied on unknown included** | `handle_billing_fetched` always calls `apply_billing_usage_to_session_exhaust(bal.usage_pct)` when `Some(bal)`. Placeholder `0.0` + unknown can **clear** a prior Marked exhaust as if free SuperGrok period reset. Shell path only applies when `included_usage_and_period_end` returns `Some(pct)`. | `billing_fetched_unknown_included_does_not_clear_allowance_exhaust_memo` — (with temp grok_home / sampler test hooks if available) mark exhaust, dispatch unknown included, assert memo still Marked / action None. |
| **Pager remember ranking cache invents 0%** | `Effect::FetchBilling` calls `remember_active_supergrok_included_billing(…, bal.usage_pct, …)` whenever `balance` is `Some`, even if `!included_usage_known` (placeholder 0.0). Shell only remembers when `usage_pct` is `Some`. | `remember_active_skipped_when_included_usage_unknown` — pure: mapping + remember gate; assert process map does not get `usage_pct: Some(0.0)` from empty config. |
| **AppBillingFetched path is thinner than agent path** | App path applies exhaust + cache write but does **not** set `billing_poll_wanted`, does not rebuild `/limits`, does not push `/usage` scrollback. Divergence can leave app poll state wrong after app-level silent poll. | `app_billing_fetched_none_clears_app_credit_balance` + `app_billing_fetched_unknown_included_does_not_invent_known_zero` — mirror agent contracts for the fields App path actually owns. |
| **Meters must stay distinct in dispatch outcomes** | Console live tests exist for `/usage` copy; still missing: SuperGrok prepaid extras cents must not overwrite `console_team_prepaid_cents`, and console cents must not appear as SuperGrok `prepaid_balance_cents`. | `billing_fetched_console_prepaid_does_not_mutate_supergrok_prepaid_field` — bal with SuperGrok extras + separate console cents; assert fields stay on their slots after dispatch. |
| **Token Economy double-entry honesty** | Local book vs remote Management samples; reconcile text already names distinct meters. Dispatch suite does not assert that a SuperGrok None clear does not invent a free SuperGrok period debit row. | Prefer shell/token_economy tests: `reconcile_does_not_invent_free_supergrok_period_debit_when_remote_absent` — remote missing → report "unavailable", not $0 local debit. |
| **Flat multi-sample SuperGrok poll honesty** | Non-silent `/usage` already threads `flat_poll_evidence_from_history` + OAuth postpaid dominates. No dispatch test forces unproven flat + asserts copy. | `usage_billing_flat_poll_unproven_mentions_honesty_note` — inject flat history (shell test hooks), non-silent fetch with known bal; assert summary contains the honesty family (not a soft "all good at 65% forever" alone). |
| **subscription_tier with None balance** | `billing_fetched_updates_subscription_tier` passes None bal + tier and only checks tier. Does not assert bal clear + poll + message consistency. | Extend or add: `billing_fetched_none_with_tier_updates_tier_and_clears_balance`. |
| **`test_bal` always sets `included_usage_known: true`** | Convenient, but every dispatch test inherits "known" and cannot regress unknown chrome by accident of helper. | Add `test_bal_unknown()` helper; use it in unknown contracts. Do not weaken existing known tests. |

Do **not** weaken asserts to match buggy clear-on-OR-success behavior. If product decides "None means clear SuperGrok always," document that and add the OR/console keep-poll tests; if product decides "transport fail keeps SuperGrok," fix the effects mapping and strengthen tests around keep-last-good.

---

## 3. Client-side risks

### 3a. Cache and fail-open that can lie

| Risk | Evidence | Severity |
|------|----------|----------|
| SuperGrok wipe when another meter succeeds | `FetchBilling`: ACP/`BillingConfigResponse` parse failure + OR or console prepaid → `BillingFetched { balance: None }` → `handle_billing_fetched` assigns `app.credit_balance = None` and clears agent via `apply_credit_balance`. Pure SuperGrok failure → `BillingError` (cache kept). | **High** — operator sees "No billing data" + still-live console $, after a transient SuperGrok proxy error. |
| Last-good forever on silent `BillingError` | Error handler only optional scrollback; no TTL, no "stale" bit on `CreditBalance`. | Medium — chrome can show 12% for hours after billing is dead if silent polls fail. |
| None vs empty config confusion | `balance = billing.config.map(credit_balance_from_config)`: missing object → None; empty object → Some(unknown). UI messages differ ("No billing data" vs "not yet available" / `...%`). | Medium — product and tests must name both. |
| Poll policy split-brain | Unknown included → poll true; None → poll false (unless OR/console). Join note claimed "no config yet" should keep polling. Cold chrome may freeze on `...%` if first response is truly config-less and no other meters. | Medium — needs an explicit cold-start policy. |
| Autotopup Unchanged vs Cleared | Transport fail on rule → Unchanged (good). No prepaid → Cleared. None SuperGrok with Unchanged can leave stale auto-topup rule if effects ever emit that combo. | Low–medium |

### 3b. Inventing 0% / mashing meters

| Risk | Evidence |
|------|----------|
| Placeholder `usage_pct: 0.0` when unknown | `credit_balance_from_config` / `credit_balance_from_billing_config`: `unwrap_or(0.0)` with `included_usage_known = false`. Chrome is gated; ranking/exhaust are not fully gated. |
| Ranking cache poisoned with false 0% | `Effect::FetchBilling` / `FetchAppBilling`: `if let Some(ref bal) = balance { remember_active_supergrok_included_billing(…, bal.usage_pct, …) }` without checking `included_usage_known`. Shell `handle_get_billing` only remembers when `included_usage_and_period_end` yields `Some(pct)`. |
| Exhaust apply on placeholder 0 | `handle_billing_fetched` / `AppBillingFetched` call `apply_billing_usage_to_session_exhaust(bal.usage_pct)` for any `Some(bal)`. Unknown 0 can clear Marked memo as if free SuperGrok period reset. Shell only applies when pct is known. |
| Console prepaid keep-on-miss | `if let Some(cents) = console_team_prepaid_cents { app… = Some(cents) }` — miss keeps prior (good). But SuperGrok None **does** clear SuperGrok while console may stay. Mixed identity chrome if sampling identity still SuperGrok. |
| SuperGrok $ extras vs console team prepaid | Separate fields exist and copy tests try to keep them honest; risk remains if summary formatters prefer SuperGrok extras while `sampling_identity` is ConsoleKey (partially covered by usage_billing_console_live_* tests). |
| Grok Build product % vs top-level included % | `productUsage` / `grok_build_usage_pct` is optional and distinct; flat top-level % with moving Build % is the flat-poll honesty story. Client must not substitute one for the other. |

### 3c. Identity and dual-auth

- Exhaust Marked → sampling identity console (when console available); Cleared → SuperGrok again only when not `is_api_key_auth`. Dispatch tests barely cover identity transitions with real exhaust side effects (filesystem / sampler durable memo).
- Sibling SuperGrok poll is best-effort in shell; failures leave "no data yet" for non-active principal — good. Active path must not inherit sibling zeros.
- `remember_active_supergrok_included_billing` uses first SuperGrok identity from `auth.json`; multi-slot teams can attribute the wrong principal if disk order lags the token that just polled (shell log path prefers polled auth identity; remember-active is disk scan).

### 3d. Token Economy

- Local ingest from `usage.jsonl`; remote Management / SuperGrok samples as JSON payloads (no secrets). Reconcile copy already refuses to mash free SuperGrok period %, SuperGrok top-up $, console prepaid, postpaid OAuth vs API class, and local calculated spend.
- Risk: treating missing remote as zero remote (false balance). Prefer "unavailable" (already present in reconcile strings). Do not invent free SuperGrok period debits from local tokens alone.

### 3e. Shared rate limits

- SuperGrok billing and Management share multi-process cooldowns (`shared_http_rate_limit`). Good against stampede. Risk: a 429 on one surface delaying the other if keys/host fingerprints collide incorrectly — verify keying stays per-host + credential fingerprint (code comments claim this; keep tests on key identity).

---

## 4. API / server-side risks (with evidence)

Public Management billing reference (accessed: 2026-08-03):
[Billing Management REST](https://docs.x.ai/developers/rest-api-reference/management/billing)

Public rate limits (accessed: 2026-08-03):
[xAI Rate Limits](https://docs.x.ai/developers/rate-limits)

Management guide (accessed: 2026-08-03):
[Using Management API](https://docs.x.ai/developers/management-api-guide)

API billing FAQ (accessed: 2026-08-03):
[FAQ – API Billing](https://docs.x.ai/docs/resources/faq-api/billing)

| Observation | Client impact | Confidence |
|-------------|---------------|------------|
| Management prepaid `total.val` is a **string** and often **negative** for remaining credit (docs example `"-1000"`). | Client maps via `prepaid_remaining_cents_from_total_val` → abs. If server ever sends positive remaining without sign flip, abs still works; if meaning flips to "spent", UI lies. | High (docs + unit tests with dogfood shapes). |
| SuperGrok credits `Cent.val` is **i64** with `#[serde(default)]` because proto3 JSON omits zero. | `$0` arrives as `{}` → 0, not parse fail. Good. Different from Management string cents. Two wire dialects in one product. | High (shell `billing.rs` comments). |
| SuperGrok credits path is **CLI proxy** `GET {proxy}/billing?format=credits` → backend `GetGrokCreditsConfig`, not Management host. | Undocumented to third parties; shape can drift. Client carries dual legacy + new fields. | High (code); external doc for this exact path is thin / product-owned. |
| `config: null` vs `{}` vs full object | Maps to None vs unknown included vs known meters. Server omitting fields is not the same as omitting config. | High for client mapping; **server intent unknown** without live capture. |
| Top-level `creditUsagePercent` can stay flat while `productUsage` / Build % moves (or reverse). | Flat-poll honesty + dual meters; client records poll history. Server may not debit free SuperGrok period for Build-only use the way operators expect. | Medium — product comments + history tests; not a public guarantee. |
| Management postpaid invoice preview lines use **description** class (OAuth vs API attribution is client-side classification of those lines). | Mis-label of description → wrong "OAuth class dominates" honesty flag. | Medium — depends on stable description strings; **needs live capture** if strings change. |
| Usage series is **POST** `…/usage` with `analyticsRequest` (timezone local wall strings, not pure UTC range). | Wrong timezone → wrong day buckets / silent zeros. Docs: cannot rely on UTC for aggregation. | High (docs). |
| Prepaid then postpaid: FAQ says prepaid consumed before postpaid invoice. Soft spending limit 0 → prepaid only. | Console meters can show prepaid remaining while postpaid is 0; not a bug. | High (docs). |
| Rate limits are per team tier (RPS/TPM by spend). Management + SuperGrok proxy can 429 independently. | Shared client cooldowns help; bare 403 on Management validation must not poison peers (code already special-cases). | High (docs + code). |
| Management key validation `teamId` deprecated vs `scopeId` | Client has multi-fallback `team_id_for_billing`. Org-scoped keys may still lack a usable billing team without explicit pin. | High (code + failure copy). |
| SuperGrok OAuth billing vs console API class | Different hosts, different credentials, different products. Server will not merge them into one meter. Client must not. | High. |
| Whether free SuperGrok period allowance resets clear server-side memo | Client clears durable exhaust from live %; if server returns stale 100% after period rollover, client stays Marked. | **Unknown — needs live capture** at period boundary. |
| Whether proxy ever returns HTTP 200 + empty config for expired session | Would clear chrome via None path instead of auth error. | **Unknown — needs live capture.** |

Do not invent "server is wrong" for flat polls without multi-sample history and product % evidence. Flat can be true zero spend, delayed aggregation, or wrong principal.

---

## 5. Recommended next actions (ranked for implementer)

Complete contracts, not half measures. Wait for any in-flight green on the two CI fails before overlapping edits; then strengthen.

1. **Fix SuperGrok keep-last-good when OR/Management succeed and SuperGrok fails**
   Effects must not emit `BillingFetched { balance: None }` as a synonym for "SuperGrok path failed." Options: always `BillingError` for SuperGrok transport/parse fail (optional separate fields for OR/console updates), or a three-state SuperGrok field (`Resolved` / `Cleared` / `Unchanged`) mirroring `AutoTopupFetch`. **Red test before green.**

2. **Gate ranking remember + exhaust apply on `included_usage_known` (or raw `Option<f64>`)**
   Align pager with shell: never feed placeholder 0 into `remember_*` or `apply_billing_usage_to_session_exhaust`. Prefer storing `Option` on `CreditBalance` long-term, or always check the flag.

3. **Name both empty states in tests and docs**
   - `config` absent → clear SuperGrok UI cache + "No billing data" + SuperGrok poll off (unless other meters).
   - `config` present but included unknown → keep `Some(bal)`, `...%`, poll on, do not clear exhaust from placeholder.
   Resolve the join-note vs code tension explicitly in residual/user-guide if cold start still needs forced poll without a bal object.

4. **Strengthen dispatch tests** from the table in §2 (agent clear, OR/console keep poll, unknown vs true zero, silent error preserve, autotopup on None). Run package-scoped:
   `cargo test -p xai-grok-pager --lib -- billing_fetched_ credit_balance_`

5. **AppBillingFetched parity for owned fields**
   At least: clear app cache on None; do not invent known 0 into ranking; document that app path does not drive agent scrollback/poll.

6. **Identity + exhaust integration tests with hermetic grok_home**
   Period reset (100 → 12) clears memo and restores SuperGrok identity when not console-primary; unknown included does not clear; 100% marks when extras policy allows.

7. **Token Economy: keep remote optional and non-inventing**
   Reconcile tests for missing Management samples, missing SuperGrok included, and distinct prepaid/postpaid/OAuth/API class lines. No free SuperGrok period debit invention.

8. **Live capture checklist (operator / dogfood, not CI)**
   At period boundary: credits config before/after reset. On proxy 5xx with Management key present: confirm SuperGrok chrome policy. Postpaid description strings for OAuth vs API class. Build `productUsage` vs top-level % during a Build-only session.

9. **Public citation hygiene**
   Any new comments that quote Management limits or prepaid sign convention: markdown link + `accessed: YYYY-MM-DD` (host/project AGENTS citation standard).

---

## 6. Out of scope / do not invent

- **Do not invent free SuperGrok period allowance debits** from local token counts or from console postpaid lines.
- **Do not merge meters:** free SuperGrok period % ≠ SuperGrok top-up dollar extras ≠ console team prepaid $ ≠ console postpaid invoice (OAuth class vs API class) ≠ OpenRouter account cents ≠ local Token Economy book.
- **Do not treat Management inference rate-limit tiers as SuperGrok included limits** (different products; see rate-limits doc for console API teams).
- **Do not invent GET for usage analytics** (documented POST with `analyticsRequest` only).
- **Do not invent server malice** for flat SuperGrok %; require multi-sample evidence and product % before honesty flags escalate.
- **Do not bulk-rewrite tests to match buggy keep/clear asymmetry**; fix product or name the intentional policy first.
- **Do not race** an in-flight implementer on the same CI fails; park additive red tests until green, then land stronger contracts.
- **No git commit** from agents; no staging unless the operator asked.

---

## Appendix A — current None-balance control flow (summary)

1. Shell `x.ai/billing` succeeds with `BillingConfigResponse { config: None, … }` **or** effects fabricate `balance: None` on SuperGrok fail when side meters exist.
2. `handle_billing_fetched` sets `app.credit_balance = None`, applies autotopup fetch enum, optionally writes OR/console cents.
3. Exhaust action: None branch → `AllowanceExhaustAction::None` (does not clear/mark).
4. `billing_poll_wanted` false unless OR or console prepaid cache is non-empty.
5. Agent: `apply_credit_balance(None, …)` clears agent SuperGrok/OR when not chat-kind; console prepaid only updates on `Some(cents)`.
6. Non-silent: scrollback "No billing data available." (and console honesty branches when identity is console).

## Appendix B — key helpers

| Helper | Role |
|--------|------|
| `included_usage_and_period_end` | SSOT for known free SuperGrok period % (Option). |
| `credit_balance_from_config` | Config → `CreditBalance` + `included_usage_known`. |
| `apply_billing_usage_to_session_exhaust*` | Mark/clear durable exhaust from known %. |
| `AutoTopupFetch` | Resolved / Cleared / Unchanged (model for SuperGrok bal three-state). |
| `prepaid_remaining_cents_from_total_val` | Management signed string cents → abs remaining. |
| `flat_poll_evidence_from_history` | Multi-sample SuperGrok honesty without inventing debit. |

---

*End of critic report. Implementers: red → green on named contracts; parent HITL reads this file only for status.*
