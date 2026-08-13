# Upstream catalog filters — land mop (shell/pager unblocked)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior:** compile greens (`impl-upstream-shell-tests-compile`, `pager-lib`, `pager-tests`) + StreamResumed runtime (`impl-upstream-stream-resumed-runtime-2026-08-11`)
**Catalog:** `doc/dev/upstream-regression-filters.md` + FORK § *Upstream regression filters*

---

## Executive status

| Item | State |
|------|--------|
| **Assert** | `./scripts/assert-process-pins.sh HEAD` **PASS** (24 files + 5 dirs) |
| **Core FORK cheat sheet (UI / DOGE / titles / retry / shell_collision)** | **PASS** after mop |
| **Dual SuperGrok / resolve_credentials / sampler hop** | **PASS** |
| **Tools densify/TOON / session_reader / plan_validate** | **PASS** |
| **Plan soft-park panel five-CTA paint** | **3 residual reds** (panel footer not five CTAs) |
| **Shell interject contract** | **3 residual reds** (soft interject queue / cancel behavior) |
| **usage.jsonl identity writes** | **2 residual reds** (file never created; fail-open write path) |
| **Pager interject / wait park markers** | **3 residual reds** (catalog-adjacent) |
| **`settings_e2e` hide_header** | **Skipped** (twice hit 300s timeout; no result) |
| **`just check`** | **Skipped** (cap scope; catalog first) |
| **Stashes** | `recon-temp-work-b-wip-2026-08-10` + `recon-resume-local-dirt-2026-08-10` **kept** |
| **Push / commit** | **Not done** (operator owns TTY GPG) |

**Bottom line:** Process pins green. Previously blocked shell/pager **product** catalog filters largely green. Land mop fixed `shell_collision` (`undo` alias) and `parked_marker` (no Tokio panic). Remaining residual is plan five-CTA panel paint, soft-interject queue contracts, and usage.jsonl disk write for identity tests. Full gate still needs a later pass.

---

## Fixes landed this mop

### 1. `shell_collision` — reserve `undo` (alias of `/rewind`)

**Red:** `slash::commands::tests::shell_collision_contract_covers_every_pager_command_and_alias`
`unreserved pager key undo`

