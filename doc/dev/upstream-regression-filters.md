# Upstream regression filters

**Role:** durable catalog of cargo (and shell) filters that harden Surmount
fork contracts against xAI **import** / **put-history** / **join**.
**Not D0 residual.** RESIDUAL § *Validate honesty* may demote; this file +
[`FORK.md`](../../FORK.md) § *Upstream regression filters* keep the commands.

**Authority for path restore:** `grok-nix-helper import-upstream-export` (`FORK_PATHS`)
+ `grok-nix-helper assert-process-pins`.
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
| Process docs / scripts / packaging | `FORK_PATHS` restore + assert (files only) |
| Seams inside `xai-grok-*` | Cherry-pick + **named cargo tests** (assert does not run them) |
| **Seven land classes** | CLI identity, `/settings` + unread config, grok-oss ledger `/spend`, DOGE/chrome paint, dual-auth hop after included SuperGrok period limits are full, last-session on start, product skills are not a Python runtime. A chrome-only pass is a failed land. |
| **Paint / dogfood** | After catalog: named `fn` still exist, then screenshot / draw list (rails, four idle plan CTAs, included SuperGrok period limits meter, titled composer white frame, SIGUSR1 after a failed install). Screenshots are an operator check, not the only check. |
| Host `~/.agents` / `~/.grok/AGENTS.md` | Outside the tree (untouched by import) |
| Shared user-guide | Conflict resolve on onto (not frozen wholesale). A guide with zero `/limits` hits is a failed land. |

---

## Process pin (shell, not cargo)

| Gate | How |
|------|-----|
| Process pins present | `grok-nix-helper assert-process-pins` or `just upstream-assert-process-pins` (+ optional `HEAD` / onto tip) |

Assert checks required files/dirs and light content sniffs (AGENTS coordinator
pin, FORK upstream words, README Grok OSS). It does **not** check DOGE default,
window-title contracts, CLI first-token identity, `/spend` ingest, `/settings`
rows, or residual filter names.

---

## Required land inventory (seven classes)

After put-history / onto / import / join, do not claim "Surmount seams
survived" until `just upstream-assert-process-pins` **and** the named tests
in this section exist and pass. `just check` cannot fail a deleted catalog
test. A chrome-only inventory is a failed land.

**Helper-green is a failed land.** Forbidden as proof:

| Lie | Why it stayed green after 1.0.3 |
|-----|----------------------------------|
| `--version` stdout contains substring `grok` | `grok 1.0.3 (…)` matches. First token must be `grok-oss`. |
| Catalog file exists / theme file exists | Theme file is not paint. |
| `grok_oss.db` schema v1 exists | Schema without `/spend` ingest writes `DoubleEntryReport::default()`. |
| `hide_header` serde default test | Field deserializes. `/settings` had no row and nothing read it. |
| Dual-auth rank helpers | Ranking is not `sampling_config` hop keys after included SuperGrok period limits are full. |
| Bundle cache still contains `memory.py` | Allowlisted intercept stubs are not a license to ship review helpers or invent new `.py`. |

`rg` each identifier below for a matching `fn` before cargo. Missing `fn` =
land failed. Do not delete a red catalog test to finish a compile mop.

### 1. CLI identity

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `client_identity::product_version_line_uses_grok_oss_not_bare_grok` | `--version` line first token is `grok-oss` (operator saw `grok 1.0.3`) |
| `xai-grok-pager` `client_identity::resume_session_command_uses_grok_oss` | Resume paste is `grok-oss --resume`, not `grok --resume` |
| `xai-grok-pager` `docs::user_guide_resume_and_version_examples_use_grok_oss` | User-guide `--version` / `--resume` examples use `grok-oss` |
| `xai-grok-pager` `docs::user_guide_operator_cli_examples_use_grok_oss` | User-guide leftover operator examples use `grok-oss`, not bare `grok login` / `grok sessions` |
| `xai-grok-pager` `client_identity::product_cli_name_is_grok_oss` | Product CLI name constant is `grok-oss` |
| `xai-grok-pager` `print_exit_resume_hint_writes_expected_lines` | Quit / relaunch hint uses `grok-oss --resume` |
| `xai-grok-pager` `welcome_badge_brands_grok_oss` | Welcome badge says **Grok OSS**, not leftover Grok Build |
| `xai-grok-pager` `hero_subtitle_brands_grok_oss` | Welcome hero thanks-line brands Grok OSS |
| `xai-grok-pager` `tutorial_list_title_brands_grok_oss` | Tutorial list title brands Grok OSS |
| `xai-grok-pager-bin` `--test version_without_tty` | No-TTY `--version` first token is `grok-oss` (`assert_version_ok`; not substring `grok`) |

```bash
cargo test -p xai-grok-pager --lib -- product_version_line_uses_grok_oss_not_bare_grok \
  resume_session_command_uses_grok_oss user_guide_resume_and_version_examples_use_grok_oss \
  user_guide_operator_cli_examples_use_grok_oss product_cli_name_is_grok_oss \
  print_exit_resume_hint_writes_expected_lines welcome_badge_brands_grok_oss \
  hero_subtitle_brands_grok_oss tutorial_list_title_brands_grok_oss
cargo test -p xai-grok-pager-bin --test version_without_tty
```

### 2. Config is a surface, not a field

A toml field that deserializes is not shipped if `/settings` has no row and
no runtime reader. Serde default tests are not this class.

| path::test | Contract |
|------------|----------|
| settings_e2e `hide_header_*` / `always_expand_thinking_*` / `scrub_ascii_punct_*` / `allow_worktree_*` / `bubble_copy_buttons_*` / `plan_approval_park_*` | `/settings` rows dispatch the typed setters (unread restack keys) |
| `xai-grok-pager` `settings::registry::theme_choices_include_doge_and_default_is_doge` | Theme picker includes DOGE; default is `doge` (not `groknight`) |
| `xai-grok-pager` `hide_header_zeroes_*` / `hide_header_zeros_*` | `hide_header` zeros status / welcome / dashboard chrome |
| `xai-grok-pager` `always_expand_thinking_keeps_blocks_expanded` | Always-expand thinking is read at paint |
| `xai-grok-pager` `always_expand_thinking_off_paints_collapsed_headers` | Off paints collapsed Thought-for headers, including while running |
| `xai-grok-pager` `always_expand_thinking_finish_overrides_sticky_collapsed` | Finish honors always-expand over session sticky |
| `xai-grok-pager` `always_expand_thinking_flip_rematerializes_stacked_thinking` | Settings flip rematerializes stacked thinking rows |
| `xai-grok-pager` `set_always_expand_thinking_refolds_live_thinking_in_parent_and_nested_overlay` | Parent and nested overlay thinking rematerialize live |
| `xai-grok-pager` `bubble_copy_buttons_on_paints_copy_icon` | Bubble copy chrome reads the flag |
| `xai-grok-pager` `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width` | A full-width first line still paints the always-on copy glyph |
| `xai-grok-pager` `append_bubble_copy_button_paints_when_first_line_fills_content_width` | The paint helper still marks a hit column when the first line fills the width |
| `xai-grok-pager` `clicking_human_bubble_copy_copies_the_prompt` | Clicking the always-on human bubble copy glyph copies that prompt. Paint-only chrome is a failed land |
| `xai-grok-pager` `clicking_assistant_bubble_copy_copies_the_message` | Clicking the always-on assistant bubble copy glyph copies that message |
| `xai-grok-pager` `clicking_wide_human_bubble_copy_still_paints_and_copies` | A full-width first line still paints a clickable copy glyph |
| `xai-grok-pager-render` `prime_applies_scrub_ascii_punct_from_ui` | ASCII scrub is seeded from config at launch |
| `xai-grok-shell` `resolve_subagents_copies_allow_worktree` | `[subagents] allow_worktree` is copied onto the runtime config |

```bash
cargo test -p xai-grok-pager --test settings_e2e -- hide_header always_expand_thinking \
  scrub_ascii_punct allow_worktree bubble_copy_buttons plan_approval_park
cargo test -p xai-grok-pager --lib -- theme_choices_include_doge_and_default_is_doge \
  hide_header_zeroes always_expand_thinking bubble_copy_buttons_on \
  append_bubble_copy_button_paints clicking_human_bubble_copy clicking_assistant_bubble_copy \
  clicking_wide_human_bubble_copy
cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub_ascii_punct_from_ui
cargo test -p xai-grok-shell --lib -- resolve_subagents_copies_allow_worktree
```

### 3. grok-oss SQL extras (Token Economy ledger `/spend`; extra SQL, not SuperGrok dollar credits)

`$GROK_HOME/grok_oss.db` is the Token Economy ledger, not the session store.
This class is extra SQL in that ledger (more tables than the session store).
It is not SuperGrok dollar credits. The heading keeps `SQL extras` so
`assert-process-pins` still matches. Do not teach extras as a SuperGrok
dollar-credits nickname.
`open_creates_schema_and_version` is not this class.

| path::test | Contract |
|------------|----------|
| `xai-grok-shell` `token_economy::spend_path_ingests_usage_jsonl_and_records_reconciliation` | Spend path ingests `usage.jsonl` into `local_usage_event` and writes `reconciliation_run` |
| `xai-grok-pager` `show_spend_ingests_usage_jsonl_and_is_not_empty_default` | `/spend` formats the live ledger, not `DoubleEntryReport::default()` |

```bash
cargo test -p xai-grok-shell --lib -- spend_path_ingests_usage_jsonl_and_records_reconciliation
cargo test -p xai-grok-pager --lib -- show_spend_ingests_usage_jsonl_and_is_not_empty_default
```

### 4. DOGE / Surmount chrome (paint)

Theme file existing is not paint. Screenshot/dogfood list stays an operator
check after these `fn`s exist.

| path::test | Contract |
|------------|----------|
| `user_prompt_block_accent_*` + `user_prompt_entry_renderer_paints_green_rail` | Human green left rail actually paints |
| `paint_composer_box_cursor_uses_human_green_not_agent_magenta` + `focused_composer_paints_human_green_box_caret_*` + `doge_human_box_caret_plate_is_rgb_0_255_0` | Box caret is Human green, never agent magenta; DOGE plate is `Rgb(0,255,0)` not named ANSI Green |
| `agent_message_block_accent_is_magenta_rail_under_doge_while_running` | Running agent rail is magenta |
| `info_line_model_name_uses_accent_model_not_gray` | Model label uses `accent_model` (magenta under DOGE) |
| `status_bar_pushes_credits_compact_included_supergrok_period_limits` | Status bar pushes `"credits"` and paints `included SuperGrok period limits · N%` |
| `hit_credits_click_dispatches_show_limits` | Click on the compact meter dispatches `ShowLimits` |
| `titled_doge_composer_frame_is_prompt_border_not_context_yellow` | Titled composer frame is `prompt_border_active` (white); title only is yellow |
| `plan_approval_footer_paints_five_cta_vocabulary` | Idle plan panel footer paints Approve / Comment / Revise / Exit. Clarify is only after Comment, not an idle top-level CTA |
| `default_theme_is_doge` | Unset theme resolves to DOGE |
| `forked_session_status_header_paints_switcher_and_dashboard` | Forked-session upper-left status header paints `[‹][›]` plus `[Dashboard]` (11:10 screenshot was git plus cwd only) |
| `forked_session_status_header_clicks_open_dashboard_and_cycle` | Header `[Dashboard]` opens the dashboard; `[‹]`/`[›]` cycle the fork family |
| `load_session_restores_fork_family_from_disk` | Resume of a persisted fork loads the parent and stamps `forked_from` so header paint can run |

Closest existing spinner/glyph neighbors (not a lower-left magenta throbber
paint `fn`; do not catalog the missing names `doge_idle_subagent_still_running`
/ `doge_tool_running_spinner`):
`doge_activity_spinners_use_striped_down_marquee_not_braille`,
`idle_with_subagents_renders_still_running_cue`.

```bash
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config \
  doge_accent_user_is_pure_green as_doge_human_green_named_ansi_is_rgb_0_255_0 \
  osc12_named_ansi_green_is_doge_rgb_0_255_0
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_entry_renderer_paints_green_rail \
  paint_composer_box_cursor_uses_human focused_composer_paints_human_green_box_caret \
  doge_human_box_caret_plate_is_rgb_0_255_0 paint_composer_box_cursor_named_ansi_green_becomes_doge_rgb \
  agent_message_block_accent info_line_model_name_uses_accent_model \
  status_bar_pushes_credits_compact_included_supergrok_period_limits \
  hit_credits_click_dispatches_show_limits \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  plan_approval_footer_paints_five_cta_vocabulary \
  forked_session_status_header_paints_switcher_and_dashboard \
  forked_session_status_header_clicks_open_dashboard_and_cycle \
  load_session_restores_fork_family_from_disk
```

### 5. Dual-auth hop after included SuperGrok period limits are full

Rank-only tests in `supergrok_identity_rank.rs` are not this class.

