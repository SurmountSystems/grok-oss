# Remaining FORK product seams (not config, not SQLite)

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Branch:** `onto-xai/b13fa526f511`  
**Mode:** diagnosis only. No product restore. No FORK / RESIDUAL / product edits.

This report owns every shipped FORK product seam the first inventory skipped, marked unproven, or never listed. It does **not** inventory `config.toml` / settings registry / unread `UiConfig` fields, and it does **not** inventory SQLite extra tables or columns. Those belong to the other two reports.

SuperGrok is a paid product. This report says **included SuperGrok period limits**, not “free SuperGrok.”

**How evidence was gathered:** this L2 agent walked the tree with parallel code search. A host spawn tool for further L3 agents was not available in this session, so disjoint scopes were searched here instead of nested agents. Docs can lie. Status is from source, not a fresh `cargo test` / `just check` pass.

## How to read the status column

| Status | Meaning |
|--------|---------|
| **present** | Code (and usually a test) still implements the named contract |
| **dropped** | FORK claims it; operator-visible paint, click, bind, or docs are gone |
| **partial** | Helpers or one surface remain; the named operator contract does not |
| **unproven** | Not walked to a hard yes/no this turn |

The first postmortem table is **not** restated in full. Items it already called **present** or **dropped** with hard evidence are listed only when this walk found a change or a hole it never named.

## 1. First postmortem: unproven, partial, or never finished

