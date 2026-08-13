# Upstream catalog residual re-verify (verify-only mop)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Tip:** `241f6f12`
**Prior reds fixed this session:** catalog mop + interject contracts + plan five-CTA panel + usage.jsonl identity (see sibling reports)
**This pass:** re-run only; no product edits

---

## Result table

| Filter / check | Result |
|----------------|--------|
| `./scripts/assert-process-pins.sh HEAD` | **PASS** (24 files + 5 dirs) |
| Shell `stream_started_emits_retry_state_stream_resumed` | **PASS** |
| Shell `interject_contract_queued_prompt_buffers_without_cancel` | **PASS** |
| Shell `interject_contract_idle_keeps_row_queued_no_cancel` | **PASS** |
| Shell `interject_contract_queued_prompt_images_ride_pending_interjections` | **PASS** |
| Shell `main_usage_jsonl_keeps_main_identity` | **PASS** |
| Shell `subagent_usage_jsonl_uses_agent_turn_identity` | **PASS** |
| Pager `interject_contract_queue_shared_never_arms_cancel_while_running` | **PASS** |
| Pager `wait_on_already_completed_task_pushes_no_parked_marker` | **PASS** |
| Pager `task_backgrounded_after_zero_work_wait_all_restores_park` | **PASS** |
| Pager `soft_park_draw_paints_panel_approval_footer_chrome` | **PASS** |
| Pager `soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared` | **PASS** |
| Pager `soft_park_fullscreen_draw_paints_approval_ctas` | **PASS** |
| Pager `shell_collision_contract_covers_every_pager_command_and_alias` | **PASS** |
| Pager `parked_marker_not_stacked_on_epoch_ticks_mid_park` | **PASS** |
| Optional DOGE (`default_theme_is_doge`, pure green/cyan accents) | **PASS** (3) |
| Optional titles/hide_header (`hide_header` `window_title` `titles_on_session` `default_title_items`) | **PASS** (9) |

**Batch counts:** shell residual batch **6/6**; pager residual batch **8/8**; all zero failed.

---

## Commands run

```bash
./scripts/assert-process-pins.sh HEAD

cargo test -p xai-grok-shell --lib -- \
  stream_started_emits_retry_state_stream_resumed \
  interject_contract_queued_prompt_buffers_without_cancel \
  interject_contract_idle_keeps_row_queued_no_cancel \
  interject_contract_queued_prompt_images_ride_pending_interjections \
  main_usage_jsonl_keeps_main_identity \
  subagent_usage_jsonl_uses_agent_turn_identity

cargo test -p xai-grok-pager --lib -- \
  interject_contract_queue_shared_never_arms_cancel_while_running \
  wait_on_already_completed_task_pushes_no_parked_marker \
  task_backgrounded_after_zero_work_wait_all_restores_park \
  soft_park_draw_paints_panel_approval_footer_chrome \
  soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared \
  soft_park_fullscreen_draw_paints_approval_ctas \
  shell_collision_contract_covers_every_pager_command_and_alias \
  parked_marker_not_stacked_on_epoch_ticks_mid_park

# optional
cargo test -p xai-grok-pager-render --lib -- \
  default_theme_is_doge doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan
cargo test -p xai-grok-pager --lib -- \
  hide_header window_title titles_on_session default_title_items
```

---

## Operator land notes

| Item | State |
|------|--------|
| Product redesign this pass | **None** (verify-only; no fail to mop) |
| Stashes kept | `recon-resume-local-dirt-2026-08-10`, `recon-temp-work-b-wip-2026-08-10` (+ older onto/main stashes) |
| Commit / push | **Not done** (operator TTY + GPG) |
| Rejoin `main` | **`origin/main` (`f17e84d8`) is not an ancestor** of tip (`merge-base --is-ancestor` exit 1). Human join + signed merge still required when ready. |
| Full `just check` | Not re-run (out of residual reverify scope) |

---

## 10-line summary

1. Assert process pins on HEAD: **PASS**.
2. Shell StreamResumed + three soft-interject contracts + two usage.jsonl identity tests: **6/6 PASS**.
3. Pager three interject/wait-park + three five-CTA soft-park panel + shell_collision + parked_marker: **8/8 PASS**.
4. Session fixes that had been residual red (interject soft path, panel CTAs, usage hub append, undo reserve, try_current picker) still hold on tip `241f6f12`.
5. Optional DOGE accents + titles/hide_header batch: **PASS**.
6. No red residual in this catalog set; no product edit required.
7. Stashes **kept** (not dropped).
8. No agent commit or push.
9. Operator still needs **TTY signed commit** of mop product + reports when ready.
10. Operator still needs **rejoin main** (`origin/main` not ancestor); use join script + signed merge, not agent force.