Do not flatten remaining to zero from `usagePct` / `creditUsagePercent` 100
plus missing SuperGrok Heavy. Never invent used-up included SuperGrok period
limits from that snapshot. SuperGrok Heavy ranking optional label is not
this class. Human prose next to `*_extras*` identifiers says SuperGrok
dollar credits.

| path::test | Contract |
|------------|----------|
| `sampling_config_auto_use_fills_console_hop_after_included_full` | `sampling_config` fills console failover when included SuperGrok period limits are full |
| `sampling_config_auto_use_omits_console` / `sampling_config_auto_use_omits_console_while_supergrok_included_headroom` | While included SuperGrok period limits still have room, stay on SuperGrok (no console hop) |
| `resolve_model_to_sampling_config_auto_use` | Resolve path uses the same auto-use hop policy |
| `sampling_config_auto_use_extras_keep_session_console_failover` | SuperGrok dollar credits keep session plus console failover (single SuperGrok identity) |
| `sampling_config_hops_to_sibling_included_before_extras` | Personal included SuperGrok period limits full hops to Business included before SuperGrok dollar credits |
| `sampling_config_hop_team_remaining_personal_exhausted_not_dollars_or_console` | Team included remaining + personal exhausted stays Team, not SuperGrok dollar credits or console |
| `sampling_config_hop_personal_remaining_team_exhausted` | Personal included remaining + Team exhausted hops to personal |
| `sampling_config_hop_both_remaining_team_first_then_personal` | Both included remaining: Team / Business first, then personal |
| `sampling_config_hop_both_included_exhausted_dollar_credits_before_console` | Both included exhausted: SuperGrok dollar credits before console |
| `sampling_config_hop_missing_heavy_false_100_keeps_sibling_included` | usage 100 + missing SuperGrok Heavy does not flatten sibling included remaining |
| `sampling_config_hop_dollar_credits_on_both_missing_heavy_keeps_team` | SuperGrok dollar credits on both + missing Heavy keeps Team included remaining |
| `afterburner_does_not_skip_mark_when_sibling_has_included_remaining` | After-burner skip of the out of included SuperGrok period limits mark only when every distinct included pool is exhausted |
| `align_after_billing_switches_sticky_personal_full_to_business_included` | After billing, `align_to_ranked_free_period_primary` switches sticky personal-full to Business included |
| `prepare_sampler_for_turn_aligns_to_ranked_included_primary` | Per-turn reconstruct uses the ranked included SuperGrok period primary JWT |
| `prepare_sampler_for_turn_does_not_flatten_missing_heavy_100_off_sibling` | Per-turn reconstruct does not flatten remaining from 100% + missing Heavy on the off sibling |
| `prepare_sampler_for_turn_does_not_flatten_dollar_credits_on_both` | Per-turn reconstruct does not flatten remaining when SuperGrok dollar credits sit on both identities |
| `pick_prefers_business_included_before_personal_when_both_have_remaining` | When both stored SuperGrok logins still have included remaining, pick Business / Team first |
| `order_credentials_business_included_before_personal_when_both_have_room` | Credential order spends Business included before personal included while both have room |
| `limits_snapshot_second_process_reads_file_and_does_not_http` | Second grok-oss process reads `$GROK_HOME/limits_snapshot.json` and does not call SuperGrok billing HTTP |
| `limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once` | A stale snapshot lets the next exclusive-flock holder fetch once |
| `limits_snapshot_never_writes_access_tokens` | Shared snapshot never stores JWTs or API keys |
| `billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http` | `x.ai/billing` uses the snapshot hub instead of unconditionally HTTP-ing siblings |

Rank neighbors (not hop by themselves; do not treat these as class 5 proof):
`hop_does_not_switch_to_console_while_stored_business_included_remaining`,
`hop_team_included_remaining_personal_exhausted_not_dollar_credits_or_console`,
`hop_personal_included_remaining_team_exhausted_to_personal`,
`hop_both_included_remaining_team_business_first_then_personal`,
`hop_both_included_exhausted_supergrok_dollar_credits_before_console`,
`hop_missing_heavy_or_false_100_does_not_exhaust_sibling_with_remaining`,
`hop_dollar_credits_on_both_missing_heavy_keeps_team_remaining`,
`hop_dollar_credits_on_both_missing_heavy_keeps_personal_remaining`.

```bash
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use sampling_config_hops_to_sibling_included_before_extras \
  sampling_config_hop_team_remaining_personal_exhausted_not_dollars_or_console \
  sampling_config_hop_personal_remaining_team_exhausted \
  sampling_config_hop_both_remaining_team_first_then_personal \
  sampling_config_hop_both_included_exhausted_dollar_credits_before_console \
  sampling_config_hop_missing_heavy_false_100_keeps_sibling_included \
  sampling_config_hop_dollar_credits_on_both_missing_heavy_keeps_team \
  resolve_model_to_sampling_config_auto_use \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
  prepare_sampler_for_turn_does_not_flatten_missing_heavy_100_off_sibling \
  prepare_sampler_for_turn_does_not_flatten_dollar_credits_on_both \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  order_credentials_business_included_before_personal_when_both_have_room \
  limits_snapshot_second_process_reads_file_and_does_not_http \
  limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once \
  limits_snapshot_never_writes_access_tokens \
  billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http
```

### 5b. Combined remaining included SuperGrok period limits (distinct pools)

These are not hop keys. They lock the sum used by compact chrome and `/limits` driver while a sibling pool still has remaining.

| path::test | Contract |
|------------|----------|
| `combined_included_remaining_sums_distinct_personal_and_business_pools` | Distinct personal + Business pools sum remaining included SuperGrok period limits |
| `combined_included_remaining_does_not_double_count_unified_pool` | Wire `is_unified_billing_user` unified pool counts once (max remaining) |
| `combined_included_remaining_does_not_collapse_matching_percent_and_reset_into_one_pool` | Matching floored used percent and the same reset_at is not one remaining number |
| `matching_percent_and_reset_does_not_collapse_combined_remaining_into_one_pool` | Compact remaining / Active driver do not collapse matching independent polls |
| `compact_meter_stays_included_while_sibling_pool_has_remaining` | Compact chrome stays on included SuperGrok period limits while a sibling pool has remaining |
| `compact_chrome_names_meter_source_not_bare_percent` | Compact chrome names included SuperGrok period limits vs SuperGrok dollar credits vs console vs combined |
| `format_limits_active_line_names_meter_source` | `/limits` Active line names the meter-source pin |
| `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining` | Active spend driver stays included SuperGrok period limits while any distinct pool has remaining |

```bash
cargo test -p xai-grok-shell --lib -- combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool \
  combined_included_remaining_does_not_collapse_matching_percent_and_reset_into_one_pool
cargo test -p xai-grok-pager --lib -- compact_meter_stays_included_while_sibling_pool_has_remaining \
  compact_chrome_names_meter_source_not_bare_percent \
  format_limits_active_line_names_meter_source \
  active_spend_driver_stays_included_while_any_distinct_pool_has_remaining \
  matching_percent_and_reset_does_not_collapse_combined_remaining_into_one_pool
```

### 6. Last-session on start

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `materialize_new_auto_opens_last_session_when_one_exists` | Interactive `grok-oss` with a remembered last session for this working directory opens that session (no welcome) |
| `xai-grok-pager` `materialize_new_auto_stays_welcome_when_no_last_session` | First-ever use with no remembered last session stays on Welcome |
| `xai-grok-pager` `materialize_new_auto_does_not_open_last_when_headless` | Headless does not steal last-session |
| `xai-grok-pager` `from_pager_args_opens_last_session_on_start` | Interactive pager args set the last-session-on-start flag |

```bash
cargo test -p xai-grok-pager --lib -- materialize_new_auto_opens_last_session_when_one_exists \
  materialize_new_auto_stays_welcome_when_no_last_session \
  materialize_new_auto_does_not_open_last_when_headless \
  from_pager_args_opens_last_session_on_start
```

### 7. Product skills are not a Python runtime

A restack that installs non-excepted Python under product skills, or that
drops the Rust intercept for the allowlisted CLI forms, is a failed land.
User-guide `08-skills.md` must keep that sentence. The host overlay under
`~/.agents/skills` is operator-owned and is not this class.

| path::test | Contract |
|------------|----------|
| `xai-grok-bundle` `sanitize_rejects_non_excepted_skill_python` | Bundle path sanitize rejects junk `.py`; keep only intercept CLI stubs and office/docx/pptx/xlsx/pdf |
| `xai-grok-bundle` `extract_archive_skips_non_excepted_skill_python` | Network archive extract does not write non-excepted `.py` into the bundled cache |
| `xai-grok-bundle` `product_repo_skill_roots_have_no_non_excepted_python` | Project `.agents/skills` and `.grok/skills` have no junk `.py` |
| `xai-grok-bundle` `default_product_skills_include_polish_and_subagent` | In-tree Grok OSS default skills include polish, subagent, what, and pull-remote-tree |
| `xai-grok-pager` `docs::user_guide_skills_are_not_a_python_runtime` | User-guide `08-skills.md` says skills are not a Python runtime and names the exceptions. `/polish` and `/subagent` are default Grok OSS skills, not project `.agents/skills` packs. |
| `xai-grok-tools` `implement_memory_snapshot_intercept_does_not_spawn_shell` | `memory.py` CLI is Rust; no Python process |
| `xai-grok-tools` `plan_validate_intercept_does_not_spawn_shell` | `validate-plan.py` CLI is Rust; no Python process |
| `xai-grok-tools` `session_reader_list_intercept_does_not_spawn_shell` | `session_reader.py` CLI is Rust; no Python process |

```bash
cargo test -p xai-grok-bundle --lib -- sanitize_rejects_non_excepted_skill_python \
  extract_archive_skips_non_excepted_skill_python \
  product_repo_skill_roots_have_no_non_excepted_python \
  default_product_skills_include_polish_and_subagent
cargo test -p xai-grok-pager --lib -- user_guide_skills_are_not_a_python_runtime
cargo test -p xai-grok-tools --lib -- implement_memory_snapshot_intercept_does_not_spawn_shell \
  plan_validate_intercept_does_not_spawn_shell session_reader_list_intercept_does_not_spawn_shell
```

### Extra restack-droppable neighbors (walk if listed; not a second numbered board)

These are not classes 8 through 14. Land still walks them because this catalog
lists the named tests. Paint-only bubble copy is already a failed land under
class 2 (click-to-copy rows).

#### Footer context chip names sampling vs catalog

AUTO compact gates on the sampling window. When that window is smaller than
the catalog window, the footer chip must name both. Do not paint unlabeled
catalog 500k as the AUTO gate. Session sampling comes from GetSessionInfo /
AutoCompactStarted. `refresh_context_used` must not copy catalog into that
field.

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `context_chip_names_sampling_window_when_catalog_differs` | Chip names sampling and catalog when they differ |
| `xai-grok-pager` `context_chip_hover_percent_uses_sampling_window_when_catalog_differs` | Hover percent is of the sampling window |
| `xai-grok-pager` `footer_chip_uses_session_sampling_window_when_economic_cache_is_off` | Footer chip uses the session sampling window when the pager economic cache is off |
| `xai-grok-pager` `refresh_context_used_does_not_copy_catalog_into_session_sampling` | `refresh_context_used` does not copy catalog 500k into session sampling |
| `xai-grok-shell` `main_session_sampling_window_is_catalog_500k_even_when_economic_is_on` | L1 spawn seeds catalog 500k |
| `xai-grok-shell` `nested_session_sampling_window_stays_200k_when_catalog_is_500k` | Nested spawn stays 200k |

```bash
cargo test -p xai-grok-pager --lib -- context_chip_names_sampling_window_when_catalog_differs \
  context_chip_hover_percent_uses_sampling_window_when_catalog_differs \
  footer_chip_uses_session_sampling_window_when_economic_cache_is_off \
  refresh_context_used_does_not_copy_catalog_into_session_sampling
cargo test -p xai-grok-shell --lib -- main_session_sampling_window_is_catalog_500k_even_when_economic_is_on \
  nested_session_sampling_window_stays_200k_when_catalog_is_500k
```

#### Plan present is not operator Approve

`exit_plan_mode` presents the plan. It is not operator Approve. Soft-park must
not steal mid-compose keys.

| path::test | Contract |
|------------|----------|
| `exit_plan_mode_present_is_not_operator_approve` | Presenting the plan is not operator Approve |
| `exit_plan_mode_tool_result_does_not_claim_operator_approval` | Tool result text does not claim the operator approved |
| `empty_enter_on_revise_prompt_does_not_approve` | Empty Enter on the Revise prompt does not Approve |
| `soft_park_empty_ctrl_c_abandons_plan_approval` | Empty-prompt Ctrl+C abandons the parked plan, not Approve |
| `exit_plan_mode_keeps_mid_compose_draft_and_a_types` | Mid-compose draft stays; `a` types into the composer |
| `exit_plan_mode_modal_park_does_not_steal_mid_compose_keys` | Modal park does not steal mid-compose keys |
| `exit_plan_mode_empty_present_printable_goes_to_composer` | Empty present: printable keys go to the composer |
| `exit_plan_mode_shows_overlay_even_in_yolo` | Always-approve permission mode still presents the plan overlay |

