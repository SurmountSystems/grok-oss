# Recon / land defense tools (existing seams)

This maps tools that already exist. Do not invent a second inventory board.
Catalog authority: [`doc/dev/upstream-regression-filters.md`](../../doc/dev/upstream-regression-filters.md).
Path-restore authority: `FORK_PATHS` in `scripts/import-upstream-export.sh` plus
`scripts/assert-process-pins.sh`.

I read those files, `FORK.md` § Land checklist, `AGENTS.md` § Survive recon,
`docs/upstream-history.md` review/HITL bits, `justfile` recipes, import and
assert scripts, `scripts/recon-status.sh`, `RESIDUAL.md` § Validate honesty,
`docs/upstream-onto-log.md` 1.0.3 note, host `~/.agents/skills/git-recon/SKILL.md`,
and host `~/.agents/skills/upstream-export-import/SKILL.md`. I did **not**
`rg` every catalog identifier against `fn` names in crates. I did **not** run
`just check` or the assert.

---

## What already defends seams

Two layers, already named in the catalog and FORK.

**Layer A. Process docs / scripts / packaging (files only).** Import
`read-tree`s the xAI tree, then restores `FORK_PATHS` from `BASE_REF`. Import
then runs `./scripts/assert-process-pins.sh` and fails closed if pins are
missing. `just upstream-assert-process-pins` is the same script. Join
(`merge -s ours`) keeps the onto tip tree. It cannot backfill a missing pin.

`FORK_PATHS` today (import script): `FORK.md`, `CONTRIBUTING.md`,
`SECURITY.md`, `README.md`, `justfile`, `flake.nix`, `flake.lock`,
`packaging`, `AGENTS.md`, `RESIDUAL.md`, `docs/upstream-history.md`,
`docs/upstream-import-log.md`, `docs/upstream-onto-log.md`,
`docs/git-workflow.md`, `docs/dev`, `doc/dev`, detect/import/sync/
put-history/replay/join/hermetic/assert/`recon-status` scripts,
`.github/workflows/upstream-export.yml`, `.github/workflows/ci.yml`,
`.grok/workflows`, `crates/codegen/grok-rate-limit`.

Assert `REQUIRED_FILES` / `REQUIRED_DIRS` mostly match that list. Extra on
assert vs a naive "docs only" read: `crates/codegen/grok-rate-limit` as a
required dir. Product user-guide is **not** in `FORK_PATHS`.

Worktree-only light sniffs (skipped when a tree-ish is passed): AGENTS
contains `parent is coordinator` (warn); FORK mentions upstream/import/onto
(warn); README mentions Grok OSS / grok-oss (warn); FORK must say
`non-excepted Python` (fail); `08-skills.md` must say `not a Python runtime`
if that file exists (fail); non-excepted `.py` under `.agents/skills` or
`.grok/skills` (fail). Allowed stubs: `memory.py`, `validate-plan.py`,
`session_reader.py`, plus office `docx`/`pptx`/`xlsx`/`pdf` trees.

**Layer B. Product seams inside `xai-grok-*` (named cargo tests).** These
are not path-restored. They survive onto only by cherry-pick plus the catalog.
FORK, AGENTS, and `docs/upstream-history.md` already say: assert proves files
exist; it does not prove contracts. A chrome-only pass is a failed land.
`just check` cannot fail a deleted catalog test.

**Dual-pin already used.** Host skills (`git-recon`, `upstream-export-import`)
plus branch `AGENTS.md` / `FORK.md` / `docs/upstream-*` / catalog. Host overlay
is outside product git. Process corrections are supposed to land on both.

**What land agents are already told.** `git-recon` `recon:land`: run assert,
then walk FORK / catalog inventory, `rg` each required identifier, treat a
missing `fn` as failed land, ban helper-green, ban chrome-only, then
`just check`. Dogfood screenshots are an operator check after those `fn`s
exist. Chrome-only land is already forbidden in the skill, the catalog, FORK,
AGENTS, and the import review checklist.

---

## What still cannot fail a dropped crate contract

1. **`assert-process-pins.sh` never runs cargo.** Deleting
   `status_bar_pushes_credits_compact_included_supergrok_period_limits` (or
   any other catalog `fn`) still exits 0 if files exist.
