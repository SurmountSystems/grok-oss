# FORK docs finish map

**Date:** 2026-08-15  
**Role:** L3 explore map for the docs finisher. Read-only on product files. No cargo. No product edits.

Sources: `.agents/reports/fork-docs-review.md`, `.agents/reports/fork-docs-defend-upstream.md`, `.agents/reports/fork-docs-fix.md`, current `FORK.md`, current `doc/dev/upstream-regression-filters.md`, FORK user-guide table vs pages under `crates/codegen/xai-grok-pager/docs/user-guide/`.

SuperGrok is a **paid** product. Meters stay distinct: included SuperGrok period limits, SuperGrok dollar credits, console team prepaid / console API credits.

Do not invent cargo tests. Do not enroll UNPROVEN seams as shipped. Do not park leftovers as optional later.

---

## Already done / no longer true

The independent review's must-fix and most nits are already on disk (`fork-docs-fix.md`). The finisher must not re-do these.

| Review leftover | Current disk |
|-----------------|--------------|
| Catalog operator cheat sheet class 5 thinner than Required land §5 plus §5b | **Done.** Cheat sheet class 5 now includes after-burner, Business / Team pick and credential order, stale-flock / never-writes-tokens / billing hub, both combined-remaining names, and `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`. |
| Catalog class 3 heading mashed meters | **Done.** Required land heading is `### 3. grok-oss SQL extras (Token Economy ledger /spend; not SuperGrok dollar credits)`. Assert prefix `### 3. grok-oss SQL extras` still matches. Leave `LAND_CLASS_MARKERS` alone. |
| After-burner catalog contract said "out-of-allowance mark" | **Done.** Class 5 row now says "out of included SuperGrok period limits mark." `fn` name unchanged. |
| Catalog extra plan / SHA / pause tables omitted FORK extra `fn`s | **Done.** Plan table has `empty_enter_on_revise_prompt_does_not_approve`, `soft_park_empty_ctrl_c_abandons_plan_approval`, `exit_plan_mode_shows_overlay_even_in_yolo`. SHA table has `build_fail_does_not_signal_leaders`. Pause table has `idle_with_subagents_paints_pause_and_stop_hits`, `global_paused_idle_paints_resume_not_stop`. Operator extra cargo matches. |
| Land-extras parentheticals omitted pause / three-layer / user-guide hop | **Done** in `docs/upstream-history.md` review item, `justfile` `upstream-land-filters`, and host `git-recon` `recon:land`. Do not rewrite those reminders. |
| FORK and catalog body language (paid SuperGrok, no "free SuperGrok", no em dashes in those two files) | **Done** for `FORK.md` and the catalog body. Do not start a wholesale em-dash rewrite of the shared xAI user-guide. |
| Welcome / last-session siblings / hop extras missing from catalog | **Stale seams report.** Those `fn`s are enrolled in Required land / extras now. Do not re-enroll. |
| Honesty leftovers undocumented | **Stale.** They are already labeled in FORK and catalog. See UNPROVEN below. Do not add cargo land rows. |

User-guide pins that **already exist** on disk (do not rewrite unless touching the page for another leftover):

- `01-getting-started.md`: binary is `grok-oss`; bare interactive open is last session for this cwd, not Welcome.
- `02-authentication.md`: SuperGrok is paid; three meters; `/limits` and compact chip; hop after included SuperGrok period limits are full. Cargo `user_guide_does_not_claim_automatic_host_hop_is_unshipped` and `user_guide_names_token_economy_spend_order` already pin 02 and `04-slash-commands.md`.
- `05-configuration.md`: `hide_header` is in-app only; titles use `title.enabled`; `[subagents] allow_worktree` defaults false.
- `06-theming.md`: default theme is DOGE; human green / agent magenta roles.
- `08-skills.md`: not a Python runtime; three intercept stubs; office/docx/pptx/xlsx/pdf exception.
- `16-subagents.md`: worktree isolation off by default; three-layer paragraph. (Soft interject sentence is still missing. See leftover 3.)
- `17-sessions.md`: last-session vs `-c` / `--resume` vs `canceled_turn_resume.json`; resume examples use `grok-oss`.
- `19-plan-mode.md`: present is not Approve; five CTAs; empty Enter never approves; freeform questions, not the questionnaire modal.
- `24-monitoring-usage.md`: `/spend` is the local Token Economy book vs org OTEL; meters named.

---

## Concrete leftovers (file + what to change)

Each item is proven from the review reports plus current disk (`rg fn` / page read). Write these.

### 1. User-guide `03-keyboard-shortcuts.md` is missing the FORK pin

**FORK promised:** "Plan keys and Enter cue (send / queue / interject). Empty Enter never approves a plan."

**Disk:** the page has mid-turn queue vs send-now. It does **not** say empty Enter never approves a plan. It does **not** list the five plan keys (`a` / `A` / `?` / `s` / `q`). It does **not** name a composer footer Enter cue as send / queue / interject.