```bash
cargo test -p xai-grok-pager --lib -- exit_plan_mode_present_is_not_operator_approve \
  empty_enter_on_revise_prompt_does_not_approve \
  soft_park_empty_ctrl_c_abandons_plan_approval \
  exit_plan_mode_keeps_mid_compose_draft_and_a_types \
  exit_plan_mode_modal_park_does_not_steal_mid_compose_keys \
  exit_plan_mode_empty_present_printable_goes_to_composer \
  exit_plan_mode_shows_overlay_even_in_yolo
cargo test -p xai-grok-tools --lib -- exit_plan_mode_tool_result_does_not_claim_operator_approval
```

#### Clickable Approve must not drop the Human-box prompt

Mouse Approve on an isolated present is not Empty Enter on Revise.
`empty_enter_on_revise_prompt_does_not_approve` does not prove this.
Clickable Approve must keep the Human-box prompt and send it on the
implement turn (Interject), not vanish into an empty composer. These
`plan_approve_lost_prompt` names are fork-owned contracts. When the tests
change, keep the stronger assert; synthesize if upstream and Surmount both
have a piece; never fit the contract to a wipe.

| path::test | Contract |
|------------|----------|
| `isolated_present_preview_click_approve_does_not_drop_human_box_prompt` | Isolated present Preview, text already in the Human box; click Approve does not drop it |
| `isolated_present_preview_typed_after_present_click_approve_sends_human_box_prompt` | Empty at present, type in Preview, click Approve still sends the typed string |
| `isolated_present_prompt_focus_click_approve_does_not_drop_human_box_prompt` | Comment then Prompt focus, type, click Approve does not drop the Human-box prompt |
| `isolated_present_click_approve_dispatches_interject_with_prompt_text` | Click Approve dispatches Interject that carries the prompt text |

```bash
cargo test -p xai-grok-pager --lib -- \
  isolated_present_preview_click_approve_does_not_drop_human_box_prompt \
  isolated_present_preview_typed_after_present_click_approve_sends_human_box_prompt \
  isolated_present_prompt_focus_click_approve_does_not_drop_human_box_prompt \
  isolated_present_click_approve_dispatches_interject_with_prompt_text
```

#### L2 spawn prompt (2026-08-20)

Process law stays in AGENTS. These cargo names keep the product prompt from
teaching "MUST spawn L3 for all tool work" after a restack.

| path::test | Contract |
|------------|----------|
| `xai-grok-agent` `child_task_description_is_concise` | L2 task description says spawn L3 only if actually hard; easy work can stay on L2 |
| `xai-grok-tools` `default_max_allows_l2_to_spawn_l3` | Default max depth lets a depth-1 agent spawn L3 |

```bash
cargo test -p xai-grok-agent --lib -- child_task_description_is_concise
cargo test -p xai-grok-tools --lib -- default_max_allows_l2_to_spawn_l3
```

#### File-level infer-from-path verify

After ACP `search_replace` / `apply_patch`, the edit tool formats and
lints a written `.rs` file as that file. Argv must include the path. It
is not `cargo clippy -p <crate> --lib`, not `cargo fmt -p`, not
`just check`. Other extensions do not get Rust cargo. The command-running
tool still rejects crate-wide cargo. Kill switch: `GROK_SKIP_EDIT_VERIFY=1`.
A restack that drops `util/rust_edit_verify.rs` or these tests is a failed
land. Extra class, not one of the seven numbered land classes.

| path::test | Contract |
|------------|----------|
| `xai-grok-tools` `rustfmt_argv_edition_2024_config_and_absolute_files` | rustfmt argv is file-level (`rustfmt --edition 2024 ... <abs.rs>`), not `cargo fmt -p` |
| `xai-grok-tools` `clippy_argv_lints_the_edited_file_not_crate_lib` | clippy-driver argv includes the written path; not `cargo clippy -p ... --lib` |
| `xai-grok-tools` `clippy_argv_includes_bin_path_not_package_lib` | editing `src/bin/<name>.rs` still lints that path, not crate `--lib` |
| `xai-grok-tools` `clippy_argv_includes_integration_test_path_not_package_lib` | editing `tests/<stem>.rs` still lints that path |
| `xai-grok-tools` `clippy_argv_is_file_level_not_package_lib` | no `-p xai-grok-shell --lib` in the lint argv |
| `xai-grok-tools` `several_rust_writes_run_file_level_clippy_per_file` | flush runs file-level clippy-driver per edited file |
| `xai-grok-tools` `clippy_driver_uses_temp_out_dir_not_the_workspace_root` | clippy-driver `--out-dir` and cwd are a temp dir, not the workspace root |
| `xai-grok-tools` `write_refuses_rmeta_at_workspace_root_and_does_not_create_the_file` | `write` refuses `*.rmeta` at workspace root and does not create the file |
| `xai-grok-tools` `write_refuses_a_out_at_workspace_root_and_does_not_create_the_file` | `write` refuses `a.out` at workspace root |
| `xai-grok-tools` `write_refuses_rust_out_at_workspace_root_and_does_not_create_the_file` | `write` refuses `rust_out` at workspace root |
| `xai-grok-tools` `write_refuses_long_type_dump_at_workspace_root_and_does_not_create_the_file` | `write` refuses `*.long-type-*.txt` at workspace root |
| `xai-grok-tools` `search_replace_refuses_a_out_at_workspace_root_and_does_not_create_the_file` | `search_replace` refuses `a.out` at workspace root |
| `xai-grok-tools` `apply_patch_refuses_add_rmeta_at_workspace_root_and_does_not_create_the_file` | `apply_patch` refuses adding `*.rmeta` at workspace root |
| `xai-grok-tools` `rustc_oneshot_without_out_dir_is_refused_and_does_not_spawn_shell` | `rustc foo.rs` is refused; rustc does not spawn |
| `xai-grok-tools` `rustc_stdin_rust_out_is_refused_and_does_not_spawn_shell` | `rustc -` (writes `rust_out`) is refused |
| `xai-grok-tools` `rustc_dash_o_a_out_at_workspace_root_is_refused_and_does_not_spawn_shell` | `rustc -o a.out` at workspace root is refused |
| `xai-grok-tools` `redirect_rmeta_at_workspace_root_is_refused_and_does_not_spawn_shell` | shell redirect to `*.rmeta` at workspace root is refused |
| `xai-grok-tools` `dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell` | `cargo fmt --all` is refused; cargo does not spawn |
| `xai-grok-tools` `dangerous_cargo_fmt_package_without_file_list_is_refused_and_does_not_spawn_shell` | `cargo fmt -p` without a file list is refused |
| `xai-grok-tools` `dangerous_cargo_clippy_all_targets_is_refused_and_does_not_spawn_shell` | `cargo clippy --all-targets` is refused |
| `xai-grok-tools` `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell` | `cargo clippy -p ... --all-targets` is refused |
| `xai-grok-tools` `dangerous_cargo_clippy_workspace_is_refused_and_does_not_spawn_shell` | `cargo clippy --workspace` is refused |
| `xai-grok-tools` `dangerous_cargo_test_workspace_is_refused_and_does_not_spawn_shell` | `cargo test --workspace` is refused |
| `xai-grok-tools` `dangerous_cargo_nextest_run_without_package_or_filter_is_refused_and_does_not_spawn_shell` | bare `cargo nextest run` is refused |
| `xai-grok-tools` `dangerous_cargo_test_package_lib_filter_is_not_refused` | honest `cargo test -p <crate> --lib <filter>` still runs |
| `xai-grok-tools` `honest_package_lib_filter_is_not_refused` | parser-only: same honest `cargo test -p --lib <filter>` is not refused |
| `xai-grok-tools` `listed_file_fmt_is_not_refused` | parser-only: rustfmt with an explicit file list is not refused |
| `xai-grok-tools` `env_prefixed_fmt_all_is_refused` | parser-only: env-prefixed `cargo fmt --all` is still refused |

```bash
cargo test -p xai-grok-tools --lib rust_edit_verify
cargo test -p xai-grok-tools --lib compiler_probe_junk
cargo test -p xai-grok-tools --lib dangerous_cargo
```

#### `from_config` cold catalog

`resolve_from_config_no_config` is the DOGE theme miss. It is **not** this
contract.

Empty `models_cache.json` is a miss in code (`load_fresh` returns `None` when
`models` is empty). That branch has no named test. Do not claim it is
cargo-proven.

| path::test | Contract |
|------------|----------|
| `xai-grok-shell` `from_config_without_prefetch_produces_usable_catalog` | `ModelsManager::from_config` with no prefetch still produces a usable bundled catalog and does not claim a real fetched catalog |

```bash
cargo test -p xai-grok-shell --lib -- from_config_without_prefetch_produces_usable_catalog
```

#### `/rebuild` SHA-aware peer relaunch

Fail-does-not-signal alone is not SHA-aware identity.

| path::test | Contract |
|------------|----------|
| `failed_install_must_not_replace_or_signal_peers` | Failed `/rebuild` install must not replace the binary or SIGUSR1 peers |
| `build_fail_does_not_signal_leaders` | A failed `/rebuild` build must not SIGUSR1 leaders |
| `parse_version_output_extracts_identity` | Parses `grok-oss` semver plus git SHA |
| `peer_relaunch_accepts_same_semver_different_sha` | Same semver plus a different SHA is newer |
| `peer_relaunch_declines_equal_identity_on_same_path` | Equal identity on the same path does not loop |
| `peer_relaunch_accepts_deleted_inode_even_when_identity_equal` | Deleted inode still relaunches |
| `leader_is_older_than_same_semver_git_sha_identity` | Leader older-than uses same-semver git SHA identity |

```bash
cargo test -p xai-grok-update --lib -- failed_install_must_not_replace_or_signal_peers \
  build_fail_does_not_signal_leaders parse_version_output_extracts_identity \
  peer_relaunch_accepts_same_semver_different_sha \
  peer_relaunch_declines_equal_identity_on_same_path \
  peer_relaunch_accepts_deleted_inode_even_when_identity_equal
cargo test -p xai-grok-shell --lib -- leader_is_older_than_same_semver_git_sha_identity
```

#### `/rebuild` signals every live grok-oss PID

SHA-aware fail-does-not-signal plus version-plus-SHA identity can stay green
while the all-PID target list disappears. After a successful install, TUI
`/rebuild` SIGUSR1s every other live grok-oss TUI PID in `active_sessions.json`
(dedupe by PID). Two windows on the same `session_id` both get a signal. Self,
dead, and non-grok PIDs are skipped. These helper tests are recon defense, not
a new implementation. There is no catalog test that
`rebuild_and_relaunch_with_progress` itself calls the peer walk.

| path::test | Contract |
|------------|----------|
| `xai-grok-update` `rebuild_signals_each_pid_after_composite_key` | Two live grok-oss PIDs on the same `session_id` are both signal targets (dedupe by PID, not by session) |
| `xai-grok-update` `peer_pids_to_signal_excludes_self_dead_and_non_grok` | Peer list is other live grok-oss PIDs only (not self, not dead, not non-grok) |

```bash
cargo test -p xai-grok-update --lib -- rebuild_signals_each_pid_after_composite_key \
  peer_pids_to_signal_excludes_self_dead_and_non_grok
```

#### Nucleo reuse-per-root

Per-matcher pool size `NUM_NUCLEO_THREADS = 2` is shipped in code. No `fn`
asserts `Some(2)`.

| path::test | Contract |
|------------|----------|
| `repeated_open_without_close_keeps_one_search_per_root` | Many opens without `close` keep one live matcher per root |
| `distinct_roots_each_keep_one_search` | Distinct roots each keep one search |
| `get_results_does_not_keep_a_stale_search_alive` | Poll-only `get_results` does not refresh the stale timer |

```bash
cargo test -p xai-grok-workspace --lib -- repeated_open_without_close_keeps_one_search_per_root \
  distinct_roots_each_keep_one_search get_results_does_not_keep_a_stale_search_alive
```

#### Aborted thinking is not the live turn

Pause with no `[stop]` must not freeze a truncated user-facing draft inside
an expanded thought. Empty instant leftovers must not paint `Thought for 0.0s`.
Internal "the user is asking me..." is reasoning, not the answer.

