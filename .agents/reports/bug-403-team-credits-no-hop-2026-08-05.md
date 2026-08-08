# Dogfood: team credit 403 on console, no hop to SuperGrok

**Date:** 2026-08-05
**Session:** Systems Lean, cwd `~/Projects/ai/iso`
**Surface:** status `135K / 200K | 0/6 ✓ | ...%` · Grok 4.5 (high) · always-approve
**Wire error (console team):** HTTP **403 Forbidden**
`Your team 61fab250-b2c1-40cf-b5b8-628e673a2eeb has either used all available credits or reached its monthly spending limit. …`

Team id matches Surmount **console** team (not SuperGrok OAuth).

---

## Answers (from code)

### 1. Is this 403 hop-worthy (credits/spend) or hard fail?

**Hop-worthy as credit-exhausted**, when a live failover identity remains.

| Layer | Behavior |
|-------|----------|
| Classification | `SamplingError::is_credit_exhausted()` → `is_credit_exhausted_status_and_message(403, msg)` |
| Body match | `is_credit_exhausted_message` matches **`spending limit`** (also "out of credits", "usage limit", …). This team text matches. |
| Bare 403 | Not credit (policy / ZDR). Status alone is not enough. |
| After classify | If hop succeeds: rotate + Retrying toast (`HopCause::CreditExhausted`). If hop fails (empty failover): **fatal** (not slept as throttle). |

Key code:

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampling-types/src/error.rs`
  `is_credit_exhausted_message`, `is_credit_exhausted_status_and_message` (~528–557)
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/actor/request_task.rs`
  `apply_retry_decision` credit path first (~527–548)

There is **no** dedicated test fixture for the exact “used all available credits or reached its monthly spending limit” string; it still matches via `spending limit`.

### 2. Does dual-auth hop console → SuperGrok (or reverse) on this 403?

**Only if SuperGrok is still in `failover_api_keys` (and not memoized dead).** Mid-turn hop does not re-resolve auth from disk; it only pops the prebuilt failover list + dual-auth host/bearer switch.

| Direction | When it works |
|-----------|----------------|
| SuperGrok → console | Classic dual-auth: console keys in failover. Auto rank with included headroom: **console omitted** from chain (limits-before-credits) until included is full. After included full + no positive SuperGrok $ extras: console becomes primary. |
| Console → SuperGrok on team 403 | Works only when SuperGrok JWT is still queued as failover. |

**Limits-first ranking gap (likely this dogfood):**
When free SuperGrok period is treated as exhausted **and** SuperGrok $ extras are 0 or unknown,
`order_credentials_for_preferred_auto` sets:

- primary = first console key
- failover = **remaining console keys only**
- `session_identity_key = None`
- SuperGrok tokens **omitted** from the hop chain

Contract is intentional in ranking (“do not invent after-burner without positive prepaid”). Side effect: **console team credit 403 cannot hop back to SuperGrok free period**, even if free period has actually reset, until **re-resolve** puts SuperGrok primary again.

Classic dual-auth without auto rank (`auto_use_included_limits = false`, `preferred_method = oauth`): session primary, console in failover. Console 403 is only hit after already hopping to console; SuperGrok is then often **memoized exhausted** and **pruned**, so reverse hop also fails.

`preferred_method = api_key` (console pin): SuperGrok still sits in failover → this 403 **should** hop to SuperGrok if the JWT is live and not memoized dead.

Key code:

- `order_credentials_for_preferred_auto`
  `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` (~433–517)
  test: `auto_both_included_exhausted_console_primary_no_supergrok_primary`
- `rotate_identity_config` / `try_rotate_to_failover_key`
  `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/prefer_live_primary.rs`
  `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/actor/request_task.rs`
- Resolve entry: `resolve_credentials_preferring_with_rank` / `_with_supergrok_sessions`
  `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/agent/config.rs` (~5050–5184)

### 3. Why status shows `...%` instead of SuperGrok free period used %?

**Honest cold / unknown included reading**, not a random glitch.

- Status compact meter is SuperGrok **included** usage (`CreditBalance.usage_pct` only when `included_usage_known`).
- When billing has neither `credit_usage_percent` nor a usable monthly limit/used pair, chrome paints **`...%`** (ASCII dots), never silent `0%`.
- Path: `credit_bar_line_for_session` → `credit_bar_loading_line`
  `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/credit_bar.rs` (~1003–1050)