| FORK claim (plain name) | Status | Evidence in this tree |
|-------------------------|--------|------------------------|
| Window titles on by default | **partial** | `TitleConfig.enabled` defaults true; default items include session name + `Grok`. OSC title writer is live (`notifications/title.rs`, `flush_idle_state`, `build_idle_escapes`). Catalog tests `window_title_always_manages_*` and `titles_on_session_*` are still **gone**. Brand token writes **`grok`**, not `grok-oss`. Never-empty branded OSC is not enforced by a remaining named test. |
| Binary / branding `grok-oss` vs Welcome | **dropped** (chrome) / **present** (bin name) | Composition-root package is `grok-oss` (`flake.nix` `pname`, `/rebuild` copy). Full TUI Welcome badge, hero subtitle, tutorial title, and pager-minimal welcome all say **Grok Build**. `product_cli_name_is_grok_oss` is still gone. Title item `Grok` emits `grok`. |
| `usage.jsonl` append (main + subagent) | **present** | `session/usage_log.rs` append path. `sampler_turn.rs` calls `record_model_call` with `UsageIdentity::main` or `agent_turn` (subagent type + optional `work_ulid`). Disk-write tests still in `record_response_token_usage_tests.rs`. Fail-open. Not re-run this turn. |
| Soft interject only (never cancel) | **present** | `interject_contract_queued_prompt_buffers_without_cancel`, bash/idle siblings, and image-ride test still assert `!cancel`. Composer Enter cue tests still live in `views/agent.rs` (`prompt_idle_submit_hint_is_send`). Catalog mop residual reds were not re-run. |
| Stuck Retrying / `StreamResumed` | **present** | Shell test `stream_started_emits_retry_state_stream_resumed` still exists. Pager `session_notification.rs` maps `RetryState::StreamResumed` to reason `reconnecting` and keeps attempt N. Sampler: `stream_headers_timeout_defaults_to_120_secs_when_env_unset`, `retry_footer_reason_uses_short_transport_label` (`connection interrupted` / `response headers timed out`), `retry_footer_backoff_hint_appends_next_try_in`, `wait_before_attempt_aborts_on_cancel`. Catalog name `retry_chrome_soft_reconnect` has **no** matching `fn`. |
| Fearless status `[pause]` / `[resume]` chips | **dropped** (paint) / **present** (dispatch) | `ToggleGlobalPause` + `global_work_pause` + dispatch tests exist. Soft stop is chord-only (`ToggleSoftStop`). The strings `[pause]` and `[resume]` appear **only in FORK.md**, not in any `.rs` file. Status-row chips are gone. |
| Hard stop status `[stop]` | **partial** | Turn-status row still paints `[stop]` (`views/turn_status.rs`) and PTY tests click it. Voice recording paints a separate `[stop]`. Fearless Work B **status-row** `[stop]` next to `[pause]` is not in `render.rs`. Catalog names `work_control_chrome_matrix` and `pause_button_click_dispatches_global_pause` are **gone**. |
| Always-on bubble copy (`⧉`) | **dropped** (paint) / **present** (default) | `[scrollback.display] bubble_copy_buttons` defaults true in pager-render appearance. **Zero** reads of that flag under `xai-grok-pager`. No per-bubble `⧉` paint. Selection-box copy in the plan line viewer is a different surface. OSC 22 pointer helper still exists for links, not for bubble copy. Catalog `bubble_copy_` / `pointer_cursor` paint filters have no matching pager tests. |
| Skills multi-source | **present** | `xai-grok-agent` `prompt/skills.rs` documents and tests: cwd `.agents` then `.grok`, repo, user `~/.agents/skills` then `~/.grok/skills`, `[skills].paths`, plugins, bundled `~/.grok/bundled` lowest. User-guide `08-skills.md` still describes `.agents` + bundled (xAI-shaped, but the machinery is there). |
| ASCII scrub of assistant output | **present** | `util/ascii_scrub.rs` map + tests (em dash → `--`). Stream/chat hook `session/helpers/assistant_ascii_scrub.rs`. Tool `disable_ascii_scrub` still registered. Env / `[ui] scrub_ascii_punct` exist (field inventory belongs to the config report). |
| Economic mode (~200k soft-cap) | **present** | Slash `/economic-mode` + persist `global on/off`. Shell `economic_mode.rs`. Token Economy implement-effort ceiling still keys off this flag. |
| Token Economy (four pillars, minus SQLite schema) | **partial** (schema not walked) | Present in tree: `token_economy/{config,implement_effort,period_pacing,ledger,reconcile}.rs`; `apply_implement_effort_policy`; `/spend`; linear-burn labels (`ahead of linear burn`). Durable store `$GROK_HOME/grok_oss.db` is the other agent’s SQL report. This walk did not open schema. |
| TUI self-screenshot | **partial** | `/screenshot` slash emits `CaptureTuiScreenshot`; event loop flag `pending_tui_screenshot`; encoder `tui_screenshot.rs`. **F9 is unbound** (`registry.lookup(&f9, …) == None` in `actions/mod.rs`). Tests `capture_tui_screenshot_bound_to_f9_always` and `try_attach_tui_screenshot_for_plan_when_approval_open` are **gone**. Plan auto-attach on F9 is gone with the bind. |
| Last-session-on-start (bare `grok-oss` opens last session) | **present** (code) | Distinct from continue-interrupted-turn. `MaterializeCtx::from_pager_args` sets `open_last_session_on_start: true`. Interactive `app/mod.rs` uses that ctx. `NewAuto` resumes most-recent cwd session; missing session stays Welcome. Headless forces false. Tests: `from_pager_args_opens_last_session_on_start`, `materialize_new_auto_opens_last_session_when_one_exists`, stay-welcome, headless-must-not. User-guide `17-sessions.md` still says launch `grok` shows a Welcome picker (see §3). |
| User-guide (not in `FORK_PATHS`) | **dropped** (Surmount pages) | Confirm: `scripts/import-upstream-export.sh` `FORK_PATHS` has no `docs/user-guide`. Whole-guide search has **no** hits for `/limits`, `grok-oss`, `Grok OSS`, `/screenshot`, `/economic-mode`, `/rebuild`, `/note`, `Token Economy`, `ASCII`, `DOGE`, `hide_header`, `window title`, `[pause]`. `19-plan-mode.md` still has five CTA keys + “Empty Enter never approves.” `08-skills.md` still lists `.agents` / bundled. `24-monitoring-usage.md` is org metrics, not `/limits`. `17-sessions.md` documents `grok --resume` / Welcome list, not last-session-on-start and not `canceled_turn_resume.json`. |