| path::test | Contract |
|------------|----------|
| `abort_turn_does_not_present_aborted_user_facing_draft_as_the_live_turn` | Thinking chrome must not present an aborted user-facing draft as the live turn |
| `abort_turn_omits_instant_empty_thinking_so_thought_for_zero_does_not_paint` | Thought for 0.0s must not paint as a real thought block (omit or merge) |
| `abort_turn_collapses_truncated_draft_out_of_expanded_thinking` | After pause, do not leave a truncated assistant draft in expanded thinking |
| `abort_turn_keeps_internal_reasoning_out_of_the_assistant_answer` | Internal reasoning must not leak as the assistant answer |
| `thought_chunk_peels_trailing_user_facing_draft_while_streaming` | A reply that leaked into thought chunks is peeled while streaming |
| `collapsed_header_never_paints_thought_for_zero_point_zero_seconds` | Collapsed header never paints `0.0s` |
| `aborted_thinking_finished_display_mode_is_collapsed_even_when_always_expand_is_on` | Aborted thinking collapses even when always-expand is on |
| `peel_trailing_user_facing_draft_keeps_internal_reasoning` | Peel keeps "the user is asking me..." and drops "Hey, sorry..." |
| `aborted_mixed_thought_expanded_body_omits_user_facing_draft` | Expanded aborted body does not include the half-apology |

```bash
cargo test -p xai-grok-pager --lib -- abort_turn_does_not_present_aborted_user_facing_draft_as_the_live_turn \
  abort_turn_omits_instant_empty_thinking_so_thought_for_zero_does_not_paint \
  abort_turn_collapses_truncated_draft_out_of_expanded_thinking \
  abort_turn_keeps_internal_reasoning_out_of_the_assistant_answer \
  thought_chunk_peels_trailing_user_facing_draft_while_streaming \
  collapsed_header_never_paints_thought_for_zero_point_zero_seconds \
  aborted_thinking_finished_display_mode_is_collapsed_even_when_always_expand_is_on \
  peel_trailing_user_facing_draft_keeps_internal_reasoning \
  aborted_mixed_thought_expanded_body_omits_user_facing_draft
```

#### Pause / Clear finished chrome

| path::test | Contract |
|------------|----------|
| `work_control_chrome_matrix_pause_not_cancel_stop_not_pause` | Pause is not cancel; stop is not pause |
| `pause_button_click_dispatches_global_pause_not_cancel` | Pause click is global pause, not cancel |
| `idle_with_subagents_paints_pause_and_stop_hits` | Idle with live subagents still paints pause and stop hits |
| `global_paused_idle_paints_resume_not_stop` | Global-paused idle paints resume, not stop |
| `clear_finished_action_idle_is_quiet_not_neon_green_or_magenta` | Clear finished idle paint is quiet secondary, not neon green or magenta |
| `clear_finished_click_does_not_open_subagent` | Clear finished click does not open a subagent |

```bash
cargo test -p xai-grok-pager --lib -- work_control_chrome_matrix_pause_not_cancel_stop_not_pause \
  pause_button_click_dispatches_global_pause_not_cancel \
  idle_with_subagents_paints_pause_and_stop_hits \
  global_paused_idle_paints_resume_not_stop \
  clear_finished_action_idle_is_quiet_not_neon_green_or_magenta \
  clear_finished_click_does_not_open_subagent
```

#### User-guide fork pins (beyond class 1 resume and class 7 skills)

The shared guide is not in `FORK_PATHS`. A guide with zero `/limits` hits is a
failed land (prose sniff; no dedicated hit-count `fn`). SuperGrok is paid.

| path::test | Contract |
|------------|----------|
| `user_guide_does_not_claim_automatic_host_hop_is_unshipped` | Guide does not claim hop after included SuperGrok period limits are full is unshipped |
| `user_guide_names_token_economy_spend_order` | Guide names spend order: included SuperGrok period limits, then SuperGrok dollar credits, then console team prepaid / console API credits |
| `user_guide_limits_names_fail_open_and_named_commands` | Guide names fail-open (client 100% / remaining 0 / $0 must not mark SuperGrok used up) plus stay-supergrok, use-console, meter, refresh, and limits_pins.json. grok-oss limits is not xAI billing truth |

```bash
cargo test -p xai-grok-pager --lib -- user_guide_does_not_claim_automatic_host_hop_is_unshipped \
  user_guide_names_token_economy_spend_order \
  user_guide_limits_names_fail_open_and_named_commands
```

#### Seeded custom model on `session/load` stays Chat Completions

`session/load` keeps a seeded custom model id on Chat Completions. It does
not remap that slug onto the default grok-4.5 Responses catalog entry.
grok-4.5 itself still uses Responses. SuperGrok is paid. This is not
last-session on start.

| path::test | Contract |
|------------|----------|
| `xai-grok-shell` `keep_unverified_persisted_model_keeps_seeded_custom_slug` | A persisted slug that is not in the catalog and is not `grok-*` stays as-is |
| `xai-grok-shell` `seeded_test_model_keeps_chat_completions_backend` | Seeded `test-model` sampling config and apply fallback stay Chat Completions |
| `xai-grok-shell` **integration** `test_image_strip_recovery::poisoned_image_session_recovers_within_the_failing_turn` | After `session/load`, a 400 `invalid_image` strips in the same turn |

```bash
cargo test -p xai-grok-shell --lib -- keep_unverified_persisted_model_keeps_seeded_custom_slug \
  seeded_test_model_keeps_chat_completions_backend
cargo test -p xai-grok-shell --test test_image_strip_recovery -- \
  poisoned_image_session_recovers_within_the_failing_turn
```

#### Baked default is Grok 4.6 at medium

Fork contract change. Upstream baked default is still grok-4.5 plus high.

| path::test | Contract |
|------------|----------|
| `xai-grok-shell` `baked_default_is_grok_46_medium_fork_contract` | Baked `default_model()` is `grok-4.6` at medium reasoning effort |

```bash
cargo test -p xai-grok-shell --lib -- baked_default_is_grok_46_medium_fork_contract
```

#### Soft plan present is a real right-side pane

A 75% centered overlay is a failed land for default soft park.

| path::test | Contract |
|------------|----------|
| `plan_soft_park_docks_right_not_centered_overlay` | Soft park docks on the right, not a 75% centered overlay |
| `plan_soft_park_draw_right_pane_matches_side_panel_status` | Right-pane geometry matches **Side panel open** |
| `plan_row_click_does_not_enter_commenting` | A plan row click does not enter Commenting |
| `plan_loop_status_does_not_claim_side_panel_when_viewer_closed` | Status does not claim Side panel open when the viewer is closed |
| `plan_preview_ctrl_z_restores_wiped_human_box` | Preview-focused Ctrl+Z restores a wiped Human box |
| `plan_prompt_ctrl_z_restores_wiped_human_box` | Prompt-focused Ctrl+Z restores a wiped Human box |
| `preview_typed_comment_rides_along_on_approve` | Preview type-after-park notes ride along with Approve |
| `prompt_tab_typed_comment_rides_along_on_approve` | Tab to Prompt then Approve still sends typed notes |
| `esc_with_human_box_draft_keeps_feedback_draft` | Esc/close keeps a Human-box draft |
| `tab_preview_prompt_keeps_human_box_draft` | Tab Preview and Prompt keeps a Human-box draft |
| `exit_with_human_box_draft_does_not_drop_unsent_text` | Exit does not silently drop unsent Human-box text |
| `approve_with_composer_comments_sends_one_human_line` | Approve with notes is one wrapped Human line |
| `empty_approve_does_not_send_composer_as_second_prompt` | Empty Approve does not invent review comments |
| `resume_restore_keeps_revise_box_draft` | Isolated present Approve does not consume a restored draft |
| `plan_human_box_keystroke_burst_does_not_flush_unsent_draft_every_char` | Plan Human-box typing does not persist every character |
| `main_composer_keystroke_burst_does_not_flush_unsent_draft_every_char` | Main prompt typing does not persist every character |
| `xai-grok-shell` `keystroke_burst_does_not_flush_unsent_draft_every_char` | Unsent-draft debounce skips writes inside the window |

```bash
cargo test -p xai-grok-pager --lib -- \
  plan_soft_park_docks_right_not_centered_overlay \
  plan_soft_park_draw_right_pane_matches_side_panel_status \
  plan_row_click_does_not_enter_commenting \
  plan_loop_status_does_not_claim_side_panel_when_viewer_closed \
  plan_preview_ctrl_z_restores_wiped_human_box \
  plan_prompt_ctrl_z_restores_wiped_human_box \
  preview_typed_comment_rides_along_on_approve \
  prompt_tab_typed_comment_rides_along_on_approve \
  esc_with_human_box_draft_keeps_feedback_draft \
  tab_preview_prompt_keeps_human_box_draft \
  exit_with_human_box_draft_does_not_drop_unsent_text \
  approve_with_composer_comments_sends_one_human_line \
  empty_approve_does_not_send_composer_as_second_prompt \
  resume_restore_keeps_revise_box_draft \
  plan_human_box_keystroke_burst_does_not_flush_unsent_draft_every_char \
  main_composer_keystroke_burst_does_not_flush_unsent_draft_every_char
cargo test -p xai-grok-shell --lib -- keystroke_burst_does_not_flush_unsent_draft_every_char
```

#### Plan-review and Linux prompt screenshot paste

| path::test | Contract |
|------------|----------|
| `event_paste_plan_commenting_empty_defers_clipboard_image_probe` | Empty plan-comment `Event::Paste` defers a clipboard image probe |
| `plan_feedback_ctrl_v_defers_clipboard_image_probe` | Plan-review Ctrl+V defers a clipboard image probe |
| `agent_empty_bracketed_paste_defers_probe_for_clipboard_image` | Empty agent `Event::Paste` defers a probe on every OS |
| `approve_or_revise_drains_plan_composer_images` | Approve / Revise drain composer image chips |

```bash
cargo test -p xai-grok-pager --lib -- \
  event_paste_plan_commenting_empty_defers_clipboard_image_probe \
  plan_feedback_ctrl_v_defers_clipboard_image_probe \
  agent_empty_bracketed_paste_defers_probe_for_clipboard_image \
  approve_or_revise_drains_plan_composer_images
```

#### Live chrome names SuperGrok dollar credits

Wire JSON field `supergrok_extras` may stay. Human chrome must not nickname SuperGrok dollar credits.

| path::test | Contract |
|------------|----------|
| `compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct` | Compact meter paints SuperGrok dollar credits, not a nickname |
| `format_supergrok_session_with_weekly_and_extras` | `/limits` human text says SuperGrok dollar credits |

```bash
cargo test -p xai-grok-pager --lib -- \
  compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct \
  format_supergrok_session_with_weekly_and_extras
```

#### No two live same-description Subagent rows

Product spawn rejects a second live same-description Task child. Unlimited retry never paints `4294967295`.

| path::test | Contract |
|------------|----------|
| `live_subagent_list_does_not_show_two_rows_with_the_same_description` | Live Subagents list keeps one same-description row |
| `task_spawn_rejects_or_replaces_second_live_same_description` | Second live same-description spawn is rejected |
| `format_activity_label_unlimited_retry_has_no_u32_max_fraction` | Unlimited retry paints `Retrying (1)`, not `1/4294967295` |
| `implement_effort_two_does_not_spawn_two_review_rows_unless_operator_asked` | Implement effort 2 is thoroughness, not two Review rows |

```bash
cargo test -p xai-grok-pager --lib -- \
  live_subagent_list_does_not_show_two_rows_with_the_same_description \
  format_activity_label_unlimited_retry_has_no_u32_max_fraction
cargo test -p xai-grok-tools --lib -- task_spawn_rejects_or_replaces_second_live_same_description
cargo test -p xai-grok-agent --lib -- implement_effort_two_does_not_spawn_two_review_rows_unless_operator_asked
```

#### L1 Subagents list is L2-only

L1 live chrome shows L2 coordinators only. Each L2 row may append a live
L3 count. L3 specialists do not get their own L1 rows or names.

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `live_subagent_list_shows_only_l2_and_reports_live_l3_count` | Live list is L2-only and reports a live L3 count |
| `xai-grok-pager` `l2_row_shows_live_l3_count_not_specialist_names` | Tasks pane L2 row shows `N specialists`, not L3 names |

```bash
cargo test -p xai-grok-pager --lib -- \
  live_subagent_list_shows_only_l2_and_reports_live_l3_count \
  l2_row_shows_live_l3_count_not_specialist_names
```

#### Nested overlay hang, duplicate prompt, L3 click (Surmount / grok-oss fork)

Finished nested L2/L3 must not leave L1 on `Waiting for the model` with a
running timer. That string is also the live sampler wait. Chrome must
distinguish live nested wait, live sampler / `TurnRunning` with no
completed-wait fallthrough, queued `pending_prompts` (1 queued while
nested still running), and false wait after nested ids already
completed. Do not call that string a hang without evidence. `/unstick`
stays operator-invoked. The same Human prompt, including `[Image #1]`,
must paint once after send. Overlay chrome that names a live L3 must
open that session view on click. `/dashboard` must not merge with
`/running`. Finished overlay elapsed must freeze at host
`SubagentFinished.duration_ms` (54m34s), not a later spawn-wall clock
(1h14m). L2 finish must surface on L1 without opening the overlay.
After `info.finished`, `AutoCompactStarted` must not set live compact.
Named tests are Surmount / grok-oss fork contracts.

