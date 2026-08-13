# Pager residual inventory (unit tests by cluster)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch tip (session):** `onto-xai/b13fa526f511` @ `241f6f12260d0b977effb54f6f915b55b095d34e`
**Agent:** L2 explore (read-only). This worker has **no shell**; live numbers are from concurrent implementer terminal logs on the same tree (same clock window as this inventory).

**Prior:**
- CI 148 pager fails: `.agents/reports/bug-ci-239-test-cluster-2026-08-11.md`
- After project-picker root: **8689 pass / 120 fail** full lib (`.agents/reports/bug-pager-mass-fail-root-2026-08-11.md`)
- Closed since then (reports): key_owner bar, share `menu_hidden`, DeleteSessionComplete / most delete path, most session lifecycle attach/deferred/chat/cycle

---

## Totals (best live evidence)

| Scope | Pass | Fail | Source |
|-------|-----:|-----:|--------|
| **`app::dispatch::tests` (all dispatch unit)** | **1339** | **45** | Concurrent: `cargo test -p xai-grok-pager --lib app::dispatch::tests` (~2.5s after compile) |
| **`app::dispatch::tests::turn`** | **85** | **0** | Concurrent targeted filter |
| **`app::dispatch::tests::session::`** | **263** | **14** | `/tmp/pager-session-tests.txt` + concurrent session implementer |
| **`app::dispatch::tests::session::fork::`** | **56** | **3** | Concurrent fork-only filter |
| **Full `--lib` (all modules)** | **not re-run here** | **≥ 45** | No full-suite log after today’s mop wave; non-dispatch residual still open (layout / acp oneshots; see below) |

**Implied lib size:** 1339 + 45 + 7436 filtered = **8820** unit tests registered (slightly above the earlier 8809 mass-fail run).

**Working residual for fan-out:** treat **45 dispatch fails** as the hard, fully-named list. Add a small non-dispatch bucket (layout + oneshots) until parent re-runs full `--lib` once.

---

## Cluster table (dispatch 45, fully counted)

| Cluster | Module path prefix | Count | Notes |
|---------|-------------------|------:|-------|
| **prompt** | `app::dispatch::tests::prompt::` | **14** | Interject/send-now, mode slash refusals, prompt-response suppress paths |
| **session load** | `…::session::load::` | **9** | Title + last-turn-summary cold-cache hydrate; sticky chat restore; standalone worktree mark |
| **settings** | `app::dispatch::tests::settings::` | **7** | `hide_header` rollback arm missing; focus/close_on_picker_exit / ZDR deep-link |
| **billing** | `app::dispatch::tests::billing::` | **6** | ShowUsage / session-usage complete chain (was green earlier; **regressed or reopened**) |
| **session fork** | `…::session::fork::` | **3** | Sticky chat rename kind; parent_is_worktree; sticky branch clear on worktree fork |
| **session lifecycle** | `…::session::lifecycle::` | **2** | Dashboard stop double-press / peek (only remaining lifecycle reds) |
| **router** | `app::dispatch::tests::router::` | **2** | Deferred switch overwrite; `/hooks` modal |
| **turn** | `app::dispatch::tests::turn::` | **0** | Green |
| **status** | `app::dispatch::tests::status::` | **0** | Not present in the 45; treat as green for this inventory |
| **key_owner** | `app::agent_view::key_owner::` | **0** | Report: 30/30 green |
| **share / registry** | `slash::registry` + ACP settings kill-switch | **0** | Report: registry 28/0; kill-switch ok |

### Non-dispatch residual (not in the 45; still open from mass-fail)

| Cluster | Est. | Path hint |
|---------|-----:|-----------|
| **scrollback layout** | ~5 | `scrollback::state::layout` (push extend / gap cache half-merge; dead methods still warn) |
| **acp_handler / oneshots** | small | plan approve flush, links, picker cursor, command palette, queue |
| **project-picker** | **0** | Fixture + bound-session guard landed |

**Full-lib fail estimate after today’s mops:** roughly **50–70** if layout + oneshots still red; **45** if only dispatch remains. Parent should confirm with one full `--lib` run after the mop wave settles.

---

## Sample panic per top cluster

### 1. prompt (14) — representative

Live list includes `prompt_response_disk_full_suppresses_turn_failed_and_toast`, mode slash blocks, send-now / interject contracts.

Contract (from test body): `PromptResponse` with disk-full / formatted 401/402 / request-failed banner must **suppress** turn-failed + toast (and 402 takes credit-limit path). Mode slash: `/expand` in fullscreen refuses with fixed system text; `/minimal` when already minimal says "You're already in minimal mode."

**Fix direction:** restore monorepo prompt-response suppress arms + screen-mode slash gate strings in `dispatch/prompt.rs` (and any half-merged interject/send-now paint/queue path). Do not weaken expects.

### 2. session load (9) — representative

```text
last_turn_summary_hydration_is_cold_cache_only
  left: Some("Stale disk read")
 right: Some("Live delivery")
  message: hydration is a cold-cache fallback, never an overwrite
```