## 2. FORK shipped claims the first table never named

| FORK claim (plain name) | Status | Evidence in this tree |
|-------------------------|--------|------------------------|
| ULID helper | **present** | `xai_grok_tools::util::ulid` mints 26-char Crockford ids; `work_ulid` file helper. |
| UDAX JSON→TOON T0–T6 | **present** | First postmortem already called this present. `json_to_toon` tool module still in tree. Not re-audited path-by-path. |
| OpenRouter model + login | **present** | `auth/openrouter.rs`; catalog id `openrouter-grok-4.5`; env + keyring + Zed probe. |
| Multi-key OpenRouter | **present** | Comma/newline lists + `OPENROUTER_API_KEYS_ENV` failover comments and helpers. |
| Dual-auth hop after included SuperGrok period limits are full | **dropped** (wire) | First postmortem already proved empty `failover_api_keys`. Not re-walked. Rank helpers still present. |
| Exhausted-fingerprint memo (`exhausted_credits/`) | **present** | `sampler/exhausted_identity.rs` durable subdir + 1h TTL comments; credit_bar still mentions the memo. Hop list fill is a separate drop. |
| Multi SuperGrok OAuth slots | **present** (helpers) | `list_supergrok_principal_slots` keeps two principals. Dual `/limits` rows were in the first postmortem as `/limits` present. |
| SuperGrok Heavy fresher-slot load | **unproven** | Multi-slot constant `SUPERGROK_PERSONAL_MULTI_SLOT` exists. Catalog names `load_candidates_prefers_live` / “prefer live base over stale multi-slot” were not found as remaining `fn`s in rank. |
| Keyring time-box + keyutils fail-loud | **present** | `KEYRING_OP_TIMEOUT` 3s; Linux keyutils fallback; TTY progress 2× timeout; no silent file dump. First postmortem already had helper-reuse present. |
| Auto-compact default 95% + live-apply | **present** | `DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT = 95` + unit test. Live-apply tests `apply_auto_compact_threshold_updates_gate`. **Todo wipe on AutoCompact** remains dropped (first postmortem). |
| Auto-run `/implement` | **present** | `app/auto_implement.rs` `maybe_enqueue_auto_implement`; Token Economy rewrite of `--effort` still in `implement_effort.rs`. |
| Shared `grok-rate-limit` | **present** | In `FORK_PATHS`. Not on-screen limits chrome. |
| Updates: no xAI channel; `grok-oss update --check` | **present** | `GROK_OSS_ENABLE_XAI_UPDATER` opt-in; default Surmount `main` compare; pager-bin tests. |
| `/rebuild` + SIGUSR1 fleet | **present** | First postmortem (source restore). Not re-walked. |
| Todo board survives auto-compact | **dropped** | First postmortem. Wipe line still the known hole. |
| `plan.json` honesty + `RestoreTodoBoard` | **present** | Todo module still chooses live Resources for `plan.json`. `SessionCommand::RestoreTodoBoard` still in run loop. Compact wipe of the **UI** board is the other drop. |
| Auto-seed `ask:<prompt_id>` | **present** | `turn_end.rs` auto-seed; cap/archive helpers in todo module. |
| Default agent uses `todo_write` | **present** | `prompt/template.rs` test still requires `todo_write` in the prompt. |
| Plan five CTAs | **present** (source) | First postmortem + later restore. Status copy is still **Waiting on plan approval**, not **Plan ready. Side panel open** (residual already said this). |
| Same-batch plan write + `exit_plan_mode` | **present** (renamed) | `split_exit_plan_tail` in `tool_calls.rs`. Test `same_batch_plan_write_before_exit_plan_mode_returns_new_body`. Catalog name `split_tool_batch_before_exit_plan_mode` is **gone**. |
| Plan soft-park auto-opens side panel | **present** | `exit_plan_mode_auto_opens_inline_cursor_plan_preview` and FileBacked sibling still set `plan_approval_view`. |
| `exit_plan_mode` present ≠ Approve | **present** | Always-approve still parks `plan_approval_view` (tests in `plan_mode.rs`). Honest tool-body strings were not re-read line-by-line. |
| Sticky `plan_decision_resolved` / no re-arm after Approve | **dropped** | Identifier `plan_decision_resolved` has **no** `.rs` hits. |
| Revise/Clarify in-flight chrome (`plan_feedback_in_flight`, **Revising plan...**) | **dropped** | Those strings and the flag have **no** `.rs` hits. Status stays **Waiting on plan approval**. |
| Revise barren-wait landing (human line + clear composer) | **unproven** | Decisive Revise click tests may still exist from the five-CTA restore. Barren-wait landing names were not found. |
| Empty Enter never approves | **present** | Plan prompt tests / PTY `plan_revise_empty_enter_does_not_approve`. User-guide `19-plan-mode.md` still says it. |
| Composer caret Human green + no residue (P3) | **dropped** (full TUI paint) | `cursor_box_filled` / `cursor_box_hollow` live only in `pager-render` glyphs. **No** `cursor_box_` use under `xai-grok-pager`. Full prompt widget does not paint the Surmount box caret. Minimal overlay uses a generic reverse-cell caret, not `accent_user`. Catalog `paint_composer_box_cursor_*` names are **gone**. |
| Composer Ctrl+Home / End / Page buffer nav | **present** | `xai-ratatui-textarea` documents Ctrl+Home / Ctrl+PageUp → buffer start. |
| Lower-left throbber agent magenta | **unproven** | `braille_spinner_frames` still used in `turn_status.rs`. Catalog `doge_idle_subagent_still_running` / `doge_tool_running_spinner` names were not found. Theme `accent_running` is still magenta under DOGE. Color on the left cue was not screenshot-proven. |
| Fearless global pause behavior (cancel all + resume once) | **present** (logic) | Dispatch + tests in `dispatch/tests/global_pause.rs`. Discoverable status chips are dropped (above). |
| Soft stop chord only (no button) | **present** | `soft_stop.rs` + tests. No soft-stop button found (matches FORK “not shipped”). |
| Continue interrupted turn (`canceled_turn_resume.json`) | **present** | Module + write helpers + pager test that cold-load re-queues. Distinct from last-session-on-start. |
| Killall / first-activity eager marker | **present** (helpers) | `canceled_turn_resume.rs` eager write comments + `write_canceled_turn_resume`. Live SIGTERM dogfood not re-run. |
| Ctrl+C closes plan approval | **dropped** | Plan overlay key path only forwards ModelPicker / CommandPalette / Quit. Empty-composer Ctrl+C does **not** call `abandon_plan`. Named test `ctrl_c` + plan abandon is gone. |
| Ctrl+C dismisses rewind overlay | **unproven** | `RewindDismiss` exists. Named test `ctrl_c_dismisses_rewind` is **gone**. Not walked to a remaining key map. |
| Rewind skips missing intermediate checkpoints | **present** | `replay_skips_missing_intermediate_checkpoint_when_later_covers_target` still exists. |
| OAuth 403 `bad-credentials` → auth | **present** | `forbidden_bad_credentials_is_auth_error`, `forbidden_bad_credentials_maps_to_auth_required`, `classify_forbidden_bad_credentials_emits_to_session`. |
| Multi-track `meta.taskId` also-guard | **present** | `todo_bound_task_id` + demote reject while Running. Prompt text still teaches bind-after-spawn. |
| Plan selection `@plan.md:N` + screenshots on plan composer | **partial** | Plan Ctrl+V / comment path still exists in plan viewer. F9 plan auto-attach is gone with F9. |
| btw Done-panel `y` copy full thread + `[a]` follow-up | **partial** | `/btw` overlay + `btw_history.jsonl` + `btw_session_id` still exist. Focused `y` / `[y]` / `full_copy_text` identifiers were **not** found. Overlay docs say Esc dismisses only. |
| Incidental “plan” ≠ plan mode (B3) | **present** | `enter_plan_mode_description_requires_explicit_intent` still requires explicit / `/plan` / not casual. |
| Trailing-whitespace strip after product edits | **present** | `util/trailing_ws.rs` + `GROK_STRIP_TRAILING_WHITESPACE` default on; used from edit paths. |
| Prefer Rust tools (implement-memory, plan-validate, session_reader, bulk-edit) | **present** | Modules under `xai-grok-tools/src/util/`. Bash intercept tests still mention the allowlisted `python3 …/memory.py` forms. |
| Subagent `allow_worktree` default false **and spawn honors it** | **dropped** (wire) / **present** (serde) | Field + default-false tests live in `config/mod.rs`. **Zero** product reads outside config serde tests. Spawn does not consult the flag. |
| `/execute-plan` honors `allow_worktree` | **unproven** | Host skill, not in this tree’s product spawn path. Product flag is unread (row above). |
| Todo fib leaves + weighted progress | **present** | `compute_leaf_progress`, size 1\|2, tool output `progress`. |
| Cleared todo archive | **present** | `cleared_todos` ring + `ClearedReason`. |
| Clear finished `[−]` pane chrome | **dropped** (paint) / **present** (slash) | Glyphs `clear_finished_button` exist. **No** pager call sites. Slash `/clear-completed-todos` + dispatch still wired. First postmortem said pane chrome was not re-walked; this walk found paint unused. |
| Click tasks model / timer / `[↗]` → open subagent | **partial** | `open_subagent_fullscreen` still runs from tasks mouse (agent row + double-click). Catalog `click_tasks_model_timer` name is **gone**. Single-click on model+elapsed+`[↗]` chrome was not isolated from double-click / row select. |
| “Worked for …” one live line | **present** (PTY) | PTY tests still require exactly one `Worked for` and no stacked markers. Catalog `parked_marker_not_stacked` name is **gone**. |
| Session recap in Settings | **present** (runtime) | Event loop still applies `session_recap_available` and notification config. Settings-row inventory belongs to the config report. |
| Session notes `/note` | **present** | Slash `/note` / `/notes`; `Action::ShowNotes`; pager `SessionNotes`. |
| Hide header paint | **dropped** | First postmortem. Not re-walked. |
| DOGE default + rails | **present** | First postmortem (restored). Not re-walked. |
| Status compact included SuperGrok period limits meter | **present** (source now) | First postmortem said `status.push("credits")` was missing. **This tree now pushes it** in `render.rs` via `credit_status_line_for_live_session`. Click → `ShowLimits` was not re-proven. Language still says “free SuperGrok period” in helpers (paid-product residual). |
| Packaging: AUR, Nix, justfile, Rust 1.97.1, release-dist | **present** | `packaging/aur/`, `flake.nix` `pname = "grok-oss"`, `justfile` `check` / `build-dist`, `rust-toolchain.toml` `1.97.1`. In `FORK_PATHS`. |
| Process docs / upstream scripts / git-recon probe | **present** | `FORK_PATHS` + assert. Not the crate-seam losses. |
| Doctor dual-principal honesty | **unproven** | Doctor modules exist (first postmortem). Not re-walked. |
| Billing Half A / Half B `/limits` team prepaid | **present** (surfaces) | First postmortem `/limits` present. Series **charts** UI still not shipped (FORK already says that). |
| Token Economy + resume in Settings GUI | **skipped** | Settings registry / unread fields are the config report. |