| path::test | Contract |
|------------|----------|
| `parent_must_not_wait_for_the_model_after_waited_nested_already_completed` | Parent wait on a completed nested id must not stay Waiting for the model |
| `waiting_for_the_model_is_not_idle_when_nested_subagent_still_running` | Live nested wait is not idle and is not the sampler hang string |
| `waiting_for_the_model_is_not_idle_when_prompt_is_queued` | 1 queued plus nested still running is not idle |
| `task_output_wait_clears_after_waited_nested_agent_completes` | Satisfied nested wait must not fall through to Waiting for the model |
| `named_task_output_wait_clears_after_waited_nested_agent_completes` | Named finished nested wait must not paint Waiting for the model |
| `nested_overlay_drops_responding_after_child_acp_turn_completes` | Finished nested overlay freezes elapsed and drops live Responding / pause-stop |
| `nested_overlay_title_elapsed_matches_host_subagent_finished_duration` | Overlay title elapsed equals host `duration_ms` (54m34s), not spawn-wall 1h14m |
| `reopening_finished_nested_overlay_does_not_start_climbing_title_clock` | Reopening a finished overlay must idle the child and keep host duration |
| `subagents_live_list_drops_responding_after_subagent_finished` | Live Subagents list must not keep Responding after `SubagentFinished` |
| `l2_finish_surfaces_to_l1_without_opening_nested_overlay` | L2 finish surfaces on the parent session; wait tool completes |
| `wait_on_completed_nested_id_missing_from_map_does_not_stay_running` | Completed nested id missing from the map is not treated as still running |
| `silent_subagent_completed_wake_surfaces_turn_completed` | Silent `subagent-completed-*` wake still paints TurnCompleted |
| `auto_compact_started_after_subagent_finished_does_not_set_live_compact_activity` | Finished nested session must not apply a new AutoCompactStarted |
| `overlay_nested_status_click_opens_l3_session_view` | Overlay L3 status click opens that specialist view |
| `interjection_echo_does_not_duplicate_last_human_prompt` | Optimistic Human line plus echo must not duplicate `[Image #1]` |
| `image_interject_leaves_one_prompt_and_empty_queue` | After image interject the prompt appears once and is not leftover in the queue |

```bash
cargo test -p xai-grok-pager --lib -- \
  parent_must_not_wait_for_the_model_after_waited_nested_already_completed \
  waiting_for_the_model_is_not_idle_when_nested_subagent_still_running \
  waiting_for_the_model_is_not_idle_when_prompt_is_queued \
  task_output_wait_clears_after_waited_nested_agent_completes \
  named_task_output_wait_clears_after_waited_nested_agent_completes \
  nested_overlay_drops_responding_after_child_acp_turn_completes \
  nested_overlay_title_elapsed_matches_host_subagent_finished_duration \
  reopening_finished_nested_overlay_does_not_start_climbing_title_clock \
  subagents_live_list_drops_responding_after_subagent_finished \
  l2_finish_surfaces_to_l1_without_opening_nested_overlay \
  wait_on_completed_nested_id_missing_from_map_does_not_stay_running \
  silent_subagent_completed_wake_surfaces_turn_completed \
  auto_compact_started_after_subagent_finished_does_not_set_live_compact_activity \
  overlay_nested_status_click_opens_l3_session_view \
  interjection_echo_does_not_duplicate_last_human_prompt \
  image_interject_leaves_one_prompt_and_empty_queue
```

#### `/start` plus leftover cancel-resume marker

`/start` is not `/resume`. Idle clean sessions do not invent a turn.
Mid-turn `/rebuild` writes `canceled_turn_resume.json`. Idle completed
turns do not write a marker and do not re-fire the last prompt. Load
drops a leftover marker after a successful primary-turn finish.

| path::test | Contract |
|------------|----------|
| `start_while_globally_paused_continues_interrupted_turn_once` | Global pause plus `/start` continues the interrupted turn once |
| `start_on_idle_clean_session_does_not_invent_a_turn` | Idle clean `/start` does not invent a turn |
| `start_with_cancel_resume_marker_continues_interrupted_turn` | Marker present: `/start` continues that turn |
| `handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn` | Mid-turn `/rebuild` writes the marker; load continues |
| `handle_rebuild_done_idle_completed_turn_does_not_write_cancel_resume_or_refire_last_prompt` | Idle completed `/rebuild` does not write a marker or re-fire |
| `session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully` | Load drops a leftover marker after a successful primary-turn finish |

```bash
cargo test -p xai-grok-pager --lib -- \
  start_while_globally_paused_continues_interrupted_turn_once \
  start_on_idle_clean_session_does_not_invent_a_turn \
  start_with_cancel_resume_marker_continues_interrupted_turn \
  handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn \
  handle_rebuild_done_idle_completed_turn_does_not_write_cancel_resume_or_refire_last_prompt \
  session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully
```

#### `/unstick` resends the last L1 prompt (Surmount / grok-oss fork)

`/unstick` is not `/resume`. It resends the last parent prompt as if the
network dropped it. It must not paint a second Human line, cancel nested
agents, rewind tokens, or invent text when there is no last prompt. A hung
`running_task` is orphaned (reconnect analog), then the retry samples.
The leader drops a hung `session/prompt` RPC with the same routing as a
disconnected client (`leader.response.orphaned`) while the pager stays
connected. WAL image file ids resend as `file://` resource links, never data URLs.

| path::test | Contract |
|------------|----------|
| `unstick_resends_last_l1_prompt_without_duplicate_human_line` | Resend last L1 prompt; no second Human line |
| `unstick_does_not_cancel_nested_subagents_or_rewind_tokens` | Do not cancel nested work, rewind, or reset usage meters |
| `unstick_does_not_collide_with_resume_slash` | `/unstick` is not `/resume` (picker) |
| `unstick_with_no_last_prompt_fails_loud` | No last prompt: short toast, do not invent text |
| `unstick_retry_does_not_append_second_user_query_when_last_turn_matches` | Shell skip-append on `_meta.unstickRetry` when last user turn matches; sample again |
| `unstick_retry_orphans_stuck_running_task_then_samples_again` | Orphan hung `running_task`, then sample; no second user query; not send-now cancel |
| `unstick_leader_drops_hung_session_prompt_like_disconnected_client` | Leader drops hung `session/prompt` via `leader.response.orphaned` while connected; retry is a new RPC; not cancel, not evict |
| `unstick_resends_wal_images_as_resource_blocks_not_data_urls` | WAL file ids resend as resource links; never data URLs |
| `wal_image_resource_blocks_use_file_uri_not_data_url` | Resource link is `file://` to the session `images/` file |
| `wal_image_resource_blocks_drop_data_url_file_ids` | `data:` file ids are not sent |

```bash
cargo test -p xai-grok-pager --lib -- \
  unstick_resends_last_l1_prompt_without_duplicate_human_line \
  unstick_does_not_cancel_nested_subagents_or_rewind_tokens \
  unstick_does_not_collide_with_resume_slash \
  unstick_with_no_last_prompt_fails_loud \
  unstick_resends_wal_images_as_resource_blocks_not_data_urls \
  wal_image_resource_blocks_use_file_uri_not_data_url \
  wal_image_resource_blocks_drop_data_url_file_ids
cargo test -p xai-grok-shell --lib -- \
  unstick_retry_does_not_append_second_user_query_when_last_turn_matches \
  unstick_retry_orphans_stuck_running_task_then_samples_again \
  session_prompt_is_unstick_retry_reads_params_meta \
  take_in_flight_session_prompts_for_unstick_leaves_other_sessions \
  response_is_orphaned_for_unstick_while_client_connected
cargo test -p xai-grok-shell --test test_leader_stdio_integration -- \
  unstick_leader_drops_hung_session_prompt_like_disconnected_client
```

#### `/rebuild` TUI persist like a network interrupt

Fork-owned. TUI `/rebuild` persist must keep unsent composer draft, queued
prompts (including mid-turn interject text), plan Human-box notes, and
`plan.md` the same way a disconnect restore does. Nested subagent ids are
not cancelled in that persist path and `/rebuild` is not blocked until
nested work finishes. Compile source is the git index, not unstaged WIP.
Keep these stronger than an upstream resume that cancels nested orphans.

Leader `RelaunchForUpdate` keeps nested ids on that leader the same way a
TUI disconnect does: this process is not exec-replaced while nested ids
are live. After nested ids finish, this process stays up while the parent
turn is still busy, with no five-second cap.

| path::test | Contract |
|------------|----------|
| `handle_rebuild_done_persists_unsent_composer_draft_and_session_load_restores_it` | Unsent composer draft survives `/rebuild` and session load |
| `handle_rebuild_done_persists_pending_prompts_including_interject_and_session_load_restores_them` | Queued / interject prompts survive `/rebuild` |
| `handle_rebuild_done_persists_plan_feedback_draft_and_plan_md` | Plan Human-box notes and `plan.md` survive `/rebuild` |
| `handle_rebuild_done_keeps_nested_subagents_for_resume` | Nested ids are not cancelled in TUI persist; Subagents list must not go empty |
| `rebuild_and_relaunch_starts_while_nested_subagents_are_running` | Nested work is not a `/rebuild` gate |
| `relaunch_drain_keeps_nested_ids_alive_after_grace_like_disconnect` | Nested ids still exist after rebuild/relaunch intent; drain does not exec-replace while they are live |
| `relaunch_drain_keeps_parent_turn_until_idle_like_disconnect` | Parent-turn busy keeps this leader up; drain does not AutoUpdate while `AgentActivity::is_busy` or IPC `agent_busy` |
| `rebuild_subcommand_parses` | CLI `grok-oss rebuild` is clap-wired |
| `export_git_index_omits_unstaged_dirty_file` | Staged compile snapshot is the git index |
| `stash_keep_index_hides_unstaged_wip_from_compile_worktree` | Unstaged WIP is not the `just install` source |

```bash
cargo test -p xai-grok-pager --lib -- \
  handle_rebuild_done_persists_unsent_composer_draft_and_session_load_restores_it \
  handle_rebuild_done_persists_pending_prompts_including_interject_and_session_load_restores_them \
  handle_rebuild_done_persists_plan_feedback_draft_and_plan_md \
  handle_rebuild_done_keeps_nested_subagents_for_resume \
  rebuild_and_relaunch_starts_while_nested_subagents_are_running \
  rebuild_subcommand_parses
cargo test -p xai-grok-shell --lib -- \
  relaunch_drain_keeps_nested_ids_alive_after_grace_like_disconnect \
  relaunch_drain_keeps_parent_turn_until_idle_like_disconnect
cargo test -p xai-grok-update --lib -- export_git_index_omits_unstaged_dirty_file \
  stash_keep_index_hides_unstaged_wip_from_compile_worktree
```

#### Prompt write-ahead log (`prompt_wal.jsonl`)

Fork-owned. Session-local append-only file next to `unsent_prompt_draft`.
Enter send, mid-turn interject, queue enqueue, plan Human-box notes that
ride Approve, and `/rebuild` persist each append (fsync that line) before
the model is asked, before compact, and before re-exec. The WAL is not
rewritten, not compacted as conversation, and not counted as model tokens.
If chat history, prompt history, and the queue lack a WAL send, session
load restores it as a pending Human turn. Nested work on the leader
survives `/rebuild` the same way a TUI disconnect does. After `--resume`
/ last-session restore, the operator prompt appears once. Unsent draft
restore and queue restore must not both rehydrate the same string. A WAL
Send must not enqueue a body already in the composer. Resume must not
arm Enter:interject unless a live sampler turn is actually running.
Waiting after resume must be a real sampler wait, not occupancy leftover.

| path::test | Contract |
|------------|----------|
| `prompt_wal_appends_on_enter_before_model_wait` | Enter send appends WAL before `Effect::SendPrompt` |
| `prompt_wal_appends_on_mid_turn_interject` | Mid-turn interject appends WAL before `SendInterject` |
| `prompt_wal_appends_on_queue_enqueue` | Queue enqueue (including L0 drain) appends WAL |
| `prompt_wal_appends_on_approve_notes` | Plan Human-box notes that ride Approve append a `PlanNotes` WAL line |
| `session_load_restores_wal_send_missing_from_prompt_history` | Missing WAL send restores as a pending Human turn |
| `handle_rebuild_done_persists_unsent_composer_draft_and_session_load_restores_it` | Rebuild flush also writes a WAL line (existing persist test, not weakened) |
| `handle_rebuild_done_persists_pending_prompts_including_interject_and_session_load_restores_them` | Rebuild flush WAL line for queued bodies (existing persist test, not weakened) |
| `resume_restore_must_not_put_the_same_operator_prompt_in_composer_and_queue` | Resume restore: operator prompt appears once, not composer plus queue #1 |
| `resume_restore_must_not_arm_enter_interject_when_no_live_sampler_turn` | Resume must not arm Enter:interject unless a live sampler turn is running |
| `resume_restore_must_not_show_waiting_when_nested_and_sampler_are_gone` | Waiting after resume is a real sampler wait, not occupancy leftover |
| `resume_restore_must_not_rehydrate_unsent_draft_and_queue_with_the_same_string` | Unsent draft and queue restore must not both rehydrate the same string |