2. **`just check` / `just ci`** is `flake-meta` + `ci-prep` + `just test`
   (fmt, clippy lib+bins, workspace nextest, doctests, mem-guard). Nextest
   only runs tests that remain. A deleted catalog test is silent. Same gap
   is written in the catalog, FORK, AGENTS, and the 1.0.3 onto-log note
   (catalog reds were deleted so the gate stayed green).
3. **Tree-ish assert skips all content sniffs.** `./scripts/assert-process-pins.sh HEAD`
   does not fail a hollow FORK, a missing Python-runtime sentence, or junk
   `.py` in skill roots. Those checks are worktree-only.
4. **User-guide is not `FORK_PATHS`.** Shared
   `crates/codegen/xai-grok-pager/docs/user-guide/` takes the xAI base on
   import. A guide with zero `/limits` hits is a failed land only if a human
   or a cargo doc test (`user_guide_*`) still exists and is run.
5. **`recon-status.sh` land next-action is weaker than FORK.** Clean onto
   tip: "Land: assert HEAD && just check". It does not name the seven-class
   catalog or the `rg` name check.
6. **Host skills are one class behind the catalog.** `git-recon` and
   `upstream-export-import` still say **six** inventory classes and omit
   class 7 (product skills are not a Python runtime) from the numbered land
   loop. Branch docs say seven. Skill text still forbids chrome-only.
7. **FORK's own numbered "seven" mixes process notes with product classes.**
   Catalog classes 1–7 are CLI, config surface, `/spend`, chrome, dual-auth
   hop, last-session, skills-not-Python. FORK items 5–6 are "FORK_PATHS is
   docs only" and "must not be chrome-only", while dual-auth and last-session
   sit inside item 6 prose. Agents can miscount. AGENTS § Survive recon lists
   the seven **product** classes correctly.
8. **Name-existence is prose, not a gate.** Catalog: "`rg` each identifier
   … Missing `fn` = land failed." Nothing mechanically fails if the mop
   deletes the test and leaves the catalog line, or deletes both.

---

## Is assert-process-pins the right seam for "FORK names still in catalog"?

**No, not as a pile-on to the path gate.** Assert is the file-presence fail-closed
for import restore. Stuffing FORK↔catalog string equality into it would:

- Mix two jobs (paths vs cargo-contract names).
- Break on every honest catalog rename unless FORK and assert are edited
  together (brittle, and the tree already has six-vs-seven drift).
- Still not prove a `fn` exists in crates.

Two different checks, if anything is added:

| Check | What it proves | Where it belongs |
|-------|----------------|------------------|
| FORK cheat-sheet identifiers still appear in the catalog | Docs did not diverge | Optional light sniff or a sibling script. Keep it small (cheat-sheet block only), not every residual-aligned substring. |
| Catalog required-land identifiers still have a matching `fn` in crates | Deleted test cannot stay green | Sibling land script (or `STRICT=1` / `LAND=1` next to assert), then a `just` recipe. Do **not** grow `REQUIRED_FILES`. |

Prefer extending the **existing** catalog + assert pair: one new script that
parses the Required land inventory / operator cheat sheet identifiers and
`rg`s `fn <name>`. Call it from `recon:land` and from the import review
checklist. Leave `assert-process-pins` as the path gate.

---

## Concrete improvements on this system (not a second board)

1. Dual-pin host `git-recon` + `upstream-export-import` from **six** to
   **seven** classes (add skills-not-Python). Keep chrome-only forbidden.
2. Align FORK numbered list with the catalog's seven **product** classes.
   Move "FORK_PATHS is files only" and "not chrome-only" out of the 1–7 count.
3. Teach `recon-status.sh` land next-action the catalog name check, not only
   assert + `just check`.
4. Add a sibling `scripts/assert-land-catalog-fns.sh` (name TBD) that fails
   if a Required-land identifier has no `fn`. Wire
   `just upstream-assert-land-catalog` next to `upstream-assert-process-pins`.
   Import can keep calling **path** assert only; land calls both.
5. Optionally run the same worktree sniffs when `TREE_ISH` is set
   (`git show $TREE_ISH:FORK.md` / skill roots), so `assert HEAD` cannot
   miss a hollow pin tree.
6. Do not add catalog identifiers into `just test` as a magic filter list.
   Nextest still cannot fail a deleted test. Name-existence is the missing
   fail-closed.

