# Plan: Rate-limit identity failover

**Status:** implemented (approved recommended path, 2026-07-26)  
**Mode:** shipped — see `/tmp/grok-1000/grok-impl-summary-rate-limit-failover.md`  
**Join map (evidence):** `/tmp/grok-1000/grok-join-rate-limit-failover-map.md`  
**Related shipped work:** credit/spending dual-auth failover (session ↔ console keys)

On approve: copy to `.agents/plans/plan-rate-limit-failover.md`, pin process notes in `AGENTS.md` (no modal questionnaires; do not invent “out of scope”), seed `impl:*` leaves.

---

## Apology / process

You asked for **rate-limit failover**. An earlier answer treated plain-429 hop as
“out of scope” because older research defaulted **soft-429 hop = No**. That was
**assuming intent from stale docs**, not listening to a stated need.

Also: plan clarifications belong **in this plan / freeform chat** — **not** host
questionnaire modals (`/plan` skill hard rule 6; re-flagged this turn). Sorry for
the modal spam.

---

## Goal (stated)

When the **active identity** is **rate-limited**, failover to **another configured
xAI identity** instead of only sleeping on the same credential forever.

Credit/spending hop already exists. Rate-limit path does **not** hop today.

---

## Current behavior (verified)

| Failure | Identity hop? | Same identity? |
|---------|---------------|----------------|
| Credit exhausted (402; credit-worded 403/429/400) | **Yes** if failover list remains | After hop |
| **Plain HTTP 429** (no credit wording) | **No** | **Yes** — sleep + shared cooldown; default effectively unlimited retries |
| Bare 401 / non-credit 403 | No | Other paths |

**Gate that blocks rate-limit hop:**  
`apply_retry_decision` in  
`crates/codegen/xai-grok-sampler/src/actor/request_task.rs`  
calls `try_rotate_to_failover_key` **only** when `err.is_credit_exhausted()`.

Classification (`xai-grok-sampling-types`):

- `is_rate_limited` = status **429 only** (body ignored).
- `is_credit_exhausted` = 402 always, or 403/429/400 **with** credit wording.
- Credit-worded 429 already hops via the **credit** path (hop runs before soft throttle).

Plain 429 today: no hop → `RetryWithBackoff` → `sleep_for_retry` + `grok-rate-limit`
shared store (base URL + key fingerprint) → retry **same** identity.

**Reusable:** FIFO `failover_api_keys`, `try_rotate_to_failover_key` (client rebuild,
dual-host session↔key, bearer stash), fingerprints, preemptive-skip pattern, toast
plumbing.

**Not reusable as-is:** 1h credit “exhausted” memo meaning, “credit exhausted” toast
copy, config comments saying credit-only, tests assuming plain 429 never hops.

---

## Recommended path (edit these if wrong)

Not frozen until you approve or revise in chat.

| Decision | Recommendation | Why |
|----------|----------------|-----|
| Trigger | Hop on plain `is_rate_limited()` (HTTP 429) when another identity remains | Stated need |
| vs sleep | **Hop first** if failover available; do not infinite same-key wait | Pain of today |
| Identity set | **Same as credit failover** (session + console FIFO, `preferred_method`) | One model; reuse rotate |
| Avoidance after hop | **Temporary** cooldown (Retry-After / shared rate-limit store), **not** 1h credit memo | Throttle ≠ out of money |
| Return to primary | Prefer primary again when cool | Don’t sticky forever on backup |
| Default | **On when failover list non-empty** | Same as credit hop availability |
| Toast | Distinct **rate-limit switched identity** (allow-listed) | Don’t claim credits |
| OpenRouter | Same hop gate if shared path; verify at implement | Don’t invent second policy blindly |
| Kill-switch | `disable_api_key_auth` still clears key failover | Unchanged |

---

## Open questions (answer freeform — no modals)

