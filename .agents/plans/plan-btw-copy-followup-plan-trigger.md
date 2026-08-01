# Plan: btw copy + follow-up; free-text plan ≠ plan mode

## Context

Operator wishlist:

1. btw panel **Copy** — entire contents; multi-turn = whole conversation.
2. btw panel **Follow-up** — another question in the **same** btw context.
3. Casual use of the word **“plan”** in a normal prompt must **not** immediately
   enter plan mode.

Constraints: red/green TDD; document in FORK/user-guide when shipping; no
agent commit. Related: softer plan modal residual is separate (toast after
unexpected entry).

Assumptions (documented):

- Today btw is **single-turn** only; follow-up is a real product slice, not a
  one-line button.
- Plan false entry is **model + tool description + auto allowlist**, not a
  client free-text keyword detector.

## Approach

Three slices; B1 and B3 can ship independently; B2 builds on B1 for copy-all-thread.

### B1 — Copy entire btw (S)

- Done-state chrome: **Copy** button + optional key when `btw_focused`.
- Copy plain text via existing selection/clipboard path
  (`full_selection_model` / reconstruct full content).
- Tests: full answer including scrolled-out lines; action dispatch.

### B2 — Follow-up in same btw (M)

- UI: follow-up affordance + message list (or prior Q/A + composer).
- Shell: reuse `btw_session_id`; pass prior turns; drop “no follow-up” for
  continuation turns; persist turns (schema evolve or multi-entry).
- B1 copy exports full thread.
- Tests: shell session-id reuse; pager second send with parent id.

### B3 — Plan free-text false positive (S / S+)

- Rewrite `EnterPlanModeTool` description for **explicit** plan-mode intent only.
- Optional: remove from auto-mode allowlist / force permission.
- Do **not** ban the word “plan” on the client.
- Keep `/plan` and settings as user-initiated paths.
- Tests: description contract; allowlist if gated; `/plan` still works.

**Not:** client keyword ban; full plan modal redesign in this plan.

## Critical files

| Path | Why |
|------|-----|
| `pager/src/views/btw_overlay.rs` | Panel chrome, copy, multi-turn UI |
| `pager/src/app/dispatch/notes.rs` | `SendBtw` / response |
| `pager/src/app/agent_view/input.rs` | Focus/keys/copy action |
| `shell/.../recap.rs` `handle_side_question` | Single-turn → multi-turn |
| `shell/.../persistence.rs` `BtwEntry` | History shape |
| `tools/.../enter_plan_mode/mod.rs` | Description |
| `workspace/.../auto_mode.rs` | Optional allowlist gate |
| user-guide btw / plan docs | Honesty |

## Reuse

| Symbol | How |
|--------|-----|
| `BtwOverlayState::full_selection_model` | Copy-all without reimplementing selection |
| `Action::SendBtw` / ACP `x.ai/btw` | Extend for parent/session id |
| Soft-park residual | Separate; still do toast for unexpected modal |

## Steps

1. **B1** red tests for copy-all → green chrome + clipboard. **Done** (focused `y`, `[y]` chrome, `full_copy_text`).
2. **B3** red description (and optional allowlist) contract → green tool/policy. **Done** (explicit-intent description + auto PromptUser).
3. **B2** red multi-turn shell+pager → green state + follow-up UX + history. **Done**
   (same `btw_session_id`, prior turns in model request, in-panel `[a]` composer,
   multi-entry `btw_history.jsonl`, `full_copy_text` whole thread).
4. Docs: FORK + RESIDUAL + research status updated for B1/B2/B3; user-guide optional.

## Risks

| Risk | Mitigation |
|------|------------|
| Multi-turn scope creep | Ship B1+B3 first if B2 slips |
| Clipboard harness flaky in CI | Prefer unit reconstruct text over pty when possible |
| Tight description over-blocks real plan requests | Keep `/plan` + clear explicit phrases |
| Auto allowlist change surprises YOLO users | Document; optional config later |

## Verification

```bash
cargo test -p xai-grok-pager --lib -- btw_overlay btw_focus copy
cargo test -p xai-grok-shell --lib -- side_question btw
cargo test -p xai-grok-tools --lib -- enter_plan_mode
# if allowlist touched:
cargo test -p xai-grok-workspace --lib -- auto_mode enter_plan
```

Manual: `/btw` → Copy pastes full Q/A; Follow-up second turn same panel;
prompt “we should plan the migration” does **not** enter plan mode; `/plan`
still does.

## Open questions

1. Follow-up: mini-input **inside** panel vs focus main prompt with btw context?
   - *Recommend:* in-panel composer when Done (keeps context visible).
2. B3 hard permission gate now or description-only first?
   - *Recommend:* description first (S); allowlist drop if still too eager.

## Research

`doc/dev/research/btw-copy-followup-plan-trigger-2026-07-26.md`
