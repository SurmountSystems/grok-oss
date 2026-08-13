# Key-owner shortcut bar (pager residual cluster 1)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Prior diagnosis:** `.agents/reports/bug-pager-mass-fail-root-2026-08-11.md` § Residual 1
**Agent:** L2 implementer

---

## Contract

Monorepo `current_shortcut_hints` uses `key_owner()` plus card walk labels
(`"next answer"` / `"next choice"` / `"next option"`) and a pinned parked-card
route-back (`Tab/Space` → `"question"` / `"permission"` / `"cancel turn"`).
Onto left a simplified hard-coded path that labeled Tab as `"scrollback"` and
never emitted walk labels.

Tests encode the product contract (FORK / AGENTS 14–15). Do not weaken them.

---

## TDD

| Step | Evidence |
|------|----------|
| **Red** | `app::agent_view::key_owner::tests::a_parked_card_contributes_one_route_back` — panic: `hint_labels` missing `"next answer"` |
| **Green** | Same test **ok**; full `key_owner::tests` **30/30 ok** |
| **Related** | `question_answer_focus_tests` **8/8 ok** (includes `shortcut_hints_name_the_answer_walk`); `views::agent::tests` **60/60 ok** |

---

## Product fix

### 1. `views/agent.rs` — `build_hints` takes a focus hint

- Renamed private `space_prompt_hint` → public **`prompt_focus_hint()`**.
- **`build_hints(..., focus_hint: HintItem, ...)`** as second arg after
  `active_pane`.
- Scrollback arm: pinned focus hint leads once; unpinned offered only where
  monorepo did (`offer_focus_hint` closure). Call sites pass
  `prompt_focus_hint()` (tests) or parked-card `BlockingCard::focus_hint`
  (render).

### 2. `app/agent_view/render.rs` — key_owner bar path

Restored monorepo helpers and wire-up:

| Piece | Role |
|-------|------|
| `ShortcutsBarContent::{Surface,Pane,Hidden}` | What the footer row paints |
| `card_esc_hint` | Esc label from `EscStep` |
| `question_shortcut_hints` | **`Tab:next answer`**, esc ladder, dismiss |
| `permission_shortcut_hints` | **`Tab:next option`**, scope, pattern edit, ctrl-f collapsible, esc ladder |
| `shortcuts_bar_content` | Match on **`key_owner()`** |
| `line_viewer_bar` | Plan-approval prompt/comment or casual comment; else Hidden |
| `plan_approval_bar` | Surmount **empty freeform** Prompt rules kept (no Enter:approve when empty); Preview → monorepo `copy plan` + **`Tab:prompt`** |
| `current_shortcut_hints` | Unwraps `shortcuts_bar_content` |
| `normal_pane_hints` | Passes `parked_card().map_or_else(prompt_focus_hint, BlockingCard::focus_hint)` into `build_hints`; dashboard overlay inserts (stop / prev-next when cycle / dashboard) live here so cheatsheet Current matches the bar |
| `draw` | Soft-park plan CTAs (no panel, `KeyOwner::PlanApproval`) still paint mouse Approve/… strip; else paint from `shortcuts_bar_content` (Surface vs compact Pane + help) |

### 3. `app/agent_view/input.rs` — parked overlay Esc route

`overlay_esc_backs_out` was a half-merge that ignored parked cards. Restored
monorepo:

1. Pending input overlay + bare scrollback + no layered Esc consumer → back out.
2. Else plan at back-out top.
3. Else `card_esc() == Some(BackOutOverlay)`.

Fixes key_owner tests for dashboard-overlay park / second Esc.

### 4. `app/agent_view/mod.rs`

Re-export `BlockingCard` and `EscStep` with `KeyOwner` (render import path).

---

## Files touched

| Path |
|------|
| `crates/codegen/xai-grok-pager/src/views/agent.rs` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` |

No git commit / add / push.

---

## Verify commands

```text
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib a_parked_card_contributes_one_route_back   # red→green
cargo test -p xai-grok-pager --lib key_owner::tests                          # 30/30
cargo test -p xai-grok-pager --lib question_answer_focus_tests               # 8/8
cargo test -p xai-grok-pager --lib views::agent::tests                       # 60/60
```

Clippy `-D warnings` on the package still fails on **pre-existing** dead-code /
test lints elsewhere in the crate (not introduced by this slice). No new
clippy hits in the product paths edited for this cluster.

---

## 10-line summary

1. Observed red: parked-card / question bar missing `"next answer"`.
2. Onto had simplified `current_shortcut_hints` (Tab:scrollback, no walk labels).
3. Restored monorepo `key_owner()` → `shortcuts_bar_content` + card helpers.
4. `build_hints` takes focus hint; parked card pins route-back.
5. Kept Surmount empty freeform plan-approval (no bare Enter:approve).
6. Soft-park still paints mouse CTAs when plan has keys and no side panel.
7. Restored parked-in-overlay Esc route (`overlay_esc_backs_out`).
8. key_owner suite **30/30** green; related answer-walk tests green.
9. Tests not weakened.
10. Residual pager clusters 2+ (DeleteSessionComplete, share menu_hidden, …) unchanged.
