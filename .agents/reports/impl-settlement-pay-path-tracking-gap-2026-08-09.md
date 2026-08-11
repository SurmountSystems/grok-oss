# Settlement pay-path tracking gap (console team prepaid paid for dogfood)

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Meters kept distinct:** free SuperGrok period used % ≠ SuperGrok dollar credits ≠ console team prepaid remaining ≠ team postpaid OAuth / Grok Build class ≠ team postpaid API class ≠ console API credits.

---

## Operator ask (essence)

Screenshot: console.x.ai Surmount team Billing, **Credits ~$343.73**, auto top-up disabled (team `61fab250-…`).

Free SuperGrok period / `activeDriver` talk missed that **console team credits / team settlement was paying for dogfood work**. If the product is not tracking or surfacing that, that is also an **implementation gap**.

Live context already measured (same day):

| Field | Value |
|-------|--------|
| `activeDriver` | `supergrok_free_period` |
| `liveSampling` | `supergrok_session` (personal) |
| `console.isLive` | `false` |
| free SuperGrok period used % | ~9% |
| SuperGrok dollar credits | ~$100.29 |
| team prepaid remaining | ~$340 (matches screenshot Credits) |
| team postpaid OAuth class | ~$1163 |
| `flatPollUnprovenDebit` | `true` |

---

## Gap name (plain English)

**Settlement pay-path tracking gap:** chrome and `activeDriver` say free SuperGrok period is the active spend driver, while real dogfood burn can still settle on **team postpaid OAuth / Grok Build class** and can change **console team prepaid remaining** (team Billing Credits wallet) without free SuperGrok period used % moving and without the console API key being live.

This is **not** "we only track xAI free SuperGrok period." The client already tracks team meters. The gap is **intent chrome vs who settles**, and weak primary honesty that team prepaid remaining is a first-class paying wallet under SuperGrok session.

---

## What the product tracks today (code map)

### Free SuperGrok period used %

| Surface | Where |
|---------|--------|
| SuperGrok billing poll `creditUsagePercent` / included rows | shell billing + poll history |
| `limits --json` `supergrok.principals[].includedUsedPct` | `limits_cmd.rs` |
| Human `/limits` SuperGrok section | `limits_snapshot.rs` |
| Compact status free SuperGrok period `%` (Design A) | `credit_bar.rs` / status render |
| `flatPollUnprovenDebit` multi-poll flat window | process + durable poll history |

**Honesty:** included % is a billing poll reading, not invent of free SuperGrok period debit. Flat note when multi-poll window stays flat.

### SuperGrok dollar credits

| Surface | Where |
|---------|--------|
| Session billing prepaidBalance / extras | SuperGrok billing poll |
| `limits --json` `dollarExtrasUsd` | `limits_cmd.rs` |
| Human SuperGrok dollar extras line | `limits_snapshot.rs` |
| After-burner when free SuperGrok period full | `ActiveSpendDriver::SuperGrokExtras` |

Distinct from console team prepaid remaining.

### Console team prepaid remaining (screenshot Credits $)

| Surface | Where |
|---------|--------|
| Management prepaid meter (cents) | shell `fetch_console_team_prepaid_*` |
| `limits --json` `console.teamPrepaidUsd` / `teamPrepaidGap` | `ConsoleCliSection` |
| Human Console line (after this fix: **Team prepaid remaining**) | `format_console` |
| Footer chip under SuperGrok live when known | `merge_supergrok_warning_with_team_meters` |
| Process cache TTL lag note | `NOTE` / `note_console_team_prepaid_may_lag` |

This is a **remaining wallet** reading (console team Billing Credits), not free SuperGrok period %, not SuperGrok dollar credits, not postpaid period total.

### Team postpaid OAuth / Grok Build class

| Surface | Where |
|---------|--------|
| Management postpaid preview OAuth class | shell management postpaid |
| `limits --json` `console.teamPostpaidOauthClassUsd` | `ConsoleCliSection` |
| Human **Team postpaid OAuth / Grok Build class** (P1 near top of Console) | `format_console` |
| Footer `team Grok Build class: $N` | `team_grok_build_class_footer_chip` |
| C6 honesty note when OAuth dominates under SuperGrok | `NOTE_SESSION_CAN_MOVE_TEAM_USAGE_DOLLARS` |

Primary dogfood settlement proof for SuperGrok session traffic that moves team Usage dollars.

### Team postpaid API class

| Surface | Where |
|---------|--------|
| `console.teamPostpaidApiClassUsd` | postpaid preview API class |
| Human **Team postpaid API class** | `format_console` |
| Usage series API class when warm | Management analytics series |

Console API key spend class; distinct from OAuth / Grok Build class.

### `activeDriver` / `liveSampling` / `console.isLive`

| Field | Meaning in product |
|-------|---------------------|
| `liveSampling` | Next-request / live sampling principal: `supergrok_session` or `console_key` |
| `console.isLive` | True only when live sampling is the console key |
| `activeDriver` | Design A **spend-order intent chrome**: `supergrok_free_period` \| `supergrok_extras` \| `console_key` |

Code comment (pre-existing): team prepaid and team Grok Build settlement are **never** the `activeDriver` label. `active_spend_driver` ignores team prepaid remaining and OAuth class dollars while free SuperGrok period has headroom.

