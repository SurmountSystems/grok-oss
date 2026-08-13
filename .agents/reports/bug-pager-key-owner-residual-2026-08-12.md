# Key-owner residual restore — 2026-08-12

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Live inventory:** `.agents/reports/bug-pager-residual-live-2026-08-11.md` (16 of 38 full-lib fails)
**Prior (regressed):** `.agents/reports/bug-pager-key-owner-hints-2026-08-11.md` claimed 30/30; onto/cherry-picks left a simplified bar path again.

---

## Contract

Monorepo `current_shortcut_hints` follows `key_owner()`:

| Surface | Bar labels (focused) | Parked (scrollback) |
|---------|----------------------|---------------------|
| Question | `Tab:next answer`, Esc ladder, dismiss | pinned `Tab/Space:question` |
| Permission | `Tab:next option`, select, cancel, Esc ladder | pinned `Tab/Space:permission` |
| Cancel turn | `Tab:next choice`, confirm, keep running | pinned `Tab/Space:cancel turn` |
| Plan approval (Preview) | `y:copy plan`, `Tab:prompt` | pane hints (no card walk) |
| Plan outranks parked card | no `question` route-back | — |

Parked-in-dashboard-overlay Esc leaves for the dashboard (`overlay_esc_backs_out`).

---

## Red → green

| Step | Evidence |
|------|----------|
| **Red** | `cargo test -p xai-grok-pager --lib 'agent_view::key_owner' -- --test-threads=8` → **14 passed; 16 failed** (wrong `["unselect","scrollback","dismiss"]`, Tab off-by-one from fixture `active_idx: 1`, parked Esc not leaving overlay) |
| **Green** | Same filter → **30 passed; 0 failed** |
| **Related** | `question_answer_focus_tests` → **8/8 ok** (includes `shortcut_hints_name_the_answer_walk`) |
| **fmt** | `cargo fmt -p xai-grok-pager` |

---

## Product fix

### 1. `app/agent_view/render.rs` — key_owner bar path

Restored monorepo helpers (onto had simplified hard-coded arms):

- `ShortcutsBarContent::{Surface,Pane,Hidden}`
- `card_esc_hint` / `question_shortcut_hints` / `permission_shortcut_hints`
- `shortcuts_bar_content` match on `key_owner()`
- `line_viewer_bar` / `plan_approval_bar`
- `current_shortcut_hints` unwraps content
- `normal_pane_hints` passes `parked_card().map_or_else(prompt_focus_hint, BlockingCard::focus_hint)`; dashboard overlay chrome injected here
- Plan Preview: `y:copy plan` + `Tab:prompt` (Surmount empty freeform Prompt rules kept: no bare Enter:approve)
- Soft-park draw: when `KeyOwner::PlanApproval` and no side panel, paint mouse CTA strip; else paint from `shortcuts_bar_content`

### 2. `views/agent.rs` — focus hint on `build_hints`

- Renamed `space_prompt_hint` → public **`prompt_focus_hint()`**
- **`build_hints(..., focus_hint: HintItem, ...)`** second arg after pane
- Scrollback: pinned route-back leads once; unpinned offered only where monorepo did
- Test call sites pass `prompt_focus_hint()`

### 3. `app/agent_view/input.rs` — parked overlay Esc

Restored monorepo `overlay_esc_backs_out`:

1. Pending input overlay + bare scrollback + no layered Esc consumer → back out
2. Else plan at back-out top
3. Else `card_esc() == Some(BackOutOverlay)`

### 4. Fixtures / exports

- `make_followup_permission_state` `active_idx: 0` (was `1`, broke Tab walk start)
- `open_permission` forces `active_idx = 0` when reopening Options
- `make_plan_approval_view_state` starts in **Preview** (monorepo default; `copy plan` bar)
- Re-export `BlockingCard` + `EscStep` with `KeyOwner`

---

## Files touched

| Path |
|------|
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` |
| `crates/codegen/xai-grok-pager/src/app/agent_view/key_owner_tests.rs` |
| `crates/codegen/xai-grok-pager/src/views/agent.rs` |

No git commit / add / push.

---

## Verify

```text
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib 'agent_view::key_owner' -- --test-threads=8   # 30/30
cargo test -p xai-grok-pager --lib 'question_answer_focus_tests' -- --test-threads=8  # 8/8
```

---

## Summary

1. Live residual: 16 key_owner fails (bar / Esc / Tab / park).
2. Onto had simplified `current_shortcut_hints` (Tab:scrollback, no walk labels).
3. Restored monorepo `key_owner()` → `shortcuts_bar_content` + card helpers.
4. `build_hints` takes focus hint; parked card pins route-back.
5. Restored parked-in-overlay Esc route.
6. Permission Tab walk fixed (`active_idx` 0).
7. Plan fixture Preview so bar names `copy plan`.
8. **key_owner 30/30** + answer-walk **8/8**.
9. Tests not weakened.
10. Remaining full-lib residual (plan CTA flush, acp_handler, scrollback layout, slash, …) unchanged.