So `...%` means: SuperGrok included meter not warmed (or last fetch failed / never succeeded). It does **not** mean “console team prepaid %”. Live sampling can still be on the **console key** while the footer meter still speaks SuperGrok included honesty.

### 4. preferred_method=oauth + auto_use_included_limits + free period reset: should this path still be on console?

**Should prefer SuperGrok free period after a real reset (used % &lt; 100), not stay on console forever.**

Recovery path in code (when billing poll succeeds):

1. Shell billing feed remembers usage and calls `apply_billing_usage_to_session_exhaust_with_period`.
2. Live used % &lt; 100 clears stale out-of-allowance memo and sets `included_remaining` from %.
3. Next resolve with auto rank: SuperGrok primary; console **omitted** from chain while included has headroom.

If stuck on console after operator believes period reset:

| Likely cause | Mechanism |
|--------------|-----------|
| Billing still cold (`...%`) | No live % → memo / default remaining can keep ExhaustedAll → console primary |
| Memo still exhausted, no successful poll | `load_supergrok_session_candidates` zeros remaining when memo exhausted and no billing row |
| Extras path | Positive SuperGrok $ extras would keep SuperGrok primary (after-burner); 0/unknown → console |
| Hard-expired SuperGrok JWT | Never counts as included headroom |

With dogfood **`...%` + console team 403**, the strongest code-backed story is: **still sampling on console** (team spend/credits dead), **SuperGrok included meter never warmed this session**, so auto re-rank never put free SuperGrok back as primary, and **failover list has no SuperGrok** so the 403 cannot hop.

Config alone (`preferred_method = oauth`, `auto_use_included_limits = true`) is **necessary but not sufficient** without a live included reading (or a non-exhausted SuperGrok candidate).

### 5. Is the error plain enough or wrapped poorly?

**Upstream body is plain; chrome double-wraps it.**

| Surface | Shape |
|---------|--------|
| Retry failed | `SamplingError` Display: `API error (status 403 Forbidden): Your team …` |
| Turn failed | ACP `internal_error` + `terminal_error_data` → often **`Internal error: { "message": "API error (status 403 …)", "http_status": 403 }`** |

Map path:

- `map_sampling_err_to_acp`: **403 → `internal_error().data(message)`** by design (not re-auth; comment says content-safety / ZDR / permission).
  `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/sampling/error.rs` (~114–134)
- Terminal sampler failure always `acp::Error::internal_error().data(terminal_error_data(...))`
  `sampler_turn.rs` (~1144–1158)
- Edge 52x/502–504 get plain English; **credit 403 does not**.
- CreditLimitBlock is a separate max-tier UX card, not this team API path.

So: message content is usable; **“Internal error” + JSON envelope + “API error (status …)” prefix** is the poor wrap. Rate-limit path has a dedicated ACP code and friendlier copy; team credit 403 does not.

### 6. Product gaps vs expected behavior

| Expected | Code today |
|----------|------------|
| Hop to SuperGrok free period when console team prepaid / monthly spend limit hits | Credit **classification** yes; **failover often empty of SuperGrok** under auto ExhaustedAll (and after SuperGrok→console hop + memo). |
| Prefer SuperGrok after free period reset with oauth + auto | Yes **if billing % &lt; 100 lands**; cold `...%` / stale memo leaves console. |
| Clear chrome when % unknown | `...%` is intentional honesty; gap is **not** inventing 0%, gap is **no plain “SuperGrok limits loading / unknown” vs “on console team, SuperGrok meter N/A”** while console-live. |
| Plain team-credit failure UX | Body text ok; **Internal error** + status prefix + no hop toast when nothing to hop to. |

---

## Root cause (plain English)

1. The request used the **Surmount console API key** (team id in the 403).
2. That team is **out of prepaid credits or over monthly spend** — product correctly treats that wording as **credit-exhausted** (hop-worthy).
3. Hop still **did not run** (or found nothing) because under free-period-first ranking, once SuperGrok included is treated as full and SuperGrok top-up dollars are not known positive, **SuperGrok is dropped from the failover list**. Mid-turn hop cannot invent a SuperGrok identity that resolve never queued.
4. Status **`...%`** means SuperGrok included usage was **never known** this session (billing cold/fail), so the client also may not have re-ranked onto free SuperGrok after a period reset.
5. The failure then surfaces as **Retry failed** + **Turn failed: Internal error {…}**, wrapping an already-clear API sentence.

