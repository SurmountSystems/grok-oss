# Plan inventory: console team 403 cannot hop to SuperGrok free period

**Date:** 2026-08-07
**Mode:** read-only inventory (no product edits)
**Dogfood source:** [`.agents/reports/bug-403-team-credits-no-hop-2026-08-05.md`](bug-403-team-credits-no-hop-2026-08-05.md)
**Related:** [`.agents/reports/plan-oauth-after-period-reset-2026-08-04.md`](plan-oauth-after-period-reset-2026-08-04.md)

---

## Problem (four slices, one product outcome)

| # | Operator pain | Code-backed cause |
|---|---------------|-------------------|
| 1 | Console team **403** (credits / monthly spend) does not hop to SuperGrok free period | Auto `ExhaustedAll` makes console primary and **omits SuperGrok** from `failover_api_keys` + `session_identity_key`. Mid-turn hop only pops that prebuilt list. Memo prune also drops SuperGrok after included-full mark. |
| 2 | Cold SuperGrok billing (`...%`) leaves rank stuck on console after free period may have reset | Period recovery needs a live included % (`apply_billing_usage_to_session_exhaust*` / enrich). No warm reading → stale memo / zero remaining → ExhaustedAll stays. |
| 3 | Limits vs console team credits confusion; status chrome when console-live | Compact meter is SuperGrok included only. `/usage` already has console-live honesty; compact status still paints bare SuperGrok `...%` while sampling may be on console. |
| 4 | Poor terminal UX for team credit 403 | Body is clear; chrome wraps as **Internal error** + JSON (`map_sampling_err_to_acp` 403 → `internal_error`; `sampler_turn` always `terminal_error_data`). |

Optional #5: distinguish **monthly spend limit** vs **zero prepaid** only if message text is enough (no separate wire codes today). Treat as copy polish inside #4.

**Root product gap (one sentence):** credit classification already works; **console-dead cannot recover onto SuperGrok free period** under free-period-first ranking once SuperGrok is off the hop chain and billing is cold, then the failure is wrapped as Internal error.

---

## Recommended approach (full, not half measures)

Ship **one vertical**: **Console-dead recovery to SuperGrok free period**, with billing warm + chrome + terminal copy as the same slice’s honesty surfaces. Do not ship hop alone without terminal plain English, or chrome alone without hop.

### Core policy (Design A preserved)

Keep limits-before-credits:

- While SuperGrok included has headroom → SuperGrok primary, **console omitted** from chain.
- Included full + SuperGrok $ extras &gt; 0 → SuperGrok after-burner primary, console failover.
- Included full + extras 0/unknown → **console primary** (unchanged intent).

**Change only the reverse direction:** when console is primary under that ExhaustedAll path, SuperGrok must remain a **recovery identity** so a **console team credit/spend 403** can hop back to free SuperGrok period (or after-burner SuperGrok if extras later known positive). Do **not** invent SuperGrok debit. Do **not** merge SuperGrok free period with console team prepaid.

### Mechanism (three cooperating seams)

1. **Resolve order: recovery SuperGrok on ExhaustedAll**
   In `order_credentials_for_preferred_auto`, when included is exhausted and extras are not known positive:
   - primary = first console key (same as today)
   - failover = remaining console keys, then **non-hard-expired SuperGrok session tokens as recovery tail**
   - `session_identity_key` = first recovery SuperGrok token (today this is `None` under ExhaustedAll)
   - Host split fields already exist (`session_base_url` / `failover_base_url`) for console↔proxy switch

2. **Credit hop + memo: do not prune recovery SuperGrok into permanent dead-end when console dies**
   Today `prefer_live_identity_after_credit_exhaust` / `prune_exhausted_failover_candidates` drop SuperGrok after the included-full **preemptive memo**. That is correct for “do not silently retry SuperGrok extras first,” but it blocks console→SuperGrok recovery.
   Fix at hop time (not by deleting the preemptive mark wholesale):
   - On **credit-exhausted** while active identity is **console**, if failover has no SuperGrok and `session_identity_key` is set (or re-resolve finds a live SuperGrok JWT), **attempt recovery hop** to SuperGrok session (bearer re-bind via `session_bearer_resolver` already exists).
   - Prefer re-read billing snapshot first: if included used % &lt; 100, **clear SuperGrok memo** then hop (period reset path).
   - If billing still cold/unknown: hop to live SuperGrok JWT **once** as recovery attempt (operator free period may have reset; wire decides). If SuperGrok then returns real credit/usage-limit, mark SuperGrok exhausted and terminal-fail with plain copy (no infinite hop).
   - Rate-limit hops may still prefer next console key before SuperGrok recovery if policy wants to avoid burning SuperGrok on throttle; credit path always tries SuperGrok recovery before terminal.

