# Upstream merge rules so restack cannot lose Surmount seams silently

**Date:** 2026-08-14
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Board:** `bug:upstream-merge-rules-prevent-loss`

Process law only. No product Rust. No git add/commit. SuperGrok is paid. This
report says **included SuperGrok period limits**.

## What rules already existed

FORK already split `FORK_PATHS` restore (docs/scripts) from crate seams
(cherry-pick + cargo). Assert was already named as files-only. After the
1.0.3 postmortem, FORK, the filter catalog, git-recon land step 9, and the
HITL review checklist already required:

- `assert-process-pins` then catalog then `rg` that named `fn`s still exist
- paint filters for five-CTA, `hide_header` zeros, `sampling_config` hop,
  AutoCompact todo board, failed `/rebuild` install
- dogfood screenshots (rails, five CTAs, compact meter, SIGUSR1 after fail)
- `just check` cannot fail a deleted catalog test
- helper-only green is a lie

That still treated **chrome/paint** as the land surface. Status compact meter
and `ShowLimits` click were cataloged as **Owed** (no paint `fn` yet). CLI
identity was sparse (`product_cli_name` + OpenRouter referer).
`version_without_tty` was listed without the first-token contract.
`/settings` unread rows, `/spend` ingest, titled composer frame, and
last-session were not required on land. A chrome-only post-restack inventory
could still be reported as complete.

## What I added

Standing law is one checklist, extended, not a second novel.

| Path | What changed |
|------|----------------|
| `FORK.md` | New § *Land checklist*. Six inventory classes in complete sentences. Helper-green ban. Assert is files only. Chrome-only is a failed land. Cheat sheet now runs the six-class cargo block. |
| `AGENTS.md` § *Survive recon* | Compressed pointer: named cargo tests, six classes, chrome-only fails, `just check` cannot fail a deleted test. No recon diary. |
| `doc/dev/upstream-regression-filters.md` | New § *Required land inventory (six classes)* with confirmed `fn` names. Identity is first-token, not sparse. Compact meter + `ShowLimits` + titled composer moved from **Owed** to **Keep**. Cheat sheet matches FORK. |
| `docs/upstream-history.md` review checklist | D2 land step: six classes + helper-green ban. Screenshots stay an operator check, not the only check. |
| `~/.agents/skills/git-recon/SKILL.md` | Land step 9, `recon:land`, anti-pattern: six classes + helper-green + chrome-only fail. |
| `~/.agents/skills/git-recon/references/hand-commands.md` | Same land template. |
| `~/.agents/skills/upstream-export-import/SKILL.md` | Import seam table + verify + join land line. |
| `~/.agents/skills/upstream-export-import/references/import-seams.md` | Seam rows + land verify. |

## Six inventory classes (now required on land)

1. **CLI identity.** Product command is `grok-oss`. `--version` first token is `grok-oss`. Resume hints are `grok-oss --resume`.
2. **Config is a surface.** A deserializing toml field is not shipped without a `/settings` row and a runtime reader.
3. **grok-oss SQL extras.** `$GROK_HOME/grok_oss.db` is the Token Economy ledger. Schema v1 is not enough. `/spend` must ingest `usage.jsonl` and write `reconciliation_run`.
4. **DOGE / Surmount chrome.** Theme file existing is not paint. Rails, box caret, magenta model / running agent, compact included SuperGrok period limits meter, titled composer white frame / yellow title only, five-CTA plan panel.
5. **`FORK_PATHS` restore is docs and scripts only.** Assert proves files exist. It does not prove contracts.
6. **Inventory must not be chrome-only.** Required surfaces: chrome/paint, `/settings` plus unread config, grok-oss ledger `/spend`, CLI branding, dual-auth hop after included SuperGrok period limits are full, last-session on start.

## Named tests now required on land

Confirmed in-tree with `rg` (`fn` exists). Do not invent missing names.

**CLI identity**

