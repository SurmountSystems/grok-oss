# Plan: Residual product campaign (open slices only)

**Status:** draft — approve to implement slices  
**Mode:** plan first; implement one open item at a time  
**Sources:** `RESIDUAL.md`, research under `doc/dev/research/`, child plans under `.agents/plans/`  
**Out of scope:** git land / commit / push / onto-join (operator-owned)

On approve: copy to `.agents/plans/plan-residual-campaign.md` and seed residual todos (merge-only).

---

## Standing engineering rules (every slice)

| Rule | Meaning |
|------|---------|
| **Document feature work** | Same turn as ship: user-visible docs where users configure/see it, short FORK bullet, RESIDUAL honesty, research note when non-obvious. |
| **Red/green TDD** | Failing tests first; implement until green; focused tests as automation feedback loop. |
| **Optional, default on** | New hygiene/behavior features: **on by default**, easy off (settings and/or env). |
| **Structured todos** | Real work size **1 or 2** only (split larger). Groupers unsized. **Merge** board updates — never casually wipe. Progress = bottom-level sized items when sizes present. |
| **One slice per implement** | Effort-1 → review to zero open issues → docs honesty → next. |
| **Requirements discipline** | Only stated user intent + verified code facts. Design choices left open until implement TDD or you decide — not a “not assumed” essay in the plan. |

---

## What “size” means

Optional **1 or 2** on real work todos. Bigger → split. Progress badge uses bottom-level (childless) sizes when any exist; else item counts.

---

## Problem

1. Open product residual is scattered without one campaign order.  
2. Model prose often includes em/en dashes, smart quotes/apostrophes, and invisible Unicode spaces that hurt ASCII-first workflows.  
3. Scrub must be strategically placed, default-on, and agent-overridable **only with approval**.  
4. Plan-mode feedback is incomplete: **selected line(s) must reach the agent**; **multi-line highlight** and **screenshot submit** are required so revise/explain does not guess.

---

## How we track work

| Place | Job |
|-------|-----|
| **`RESIDUAL.md`** | Open residual only; ship → lasting truth to FORK |
| **Session todo board** | Live progress; namespaced; size 1\|2; merge-only |
| **This plan + child plans** | Order + acceptance; status shipped vs open |

---

## Shipped baseline (do not re-implement)

Soft interject; plan CTAs + soft-park A; btw B1–B3; hide_header + DOGE theme; dual-auth D1–D3+S1; UDAX T0–T3; todo size/progress/merge product; Python→Rust A1–A3 + plan_validate + partial session_reader; usage.jsonl; ULID; trailing-ws; cleared_todos.

---

## Open product work (ordered)

### Wave 0 — ASCII scrub of AI output

**Your stated requirements:**

1. Automatically replace in **AI output**:
   - em dash → `--`
   - en dash → `-`
   - smart quotes → `"`
   - smart apostrophes → `'`
   - evil invisible Unicode spaces → ASCII-safe form (exact empty vs space: define in red tests when implementing)
2. **Strategically placed** choke point(s) — discover in code; prove with TDD (explore note is a map, not a frozen design: `/tmp/grok-1000/explore-ascii-scrub-sites.md`).
3. **Agent override** only **with approval**.
4. **Optional but on by default**.
5. **Document** meticulously.
6. **Red/green TDD**.

#### Slices

| Id | Size | Red first | Green |
|----|------|-----------|--------|
| **S0** | 1 | Unit tests for the replacements above | Pure scrub helper — **shipped** |
| **S1** | 2 | AI output path shows scrubbed text when on | Wire strategic placement + default on — **shipped** |
| **S2** | 1 | Flag off preserves original characters | Config/env off — **shipped** |
| **S3** | 2 | Override requires approval; reject keeps scrub on | Approval gate — **shipped** |
| **S4** | 1 | User-guide + FORK + RESIDUAL + research | Documentation + Appearance row — **shipped** |

---

### Wave 0b — Plan mode: selection + screenshots (product gap)

**Stated from this session (and prior soft-park residual):**