3. **Billing warm so rank and memo agree after period reset**
   - Session / first-turn path already has billing feed hooks (`extensions/billing.rs`, pager `dispatch/billing.rs`). Ensure auto+oauth dual-auth **forces SuperGrok included warm** before relying on ExhaustedAll for the turn when meter is unknown.
   - Existing pure helpers already clear memo on live % &lt; 100: `apply_included_billing_to_headroom`, `enrich_candidates_with_included_billing`, `apply_billing_usage_to_session_exhaust_with_period`. Wire them so cold `...%` cannot silently keep console-only forever when a live SuperGrok JWT exists.

4. **Chrome honesty (same ship)**
   Compact status when **live sampling is console**: do not leave only SuperGrok `...%` as if SuperGrok is driving the turn. Reuse `/usage` console-live stack:
   - `format_usage_summary_with_live_identity_*`, `ConsoleTeamPrepaidGap`, `SamplingIdentityKind`
   - Compact form: e.g. console team prepaid `$N` / gap string, or plain “console · SuperGrok % unknown”, still distinct meters.

5. **Terminal error copy (same ship)**
   When `is_credit_exhausted()` and status 403 (team spend body), terminal agentResult must be **plain American English** (team admin: add credits / raise monthly spend limit), not `Internal error: { "message": "API error (status 403…)", … }`. Mirror outage/rate-limit friendliness; keep bare 403 (policy/ZDR) on existing internal_error path.

### Optional micro-copy (#5)

If message contains both “used all available credits” and “monthly spending limit,” keep one sentence that covers either case (upstream already does). No new wire enum unless API later adds codes.

---

## Critical files (absolute paths)

