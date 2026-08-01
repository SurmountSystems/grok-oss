# Plan: Limits Half B — console team Business Usage meter (keep Half A)

**Status:** Approved 2026-07-30 with notes.

**Approval notes (operator):** also address **graceful automatic failover** via the second config option with the better name: **`[auth] auto_use_included_limits = true`** (not `preferred_method=auto`). Prefer included SuperGrok weekly limits before SuperGrok $ extras / console; hop on exhaust; honor oidc/oauth/api_key pins. Refine any remaining gaps so this flag drives real graceful failover, not only ranking fixtures.

## Context

**Full limits ask is two halves (operator pin 2026-07-30):**

| Half | What | Status |
|------|------|--------|
| **A** | SuperGrok session meters: included weekly + SuperGrok $ extras, dual SuperGrok login, sibling poll, `/limits` dual rows, footer honesty | **Shipped — keep** |
| **B** | TUI picture of **console team Grok Business Usage class** data (team tokens / spend / prepaid class, e.g. Team Surmount) | **Open — this plan** |

Half A was correct and remains wanted. Half B was wrongly treated as "website only / not CLI." The product goal is **in-CLI display** of that class of data, not fixing console.x.ai HTML.

**Constraints**

- Do **not** scrape console.x.ai HTML or invent undocumented endpoints.
- Meters stay distinct: SuperGrok included weekly ≠ SuperGrok $ extras ≠ **console team prepaid / Business Usage** ≠ second SuperGrok OIDC principal (Business SuperGrok is not console team prepaid).
- Inference console key (`XAI_API_KEY` / `api.x.ai`) has **no** team prepaid balance API in product today. Residual pins Management API:
  `GET https://management-api.x.ai/v1/billing/teams/{team_id}/prepaid/balance`
  with a **separate management key** + known `team_id` (console Settings → Management Keys). Not the same as inference key.
- Existing `"no $ meter yet"` honesty when console is live must stay until a real fetch path exists.
- No plan soft-park B/C/D, no onto/git land in this plan.
- Red/green TDD for new fetch + TUI behavior.

**Non-goals**

- Replacing or ripping out Half A SuperGrok `/limits`.
- Pixel-perfect clone of console.x.ai charts (v1 = real numbers / prepaid + plain usage rows if documented; charts later if data exists).
- Enterprise `GROK_DEPLOYMENT_KEY` managed-config as a substitute for this meter.
- Assuming SuperGrok OIDC `team_id` equals Management API `team_id` without evidence.

**Assumptions**

- Prepaid balance is the first shippable Management API surface already named in residual.
- Token/spend **series** endpoints only if xAI documents them; otherwise v1 is balance (+ honest "no series yet").
- Dogfood of live Management API needs operator management key + team id (not available to hermetic CI).

Evidence join: `/tmp/grok-join-plan-console-business-usage-2026-07-30.md`. Residual SoT: `RESIDUAL.md` §4.

## Approach

**Recommended path:** research → secrets → team_id → hermetic fetch → TUI wire → docs, in that order. Keep Half A paths untouched except where `/limits`/footer **gain** console rows when management data is present.

1. **Endpoint inventory (docs/code only)**
   Confirm prepaid response shape and any **documented** usage/spend/token endpoints. Re-check whether inference key ever gains balance (if yes, prefer simpler path). Write a short research note under `doc/dev/research/` so implementers do not invent series URLs.

2. **Management credential product**
   Secure store for management key (keyring parity with console inference keys; never argv secrets; plain labels). Distinct from `XAI_API_KEY` and SuperGrok OIDC. Existing `[endpoints] management_api_key` is load-only today; either wire it into a real client or replace with a clearer auth-surface name. Prefer clear names: management key for billing, not "deployment key."

3. **`team_id` for Management API**
   Explicit config / UX (config field or interactive set). Do not silently reuse SuperGrok OIDC `team_id` unless research proves equality.

4. **Fetch client + cache**
   Hermetic HTTP mock → map prepaid (and optional usage) into a console-team meter model. Host `management-api.x.ai`. Fail loud on missing key/team_id; keep `"no $ meter yet"` when not configured.

5. **TUI: footer + `/limits`**
   Populate `ConsoleMeter.balance_cents` (and any usage rows) when fetch succeeds. Plain copy: **console team prepaid / Business Usage**. Never SuperGrok extras labels when console is live. When SuperGrok is live, keep Half A SuperGrok rows; when dual principals + console key exist, show both families of meters without merging numbers.

6. **User-guide + residual honesty**
   Document management key + team_id setup; mark Half B shipped only when fetch + TUI green; leave soft chart-series residual if only balance landed.

**Not chosen**

- **Not scrape console.x.ai** — brittle, forbidden by residual.
- **Not invent OpenRouter-style credits on `api.x.ai`** — no such product path.
- **Not treat SuperGrok Business OIDC as console team prepaid** — different meter; already labeled separately in Half A.

## Critical files

