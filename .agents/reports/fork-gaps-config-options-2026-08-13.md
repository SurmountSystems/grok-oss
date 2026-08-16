# Config options after the 1.0.3 restack

**Date:** 2026-08-13
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** diagnosis only. No product edits.

## What happened

The 1.0.3 restack kept many Surmount **structs and persist helpers**. It did not keep the **operator surfaces** that make those options real: `/settings` rows, user-guide copy, and several paint or spawn readers. A field that still deserializes is not a working option if nothing reads it.

This walk is config only. SuperGrok is paid. This report says **included SuperGrok period limits**, never "free SuperGrok." FORK and some chrome strings still say "free SuperGrok period." That is a language residual, not a missing key.

`hide_header` is unread, as already known. It is not the only unread key.

**Counts (exclusive buckets below):** **22 missing** operator surfaces, **6 unread** fields, **18 still present** fork-claimed keys with a real reader.

## How to read status

| Status | Meaning |
|--------|---------|
| **present** | Key deserializes and a runtime path reads it for the named contract |
| **unread** | Field or table exists; nobody consumes it for that contract |
| **missing** | FORK or user-guide claims a user-facing option; `/settings` catalog and/or the shipped guide do not expose it |
| **docs-only** | Prose claims it; code was not the owner or the guide is xAI copy |
| **unproven** | Not decided this turn; the deciding check is named |

`/settings` catalog SoT is `crates/codegen/xai-grok-pager/src/settings/defs.rs` (47 `key:` rows). None of the Surmount-only keys below are in that list. User-guide under `crates/codegen/xai-grok-pager/docs/user-guide/` and `~/.grok/docs/user-guide/` has **zero** hits for `hide_header`, `economic_mode`, `token_economy`, `always_expand_thinking`, `doge`, `allow_worktree`, `preferred_method`, `auto_use_included_limits`, or `management_team_id`.

## Missing or unread (operator-visible)

