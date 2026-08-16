# Surmount product seams a restack can drop

**Date:** 2026-08-15  
**Tree:** `/home/hunter/Projects/surmount/grok-build`  
**Mode:** read-only inventory. This turn did **not** run cargo or `just check`. **Shipped and proven** means the named `fn` exists in this tree and encodes the contract. It is not a fresh green nextest pass.

SuperGrok is a **paid** product. This report says **included SuperGrok period limits**, **SuperGrok dollar credits**, and **console team prepaid / console API credits**. Never "free SuperGrok."

## How this file is supposed to be used

The 1.0.3 restack kept `FORK_PATHS` files and many helpers. It dropped operator-visible seams inside `xai-grok-*`. Import restore never owned those seams. `scripts/assert-process-pins.sh` only proves files exist. Helper-green is a failed land. A chrome-only inventory is a failed land. Crate seams survive only via cherry-picks plus **named cargo tests**.

Prior reports under `.agents/reports/` (especially `fork-loss-postmortem-2026-08-13.md`, `fork-gaps-remaining-seams-2026-08-13.md`, `fork-gaps-config-options-2026-08-13.md`, `impl-fork-recon-land-pins.md`) named the drop. They are **not proof of the current tree**. Several seams those files called dropped now have restore `fn`s. Several filters the catalog still lists have **no matching `fn`**. Evidence below is from this tree.

**Status words**

| Status | Meaning |
|--------|---------|
| **shipped and proven** | Code path exists and a named `#[test]` encodes the contract |
| **shipped in code, no named test** | Call site exists; no `fn` that would go red if the seam is deleted |
| **docs only / unproven** | Docs or FORK claim it; this walk did not find both code and a named test |
| **open residual, not shipped** | Named contract is missing or catalog lists a deleted `fn` |

**Catalog** means [`doc/dev/upstream-regression-filters.md`](../../doc/dev/upstream-regression-filters.md). **FORK land** means [`FORK.md`](../../FORK.md) § *Land checklist* plus the cheat sheet.

---

## Class 1. CLI branding (`grok-oss`, not bare `grok`)

The product command is **grok-oss**. `--version` first token must be `grok-oss`. Operator-facing resume and relaunch hints are `grok-oss --resume`. Welcome chrome says **Grok OSS**, not leftover **Grok Build**. A substring check for `grok` is how `grok 1.0.3` stayed green.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| `--version` first token | `crates/codegen/xai-grok-pager/src/client_identity.rs` | `product_version_line_uses_grok_oss_not_bare_grok` | shipped and proven | yes |
| CLI name constant | same | `product_cli_name_is_grok_oss` | shipped and proven | yes |
| Resume paste | same | `resume_session_command_uses_grok_oss` | shipped and proven | yes |
| Quit / relaunch hint | `crates/codegen/xai-grok-pager/src/app/mod.rs` | `print_exit_resume_hint_writes_expected_lines` | shipped and proven | yes |
| No-TTY `--version` (ENXIO) | `crates/codegen/xai-grok-pager-bin/tests/version_without_tty.rs` (`assert_version_ok`) | `cargo test -p xai-grok-pager-bin --test version_without_tty` | shipped and proven | yes |
| Guide `--version` / `--resume` | `crates/codegen/xai-grok-pager/src/docs.rs` | `user_guide_resume_and_version_examples_use_grok_oss` | shipped and proven | yes |
| Guide leftover `grok login` / `grok sessions` | same | `user_guide_operator_cli_examples_use_grok_oss` | shipped and proven | **no** |
| Welcome badge **Grok OSS** | `crates/codegen/xai-grok-pager/src/views/welcome/mod.rs` | `welcome_badge_brands_grok_oss` | shipped and proven | **no** |
| Hero thanks-line | `.../welcome/hero_box.rs` | `hero_subtitle_brands_grok_oss` | shipped and proven | **no** |
| Tutorial list title | `.../views/tutorial.rs` | `tutorial_list_title_brands_grok_oss` | shipped and proven | **no** |
| Window OSC brand `grok-oss` | `.../app/mod.rs` `terminal_title_string` | `window_title_always_manages_non_empty_branded_osc`, `titles_on_session_name_osc_is_non_empty_branded`, `window_title_osc_payload_never_empty_string` | shipped and proven | yes (window_title / titles_on_session) |

**Land filters to keep:** the class 1 cheat sheet in the catalog, plus `user_guide_operator_cli_examples_use_grok_oss`, `welcome_badge_brands_grok_oss`, `hero_subtitle_brands_grok_oss`, `tutorial_list_title_brands_grok_oss`.

---

## Class 2. `/settings` plus unread config