---

## Named cargo filters already in the catalog (grouped)

### Required land inventory (seven classes)

**1. CLI identity** (`xai-grok-pager` unless noted):
`client_identity::product_version_line_uses_grok_oss_not_bare_grok`,
`client_identity::resume_session_command_uses_grok_oss`,
`docs::user_guide_resume_and_version_examples_use_grok_oss`,
`client_identity::product_cli_name_is_grok_oss`,
`print_exit_resume_hint_writes_expected_lines`,
`xai-grok-pager-bin --test version_without_tty`.

**2. Config is a surface:** settings_e2e `hide_header_*` /
`always_expand_thinking_*` / `scrub_ascii_punct_*` / `allow_worktree_*` /
`bubble_copy_buttons_*` / `plan_approval_park_*`;
`settings::registry::theme_choices_include_doge_and_default_is_doge`;
`hide_header_zeroes_*` / `hide_header_zeros_*`;
`always_expand_thinking_keeps_blocks_expanded`;
`bubble_copy_buttons_on_paints_copy_icon`,
`bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width`,
`append_bubble_copy_button_paints_when_first_line_fills_content_width`,
`clicking_human_bubble_copy_copies_the_prompt`,
`clicking_assistant_bubble_copy_copies_the_message`,
`clicking_wide_human_bubble_copy_still_paints_and_copies`;
`xai-grok-pager-render` `prime_applies_scrub_ascii_punct_from_ui`;
`xai-grok-shell` `resolve_subagents_copies_allow_worktree`.

**3. grok-oss ledger `/spend`:**
`xai-grok-shell` `token_economy::spend_path_ingests_usage_jsonl_and_records_reconciliation`;
`xai-grok-pager` `show_spend_ingests_usage_jsonl_and_is_not_empty_default`.
(`open_creates_schema_and_version` is explicitly **not** this class.)

**4. DOGE / chrome paint:**
`user_prompt_block_accent_*`, `user_prompt_entry_renderer_paints_green_rail`,
`paint_composer_box_cursor_uses_human_green_not_agent_magenta`,
`focused_composer_paints_human_green_box_caret_*`,
`agent_message_block_accent_is_magenta_rail_under_doge_while_running`,
`info_line_model_name_uses_accent_model_not_gray`,
`status_bar_pushes_credits_compact_included_supergrok_period_limits`,
`hit_credits_click_dispatches_show_limits`,
`titled_doge_composer_frame_is_prompt_border_not_context_yellow`,
`plan_approval_footer_paints_five_cta_vocabulary`,
`default_theme_is_doge`. Neighbors (not the missing lower-left throbber `fn`):
`doge_activity_spinners_use_striped_down_marquee_not_braille`,
`idle_with_subagents_renders_still_running_cue`. Render also lists
`resolve_from_config_no_config`, `doge_accent_user_is_pure_green`.

**5. Dual-auth hop after included SuperGrok period limits are full**
(rank-only `supergrok_identity_rank.rs` is **not** this class):
`sampling_config_auto_use_fills_console_hop_after_included_full`,
`sampling_config_auto_use_omits_console` /
`sampling_config_auto_use_omits_console_while_supergrok_included_headroom`,
`resolve_model_to_sampling_config_auto_use`,
`sampling_config_auto_use_extras_keep_session_console_failover`,
`sampling_config_hops_to_sibling_included_before_extras`,
`afterburner_does_not_skip_mark_when_sibling_has_included_remaining`,
`align_after_billing_switches_sticky_personal_full_to_business_included`,
`prepare_sampler_for_turn_aligns_to_ranked_included_primary`,
`pick_prefers_business_included_before_personal_when_both_have_remaining`,
`order_credentials_business_included_before_personal_when_both_have_room`,
`limits_snapshot_second_process_reads_file_and_does_not_http`,
`limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once`,
`limits_snapshot_never_writes_access_tokens`,
`billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http`.

**5b. Combined remaining included SuperGrok period limits:**
`combined_included_remaining_sums_distinct_personal_and_business_pools`,
`combined_included_remaining_does_not_double_count_unified_pool`,
`compact_meter_stays_included_while_sibling_pool_has_remaining`,
`active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`.

**6. Last-session on start:**
`materialize_new_auto_opens_last_session_when_one_exists`.

