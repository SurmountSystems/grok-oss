# C4 ticket addendum: free SuperGrok period still ~6% (2026-08-07)

**Audience:** operator paste into xAI billing / support ticket
**Not a product code fix.** Client Design A is OK; this documents **server absorption still unproven**.
**Companion:** `.agents/joins/c4-supergrok-debit-evidence-package-2026-08-02.md`,
`.agents/reports/how-to-fix-c4-free-period-debit-2026-08-07.md`

---

## One-line ask

Under heavy SuperGrok **session** load (cli-chat-proxy / SessionToken, console ApiKey **not** live), free SuperGrok period used % stays near **6%** while team Grok Build / OAuth settlement dollars keep climbing. Please confirm debit path and why `creditUsagePercent` does not step with that traffic.

---

## Live snapshot (post `just install`, evening 2026-08-07)

| Field | Value |
|-------|--------|
| Client binary | `grok-oss 0.2.111 (c87f66a61d94) [stable]` |
| Live sampling | SuperGrok session (**business**) |
| **activeDriver** | **`supergrok_free_period`** (client Design A correct) |
| Free SuperGrok period used | **6.0%** business + **6.0%** personal (shared pool) |
| Free SuperGrok period remaining | **94%** |
| Included source | `live_poll` both principals; `pollSucceeded: true` |
| Next reset | August 10, 19:25 (weekly) |
| SuperGrok $ extras | **$100.29** (flat side meter; not live driver) |
| console.isLive | **false** |
| console.keyAvailable | true (present, not primary) |
| Team prepaid | **$340** |
| Team postpaid period total | **$1018.63** |
| **Team postpaid OAuth / Grok Build class** | **$1012.83** (still climbing vs free period flat) |
| Team postpaid API class | $5.80 |
| Team usage series OAuth class (window ~2026-08-02..09) | ~**$855.37** (Grok Build OAuth grok-4.5-build) |

Command used:

```bash
~/.cargo/bin/grok-oss limits --json
```

---

## Contrast with 2026-08-02 evidence package

| Window | Free period | Team OAuth / Build settlement | SuperGrok primary? |
|--------|-------------|-------------------------------|--------------------|
| 2026-08-02 (prior period dogfood) | Stuck ~**65%** for hours (weak 65→66); Build productUsage ~54% flat | Team Usage $ moved under session load | Yes, SuperGrok session |
| **2026-08-07 (this period after reset)** | Stuck ~**6%** all day of dogfood / implement / verify | OAuth class ~**$1013**; series ~$855 Build | Yes; `activeDriver=supergrok_free_period` |

**Reading:** this is a **new period** stuck-low pattern (not replaying the 65% tape). After weekly reset, free period has headroom but does not absorb SuperGrok session / Build-class traffic into the included % poll. Client chrome correctly shows **6%** free-period-first.

---

## Client status (so xAI does not chase a status-bar bug)

1. Product **limits-before-credits / Design A** unit filters re-verified green 2026-08-07 (including smoking gun: free period 6% + sticky memo + team prepaid → compact **`6%`**, not `console · $340`).
2. Live: free SuperGrok period is the **active driver**; console key is **not** live.
3. Client **does not invent** a higher free-period % and will not hop to console to "fix" free period while headroom remains.
4. Honesty notes already surface: SuperGrok included % is the billing poll reading, not proof of included-limit burn; team OAuth $ can move without free-period proof.

---

## Exact questions (short form for ticket)

- **Q1:** Intended debit path for SessionToken → cli-chat-proxy on business SuperGrok Heavy / Grok Build traffic.
- **Q2:** Why `creditUsagePercent` / free SuperGrok period used can stay flat (~6% this period; ~65% prior period) under multi-hour load while live poll succeeds.
- **Q3:** Why Grok Build productUsage (when on wire) can stay flat under Build-heavy session traffic.
- **Q4:** Confirm SuperGrok $ extras should not move until free period is exhausted (client already after-burns only at ≥100%).
- **Q5:** Is team OAuth / Grok Build Usage settlement supposed to reduce free SuperGrok period % on the same poll, or intentionally parallel only?
- **Q6:** Authoritative fields for included weekly % and Build product usage %; any known delay / 1% quantization / wrong pool.

---

## Suggested title (updated)

`SuperGrok free period stuck ~6% (post-reset 2026-08-07) under cli-chat-proxy / SuperGrok primary while team OAuth Grok Build class ~$1013 moves; prior period stuck ~65% (2026-08-02 package)`

---

## Operator action

Paste this addendum + the 2026-08-02 evidence package into the human xAI channel. **Agents cannot close C4.** No further client invent of free-period debit.