A toml field that deserializes is not shipped if `/settings` has no row and nothing reads it. The 1.0.3 restack left serde helpers. Current tree has `/settings` rows in `settings/defs.rs` for `hide_header`, `always_expand_thinking`, `scrub_ascii_punct`, `allow_worktree`, `bubble_copy_buttons`, `plan_approval_park`, and theme default **doge**. Settings e2e dispatches the typed setters. Paint and spawn readers exist for the unread-restore set.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| `/settings` hide header toggle | `crates/codegen/xai-grok-pager/tests/settings_e2e.rs` | `hide_header_space_dispatches_typed_setter`, `hide_header_mouse_click_two_stage_toggles` | shipped and proven | yes (`hide_header`) |
| `/settings` always-expand thinking | same | `always_expand_thinking_*` | shipped and proven | yes |
| `/settings` ASCII scrub | same | `scrub_ascii_punct_*` | shipped and proven | yes |
| `/settings` worktrees | same | `allow_worktree_*` | shipped and proven | yes |
| `/settings` bubble copy | same | `bubble_copy_buttons_*` | shipped and proven | yes |
| `/settings` plan park | same | `plan_approval_park_*` | shipped and proven | yes |
| Theme picker includes DOGE; default doge | `.../settings/registry.rs` | `theme_choices_include_doge_and_default_is_doge` | shipped and proven | yes |
| `hide_header` zeros chrome | `views/agent.rs`, `views/welcome/mod.rs`, `views/dashboard/layout.rs` | `hide_header_zeroes_status_bar_height`, `hide_header_zeros_welcome_top_bar_height`, `hide_header_zeroes_header_and_header_gap` | shipped and proven | yes |
| Always-expand thinking at paint | `scrollback/blocks/thinking.rs` | `always_expand_thinking_keeps_blocks_expanded` | shipped and proven | yes |
| ASCII scrub seeded at launch | `xai-grok-pager-render` `appearance/cache.rs` | `prime_applies_scrub_ascii_punct_from_ui` | shipped and proven | yes |
| `[subagents] allow_worktree` copied onto runtime | `xai-grok-shell` `config/tests.rs`; used in `mvp_agent/subagent_coordinator.rs` | `resolve_subagents_copies_allow_worktree` | shipped and proven (copy). Spawn isolation uses `cfg.subagent_allow_worktree`. No named test that spawn isolation actually changes. | yes (copy only) |
| Serde default only | `xai-grok-shared` `ui_config.rs` | `hide_header_defaults_false_and_parses`, `stale_hide_title_bar_key_is_ignored` | shipped and proven as serde. **Not** this class by itself. | yes |

**Land filters to keep:** the class 2 cheat sheet. Do not accept serde-only green.

The 2026-08-13 config-gaps report is **stale** for those six unread keys. They have rows and readers now. Other FORK-claimed settings (Token Economy table in the GUI, economic-mode row, auto-run implement row) were not re-proven as `/settings` rows this turn. Do not claim those GUI rows from this walk.

---

## Class 3. grok-oss ledger `/spend`

`$GROK_HOME/grok_oss.db` is the Token Economy ledger, not the session store. Schema v1 without ingest is a lie. `/spend` must ingest `usage.jsonl` into `local_usage_event` and write `reconciliation_run`. The pager must format the live ledger, not `DoubleEntryReport::default()`.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Spend ingest + reconciliation | `xai-grok-shell` `token_economy/mod.rs` | `spend_path_ingests_usage_jsonl_and_records_reconciliation` | shipped and proven | yes |
| `/spend` is not empty default | `xai-grok-pager` `app/dispatch/tests/status.rs` | `show_spend_ingests_usage_jsonl_and_is_not_empty_default` | shipped and proven | yes |

**Land filters to keep:** those two exact names.

---

## Class 4. chrome / paint (DOGE, rails, meter, titled composer, five-CTA)

