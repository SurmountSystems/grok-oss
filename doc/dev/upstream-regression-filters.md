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
| Process pins present | `./scripts/assert-process-pins.sh` or `just upstream-assert-process-pins` (+ optional `HEAD` / onto tip) |

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
  hide_header_zeroes always_expand_thinking_keeps_blocks_expanded bubble_copy_buttons_on \
  append_bubble_copy_button_paints clicking_human_bubble_copy clicking_assistant_bubble_copy \
  clicking_wide_human_bubble_copy
cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub_ascii_punct_from_ui
cargo test -p xai-grok-shell --lib -- resolve_subagents_copies_allow_worktree
```

### 3. grok-oss SQL extras (Token Economy ledger /spend; not SuperGrok dollar credits)

`$GROK_HOME/grok_oss.db` is the Token Economy ledger, not the session store.
This class is extra SQL in that ledger. It is not SuperGrok dollar credits.
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
| `paint_composer_box_cursor_uses_human_green_not_agent_magenta` + `focused_composer_paints_human_green_box_caret_*` | Box caret is Human green, never agent magenta |
| `agent_message_block_accent_is_magenta_rail_under_doge_while_running` | Running agent rail is magenta |
| `info_line_model_name_uses_accent_model_not_gray` | Model label uses `accent_model` (magenta under DOGE) |
| `status_bar_pushes_credits_compact_included_supergrok_period_limits` | Status bar pushes `"credits"` and paints `included SuperGrok period limits · N%` |
| `hit_credits_click_dispatches_show_limits` | Click on the compact meter dispatches `ShowLimits` |
| `titled_doge_composer_frame_is_prompt_border_not_context_yellow` | Titled composer frame is `prompt_border_active` (white); title only is yellow |
| `plan_approval_footer_paints_five_cta_vocabulary` | Idle plan panel footer paints Approve / Comment / Revise / Exit. Clarify is only after Comment, not an idle top-level CTA |
| `default_theme_is_doge` | Unset theme resolves to DOGE |

Closest existing spinner/glyph neighbors (not a lower-left magenta throbber
paint `fn`; do not catalog the missing names `doge_idle_subagent_still_running`
/ `doge_tool_running_spinner`):
`doge_activity_spinners_use_striped_down_marquee_not_braille`,
`idle_with_subagents_renders_still_running_cue`.

```bash
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config \
  doge_accent_user_is_pure_green
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_entry_renderer_paints_green_rail \
  paint_composer_box_cursor_uses_human focused_composer_paints_human_green_box_caret \
  agent_message_block_accent info_line_model_name_uses_accent_model \
  status_bar_pushes_credits_compact_included_supergrok_period_limits \
  hit_credits_click_dispatches_show_limits \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  plan_approval_footer_paints_five_cta_vocabulary
```

### 5. Dual-auth hop after included SuperGrok period limits are full

Rank-only tests in `supergrok_identity_rank.rs` are not this class.

| path::test | Contract |
|------------|----------|
| `sampling_config_auto_use_fills_console_hop_after_included_full` | `sampling_config` fills console failover when included SuperGrok period limits are full |
| `sampling_config_auto_use_omits_console` / `sampling_config_auto_use_omits_console_while_supergrok_included_headroom` | While included SuperGrok period limits still have room, stay on SuperGrok (no console hop) |
| `resolve_model_to_sampling_config_auto_use` | Resolve path uses the same auto-use hop policy |
| `sampling_config_auto_use_extras_keep_session_console_failover` | SuperGrok dollar credits keep session plus console failover (single SuperGrok identity) |
| `sampling_config_hops_to_sibling_included_before_extras` | Personal included SuperGrok period limits full + extras hops to Business included before SuperGrok dollar credits |
| `afterburner_does_not_skip_mark_when_sibling_has_included_remaining` | After-burner extras skip the out of included SuperGrok period limits mark only when every distinct included pool is exhausted |
| `align_after_billing_switches_sticky_personal_full_to_business_included` | After billing, `align_to_ranked_free_period_primary` switches sticky personal-full to Business included |
| `prepare_sampler_for_turn_aligns_to_ranked_included_primary` | Per-turn reconstruct uses the ranked included SuperGrok period primary JWT |
| `pick_prefers_business_included_before_personal_when_both_have_remaining` | When both stored SuperGrok logins still have included remaining, pick Business / Team first |
| `order_credentials_business_included_before_personal_when_both_have_room` | Credential order spends Business included before personal included while both have room |
| `limits_snapshot_second_process_reads_file_and_does_not_http` | Second grok-oss process reads `$GROK_HOME/limits_snapshot.json` and does not call SuperGrok billing HTTP |
| `limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once` | A stale snapshot lets the next exclusive-flock holder fetch once |
| `limits_snapshot_never_writes_access_tokens` | Shared snapshot never stores JWTs or API keys |
| `billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http` | `x.ai/billing` uses the snapshot hub instead of unconditionally HTTP-ing siblings |

```bash
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use sampling_config_hops_to_sibling_included_before_extras \
  resolve_model_to_sampling_config_auto_use \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
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
| `combined_included_remaining_does_not_double_count_unified_pool` | Unified pool (wire flag or same floored percent + reset) counts once |
| `compact_meter_stays_included_while_sibling_pool_has_remaining` | Compact chrome stays on included SuperGrok period limits while a sibling pool has remaining |
| `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining` | Active spend driver stays included SuperGrok period limits while any distinct pool has remaining |