**Product:**

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/slash/commands/mod.rs` | Add `"undo"` to test `SHELL_RESERVED` |
| `crates/codegen/xai-grok-shell/src/session/slash_commands.rs` | Add `"undo"` to `PAGER_COMMAND_KEYS` + `is_reserved_slash_name` |

**Green:** same filter, 1 passed.

### 2. `parked_marker_not_stacked_on_epoch_ticks_mid_park` — no Tokio required

**Red:** `SendPrompt` → `open_project_question` called `Handle::current()` without a runtime.

**Product:** `crates/codegen/xai-grok-pager/src/app/dispatch/session/fork.rs`
Use `Handle::try_current()`; on `Err`, empty recent-dirs (cwd-only path skips modal).

**Green:** same filter, 1 passed.

### Not fixed (residual, not quick)

Panel soft-park footer still paints older a/c/s/q + comment chrome; tests expect five Surmount CTAs (`approve`, `approve_notes`, `questions`, `send`/`revise`, `abandon`). `line_viewer` footer never assigns `approve_notes_button_area` / `questions_button_area`. Needs a dedicated plan-footer paint pass (not a one-line mop).

---

## Filter log (command → result)

### Process

| Command | Result |
|---------|--------|
| `./scripts/assert-process-pins.sh HEAD` | **PASS** |

### Core product harden (FORK cheat sheet)

| Command | Result | Notes |
|---------|--------|-------|
| `cargo test -p xai-grok-shared --lib -- hide_header stale_hide_title` | **PASS** (2) | |
| `cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan doge_roles` | **PASS** (5) | |
| `cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_prefix_matches recap_accent` | **PASS** (4) | |
| `cargo test -p xai-grok-pager --lib -- hide_header window_title titles_on_session default_title_items shell_collision retry_chrome_soft_reconnect stream_resumed_without_prior_retry clip_retry_reason retrying_*` | **PASS** (16) after undo mop | was 15/1 before mop |
| `cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed` | **PASS** (1) | prior product fix held |
| `cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel retry_footer_reason retry_footer_backoff stream_headers_timeout_defaults` | **PASS** (4) | |
| `cargo test -p xai-grok-sampler --test stream_headers_timeout` | **PASS** (1) | |
| `cargo test -p xai-grok-pager --test settings_e2e -- hide_header` | **SKIP / TIMEOUT** | 300s kill ×2; not diagnosed |

### Dual SuperGrok / dual-auth / credits

| Command | Result | Notes |
|---------|--------|-------|
| `cargo test -p xai-grok-shell --lib -- load_candidates_prefers_live resolve_auto_uses_live_supergrok dual_supergrok upsert_personal_then_business` | **PASS** (6) | |
| `cargo test -p xai-grok-shell --lib -- resolve_credentials enforce_disable_api_key store_and_load_round_trip fingerprint_is_not_raw_key multi_add` | **PASS** (27) | |
| `cargo test -p xai-grok-pager --lib -- show_limits format_supergrok_session footer_names_live_principal format_dual_principals live_console_omits` | **PASS** (14) | |
| `cargo test -p xai-grok-sampler --lib -- rotate_ exhausted memo fingerprint hop_reason live_rebind` | **PASS** (33) | |
| `cargo test -p xai-grok-sampling-types --lib -- credit_exhausted` | **PASS** (3) | |

### Bubble / clear-done / identity / clear finished / caret / throbber

| Command | Result | Notes |
|---------|--------|-------|
| `cargo test -p xai-grok-pager --lib -- bubble_copy_ clear_completed_todos product_cli_name_is_grok_oss` | **PASS** (20) | |
| Soft-park strip + clear_finished + caret + throbber subset (with plan filters) | **30 PASS** in mixed batch | 3 plan reds below |

### Plan soft-park (partial)

| Test | Result |
|------|--------|
| `exit_plan_mode_soft_parks_with_toast_not_modal` (+ dismiss/reopen siblings) | **PASS** |
| `soft_park_draw_falls_back_to_strip_ctas_when_panel_cannot_paint` | **PASS** |
| `soft_park_draw_paints_panel_approval_footer_chrome` | **FAIL** residual |
| `soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared` | **FAIL** residual |
| `soft_park_fullscreen_draw_paints_approval_ctas` | **FAIL** residual |
| `parked_marker_not_stacked_on_epoch_ticks_mid_park` | **PASS** after tokio mop |
| `plan_panel_preview_ctrl_v` / `soft_park_prompt_ctrl_v` | **PASS** |
| `paint_composer_box_cursor_uses_human` / `caret_move_clears_*` | **PASS** |
| `doge_idle_subagent_still_running` / `doge_tool_running_spinner` | **PASS** |
| `pointer_cursor` subset | **PASS** |

### Tools residual-aligned

| Command | Result |
|---------|--------|
| `cargo test -p xai-grok-tools --lib -- densify_mcp densify_structured toon enter_plan_mode session_reader plan_validate` | **PASS** (99) |

### Shell / pager residual-aligned (shipped neighbors; reds recorded)

| Command | Result | Notes |
|---------|--------|-------|
| `cargo test -p xai-grok-shell --lib -- interject handle_interject` | **PARTIAL** | 44 pass; **3 interject contract fails** |
| `cargo test -p xai-grok-shell --lib -- usage_log record_response_token_usage` | **PARTIAL** | chat-state tests pass; **2 usage.jsonl disk fails** |
| `cargo test -p xai-grok-pager --lib -- btw interject force_interject login_ dual_auth_hop_reason` | **PARTIAL** | 183 pass; **3 pager interject/park fails** |

### Skipped

| Item | Why |
|------|-----|
| `just check` / full nextest | Scope cap; catalog first |
| Full residual UDAX T0–T6 free_text / dynamic_to_prompt / … | Tools densify/toon green; broader residual block not re-run |
| Full dual-auth `login_` / pager dual_fill_provenance / limits_honesty | Not in FORK minimum; partial via limits format filters |
| workspace / btw product e2e beyond lib filters | Not required for this mop |
| `settings_e2e` | Timeout |

---

## Residual reds (fail logs)

### A. Plan panel five-CTA footer (product; not strip)

```
soft_park_draw_paints_panel_approval_footer_chrome
  panel footer must expose all five approval CTA hit targets after soft-park draw

soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared
  usual five approval CTA hits must paint after resync; comment_btn=Some(Rect { x: 57, y: 26, width: 9, height: 1 })

soft_park_fullscreen_draw_paints_approval_ctas
  fullscreen approval must not paint casual c-comment as the only footer
