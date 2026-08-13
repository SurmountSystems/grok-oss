# Still shows `6%` after limits-before-credits?

**Date:** 2026-08-07
**Mode:** read-only (code + prior reports; no host keyring probes)

## Verdict

**Intentional product chrome, not a remaining bug.**
`6%` is free SuperGrok period **used** (wire `creditUsagePercent` / included). With SuperGrok live and free period under 100%, compact status is supposed to stay bare free-period `%` even when SuperGrok `$` extras and console team prepaid both exist on the account.

What the limits-before-credits work fixed was the opposite smoke: sticky exhaust memo painting **`console · $340`** while free period still had room. Healthy dogfood with ~6% used and room left is **`6%`**.

---

## 1. How compact status formats the `%`

| Layer | Path | Role |
|-------|------|------|
| Pure meter string | `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Builds the text the status bar shows |
| Sticky identity | same file: `status_sampling_identity_for_compact_meter` | Blocks false console pin when free period known and used &lt; 100% |
| Active driver (observe) | same file: `active_spend_driver` → wire `activeDriver` | Same Design A order for `/limits` and `limits --json` |
| Paint wire | `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Status bar + footer use helper + meter text |

**Exact string path (SuperGrok live, poll OK, included known, used &lt; 100%):**

1. `status_sampling_identity_for_compact_meter(...)` → `SuperGrokSession` when free period has headroom (even if exhaust memo claims out + console ready).
2. `credit_bar_line_for_session` → `compact_meter_text_for_live_identity` / `_with_active_poll`.
3. Free-period branch: `format!("{included_usage_pct:.0}%")` → e.g. **`6%`**, **`42%`**.

**Design A compact rules (active meter only):**

| Live path | Compact text |
|-----------|----------------|
| Console live | `console · $N` or honest gap (never bare SuperGrok `%`) |
| SuperGrok, free period &lt; 100% | `{pct:.0}%` (e.g. **`6%`**) — **ignores** SuperGrok extras $ and console prepaid for this string |
| SuperGrok, free period ≥ 100%, extras &gt; 0 | `SuperGrok extras · $N` |
| SuperGrok, free period ≥ 100%, no extras | `100%` |
| SuperGrok cold / active AuthFailed | `...%` |

Named unit proof of the dogfood shape:
`compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars` (6% + memo out + team prepaid → **`6%`**, not `console · $340`).

---

## 2. What prior reports say about `6%`

| Report | Takeaway |
|--------|----------|
| `.agents/reports/live-limits-vs-credits-check-2026-08-07.md` | Live dogfood: SuperGrok session, free period **6% used**, room left; compact should be **`6%`**. Not the "on credits while free-period chrome" bug. Extras ~$100.29 and console ~$340 are side meters. |
| `.agents/reports/verify-compact-status-chrome-2026-08-07.md` | Pure helper: included 6.0 → string **`6%`**. Healthy free-period vs cold `...%` table. |
| `.agents/reports/impl-limits-before-credits-2026-08-07.md` | Smoking-gun fix was sticky pin. Dogfood checklist item 2: free period still ~6%, SuperGrok live → compact **`6%`**, not `console · $340`. `activeDriver` = `supergrok_free_period`. |
| `.agents/reports/plan-limits-before-credits-inventory-2026-08-07.md` | Design A: SuperGrok live + free period &lt; 100% → free-period used `%`. Multi-meter `/limits` can still list extras and team $ next to compact `6%` (easy to misread as "on credits"). Settlement: team Grok Build class can climb while free period stays ~6% (server dual-bill honesty, not client wrong primary). |
| `.agents/reports/impl-dual-supergrok-billing-honesty-2026-08-07.md` | Dual-principal honesty; not a reason to hide free-period `%` when active SuperGrok poll is healthy. |
| `.agents/reports/bug-limits-chrome-when-on-credits-2026-08-07.md` | Real bug class was free-period chrome **when spend was already credits** (console live, or free period full + extras after-burner). **Not** "still 6% with room." |

---

## 3. Intentional product vs remaining bug

| Question | Answer |
|----------|--------|
| Is compact **`6%`** wrong while free period has room and SuperGrok is live? | **No.** That is Design A free-period-first chrome. |
| Does console prepaid / SuperGrok extras still available mean chrome should switch to `$`? | **No.** Those meters wait until free period is full (extras) or live sampling is console. |
| What would be a remaining chrome bug? | Still painting `console · $N` or `SuperGrok extras · $N` while free period used &lt; 100% and SuperGrok live; or painting healthy `N%` when active poll is AuthFailed (should be `...%`). |
| Separate honesty residual (not this chrome string) | **C4:** server free-period debit may lag under load (team settlement $ can move while free period % stays flat). Client must not invent free-period burn. |

**If operator expected dollars:** that expectation is multi-meter reading (`/limits` lists extras and team $ while compact correctly stays free-period used %). Confirm with:

```bash
grok-oss limits --json
# expect: liveSampling SuperGrok; activeDriver = supergrok_free_period;
# included / free period used ~6; console.isLive false
```

**If free period is actually full in billing but UI still says 6%:** that would be a poll / cache / principal-fill honesty issue, not Design A policy. Live reports for this dogfood window said included really was ~6%.

---

## Bottom line

**Still showing `6%` after the fix is success for free-period-first chrome**, not a regression. The fix stopped false console dollar paint; it did not (and should not) replace free-period used `%` with credits chrome while free SuperGrok period still has headroom.