A theme file existing is not paint. Human chrome is **green** (`accent_user`). Agent activity is **magenta** (`accent_running` / `accent_model`). The compact status chip is **included SuperGrok period limits · N%**. Click opens `/limits`. The titled composer frame is `prompt_border_active` (white). The title only is yellow. Plan footer paints Approve / Notes / Clarify / Revise / Quit. Clear finished is quiet secondary, not neon green or magenta. Pause / resume chips live on the turn-status work-control row.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Unset theme is DOGE | `xai-grok-pager-render` `theme/cache.rs` | `default_theme_is_doge`, `resolve_from_config_no_config_returns_doge`, `resolve_auto_dark_system_returns_doge` | shipped and proven | yes |
| Dark map → DOGE | `theme/system_appearance.rs` | `to_theme_kind_dark_defaults_to_doge` | shipped and proven | yes |
| Human green `#00FF00` | `theme/doge.rs` | `doge_accent_user_is_pure_green_for_human` | shipped and proven | yes |
| System cyan for limits/credits | same | `doge_accent_system_is_pure_cyan_for_system_limits_credits` | shipped and proven | yes |
| Role map (no blue UI, no gray text) | same | `doge_roles_green_cyan_no_blue_ui_no_gray_text` | shipped and proven | yes (residual block) |
| Human left rail paints | `scrollback/blocks/user.rs` | `user_prompt_block_accent_is_static_human_rail`, `user_prompt_block_accent_is_green_rail_under_doge_default`, `user_prompt_entry_renderer_paints_green_rail`, `user_prompt_prefix_matches_human_rail_color` | shipped and proven | yes |
| Running agent rail magenta | `scrollback/blocks/agent.rs` | `agent_message_block_accent_is_magenta_rail_under_doge_while_running` | shipped and proven | yes |
| Composer box caret Human green | `views/prompt_widget/tests.rs` | `paint_composer_box_cursor_uses_human_green_not_agent_magenta`, `focused_composer_paints_human_green_box_caret_hides_terminal_cursor` | shipped and proven | yes |
| Model label uses `accent_model` | same | `info_line_model_name_uses_accent_model_not_gray` | shipped and proven | yes |
| Titled composer white frame | same | `titled_doge_composer_frame_is_prompt_border_not_context_yellow` | shipped and proven | yes |
| Status pushes `"credits"` included-period string | `app/agent_view/render.rs` | `status_bar_pushes_credits_compact_included_supergrok_period_limits` | shipped and proven | yes |
| Meter click → `ShowLimits` | same | `hit_credits_click_dispatches_show_limits` | shipped and proven | yes |
| Five-CTA footer | `views/file_search/line_viewer.rs` | `plan_approval_footer_paints_five_cta_vocabulary` | shipped and proven | yes |
| AutoCompact does not wipe todos | `app/acp_handler/tests/subagents.rs` | `auto_compact_completed_preserves_todo_board` | shipped and proven | yes (paint keep list) |
| Still-running cue (idle + subagents) | `views/turn_status.rs` | `idle_with_subagents_renders_still_running_cue` | shipped and proven | neighbor, not class 4 cheat sheet |
| Activity spinner is striped marquee, not braille | `pager-render` `glyphs.rs` | `doge_activity_spinners_use_striped_down_marquee_not_braille` | shipped and proven | catalog names it as neighbor, not a lower-left throbber color `fn` |
| Pause click is global pause, not cancel | `app/agent_view/render.rs` | `pause_button_click_dispatches_global_pause_not_cancel` | shipped and proven | **no** (catalog once listed `pause_button_click_dispatches_global_pause`; current name is longer) |
| Pause / stop matrix | `views/turn_status.rs` | `work_control_chrome_matrix_pause_not_cancel_stop_not_pause`, `idle_with_subagents_paints_pause_and_stop_hits`, `global_paused_idle_paints_resume_not_stop` | shipped and proven | **no** |
| Clear finished quiet paint | `scrollback/selection.rs` | `clear_finished_action_idle_is_quiet_not_neon_green_or_magenta`, hover / disabled siblings | shipped and proven | **no** |
| Clear finished hit / click | `render.rs`, `panes.rs` | `clear_finished_only_when_open_with_finished_rows`, `clear_finished_hit_does_not_intersect_tasks_subagent_open_or_kill`, `clear_finished_click_does_not_open_subagent`, `clear_completed_todos_x_key_only_when_todo_pane_focused` | shipped and proven | residual-aligned `clear_completed_todos` only |
| Recap idle rail stays tool-white | `scrollback/blocks/session_event.rs` | `recap_accent_and_bullet_use_neutral_tool_color_when_idle` | shipped and proven | yes (human-rail section) |

**Do not claim** live TUI dogfood. The catalog screenshot list stays an operator check after these `fn`s exist.

**Land filters to keep:** the class 4 cheat sheet in the catalog. Missing names `doge_idle_subagent_still_running` and `doge_tool_running_spinner` are still absent. Do not invent them.

---

## Class 5. Dual-auth hop after included SuperGrok period limits are full

Rank helpers are not hop. After included SuperGrok period limits are full, `sampling_config` must fill console failover. While those included limits still have room, stay on SuperGrok session. Spend Business / Team included before personal included. Sibling included SuperGrok period limits beat this login's SuperGrok dollar credits. Combined remaining sums distinct pools and does not double-count a unified pool. One `grok-oss` process fetches SuperGrok billing. Others read `$GROK_HOME/limits_snapshot.json`. The snapshot never stores JWTs or API keys.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Console hop after included full | `xai-grok-shell` `agent/config_tests.rs` | `sampling_config_auto_use_fills_console_hop_after_included_full` | shipped and proven | yes (`sampling_config_auto_use`) |
| Omit console while included has room | same + `agent/models/tests.rs` | `sampling_config_auto_use_omits_console`, `sampling_config_auto_use_omits_console_while_supergrok_included_headroom` | shipped and proven | yes |
| Resolve path same policy | `config_tests.rs` | `resolve_model_to_sampling_config_auto_use` | shipped and proven | yes (class 5 table; cheat sheet is shorter) |
| Dollar credits keep session + console failover | same | `sampling_config_auto_use_extras_keep_session_console_failover` | shipped and proven | yes |
| Sibling included before extras | same | `sampling_config_hops_to_sibling_included_before_extras` | shipped and proven | yes |
| After-burner skip only when every included pool is exhausted | `auth/allowance_exhaust_from_billing.rs` | `afterburner_does_not_skip_mark_when_sibling_has_included_remaining` | shipped and proven | yes (FORK cheat sheet; catalog §5) |
| After billing, sticky personal-full → Business included | `auth/manager_tests.rs` | `align_after_billing_switches_sticky_personal_full_to_business_included` | shipped and proven | catalog §5 table; **not** FORK cheat sheet |
| Per-turn reconstruct uses ranked included primary | `session/acp_session_impl/sampler_turn.rs` | `prepare_sampler_for_turn_aligns_to_ranked_included_primary` | shipped and proven | catalog §5 table; **not** FORK cheat sheet |
| Pick Business included first | `auth/supergrok_identity_rank.rs` | `pick_prefers_business_included_before_personal_when_both_have_remaining` | shipped and proven | yes |
| Credential order Business before personal | same | `order_credentials_business_included_before_personal_when_both_have_room` | shipped and proven | yes |
| Combined remaining sums distinct pools | same | `combined_included_remaining_sums_distinct_personal_and_business_pools` | shipped and proven | yes (§5b) |
| Unified pool counts once | same | `combined_included_remaining_does_not_double_count_unified_pool` | shipped and proven | yes (§5b) |
| Compact meter stays included while sibling remaining | `pager` `views/credit_bar.rs` | `compact_meter_stays_included_while_sibling_pool_has_remaining` | shipped and proven | yes |
| Active spend driver same | same | `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining` | shipped and proven | yes |
| Second process reads snapshot, no HTTP | `auth/limits_snapshot_hub.rs` | `limits_snapshot_second_process_reads_file_and_does_not_http` | shipped and proven | yes |
| Stale snapshot: next flock holder fetches once | same | `limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once` | shipped and proven | yes |
| Snapshot never writes tokens | same | `limits_snapshot_never_writes_access_tokens` | shipped and proven | yes |
| Billing handler uses hub | `extensions/billing.rs` | `billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http` | shipped and proven | yes |