- `product_version_line_uses_grok_oss_not_bare_grok`
- `resume_session_command_uses_grok_oss`
- `user_guide_resume_and_version_examples_use_grok_oss`
- `product_cli_name_is_grok_oss`
- `print_exit_resume_hint_writes_expected_lines`
- `xai-grok-pager-bin --test version_without_tty` (`assert_version_ok` first-token contract)

**Config as a surface**

- settings_e2e: `hide_header_*`, `always_expand_thinking_*`, `scrub_ascii_punct_*`, `allow_worktree_*`, `bubble_copy_buttons_*`, `plan_approval_park_*`
- `theme_choices_include_doge_and_default_is_doge`
- `hide_header_zeroes_status_bar_height`, `hide_header_zeros_welcome_top_bar_height`, `hide_header_zeroes_header_and_header_gap`
- `always_expand_thinking_keeps_blocks_expanded`
- `bubble_copy_buttons_on_paints_copy_icon`
- `prime_applies_scrub_ascii_punct_from_ui`
- `resolve_subagents_copies_allow_worktree`

**grok-oss ledger `/spend`**

- `spend_path_ingests_usage_jsonl_and_records_reconciliation`
- `show_spend_ingests_usage_jsonl_and_is_not_empty_default`

**DOGE / chrome paint**

- `user_prompt_block_accent_*`, `user_prompt_entry_renderer_paints_green_rail`
- `paint_composer_box_cursor_uses_human_green_not_agent_magenta`
- `focused_composer_paints_human_green_box_caret_hides_terminal_cursor`
- `agent_message_block_accent_is_magenta_rail_under_doge_while_running`
- `info_line_model_name_uses_accent_model_not_gray`
- `status_bar_pushes_credits_compact_included_supergrok_period_limits`
- `hit_credits_click_dispatches_show_limits`
- `titled_doge_composer_frame_is_prompt_border_not_context_yellow`
- `plan_approval_footer_paints_five_cta_vocabulary`
- `default_theme_is_doge`
- `auto_compact_completed_preserves_todo_board`

**Dual-auth hop** (class 6 surface)

- `sampling_config_auto_use_fills_console_hop_after_included_full`
- `sampling_config_auto_use_omits_console` / `sampling_config_auto_use_omits_console_while_supergrok_included_headroom`
- `sampling_config_auto_use_extras_keep_session_console_failover`
- `resolve_model_to_sampling_config_auto_use`

**Last-session on start** (class 6 surface)

- `materialize_new_auto_opens_last_session_when_one_exists`

**Neighbor still required** (already on the old cheat sheet)

- `failed_install_must_not_replace_or_signal_peers`
- window titles / stuck-retry / `shell_collision`

## Leftover honesty (what still cannot be mechanically proven)

- **Lower-left throbber magenta.** Old cheat sheet names `doge_idle_subagent_still_running` and `doge_tool_running_spinner` are **not** in the tree. Closest existing: `agent_message_block_accent_is_magenta_rail_under_doge_while_running`, `doge_activity_spinners_use_striped_down_marquee_not_braille`, `idle_with_subagents_renders_still_running_cue`. Cataloged those. Did not invent the missing paint `fn`.
- **Screenshots / live TUI.** Still an operator check. An old installed `grok-oss` can look wrong while source tests are green.
- **User-guide `/limits`.** Still a guide `rg`, not a cargo test. Zero hits remains a failed land by prose.
- **`just check` still cannot fail a deleted catalog test.** Documented. No mechanical name-existence gate in CI.
- **Assert still cannot prove contracts.** Documented. No change to `assert-process-pins.sh`.
- **Not every leftover `/settings` knob has a land filter.** Economic mode and auto-run implement have settings_e2e rows; eight `[token_economy]` knobs are not each named on the land list.
- **Host skills live outside product git.** Dual-pinned here. They do not ride import `FORK_PATHS`.
- **Older FORK product bullets** still say "free SuperGrok period" in places. Language residual. Not bulk-rewritten in this slice.
- Did not run `just check` or the catalog cargo blocks. This slice is docs/skills only.

Stopped.