1. **Hop timing** — Immediate on first plain 429 if failover remains? Or after N same-key retries / only if Retry-After &gt; T seconds?
2. **Identity set** — Full dual-auth (session ↔ keys), keys-only, or also third-party/OpenRouter multi-key?
3. **Cooldown / memo** — Header-driven temporary skip only? Short separate TTL? Never put rate-limited identities in the **credit** 1h exhausted map?
4. **Return-to-primary** — Sticky until new identity fails? Or resume preferred primary when cool?
5. **Default on/off** — Always when multi-identity configured? Opt-in setting? Env kill-switch?
6. **Retry budget** — Shared across identities vs reset per hop? Keep unlimited 429 unless policy caps?
7. **503 / capacity** — Hop only on 429, or also capacity-style 503 if classified later?
8. **Multi-process** — Peer processes keep cooling the rate-limited fingerprint in `grok-rate-limit` while this process uses the next identity? (Recommend yes.)
9. **User visibility** — Preferred toast/docs wording?
10. **Optics** — Prior research worried soft-429 hop looks like limit evasion. Accept for multi-account power users, or gate behind explicit setting?

Reply e.g. “approve as recommended” or answer 1–10 in bullets.

---

## Implementation steps (after answers + approve)

Red/green TDD. Docs same turn. Leaf sizes 1|2.

### Step 1 — Red: plain-429 with failover must hop (size 2)

- Sampler/actor tests: multi-identity, plain 429 on primary → next identity used; credit still hops; bare 401 no hop.
- Prove current gate fails (red first).

### Step 2 — Green: gate + temporary cooldown (size 2)

- Extend `apply_retry_decision` (or helper) so rate-limit can call `try_rotate_to_failover_key` under agreed policy.
- Do **not** reuse credit 1h memo for temporary throttles unless you choose that.
- Distinct hop reason strings; green Step 1; credit tests stay green.

### Step 3 — Config / comments / opt-out (size 1)

- Stop documenting failover as credit-only.
- Optional setting/env if default-off or kill-switch chosen.
- `disable_api_key_auth` unchanged unless you say otherwise.

### Step 4 — Toast + user-guide + FORK/RESIDUAL (size 1)

- Allow-list rate-limit hop reason for pager toast.
- User-guide: rate-limit hop vs credit hop.
- FORK bullet; RESIDUAL honesty (reword any “credit-only” residual).

### Step 5 — Focused verify (size 1)

- Sampler unit/integration; hop toast unit if split.

---

## Critical files

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-sampler/src/actor/request_task.rs` | Gate, rotate, sleep/shared store |
| `crates/codegen/xai-grok-sampler/src/exhausted_identity.rs` | Credit memo — separate vs share |
| `crates/codegen/xai-grok-sampler/src/config.rs` | Failover docs / flags |
| `crates/codegen/xai-grok-sampler/src/retry.rs` | Rate-limit classify / backoff |
| `crates/codegen/xai-grok-sampling-types/src/error.rs` | `is_rate_limited` / `is_credit_exhausted` |
| `crates/codegen/grok-rate-limit/` | Shared cooldown |
| `crates/codegen/xai-grok-shell/src/agent/config.rs` | Resolve/stamp failover list |
| Pager hop toast + `02-authentication.md` / `11-custom-models.md` | UX + docs |
| `FORK.md`, `RESIDUAL.md` | Honesty |

---

## Risks

| Risk | Mitigation |
|------|------------|
| Burning every identity on a global org limit | Cooldown + hop; avoid infinite rotate without backoff if all 429 |
| Conflating credit-dead with temporary throttle | Separate memo/cooldown + toast copy |
| Retry-forever 429 surprises | Document; policy env already can cap |
| Stale “soft-429 hop = No” research | Update FORK/RESIDUAL on ship; this plan + code win |

---

## Verification (post-implement)

```bash
cargo test -p xai-grok-sampler --lib -- rate_limit failover
cargo test -p xai-grok-sampler --test test_actor -- rate_limit
```

Optional manual: two keys or session+key; force 429 on primary; confirm hop toast and work continues on secondary.

---

## Not in steps unless you add them

These are **not** dismissed forever — only **not scheduled** until you expand:

- Dual SuperGrok OAuth as first-class identities
- Durable `$GROK_HOME` exhausted memo
- Per-model rate-limit routing / model-aware order

If dual OAuth **is** part of what you need for rate-limit failover, say so and expand the plan.

---

## Approve / revise

- “Approve as recommended”
- Or answer open questions 1–10 in freeform bullets
- Or “revise: …” with concrete policy

No implement until explicit approve.