**Land filters to keep:** catalog §5 plus §5b. Rank-only tests in `supergrok_identity_rank.rs` that do not mention `sampling_config` are not this class.

---

## Class 6. Last-session on start

Interactive `grok-oss` with a remembered last session for this working directory opens that session. It does not land on Welcome first. First-ever use stays Welcome. Headless does not steal last-session. This is not continue interrupted turn (`canceled_turn_resume.json`) and not `/resume`.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Opens last session | `app/session_startup.rs` | `materialize_new_auto_opens_last_session_when_one_exists` | shipped and proven | yes |
| No last session → Welcome | same | `materialize_new_auto_stays_welcome_when_no_last_session` | shipped and proven | **no** |
| Headless does not open last | same | `materialize_new_auto_does_not_open_last_when_headless` | shipped and proven | **no** |
| Interactive ctx flag | same | `from_pager_args_opens_last_session_on_start` | shipped and proven | **no** |
| User-guide last-session sentences | `docs/user-guide/01-getting-started.md`, `17-sessions.md` | no dedicated `fn` | shipped in code (guide text exists). No cargo pin that those sentences stay. | no |

**Land filters to keep:** `materialize_new_auto_opens_last_session_when_one_exists` is the required land name. The three sibling `fn`s should enroll so Welcome / headless cannot regress silently.

---

## Class 7. Product skills are not a Python runtime

A restack that installs non-excepted Python under product skills, or that drops the Rust intercept for the allowlisted CLI forms, is a failed land. Exceptions: office/docx/pptx/xlsx/pdf scripts, and the three intercept stubs (`memory.py`, `validate-plan.py`, `session_reader.py`). Host `~/.agents/skills` is operator-owned and is not this class. User-guide `08-skills.md` must keep the sentence.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Sanitize rejects junk `.py` | `xai-grok-bundle` `lib.rs` | `sanitize_rejects_non_excepted_skill_python` | shipped and proven | yes |
| Archive extract skips junk `.py` | same | `extract_archive_skips_non_excepted_skill_python` | shipped and proven | yes |
| Project skill roots have no junk `.py` | same | `product_repo_skill_roots_have_no_non_excepted_python` | shipped and proven | yes |
| Guide sentence + exceptions | `pager` `docs.rs` | `user_guide_skills_are_not_a_python_runtime` | shipped and proven | yes |
| `memory.py` is Rust | `xai-grok-tools` `bash/mod.rs` | `implement_memory_snapshot_intercept_does_not_spawn_shell` | shipped and proven | yes |
| `validate-plan.py` is Rust | same | `plan_validate_intercept_does_not_spawn_shell` | shipped and proven | yes |
| `session_reader.py` is Rust | same | `session_reader_list_intercept_does_not_spawn_shell` | shipped and proven | yes |

---

## Extra class A. Always-on bubble copy (paint plus click)

Paint-only bubble copy is a failed land. Flag on paints `⧉`. A full-width first line still paints a hit. Click on the human glyph copies that prompt. Click on the assistant glyph copies that message.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Flag paints icon | `scrollback/blocks/user.rs` | `bubble_copy_buttons_on_paints_copy_icon` | shipped and proven | yes |
| Full first line still paints | same | `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width` | shipped and proven | yes |
| Helper marks hit when line fills width | `scrollback/blocks/mod.rs` | `append_bubble_copy_button_paints_when_first_line_fills_content_width` | shipped and proven | yes |
| Human click copies prompt | `app/mouse.rs` | `clicking_human_bubble_copy_copies_the_prompt` | shipped and proven | yes |
| Assistant click copies message | same | `clicking_assistant_bubble_copy_copies_the_message` | shipped and proven | yes |
| Wide human still paints and copies | same | `clicking_wide_human_bubble_copy_still_paints_and_copies` | shipped and proven | yes |