Not primarily “403 not classified.” Primary product gap: **console-dead cannot fall back to SuperGrok free period under current auto order**, and **cold SuperGrok billing leaves chrome and rank stuck**.

---

## Residual recommendations

### Product fix (code)

1. **Console team credit 403 → SuperGrok free period**
   When live sampling is console and the error is credit-exhausted team spend/credits, and SuperGrok session has included headroom (or is not memo-dead and JWT is live), hop to SuperGrok even if auto ExhaustedAll omitted it from `failover_api_keys`. Options: keep SuperGrok in failover as “recovery only,” or re-resolve / re-bind session identity on credit fatal before terminal fail.
   Red tests: exact team spending-limit body; ExhaustedAll order + console 403 must hop when SuperGrok headroom known; no hop when SuperGrok also dead.

2. **Period reset + cold meter**
   Ensure session start / first turn forces SuperGrok billing warm enough to clear memo and re-rank before burning console; or when meter is unknown, do not silently prefer console if oauth + auto and SuperGrok JWT is live (park ranking policy carefully).

3. **Chrome when unknown**
   While console-live: show console team honesty (Management prepaid / spend) or plain “SuperGrok % unknown · sampling on console key”, not only bare `...%` that reads like SuperGrok still drives the turn.

4. **Error copy**
   Credit-exhausted 403: strip `API error (status …):` and avoid ACP **Internal error** JSON for terminal agentResult (same spirit as outage plain English / rate-limit code). Prefer team admin “add credits / raise spend limit” copy when body matches team spend.

### Operator config / ops (no code)

1. Confirm `[auth] preferred_method = "oauth"` and `auto_use_included_limits = true` (or default true).
2. `grok login` SuperGrok session live; `grok-oss limits` / `/limits` — if SuperGrok included still 100% or blank, free period may not have reset on wire.
3. Team spend: raise console team monthly limit or purchase team credits on console.x.ai for team `61fab250-…`.
4. Temporary: if SuperGrok free period has headroom but binary stays on empty-failover console, only a **rebuild + warm billing** or explicit SuperGrok-primary path helps; config pin alone will not hop mid-403 without SuperGrok in the failover list.

### Non-goals / do not invent

- Do not treat bare 403 as credit hop.
- Do not merge SuperGrok free period pool with console team prepaid.
- Do not clear SuperGrok credit memo on SuperGrok session 200 (extras after-burner contract).

---

## Key files / functions (index)

| Area | Path |
|------|------|
| Credit body classify | `xai-grok-sampling-types/src/error.rs` — `is_credit_exhausted*`, tests `credit_exhausted_detects_*` |
| Mid-turn hop | `xai-grok-sampler/src/actor/request_task.rs` — `apply_retry_decision`, `try_rotate_to_failover_key` |
| Rotate / memo / prune | `xai-grok-sampler/src/prefer_live_primary.rs`, `exhausted_identity.rs` |
| Auto rank order | `xai-grok-shell/src/auth/supergrok_identity_rank.rs` — `order_credentials_for_preferred_auto` |
| Candidates + billing enrich | `xai-grok-shell/src/auth/allowance_exhaust_from_billing.rs` — `load_supergrok_session_candidates` |
| Resolve dual-auth | `xai-grok-shell/src/agent/config.rs` — `resolve_credentials_preferring_*` |
| Billing → memo clear | `xai-grok-shell/src/extensions/billing.rs` (~501–526) |
| Status `...%` | `xai-grok-pager/src/views/credit_bar.rs` — `credit_bar_line_for_session`, `credit_bar_loading_line` |
| ACP wrap | `xai-grok-shell/src/sampling/error.rs` — `map_sampling_err_to_acp`, `terminal_error_data` |
| Terminal fail path | `xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` |
| Docs | `xai-grok-pager/docs/user-guide/02-authentication.md` (dual-auth + free-period-first table) |

---

## Suggested regression filters (if implementing)

- Unit: team monthly spending limit 403 message → `is_credit_exhausted() == true`.
- Rank + hop: ExhaustedAll console-primary + SuperGrok headroom after “reset” → primary SuperGrok, console omitted.
- Hop: console primary, SuperGrok only as recovery candidate, team credit 403 → hop reason “out of allowance” / console→session toast.
- UI: terminal credit 403 agentResult must not be bare `Internal error: {…}` JSON for operators.