## 3. User-guide losses (guide is not in `FORK_PATHS`)

Onto kept the xAI shared guide. Surmount product pages that FORK said were documented are missing or contradicted.

| Guide claim FORK named | Now |
|------------------------|-----|
| Click the compact meter / `/limits` | **No** `/limits` string in the guide tree |
| Grok OSS / `grok-oss` branding | **No** hits. Getting started still reads as upstream `grok` |
| DOGE default, titles on, hide_header | **No** hits in `06-theming.md` |
| `/screenshot`, F9, plan auto-attach | **No** slash docs. Only OS paste “screenshots” in `03-keyboard-shortcuts.md` |
| `[pause]` / `[resume]` / `[stop]` / soft-stop chord | **No** Work B section in `03` |
| Last-session-on-start | Guide says Welcome lists sessions. Code opens the last session. |
| Continue interrupted turn | **No** `canceled_turn_resume` / “Continuing interrupted turn” |
| Token Economy, economic mode, ASCII scrub | **No** hits in `05-configuration.md` (worktree **session** modes remain; that is a different flag) |
| Skills `.agents` + bundled | **Partial**: `08-skills.md` still describes multi-source |
| Plan five CTAs + empty Enter | **Present** in `19-plan-mode.md` |
| Clear finished / click-open subagent | **No** hits walked in `16` / `17` this turn |