| Option | Exact key | FORK / docs claim | Code status | Evidence | Operator-visible miss |
|--------|-----------|-------------------|-------------|----------|------------------------|
| Hide in-app header | `[ui] hide_header` | FORK: zeros status / welcome / dashboard headers. Default false. Distinct from window titles. | **unread** | Field on `UiConfig` and `AppearanceConfig`. `.hide_header` is never read. Appearance default is hardcoded `false` (`appearance/config.rs` seed comment). Catalog paint tests `hide_header_zeroes_*` are gone. Not in `settings/defs.rs`. | `[ui] hide_header = true` does nothing. No `/settings` row. |
| Always expand thinking | `[ui] always_expand_thinking` | FORK / `impl-thinking-always-expanded`: keep thinking blocks expanded; hide Ctrl+E. Settings row. | **unread** | Field + `resolve_always_expand_thinking` + cache load/set + `set_always_expand_thinking` persist. `load_always_expand_thinking` is only called from `appearance/cache.rs` (seed + unit test). No thinking-block paint read. Not in `defs.rs`. | Config and persist exist. Thinking still collapses. No settings row. |
| Plan approval park | `[ui] plan_approval_park` | FORK: `"soft"` default; `"modal"` forces fullscreen. | **unread** | `UiConfig` helpers + serde tests only. No pager call to `plan_approval_force_modal`. Not in `defs.rs`. | `plan_approval_park = "modal"` cannot switch park style. |
| Subagent worktrees | `[subagents] allow_worktree` | FORK: default false; empty config force-none; `true` opts in. User-guide `05` / `16`. | **unread** | `SubagentsConfig.allow_worktree` deserializes; unit tests parse it. `Config::resolve_subagents` copies enabled / models / toggle / roles / personas / max_depth only. No `subagent_allow_worktree` on `Config`. `handle_request.rs` has no force-none. User-guide `16` has no `allow_worktree`. | `allow_worktree = true` cannot enable worktree isolation. Default-false is accidental (nobody reads the flag), not a force-none policy. |
| ASCII scrub of assistant text | `[ui] scrub_ascii_punct` | FORK: default on; env `GROK_SCRUB_ASCII_PUNCT=0`; Appearance row; AllowAlways writes disk. | **unread** (disk at startup) | `seed_from_effective_config` exists and is **never called**. Pager `load_scrub_ascii_punct` is never called outside `cache.rs`. Reloader only updates on a later file *change*. Stream uses `AtomicBool` default **true**. Env still gates `should_scrub`. | `scrub_ascii_punct = false` in `config.toml` does not apply at launch. No `/settings` row. Env still works. |
| Bubble copy buttons | `[scrollback.display] bubble_copy_buttons` (`pager.toml`) | FORK: Settings Appearance toggle; default on. | **unread** | Field loads and persist helper exists. No render or pager read of `bubble_copy_buttons` outside `appearance/config.rs`. Not in `defs.rs`. | Toggling the key (if written by hand) cannot hide or show bubble copy chrome. No settings row. |
| Economic mode (settings) | `[ui] economic_mode` | FORK: settings default on; `/economic-mode`; Token Economy caps when on. User-guide `05`. | **missing** (settings + docs). Runtime **present** (see still-works). | Not in `defs.rs`. No `set_economic_mode` arm in `dispatch/settings`. User-guide has no `economic_mode`. | Operator cannot find or toggle it in `/settings` or the guide. Slash and toml still work. |
| Auto-run `/implement` (settings) | `[ui] auto_run_implement` | FORK: settings modal; default on. | **missing** (settings). Runtime **present**. | Cache + `auto_implement.rs` read it. Persist exists. Not in `defs.rs`. | No settings row. Disk value still applies if already written. |
| Continue interrupted turn (settings) | `[ui] resume_canceled_turn_on_restart` | FORK: Settings GUI; default on. User-guide `05` / `17`. | **missing** (settings + docs). Runtime **present**. | Session load reads `resume_canceled_turn_on_restart_enabled()`. Persist exists. Not in `defs.rs`. Guide has no key. | No settings row. Disk value still applies. |
| Token Economy table (settings) | `[token_economy]` `cap_implement_effort_when_economic`, `max_implement_effort`, `min_implement_effort`, `lock_implement_effort`, `desired_implement_effort`, `show_period_pacing`, `local_spend_ledger`, `reconcile_management_usage`, `grok_oss_database_path` | FORK: Settings modal covers caps, pacing, ledger, reconcile. User-guide `05`. | **missing** (settings + docs). Runtime **present** via toml parse. | `token_economy_from_toml` + live cache. Persist helpers `set_token_economy_*`. Zero keys in `defs.rs`. Guide has no `[token_economy]`. | Eight knobs cannot be edited in `/settings`. Hand-edited toml still parses. |
| Auto-compact threshold (settings) | `[session] auto_compact_threshold_percent` | FORK: default 95; settings live-apply (`restart_required: false`). User-guide says default **85**. | **missing** (settings). Default in code is **present** (95). Docs **wrong**. | `defs.rs` has no row. Modal test says it is "not exposed." Persist `set_auto_compact_threshold_percent` exists. Compaction crate default is 95. Guide `05` and `04` say 85. | No settings row. Guide lies about 85. Hand-edited `[session]` still resolves. |
| Session recap (settings) | `[ui.notifications] session_recap`, `session_recap_threshold_secs`; `[features] session_recap` | FORK: Settings search `recap`; master feature restart-required. | **missing** (settings). Runtime **present**. | `NotificationConfig` + `Features.session_recap` + `resolve_session_recap`. Persist `update_features_session_recap`. Not in `defs.rs`. | No settings rows. Toml / feature flag still resolve. |
| Cancel subagents with turn (settings) | `[ui] cancel_subagents_on_turn_cancel` | FORK: sticky enum under Agent in Settings. | **missing** (settings). Runtime **present**. | `dispatch/settings/ui.rs` has arms. `turn.rs` reads `current_ui`. **Not** in `defs.rs`, so the modal never shows the row. | No settings row. Disk + in-session "Always stop / continue" persist can still apply if something writes the key. |
| DOGE in Settings theme picker | `[ui] theme = "doge"` | FORK: default theme; display "DOGE"; auto-dark maps to doge. User-guide `06`. | **missing** (settings + docs). `/theme` and toml **present**. | `THEME_CHOICES` in `defs.rs`: auto, groknight, grokday, tokyonight, rosepine-moon, oscura-midnight. **No doge.** Settings default string is `"groknight"`. Runtime `ThemeKind::from_name("doge")` and `/theme` `ThemeKind::available()` include Doge. Unset theme still resolves to Doge (`theme/cache.rs`). Guide `06` lists five xAI themes and names GrokNight the default. | `/settings` cannot pick DOGE. Opening Settings and committing Theme can write `groknight` over the product default. `/theme doge` still works. |
| Dual-auth / spend-order keys (docs) | `[auth] preferred_method`, `[auth] auto_use_included_limits`, `[auth] allow_spend_when_free_period_debit_unproven`, `[endpoints] management_team_id` | FORK: shipped. User-guide `02` / `05` should name them. | **missing** (docs). Runtime **present** (see still-works). Hop *effect* of `auto_use_included_limits` is a separate restack loss (empty `failover_api_keys`), not an unread field. | Zero user-guide hits. Structs and loaders exist. | Operator cannot discover the keys in the shipped guide. |
| Window title vs hide_header (docs) | `[ui.notifications.title] enabled` | FORK: titles on by default; opt-out is this key only; no `hide_title_bar`. | **docs-only** gap. Runtime **present**. | `TitleConfig.enabled` defaults true. `notifications/mod.rs` gates OSC on it. Stale `hide_title_bar` is ignored (shared test). Guide `06` has no hide_header vs title split. | Titles still work. Guide does not teach the fork contract. |