| Path | Why |
|------|-----|
| `RESIDUAL.md` §4 | Intent SoT: both halves; rank #1 Half B |
| `crates/codegen/xai-grok-shell/src/extensions/billing.rs` | SuperGrok `GetGrokCreditsConfig` pattern to mirror for management fetch (new module likely) |
| `crates/codegen/xai-grok-shell/src/auth/xai_console.rs` | Console inference key store pattern for management-key parity |
| `crates/codegen/xai-grok-shell/src/auth/credentials_store.rs` | Secret storage patterns |
| `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` | `/limits` VM; `ConsoleMeter.balance_cents` today always `None` |
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Footer `"no $ meter yet"` branch |
| `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs` | `dispatch_show_limits` |
| `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` | Billing refresh effects (`x.ai/billing`) |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | Auth surfaces docs |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` | `/limits` docs |
| Config endpoints / `management_api_key` load path | Stub exists; not a billing client today |

## Reuse

| Symbol / module | Path | How |
|-----------------|------|-----|
| `fetch_credits_config*` / sibling poll | `extensions/billing.rs` | Mirror async HTTP + process cache shape for management prepaid |
| `LimitsSnapshot` / `ConsoleMeter` | `limits_snapshot.rs` | Fill `balance_cents` + new usage fields when present |
| Footer console branch | `credit_bar.rs` | Switch from honest absence to real cents when cache has console team meter |
| Console key keyring path | `xai_console.rs` | Pattern for management key (separate slot/label) |
| Honesty tests | `credit_bar` / `limits_snapshot` / `status` tests | Keep green; add fixtures that only invent balance via mock management path |

## Steps

0. **`auto_use_included_limits` graceful failover refine (size 2)** — **Approval note.** Audit resolve/hop/rank paths for `[auth] auto_use_included_limits = true`: prefer included SuperGrok before $ extras and console; earlier `reset_at` + headroom among included pools; hop on exhaust; honor `preferred_method` oidc/oauth/api_key pins; meter honesty when console is sticky. Fix agent-doable gaps with red→green TDD. Docs already name the field; product path must match. Parallel-safe with step 1 (auth vs research). Can land before management key.

1. **Research note (size 1)** — Documented Management API surfaces only (prepaid balance required; series if real). Confirm response fields. Note whether inference key has any balance. Output: `doc/dev/research/console-team-business-usage-meter-2026-07-30.md` (or dated). Gate: no invented endpoints in the note.

2. **Management key store (size 2)** — Secure store + config surface; distinct from inference key and SuperGrok OIDC; no argv secrets. TDD: round-trip store/load; reject conflation with `XAI_API_KEY`. Depends on 1 for naming/host constants.

3. **`team_id` pin (size 1)** — Config/UX for Management API team id; docs. Optional: warn if SuperGrok OIDC team id differs. Depends on 2.

4. **Management prepaid fetch client (size 2)** — HTTP client + hermetic mock; map to cents (and any documented usage fields). Process cache keyed by team. Fail paths: missing key, missing team_id, HTTP error → leave meter absent. Depends on 2–3.

5. **TUI wire (size 2)** — `/limits` + footer when console live **and** management meter present; plain labels; dual SuperGrok Half A rows unchanged. Red: `console_live_with_management_fixture_shows_prepaid_balance` (named contract). Green: same test after product wire. Depends on 4.

6. **User-guide + residual close-out (size 1)** — Auth + `/limits` docs; RESIDUAL Half B honesty (shipped vs soft "no token series"). Depends on 5.

**Optional later (out of this plan unless research finds endpoints):** time-series / chart-class token+spend rows; richer GRLD-like chrome.

## Risks

| Risk | Mitigation |
|------|------------|
| Management API shape differs from residual pin | Step 1 research; fail-loud parse; no scrape |
| Operators only have inference key | Docs: management key required for Half B; keep `"no $ meter yet"` without it |
| Confuse SuperGrok Business with console team | Separate labels; never copy SuperGrok extras into console live footer |
| Secrets leak in Debug / argv | Follow existing no-argv / keyring patterns; audit Debug derives |
| Overclaim "charts" when only balance ships | Residual honesty: balance v1; series soft if undocumented |

## Verification

**Red → green (named contracts)**

1. **Store:** management key round-trip; not equal to inference key path.
   Command filter: focused `cargo test` on new store module.
2. **Fetch:** hermetic mock of prepaid balance returns cents; missing key leaves `None`.
   RED before client exists; GREEN after.
3. **TUI:** console live + management fixture → `/limits` and footer show console team prepaid line; SuperGrok extras still hidden as live console spend.
   Extend `limits_snapshot` / `credit_bar` / `show_limits_*` tests.
   Keep existing honesty tests green:
   `warning_console_primary_does_not_show_supergrok_extras_dollars`,
   `format_console_live_honest_no_dollar_meter`,
   `show_limits_console_live_keeps_meters_distinct`.
4. **Half A regression:** dual SuperGrok `/limits` filters still pass after TUI changes.
5. **Manual dogfood (operator):** management key + team_id configured → `/usage` or refresh → `/limits` shows console team numbers; without key, still `"no $ meter yet"`. Does not require website to change.

**SCORE:** focused tests fail=0; `just check` or workspace nextest green before claim done.

## Open questions

- Prefer config-only management key first, or interactive login flow in v1? **Recommendation:** config + keyring store first (matches residual "management key UX"); interactive polish if dogfood jars.
- Is prepaid balance enough for "Business Usage class" v1, or is token/spend series required before calling Half B done? **Recommendation:** prepaid + plain remaining/used if in prepaid response = shippable v1; series only if documented in research note.
- Operator team id source for Surmount: paste from console vs discover API? **Recommendation:** explicit config until a list-teams endpoint is documented.

### Critical Files for Implementation
- `RESIDUAL.md` — both-halves intent
- `crates/codegen/xai-grok-shell/src/extensions/billing.rs` — SuperGrok pattern
- `crates/codegen/xai-grok-shell/src/auth/xai_console.rs` — key store pattern
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` — `/limits` VM
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` — footer
- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs` — `/limits` dispatch
- User-guide `02-authentication.md`, `04-slash-commands.md`