```bash
cargo test -p xai-grok-shell --lib -- combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool
cargo test -p xai-grok-pager --lib -- compact_meter_stays_included_while_sibling_pool_has_remaining \
  active_spend_driver_stays_included_while_any_distinct_pool_has_remaining
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
| `xai-grok-pager` `docs::user_guide_skills_are_not_a_python_runtime` | User-guide `08-skills.md` says skills are not a Python runtime and names the exceptions |
| `xai-grok-tools` `implement_memory_snapshot_intercept_does_not_spawn_shell` | `memory.py` CLI is Rust; no Python process |
| `xai-grok-tools` `plan_validate_intercept_does_not_spawn_shell` | `validate-plan.py` CLI is Rust; no Python process |
| `xai-grok-tools` `session_reader_list_intercept_does_not_spawn_shell` | `session_reader.py` CLI is Rust; no Python process |

```bash
cargo test -p xai-grok-bundle --lib -- sanitize_rejects_non_excepted_skill_python \
  extract_archive_skips_non_excepted_skill_python \
  product_repo_skill_roots_have_no_non_excepted_python
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
catalog 500k as the AUTO gate.

| path::test | Contract |
|------------|----------|
| `xai-grok-pager` `context_chip_names_sampling_window_when_catalog_differs` | Chip names sampling and catalog when they differ |
| `xai-grok-pager` `context_chip_hover_percent_uses_sampling_window_when_catalog_differs` | Hover percent is of the sampling window |

```bash
cargo test -p xai-grok-pager --lib -- context_chip_names_sampling_window_when_catalog_differs \
  context_chip_hover_percent_uses_sampling_window_when_catalog_differs
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

#### Always-three-layer product prompt

Process law stays in AGENTS. These cargo names keep the product prompt from
teaching the old "many greps / half the window" rule after a restack.

| path::test | Contract |
|------------|----------|
| `xai-grok-agent` `child_task_description_is_concise` | L2 task description says three layers always and must spawn L3 |
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

```bash
cargo test -p xai-grok-pager --lib -- user_guide_does_not_claim_automatic_host_hop_is_unshipped \
  user_guide_names_token_economy_spend_order
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