A guide with zero `/limits` and zero `grok-oss` is a failed land for Surmount docs, even when slash `/limits` still exists in code.

## 4. Leftover honesty

- This slice did **not** run catalog cargo filters or `just check`. Green/red in the tables is from identifiers and call sites, not a fresh nextest pass.
- Config options and unread `UiConfig` fields were **out of scope**. SQLite extras were **out of scope**.
- The first postmortem’s compact-meter row is **stale**: `render.rs` now `status.push("credits", …)`. Click-opens-`/limits` was not re-proven here.
- Live TUIs can still be old binaries. Source present ≠ installed `grok-oss`.
- SuperGrok chrome copy still says “free SuperGrok period” in several helpers. SuperGrok is paid. Restoring or documenting meters should say **included SuperGrok period limits**.
- Catalog cheat sheet still lists deleted `fn` names (`window_title_always_manages_*`, `titles_on_session_*`, `product_cli_name_is_grok_oss`, `retry_chrome_soft_reconnect`, `bubble_copy_`, `paint_composer_box_cursor_*`, `split_tool_batch_before_exit_plan_mode`, `click_tasks_model_timer`, `parked_marker_not_stacked`, `work_control_chrome_matrix`). Missing `fn` = land failed, same as the first postmortem’s proposed paint-filter rule.
- I did not invent a leftover “hybrid handshake” or ranked guess list for the unread spawn flag. `allow_worktree` is unread in product code. That is the finding.

