# Wishlist — btw copy + follow-up; free-text “plan” ≠ plan mode

Date: 2026-07-26 · status: **B1 + B2 + B3 shipped**  
Class: D2 research + acceptance for red/green TDD.

## Intent

1. **btw dialog — Copy:** button (and/or key) copies the **entire** btw content.
   When multi-turn exists, copy the **whole conversation**.
2. **btw dialog — Follow-up:** button to ask **another question in the same
   btw context** (not a brand-new one-off `/btw`).
3. **Plan entry:** putting the word “plan” in a normal prompt must **not**
   immediately shift into plan mode.

## Findings (code)

Explore join: `/tmp/grok-1000/grok-explore-btw-plan-trigger.md`

| Area | Today |
|------|--------|
| btw UI | Inline panel `views/btw_overlay.rs` — Esc/close, scroll, drag-select; **no** copy-all or follow-up chrome |
| btw turns | **Single-shot only** — new `btw-UUID` per call; shell says “no follow-up turns”; `BtwEntry` = one Q/A in `btw_history.jsonl` |
| Plan mode entry | `/plan`, settings, or agent tool `enter_plan_mode` — **no client free-text keyword** |
| “plan” false positive | Tool description (“when the user asks you to write a plan”) + read-only + auto-allowlist → model enters immediately |

## Acceptance

### B1 — Copy entire btw

- Done panel has visible **Copy** (and optional key, e.g. `y` when focused).
- Clipboard gets **full** plain text: question + complete answer (not only
  viewport). After multi-turn ships: all turns, ordered.
- Red/green: pager tests on `btw_overlay` / input dispatch (extend
  selection/copy tests that already cover scrolled-out lines).

### B2 — Follow-up in same btw — **shipped**

- Done panel has **Ask again** / follow-up composer (`[a]`) that keeps prior
  Q/A visible and reuses the same `btw_session_id`.
- Shell `handle_side_question` sends prior turns; relax “one-off / no
  follow-up” for continuation (`helpers/side_question.rs`).
- Copy (B1) includes full thread when multi-turn.
- History: multi-entry `btw_history.jsonl` — one `BtwEntry` per turn, shared
  `btw_session_id`, ordered by `asked_at`.
- Red/green: shell session-id reuse + prior-turn request items; pager
  follow-up effect + overlay `full_copy_text` thread.

### B3 — Incidental “plan” does not force plan mode

- **Not** a client ban-list on the word “plan”.
- Tighten `enter_plan_mode` tool description: enter only for **explicit**
  plan-mode intent (`/plan`, “enter plan mode”, “write an implementation
  plan and wait for approval”) — not casual “we should plan to…”.
- Optional harder gate: drop `enter_plan_mode` from auto-mode allowlist /
  require permission even if read-only.
- Explicit `/plan` + settings still work.
- Red/green: description contract test; allowlist tests if gate changes;
  explicit `/plan` still activates.

## Ship order (recommended)

| Slice | Effort | Note |
|-------|--------|------|
| **B1** copy-all | S | Reuse selection/clipboard plumbing |
| **B3** plan false entry | S (desc) / S+ (gate) | Orthogonal to softer-park toast residual |
| **B2** multi-turn follow-up | M | Real conversation state end-to-end |

## Related residual

- Soft plan **modal park** toast: `feat:plan-modal-softer-park` (after
  unexpected entry — separate from preventing false entry).
- Plan: [`.agents/plans/plan-btw-copy-followup-plan-trigger.md`](../../../.agents/plans/plan-btw-copy-followup-plan-trigger.md)

## Out of scope here

- Full non-modal plan redesign
- Theme hide-header / DOGE
- Agent commit/push