This is already under catalog class 2 plus the dedicated bubble-copy section. FORK land class 4 chrome list does not name click-to-copy. A restack that keeps paint tests and drops click tests is the 1.0.3 failure mode.

---

## Extra class B. Plan five-CTA, present ≠ approve, modal-free typing

`exit_plan_mode` presents the plan. It is not operator Approve. Always-approve permission mode does not click the CTA. Empty Enter never approves. Soft-park must not steal mid-compose keys. Mid-compose draft stays. Printable keys go to the composer.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Footer vocabulary | `line_viewer.rs` | `plan_approval_footer_paints_five_cta_vocabulary` | shipped and proven | yes |
| Present is not Approve | `app/agent_view/plan.rs` | `exit_plan_mode_present_is_not_operator_approve` | shipped and proven | residual `plan` / `soft_park`; **not** named on the seven-class cheat sheet |
| Tool result does not claim approval | `xai-grok-tools` `exit_plan_mode/mod.rs` | `exit_plan_mode_tool_result_does_not_claim_operator_approval` | shipped and proven | **no** |
| Empty Enter on revise does not approve | `plan.rs` | `empty_enter_on_revise_prompt_does_not_approve` | shipped and proven | residual plan block |
| Soft-park empty Ctrl+C abandons | same | `soft_park_empty_ctrl_c_abandons_plan_approval` | shipped and proven | residual |
| Mid-compose draft + `a` types | `acp_handler/tests/plan_mode.rs` | `exit_plan_mode_keeps_mid_compose_draft_and_a_types` | shipped and proven | residual |
| Modal park does not steal keys | same | `exit_plan_mode_modal_park_does_not_steal_mid_compose_keys` | shipped and proven | residual |
| Empty present printable → composer | same | `exit_plan_mode_empty_present_printable_goes_to_composer` | shipped and proven | residual |
| Always-approve still parks preview | same file family (`exit_plan_mode_shows_overlay_even_in_yolo`) | `exit_plan_mode_shows_overlay_even_in_yolo` | shipped and proven | residual |
| Settings park picker | `settings_e2e.rs` | `plan_approval_park_*` | shipped and proven | yes (class 2) |

FORK neighbor cheat sheet uses `exit_plan_mode_soft`. That substring still hits several `fn`s. Prefer the exact present-is-not-approve names above so a compile mop cannot keep a soft-park helper and drop the honesty test.

---

## Extra class C. Always-three-layer agent depth (product plus process)

Process law lives in `AGENTS.md` / host AGENTS (D1). Product also teaches it: L2 task description must say three layers always, must spawn L3, must not teach "many greps" / "half the window." Default max depth must let depth-1 spawn L3.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| L2 task description | `xai-grok-agent` `builder.rs` `CHILD_TASK_DESCRIPTION` | `child_task_description_is_concise` | shipped and proven | **no** |
| Default max allows L2 → L3 | `xai-grok-tools` `task/mod.rs` | `default_max_allows_l2_to_spawn_l3` | shipped and proven | **no** |
| User-guide three-layer paragraph | `docs/user-guide/16-subagents.md` | no dedicated `fn` | shipped in code (guide text). No cargo pin. | no |
| D1 process pins | `AGENTS.md`, `FORK.md` | `./scripts/assert-process-pins.sh` (file sniff, not this sentence) | process, not a crate seam | assert, not catalog |

This is both process and product. A restack can keep AGENTS via `FORK_PATHS` and still drop `CHILD_TASK_DESCRIPTION`. That is why the cargo `fn`s matter.

---

## Extra class D. `from_config` empty-cache miss

`ModelsManager::from_config` with no prefetch argument is a zero-network boot. It must produce a usable bundled catalog and must not claim a real fetched catalog. An empty `models_cache.json` is a miss, not a fetch.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| No-prefetch boot usable, not "real" | `xai-grok-shell` `agent/models/tests.rs` | `from_config_without_prefetch_produces_usable_catalog` | shipped and proven | **no** |
| Empty disk file is a miss | `agent/models/cache.rs` `load_fresh` returns `None` when `models` is empty | no dedicated `fn` | shipped in code, no named test | no |

The named test is the land pin. The empty-file branch is only a debug return today. A restack can delete that `is_empty` check and stay green unless a new `fn` loads an empty file.

---

## Extra class E. `/rebuild` SHA-aware plus fail does not signal

`/rebuild` is local `just install`, not an xAI download. Verify package version plus git SHA. Same semver plus a different SHA is newer. Failed install must not replace the binary or SIGUSR1 peers.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Failed install does not replace or signal | `xai-grok-update` `rebuild.rs` | `failed_install_must_not_replace_or_signal_peers` | shipped and proven | yes (neighbor) |
| Build fail does not signal leaders | same | `build_fail_does_not_signal_leaders` | shipped and proven | **no** |
| Parse `grok-oss 0.1.100 (sha)` | same | `parse_version_output_extracts_identity` | shipped and proven | **no** |
| Same semver, different SHA → peer relaunch | same | `peer_relaunch_accepts_same_semver_different_sha` | shipped and proven | **no** |
| Equal identity + same path does not loop | same | `peer_relaunch_declines_equal_identity_on_same_path` | shipped and proven | **no** |
| Deleted inode still relaunches | same | `peer_relaunch_accepts_deleted_inode_even_when_identity_equal` | shipped and proven | **no** |
| Leader older-than same semver SHA | `xai-grok-shell` `leader/mod.rs` | `leader_is_older_than_same_semver_git_sha_identity` | shipped and proven | **no** |

