# Team settlement chrome vs free SuperGrok period limits (2026-08-09)

## Operator question (first answer)

**Are we paying team prepaid right now?**

**No for the active client spend path.** In the screenshot state:

| Surface | What it means |
|---------|----------------|
| Top: `free SuperGrok period · 25% · … behind linear burn` | **Active spend-order driver** is free SuperGrok period limits. SuperGrok OAuth session is primary (not console API key). |
| Bottom (before fix): `Team settlement: prepaid $340 · …` | **Secondary team wallet chrome only**: Management **team prepaid remaining** $340. Not the Design A compact driver. Not "console is primary." |

So: free SuperGrok period is primary for client spend-order chrome. The footer was **not** claiming the live sampling principal was the console key. The bug was **wording honesty**: `Team settlement: prepaid $340` still read as "we are paying with team prepaid / credits now."

**Also true (deeper honesty, not this chrome bug):** SuperGrok session traffic can still move team Billing Credits / Grok Build class on the **server settlement** side without free SuperGrok period used % moving. That is documented as intent vs settlement (`NOTE_ACTIVE_DRIVER_IS_INTENT_NOT_SETTLEMENT`). This ticket only fixes the footer string so operators do not read secondary meters as active pay.

---

## 1. What the string meant in code (before)

### Construction path

File: `crates/codegen/xai-grok-pager/src/views/credit_bar.rs`

1. SuperGrok live footer path:
   `usage_warning_for_session_with_identity_principal_gap_and_postpaid`
   → when `sampling_identity` is SuperGrok session and `usage_visible`
2. Merge:
   `merge_supergrok_warning_with_team_meters`
   = optional SuperGrok % / extras warning **plus** secondary team meters
3. Team fragment:
   `format_team_settlement_footer(...)`
   - known cents → body part `prepaid $N` (was)
   - optional Grok Build class → `Grok Build class $M`
   - prefix was `Team settlement:`

Example full footer when mid free-period (no SuperGrok warning alone):

```text
Team settlement: prepaid $340 · Grok Build class $M
```

Cold management (key present, cents unknown):

```text
Team settlement: loading team prepaid...
```

### Secondary vs active

| Chrome | Function | Role |
|--------|----------|------|
| Compact status (top) | `compact_meter_text_for_live_identity` | Spend-order **intent**: free SuperGrok period · N% while included &lt; 100% |
| `activeDriver` | `active_spend_driver` | Same rule; wire `supergrok_free_period` while free period has room |
| Footer team $ | `format_team_settlement_footer` | Secondary Management team meters; **never** replaces compact free SuperGrok period while it has room |

Product **already** intended secondary settlement chrome (Work C / 2026-08-09 labels). Spend path was **not** "console key primary while free SuperGrok period has room." The operator misread was the **label**, not a flip of live principal to team prepaid.

---

## 2. Active driver / spend path for that chrome state

When free SuperGrok period is **25% used** and team prepaid remaining is **$340**:

| Signal | Expected value |
|--------|----------------|
| Live sampling | SuperGrok session (OAuth primary) |
| Compact status | `free SuperGrok period · 25%` (+ optional linear-burn chip) |
| `active_spend_driver(...)` | `ActiveSpendDriver::SuperGrokFreePeriod` |
| `limits --json` `activeDriver` | `"supergrok_free_period"` |
| `activeDriverLabel` | contains `Active: free SuperGrok period` |
| Console key primary? | **No** (would paint `console · $N` compact instead) |
| Team prepaid $340 | Footer secondary only; not compact; not `activeDriver` |

Extras on the SuperGrok account do **not** flip the driver while free period has headroom (same `active_spend_driver` rule).

---

## 3. Honesty decision

| Question | Decision |
|----------|----------|
| Was product settling client path on team prepaid while free SuperGrok period had room? | **No** for Design A / live principal / compact / `activeDriver`. Not a limits-first **spend-path** flip bug in this chrome. |
| Was chrome wording dishonest enough to fix? | **Yes.** `Team settlement: prepaid` was read as active pay. Relabel. |
| Both? | Relabel only for this ticket; residual still tracks real settlement-vs-intent dogfood separately. |

### Fix (product)

Prefix and body now make secondary status unmissable:

```text
not the active spend path: team prepaid remaining $340
```

With Grok Build class:

```text
not the active spend path: team prepaid remaining $340 · Grok Build class $M
```

Cold:

```text
not the active spend path: loading team prepaid...
```

Constants:

- `TEAM_SECONDARY_METERS_LABEL = "not the active spend path"`
- Deprecated alias `TEAM_SETTLEMENT_LABEL` → same string (do not use; misread history)

"Team prepaid remaining" matches `/limits` vocabulary. Prefix states the meter is **not** the active spend path (complete American English; not bare "settlement" jargon).

---

## 4. TDD

**Named contract (plain language):**

When SuperGrok is live, free SuperGrok period is 25% used, and team prepaid remaining is $340:

1. Compact status is free SuperGrok period · 25%
2. `activeDriver` is free SuperGrok period (`supergrok_free_period`)
3. Footer is exactly
   `not the active spend path: team prepaid remaining $340`
4. Footer must **not** contain `Team settlement` or look like console primary

**Test:** `operator_screenshot_free_period_25_team_prepaid_340_not_active_pay`
(plus updated Work C / footer tests that reject `team settlement` and require the new prefix)

**Red → green:** expectations updated to the operator-facing honesty contract; product string changed to match. Prior green tests that required `Team settlement` would fail under the new contract (intentional contract change).

---

## 5. Verify

```text
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # exit 0
cargo test -p xai-grok-pager --lib views::credit_bar::tests
# 88 passed (includes operator_screenshot_… and Work C suite)
```

No git commit / stage / push.

---

## 6. Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Relabel footer; tests; operator screenshot contract |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | User-facing strings |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` | Footer docs |
| `FORK.md` | Shipped footer wording |
| `RESIDUAL.md` | Open honesty note + report link |
| `.agents/reports/impl-team-settlement-chrome-vs-limits-2026-08-09.md` | This report |

---

## 7. Not claimed fixed here

- Server free SuperGrok period debit / flat-poll (C4)
- Machine `payingMeter` that proves which wallet debited last request
- Dogfood that team prepaid remaining dollars still move under SuperGrok while free period % is flat (tracking gap remains open residual)
