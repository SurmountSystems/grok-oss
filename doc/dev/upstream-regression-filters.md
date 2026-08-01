# Upstream regression filters

**Role:** durable catalog of cargo (and shell) filters that harden Surmount
fork contracts against xAI **import** / **put-history** / **join**.
**Not D0 residual.** RESIDUAL § *Validate honesty* may demote; this file +
[`FORK.md`](../../FORK.md) § *Upstream regression filters* keep the commands.

**Authority for path restore:** `scripts/import-upstream-export.sh` (`FORK_PATHS`)
+ `scripts/assert-process-pins.sh`.
**Authority for product seams inside `xai-grok-*`:** cherry-picks on onto +
**these tests** (assert does not run them).

---

## Why these tests exist

Import restores only listed paths. Shared crate trees and the pager user-guide
take the xAI base on import; onto re-applies product via cherry-pick and
conflict resolve. A silent drop of DOGE-as-default, titles-on, stuck-retry
clear, dual-auth hop, or shell-reserved slash names will **not** fail
`assert-process-pins`. Cargo filters below encode the Surmount contracts so
recon cannot ship a hollow tree without a red test.

| Layer | What survives how |
|-------|-------------------|
| Process docs / scripts / packaging | `FORK_PATHS` restore + assert |
| Seams inside `xai-grok-*` | Cherry-pick + **cargo tests** |
| Host `~/.agents` / `~/.grok/AGENTS.md` | Outside the tree (untouched by import) |
| Shared user-guide | Conflict resolve on onto (not frozen wholesale) |

---

## Process pin (shell, not cargo)

| Gate | How |
|------|-----|
| Process pins present | `./scripts/assert-process-pins.sh` or `just upstream-assert-process-pins` (+ optional `HEAD` / onto tip) |

Assert checks required files/dirs and light content sniffs (AGENTS coordinator
pin, FORK upstream words, README Grok OSS). It does **not** check DOGE default,
window-title contracts, or residual filter names.

---

## Product filter catalog

Paths below are crate-relative module paths as rustc / `cargo test` see them;
use the filter substring for nextest. Prefer the **filter blocks** at the end
of each section for day-to-day recon.

### shell_collision / SHELL_RESERVED

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `slash::commands::shell_collision_contract_covers_every_pager_command_and_alias` | Every pager slash name/alias is in static `SHELL_RESERVED` (includes `clear-completed-todos`). Filter: `shell_collision` |

```bash
cargo test -p xai-grok-pager --lib -- shell_collision
```

### Stuck retry / StreamResumed / headers timeout / transport footer

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `app::acp_handler::tests::session_events::retry_chrome_clears_when_retry_stream_starts` | `RetryState::StreamResumed` clears sticky `TurnActivity::Retrying` |
| `xai-grok-pager` `views::turn_status::clip_retry_reason_does_not_strand_bare_error_word` | Footer clip hygiene |
| `xai-grok-pager` `views::turn_status::clip_retry_reason_keeps_short_human_label` | Short human labels |
| `xai-grok-pager` `views::turn_status::retrying_activity_label_uses_clipped_reason` | Activity label uses clip |
| `xai-grok-shell` `session::acp_session_tests::replay_buffer_send_update_tests::stream_started_emits_retry_state_stream_resumed` | Stream start emits StreamResumed retry state |
| `xai-grok-sampler` `actor::request_task::wait_before_attempt_aborts_on_cancel` | Esc cancels shared cooldown wait |
| `xai-grok-sampler` `actor::request_task::retry_footer_reason_uses_short_transport_label` | Short transport footer (not opaque `Transport error: error`) |
| `xai-grok-sampler` `client::tests::stream_headers_timeout_defaults_to_120_secs_when_env_unset` | Default stream headers timeout is **120s** when env unset (`0` / invalid → 120; positive override honored) |
| `xai-grok-sampler` **integration** `stream_headers_timeout::streaming_execute_times_out_waiting_for_headers` | Hang after accept, no headers → fail within headers budget (`GROK_STREAM_HEADERS_TIMEOUT_SECS=1` in that binary) |

```bash
cargo test -p xai-grok-pager --lib -- retry_chrome_clears clip_retry_reason retrying_activity_label
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout
```

**Note:** unit locks the **default** headers timeout constant (**120s** when env
unset). Integration proves timeout *works* under env=1. Do not treat env=1
alone as proof of the product default.

### hide_header vs window titles (+ title items)