Also: title hydrate clobber / sanitize / control-only; sticky `--chat` restore rename kind; standalone worktree `session.is_worktree`.

**Fix direction:** `SessionMetaFromDisk` / title hydrate must be **cold-cache only** (skip when live value present); sanitize/cap titles; stamp worktree on load/restore; sticky chat → `conversation_entry`. Paths: `dispatch/session/load.rs` + meta-from-disk handler.

### 3. settings (7) — representative

```text
every_persisting_setting_has_rollback_arm
  move_setting_away_from_default: no arm for `hide_header`.
  Add one when registering a new setting.
```

Also: `close_on_picker_exit` false when focus opens chooser; ZDR must stop at locked row; deep-link preview Esc + revert.

**Fix direction:** add `hide_header` arm in settings test helpers **only if** product already registers the setting (product has `hide_header` in registry/defs); restore monorepo open-settings-focus close flags. Paths: `dispatch/settings*.rs`, settings registry, tests `settings.rs` helper match.

### 4. billing (6) — representative

Tests: `show_usage_schedules_session_fetch_only`, `session_usage_complete_*` chain, `show_usage_without_session_still_surfaces_credits`.

Contract: `ShowUsage` always schedules session-usage fetch (even when surface hidden); complete pushes block and chains billing unless surface hidden; no-session still shows "unavailable" + non-silent billing.

**Fix direction:** half-merge in usage/billing dispatch or surface-hidden short-circuit. Note: mass-fail had billing **79/79 green** earlier; this cluster may be **new thrash** from concurrent usage work. Paths: `dispatch/billing` or slash usage + task_result usage complete.

### 5. session fork + lifecycle tail (3+2) — representatives

```text
fork_session_ready_sticky_chat_sets_rename_kind_chat
  sticky --chat fork with no local disk opens as chat (rename kind)

startup_fork_parent_is_worktree_for_standalone_clone
  parent_is_worktree: false (expected true for standalone clone)

worktree_forked_clears_sticky_branch_from_main_repo
  sticky main-repo branch must not survive the worktree cwd switch

dashboard_stop_double_press_via_handle_key_deletes_top_level
  first Ctrl+X must arm delete_confirm

dashboard_stop_with_peek_open_moves_selection_and_peek_down_one
  no entry found for key
```

**Fix direction:** fork sticky chat / `parent_is_worktree` / clear sticky branch on worktree fork (sibling implementer may already be mid-path). Dashboard stop: re-land monorepo arm `delete_confirm` + complete path through key_owner (delete report claimed green; **live re-run still red** for these two).

### 6. router (2)

- `deferred_switch_overwritten_by_second_switch` — second SwitchModel must replace stash
- `slash_hooks_opens_modal` — `/hooks` must open hooks modal

Small, independent of load/prompt.

---

## Full failure name list (dispatch 45)

```
app::dispatch::tests::billing::session_usage_complete_no_billing_when_surface_hidden
app::dispatch::tests::billing::session_usage_complete_pushes_block_and_chains_billing
app::dispatch::tests::billing::session_usage_complete_redirect_after_session_block
app::dispatch::tests::billing::session_usage_failed_pushes_error_and_chains_billing
app::dispatch::tests::billing::show_usage_schedules_session_fetch_only
app::dispatch::tests::billing::show_usage_without_session_still_surfaces_credits
app::dispatch::tests::prompt::bash_before_the_session_binds_is_queued_and_recorded
app::dispatch::tests::prompt::fullscreen_mode_blocks_minimal_only_slash_command
app::dispatch::tests::prompt::goal_send_now_painted_block_survives_queue_changed_removal
app::dispatch::tests::prompt::goal_send_now_painted_block_survives_removed_from_queue_response
app::dispatch::tests::prompt::interject_contract_queue_shared_never_arms_cancel_while_running
app::dispatch::tests::prompt::minimal_mode_blocks_fullscreen_pane_slash_command
app::dispatch::tests::prompt::mode_switcher_in_its_own_mode_says_you_are_already_there
app::dispatch::tests::prompt::plain_send_during_blocking_wait_does_not_arm_and_meta_less_cancel_is_visible
app::dispatch::tests::prompt::plain_send_during_blocking_wait_trusts_wire_send_now_trigger
app::dispatch::tests::prompt::plain_send_during_pending_subagent_wait_keeps_confirmed_queue_row_reachable
app::dispatch::tests::prompt::prompt_response_disk_full_suppresses_turn_failed_and_toast
app::dispatch::tests::prompt::prompt_response_formatted_401_suppresses_turn_failed_and_stashes_prompt
app::dispatch::tests::prompt::prompt_response_formatted_402_takes_credit_limit_path
app::dispatch::tests::prompt::prompt_response_request_failed_banner_suppresses_turn_failed_and_toast
app::dispatch::tests::prompt::send_now_during_active_goal_does_not_arm_expectation
app::dispatch::tests::router::deferred_switch_overwritten_by_second_switch
app::dispatch::tests::router::slash_hooks_opens_modal
app::dispatch::tests::session::fork::fork_session_ready_sticky_chat_sets_rename_kind_chat
app::dispatch::tests::session::fork::startup_fork_parent_is_worktree_for_standalone_clone
app::dispatch::tests::session::fork::worktree_forked_clears_sticky_branch_from_main_repo
app::dispatch::tests::session::lifecycle::dashboard_stop_double_press_via_handle_key_deletes_top_level
app::dispatch::tests::session::lifecycle::dashboard_stop_with_peek_open_moves_selection_and_peek_down_one
app::dispatch::tests::session::load::last_turn_summary_hydration_does_not_restore_after_rewind_clear
app::dispatch::tests::session::load::last_turn_summary_hydration_is_cold_cache_only
app::dispatch::tests::session::load::load_session_marks_standalone_worktree_cwd
app::dispatch::tests::session::load::remote_restore_marks_standalone_worktree_cwd
app::dispatch::tests::session::load::session_restored_sticky_chat_sets_conversation_entry
app::dispatch::tests::session::load::session_title_hydration_does_not_clobber_live_generated_title
app::dispatch::tests::session::load::session_title_hydration_manual_restores_display_name_cold_cache_only
app::dispatch::tests::session::load::session_title_hydration_sanitizes_and_caps_dirty_title
app::dispatch::tests::session::load::session_title_hydration_skips_control_only_title
app::dispatch::tests::settings::deep_link_preview_esc_closes_modal_and_forwards_revert_action
app::dispatch::tests::settings::dispatch_open_settings_focus_reopens_when_already_open
app::dispatch::tests::settings::dispatch_open_settings_focus_sets_close_on_picker_exit_when_chooser_opens
app::dispatch::tests::settings::dispatch_open_settings_focus_skips_the_chooser_only_when_locked
app::dispatch::tests::settings::every_persisting_setting_has_rollback_arm
app::dispatch::tests::settings::every_setting_has_action_for_reset_arm
app::dispatch::tests::settings::open_settings_focus_enter_closes_settings_modal
app::dispatch::tests::settings::open_settings_focus_esc_closes_settings_modal
```