| Capability | Requirement |
|------------|-------------|
| **Single-line selection** | When the user has a plan line selected and revises/asks about “this line,” the agent receives path + line number + line text. Today: selection was not delivered → agent guessed. |
| **Multi-line highlight** | User can highlight **multiple** lines; agent receives the full range (start–end, text). |
| **Screenshot submit** | User can attach/submit screenshot(s) in plan mode so visual context rides with revise/clarify (same turn as the plan message). |

**Slices (TDD):**

| Id | Size | Red first | Green |
|----|------|-----------|--------|
| **P1** | 2 | Revise-with-selection payload includes selected line(s) | Selection → agent context — **shipped** |
| **P2** | 2 | Multi-line highlight range delivered intact | Multi-line selection — **shipped** |
| **P3** | 2 | Screenshot attach in plan mode reaches agent turn | Image attach path — **shipped** |
| **P4** | 1 | User-guide plan mode documents selection + screenshots | Docs — **shipped** |

Order relative to Wave 0: **can interleave**; P1 is high because it blocks accurate plan revise. Prefer P1 before more plan-text thrash.

Design note for park UX family: `doc/dev/research/plan-modal-softer-park-2026-07-26.md` (extend; do not invent park B/C/D here unless needed for these caps).

---

### Wave 1 — tools / agent experience

| Slice | Size | What |
|-------|------|------|
| **T4** | 2 | Optional agent tool JSON → TOON — **shipped** |
| **Codex SQLite** | 2 | session_reader Codex DB parity — **shipped** |
| **Cursor SQLite** | 2 | session_reader Cursor DB parity — **shipped** |
| **Skill text** | 1 | Drop py-only steers when intercept enough |
| **T5** | 2 | Densify large handoff JSON (later) |
| **T6** | 1 | Optional savings metrics log (later) |

Order: T4 → Codex → Cursor → T5/T6. One tools-heavy implement at a time.

---

### Wave 2 — optional UX polish (re-rank with you)

Plan soft-park B/C/D only if A still jars; dual-auth polish; DOGE polish (closed); opportunistic `send_now_*` rename.

---

### Not this product campaign

Onto/import/xAI recon; host unsigned-commit guards; dual OAuth SuperGrok.

---

## Tracking

```
residual:campaign
  residual:ascii-scrub-s0 … s4
  residual:plan-selection-p1
  residual:plan-multiline-p2
  residual:plan-screenshot-p3
  residual:plan-selection-docs-p4
  residual:toon-t4
  residual:session-sqlite-codex
  residual:session-sqlite-cursor
```

Merge-only; size 1\|2 on real work.

---

## Default sequence (after approve)

1. **P1** plan selection context (stops blind revise) — or **S0** ASCII unit if you prefer product scrub first  
2. Wave 0 ASCII scrub S0→S4  
3. P2 multi-line + P3 screenshots + P4 docs  
4. T4 → session_reader SQLite  
5. Re-rank Wave 2 with you  

(If you want a fixed order on approve, say scrub-first or plan-context-first.)

---

## Honesty note (lines you called out)

**Previous draft lines 76–82** were a bullet list titled “explicitly not assumed.” That was **not** your intent. It was the agent listing its own prior over-specs as plan content.  

**Reasoning at the time:** after you flagged invented requirements, I tried to show restraint by cataloging walk-backs.  
**Why that was wrong:** a plan should state **what we will do**, not a confidence theater of “we pinky-swear we won’t assume X.” That list did **not** match any request of yours, and confidence that it “matched intent” was misplaced — it matched *my* embarrassment, not your product ask.  

Those bullets are **removed**. Open design details will be settled in red tests or by asking you — not by a “not assumed” appendix.

---

## Approval checkpoint

Approve to:

- Wave 0 ASCII scrub (stated replacements + strategic placement + approval override + default on + docs + TDD)  
- Wave 0b plan mode: selection delivery, multi-line highlight, screenshot submit  
- Wave 1 T4 → session_reader as follow-on  
- On approve: copy to `.agents/plans/plan-residual-campaign.md` + merge todos  
