# Verify: still ~6% free SuperGrok period — are we using limits?

**Date:** 2026-08-08
**Binary:** `grok-oss 0.2.111 (c87f66a61d94) [stable]` (`~/.cargo/bin/grok-oss`)
**Meters kept distinct:** free SuperGrok period % ≠ SuperGrok $ extras ≠ console team prepaid $ ≠ team postpaid OAuth / Grok Build class $ ≠ team postpaid API class $ ≠ team default credits

---

## Short status (operator)

| Field | Value |
|-------|--------|
| **Client path OK?** | **Yes** — free SuperGrok period first; console not live |
| Free SuperGrok period used % | **6.0%** business + personal (shared pool), 94% remaining |
| `console.isLive` | **false** |
| Team postpaid OAuth / Grok Build class | **~$1019.56 → $1019.85** (still climbing under SuperGrok session) |
| SuperGrok $ extras | **$100.29** (flat; side meter, not live driver) |
| `activeDriver` | `supergrok_free_period` |
| `liveSampling` / role | `supergrok_session` / `business` |
| `flatPollUnprovenDebit` | **true** |

**Not a client rank fail.** “6% should climb” is **C4 server absorption** (free SuperGrok period debit under SuperGrok session load is unproven). Product does not invent free-period debit % and does not hop console to “fix” free period.

---

## 1. Live proof of path (limits-before-credits client)

### Commands

```bash
~/.cargo/bin/grok-oss --version
~/.cargo/bin/grok-oss limits --json
~/.cargo/bin/grok-oss limits
```

No separate multipoll binary; two `limits --json` polls ~8s apart recorded below.

### Snapshot (primary poll, 2026-08-08)

| Field | Evidence |
|-------|----------|
| `activeDriver` | `supergrok_free_period` (“Active: free SuperGrok period”) |
| `liveSampling` | `supergrok_session` |
| `livePrincipalRole` | `business` |
| Free period used % (business) | **6.0** (`includedSource: live_poll`, `pollSucceeded: true`) |
| Free period used % (personal) | **6.0** (same; `sharedUnifiedPool: true`) |
| Free period remaining | 94% both; next reset August 10, 19:25 |
| SuperGrok $ extras | **$100.29** both principals (`dollarExtrasObserved: true`) |
| `console.keyAvailable` | true |
| `console.isLive` | **false** |
| Console team prepaid | $340.00 |
| Team postpaid OAuth / Grok Build class | **$1019.56** (primary) then multipoll climbed |
| Team postpaid API class | $5.80 |
| Team postpaid period total | ~$1025.36 |
| Team usage series OAuth class | ~$862.10 (window 2026-08-02 .. 2026-08-09) |
| `flatPollUnprovenDebit` | **true** |
| `flatPollObservedBuild` | false |
| `flatPollObservedExtras` | true |

Honesty notes already on wire (product): included % is poll reading not proof of included-limit burn; SuperGrok session can move team OAuth / Grok Build $ without free period moving; client does not invent free-period debit.

### Multipoll sample A → B (~8s)

| Sample | Free period % | SuperGrok $ extras | Team OAuth class $ | Usage-series OAuth $ | `console.isLive` | `activeDriver` |
|--------|---------------|--------------------|--------------------|----------------------|------------------|----------------|
| A | 6.0 / 6.0 | 100.29 | 1019.67 | 862.20 | false | `supergrok_free_period` |
| B | 6.0 / 6.0 | 100.29 | **1019.85** | **862.38** | false | `supergrok_free_period` |

Free period flat; team OAuth class and usage-series OAuth class both stepped up under SuperGrok session while console API key stayed not live. Same pattern as earlier multipoll today (poll A/B files at `c4-limits-poll-*2026-08-08T055844Z.json`: free 6.0% flat, OAuth $1013.35 → $1013.77).

### Earlier same-day multipoll (context)

From `.agents/reports/c4-limits-poll-a-2026-08-08T055844Z.json`: free 6.0%, extras $100.29, `console.isLive` false, team OAuth class **$1013.35**, usage-series OAuth **$855.88**.
Now (hours later): free still **6.0%**, extras still **$100.29**, OAuth class **~$1019.85**, usage-series OAuth **~$862**. Team settlement climbed; free SuperGrok period did not.

---

## 2. Unit re-verify Design A (smoking guns)

All **passed** (2026-08-08):

```text
cargo test -p xai-grok-pager --lib -- \
  compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars \
  active_driver_free_period_headroom_even_with_extras_and_team_prepaid \
  status_bar_free_period_headroom_not_console_prepaid_dollars
# 3 passed

cargo test -p xai-grok-shell --lib -- \
  auto_order_omits_console_while_any_supergrok_included_headroom \
  auto_with_included_headroom_still_omits_console
# 2 passed
```

Contracts covered: compact chrome shows free-period **%** (not console dollars) while free period has headroom; active driver stays free period even with SuperGrok $ extras and team prepaid present; auto order **omits console** while any SuperGrok included headroom remains.

---

## 3. Honest reading for operator

| Claim | Evidence |
|-------|----------|
| Client using free SuperGrok period first? | **Yes.** `activeDriver = supergrok_free_period`, `console.isLive = false`, live sampling SuperGrok session business. Design A unit tests green. |
| Free period absorbing dogfood burn? | **Unproven on the included % meter.** Free period still ~6% while team OAuth / Grok Build class $ keeps climbing under SuperGrok session. That is **C4 server-side** (included debit unproven), **not** client rank fail. |
| What “using limits” means vs “6% should climb” | **Using limits (client):** stay on SuperGrok session, prefer free SuperGrok period as active driver, omit console ApiKey from primary while free period has headroom, show honest poll % (6%), do not invent higher free-period debit, do not hop console to “fix” free period. **6% should climb (server):** xAI billing should step `creditUsagePercent` (included free SuperGrok period) when SuperGrok session traffic runs through cli-chat-proxy. Flat multipolls with rising team OAuth $ mean settlement moved without proving free-period absorption. |

### Why 6% is “absurd” but still correct chrome

- Heavy dogfood + local calculated spend in the thousands of dollars can coexist with free SuperGrok period stuck at 6% if **server debit of the included pool is missing or coarse**.
- Product chrome is **honest about the poll**, not a progress bar of “how hard we worked.”
- Linear-burn note on human limits (“55% behind linear burn…”) is time-share context only; it does not invent a higher used %.

### Forbidden conclusions (not taken)

- Inventing free SuperGrok period debit above 6%.
- Claiming client hop to console would fix free period (console not live by design while headroom remains).
- Claiming C4 fixed by client path work.

---

## 4. Client path wrong? (console live while free period headroom)

**No.** Gate for product bug: `console.isLive == true` **and** free SuperGrok period remaining > 0 with free-period-first intent. Observed: console **not** live, free period **94% remaining**, driver free period. **No red/green product fix this turn.**

---

## Related artifacts

- C4 paste-ready ticket: [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](c4-xai-ticket-paste-ready-2026-08-07.md) (addendum for 2026-08-08 later multipoll)
- Prior multipoll JSON: `c4-limits-poll-a-2026-08-08T055844Z.json`, `c4-limits-poll-b-2026-08-08T055844Z.json`
- Prior doubt writeup: [`.agents/reports/doubt-free-period-stuck-6pct-2026-08-07.md`](doubt-free-period-stuck-6pct-2026-08-07.md)
- Limits-before-credits impl: [`.agents/reports/impl-limits-before-credits-2026-08-07.md`](impl-limits-before-credits-2026-08-07.md)
