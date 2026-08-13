# bug: pager status / turn / settings dispatch residual (2026-08-11)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Scope:** `app::dispatch::tests::{status,turn,settings}` after lib-compile green
**Prior:** `.agents/reports/bug-pager-mass-fail-root-2026-08-11.md`,
`.agents/reports/bug-pager-lib-compile-half-merge-2026-08-11.md`

---

## Executive status

| Filter | Before | After |
|--------|--------|-------|
| `app::dispatch::tests::status` | **58 pass / 0 fail** (already green) | **58 pass / 0 fail** |
| `app::dispatch::tests::turn` | **85 pass / 0 fail** (already green) | **85 pass / 0 fail** |
| `app::dispatch::tests::settings` | **121 pass / 8 fail** | **129 pass / 0 fail** |

**Settings residual closed.** Status and turn needed no product edits this turn.

---

## Settings roots (half-merge restore)

### Root A — deep-link open / `close_on_picker_exit` / `ActionThenClose`

**Tests:**
`dispatch_open_settings_focus_*`, `open_settings_focus_esc/enter_*`,
`deep_link_preview_esc_closes_modal_and_forwards_revert_action`

**Product holes (onto half-merge):**

1. `dispatch_open_settings` always called `try_enter_picking_enum` but never set
   `close_on_picker_exit` on success.
2. `SettingsKeyOutcome::ActionThenClose` missing; picker Enter/Esc always
   returned to Browse and never dismissed the modal for deep-link open.
3. `apply_settings_outcome` had no `ActionThenClose` arm (close + forward Action).
4. Breadcrumb mouse Esc did not clear `close_on_picker_exit` (would dismiss
   instead of hierarchical “up”).

**Restore (monorepo tip `dd04f397` shape):**

| Path | Change |
|------|--------|
| `app/dispatch/settings/ui.rs` | On focused open, set `close_on_picker_exit` only when chooser actually opens |
| `views/settings_modal/state.rs` | `ActionThenClose(Action)` variant |
| `views/settings_modal/input.rs` | Enter/Esc honor `close_on_picker_exit`; breadcrumb clears flag before Esc |
| `app/agent_view/mod.rs` | `ActionThenClose` → clear modal + `InputOutcome::Action` |

### Root B — ZDR / team lock on coding-data-sharing row

**Test:** `dispatch_open_settings_focus_skips_the_chooser_only_when_locked`

**Product holes:**

1. `CodingDataSharingLock` + `PagerLocalSnapshot.coding_data_sharing_lock` missing.
2. `AppView::coding_data_sharing_lock()` missing (ZDR / team non-admin).
3. `SettingsModalState::row_lock` + `try_enter_picking_enum` refuse-when-locked missing.
4. Snapshot builders did not seed the lock field.

**Restore:**

| Path | Change |
|------|--------|
| `settings/registry.rs` + `settings/mod.rs` | Enum + snapshot field + Default `None` |
| `app/app_view.rs` | `coding_data_sharing_lock()` mirrors set-coding-data guards |
| `views/settings_modal/state.rs` | `row_lock`; guard in `try_enter_picking_enum` |
| `views/settings_modal/input.rs` | Browse `d` no-op when row locked |
| `app/dispatch/settings/ui.rs` | Snapshot refresh / open / `build_pager_snapshot` seed lock |
| `app/dispatch/prompt.rs`, `dashboard.rs` | Full snapshot literals seed lock |

### Root C — exhaustive move-away helper lagging new settings

**Tests:** `every_setting_has_action_for_reset_arm`,
`every_persisting_setting_has_rollback_arm`

**Not a product bug.** Helper `move_setting_away_from_default` lagged keys
registered after the last mop:

- `hide_header`, `scrub_ascii_punct`, `bubble_copy_buttons`
- `plan_approval_park`, `cancel_subagents_on_turn_cancel`
- `notifications.session_recap`, `notifications.session_recap_threshold_secs`
- `features.session_recap`

**Also:** deep-link preview Esc expected monorepo default theme `groknight`;
Surmount product default is **`doge`**. Assertion updated to product default
(named contract: Esc reverts to the theme live when the chooser opened).

---

## Files touched

**Product**

- `crates/codegen/xai-grok-pager/src/settings/registry.rs`
- `crates/codegen/xai-grok-pager/src/settings/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/app_view.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/settings/ui.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/dashboard.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`
- `crates/codegen/xai-grok-pager/src/views/settings_modal/state.rs`
- `crates/codegen/xai-grok-pager/src/views/settings_modal/input.rs`

**Tests**

- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/settings.rs`
  (move-away arms + deep-link theme expect)

No shared dispatch router thrash; no dashboard-stop lifecycle edits.

---

## Verify

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::settings' -- --test-threads=8
# ok. 129 passed; 0 failed

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::status' -- --test-threads=8
# ok. 58 passed; 0 failed

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::turn' -- --test-threads=8
# ok. 85 passed; 0 failed

cargo fmt -p xai-grok-pager
# exit 0
```

**Clippy:** `cargo clippy -p xai-grok-pager --lib -- -D warnings` fails in
**dependency** `xai-grok-tools` (pre-existing dead_code + disallowed
`Command::spawn`), not in pager changes this turn. Not mopped here.

---

## Residual (out of this slice)

| Area | Notes |
|------|--------|
| Dashboard stop lifecycle | Still 2 fails from compile mop: `dashboard_stop_double_press_*`, `dashboard_stop_with_peek_*` |
| Session fork / load | Separate residual filters (sticky chat, worktree cwd, title hydration) |
| Key-owner shortcut bar | Mass-fail cluster #1 still open |
| DeleteSessionComplete full After | Mass-fail cluster #2 still open |
| Share menu_hidden | Mass-fail cluster #3 still open |
| Settings render lock chrome | ZDR/team lock **behavior** restored for open/enter; monorepo `value_display` ZDR/"Admin Managed" paint not re-ported (no unit fail in this filter) |
| Clippy deps | `xai-grok-tools` unrelated reds |

---

## 10-line summary

1. Status **58/58** and turn **85/85** were already green; no product work.
2. Settings had **8** fails → two product half-merges + exhaustive helper lag.
3. Restored monorepo deep-link: `close_on_picker_exit` + `ActionThenClose`.
4. Restored coding-data row lock (ZDR / team non-admin) end-to-end in snapshots.
5. Filled `move_setting_away_from_default` arms for newer registered settings.
6. Deep-link theme expect: **doge** (Surmount default), not groknight.
7. Settings **129 pass / 0 fail**; fmt clean.
8. No git commit/add/push.