```bash
cargo test -p xai-grok-pager --lib -- \
  plan_soft_park_docks_right_not_centered_overlay \
  plan_soft_park_draw_right_pane_matches_side_panel_status \
  plan_row_click_does_not_enter_commenting \
  plan_loop_status_does_not_claim_side_panel_when_viewer_closed
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
| `xai-grok-sampler` `actor::request_task::retry_footer_reason_uses_short_transport_label` | Short transport footer (not opaque `Transport error: error`) |
| `xai-grok-sampler` `actor::request_task::retry_footer_backoff_hint_appends_next_try_in` | Backoff suffix `· next try in Ns` |
| `xai-grok-sampler` `client::tests::stream_headers_timeout_defaults_to_120_secs_when_env_unset` | Default stream headers timeout is **120s** when env unset (`0` / invalid → 120; positive override honored) |
| `xai-grok-sampler` **integration** `stream_headers_timeout::streaming_execute_times_out_waiting_for_headers` | Hang after accept, no headers → fail within headers budget (`GROK_STREAM_HEADERS_TIMEOUT_SECS=1` in that binary) |

```bash
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason retry_footer_backoff stream_headers_timeout_defaults
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
`auto_compact_completed_preserves_todo_board`; skills order
(`agents_home_skills_shadow_grok_user_skills`,
`local_agents_skills_shadow_local_grok_skills`); UDAX toon filters; plan
soft-park filters. Extra restack-droppable neighbors live under *Required land
inventory* (plan present is not Approve, three-layer product prompt,
`from_config` cold catalog, SHA-aware `/rebuild`, nucleo, Pause / Clear
finished, user-guide hop and spend-order pins, seeded custom model on
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
| `hide_header_zeroes_*` | `hide_header` zeros status / welcome / dashboard chrome | **Keep** (serde default tests are not paint) |
| `failed_install_must_not_replace_or_signal_peers` | Failed `/rebuild` install must not replace the binary or SIGUSR1 peers | **Keep** |
| `version_without_tty` | `--version` first token is `grok-oss` with no TTY | **Keep** (substring `grok` is forbidden) |

```bash
# Prove the names still exist (missing fn = land failed), then run:
cargo test -p xai-grok-pager --lib -- status_bar_pushes_credits_compact_included_supergrok_period_limits \
  hit_credits_click_dispatches_show_limits \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  plan_approval_footer_paints_five_cta_vocabulary \
  auto_compact_completed_preserves_todo_board hide_header_zeroes
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

Minimum after import restore or onto tip land. Same seven classes as FORK
§ *Land checklist*, plus extra neighbors this catalog lists. Chrome-only is a
failed land. `just upstream-land-filters` reminds after assert. It does not
replace the cargo blocks or `just check`.

```bash
just upstream-assert-process-pins
./scripts/assert-process-pins.sh HEAD   # or onto tip
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
  hide_header_zeroes always_expand_thinking_keeps_blocks_expanded bubble_copy_buttons_on \
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
  agent_message_block_accent info_line_model_name_uses_accent_model \
  status_bar_pushes_credits_compact_included_supergrok_period_limits \
  hit_credits_click_dispatches_show_limits \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  plan_approval_footer_paints_five_cta_vocabulary \
  auto_compact_completed_preserves_todo_board

# 5. Dual-auth hop after included SuperGrok period limits are full
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use sampling_config_hops_to_sibling_included_before_extras \
  resolve_model_to_sampling_config_auto_use \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  order_credentials_business_included_before_personal_when_both_have_room \
  limits_snapshot_second_process_reads_file_and_does_not_http \
  limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once \
  limits_snapshot_never_writes_access_tokens \
  billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http \
  combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool
cargo test -p xai-grok-pager --lib -- compact_meter_stays_included_while_sibling_pool_has_remaining \
  active_spend_driver_stays_included_while_any_distinct_pool_has_remaining

# 6. Last-session on start
cargo test -p xai-grok-pager --lib -- materialize_new_auto_opens_last_session_when_one_exists \
  materialize_new_auto_stays_welcome_when_no_last_session \
  materialize_new_auto_does_not_open_last_when_headless \
  from_pager_args_opens_last_session_on_start

# 7. Product skills are not a Python runtime
cargo test -p xai-grok-bundle --lib -- sanitize_rejects_non_excepted_skill_python \
  extract_archive_skips_non_excepted_skill_python \
  product_repo_skill_roots_have_no_non_excepted_python
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
  user_guide_names_token_economy_spend_order window_title titles_on_session
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
  event_paste_plan_commenting_empty_defers_clipboard_image_probe \
  plan_feedback_ctrl_v_defers_clipboard_image_probe \
  agent_empty_bracketed_paste_defers_probe_for_clipboard_image \
  approve_or_revise_drains_plan_composer_images \
  compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct \
  format_supergrok_session_with_weekly_and_extras \
  live_subagent_list_does_not_show_two_rows_with_the_same_description \
  format_activity_label_unlimited_retry_has_no_u32_max_fraction
cargo test -p xai-grok-tools --lib -- task_spawn_rejects_or_replaces_second_live_same_description
cargo test -p xai-grok-agent --lib -- implement_effort_two_does_not_spawn_two_review_rows_unless_operator_asked
cargo test -p xai-grok-update --lib -- failed_install_must_not_replace_or_signal_peers \
  build_fail_does_not_signal_leaders parse_version_output_extracts_identity \
  peer_relaunch_accepts_same_semver_different_sha \
  peer_relaunch_declines_equal_identity_on_same_path \
  peer_relaunch_accepts_deleted_inode_even_when_identity_equal
cargo test -p xai-grok-workspace --lib -- repeated_open_without_close_keeps_one_search_per_root \
  distinct_roots_each_keep_one_search get_results_does_not_keep_a_stale_search_alive
cargo test -p xai-grok-shell --test test_image_strip_recovery -- \
  poisoned_image_session_recovers_within_the_failing_turn
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason \
  retry_footer_backoff stream_headers_timeout_defaults
cargo test -p xai-grok-sampler --test stream_headers_timeout

just check   # full gate before push/PR; cannot fail a deleted catalog test
```

**Name check before cargo:** `rg` each cheat-sheet identifier for a matching
`fn`. Helper-only green is a lie. Do not `rg` honesty leftovers that have no
`fn` (`retry_chrome_soft_reconnects_when_retry_stream_starts`,
`stream_resumed_without_prior_retry_clears_activity`, `clip_retry_reason_*`,
`retrying_activity_label_*`, `retrying_label_shows_timeout_*`,
`shell_collision_contract_covers_every_pager_command_and_alias`,
`default_title_items_include_agents`, `title_escape_never_empty_payload`,
`title_updates_gated_only_by_title_enabled`).

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

---

## Related

| Path | Role |
|------|------|
| [`FORK.md`](../../FORK.md) § *Upstream regression filters* | D1 one-page cheat + recon table |
| [`RESIDUAL.md`](../../RESIDUAL.md) § *Validate honesty* | D0 open residual mirror (may demote) |
| [`docs/upstream-history.md`](../../docs/upstream-history.md) | Import review checklist |
| `scripts/assert-process-pins.sh` | Path presence gate (catalog file + seven class titles; not crate `fn`s) |
| `just upstream-land-filters` | Assert plus reminder to walk this catalog. Not a second inventory. |
| `doc/dev/research/fork-paths-hardening-2026-07-24.md` | Why FORK_PATHS + assert (list authority = import script) |

*Catalog created 2026-07-30 from explore join inventory.*