| Area | Path | What to touch / reuse |
|------|------|------------------------|
| Credit classify | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampling-types/src/error.rs` | `is_credit_exhausted_message`, `is_credit_exhausted_status_and_message`, existing tests; add exact team body fixture |
| Mid-turn hop | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/actor/request_task.rs` | `apply_retry_decision` credit path (~531); may call recovery after empty failover |
| Rotate / prune / sticky | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/prefer_live_primary.rs` | `rotate_identity_config`, `prune_exhausted_failover_candidates`, `prefer_live_identity_after_credit_exhaust`, `session_bearer_resolver` re-bind |
| Hop labels | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/exhausted_identity.rs` | `HopCause::CreditExhausted`, `format_hop_reason` (“out of allowance”) |
| Sampler config | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/config.rs` | `failover_api_keys`, `session_identity_key`, session/failover base URLs |
| Auto rank | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` | **`order_credentials_for_preferred_auto`** ExhaustedAll branch (~504–517); `AutoCredentialOrder`; test `auto_both_included_exhausted_console_primary_no_supergrok_primary` **must be revised** to recovery contract |
| Candidates + billing | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` | `load_supergrok_session_candidates`, `apply_billing_usage_to_session_exhaust*`, enrich helpers |
| Resolve dual-auth | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/agent/config.rs` | `resolve_credentials_preferring_with_supergrok_sessions` (~5102–5184); session_identity fill when console primary |
| Turn sticky + terminal | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` | `prefer_live_identity_after_credit_exhaust` (~551); terminal `internal_error` + `terminal_error_data` (~1153) |
| ACP map | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/sampling/error.rs` | `map_sampling_err_to_acp` 403 branch; `terminal_error_data` |
| Billing feed | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/extensions/billing.rs` | warm + `apply_billing_usage_to_session_exhaust_with_period` |
| Pager billing | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/dispatch/billing.rs` | same apply on poll |
| Status chrome | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | `credit_bar_line_for_session`, `credit_bar_loading_line`; reuse console-live `/usage` helpers |
| Actor hop integration | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/tests/test_actor.rs` | pattern: `credit_exhausted_with_failover_still_hops` |
| Docs (after green) | user-guide `02-authentication.md` (dual-auth free-period-first table) | short note: console team dead → recovery hop to SuperGrok free period when JWT live |

---

## Functions to reuse (do not reinvent)

| Function | Role |
|----------|------|
| `is_credit_exhausted` / `is_credit_exhausted_message` | Already matches “spending limit”; team dogfood body qualifies |
| `order_credentials_for_preferred_auto` | Single order SoT for auto rank; extend ExhaustedAll only |
| `order_after_supergrok_included_exhaust` | Sibling SuperGrok before console (already good) |
| `load_supergrok_session_candidates` | Auth.json + memo + billing enrich |
| `included_remaining_from_usage_pct` / `usage_pct_has_included_headroom` | Period reset headroom |
| `apply_included_billing_to_headroom` / `enrich_candidates_with_included_billing` | Live % clears memo tokens |
| `apply_billing_usage_to_session_exhaust_with_period` | Feed → memo + ranking headroom |
| `rotate_identity_config` + `session_bearer_resolver` | Console→session host/bearer switch without re-stash |
| `try_rotate_to_failover_key` / credit branch in `apply_retry_decision` | Mid-turn hop + Retrying toast |
| `format_hop_reason` / `HopCause::CreditExhausted` | “out of allowance” chrome |
| `SamplingIdentityKind` + `format_usage_summary_with_live_identity_*` + `ConsoleTeamPrepaidGap` | Console-live honesty |
| `credit_bar_loading_line` | Keep honest cold SuperGrok meter; layer console-live label |

---

## Red test contracts (named; observe fail before product edit)

### A. Classification

1. **`credit_exhausted_detects_console_team_monthly_spending_limit_403`**
   Exact dogfood body:
   `Your team … has either used all available credits or reached its monthly spending limit.`
   + status 403 → `is_credit_exhausted() == true`.
   Bare 403 / “usage guidelines” still false.

### B. Rank + recovery identity

2. **`auto_exhausted_all_console_primary_keeps_supergrok_recovery_in_failover`**
   (Replaces or tightens `auto_both_included_exhausted_console_primary_no_supergrok_primary`.)
   Included remaining 0, extras 0/None, live SuperGrok JWT:
   - primary = console
   - SuperGrok token in `failover` (recovery)
   - `session_identity_key` = SuperGrok token
   - still `primary_is_supergrok_included == false`
   Design A unchanged: while included headroom &gt; 0, console still omitted.

3. **`auto_exhausted_all_hard_expired_supergrok_not_recovery`**
   Hard-expired JWT must not be recovery tail.

### C. Hop (sampler)

4. **`console_team_credit_403_hops_to_supergrok_recovery`**
   Console primary 403 team spend body; SuperGrok recovery in failover (or session_identity + bearer); expect Retrying “out of allowance” and second attempt on SuperGrok token. Pattern: `credit_exhausted_with_failover_still_hops` with dual hosts if needed.

5. **`console_team_credit_403_no_hop_when_supergrok_also_dead`**
   SuperGrok hard-expired or SuperGrok already memo-dead from wire credit error, empty recovery → fatal (no false hop).

6. **`period_reset_billing_clears_memo_and_supergrok_primary_again`**
   Memo exhausted + live usage 12% → clear memo; `order_credentials_for_preferred_auto` SuperGrok primary, console omitted. (Strengthen existing allowance_exhaust_from_billing tests if gaps.)

### D. Terminal UX

7. **`terminal_credit_exhausted_403_is_plain_english_not_internal_error_json`**
   Map or terminal path for team spend 403: operator-visible string is the team sentence (or short admin copy), **not** bare ACP Internal error envelope JSON as the only message. Rate-limit / bare 403 policy paths unchanged.

### E. Chrome (pager unit)

8. **`compact_status_console_live_does_not_imply_supergrok_drives_turn`**
   When live identity is console and SuperGrok included unknown: compact line must name console gap / “console” honesty, not only SuperGrok `...%` as if SuperGrok is live.

### Suggested filters after green

```text
cargo test -p xai-grok-sampling-types --lib -- credit_exhausted
cargo test -p xai-grok-shell --lib -- auto_exhausted
cargo test -p xai-grok-shell --lib -- order_credentials_for_preferred_auto
cargo test -p xai-grok-sampler --test test_actor -- credit
cargo test -p xai-grok-pager --lib -- credit_bar
```

---

## Ordered ship steps

| Step | Work | Done when |
|------|------|-----------|
| **1** | Red: team body classify (A1) | Observed fail or already green with fixture added |
| **2** | Red: ExhaustedAll recovery order (B2–B3) | Observed fail on new contract |
| **3** | Green: `order_credentials_for_preferred_auto` ExhaustedAll recovery tail + `session_identity_key` | Rank tests green; Design A headroom tests still green |
| **4** | Red: sampler console 403 → SuperGrok hop (C4–C5) | Observed fail with empty-failover / prune interaction |
| **5** | Green: hop path (credit-only recovery inject or non-prune of recovery SuperGrok on console credit fail) + memo clear when billing % &lt; 100 | C4 green; C5 no false hop |
| **6** | Red/green: period reset / cold warm (C6) + ensure first-turn billing apply path under oauth+auto | SuperGrok primary after live % &lt; 100; cold does not strand forever when JWT live |
| **7** | Red/green: terminal plain English (D7) | No Internal error JSON-only for team credit 403 |
| **8** | Red/green: compact console-live chrome (E8) | Status distinguishes SuperGrok free period vs console team |
| **9** | fmt + clippy + targeted tests on touched packages; short user-guide dual-auth note | Post-impl verify clean |

Do **not** split “UX only” or “classify only” as a claimed fix for the dogfood failure. Hop+rank is the product fix; chrome and terminal copy are honesty for the same incident.

---

## Risks

| Risk | Mitigation |
|------|------------|
| SuperGrok recovery after ExhaustedAll re-burns SuperGrok extras or hits usage limit | Credit hop only after console credit death; one recovery attempt; real SuperGrok credit error re-memos SuperGrok and terminals |
| Rate-limit hop flapping console↔SuperGrok | Prefer next console key on rate-limit before SuperGrok recovery; SuperGrok recovery is **credit-path** priority |
| Revising `auto_both_included_exhausted_console_primary_no_supergrok_primary` looks like Design A regression | Keep primary console; only add recovery failover + session key; document named contract change |
| Cold hop to SuperGrok when free period still full | Accept one failed SuperGrok attempt then plain dual-fail copy; billing warm reduces this |
| Memo clear too aggressive | Clear SuperGrok memo only on live included % &lt; 100 (existing enrich contract) or on successful SuperGrok sampling if product already allows (console success already clears console side) |
| ACP 403 internal_error used for content-safety | Branch only when `is_credit_exhausted()`; bare 403 stays internal_error without re-auth |

---

## Non-goals

- Treat bare 403 as credit hop.
- Merge SuperGrok free period with console team prepaid or Management dollars.
- Clear SuperGrok credit memo on SuperGrok session 200 alone (extras after-burner contract).
- Invent free SuperGrok period debit (C4 server ticket).
- Fill Grok Business license charts.
- Operator config-only “fix” (config is necessary but not sufficient without recovery chain + warm billing).

---

## Acceptance (dogfood replay)

Given `preferred_method = oauth`, `auto_use_included_limits = true`, live SuperGrok JWT, console team out of credits/spend:

1. Console team 403 either hops to SuperGrok free period (Retrying toast, sampling continues) **or** fails with plain team admin English if SuperGrok also dead.
2. After free period reset and billing warm (used % &lt; 100), next resolve uses SuperGrok primary and omits console while headroom remains.
3. Status does not imply SuperGrok free period is the live spend pool while sampling is on console.
4. No terminal-only “Internal error” JSON wrap for this team credit body.

---

## Implementer note

Parent should spawn one implementer for steps 1–6 (rank+hop+billing), then chrome+terminal can be same implementer or a short follow-up if file conflict risk is high; do not claim done without red logs for hop and classify. Effort ≥ 2: process mop fmt/clippy/tests after green.