**7. Product skills are not a Python runtime:**
`xai-grok-bundle` `sanitize_rejects_non_excepted_skill_python`,
`extract_archive_skips_non_excepted_skill_python`,
`product_repo_skill_roots_have_no_non_excepted_python`;
`xai-grok-pager` `docs::user_guide_skills_are_not_a_python_runtime`;
`xai-grok-tools` `implement_memory_snapshot_intercept_does_not_spawn_shell`,
`plan_validate_intercept_does_not_spawn_shell`,
`session_reader_list_intercept_does_not_spawn_shell`.

### Product filter catalog (neighbors, still named)

- **shell_collision / SHELL_RESERVED:**
  `slash::commands::shell_collision_contract_covers_every_pager_command_and_alias`
  (filter `shell_collision`).
- **Stuck retry / StreamResumed / headers timeout / transport footer:**
  `retry_chrome_soft_reconnects_when_retry_stream_starts`,
  `stream_resumed_without_prior_retry_clears_activity`,
  `clip_retry_reason_does_not_strand_bare_error_word`,
  `clip_retry_reason_keeps_short_human_label`,
  `retrying_activity_label_uses_clipped_reason`,
  `retrying_label_shows_timeout_backoff_and_reconnecting`,
  `stream_started_emits_retry_state_stream_resumed`,
  `wait_before_attempt_aborts_on_cancel`,
  `retry_footer_reason_uses_short_transport_label`,
  `retry_footer_backoff_hint_appends_next_try_in`,
  `stream_headers_timeout_defaults_to_120_secs_when_env_unset`,
  integration `stream_headers_timeout::streaming_execute_times_out_waiting_for_headers`.
- **hide_header vs window titles:**
  `hide_header_defaults_false_and_parses`,
  `stale_hide_title_bar_key_is_ignored`,
  `window_title_always_manages_non_empty_branded_osc`,
  `window_title_osc_payload_never_empty_string`,
  `titles_on_session_name_osc_is_non_empty_branded`,
  `title_updates_gated_only_by_title_enabled`,
  `default_title_items_include_agents`,
  `title_escape_never_empty_payload`,
  `hide_header_zeroes_status_bar_height`,
  `hide_header_zeros_welcome_top_bar_height`,
  `hide_header_zeroes_header_and_header_gap`.
- **DOGE default theme:** `default_theme_is_doge`,
  `resolve_from_config_no_config_returns_doge`,
  `resolve_auto_dark_system_returns_doge`,
  `to_theme_kind_dark_defaults_to_doge`, plus `theme::doge::*` / `syntax::*doge*`.
- **Human green + role map:**
  `user_prompt_block_accent_is_static_human_rail`,
  `user_prompt_block_accent_is_green_rail_under_doge_default`,
  `user_prompt_prefix_matches_human_rail_color`,
  `recap_accent_and_bullet_use_neutral_tool_color_when_idle`,
  `doge_accent_user_is_pure_green_for_human`,
  `doge_accent_system_is_pure_cyan_for_system_limits_credits`,
  `doge_roles_green_cyan_no_blue_ui_no_gray_text`.
- **Clear completed todos:**
  `todo::clear_completed_archives_done_and_cancelled_leaves_open`,
  `clear_completed_todos_*`,
  `clear_completed_todos_x_key_only_when_todo_pane_focused`.
- **Paint keep-list extras:** `auto_compact_completed_preserves_todo_board`,
  `failed_install_must_not_replace_or_signal_peers`.
- **OpenRouter neighbor (not class 1):** `referer_is_surmount_*`,
  `title_is_grok_oss`.

### Residual-aligned blocks (Validate honesty mirror)