Worse: the same keys FORK calls soft interject (`Ctrl+Enter`, VS Code `Ctrl+L`) are documented as **Send now (cancels the current turn)**. That contradicts FORK Chrome ("mid-turn interject injects into the current turn and **never cancels**") and `interject_contract_*` (those `fn`s exist). `21-terminal-support.md` heading "Ctrl+Enter does not interject in WezTerm" keeps the interject name.

**Change:** add a short pin block (or a one-line pointer to `19-plan-mode.md` plus the missing sentences here): plan keys; empty Enter never approves; Enter cue is send / queue / interject. Separate **soft interject** (inject, never cancel; cancel is Esc / `[stop]` only) from any cancel-and-send path. Do not invent a cargo test. Do not use a media-player freeze metaphor.

### 2. User-guide `16-subagents.md` is missing "soft interject never cancels"

**FORK table and process-gaps pin:** worktree isolation off by default (present) **and** soft interject never cancels (absent). `rg` on this page finds no `interject` / `never cancel`.

**Change:** one complete sentence next to the three-layer / worktree pins: mid-turn interject injects into the current turn and never cancels. Cancel is Esc / stop only. Point at `03-keyboard-shortcuts.md` if the Enter-cue block lands there.

### 3. User-guide `22-permissions-and-safety.md` is missing the plan-Approve pin

**FORK promised:** "Always-approve is tool permissions only, not plan Approve."

**Disk:** the page explains always-approve as a tool-permission mode. It never says `exit_plan_mode` present is not operator Approve, and never says always-approve does not click Approve. That sentence lives only on `19-plan-mode.md`.

**Change:** one complete thought near the always-approve definition: always-approve skips tool-permission prompts only. It does not click plan Approve. Link `19-plan-mode.md`.

### 4. `FORK.md` cheat sheet class 5 still mixes residual `/limits` neighbors into hop

**Review nit** (`fork-docs-review.md`). **Fix report skipped `FORK.md`.**

**Disk:** land class 5 cheat sheet (`FORK.md` ~850–869) runs hop + flock + 5b **and** `show_limits`, `format_supergrok_session`, `footer_names_live_principal`, `limits_json_lists_two_supergrok_principals_when_both_slots_exist`, `limits_json_honest_single_supergrok_session_cannot_see_team_plan`. Those `fn`s exist. They are not hop keys. Catalog operator cheat sheet class 5 correctly omits them (they stay in residual-aligned block 2b).

**Change:** move those five names out of `# 5.` into the existing neighbor cargo block at the bottom of the cheat sheet (with window titles / retry emit). Leave class 5 as hop + after-burner + Business / Team + flock + combined remaining + 5b compact-meter names, matching the catalog operator sheet.

### 5. `FORK.md` names two identifiers that have no matching `fn`

**Land rule already in FORK:** do not list a catalog identifier that has no matching `fn`.

| FORK line | Disk |
|-----------|------|
| Product **Same-batch plan write** names `split_tool_batch_before_exit_plan_mode` | **No** `fn split_tool_batch`. Live name is `same_batch_plan_write_before_exit_plan_mode_returns_new_body` in `xai-grok-shell` `plan_approval_resume_tests.rs`. |
| Chrome **Soft interject / Enter cue** says the composer footer follows `enter_prompt_mode` | **No** `enter_prompt_mode` symbol and **no** `fn enter_prompt_mode_matrix` in this tree. Soft interject is proven by `interject_contract_*` only. |
| Dogfood snapshot cargo (`FORK.md` ~626–636) lists `enter_prompt_mode_matrix`, `ctrl_c_dismisses_rewind`, `soft_park_revise_cta_click_submits_cancelled_immediately`, `panel_empty_prompt_s_submits_cancelled_immediately`, `plan_panel_click_clarify_revise_quit`, `panel_prompt_empty_enter`, `soft_park_present_status`, `plan_feedback_queue`, `split_tool_batch_before_exit_plan_mode`, `credentials_rejected` | **No** matching `fn` for those exact names. Some nearby names exist (`empty_enter_on_revise_prompt_does_not_approve`, `in_flight_followup_shows_plan_feedback_queue_toast`, `forbidden_bad_credentials_is_auth_error`, `work_control_chrome_matrix_pause_not_cancel_stop_not_pause`). |

**Change:**

- Product same-batch bullet: replace the dead name with `same_batch_plan_write_before_exit_plan_mode_returns_new_body`, keep "not one of the seven product land classes."
- Chrome Enter-cue sentence: drop `enter_prompt_mode`. Say the cue is shipped in code with **no named footer `fn`**. Keep `interject_contract_*` as the proven cancel-never contract.
- Dogfood cargo: delete identifiers with no `fn`. Keep only live names, or prefix-safe live filters already used in the seven-class / extra sheets (`work_control_chrome_matrix_pause_not_cancel_stop_not_pause`, `pause_button_click_dispatches_global_pause_not_cancel`). The snapshot stays dated and is not required land.

### 6. Catalog extra honesty notes that FORK already has, catalog extra sections still omit

Review honesty table said empty `models_cache.json` and nucleo `Some(2)` are labeled in the catalog extra notes. **Disk:** they are labeled in FORK Product only.

**File:** `doc/dev/upstream-regression-filters.md`