| path::test | Contract |
|------------|----------|
| `xai-grok-shared` `ui_config::hide_header_defaults_false_and_parses` | `hide_header` default false + serde |
| `xai-grok-shared` `ui_config::stale_hide_title_bar_key_is_ignored` | Removed key ignored on deserialize |
| `xai-grok-pager` `app::window_title_always_manages_non_empty_branded_osc` | Always manage OSC; branded non-empty |
| `xai-grok-pager` `app::window_title_osc_payload_never_empty_string` | Never empty OSC payload |
| `xai-grok-pager` `app::titles_on_session_name_osc_is_non_empty_branded` | Session OSC branded |
| `xai-grok-pager` `notifications::title_updates_gated_only_by_title_enabled` | Opt-out = `title.enabled` only |
| `xai-grok-pager` `notifications::title::default_title_items_include_agents` | Default items include session-name/agents |
| `xai-grok-pager` `notifications::title::title_escape_never_empty_payload` | Dynamic OSC never empty |
| `xai-grok-pager` `views::agent::hide_header_zeroes_status_bar_height` | In-app status bar height 0 |
| `xai-grok-pager` `views::welcome::hide_header_zeros_welcome_top_bar_height` | Welcome top bar |
| `xai-grok-pager` `views::dashboard::layout::hide_header_zeroes_header_and_header_gap` | Dashboard header |
| settings_e2e `hide_header_*` | Settings registry + UI toggle (in-app only) |

```bash
cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title
cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session default_title_items title_state notifications::
cargo test -p xai-grok-pager --test settings_e2e -- hide_header
```

### DOGE default theme

| path::test | Contract |
|------------|----------|
| `xai-grok-pager-render` `theme::cache::default_theme_is_doge` | Unset → DOGE |
| `xai-grok-pager-render` `theme::cache::resolve_from_config_no_config_returns_doge` | No config → DOGE |
| `xai-grok-pager-render` `theme::cache::resolve_auto_dark_system_returns_doge` | Auto dark → DOGE |
| `xai-grok-pager-render` `theme::system_appearance::to_theme_kind_dark_defaults_to_doge` | Dark map → DOGE |
| Plus many `theme::doge::*` / `syntax::*doge*` purity tests | Palette contracts |

```bash
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config theme doge
```

### Human green rail + DOGE semantic roles

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `user_prompt_block_accent_is_static_human_rail` | All Human prompt kinds return static left accent |
| `xai-grok-pager` `user_prompt_block_accent_is_green_rail_under_doge_default` | DOGE Human rail pure green |
| `xai-grok-pager` `user_prompt_prefix_matches_human_rail_color` | Pointer and rail share Human token |
| `xai-grok-pager` `recap_accent_and_bullet_use_neutral_tool_color_when_idle` | Recap idle rail stays `accent_tool` (white on DOGE) |
| `xai-grok-pager-render` `doge_accent_user_is_pure_green_for_human` | `accent_user` = `#00FF00` |
| `xai-grok-pager-render` `doge_accent_system_is_pure_cyan_for_system_limits_credits` | `accent_system` = `#00FFFF` |
| `xai-grok-pager-render` `doge_roles_green_cyan_no_blue_ui_no_gray_text` | Role map + no gray UI slots |

```bash
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_prefix_matches recap_accent
cargo test -p xai-grok-pager-render --lib -- doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan doge_roles
```

### Clear completed todos (fork-adjacent)

| path::test | Contract |
|------------|----------|
| tools `todo::clear_completed_archives_done_and_cancelled_leaves_open` (+ siblings) | Archive math |
| pager `dispatch::tests::router::clear_completed_todos_*` | Effect not merge:false wipe |
| pager `agent_view::panes::clear_completed_todos_x_key_only_when_todo_pane_focused` | Focused X |
| `shell_collision` (above) | Slash reserved |

```bash
cargo test -p xai-grok-pager --lib -- clear_completed_todos shell_collision
```

### Always-on bubble copy / one-click copy

```bash
cargo test -p xai-grok-pager --lib -- bubble_copy_
```

### Surmount / OSS identity (sparse)

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `client_identity::product_cli_name_is_grok_oss` | CLI name |
| `xai-grok-shell` openrouter `referer_is_surmount_*` / `title_is_grok_oss` | OpenRouter attribution |
| shell tests `openrouter_attribution::referer_is_surmount_grok_oss_not_xai` | Same |

### Other high-value fork contracts (keep)

Dual-auth hop + multi SuperGrok + `/limits`; `interject_contract_*`;
`auto_compact_completed_preserves_todo_board`; skills order
(`agents_home_skills_shadow_grok_user_skills`,
`local_agents_skills_shadow_local_grok_skills`); UDAX toon filters; plan
soft-park filters. Full residual-aligned blocks below.

---

## Residual-aligned filter blocks (Validate honesty mirror)