```bash
cargo test -p xai-grok-pager --lib -- \
  prompt_wal_appends_on_enter_before_model_wait \
  prompt_wal_appends_on_mid_turn_interject \
  prompt_wal_appends_on_queue_enqueue \
  prompt_wal_appends_on_approve_notes \
  session_load_restores_wal_send_missing_from_prompt_history \
  handle_rebuild_done_persists_unsent_composer_draft_and_session_load_restores_it \
  handle_rebuild_done_persists_pending_prompts_including_interject_and_session_load_restores_them \
  resume_restore_must_not_put_the_same_operator_prompt_in_composer_and_queue \
  resume_restore_must_not_arm_enter_interject_when_no_live_sampler_turn \
  resume_restore_must_not_show_waiting_when_nested_and_sampler_are_gone \
  resume_restore_must_not_rehydrate_unsent_draft_and_queue_with_the_same_string
cargo test -p xai-grok-shell --lib -- \
  append_fsyncs_a_line_and_does_not_rewrite_prior_lines \
  skips_prompt_wal_jsonl_because_it_is_not_conversation
```

#### Compact must not re-enqueue occupancy

Fork-owned. Successful `/compact` and AUTO compact must not copy the
occupancy operator prompt, or any operator prompt, onto
`pending_prompts`. Compact is not a successful agent turn, so auto-run
`/implement` must not fire. Drain already-queued work only. This is not
the compact-fail pause unstick path.

| path::test | Contract |
|------------|----------|
| `compact_complete_does_not_reenqueue_occupancy_or_any_operator_prompt` | `/compact` complete must not re-enqueue occupancy or auto-run leftover `/implement` |
| `auto_compact_completed_does_not_reenqueue_occupancy_or_any_operator_prompt` | AUTO compact must not re-enqueue occupancy or any operator prompt |

```bash
cargo test -p xai-grok-pager --lib -- \
  compact_complete_does_not_reenqueue_occupancy_or_any_operator_prompt \
  auto_compact_completed_does_not_reenqueue_occupancy_or_any_operator_prompt
```

#### `/view-plan` never samples

Fork-owned. Resume `--continue` can submit `/view-plan` before slash
dispatch is ready. PassThrough would steal the next scripted implement
turn (`plan_approval_restored_after_resume`). Trailing slash is the
same command.

| path::test | Contract |
|------------|----------|
| `send_prompt_view_plan_never_sends_to_model` | `/view-plan` and `/view-plan/` must not `SendPrompt` / `SendInterject` |
| `parses_view_plan_with_trailing_slash` | `/view-plan/` parses as token `view-plan` |
| `plan_approval_restored_after_resume` | After resume, panel Approve starts the scripted implement sentinel |

```bash
cargo test -p xai-grok-pager --lib -- send_prompt_view_plan_never_sends_to_model
cargo test -p xai-grok-pager --lib -- parses_view_plan_with_trailing_slash
cargo test -p xai-grok-pager-pty-harness --test plan_approval_resume
```

#### Sticky message timestamps and reconnect banner

Fork-owned. Message clocks stay on the first visible row when the original
timestamp line has scrolled above the fold. Leader disconnect paints a
sticky `Disconnected. Reconnecting...` banner until reconnect completes.

| path::test | Contract |
|------------|----------|
| `timestamp_stays_visible_when_first_line_scrolls_above_fold` | Clipped message still paints the clock on the first visible row |
| `live_prompt_task_pauses_honest_clock_at_disconnect_toast` | Disconnect toast plus sticky banner; sticky clears after reconnect |

```bash
cargo test -p xai-grok-pager --lib -- timestamp_stays_visible_when_first_line_scrolls_above_fold
cargo test -p xai-grok-pager --lib -- live_prompt_task_pauses_honest_clock_at_disconnect_toast
```

#### ForceRefresh on explicit `/limits`

Explicit TUI `/limits` open and CLI `grok-oss limits` collect are
ForceRefresh. Background FetchBilling is HonorTtl. ForceRefresh without
a management key does not clear Management caches.

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `management_meter_cache_policy_collect_force_background_honor_ttl` | Collect is ForceRefresh; background poll is HonorTtl |
| `xai-grok-pager` `should_clear_management_meter_caches_force_with_key_only` | ForceRefresh without a management key does not clear Management caches |
| `xai-grok-shell` `limits_snapshot_mode_for_get_billing_explicit_is_force_refresh` | Explicit get-billing snapshot mode is ForceRefresh |

```bash
cargo test -p xai-grok-pager --lib -- \
  management_meter_cache_policy_collect_force_background_honor_ttl \
  should_clear_management_meter_caches_force_with_key_only
cargo test -p xai-grok-shell --lib -- \
  limits_snapshot_mode_for_get_billing_explicit_is_force_refresh
```

#### Spawn-prompt fold plus last-answer caps

Parent ingest only. Live L2 still executes the full spawn prompt. Stored
child output can still be the full last answer. No automatic on-disk
last-answer report. Filter `fold_spawn_prompt` matches the fold `fn`s.

| path::test | Contract |
|------------|----------|
| `xai-grok-sampling-types` `huge_spawn_prompt_becomes_pointer_with_description_and_report` | Spawn prompt over 40k becomes a pointer (description + size + report path if any) |
| `xai-chat-state` `parent_estimated_tokens_omit_huge_spawn_prompt` | Parent estimated tokens omit the huge spawn prompt body |
| `xai-tool-types` `to_model_text_caps_huge_last_answer_for_parent_ingest` | Huge last answers are capped for parent ingest |
| `xai-grok-tools` `completed_subagent_task_output_is_capped_or_points_at_report` | Completed poll output is capped or points at a report |
| `xai-grok-tools` `blocking_spawn_subagent_completed_to_prompt_format_is_capped` | Blocking-spawn prompt format is capped |

```bash
cargo test -p xai-grok-sampling-types --lib -- fold_spawn_prompt
cargo test -p xai-chat-state --lib -- parent_estimated_tokens_omit_huge_spawn_prompt
cargo test -p xai-tool-types --lib -- to_model_text_caps_huge_last_answer_for_parent_ingest
cargo test -p xai-grok-tools --lib -- \
  completed_subagent_task_output_is_capped_or_points_at_report \
  blocking_spawn_subagent_completed_to_prompt_format_is_capped
```

---

## Product filter catalog

Paths below are crate-relative module paths as rustc / `cargo test` see them;
use the filter substring for nextest. Prefer the **filter blocks** at the end
of each section for day-to-day recon.

### shell_collision / SHELL_RESERVED

**Honesty (not required land):**
`shell_collision_contract_covers_every_pager_command_and_alias` has **no**
matching `fn` in this tree. The slash command `/clear-completed-todos` exists.
Pager `SHELL_RESERVED` as a land identifier is gone. Do not `rg` that old name
as a required-land test.

Keep the slash command itself. Do not treat a silent `cargo test -- shell_collision`
as proof.

### Stuck retry / StreamResumed / headers timeout / transport footer

**Honesty (not required land).** These pager identifiers have **no** matching
`fn` in this tree. Do not treat them as required-land names:
`retry_chrome_soft_reconnects_when_retry_stream_starts`,
`stream_resumed_without_prior_retry_clears_activity`,
`clip_retry_reason_*`, `retrying_activity_label_*`,
`retrying_label_shows_timeout_*`. Shell emit exists. Pager chrome for stuck
retry is not catalog-proven.

**Still present neighbors** (required land for this neighbor seam):

| path::test | Contract |
|------------|----------|
| `xai-grok-shell` `session::acp_session_tests::replay_buffer_send_update_tests::stream_started_emits_retry_state_stream_resumed` | Stream start emits StreamResumed retry state |
| `xai-grok-sampler` `actor::request_task::wait_before_attempt_aborts_on_cancel` | Esc cancels shared cooldown wait |
| `xai-grok-sampler` **integration** `peer_process_rate_limit::peer_process_does_not_sample_during_shared_rate_limit_cooldown` | Process B must not fire sampling HTTP while process A's flock 429 cooldown is live on disk; filename fingerprints the bearer |
| `xai-grok-sampler` `actor::request_task::retry_footer_reason_uses_short_transport_label` | Short transport footer (not opaque `Transport error: error`) |
| `xai-grok-sampler` `actor::request_task::retry_footer_backoff_hint_appends_next_try_in` | Backoff suffix `· next try in Ns` |
| `xai-grok-sampler` `client::tests::stream_headers_timeout_defaults_to_120_secs_when_env_unset` | Default stream headers timeout is **120s** when env unset (`0` / invalid → 120; positive override honored) |
| `xai-grok-sampler` **integration** `stream_headers_timeout::streaming_execute_times_out_waiting_for_headers` | Hang after accept, no headers → fail within headers budget (`GROK_STREAM_HEADERS_TIMEOUT_SECS=1` in that binary) |

```bash
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason retry_footer_backoff stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout
cargo test -p xai-grok-sampler --test peer_process_rate_limit -- peer_process_does_not_sample_during_shared_rate_limit_cooldown
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
| `xai-grok-pager` `views::agent::hide_header_zeroes_status_bar_height` | In-app status bar height 0 |
| `xai-grok-pager` `views::welcome::hide_header_zeros_welcome_top_bar_height` | Welcome top bar |
| `xai-grok-pager` `views::dashboard::layout::hide_header_zeroes_header_and_header_gap` | Dashboard header |
| settings_e2e `hide_header_*` | Settings registry + UI toggle (in-app only) |

**Honesty (not required land).** These title identifiers have **no** matching
`fn` in this tree: `title_updates_gated_only_by_title_enabled`,
`default_title_items_include_agents`, `title_escape_never_empty_payload`. Keep
the branded `window_title_*` / `titles_on_session_*` neighbors above.

```bash
cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title
cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session
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

`shell_collision` is honesty leftover, not required land (see above).

```bash
cargo test -p xai-grok-pager --lib -- clear_completed_todos
```

### Always-on bubble copy / one-click copy

A restack that keeps the paint-only `bubble_copy_buttons_on_paints_copy_icon`
test and drops click-to-copy is a failed land.

| path::test | Contract |
|------------|----------|
| `bubble_copy_buttons_on_paints_copy_icon` | Flag on paints the glyph |
| `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width` | Full-width first line still paints the glyph |
| `append_bubble_copy_button_paints_when_first_line_fills_content_width` | Helper still marks a hit when the first line fills the width |
| `clicking_human_bubble_copy_copies_the_prompt` | Human bubble `⧉` click copies that prompt |
| `clicking_assistant_bubble_copy_copies_the_message` | Assistant bubble `⧉` click copies that message |
| `clicking_wide_human_bubble_copy_still_paints_and_copies` | Full-width first line still paints a clickable `⧉` |

```bash
cargo test -p xai-grok-pager --lib -- bubble_copy_ append_bubble_copy_button_paints \
  clicking_human_bubble_copy clicking_assistant_bubble_copy clicking_wide_human_bubble_copy
```

### Surmount / OSS identity (first token)

Land class 1 above is required. Do not treat OpenRouter attribution alone as
CLI identity. A test that only checks stdout contains substring `grok` is
forbidden.

| path::test | Contract |
|------------|----------|
| `product_version_line_uses_grok_oss_not_bare_grok` | First token `grok-oss` |
| `resume_session_command_uses_grok_oss` | `grok-oss --resume` |
| `user_guide_resume_and_version_examples_use_grok_oss` | Guide examples |
| `user_guide_operator_cli_examples_use_grok_oss` | Guide leftover operator CLI examples |
| `welcome_badge_brands_grok_oss` | Welcome badge Grok OSS |
| `hero_subtitle_brands_grok_oss` | Hero thanks-line Grok OSS |
| `tutorial_list_title_brands_grok_oss` | Tutorial list title Grok OSS |
| `product_cli_name_is_grok_oss` | CLI name constant |
| `version_without_tty` (`assert_version_ok`) | No-TTY first token |
| `xai-grok-shell` openrouter `referer_is_surmount_*` / `title_is_grok_oss` | OpenRouter attribution (neighbor, not class 1) |

### Other high-value fork contracts (keep)