```

**Cause (evidence):** `views/file_search/line_viewer.rs` footer paints approve / request-changes / **comment** / quit. It never sets `approve_notes_button_area` or `questions_button_area`. Strip path `paint_soft_park_cta_buttons` is five-CTA and used as fallback when panel has no approval hits; side-panel path is still old chrome.

**Suggested fix (next implementer):** teach line_viewer plan-approval footer to paint Surmount five CTAs (a / A notes / ? clarify / s revise / q) and clear `comment_button_area` while `feedback_active`. Mirror `paint_soft_park_cta_buttons` hit model.

### B. Soft interject queue contracts (shell)

```
interject_contract_queued_prompt_buffers_without_cancel
  left: ["running", "p1", "held"]  right: ["running", "held"]
  (interjected row should leave the queue)

interject_contract_idle_keeps_row_queued_no_cancel
  left: ["q2", "q1"]  right: ["q1", "q2"]
  (idle soft-interject must not reorder)

interject_contract_queued_prompt_images_ride_pending_interjections
  soft interject must never request cancel
```

### C. usage.jsonl identity (shell)

```
main_usage_jsonl_keeps_main_identity
subagent_usage_jsonl_uses_agent_turn_identity
  usage.jsonl written for …: Os { code: 2, NotFound }
```

`append_usage_record` is fail-open + create_dir_all; disk file never appears → `record_response_token_usage` likely not calling append on this onto hybrid, or session_dir path wrong. Other `record_response_token_usage_*` chat-state tests **pass**.

### D. Pager interject / wait park (catalog-adjacent)

```
interject_contract_queue_shared_never_arms_cancel_while_running
  assertion failed: is_self_originated_prompt("srv-row-1")

wait_on_already_completed_task_pushes_no_parked_marker
  assertion failed: !agent.renders_parked()

task_backgrounded_after_zero_work_wait_all_restores_park
  left: 0  right: 1  (park re-eval)
```

---

## Tree notes

- Working tree still has **prior recon mop dirt** (many pager/shell paths staged/unstaged from earlier compile land). This mop’s intentional product edits:
  - `pager/.../slash/commands/mod.rs` (`undo` in SHELL_RESERVED)
  - `shell/.../slash_commands.rs` (`undo` in `PAGER_COMMAND_KEYS` + reserved match; rustfmt re-indented the const array)
  - `pager/.../dispatch/session/fork.rs` (`try_current` for project picker)
- Stashes **not** dropped.
- No agent commit / push.
- `cargo fmt -p xai-grok-pager` hit pre-existing broken `tests/pty_e2e/reparked_wait_repushes_buried_marker` mod path (missing file). Touched sources formatted with edition-2024 rustfmt on paths only.

---

## Operator next

1. **Commit mop** (TTY, signed): include this report + undo reserve + project-picker try_current + any still-uncommitted compile mop from earlier reports. Agent will not stage/commit unless asked.
2. **Plan five-CTA panel paint** implementer (A above) before dogfood of plan approval on onto tip.
3. **Interject + usage.jsonl** mop (B/C/D) when soft-interject and double-entry spend honesty are recon-critical.
4. **`settings_e2e hide_header`** re-run with longer budget or isolate hang.
5. **`just check`** once residual reds of interest are green (or consciously waived).
6. **Rejoin main** still needs human TTY: tip `main` (`f17e84d` class) is **not** an ancestor of this onto branch; use join script + signed merge when ready. Do not force from agent.

---

## 10-line summary

1. Assert process pins on HEAD: **PASS**.
2. Core catalog (DOGE, titles, retry, StreamResumed, sampler headers, dual SuperGrok resolve, limits format): **PASS**.
3. `shell_collision` red fixed: reserve **`undo`** (rewind alias) in pager test list + shell `PAGER_COMMAND_KEYS`.
4. `parked_marker` red fixed: project picker uses **`Handle::try_current`**.
5. Plan soft-park **strip** fallback green; **panel five-CTA** paint still red (3 tests).
6. Soft **interject** shell contracts red (3); usage.jsonl disk identity red (2).
7. Pager interject/wait park reds (3) noted residual.
8. Tools densify/TOON/session_reader/plan_validate: **PASS** (99).
9. `settings_e2e` + `just check` **skipped** (timeout / scope).
10. Stashes kept; no commit/push; rejoin main still operator TTY.
