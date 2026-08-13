# PR #36 first product fail, run 31673700687

Read-only log pull. No product edits. No GitHub write. Logs came from GitHub MCP `get_workflow_run` / `get_workflow_job` plus the signed Actions logs zip (MCP `get_job_logs` tail was still inside passing nextest). Zip extracted under `/tmp/pr36-ci-a036327e/` only.

## Run

| Item | Value |
|------|--------|
| Run | [31673700687](https://github.com/SurmountSystems/grok-oss/actions/runs/31673700687) (CI #55, `pull_request`) |
| Job | [94363580195](https://github.com/SurmountSystems/grok-oss/actions/runs/31673700687/job/94363580195) `just ci` |
| SHA | `a036327e6151398f7c46b79948256b24b2ae1832` |
| Branch | `onto-xai/b13fa526f511` |
| PR | [#36](https://github.com/SurmountSystems/grok-oss/pull/36) |
| Conclusion | **failure** |
| Started / ended | 06:24:35Z – 07:31:58Z (~67 min) |
| Failed step | `just ci-prep && just test` (step 6) |
| Recipe that died | `test-unit` → `cargo nextest run --workspace --locked` |
| Nextest summary | `29833 tests run: 29747 passed, 86 failed, 449 skipped` (1177.963s) then `error: test run failed` |

`cargo fmt --all -- --check` passed (06:28:10Z–06:28:44Z).  
`cargo clippy --workspace --lib --bins --locked -- -D warnings` passed (`Finished dev` 06:37:21Z, 8m 34s).  
`cargo test --no-run --workspace` passed (`Finished test` 07:12:04Z, 34m 38s). Nextest then started 29833 tests across 252 binaries.  
`test-doc` and `test-mem-guard` never ran (nextest failed first).

This is not the prior `82fa1794` test-compile pile (`ctor` + restack). Workspace `--no-run` was green on this SHA.

## Classification

**test-runtime**

Not fmt. Not clippy. Not test-compile. Not test-doc. Not mem-guard. Not infra.

## First real fail

- Crate: `xai-grok-agent` (`lib test`)
- Test: `prompt::template::tests::test_base_template_contains_resolved_tool_names`
- File: `crates/codegen/xai-grok-agent/src/prompt/template.rs:247`
- Time: 07:13:13Z (TRY 1), same panic on TRY 2
- Immediate siblings (same cause, next two tests):
  - `test_base_template_plan_present_includes_planning` (`template.rs:258`)
  - `test_encrypted_templates_not_stale` (`template.rs:79`)

```
TRY 1 FAIL [   0.036s] xai-grok-agent prompt::template::tests::test_base_template_contains_resolved_tool_names
stderr:
  thread 'prompt::template::tests::test_base_template_contains_resolved_tool_names' panicked at
  crates/codegen/xai-grok-agent/src/prompt/template.rs:247:9:
  default renderer includes Plan tool; prompt should teach todo_write
```

Second fail, 56ms later:

```
TRY 1 FAIL [   0.033s] xai-grok-agent prompt::template::tests::test_base_template_plan_present_includes_planning
stderr:
  panicked at crates/codegen/xai-grok-agent/src/prompt/template.rs:258:9:
  Planning section should render when plan tool is present
```

Third fail names the regeneration hole:

```
TRY 1 FAIL [   0.033s] xai-grok-agent prompt::template::tests::test_encrypted_templates_not_stale
stderr:
  panicked at crates/codegen/xai-grok-agent/src/prompt/template.rs:79:9:
  assertion `left == right` failed: prompt.md encrypted bytes are stale — run scripts/encrypt_templates.py
  left: [3, 52, 41, 125, ...]   # BASE_PROMPT_ENC in prompt_encrypted.rs
  right: xor_encrypt(include_bytes!("../../templates/prompt.md"), PROMPT_SEEDS[0])
```

`render_base` decrypts `BASE_PROMPT_ENC` from `prompt_encrypted.rs`. That blob is older than `templates/prompt.md`. The plaintext already has the Surmount planning block:

```
${%- if tools.by_kind.plan %}
<planning>
Use `${{ tools.by_kind.plan }}` for multi-step work ...
```

`default_renderer()` maps `ToolKind::Plan` → `"todo_write"`. The decrypted blob does not, so the rendered prompt has neither `todo_write` nor `<planning>`.

## Named contract (do not weaken)

When the default renderer includes the Plan tool, the base system prompt must teach `todo_write` and must render the `<planning>` section (session board, `feat:` / `bug:`, red/green TDD, `meta.taskId`). Encrypted template bytes in `prompt_encrypted.rs` must match the current `templates/prompt.md` (and the apply-patch / subagent siblings). Do not delete or loosen `test_base_template_contains_resolved_tool_names`, `test_base_template_plan_present_includes_planning`, or `test_encrypted_templates_not_stale`.

## Smallest product fix (first hole only)

Regenerate the encrypted templates with the **existing** in-tree script (do not invent a new one):

```bash
python3 scripts/encrypt_templates.py
```

That should rewrite `crates/codegen/xai-grok-agent/src/prompt/prompt_encrypted.rs` so `BASE_PROMPT_ENC` matches `templates/prompt.md`. Re-run:

```bash
cargo nextest run -p xai-grok-agent prompt::template::tests::test_base_template_contains_resolved_tool_names prompt::template::tests::test_base_template_plan_present_includes_planning prompt::template::tests::test_encrypted_templates_not_stale
```

Those three should go green together. That does **not** clear the other 83 runtime fails.

## Would local targeted nextest have seen it?

**No.** The first fail is `xai-grok-agent`. A local filter of `xai-grok-shell` / `xai-grok-pager` / `xai-grok-sampler` / `xai-grok-pager-minimal` would miss it.

That same targeted filter **would** have seen 47 of the later 83 (22 pager + 21 shell lib + 2 sampler + 1 pager-bin + 1 pager-minimal + 1 pty-harness, plus more shell integration binaries). It would also have seen the three `ABRT` stack overflows in `xai-grok-shell` `acp_session` tests.

## Nextest runtime fails (86 total, first 40)

Nextest: 86 failed after TRY 2 (83 `FAIL` + 3 `ABRT`). Chronological first-seen order:

1. `xai-grok-agent prompt::template::tests::test_base_template_contains_resolved_tool_names` **(first)**
2. `xai-grok-agent prompt::template::tests::test_base_template_plan_present_includes_planning`
3. `xai-grok-agent prompt::template::tests::test_encrypted_templates_not_stale`
4. `xai-grok-pager app::acp_handler::tests::session_events::retry_exhausted_rate_limited_surfaces_server_detail`
5. `xai-grok-pager app::auto_implement::tests::implement_effort_entry_paths_use_shared_helper`
6. `xai-grok-pager app::dispatch::tests::billing::open_url_welcome_toasts_single_line_url_when_browser_unavailable`
7. `xai-grok-pager app::dispatch::tests::global_pause::drain_blocked_while_paused`
8. `xai-grok-pager app::dispatch::tests::settings::set_auto_dark_theme_applies_when_theme_is_auto_and_system_is_dark`
9. `xai-grok-pager app::dispatch::tests::soft_stop::release_holding_resumes_drain`
10. `xai-grok-pager app::dispatch::tests::soft_stop::soft_stop_with_non_empty_queue_does_not_drain_next`
11. `xai-grok-pager app::effects::tests::format_acp_error_rate_limit_surfaces_detail_or_fallback`
12. `xai-grok-pager diagnostics::doctor_format::tests::limited_color_output_is_stable`
13. `xai-grok-pager doctor_cmd::tests::fake_standalone_facts_compose_through_shared_view`
14. `xai-grok-pager doctor_cmd::tests::human_mixed_fixture_is_exact`
15. `xai-grok-pager doctor_cmd::tests::json_contract_is_structural_stable_ordered_and_ansi_free`
16. `xai-grok-pager doctor_cmd::tests::json_empty_fixture_pins_null_policy`
17. `xai-grok-pager scrollback::blocks::user::tests::invalid_token_ranges_are_dropped`
18. `xai-grok-pager scrollback::blocks::user::tests::mid_text_multiple_tokens_each_teal`
19. `xai-grok-pager scrollback::blocks::user::tests::mid_text_token_on_second_logical_line`
20. `xai-grok-pager scrollback::wrappers::entry_renderer::tests::background_block_gutter_uses_block_background_fill`
21. `xai-grok-pager slash::commands::tests::pager_builtin_triggers_are_reserved_in_shell`
22. `xai-grok-pager views::dashboard::render::tests::render_row_needs_input_yellow_blink_no_badge_pending_prefix`
23. `xai-grok-pager views::prompt_widget::tests::title_renders_on_top_border_with_corners_intact`
24. `xai-grok-pager views::settings_modal::tests::max_thoughts_width_preview_title_styling_distinguishes_from_content`
25. `xai-grok-pager views::welcome::toast::tests::paint_welcome_toast_truncates_narrow_width`
26. `xai-grok-pager-bin::update_never_blocked_by_config corrupt_config_never_changes_update_outcome`
27. `xai-grok-pager-minimal overlay::tests::question_input_mode_editor_grows_and_keeps_row_prefix`
28. `xai-grok-pager-pty-harness::plan_approval_resume plan_approval_restored_after_resume`
29. `xai-grok-sampler retry::tests::classify_clamps_and_jitters_retry_after_on_generic_path_but_not_on_429`
30. `xai-grok-sampler retry::tests::cloudflare_edge_range_is_transient`
31. `xai-grok-shell agent::config::tests::resolve_credentials_openrouter_does_not_use_xai_session`
32. `xai-grok-shell agent::subagent::tests::resolve_model_override_api_key_pin_keeps_console_primary`
33. `xai-grok-shell auth::manager::tests::reauth_clear_keeps_supergrok_multi_slots`
34. `xai-grok-shell auth::manager::tests::team_login_then_personal_keeps_both_principals`
35. `xai-grok-shell auth::manager::tests::update_stores_team_token_under_base_and_multi_slot`
36. `xai-grok-shell sampling::error::tests::service_unavailable_retains_http_status_for_classification`
37. `xai-grok-shell session::acp_session::auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries` **ABRT stack overflow**
38. `xai-grok-shell session::acp_session::auth_retry_budget_tests::fail_closed_401_is_uncharged_and_turn_survives` **ABRT stack overflow**
39. `xai-grok-shell session::acp_session::chat_history_integrity_tests::mid_turn_user_injection_must_not_duplicate_tool_results_for_one_tool_use_id` **ABRT stack overflow**
40. `xai-grok-shell session::acp_session::inline_auto_compact_flow_tests::test_economic_mode_caps_header_upgrade_at_200k`

46 more after the cap. Crate counts: `xai-grok-shell::team_managed_config` 30, `xai-grok-pager` 22, `xai-grok-shell` lib 21, `xai-grok-agent` 3, `xai-grok-sampler` 2, then one each of pager-bin, pager-minimal, pager-pty-harness, shell external-auth, shell sampling-client, tools, workspace, ratatui-textarea.

Largest later clusters (not the first fail; do not weaken):

- **30** `xai-grok-shell::team_managed_config *`: mostly `home.join("requirements.toml").exists()` false, `wrote` false, or `unwrap` `NotFound`. Team managed policy is not landing on disk.
- **3 ABRT** `xai-grok-shell` `acp_session` tests: `thread ... has overflowed its stack` / SIGABRT. Same stack-overflow class, not an assertion rewrite.
- Pager pause/soft-stop: queue still drains while paused / holding.
- Sampler: `got 120s` (retry-after clamp) and `classify 525` (Cloudflare edge not transient).

## Remaining 46 names (after the cap)

41. `xai-grok-shell session::acp_session::interjection_actor_tests::interject_contract_queued_prompt_images_ride_pending_interjections`
42. `xai-grok-shell session::acp_session::prompt_queue_actor_tests::interject_contract_idle_keeps_row_queued_no_cancel`
43. `xai-grok-shell session::acp_session::prompt_queue_actor_tests::interject_contract_queued_prompt_buffers_without_cancel`
44. `xai-grok-shell session::acp_session::record_response_token_usage_tests::main_usage_jsonl_keeps_main_identity`
45. `xai-grok-shell session::acp_session::record_response_token_usage_tests::subagent_usage_jsonl_uses_agent_turn_identity`
46. `xai-grok-shell session::acp_session::replay_buffer_send_update_tests::channel_token_text_scrubs_curly_punctuation_when_on`
47. `xai-grok-shell session::acp_session::replay_buffer_send_update_tests::stream_started_emits_retry_state_stream_resumed`
48. `xai-grok-shell session::slash_commands::tests::build_skill_information_for_refs_loads_and_wraps`
49. `xai-grok-shell session::storage::jsonl::tests::workflow_restore_rejects_symlinks_and_caps_run_count`
50. `xai-grok-shell session::unified_list::tests::parse_list_req_forces_kind_under_process_chat_mode_only`
51. `xai-grok-shell util::config::persist::tests::resolve_model_list_drops_remote_auto_compact_undercut_on_stock_grok_45`
52. `xai-grok-shell::external_auth_expired_credential expired_external_credential_routes_to_the_provider_login_flow`
53–82. `xai-grok-shell::team_managed_config` (30 tests; see crate count above)
83. `xai-grok-shell::test_sampling_client test_doom_loop_check_enabled_sends_header_and_absorbs_check_event`
84. `xai-grok-tools computer::local::shell_state::tests::test_user_cmd_var_not_exported_under_allexport_bash`
85. `xai-grok-workspace session::git::restore_code_tests::ensure_binding_forks_conv_branch_off_base_and_is_idempotent`
86. `xai-ratatui-textarea textarea::tests::home_end_use_logical_line_when_soft_wrapped`

Parsed detail TSV: `/tmp/pr36-ci-a036327e/fail-detail.txt`. Full job log: `/tmp/pr36-ci-a036327e/extracted/just ci/6_just ci-prep && just test.txt`.
