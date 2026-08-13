# PR #36 `just ci` FAIL names on `2174fd75`

Read-only extract from GitHub MCP `get_job_logs` job `94384792780` run `31680531078`. No GitHub writes.

## Summary

`Summary [1140.531s] 29833 tests run: 29791 passed, 42 failed, 449 skipped`

Exit 100. fmt/clippy/`--no-run` compiled. This is nextest runtime.

## Clusters

| Crate / binary | Count | Notes |
|----------------|-------|--------|
| `xai-grok-shell::team_managed_config` | 30 | Failed **on GHA**. Not a local `GROK_HOME` excuse. |
| `xai-grok-shell` (lib/other) | 5 | Auth, queue, chat-mode list |
| `xai-grok-pager` | 4 | doctor, scrollback gutter, dashboard blink, prompt title |
| `xai-grok-pager-bin` | 1 | corrupt config vs update |
| `xai-grok-pager-pty-harness` | 1 | plan approval after resume |
| `xai-grok-sampler` | 1 | `cloudflare_edge_range_is_transient` — 525 stays **Fatal**; do not weaken this test |

## Names

### pager (4)

- `xai-grok-pager doctor_cmd::tests::fake_standalone_facts_compose_through_shared_view`
- `xai-grok-pager scrollback::wrappers::entry_renderer::tests::background_block_gutter_uses_block_background_fill`
- `xai-grok-pager views::dashboard::render::tests::render_row_needs_input_yellow_blink_no_badge_pending_prefix`
- `xai-grok-pager views::prompt_widget::tests::title_renders_on_top_border_with_corners_intact`

### pager-bin / pty-harness / sampler (3)

- `xai-grok-pager-bin::update_never_blocked_by_config corrupt_config_never_changes_update_outcome`
- `xai-grok-pager-pty-harness::plan_approval_resume plan_approval_restored_after_resume`
- `xai-grok-sampler retry::tests::cloudflare_edge_range_is_transient`

### shell non-team (5)

- `xai-grok-shell agent::config::tests::resolve_credentials_openrouter_does_not_use_xai_session`
- `xai-grok-shell agent::subagent::tests::resolve_model_override_api_key_pin_keeps_console_primary`
- `xai-grok-shell session::acp_session::auth_retry_budget_tests::authenticated_401s_still_exhaust_after_three_retries`
- `xai-grok-shell session::acp_session::prompt_queue_actor_tests::queue_send_now_never_cancels_uncommitted_front`
- `xai-grok-shell session::unified_list::tests::parse_list_req_forces_kind_under_process_chat_mode_only`

### shell team_managed_config (30)

- `blank_team_id_neither_fails_closed_nor_purges`
- `bootstrap_fails_closed_when_managed_policy_compromised`
- `deploy_key_machine_never_gate_purges_on_team_switch`
- `deployment_key_served_then_deleted_heals_online`
- `deployment_key_sync_records_principal_and_key_fingerprint`
- `deployment_key_wins_over_team_when_both_present`
- `dk_synced_marker_survives_config_blip_with_team_signed_in`
- `empty_deployment_response_falls_through_to_team`
- `empty_dk_response_with_failing_team_leaves_team_policy_intact`
- `expired_refreshable_team_token_heals_after_auth_refresh`
- `expired_team_token_without_successful_refresh_stays_failed_closed`
- `fail_closed_env_cannot_disarm_the_gate`
- `gate_purge_retries_past_a_transient_lock_holder`
- `gate_purge_skips_while_lock_contended`
- `identity_change_permits_offline_team_switch_and_purges_prior_team`
- `logout_clears_team_config`
- `managed_policy_gate_fails_closed_on_deleted_policy_offline`
- `managed_policy_gate_fails_closed_on_deployment_key_switch_offline`
- `marker_dir_squat_is_cleared_and_marker_written`
- `padded_team_id_is_one_identity`
- `post_login_pins_authenticated_team_over_disk`
- `purge_crash_prefixes_stay_armed_and_converge`
- `rejected_deployment_key_falls_back_to_team`
- `served_then_deleted_refetches_best_effort`
- `setup_lock_skip_is_not_reported_as_no_config`
- `sync_retries_after_body_phase_drop`
- `sync_retries_after_transient_error`
- `team_switch_evicts_prior_teams_policy`
- `team_sync_writes_files`
- `withdrawn_artifact_is_removed_on_next_sync`

## Honesty vs prior local mop

Local mop said team_managed + some auth tests were "likely GHA-clean." **Wrong.** They failed on this GHA run. Fix product or test-support so the named contracts pass in a hermetic workspace.

Do not weaken `cloudflare_edge_range_is_transient`. 525 is Fatal (`classify_cloudflare_525_is_fatal` is the sibling SoT).