---

## Suggested fix order (tests-as-spec: product restore first)

Parallel-safe scopes; do **not** mass-reshape expects. Prefer monorepo product path restore.

| Order | Scope | Est. impact | Disjoint paths |
|------:|-------|------------:|----------------|
| **1** | **session load** hydrate / title / worktree / sticky chat | **9** | `dispatch/session/load.rs`, SessionMetaFromDisk |
| **2** | **prompt** response suppress + mode slash + interject/send-now | **14** | `dispatch/prompt.rs`, interject helpers |
| **3** | **settings** `hide_header` arm + focus close flags | **7** | `dispatch/settings*`, settings registry/helpers |
| **4** | **billing** ShowUsage / session-usage complete | **6** | `dispatch` billing/usage (+ slash usage if needed) |
| **5** | **fork** sticky chat / parent_is_worktree / sticky branch | **3** | `dispatch/session/fork.rs` (+ lifecycle worktree clear if shared) |
| **6** | **dashboard stop** delete_confirm wire | **2** | `dispatch/dashboard.rs` + key path (avoid racing load/prompt) |
| **7** | **router** deferred overwrite + `/hooks` | **2** | `dispatch/router.rs` |
| **8** | **layout** push/gap cache | ~5 | `scrollback/state/layout.rs` only |
| **9** | **oneshots / acp** | small | one filter each after 1–7 green |

**Skip / already green for mop fan-out:** turn, status (dispatch), key_owner, share registry, project-picker fixture, most lifecycle create/attach/deferred.

**Caveat:** concurrent implementers are already on session fork/load and status/turn/settings. Parent should inventory live `task_id`s before spawning writers on the same files.

---

## Parent verify command (full suite)

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8 \
  2>&1 | tee /tmp/pager-lib-full-test.txt | tail -50
```

Then cluster:

```bash
rg 'FAILED$|^    app::' /tmp/pager-lib-full-test.txt
```

---

## 10-line summary

1. Lib **compiles**; residual is **runtime asserts**, not compile.
2. Live dispatch suite: **1339 pass / 45 fail** (hard named list above).
3. Turn **85/0**; status not in the 45 (treat green); key_owner + share closed.
4. Top clusters by count: **prompt 14**, **load 9**, **settings 7**, **billing 6**, fork 3, lifecycle stop 2, router 2.
5. Sample panics: cold-cache hydrate overwrite; `hide_header` rollback arm; disk-full suppress; sticky chat / parent_is_worktree; delete_confirm not armed.
6. Billing reopened vs earlier mass-fail green; check concurrent thrash.
7. Full `--lib` not re-run in this explore worker (no shell); full fail count **≥ 45**, plus possible layout/oneshots.
8. Fix order: load → prompt → settings → billing → fork → dashboard-stop → router → layout.
9. Tests remain product contracts; restore monorepo paths first.
10. No product edits from this inventory agent.
