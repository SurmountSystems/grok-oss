# Restore unread config options (2026-08-13)

SuperGrok is paid. This report says **included SuperGrok period limits**, not "free SuperGrok."

Diagnosis was already on disk
([`fork-gaps-config-options-2026-08-13.md`](fork-gaps-config-options-2026-08-13.md)).
This turn wired the six unread fields, put them in `/settings`, and added DOGE
to the theme picker. It did not invent new options. It did not touch `/spend`
or last-session / canceled-turn startup.

## Named contracts restored

1. `[ui] hide_header` — pager zeros status / welcome / dashboard headers.
2. `[ui] always_expand_thinking` — thinking blocks stay expanded; Ctrl+E is hidden. Distinct from `show_thinking_blocks`.
3. `[ui] plan_approval_park` — `"modal"` forces fullscreen; `"soft"` is default. Pager calls `plan_approval_force_modal`.
4. `[subagents] allow_worktree` — `resolve_subagents` copies it. Empty or false forces no worktree isolation. `true` opts in. Spawn honors it.
5. `[ui] scrub_ascii_punct` — `prime` plus `seed_from_effective_config` at startup so disk `false` applies at launch. Env `GROK_SCRUB_ASCII_PUNCT=0` still works.
6. `[scrollback.display] bubble_copy_buttons` — render and pager read the field. When on, user/agent first line shows ⧉ and the selection box omits ⧉.
7. DOGE in `/settings` theme picker — `doge` is in `THEME_CHOICES` and `CONCRETE_THEME_CHOICES`. Settings default is `"doge"`. Unset theme no longer writes `groknight` over the product default.

Six `/settings` catalog rows were added for those keys (`settings/defs.rs`).

## TDD

Red was observed before the product readers were wired. Tests that already
existed for a missing reader were kept. Catalog paint tests
`hide_header_zeroes_*` / `hide_header_zeros_*` were restored in the prior
paint shape (status, welcome, dashboard).

### Red (observed before product restore)

```bash
cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub_ascii_punct_from_ui
```

**Fail:** `prime must seed scrub_ascii_punct from UiConfig so disk false applies at launch`

```bash
cargo test -p xai-grok-pager --lib -- \
  always_expand_thinking_keeps_blocks_expanded \
  always_expand_thinking_hides_ctrl_e_hint \
  theme_choices_include_doge_and_default_is_doge \
  plan_approval_soft_park_is_not_fullscreen \
  plan_approval_modal_park_is_fullscreen
```

**Fail (same filter, before paint/settings readers):**

- `theme_choices_include_doge_and_default_is_doge` — left `groknight`, right `doge`
- `plan_approval_soft_park_is_not_fullscreen` — soft park still opened fullscreen
- `always_expand_thinking_hides_ctrl_e_hint` — still painted `Thought  (ctrl+e to expand)`
- `always_expand_thinking_keeps_blocks_expanded` — left `Truncated`, right `Expanded`
- `plan_approval_modal_park_is_fullscreen` — already passed (modal helper existed)

### Green (same contracts after restore)

```bash
cargo test -p xai-grok-pager --lib -- \
  hide_header_zeroes_status_bar_height \
  hide_header_zeros_welcome_top_bar_height \
  hide_header_zeroes_header_and_header_gap \
  always_expand_thinking_keeps_blocks_expanded \
  always_expand_thinking_hides_ctrl_e_hint \
  theme_choices_include_doge_and_default_is_doge \
  plan_approval_soft_park_is_not_fullscreen \
  plan_approval_modal_park_is_fullscreen \
  bubble_copy_buttons_on_paints_copy_icon \
  bubble_copy_buttons_off \
  every_setting_has_action_for_reset
```

**Pass** (16 pager lib tests in that wave, including confirm-reset / deep-link /
auto-dark persist that now expect `doge`).

```bash
cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub_ascii_punct_from_ui
```

**Pass.**

```bash
cargo test -p xai-grok-shell --lib -- \
  resolve_subagents_copies_allow_worktree \
  apply_allow_worktree_policy_false_forces_none \
  subagents_config_allow_worktree
```

**Pass** (5 shell lib tests).

```bash
cargo test -p xai-grok-pager --lib -- \
  defaults_match_ui_config_default \
  defaults_match_pager_state \
  auto_dark
```

**Pass** (registry defaults + auto-dark fallback now `Enum("doge")`).

```bash
cargo test -p xai-grok-pager --lib -- \
  breadcrumb \
  d_key
```

**Pass** (settings modal original default is `doge`).

```bash
cargo test -p xai-grok-pager --test settings_e2e -- \
  ALL_SETTINGS_EXERCISED \
  hide_header \
  always_expand_thinking \
  scrub_ascii_punct \
  allow_worktree \
  bubble_copy_buttons \
  plan_approval_park \
  registry_kind_membership \
  enum_settings_membership \
  defaults_round_trip
```

**Pass** (19 settings_e2e tests in that wave: matrix, keyboard/mouse for the
five bools, plan park picker, kind membership, defaults).

## What changed

### Readers (unread 6)

- Appearance cache: `hide_header`, `plan_approval_force_modal`,
  `allow_worktree`, `bubble_copy_buttons` load/set. `prime()` seeds
  `scrub_ascii_punct`, `hide_header`, and plan park from `UiConfig`.
- Event loop after prime: seeds bubble copy, appearance hide header, then
  `seed_from_effective_config()` so disk `scrub_ascii_punct = false` applies
  at launch.
- Agent status height, welcome top bar, and dashboard chrome read
  `load_hide_header()` and go to zero when on.