FORK claims SHA-aware identity. Catalog only keeps the fail-does-not-signal `fn`. A restack can drop SHA compare and keep the fail-plan test green.

---

## Extra class F. rustc 1.97.1 wins

Project pin is `rust-toolchain.toml` channel `1.97.1` plus matching fenix FOD in `flake.nix`. After an upstream export that still lists 1.94.x, keep Surmount **1.97.1** unless the operator chooses another channel.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Toolchain channel | `/home/hunter/Projects/surmount/grok-build/rust-toolchain.toml` | none | shipped in code, no named test | no |
| Fenix matches file | `flake.nix` `fromToolchainFile` | none | shipped in code, no named test | no |
| FORK claim | `FORK.md` Rust 1.97.1 bullet | none | docs | n/a |
| `FORK_PATHS` | `scripts/import-upstream-export.sh` includes `flake.nix`, **not** `rust-toolchain.toml` | assert does not sniff `1.97.1` | process hole | no |

`flake.nix` is path-restored. `rust-toolchain.toml` is **not** in `FORK_PATHS`. Import can keep the flake and take upstream's toolchain file, or the reverse if someone adds only one path. There is no cargo `fn` that asserts channel `1.97.1`. This class is restack-droppable and **unproven by catalog**.

---

## Extra class G. Nucleo reuse-per-root

Opening many workspace fuzzy searches without `close` must keep **one live matcher per root**, not a new `Nucleo::new(..., Some(2), 1)` per `open`. Poll-only `get_results` must not refresh the stale timer. Per-matcher pool size is 2 (`NUM_NUCLEO_THREADS`).

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| 20 opens → 1 search | `xai-grok-workspace` `file_system/mod.rs` | `repeated_open_without_close_keeps_one_search_per_root` | shipped and proven | **no** |
| Distinct roots stay 1 each | same | `distinct_roots_each_keep_one_search` | shipped and proven | **no** |
| Poll does not keep stale alive | same | `get_results_does_not_keep_a_stale_search_alive` | shipped and proven | **no** |
| Nucleo pool is `Some(2)` | `xai-fuzzy-file-search` `lib.rs` `NUM_NUCLEO_THREADS = 2` | no `fn` that asserts `Some(2)` | shipped in code, no named test | no |

---

## Extra class H. User-guide fork-specific pins

The shared guide under `crates/codegen/xai-grok-pager/docs/user-guide/` is **not** in `FORK_PATHS`. Onto takes the xAI guide unless conflict resolve keeps Surmount pages. Current tree **does** contain `/limits`, spend order, last-session, `grok-oss`, and skills-not-Python. Cargo pins exist for some sentences, not all.

| Seam | Code path | Test filter | Status | Catalog? |
|------|-----------|-------------|--------|----------|
| Skills not a Python runtime | `docs.rs` | `user_guide_skills_are_not_a_python_runtime` | shipped and proven | yes (class 7) |
| Resume / version examples | same | `user_guide_resume_and_version_examples_use_grok_oss` | shipped and proven | yes (class 1) |
| Operator CLI leftovers | same | `user_guide_operator_cli_examples_use_grok_oss` | shipped and proven | **no** |
| Hop after included full is shipped | same | `user_guide_does_not_claim_automatic_host_hop_is_unshipped` | shipped and proven | **no** |
| Spend order + one fetcher + no "free SuperGrok" | same | `user_guide_names_token_economy_spend_order` | shipped and proven | **no** |
| Guide has `/limits` at all | `02-authentication.md`, `04-slash-commands.md` | no `fn` that counts `/limits` hits | shipped in code (text exists). Catalog prose says zero hits = failed land. No cargo pin. | no |
| Last-session sentences | `01-getting-started.md`, `17-sessions.md` | none | shipped in code, no named test | no |
| Three-layer guide | `16-subagents.md` | none | shipped in code, no named test | no |

---

## Neighbor catalog names that are **missing `fn`s** (catalog lie)

FORK says missing `fn` = land failed. These identifiers are still in the catalog or FORK cheat sheet and have **no** matching `fn` in this tree:

| Catalog / FORK identifier | What this walk found |
|---------------------------|----------------------|
| `retry_chrome_soft_reconnects_when_retry_stream_starts` | **no `fn`**. Pager maps `RetryState::StreamResumed` in `session_notification.rs`. |
| `stream_resumed_without_prior_retry_clears_activity` | **no `fn`** |
| `clip_retry_reason_*` / `retrying_activity_label_*` / `retrying_label_shows_timeout_*` | **no `fn`** |
| `shell_collision_contract_covers_every_pager_command_and_alias` | **no `fn`**. Slash `/clear-completed-todos` exists. Pager `SHELL_RESERVED` identifier is gone. |
| `default_title_items_include_agents` | **no `fn`** |
| `title_escape_never_empty_payload` | **no `fn`** |
| `title_updates_gated_only_by_title_enabled` | **no `fn`** |