UDAX: `toon`, `json_to_toon`, `dynamic_to_prompt`, `free_text`,
`densify_mcp`, `densify_structured`, `task_output_handoff`,
`subagent_completed_handoff`. Dual-auth / multi SuperGrok / poll honesty:
`resolve_credentials`, `enforce_disable_api_key`, `store_and_load_round_trip`,
`fingerprint_is_not_raw_key`, `multi_add`, `rotate_`, `exhausted`, `memo`,
`fingerprint`, `hop_reason`, `live_rebind`, `login_`, `dual_auth_hop_reason`,
`credit_exhausted`, `upsert_personal_then_business`,
`team_login_then_personal_keeps`, `dual_supergrok`,
`load_supergrok_candidates`, `two_principals_billing`, `enrich_candidates`,
`principal_limits_label`, `non_active_poll_targets`,
`remember_both_principals`, `included_usage`, `poll_non_active_remembers`,
`format_dual_principals`, `live_console_omits`, `extra_principals_hook`,
`show_limits`, `format_supergrok_session`, `footer_names_live_principal`,
`limits_json_lists_two_supergrok_principals_when_both_slots_exist`,
`limits_json_honest_single_supergrok_session_cannot_see_team_plan`,
`auth_failed_poll`, `billing_fail_note`, `remember_poll_ok`,
`order_live_prefers_poll_ok`, `format_human_dual_poll`,
`sibling_poll_skips_after_n`, `session_needs_oidc_refresh`,
`ensure_fresh_refreshes_expired`, `find_and_persist_refreshed`,
`dual_fill_provenance`, `compact_status_active_auth_failed`,
`format_unified_fills`, `format_dual`, `limits_honesty`. Plan soft-park:
`plan`, `softer_park`, `toast`, `focus_plan`, `plan_approval`, `soft_park`.
Intercepts: `session_reader`, `plan_validate`, `bulk_edit_policy`,
`implement_memory`, `opencode`, `edit`. TUI screenshot: `tui_screenshot`,
`screenshot::`, `capture_tui_screenshot`, `try_attach_tui_screenshot`.
Shipped neighbors: `interject`, `handle_interject`, `force_interject`,
`cancel_turn`, `btw`, `enter_plan_mode`,
`enter_plan_mode_not_auto`, `enter_plan_mode_fast_path`, `usage_log`,
`record_response_token_usage`.

Helper-green bans already written: substring `grok` version, theme-file-exists,
schema-without-`/spend`, serde-only `hide_header`, rank-without-hop,
bundle-still-has-`memory.py`.

---

## Other files actually opened

- `/home/hunter/Projects/surmount/grok-build/FORK.md` (What recon keeps, Land
  checklist, cheat sheet, CI table)
- `/home/hunter/Projects/surmount/grok-build/AGENTS.md` § Survive recon
- `/home/hunter/Projects/surmount/grok-build/justfile` (`check`=`ci`;
  `upstream-assert-process-pins`; `recon-status`; no land-catalog recipe)
- `/home/hunter/Projects/surmount/grok-build/scripts/import-upstream-export.sh`
- `/home/hunter/Projects/surmount/grok-build/scripts/recon-status.sh`
- `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md` § Validate honesty
  (points at the catalog; item 11 is path assert only)
- `/home/hunter/Projects/surmount/grok-build/docs/upstream-onto-log.md`
  (1.0.3 seam-loss: helpers kept, catalog reds deleted)
- `/home/hunter/Projects/surmount/grok-build/doc/dev/research/fork-paths-hardening-2026-07-24.md`
  (historical; live list is the import script)
- `/home/hunter/.agents/skills/upstream-export-import/SKILL.md` (still six-class)

---

## Honesty

I did not verify that each catalog identifier still has a matching `fn` in
this tree. I did not execute assert or `just check`. I did not open
`import-seams.md` beyond grep hits. SuperGrok is paid. The compact meter is
included SuperGrok period limits, not SuperGrok dollar credits, and not
console team prepaid / console API credits.

---

## Process improver should...

- Keep one catalog (`doc/dev/upstream-regression-filters.md`) and one path
  assert. Do not start a second inventory file.
- Dual-pin host `git-recon` and `upstream-export-import` to **seven** classes
  (add skills-not-Python) so land agents stop walking a stale six-class loop.
- Renumber FORK § Land checklist to match the catalog's seven product classes.
- Add a sibling land script that fails when a Required-land catalog identifier
  has no `fn`. Wire a `just` recipe. Do not fold that into `REQUIRED_FILES`.
- Update `recon-status.sh` land next-action to name assert **and** the catalog
  name check, not only `just check`.
- Consider running worktree sniffs against `git show $TREE_ISH:…` so
  `assert HEAD` cannot miss a hollow FORK or skill-root `.py`.
- Leave `just check` as the quality gate. It will never fail a deleted test.
- When strengthening, edit these same seams. Do not invent a parallel board.