## Still works

These fork-claimed (or required companion) keys still have a reader. Several have **no** `/settings` row. That is listed above as missing surface, not as unread field.

| Option | Exact key | Claim | Code status | Evidence |
|--------|-----------|-------|-------------|----------|
| Permission mode | `[ui] permission_mode` (legacy `approval_mode`, `yolo`) | Settings + `/always-approve`. | **present** | `load_permission_mode` in `util/config/permissions.rs`. In `defs.rs`. |
| First permission cursor | `[ui] default_selected_permission` | Settings + guide `05`. | **present** | In `defs.rs`. Pager permission view. |
| Remember tool approvals | `[ui] remember_tool_approvals` | Settings + guide `22`. | **present** | In `defs.rs`. Persist + gate. |
| Economic context cap | `[ui] economic_mode` | Soft-cap ~200k. | **present** (runtime) | Spawn seeds `compaction.economic_mode` from `economic_mode_from_disk()`. `apply_economic_context_cap` on spawn / model switch / header upgrade. `/economic-mode` slash registered. Implement-effort policy reads `load_economic_mode()`. |
| Auto-run implement | `[ui] auto_run_implement` | After a successful turn. | **present** (runtime) | `app/auto_implement.rs` calls `load_auto_run_implement()`. |
| Continue interrupted turn | `[ui] resume_canceled_turn_on_restart` | Default on. | **present** (runtime) | `app/dispatch/session/load.rs` reads `resume_canceled_turn_on_restart_enabled()`. |
| Token Economy policy | `[token_economy].*` | Effort lock / min / max / desired; pacing; ledger. | **present** (toml) | `token_economy_from_toml` + `apply_implement_effort_policy`. `/limits` reads `show_period_pacing` and `reconcile_management_usage`. Spend report reads ledger flags. Compact status chip that *also* used pacing is still unpainted (chrome, not this key). |
| Prefer included SuperGrok period limits | `[auth] auto_use_included_limits` | Default true on empty config. | **present** (rank / limits / doctor) | `GrokComConfig` field. Ranking + `limits_cmd.rs` + doctor copy. **Hop list still empty** after included SuperGrok period limits are full (`failover_api_keys: Vec::new()`). That is a spend-path restack loss, not an unread bool. |
| Pin auth method | `[auth] preferred_method` | `api_key` / `oidc`. | **present** | Serde + `auth/manager.rs` fail-closed. |
| Unproven debit hard block | `[auth] allow_spend_when_free_period_debit_unproven` | Default true; `false` blocks. Env `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN`. | **present** | `allow_spend_when_free_period_debit_unproven_from_config` used by guard, `/limits`, credit_bar honesty helpers. |
| Management team id | `[endpoints] management_team_id` | Team prepaid / usage series. | **present** | `load_management_team_id_sync` + `resolve_management_team_id`. |
| Window titles | `[ui.notifications.title] enabled` (default true) | Titles on by default. | **present** | `TitleConfig` + `notifications/mod.rs`. |
| DOGE theme via slash / toml | `[ui] theme = "doge"` | Default when unset; `/theme`. | **present** (`/theme`, toml, unset default) | `ThemeKind::from_name("doge")`. `/theme` lists `ThemeKind::available()`. Unset resolves to Doge. |
| Auto-compact default 95 | `[session] auto_compact_threshold_percent` unset | FORK default 95. | **present** (code default) | `DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT = 95`. User toml still wins if set. |
| Session recap master | `[features] session_recap` | Disable recap. | **present** | `Features.session_recap` + `resolve_session_recap`. |
| Session recap notify knobs | `[ui.notifications] session_recap`, `session_recap_threshold_secs` | Auto recap after away. | **present** | `NotificationConfig` + event loop. |
| Cancel subagents policy | `[ui] cancel_subagents_on_turn_cancel` | `ask` / `always_stop` / `always_continue`. | **present** (runtime) | `dispatch/turn.rs` maps the string. |
| Force plan approval | `[ui] require_plan_approval` | Raw toml; always open plan even in always-approve. | **present** | `load_require_plan_approval` reads **raw** toml (not a `UiConfig` field). Event loop assigns `app.require_plan_approval`. |
| Permission / appearance stock rows | `compact_mode`, `screen_mode`, `theme` (non-doge), `vim_mode`, `simple_mode`, `show_thinking_blocks`, `permission_mode`, scroll knobs, etc. | Upstream settings. | **present** | All 47 `defs.rs` keys still register. |