Still present neighbors (do not delete): `stream_started_emits_retry_state_stream_resumed`, `wait_before_attempt_aborts_on_cancel`, `retry_footer_reason_uses_short_transport_label`, `retry_footer_backoff_hint_appends_next_try_in`, `stream_headers_timeout_defaults_to_120_secs_when_env_unset`, `cargo test -p xai-grok-sampler --test stream_headers_timeout`.

Do **not** claim stuck-retry chrome is fully proven. Shell emit exists. Pager chrome tests named in the catalog do not.

---

## Cargo filters a land must keep

Copy is the catalog cheat sheet in `doc/dev/upstream-regression-filters.md` (seven classes) **plus** the proven extras below. `rg` each identifier for a matching `fn` first.

```bash
# Class 1 CLI
cargo test -p xai-grok-pager --lib -- product_version_line_uses_grok_oss_not_bare_grok \
  resume_session_command_uses_grok_oss user_guide_resume_and_version_examples_use_grok_oss \
  product_cli_name_is_grok_oss print_exit_resume_hint_writes_expected_lines \
  user_guide_operator_cli_examples_use_grok_oss welcome_badge_brands_grok_oss \
  hero_subtitle_brands_grok_oss tutorial_list_title_brands_grok_oss
cargo test -p xai-grok-pager-bin --test version_without_tty

# Class 2 settings + readers
cargo test -p xai-grok-pager --test settings_e2e -- hide_header always_expand_thinking \
  scrub_ascii_punct allow_worktree bubble_copy_buttons plan_approval_park
cargo test -p xai-grok-pager --lib -- theme_choices_include_doge_and_default_is_doge \
  hide_header_zeroes always_expand_thinking_keeps_blocks_expanded bubble_copy_buttons_on \
  append_bubble_copy_button_paints clicking_human_bubble_copy clicking_assistant_bubble_copy \
  clicking_wide_human_bubble_copy
cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub_ascii_punct_from_ui
cargo test -p xai-grok-shell --lib -- resolve_subagents_copies_allow_worktree

# Class 3 /spend
cargo test -p xai-grok-shell --lib -- spend_path_ingests_usage_jsonl_and_records_reconciliation
cargo test -p xai-grok-pager --lib -- show_spend_ingests_usage_jsonl_and_is_not_empty_default

# Class 4 chrome
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

# Class 5 hop + flock + Business first
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use sampling_config_hops_to_sibling_included_before_extras \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
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

# Class 6 last-session
cargo test -p xai-grok-pager --lib -- materialize_new_auto_opens_last_session_when_one_exists

# Class 7 skills
cargo test -p xai-grok-bundle --lib -- sanitize_rejects_non_excepted_skill_python \
  extract_archive_skips_non_excepted_skill_python \
  product_repo_skill_roots_have_no_non_excepted_python
cargo test -p xai-grok-pager --lib -- user_guide_skills_are_not_a_python_runtime
cargo test -p xai-grok-tools --lib -- implement_memory_snapshot_intercept_does_not_spawn_shell \
  plan_validate_intercept_does_not_spawn_shell session_reader_list_intercept_does_not_spawn_shell
```

---

## Must enroll in catalog

Proven named tests **not** in `doc/dev/upstream-regression-filters.md` (or only mentioned in residual prose, not the seven-class cheat sheet):

1. `user_guide_operator_cli_examples_use_grok_oss`
2. `user_guide_does_not_claim_automatic_host_hop_is_unshipped`
3. `user_guide_names_token_economy_spend_order`
4. `welcome_badge_brands_grok_oss`, `hero_subtitle_brands_grok_oss`, `tutorial_list_title_brands_grok_oss`
5. `materialize_new_auto_stays_welcome_when_no_last_session`, `materialize_new_auto_does_not_open_last_when_headless`, `from_pager_args_opens_last_session_on_start`
6. `exit_plan_mode_present_is_not_operator_approve`, `exit_plan_mode_tool_result_does_not_claim_operator_approval`, `exit_plan_mode_keeps_mid_compose_draft_and_a_types`, `exit_plan_mode_modal_park_does_not_steal_mid_compose_keys`, `exit_plan_mode_empty_present_printable_goes_to_composer`
7. `child_task_description_is_concise`, `default_max_allows_l2_to_spawn_l3`
8. `from_config_without_prefetch_produces_usable_catalog`
9. `peer_relaunch_accepts_same_semver_different_sha`, `peer_relaunch_declines_equal_identity_on_same_path`, `peer_relaunch_accepts_deleted_inode_even_when_identity_equal`, `leader_is_older_than_same_semver_git_sha_identity`, `parse_version_output_extracts_identity`
10. `repeated_open_without_close_keeps_one_search_per_root`, `distinct_roots_each_keep_one_search`, `get_results_does_not_keep_a_stale_search_alive`
11. Pause / Clear finished paint: `work_control_chrome_matrix_pause_not_cancel_stop_not_pause`, `pause_button_click_dispatches_global_pause_not_cancel`, `clear_finished_action_idle_is_quiet_not_neon_green_or_magenta`, `clear_finished_click_does_not_open_subagent`
12. Catalog §5 table names that FORK cheat sheet omits: `align_after_billing_switches_sticky_personal_full_to_business_included`, `prepare_sampler_for_turn_aligns_to_ranked_included_primary`, `resolve_model_to_sampling_config_auto_use`

