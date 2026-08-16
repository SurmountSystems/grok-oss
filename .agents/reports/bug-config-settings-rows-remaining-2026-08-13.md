# Leftover `/settings` catalog rows (2026-08-13)

SuperGrok is paid. This report says **included SuperGrok period limits**, not
"free SuperGrok."

This is the leftover Settings GUI slice after
[`.agents/reports/bug-config-unread-restore-2026-08-13.md`](bug-config-unread-restore-2026-08-13.md).
That prior slice already restored the six unread config wires, DOGE in the
theme picker, and those six `/settings` rows. This turn did **not** redo that
work.

Diagnosis:
[`.agents/reports/fork-gaps-config-options-2026-08-13.md`](fork-gaps-config-options-2026-08-13.md).
FORK said these knobs already had runtime readers and persist helpers. The
catalog and modal were missing after the 1.0.3 restack.

No new runtime. Catalog + modal + existing persist/reset/setter arms only.
Auto-compact live-apply after disk persist (`x.ai/auto_compact_threshold_changed`)
and NotificationService recap setters were restored because origin Settings
already used them. They are not new product behavior.

## Named rows added

Agent (declaration order in this tree, after `allow_worktree`):

1. `[ui] cancel_subagents_on_turn_cancel` (sticky enum: `ask` / `always_stop` / `always_continue`)
2. `[ui] auto_run_implement` (Bool, default on)
3. `[ui] economic_mode` (Bool, default on)
4. `[token_economy] cap_implement_effort_when_economic`
5. `[token_economy] max_implement_effort` (1-5, default 3)
6. `[token_economy] min_implement_effort` (1-5, default 1)
7. `[token_economy] desired_implement_effort` (1-5, default 2)
8. `[token_economy] lock_implement_effort` (0-5, 0 = unlocked)
9. `[token_economy] show_period_pacing` (included SuperGrok period limits)
10. `[token_economy] local_spend_ledger`
11. `[token_economy] reconcile_management_usage`

Session:

12. `[ui] resume_canceled_turn_on_restart` (Bool, default on)
13. `[ui.notifications] session_recap`
14. `[ui.notifications] session_recap_threshold_secs` (default 30)
15. `[features] session_recap` (restart-required master)
16. `[session] auto_compact_threshold_percent` (Enum dual: `85`/`90`/`95`/`98`/`200k`/`475k`; default canonical `"95"`; `restart_required: false`)

`[token_economy] grok_oss_database_path` stays toml-only. There is no persist
helper and no Settings row.

## TDD

Red was observed **before** the catalog rows landed (earlier turn of this
slice). The membership test listed all 16 leftover keys as missing.

### Red (observed before defs.rs rows)

```bash
cargo test -p xai-grok-pager --test settings_e2e leftover_fork_settings_rows_are_registered -- --nocapture
```

**Fail:** leftover fork settings must appear in `/settings` catalog. Missing
all 16 keys listed above.

The leftover membership test itself was not rewritten after that fail.

### Green (same contracts after catalog + modal + persist/reset arms)

Workspace target (`CARGO_TARGET_DIR` unset). The prompt's
`/tmp/grok-settings-rows-target` cold rebuild was too slow on this host; other
mop jobs were already using `/tmp`.

```bash
cargo test -p xai-grok-pager --test settings_e2e leftover_fork_settings_rows_are_registered
```

**Pass.**

```bash
cargo test -p xai-grok-pager --test settings_e2e -- \
  leftover_fork auto_run_implement economic_mode auto_compact_threshold \
  search_recap notifications_session_recap features_session_recap \
  cancel_subagents session_recap_threshold token_economy \
  defaults_round_trip registry_kind_membership enum_settings_membership \
  settings_value_payload every_registered_setting matrix_is_subset
```

**Pass.** 40 tests, 0 failed.

```bash
cargo test -p xai-grok-pager --lib -- every_setting_has_action_for_reset
cargo test -p xai-grok-pager --lib -- rows_contain_categories
cargo test -p xai-grok-pager --lib -- every_persisting_setting_has_rollback_arm
```

**Pass.** `every_setting_has_action_for_reset_arm`,
`rows_contain_categories_and_settings_through_pr_14`,
`every_persisting_setting_has_rollback_arm`.

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

**Pass.** fmt check clean. clippy `--lib -D warnings` exit 0.

## What was wired

Catalog SoT: `crates/codegen/xai-grok-pager/src/settings/defs.rs`.

Also:

- Registry `current_value_for` / `defaults_match_ui_config_default` / `PagerLocalSnapshot` leftover fields
- Action variants after `SetPromptSuggestions` (F9 keybinds in `app/actions.rs` not touched)
- Dispatch setters, router arms, `persist_setting`, auto-compact ACP notify
- Modal `action_for_bool` / `action_for_enum` / `action_for_int` / commit
- `action_for_reset` + `apply_setting_rollback` (token_economy live cache; recap inners; auto-compact default `"95"` restores AppView `None`/`None`)
- AppView mirrors: `features_session_recap`, `auto_compact_threshold_percent` / `_tokens`
- NotificationService + FocusTracker recap live setters
- Slash-command snapshots in `dashboard.rs` and `prompt.rs` (named `PagerLocalSnapshot` structs needed the new fields)

Origin/main `settings/defs.rs` was the row shape. Agent row order in **this**
tree is this file's declaration order (cancel, auto-run, economic, eight
token_economy knobs, then `default_selected_permission`). It is not
origin/main's after-`plan_mode` order.

## Files touched this slice (product + tests)

- `crates/codegen/xai-grok-pager/src/settings/defs.rs`
- `crates/codegen/xai-grok-pager/src/settings/registry.rs`
- `crates/codegen/xai-grok-pager/src/settings/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/actions.rs` (Action enum only)
- `crates/codegen/xai-grok-pager/src/app/dispatch/settings/setters.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/settings/ui.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/dashboard.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/settings.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/app_view.rs`
- `crates/codegen/xai-grok-pager/src/app/app_view_tests.rs`
- `crates/codegen/xai-grok-pager/src/notifications/mod.rs`
- `crates/codegen/xai-grok-pager/src/notifications/focus.rs`
- `crates/codegen/xai-grok-pager/src/views/settings_modal/state.rs`
- `crates/codegen/xai-grok-pager/src/views/settings_modal/tests.rs`
- `crates/codegen/xai-grok-pager/tests/settings_e2e.rs`
- Persist wrappers already existed in
  `crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs`
  (`update_features_session_recap` in `persist.rs`). This slice did not rewrite
  the spend ledger.

## Left alone (as asked)

- `app/actions.rs` F9 keybinds (live F9 writer)
- `views/prompt_widget/**`
- `token_economy` spend path / `ledger.rs`
- user-guide (separate writer)

## Still leftover (not this slice)

- User-guide still describes xAI Settings, not these Surmount rows.
- `[token_economy] grok_oss_database_path` remains toml-only.
- Dual-auth / included SuperGrok period copy in docs is a separate writer.

No `git add` / commit / push.