- Extra `#### from_config cold catalog`: after the DOGE-theme miss sentence, add the FORK honesty sentence: empty `models_cache.json` is a miss in code (`load_fresh` returns `None` when `models` is empty). That branch has no named test. Do not claim it is cargo-proven. Do not add a cargo line.
- Extra `#### Nucleo reuse-per-root`: add that per-matcher pool size `NUM_NUCLEO_THREADS = 2` is shipped in code. No `fn` asserts `Some(2)`.

### 7. Catalog operator cheat sheet class 3 heading still mashes the ledger name

**File:** `doc/dev/upstream-regression-filters.md` line ~715.

Required land heading already says Token Economy ledger, not SuperGrok dollar credits. The operator sheet still says `# 3. grok-oss ledger /spend`.

**Change:** `# 3. Token Economy ledger /spend (extra SQL, not SuperGrok dollar credits)`. Do not change the assert marker (`### 3. grok-oss SQL extras` on the Required land heading).

### 8. Language only if you touch those sentences

- Do not wholesale-scrub unicode em dashes in the shared user-guide (upstream prose). If you edit 03 / 16 / 22 pin sentences, write those new sentences in ASCII (comma, period, or `...`).
- Do not write "free SuperGrok."
- Do not revive "out-of-allowance mark."
- Do not invent a media-player pause metaphor (FORK dogfood already forbids it).

---

## UNPROVEN seams (do not enroll as shipped)

Already labeled. Leave the honesty labels. Do **not** add catalog required-land rows. Do **not** invent `fn`s.

| Seam | Status |
|------|--------|
| rustc 1.97.1 / fenix match | File pin only. `rust-toolchain.toml` is not in `FORK_PATHS`. |
| Empty `models_cache.json` miss | Code miss; no dedicated `fn`. Leftover 6 is the catalog note only. |
| Nucleo pool `Some(2)` | Constant only. Reuse-per-root is proven. Leftover 6 is the catalog note only. |
| User-guide `/limits` hit-count; last-session guide sentences; three-layer guide paragraph | Text exists. No dedicated cargo pin. |
| Stuck-retry **pager** chrome (`retry_chrome_*`, `clip_retry_reason_*`, `retrying_*`) | No matching `fn`. Shell emit neighbors exist. |
| `shell_collision` / pager `SHELL_RESERVED` | `fn` gone. |
| `default_title_items_include_agents`, `title_escape_never_empty_payload`, `title_updates_gated_only_by_title_enabled` | No matching `fn`. Branded `window_title_*` neighbors exist. |
| Lower-left throbber **color** (`doge_idle_subagent_still_running`, `doge_tool_running_spinner`) | Absent. |
| Token Economy / economic-mode / auto-run `/settings` GUI rows | Not re-proven. |
| Session recap / cancel-subagents Settings e2e; `[subagents] allow_worktree` actually changing spawn isolation | Copy `fn` only. |
| Host `~/.agents/skills` as a product land class | It is not. |
| Live TUI / dogfood of a rebuilt `grok-oss` | Operator-gated. |
| Composer footer Enter cue (`enter_prompt_mode` / `enter_prompt_mode_matrix`) | UNPROVEN as a named test. Soft interject never-cancel is proven by `interject_contract_*` only. |
| Dead dogfood identifiers in leftover 5 | UNPROVEN / gone. Do not put them in Required land. |

---

## Suggested surgical edits

### `FORK.md`

1. Class 5 cheat sheet: hop + flock + 5b only. Residual `/limits` names move to the neighbor block.
2. Same-batch Product bullet: live `fn` `same_batch_plan_write_before_exit_plan_mode_returns_new_body`.
3. Chrome soft-interject bullet: drop `enter_prompt_mode`. Name `interject_contract_*`. Label the footer cue "shipped in code, no named test."
4. Dogfood cargo: drop no-`fn` identifiers. Keep the dated-snapshot framing.
5. Do **not** grow hierarchical one-liners. Do **not** claim empty-cache or rustc as cargo land.

### `doc/dev/upstream-regression-filters.md`

1. Operator cheat sheet `# 3.` heading: Token Economy ledger, not SuperGrok dollar credits.
2. Extra `from_config` note: empty `models_cache.json` honesty.
3. Extra nucleo note: `Some(2)` honesty.
4. Do not delete Required land class 5 / 5b tables.
5. Do not add rustc, empty-cache, retry pager chrome, title-item ghosts, or throbber-color ghosts as cargo land.

### User-guide (FORK-promised pins only)

| File | Add |
|------|-----|
| `03-keyboard-shortcuts.md` | Plan keys + empty Enter never approves + Enter cue send / queue / interject. Soft interject never cancels. |
| `16-subagents.md` | Soft interject never cancels. |
| `22-permissions-and-safety.md` | Always-approve is tool permissions only, not plan Approve. |

Do not paste the user-guide into `FORK.md`. FORK already has one line + link.

---

## Out of scope for the finisher

- New cargo tests for UNPROVEN seams
- Product / `*.rs` edits
- Host skill / justfile / history reminder rewrites (already done)
- Wholesale user-guide em-dash scrub
- Parking any leftover above as "optional later"

End of map.