Dual-auth hop + multi SuperGrok + `/limits`; `interject_contract_*`;
`auto_compact_completed_preserves_todo_board`;
`compact_complete_does_not_reenqueue_occupancy_or_any_operator_prompt`;
`auto_compact_completed_does_not_reenqueue_occupancy_or_any_operator_prompt`;
skills order
(`agents_home_skills_shadow_grok_user_skills`,
`local_agents_skills_shadow_local_grok_skills`); UDAX toon filters; plan
soft-park filters. Extra restack-droppable neighbors live under *Required land
inventory* (plan present is not Approve, three-layer product prompt,
`from_config` cold catalog, SHA-aware `/rebuild`, all-PID `/rebuild` SIGUSR1, nucleo, Pause / Clear
finished, aborted thinking is not the live turn, user-guide hop and spend-order pins, seeded custom model on
`session/load` stays Chat Completions). Full residual-aligned blocks
below.

### Paint filters (restack land)

Helper tests are not paint. If a named filter has no matching `fn`, land
failed. Do not delete a red catalog test to finish a compile mop. SuperGrok
is paid: the compact meter is **included SuperGrok period limits**.
Do not call SuperGrok free.

| Filter identifier | Contract | Land |
|-------------------|----------|------|
| `status_bar_pushes_credits_compact_included_supergrok_period_limits` | Draw pushes `status` key `"credits"` with `included SuperGrok period limits · N%` | **Keep** (`credit_bar` helpers alone do not count) |
| `hit_credits_click_dispatches_show_limits` | Click on the compact meter dispatches `Action::ShowLimits` | **Keep** |
| `titled_doge_composer_frame_is_prompt_border_not_context_yellow` | Titled composer frame is white (`prompt_border_active`); title only is yellow | **Keep** |
| `plan_approval_footer_paints_five_cta_vocabulary` | Idle plan panel footer paints Approve / Comment / Revise / Exit. Clarify is only after Comment, not an idle top-level CTA | **Keep** (old `soft_park_draw_paints_panel_*` names are gone; do not revive them) |
| `sampling_config_auto_use_*` | `sampling_config_for_model` / `prepare_sampling_config_for_model` fills console failover when included SuperGrok period limits are full | **Keep** |
| `sampling_config_hops_to_sibling_included_before_extras` | Next stored SuperGrok login's included SuperGrok period limits beat this login's SuperGrok dollar credits | **Keep** |
| `limits_snapshot_second_process_reads_file_and_does_not_http` | One grok-oss process fetches SuperGrok billing; others read the flock snapshot | **Keep** |
| `compact_meter_stays_included_while_sibling_pool_has_remaining` | Compact meter stays on included SuperGrok period limits while a distinct sibling pool has remaining | **Keep** |
| `auto_compact_completed_preserves_todo_board` | AutoCompact does not wipe the UI todo board | **Keep** |
| `compact_complete_does_not_reenqueue_occupancy_or_any_operator_prompt` | `/compact` complete must not re-enqueue occupancy or any operator prompt | **Keep** |
| `auto_compact_completed_does_not_reenqueue_occupancy_or_any_operator_prompt` | AUTO compact must not re-enqueue occupancy or any operator prompt | **Keep** |
| `todo_badge_names_tasks_not_only_fraction` | Status-row badge names tasks, not only `614/638` | **Keep** |
| `status_header_todo_badge_names_tasks` | Agent status header paints `tasks N/M` and does not auto-open the pane | **Keep** |
| `nested_l2_overlay_todo_toggle_stays_findable` | Nested L2 overlay keeps that nested session's tasks badge and Ctrl+T | **Keep** |
| `hide_header_zeroes_*` | `hide_header` zeros status / welcome / dashboard chrome | **Keep** (serde default tests are not paint) |
| `forked_session_status_header_paints_switcher_and_dashboard` | Forked-session status header paints switcher plus `[Dashboard]`, not git plus cwd only | **Keep** (the yellow `/dashboard` banner is not this paint) |
| `forked_session_status_header_clicks_open_dashboard_and_cycle` | Header chips open the dashboard and cycle the fork family | **Keep** |
| `load_session_restores_fork_family_from_disk` | Resume of a persisted fork restores the parent as a live agent and stamps `forked_from` | **Keep** (paint tests alone cannot catch load dropping the family) |
| `failed_install_must_not_replace_or_signal_peers` | Failed `/rebuild` install must not replace the binary or SIGUSR1 peers | **Keep** |
| `version_without_tty` | `--version` first token is `grok-oss` with no TTY | **Keep** (substring `grok` is forbidden) |

```bash
# Prove the names still exist (missing fn = land failed), then run:
cargo test -p xai-grok-pager --lib -- status_bar_pushes_credits_compact_included_supergrok_period_limits \
  hit_credits_click_dispatches_show_limits \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  plan_approval_footer_paints_five_cta_vocabulary \
  auto_compact_completed_preserves_todo_board \
  compact_complete_does_not_reenqueue_occupancy_or_any_operator_prompt \
  auto_compact_completed_does_not_reenqueue_occupancy_or_any_operator_prompt \
  todo_badge_names_tasks_not_only_fraction \
  status_header_todo_badge_names_tasks \
  nested_l2_overlay_todo_toggle_stays_findable hide_header_zeroes \
  forked_session_status_header_paints_switcher_and_dashboard \
  forked_session_status_header_clicks_open_dashboard_and_cycle \
  load_session_restores_fork_family_from_disk
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use sampling_config_hops_to_sibling_included_before_extras \
  limits_snapshot_second_process_reads_file_and_does_not_http
cargo test -p xai-grok-pager --lib -- compact_meter_stays_included_while_sibling_pool_has_remaining
cargo test -p xai-grok-update --lib -- failed_install_must_not_replace_or_signal_peers
cargo test -p xai-grok-pager-bin --test version_without_tty
```

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
cargo test -p xai-grok-pager --lib -- format_dual_principals live_console_omits extra_principals_hook show_limits format_supergrok_session footer_names_live_principal limits_json_lists_two_supergrok_principals_when_both_slots_exist limits_json_honest_single_supergrok_session_cannot_see_team_plan

# 2c. Dual SuperGrok billing poll honesty (role fail notes, fill provenance, rank, doctor)
cargo test -p xai-grok-shell --lib -- auth_failed_poll billing_fail_note remember_poll_ok order_live_prefers_poll_ok format_human_dual_poll sibling_poll_skips_after_n session_needs_oidc_refresh ensure_fresh_refreshes_expired find_and_persist_refreshed
cargo test -p xai-grok-pager --lib -- dual_fill_provenance compact_status_active_auth_failed format_unified_fills format_dual limits_honesty

# 3. DOGE default / Human green rail + role map / hide_header / window titles / title items + bubble + clear-done
cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config theme doge doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_prefix_matches recap_accent
cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session
cargo test -p xai-grok-pager --test settings_e2e -- hide_header
cargo test -p xai-grok-pager --lib -- bubble_copy_ append_bubble_copy_button_paints \
  clicking_human_bubble_copy clicking_assistant_bubble_copy clicking_wide_human_bubble_copy \
  clear_completed_todos

# 4. Plan soft-park A
cargo test -p xai-grok-pager --lib -- plan softer_park toast focus_plan plan_approval soft_park

# 5. session_reader / plan_validate / bulk_edit intercepts
cargo test -p xai-grok-tools --lib -- session_reader plan_validate bulk_edit_policy implement_memory opencode edit

# 5b. TUI self-screenshot
cargo test -p xai-grok-pager-render --lib -- tui_screenshot
cargo test -p xai-grok-pager --lib -- screenshot:: capture_tui_screenshot try_attach_tui_screenshot

# 5c. StreamResumed emit + stream headers timeout + transport footer
# Pager retry_chrome / clip_retry_reason / retrying_* and shell_collision have
# no matching fn (honesty leftover; not required land).
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason retry_footer_backoff stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout
cargo test -p xai-grok-sampler --test peer_process_rate_limit -- peer_process_does_not_sample_during_shared_rate_limit_cooldown
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
grok-nix-helper assert-process-pins
```

---

## Operator cheat sheet (post-recon)

Minimum after import restore or onto tip land. Same seven classes as FORK
§ *Land checklist*, plus extra neighbors this catalog lists. Chrome-only is a
failed land. `just upstream-land-filters` reminds after assert. It does not
replace the cargo blocks or `just check`.

```bash
just upstream-assert-process-pins
grok-nix-helper assert-process-pins HEAD   # or onto tip
just upstream-land-filters              # assert + reminder; then run this sheet

# 1. CLI identity (first token grok-oss; substring "grok" is not enough)
cargo test -p xai-grok-pager --lib -- product_version_line_uses_grok_oss_not_bare_grok \
  resume_session_command_uses_grok_oss user_guide_resume_and_version_examples_use_grok_oss \
  user_guide_operator_cli_examples_use_grok_oss product_cli_name_is_grok_oss \
  print_exit_resume_hint_writes_expected_lines welcome_badge_brands_grok_oss \
  hero_subtitle_brands_grok_oss tutorial_list_title_brands_grok_oss
cargo test -p xai-grok-pager-bin --test version_without_tty

# 2. Config is a surface
cargo test -p xai-grok-pager --test settings_e2e -- hide_header always_expand_thinking \
  scrub_ascii_punct allow_worktree bubble_copy_buttons plan_approval_park
cargo test -p xai-grok-pager --lib -- theme_choices_include_doge_and_default_is_doge \
  hide_header_zeroes always_expand_thinking bubble_copy_buttons_on \
  append_bubble_copy_button_paints clicking_human_bubble_copy clicking_assistant_bubble_copy \
  clicking_wide_human_bubble_copy
cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title
cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub_ascii_punct_from_ui
cargo test -p xai-grok-shell --lib -- resolve_subagents_copies_allow_worktree

# 3. Token Economy ledger /spend (extra SQL, not SuperGrok dollar credits)
cargo test -p xai-grok-shell --lib -- spend_path_ingests_usage_jsonl_and_records_reconciliation
cargo test -p xai-grok-pager --lib -- show_spend_ingests_usage_jsonl_and_is_not_empty_default

# 4. DOGE / chrome paint
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config \
  doge_accent_user_is_pure_green
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_entry_renderer_paints_green_rail \
  paint_composer_box_cursor_uses_human focused_composer_paints_human_green_box_caret \
  doge_human_box_caret_plate_is_rgb_0_255_0 paint_composer_box_cursor_named_ansi_green_becomes_doge_rgb \
  agent_message_block_accent info_line_model_name_uses_accent_model \
  status_bar_pushes_credits_compact_included_supergrok_period_limits \
  hit_credits_click_dispatches_show_limits \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  plan_approval_footer_paints_five_cta_vocabulary \
  auto_compact_completed_preserves_todo_board \
  compact_complete_does_not_reenqueue_occupancy_or_any_operator_prompt \
  auto_compact_completed_does_not_reenqueue_occupancy_or_any_operator_prompt \
  todo_badge_names_tasks_not_only_fraction \
  status_header_todo_badge_names_tasks \
  nested_l2_overlay_todo_toggle_stays_findable

# 5. Dual-auth hop after included SuperGrok period limits are full
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use sampling_config_hops_to_sibling_included_before_extras \
  sampling_config_hop_team_remaining_personal_exhausted_not_dollars_or_console \
  sampling_config_hop_personal_remaining_team_exhausted \
  sampling_config_hop_both_remaining_team_first_then_personal \
  sampling_config_hop_both_included_exhausted_dollar_credits_before_console \
  sampling_config_hop_missing_heavy_false_100_keeps_sibling_included \
  sampling_config_hop_dollar_credits_on_both_missing_heavy_keeps_team \
  resolve_model_to_sampling_config_auto_use \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
  prepare_sampler_for_turn_does_not_flatten_missing_heavy_100_off_sibling \
  prepare_sampler_for_turn_does_not_flatten_dollar_credits_on_both \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  order_credentials_business_included_before_personal_when_both_have_room \
  limits_snapshot_second_process_reads_file_and_does_not_http \
  limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once \
  limits_snapshot_never_writes_access_tokens \
  billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http \
  combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool \
  combined_included_remaining_does_not_collapse_matching_percent_and_reset_into_one_pool
cargo test -p xai-grok-pager --lib -- compact_meter_stays_included_while_sibling_pool_has_remaining \
  active_spend_driver_stays_included_while_any_distinct_pool_has_remaining \
  matching_percent_and_reset_does_not_collapse_combined_remaining_into_one_pool

# 6. Last-session on start
cargo test -p xai-grok-pager --lib -- materialize_new_auto_opens_last_session_when_one_exists \
  materialize_new_auto_stays_welcome_when_no_last_session \
  materialize_new_auto_does_not_open_last_when_headless \
  from_pager_args_opens_last_session_on_start

# 7. Product skills are not a Python runtime
cargo test -p xai-grok-bundle --lib -- sanitize_rejects_non_excepted_skill_python \
  extract_archive_skips_non_excepted_skill_python \
  product_repo_skill_roots_have_no_non_excepted_python \
  default_product_skills_include_polish_and_subagent
cargo test -p xai-grok-pager --lib -- user_guide_skills_are_not_a_python_runtime
cargo test -p xai-grok-tools --lib -- implement_memory_snapshot_intercept_does_not_spawn_shell \
  plan_validate_intercept_does_not_spawn_shell session_reader_list_intercept_does_not_spawn_shell