`show_thinking_blocks` is present. It is **not** `always_expand_thinking`. Showing the block and keeping it expanded are different keys.

## User-guide

Product guide: `crates/codegen/xai-grok-pager/docs/user-guide/`. Host copy: `~/.grok/docs/user-guide/`. Same xAI restack body.

| Claim that should be in the guide | Status |
|-----------------------------------|--------|
| `[ui] hide_header` | **docs-only miss** (no hits) |
| DOGE default theme | **docs-only miss**. `06-theming.md` lists five themes; default named GrokNight |
| `[ui] economic_mode`, `[token_economy]` | **docs-only miss** |
| `[subagents] allow_worktree` | **docs-only miss** (`16-subagents.md` has no key) |
| Dual-auth / included SuperGrok period limits keys | **docs-only miss** |
| Auto-compact default 95 | **docs lie**: guide says 85 |
| `/limits` | Already named in the chrome postmortem. Still zero hits. |

## Leftovers this walk did not finish

- Pre-restack `git show` / `git log -S` of `UiConfig` and `settings/defs.rs`. This explore agent has no shell. The deciding check is a read-only `git log -S hide_header -- crates/codegen/xai-grok-pager/src/settings/defs.rs` on Surmount `main` vs this onto tip.
- Every upstream `[features]`, `[cli]`, `[models]`, `[memory]`, `[compat]`, `[workflows]`, MCP, and `pager.toml` stock key. Those were not the operator's "config options missing" report. Stock `/settings` rows still register.
- Whether `/theme` cycle order vs Settings default `"groknight"` has already rewritten a live `config.toml` on this host. Deciding check: read the operator's `~/.grok/config.toml` `[ui] theme` (do not dump secrets).
- Whether `require_plan_approval` was ever a `UiConfig` field (today it is raw-toml only). Unproven without history.
- Host skill text vs product `allow_worktree` (skill may still document a force-none the spawn path no longer does).
- Dual-auth hop empty `failover_api_keys`: config key is read; spend hop is not. Covered in `.agents/reports/fork-loss-postmortem-2026-08-13.md`, not re-litigated here.

## Counts

| Bucket | N | What is counted |
|--------|---|-----------------|
| **missing** | **22** | Operator surfaces FORK/docs promised that `/settings` and/or the shipped guide do not expose: hide_header, always_expand_thinking, plan_approval_park, allow_worktree, scrub_ascii_punct, bubble_copy_buttons, economic_mode settings row, auto_run_implement settings row, resume_canceled settings row, eight `[token_economy]` settings rows, auto_compact settings row, two recap settings keys (`[ui.notifications]` pair counted as one row plus `[features] session_recap`), cancel_subagents settings row, DOGE in `THEME_CHOICES`, dual-auth/docs cluster counted as one guide miss |
| **unread** | **6** | Fields that deserialize with no consumer of the named contract: `hide_header`, `always_expand_thinking`, `plan_approval_park`, `allow_worktree`, `scrub_ascii_punct` (startup seed), `bubble_copy_buttons` |
| **still present** | **18** | Fork-claimed keys with a real reader: permission_mode, default_selected_permission, remember_tool_approvals, economic_mode runtime, auto_run_implement runtime, resume_canceled runtime, `[token_economy]` toml, auto_use_included_limits, preferred_method, allow_spend_when_free_period_debit_unproven, management_team_id, title.enabled, theme=doge via `/theme`/toml/unset, auto-compact code default 95, features.session_recap, notifications.session_recap, cancel_subagents runtime, require_plan_approval raw toml |

Unread keys are also missing from `/settings`. They are not double-counted in the return line: **22 missing surfaces, 6 unread fields, 18 still-present keys**.