Also **remove or restore** catalog names that have no `fn` (retry chrome pager tests, `shell_collision`, `default_title_items_include_agents`). A named filter with no `fn` is a failed land, not a keep list.

---

## Must list in FORK

FORK land checklist already names the seven classes, then muddles them (item 5 is `FORK_PATHS`, item 6 restates the list). Thin or missing as **land classes** even when a long product bullet exists:

1. **Bubble copy click** (not paint-only). FORK chrome list does not say click-to-copy.
2. **Plan present ≠ Approve + modal-free typing.** Five-CTA paint is listed. Honesty + typing are not a land class.
3. **Business / Team included before personal; sibling included before SuperGrok dollar credits; one-process limits flock.** Buried under dual-auth. Restack-droppable as its own class (already has tests).
4. **Always-three-layer product prompt** (`CHILD_TASK_DESCRIPTION`). FORK process checkbox is not a cargo land.
5. **`from_config` empty-cache miss.** Not in FORK land list.
6. **`/rebuild` SHA-aware.** FORK product bullet exists. Land cheat sheet only has fail-does-not-signal.
7. **Nucleo reuse-per-root.** Residual shipped bullet. Not a land class.
8. **User-guide cargo pins** beyond skills + resume. FORK says a guide with zero `/limits` is a failed land, but there is no `fn` and no cheat-sheet filter for `/limits` hits.
9. **Welcome / tutorial Grok OSS chrome.** CLI identity is listed. Badge chrome is not.
10. **Pause / resume / Clear finished** as chrome land, or explicitly neighbor. FORK product text names them. Seven-class list does not.
11. **`rust-toolchain.toml` in `FORK_PATHS` or a sniff.** Flake alone is not enough.

---

## Do not claim

- Live TUI / dogfood. This walk did not open a rebuilt `grok-oss`.
- Fresh cargo green. Tests were **found**, not run.
- Stuck-retry **pager** chrome is catalog-proven. The named pager `fn`s are gone.
- `shell_collision` / pager `SHELL_RESERVED` still exists. The slash command exists. The collision contract test does not.
- Token Economy `/settings` table rows, economic-mode settings row, F9 screenshot bind, or every FORK checkbox from 2026-08-13 config-gaps. Those were not re-proven this turn.
- rustc 1.97.1 is cargo-proven. It is a file pin only.
- Empty `models_cache.json` miss is cargo-proven. Only the no-prefetch boot test is.
- Nucleo `Some(2)` is cargo-proven. Reuse-per-root is. Pool size is a constant.
- Host overlay skills (`~/.agents/skills`) are a product land class. They are not.
- 2026-08-13 postmortem rows that said compact meter, hop wire, hide_header paint, bubble copy, last-session, or five-CTA were dropped. Those restores have named `fn`s in **this** tree.

---

## Suggested extra land classes (proven and restack-droppable)

Only these, because this walk found both a product path and a named test (except rustc, called out as unproven):

1. **Always-on bubble copy click + wrap** (already half-enrolled; treat paint-only as failed land).
2. **Plan present ≠ approve + modal-free typing.**
3. **Business / Team included first + sibling included before SuperGrok dollar credits + one-process limits flock** (split from generic dual-auth if the seven stay crowded).
4. **User-guide fork pins** (hop shipped, spend order, `grok-oss` CLI leftovers, `/limits` presence).
5. **`from_config` cold catalog** (`from_config_without_prefetch_produces_usable_catalog`).
6. **`/rebuild` SHA-aware peer relaunch** (not only fail-does-not-signal).
7. **Nucleo reuse-per-root.**
8. **Always-three-layer product description** (`child_task_description_is_concise`).
9. **Pause / Clear finished chrome** (if land must keep Work B / quiet minus).

**Do not add rustc 1.97.1 as a cargo land class until a named test or assert sniff exists.** Path-restore `rust-toolchain.toml` plus a channel sniff is the honest gate.

---

## Prior reports (context only)

| File | Use |
|------|-----|
| `.agents/reports/fork-loss-postmortem-2026-08-13.md` | Why 1.0.3 stayed green. Stale on later restores. |
| `.agents/reports/fork-gaps-remaining-seams-2026-08-13.md` | Broad FORK claim walk. Many rows stale. |
| `.agents/reports/fork-gaps-config-options-2026-08-13.md` | Unread keys as of that day. The six settings rows are restored now. |
| `.agents/reports/impl-fork-recon-land-pins.md` | How FORK + catalog paint filters were pinned. Compact-meter tests were still owed then; they exist now. |
| `doc/dev/upstream-regression-filters.md` | Durable catalog. Contains both current `fn`s and deleted names. |
| `FORK.md` land checklist | Seven classes. Item 5 is process (`FORK_PATHS`), not a product class. |

End of inventory.