---

## Can SuperGrok session still settle on team prepaid / team OAuth while free SuperGrok period chrome is active?

**Yes.** Path evidence:

1. **Client design:** `active_spend_driver` only looks at live identity + free SuperGrok period % + SuperGrok extras cents. Team prepaid remaining and team OAuth class never flip `activeDriver` while free SuperGrok period has headroom (`credit_bar.rs`).

2. **Live dogfood:** `liveSampling=supergrok_session`, `console.isLive=false`, `activeDriver=supergrok_free_period`, free SuperGrok period flat/low, while team postpaid OAuth class is large (~$1163) and team prepaid remaining matches Billing Credits (~$340). `flatPollUnprovenDebit=true`.

3. **Prior client path bug (2026-08-08):** sticky Team JWT on SessionToken while chrome ranked free SuperGrok period first could climb team OAuth settlement while free SuperGrok period % stayed flat. Align fix shipped; free SuperGrok period debit still unproven on server when flat after path proof (C4).

4. **Honesty already partially named OAuth:** C6 + flat+settlement notes. They did **not** clearly say Active is intent chrome, or name team prepaid remaining / Billing Credits as a first-class tracked wallet under SuperGrok session.

**What we do not claim without deltas:** that every dollar of dogfood came only from prepaid remaining vs only from postpaid OAuth class. Both team surfaces are settlement-related; prepaid remaining is the Billing Credits wallet screenshot; OAuth class is the invoice/class spend that climbs under SuperGrok session. Product tracks both when a management key is set.

---

## Implementation gap (before this slice)

| Tracked | Missed / weak |
|---------|----------------|
| All five money meters as fields | Primary "who is paying" vs Active intent |
| C6 OAuth settlement under SuperGrok | Explicit "Active is not settlement proof" |
| Team prepaid as `Balance:` under Console | First-class **Team prepaid remaining** wording |
| Doctor dogfood block → OAuth / team Usage | Team prepaid remaining + intent chrome sentence |
| `activeDriver` free SuperGrok period first | Machine `payingMeter` that asserts last debit wallet |

Client invent of free SuperGrok period used % remains **banned**. Honesty and tracking only.

---

## Minimal fix slices (proposal)

| Slice | Status |
|-------|--------|
| **A. Honesty: Active is intent, not settlement** when SuperGrok live + team prepaid remaining and/or OAuth dominates | **Shipped** this report |
| **B. Human Console: Team prepaid remaining** label (not short Balance) | **Shipped** |
| **C. Doctor dogfood block:** name team prepaid remaining + intent vs settlement | **Shipped** |
| **D. JSON `payingMeter` / settlement delta history** | **Parked** (needs multi-sample prepaid/OAuth deltas; do not invent free SuperGrok period %) |
| **E. Compact status prefers settlement over free SuperGrok period %** | **Parked** (fights Design A free-period-first chrome on purpose; operator must approve) |

---

## What shipped (code)

### Files

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` | `NOTE_ACTIVE_DRIVER_IS_INTENT_NOT_SETTLEMENT`; emit under SuperGrok live when team prepaid remaining shown and/or OAuth dominates; doctor block names team prepaid remaining + intent chrome; unit tests |
| `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` | Human Console line **Team prepaid remaining**; comments |
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | `ActiveSpendDriver` docs: intent chrome, not settlement proof |
| `crates/codegen/xai-grok-pager/src/limits_cmd.rs` | `active_spend_driver_from_snapshot` comment; C6 test also checks intent-not-settlement note |
| `crates/codegen/xai-grok-pager/src/views/limits_modal.rs` | Test expectation for new prepaid line |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/status.rs` | Test expectations for new prepaid line |
| `RESIDUAL.md` | Open residual pin for gap + soft remainders |

### Note text (shipped)

> Note: Active free SuperGrok period (activeDriver) is the client spend-order driver and intent chrome, not proof of which wallet settles the bill. SuperGrok session traffic can still settle on team postpaid OAuth / Grok Build class and can change console team prepaid remaining (team Billing Credits) without free SuperGrok period used % moving and without the console API key being live. Product tracks team prepaid remaining and team OAuth class when a management key is set; it does not invent free SuperGrok period debit.

### Tests (green)

```bash
cargo test -p xai-grok-pager --lib -- limits_honesty
cargo test -p xai-grok-pager --lib -- prepaid Balance team_prepaid format_console format_limits human_output_names limits_json
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

Named new contract: `active_driver_intent_not_settlement_note_when_team_meters_under_supergrok`.

---

## Residual status

- **Open residual:** settlement pay-path tracking gap named in `RESIDUAL.md` under limits-first / Half B still-open list.
- **Honesty slice complete** for intent vs settlement + Team prepaid remaining label + doctor.
- **Soft remainders:** `payingMeter` wire field; prepaid/OAuth delta burn proof; compact status settlement-primary (needs explicit operator OK).

---

## Not done / do not claim

- Free SuperGrok period used % was debited client-side (banned invent).
- xAI always draws prepaid before postpaid (billing policy; not client-owned).
- Console API key was live during this dogfood (`console.isLive=false` measured).
- Full Design A chrome rewrite.