# Extra neighbors this catalog lists (not a second numbered board)
cargo test -p xai-grok-pager --lib -- exit_plan_mode_present_is_not_operator_approve \
  empty_enter_on_revise_prompt_does_not_approve \
  soft_park_empty_ctrl_c_abandons_plan_approval \
  exit_plan_mode_keeps_mid_compose_draft_and_a_types \
  exit_plan_mode_modal_park_does_not_steal_mid_compose_keys \
  exit_plan_mode_empty_present_printable_goes_to_composer \
  exit_plan_mode_shows_overlay_even_in_yolo \
  work_control_chrome_matrix_pause_not_cancel_stop_not_pause \
  pause_button_click_dispatches_global_pause_not_cancel \
  idle_with_subagents_paints_pause_and_stop_hits \
  global_paused_idle_paints_resume_not_stop \
  clear_finished_action_idle_is_quiet_not_neon_green_or_magenta \
  clear_finished_click_does_not_open_subagent \
  user_guide_does_not_claim_automatic_host_hop_is_unshipped \
  user_guide_names_token_economy_spend_order \
  user_guide_limits_names_fail_open_and_named_commands window_title titles_on_session
cargo test -p xai-grok-tools --lib -- exit_plan_mode_tool_result_does_not_claim_operator_approval \
  default_max_allows_l2_to_spawn_l3 rust_edit_verify dangerous_cargo
cargo test -p xai-grok-agent --lib -- child_task_description_is_concise
cargo test -p xai-grok-shell --lib -- from_config_without_prefetch_produces_usable_catalog \
  baked_default_is_grok_46_medium_fork_contract \
  stream_started_emits_retry_state_stream_resumed \
  keep_unverified_persisted_model_keeps_seeded_custom_slug \
  seeded_test_model_keeps_chat_completions_backend \
  leader_is_older_than_same_semver_git_sha_identity
cargo test -p xai-grok-pager --lib -- \
  plan_soft_park_docks_right_not_centered_overlay \
  plan_soft_park_draw_right_pane_matches_side_panel_status \
  plan_row_click_does_not_enter_commenting \
  plan_loop_status_does_not_claim_side_panel_when_viewer_closed \
  plan_preview_ctrl_z_restores_wiped_human_box \
  plan_prompt_ctrl_z_restores_wiped_human_box \
  preview_typed_comment_rides_along_on_approve \
  prompt_tab_typed_comment_rides_along_on_approve \
  esc_with_human_box_draft_keeps_feedback_draft \
  tab_preview_prompt_keeps_human_box_draft \
  exit_with_human_box_draft_does_not_drop_unsent_text \
  approve_with_composer_comments_sends_one_human_line \
  empty_approve_does_not_send_composer_as_second_prompt \
  resume_restore_keeps_revise_box_draft \
  isolated_present_preview_click_approve_does_not_drop_human_box_prompt \
  isolated_present_preview_typed_after_present_click_approve_sends_human_box_prompt \
  isolated_present_prompt_focus_click_approve_does_not_drop_human_box_prompt \
  isolated_present_click_approve_dispatches_interject_with_prompt_text \
  plan_human_box_keystroke_burst_does_not_flush_unsent_draft_every_char \
  main_composer_keystroke_burst_does_not_flush_unsent_draft_every_char \
  event_paste_plan_commenting_empty_defers_clipboard_image_probe \
  plan_feedback_ctrl_v_defers_clipboard_image_probe \
  agent_empty_bracketed_paste_defers_probe_for_clipboard_image \
  approve_or_revise_drains_plan_composer_images \
  compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct \
  format_supergrok_session_with_weekly_and_extras \
  live_subagent_list_does_not_show_two_rows_with_the_same_description \
  format_activity_label_unlimited_retry_has_no_u32_max_fraction \
  live_subagent_list_shows_only_l2_and_reports_live_l3_count \
  l2_row_shows_live_l3_count_not_specialist_names \
  parent_must_not_wait_for_the_model_after_waited_nested_already_completed \
  waiting_for_the_model_is_not_idle_when_nested_subagent_still_running \
  waiting_for_the_model_is_not_idle_when_prompt_is_queued \
  task_output_wait_clears_after_waited_nested_agent_completes \
  named_task_output_wait_clears_after_waited_nested_agent_completes \
  nested_overlay_drops_responding_after_child_acp_turn_completes \
  nested_overlay_title_elapsed_matches_host_subagent_finished_duration \
  reopening_finished_nested_overlay_does_not_start_climbing_title_clock \
  subagents_live_list_drops_responding_after_subagent_finished \
  l2_finish_surfaces_to_l1_without_opening_nested_overlay \
  wait_on_completed_nested_id_missing_from_map_does_not_stay_running \
  silent_subagent_completed_wake_surfaces_turn_completed \
  auto_compact_started_after_subagent_finished_does_not_set_live_compact_activity \
  overlay_nested_status_click_opens_l3_session_view \
  interjection_echo_does_not_duplicate_last_human_prompt \
  image_interject_leaves_one_prompt_and_empty_queue \
  start_while_globally_paused_continues_interrupted_turn_once \
  start_on_idle_clean_session_does_not_invent_a_turn \
  start_with_cancel_resume_marker_continues_interrupted_turn \
  handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn \
  handle_rebuild_done_idle_completed_turn_does_not_write_cancel_resume_or_refire_last_prompt \
  session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully \
  handle_rebuild_done_keeps_nested_subagents_for_resume \
  rebuild_and_relaunch_starts_while_nested_subagents_are_running \
  rebuild_subcommand_parses \
  prompt_wal_appends_on_enter_before_model_wait \
  prompt_wal_appends_on_mid_turn_interject \
  prompt_wal_appends_on_queue_enqueue \
  prompt_wal_appends_on_approve_notes \
  session_load_restores_wal_send_missing_from_prompt_history \
  resume_restore_must_not_put_the_same_operator_prompt_in_composer_and_queue \
  resume_restore_must_not_arm_enter_interject_when_no_live_sampler_turn \
  resume_restore_must_not_show_waiting_when_nested_and_sampler_are_gone \
  resume_restore_must_not_rehydrate_unsent_draft_and_queue_with_the_same_string \
  context_chip_names_sampling_window_when_catalog_differs \
  context_chip_hover_percent_uses_sampling_window_when_catalog_differs \
  footer_chip_uses_session_sampling_window_when_economic_cache_is_off \
  refresh_context_used_does_not_copy_catalog_into_session_sampling \
  management_meter_cache_policy_collect_force_background_honor_ttl \
  should_clear_management_meter_caches_force_with_key_only
cargo test -p xai-grok-tools --lib -- task_spawn_rejects_or_replaces_second_live_same_description \
  completed_subagent_task_output_is_capped_or_points_at_report \
  blocking_spawn_subagent_completed_to_prompt_format_is_capped
cargo test -p xai-grok-agent --lib -- implement_effort_two_does_not_spawn_two_review_rows_unless_operator_asked
cargo test -p xai-grok-update --lib -- failed_install_must_not_replace_or_signal_peers \
  build_fail_does_not_signal_leaders parse_version_output_extracts_identity \
  peer_relaunch_accepts_same_semver_different_sha \
  peer_relaunch_declines_equal_identity_on_same_path \
  peer_relaunch_accepts_deleted_inode_even_when_identity_equal \
  rebuild_signals_each_pid_after_composite_key \
  peer_pids_to_signal_excludes_self_dead_and_non_grok
cargo test -p xai-grok-workspace --lib -- repeated_open_without_close_keeps_one_search_per_root \
  distinct_roots_each_keep_one_search get_results_does_not_keep_a_stale_search_alive
cargo test -p xai-grok-shell --test test_image_strip_recovery -- \
  poisoned_image_session_recovers_within_the_failing_turn
cargo test -p xai-grok-shell --lib -- \
  limits_snapshot_mode_for_get_billing_explicit_is_force_refresh \
  main_session_sampling_window_is_catalog_500k_even_when_economic_is_on \
  nested_session_sampling_window_stays_200k_when_catalog_is_500k \
  relaunch_drain_keeps_nested_ids_alive_after_grace_like_disconnect \
  relaunch_drain_keeps_parent_turn_until_idle_like_disconnect
cargo test -p xai-grok-sampling-types --lib -- fold_spawn_prompt
cargo test -p xai-chat-state --lib -- parent_estimated_tokens_omit_huge_spawn_prompt
cargo test -p xai-tool-types --lib -- to_model_text_caps_huge_last_answer_for_parent_ingest
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason \
  retry_footer_backoff stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout
cargo test -p xai-grok-sampler --test peer_process_rate_limit -- peer_process_does_not_sample_during_shared_rate_limit_cooldown

just check   # full gate before push/PR; cannot fail a deleted catalog test
```

**Name check before cargo:** `rg` each cheat-sheet identifier for a matching
`fn`. Helper-only green is a lie. Do not `rg` honesty leftovers that have no
`fn` (`retry_chrome_soft_reconnects_when_retry_stream_starts`,
`stream_resumed_without_prior_retry_clears_activity`, `clip_retry_reason_*`,
`retrying_activity_label_*`, `retrying_label_shows_timeout_*`,
`shell_collision_contract_covers_every_pager_command_and_alias`,
`default_title_items_include_agents`, `title_escape_never_empty_payload`,
`title_updates_gated_only_by_title_enabled`,
economic-mode
slash BuiltinAction, SuperGrok Heavy ranking optional label,
`default_multipoll_out_dir`, Hierarchical fast path cargo `fn`).

**Dogfood screenshot list** (after assert + catalog; operator check, not the
only check): Human/agent rails, titled composer white frame with yellow title
only, plan four idle CTAs (Approve / Comment / Revise / Exit), included SuperGrok period limits compact meter (click
opens `/limits`), SIGUSR1 fleet still alive after a **failed** install. Do not
accept "compile mop re-applied seams" without the seven-class cargo list.

**User-guide on onto:** shared path under
`crates/codegen/xai-grok-pager/docs/user-guide/` is **not** in `FORK_PATHS`.
Resolve conflicts for `/limits`, DOGE default, window titles / `title.enabled`
vs `hide_header`, and Grok OSS branding sections; do not wholesale-pin the
guide to Surmount. A guide with zero `/limits` hits is a failed land.

#### Vendored bm25 has no fxhash

crates.io `bm25` 2.3.2 pulled unmaintained `fxhash` 0.2.1
([RUSTSEC-2025-0057](https://rustsec.org/advisories/RUSTSEC-2025-0057.html),
accessed: 2026-08-27). Land must keep the path patch and lockfile without
that package.

| path::test | Contract |
|------------|----------|
| grok-nix-helper `justfile_contracts` `workspace_lockfile_has_no_unmaintained_fxhash` | `Cargo.lock` has no `fxhash` package; workspace patches `bm25` to `third_party/bm25` (rustc-hash) |

```bash
cargo test -p grok-nix-helper --lib -- workspace_lockfile_has_no_unmaintained_fxhash
```

Shell haystack ranking tests stay named (`haystack_bm25_*` in
`xai-grok-shell` `session/tool_index_tests.rs`). Do not
`cargo audit --ignore RUSTSEC-2025-0057`.

#### Vendored rhai has no smartstring

crates.io `rhai` 1.25.1 / 1.26.0 pulled unmaintained `smartstring` 1.0.1
([RUSTSEC-2026-0249](https://rustsec.org/advisories/RUSTSEC-2026-0249.html),
accessed: 2026-08-27). Land must keep the path patch and lockfile without
that package.

| path::test | Contract |
|------------|----------|
| grok-nix-helper `justfile_contracts` `workspace_lockfile_has_no_unmaintained_smartstring` | `Cargo.lock` has no `smartstring` package; workspace patches `rhai` to `third_party/rhai` (compact_str) |

```bash
cargo test -p grok-nix-helper --lib -- workspace_lockfile_has_no_unmaintained_smartstring
```

xai-workflow named tests stay. Do not
`cargo audit --ignore RUSTSEC-2026-0249`.

---

## Related

| Path | Role |
|------|------|
| [`FORK.md`](../../FORK.md) § *Upstream regression filters* | D1 one-page cheat + recon table |
| [`RESIDUAL.md`](../../RESIDUAL.md) § *Validate honesty* | D0 open residual mirror (may demote) |
| [`docs/upstream-history.md`](../../docs/upstream-history.md) | Import review checklist |
| `grok-nix-helper assert-process-pins` | Path presence gate (catalog file + seven class titles; not crate `fn`s) |
| `just upstream-land-filters` | Assert plus reminder to walk this catalog. Not a second inventory. |
| `doc/dev/research/fork-paths-hardening-2026-07-24.md` | Why FORK_PATHS + assert (list authority = grok-nix-helper import-upstream-export) |

*Catalog created 2026-07-30 from explore join inventory.*