Same commands as RESIDUAL § *Validate honesty* so open residual and this catalog
stay in sync when residual still lists them. Prefer editing **this file** when
a shipped neighbor leaves D0.

### Open residual + dual-auth regression

```bash
# 1. UDAX T0–T6
cargo test -p xai-grok-tools --lib -- toon json_to_toon dynamic_to_prompt free_text densify_mcp densify_structured task_output_handoff subagent_completed_handoff

# 2. Dual-auth (session ↔ console key hop + live re-bind + multi-add)
cargo test -p xai-grok-shell --lib -- resolve_credentials enforce_disable_api_key store_and_load_round_trip fingerprint_is_not_raw_key multi_add
cargo test -p xai-grok-sampler --lib -- rotate_ exhausted memo fingerprint hop_reason live_rebind
cargo test -p xai-grok-pager --lib -- login_ dual_auth_hop_reason
cargo test -p xai-grok-sampling-types --lib -- credit_exhausted

# 2b. Multi SuperGrok principals + live ranking + dual /limits + sibling poll
cargo test -p xai-grok-shell --lib -- upsert_personal_then_business team_login_then_personal_keeps dual_supergrok load_supergrok_candidates two_principals_billing enrich_candidates principal_limits_label non_active_poll_targets remember_both_principals included_usage poll_non_active_remembers
cargo test -p xai-grok-pager --lib -- format_dual_principals live_console_omits extra_principals_hook show_limits format_supergrok_session footer_names_live_principal

# 3. DOGE default / Human green rail + role map / hide_header / window titles / title items + bubble + clear-done
cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config theme doge doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_prefix_matches recap_accent
cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session default_title_items title_state notifications::
cargo test -p xai-grok-pager --test settings_e2e -- hide_header
cargo test -p xai-grok-pager --lib -- bubble_copy_ clear_completed_todos

# 4. Plan soft-park A
cargo test -p xai-grok-pager --lib -- plan softer_park toast focus_plan plan_approval soft_park

# 5. session_reader / plan_validate / bulk_edit intercepts
cargo test -p xai-grok-tools --lib -- session_reader plan_validate bulk_edit_policy implement_memory opencode edit

# 5b. TUI self-screenshot
cargo test -p xai-grok-pager-render --lib -- tui_screenshot
cargo test -p xai-grok-pager --lib -- screenshot:: capture_tui_screenshot try_attach_tui_screenshot

# 5c. Stuck Retrying chrome + stream headers timeout + transport footer + shell_collision
cargo test -p xai-grok-pager --lib -- retry_chrome_clears clip_retry_reason retrying_activity_label
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout
cargo test -p xai-grok-pager --lib -- shell_collision
```

### Shipped neighbors (smoke if touching shared files)

```bash
# 6–8. Soft interject + btw + plan entry
cargo test -p xai-grok-shell --lib -- interject handle_interject
cargo test -p xai-grok-pager --lib -- interject force_interject cancel_turn
cargo test -p xai-grok-pager --lib -- btw
cargo test -p xai-grok-tools --lib -- enter_plan_mode
cargo test -p xai-grok-workspace --lib -- enter_plan_mode_not_auto enter_plan_mode_fast_path

# 9. usage.jsonl
cargo test -p xai-grok-shell --lib -- usage_log record_response_token_usage

# 10–11. Full gate + process pins
just check
./scripts/assert-process-pins.sh
```

---

## Operator cheat sheet (post-recon)

Minimum after import restore or onto tip land:

```bash
./scripts/assert-process-pins.sh
./scripts/assert-process-pins.sh HEAD   # or onto tip

# Core product harden (from this catalog)
cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config
cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session default_title_items shell_collision retry_chrome_clears
cargo test -p xai-grok-pager --test settings_e2e -- hide_header
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout

just check   # full gate before push/PR
```

**User-guide on onto:** shared path under
`crates/codegen/xai-grok-pager/docs/user-guide/` is **not** in `FORK_PATHS`.
Resolve conflicts for DOGE default, window titles / `title.enabled` vs
`hide_header`, and Grok OSS branding sections; do not wholesale-pin the guide
to Surmount.

---

## Related

| Path | Role |
|------|------|
| [`FORK.md`](../../FORK.md) § *Upstream regression filters* | D1 one-page cheat + recon table |
| [`RESIDUAL.md`](../../RESIDUAL.md) § *Validate honesty* | D0 open residual mirror (may demote) |
| [`docs/upstream-history.md`](../../docs/upstream-history.md) | Import review checklist |
| `scripts/assert-process-pins.sh` | Path presence gate |
| `doc/dev/research/fork-paths-hardening-2026-07-24.md` | Why FORK_PATHS + assert (list authority = import script) |

*Catalog created 2026-07-30 from explore join inventory.*