- Thinking blocks: default / finished / collapse stay `Expanded` when
  `load_always_expand_thinking()` is on. Expand hint and footer Ctrl+E are
  hidden.
- Plan preview: `handle_exit_plan_mode` syncs
  `set_plan_approval_force_modal(app.current_ui.plan_approval_force_modal())`.
  `show_plan_preview` sets `viewer.fullscreen` from the cache.
- `Config::resolve_subagents` copies `allow_worktree` onto
  `Config.subagent_allow_worktree`. Spawn context carries it.
  `apply_allow_worktree_policy` after `resolve_runtime_config` forces
  `IsolationMode::None` when empty or false.
- User and agent bubbles call `append_bubble_copy_button` when the display
  flag is on. Selection box omits ⧉ when bubble copy is on.

### `/settings` (6 rows + DOGE)

- `THEME_CHOICES` / `CONCRETE_THEME_CHOICES` include `doge` first after
  `auto` / first concrete. Theme and auto-dark defaults are `"doge"`.
  `current_value_for` uses `unwrap_or("doge")`.
- Catalog rows: `hide_header` (Appearance, after compact),
  `plan_approval_park` and `allow_worktree` (Agent, after remember tool
  approvals), `always_expand_thinking`, `scrub_ascii_punct`,
  `bubble_copy_buttons` (Appearance, after `collapsed_edit_blocks`, not
  between the show/respect/group/collapsed sandwich).
- Actions, setters, reset, rollback, and persist arms for those keys.
- `set_allow_worktree` writes only `[subagents].allow_worktree` via
  `persist::update_subagents_allow_worktree`. Persist `Config` has no
  `subagents` field, so a full splat would have been wrong.
- `set_auto_dark_theme` unset rollback is `ThemeKind::Doge`, not GrokNight.

## Files touched (product)

- `crates/codegen/xai-grok-pager-render/src/appearance/cache.rs`
- `crates/codegen/xai-grok-pager-render/src/theme/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/event_loop.rs`
- `crates/codegen/xai-grok-pager/src/app/actions.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs`
- `crates/codegen/xai-grok-pager/src/app/acp_handler/interactions.rs`
- `crates/codegen/xai-grok-pager/src/app/acp_handler/tests/plan_mode.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/settings/setters.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/settings/ui.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/settings.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/status.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/agent.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/thinking.rs`
- `crates/codegen/xai-grok-pager/src/views/agent.rs`
- `crates/codegen/xai-grok-pager/src/views/welcome/mod.rs`
- `crates/codegen/xai-grok-pager/src/views/dashboard/layout.rs`
- `crates/codegen/xai-grok-pager/src/views/settings_modal/state.rs`
- `crates/codegen/xai-grok-pager/src/views/settings_modal/tests.rs`
- `crates/codegen/xai-grok-pager/src/settings/defs.rs`
- `crates/codegen/xai-grok-pager/src/settings/registry.rs`
- `crates/codegen/xai-grok-pager/tests/settings_e2e.rs`
- `crates/codegen/xai-grok-shell/src/agent/config.rs`
- `crates/codegen/xai-grok-shell/src/agent/subagent/mod.rs`
- `crates/codegen/xai-grok-shell/src/agent/subagent/handle_request.rs`
- `crates/codegen/xai-grok-shell/src/agent/mvp_agent/subagent_coordinator.rs`
- `crates/codegen/xai-grok-shell/src/config/tests.rs`
- `crates/codegen/xai-grok-shell/src/test_support/lsp_runtime.rs`
- `crates/codegen/xai-grok-shell/src/util/config/persist.rs`
- `crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs`

## Post-impl

- `cargo fmt -p xai-grok-pager -p xai-grok-pager-render -p xai-grok-shell` — ok
- `cargo clippy -p xai-grok-pager --lib -- -D warnings` — exit 0
- `cargo clippy -p xai-grok-pager-render --lib -- -D warnings` — exit 0
- `cargo clippy -p xai-grok-shell --lib -- -D warnings` — exit 0
- Targeted tests above — ok
- `cargo clippy --all-targets -D warnings` on pager/shell still fails on
  **pre-existing** files this slice did not edit (doctor early dispatch,
  benches, auth field reassign, ascii-scrub await lock, subprocess test
  module order, shared HTTP rate limit). Not mopped.

## Leftovers (not this slice)

Settings rows this turn did **not** add (runtime already present; still no
catalog):

- `[ui] economic_mode`
- `[ui] auto_run_implement`
- `[ui] resume_canceled_turn_on_restart`
- eight `[token_economy]` knobs
- `[session] auto_compact_threshold_percent`
- `[ui.notifications] session_recap` / `session_recap_threshold_secs` and
  `[features] session_recap`
- `[ui] cancel_subagents_on_turn_cancel`

User-guide under `crates/codegen/xai-grok-pager/docs/user-guide/` is still
the xAI restack body. Zero hits for `hide_header`, `always_expand_thinking`,
`plan_approval_park`, `allow_worktree`, `scrub_ascii_punct`,
`bubble_copy_buttons`, `doge`, `economic_mode`, `token_economy`,
`preferred_method`, `auto_use_included_limits`, or `management_team_id`.
Theming guide still lists five xAI themes and names GrokNight the default.
Auto-compact guide still says 85; code default is 95.

Live `/settings` `allow_worktree` updates cache and disk. Spawn honors
`Config.subagent_allow_worktree` from `resolve_subagents` on reload or
restart. This slice did not invent a second live `Config` writer.

## Counts

| Restored | N |
|----------|---|
| Unread fields now read | **6** |
| DOGE in Settings theme picker | **1** |
| `/settings` catalog rows for those keys | **6** |

**6 unread + DOGE picker + 6 settings rows.**
