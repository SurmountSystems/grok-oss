# Dashboard Ctrl+X stop → delete_confirm lifecycle (2026-08-11)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior:** `.agents/reports/bug-pager-lib-compile-half-merge-2026-08-11.md` § Lifecycle smoke; `.agents/reports/bug-pager-delete-session-complete-2026-08-11.md`

---

## Status

| Check | Result |
|-------|--------|
| `dashboard_stop_double_press_via_handle_key_deletes_top_level` | **GREEN** |
| `dashboard_stop_with_peek_open_moves_selection_and_peek_down_one` | **GREEN** |
| Full `app::dispatch::tests::session::lifecycle::` | **88 passed; 0 failed** |
| Related dashboard_stop / footer / nav disarm | **10/10** green (subset) |
| `cargo check -p xai-grok-pager --lib` | **GREEN** |
| `cargo fmt -p xai-grok-pager` | exit 0 |

---

## Red (before)

```text
dashboard_stop_double_press_via_handle_key_deletes_top_level
  → first Ctrl+X must arm delete_confirm

dashboard_stop_with_peek_open_moves_selection_and_peek_down_one
  → no entry found for key  (agent already gone before DeleteSessionComplete)
```

Half-merge had restored **legacy** list stop:

- first Ctrl+X armed `stop_confirm`
- second press called `dispatch_sessions_confirm_close` and **removed the agent immediately**

Monorepo / lifecycle tests encode:

- idle row: first Ctrl+X arms **`delete_confirm`**
- second press emits **`Effect::DeleteSession { after: Dashboard }`**
- agent removed only on **`DeleteSessionComplete`** (selection + peek move there)

---

## Product fix (monorepo restore)

### 1. `dispatch/dashboard.rs` — `dispatch_dashboard_stop`

Restored monorepo path from tip `b13fa526`:

| Row | Behavior |
|-----|----------|
| Idle / deletable top-level or roster | `arm_or_delete` → second press `delete_dashboard_row` → `DeleteSession` |
| Busy top-level | `stop_top_level_activity` (cancel turn / kill bg / drop loops+queue); **never arm** |
| Busy roster | toast "Stop the session before deleting"; no arm |
| Subagent | `KillSubagent` (unchanged) |

Helpers added: `stop_top_level_activity`, `arm_or_delete`. Existing `delete_dashboard_row` / `dispatch_dashboard_delete` kept.

### 2. `views/dashboard/state.rs` — key path

- Preserve **`delete_confirm`** on Ctrl+X (`DashboardStop`); disarm on other keys.
- List-focused bare `y`/`n` → `Action::DashboardDelete` / cancel via `handle_delete_confirm_key`.

### 3. Wire

- `Action::DashboardDelete` + router arm → `dispatch_dashboard_delete`
- `CancelTrigger::DashboardStop` for busy-row cancel telemetry
- Footer: live delete arm → pending "delete this session" (or y/n when list focused)

### 4. Tests (contract alignment, not weaken)

Dashboard tests that still expected `stop_confirm` + immediate agent drop were restored to monorepo:

- arm `delete_confirm`
- second press emits `DeleteSession`
- selection moves after `DeleteSessionComplete`

Same for footer / `nav_key_disarms_pending_delete_confirm`.

---

## Files touched

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/app/dispatch/dashboard.rs` | monorepo stop / arm_or_delete / stop_top_level_activity |
| `crates/codegen/xai-grok-pager/src/app/actions.rs` | `DashboardDelete`, `CancelTrigger::DashboardStop` |
| `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` | DashboardDelete route |
| `crates/codegen/xai-grok-pager/src/views/dashboard/state.rs` | delete_confirm key preserve + y/n |
| `crates/codegen/xai-grok-pager/src/views/dashboard/render.rs` | footer delete arm |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/dashboard.rs` | stop tests → monorepo |
| `crates/codegen/xai-grok-pager/src/views/dashboard/state.rs` (unit) | nav disarm test |
| `crates/codegen/xai-grok-pager/src/views/dashboard/render.rs` (unit) | footer tests |

No git commit/add/push. No FORK dual-pin (intent matches monorepo tests + action long_help).

---

## Residual

| Item | Notes |
|------|--------|
| Legacy `DashboardState::stop_confirm` field | Still present, unused by list stop (overlay uses `pending_action` + `STOP_CONFIRM_WINDOW`). Optional scrub later |
| Clippy `-D warnings` on full pager lib | Pre-existing dead_code / disallowed_methods elsewhere; not introduced by this slice |
| `cargo check --lib --tests` | Pre-existing `settings_e2e` exhaustiveness hole unrelated to stop |
| Broader busy-row / bg-work / queue stop tests | Not all re-added this pass; monorepo has more; product helpers support them |

---

## Commands

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::session::lifecycle::' -- --test-threads=8
# 88 passed; 0 failed

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8 \
  dashboard_stop_ nav_key_disarms_pending_delete render_footer_delete
# related stop cluster green

cargo fmt -p xai-grok-pager
nice -n 19 ionice -c3 cargo check -p xai-grok-pager --lib
```