## 5. Counts

| Bucket | Count |
|--------|-------|
| Rows in §1 (first-postmortem unfinished) | 15 |
| Rows in §2 (never named or only named in FORK) | 53 |
| **Total walked claims** | **68** |
| **present** | 38 |
| **partial** | 10 |
| **dropped** | 14 |
| **unproven** | 5 |
| **skipped** (config report) | 1 |

§1+§2 status mix (68): present 38, partial 10, dropped 14, unproven 5, skipped 1.

Highest-signal drops this inventory adds beyond the first chrome postmortem: Welcome still says **Grok Build**; window titles brand `grok` not `grok-oss`; status `[pause]`/`[resume]` never appear in Rust; always-on bubble `⧉` flag is unread; Clear finished `[−]` glyphs are unused; F9 screenshot bind is gone; plan sticky / Revising chrome identifiers are gone; full-TUI Human-green box caret is unused; Ctrl+C does not abandon plan approval; `allow_worktree` is never read by spawn; user-guide lost `/limits` and Grok OSS.

## Paths (absolute)

- `/home/hunter/Projects/surmount/grok-build/.agents/reports/fork-gaps-remaining-seams-2026-08-13.md`
- `/home/hunter/Projects/surmount/grok-build/FORK.md`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/fork-loss-postmortem-2026-08-13.md`
- `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md`
- `/home/hunter/Projects/surmount/grok-build/scripts/import-upstream-export.sh`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/session_startup.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/welcome/mod.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/notifications/title.rs`
