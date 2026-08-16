# Postmortem: 1.0.3 restack dropped FORK product seams

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Mode:** diagnosis only. No product restore in this slice.

## 1. What happened

The 2026-08-12 restack onto public Grok Build 1.0.3 (`e5fd4816`, residual Open bullet) kept `FORK_PATHS` files and many helper modules. It did **not** keep several Surmount product seams that live inside shared `xai-grok-*` crates. Import restore never owned those seams. Assert only checks that process files exist. The compile mop that “re-applied Surmount seams” left helpers and unit tests that paint nothing, and in several cases the catalog tests that would have gone red were themselves deleted. The operator’s missing rate-limit display is that class of loss: the always-visible status compact meter for **included SuperGrok period limits** is still computed in `credit_bar.rs` and never pushed onto the status bar. SuperGrok is a paid product. This report says **included SuperGrok period limits**, not “free SuperGrok.” FORK and current chrome copy still say “free SuperGrok period.” That is a language residual, not the missing display.

## 2. Inventory: FORK product seams vs this tree

**How to read the status column**

| Status | Meaning |
|--------|---------|
| **present** | Code (and usually a test) still implements the named contract |
| **dropped** | FORK claims it; paint, click, or hop wire is gone |
| **partial** | Helpers or one surface remain; the operator-visible contract does not |
| **unproven** | Not walked to a hard yes/no this turn |

`FORK_PATHS` / assert **do** restore process docs, scripts, flake, justfile, packaging, and the `grok-rate-limit` crate. Those are not the losses. The table is **crate seams** FORK lists as shipped product.

| FORK claim (plain name) | Status | Evidence in this tree |
|-------------------------|--------|------------------------|
| Status-bar limits meter (compact included SuperGrok period `%` / console `$`; click opens `/limits`) | **dropped** | Helpers + tests in `views/credit_bar.rs`. `render.rs` never `status.push("credits", …)`. `hit_credits.rect` looks up a segment that is never painted. No click handler dispatches `ShowLimits`. See §3. |
| Compact label `free SuperGrok period · N%` (Work C) | **partial** | `compact_meter_text_for_live_identity*` still builds that string. Nothing paints it. Copy still says “free SuperGrok period,” not **included SuperGrok period limits**. |
| `/limits` slash + modal + `limits --json` + FetchBilling | **present** | `slash/commands/limits.rs`, `dispatch_show_limits`, `limits_snapshot.rs`, `limits_cmd.rs`. Operator can still type `/limits`. User-guide no longer documents it (see below). |
| Footer usage warning / team meters after included period is full | **present** | `render.rs` still calls `usage_warning_for_session`. Different surface from the compact status meter. |
| Dual-auth hop after included SuperGrok period limits are full | **dropped** (wire) / **present** (rank helpers) | `order_credentials_for_preferred_auto` and tests still live in `supergrok_identity_rank.rs`. `sampling_config_for_model` hard-codes `failover_api_keys: Vec::new()`. `prepare_sampling_config_for_model` does not fill the chain. Residual already named this. |
| Plan panel five CTAs (Approve / Notes / Clarify / Revise / Quit) | **present** (source, restored 2026-08-13) | `line_viewer.rs` now paints `a approve \| A notes \| ? clarify \| s revise \| q quit` with hit rects. New test `plan_approval_footer_paints_five_cta_vocabulary`. Old catalog names `soft_park_draw_paints_panel_*` are still **gone**. Live TUI old until rebuild. |
| Human green rail + agent magenta rail while running | **present** (restored 2026-08-13) | `UserPromptBlock::accent` / `AgentMessageBlock::accent` + catalog tests. See `.agents/reports/bug-theme-chrome-and-line-color.md`. Live TUI still old until rebuild. |
| DOGE default theme | **present** | `default_theme_is_doge`, `resolve_from_config_no_config_returns_doge`. |
| `hide_header` zeros status / welcome / dashboard headers | **dropped** (paint) | Field still on `UiConfig` and appearance config. **No pager read.** Catalog tests `hide_header_zeroes_*` are gone. Serde default-false tests still pass. |
| Window titles on by default | **partial** | `TitleConfig.enabled` defaults true; `notifications` still gates OSC on it. Catalog tests `window_title_always_manages_*` / `titles_on_session_*` are **gone**. |
| Auto-compact must not wipe the todo board | **dropped** | `session_notification.rs` on `AutoCompactCompleted` still calls `agent.todo.update_todos(Vec::new())`. Test `auto_compact_completed_preserves_todo_board` is **gone**. |
| Binary / branding `grok-oss` | **partial** | Composition-root bin is `grok-oss`. Welcome / tutorial chrome says **Grok Build**. Test `product_cli_name_is_grok_oss` is gone. |
| Shared HTTP rate-limit crate (`grok-rate-limit`) | **present** | In `FORK_PATHS` + assert `REQUIRED_DIRS`. Cooldowns under `~/.grok/rate_limits/`. This is **not** the status compact meter. |
| OpenRouter model + login | **present** | `auth/openrouter.rs` and credential probe still in tree. |
| UDAX JSON→TOON | **present** | `util/toon` + densify paths in tools. |
| `usage.jsonl` append | **partial / unproven** | Write path in `session/usage_log.rs` + tests. 2026-08-11 catalog mop had two disk-write reds. Not re-run this turn. |
| Soft interject contracts | **partial / unproven** | `interject_contract_*` tests still exist. 2026-08-11 mop had residual reds. Not re-run. |
| `/rebuild` + fleet SIGUSR1 + fail does not signal | **present** (source, 2026-08-13) | See rebuild / ENXIO reports. Installed binary may still be the pre-fix copy. |
| `--version` without a TTY | **present** (source, 2026-08-13) | Early dispatch in `pager-bin`. Install verify still fails on the old installed file. |
| Keyring helper reuse + one history-search thread | **present** (source) | Tests named in `.agents/reports/bug-thread-leak-keyring-history.md`. Live TUIs still hold leaked threads until quit. |
| Stuck Retrying / StreamResumed | **unproven** | Catalog names still exist; not re-run this turn. |
| Clear finished `/clear-completed-todos` | **present** (slash + dispatch) | Pane chrome `[−]` paint not re-walked. |
| Fearless `[pause]` / `[stop]` status chips | **partial / unproven** | Dispatch exists (`ToggleGlobalPause`). Status-row `[pause]` paint not found in `render.rs` this turn. |
| Composer Enter send / queue / interject cue | **present** | `prompt_idle_submit_hint_is_send` and siblings still in `views/agent.rs`. |
| Always-on bubble copy | **partial / unproven** | Config default on in pager-render. Paint path not walked. |
| Skills multi-source / ASCII scrub / Token Economy / economic mode / screenshot | **unproven** | Modules exist. Not the operator’s rate-limit ask. |

User-guide under `crates/codegen/xai-grok-pager/docs/user-guide/` is **not** in `FORK_PATHS`. Whole-tree search of that guide has **no** `/limits` section. FORK said click-the-meter and `/limits` were documented. Onto took the xAI guide.

## 3. Rate-limit / limits chrome (operator’s ask)

The helpful display FORK names is **not** a new meter and **not** the shared HTTP 429 cooldown crate.

**Real surfaces**

| Surface | What it is | Now |
|---------|------------|-----|
| Top status compact meter | Always-on chip: included SuperGrok period limits used `%` (FORK/chrome still spell this `free SuperGrok period · N%`), or `SuperGrok extras · $N`, or `console · $N`. Click was supposed to open `/limits`. | **Dead.** Helpers exist. Paint and click do not. |
| Prompt footer warning | High/full included period, extras left, team prepaid after the included period is full | **Wired** via `usage_warning_for_session`. Not the glanceable compact chip. |
| `/limits` modal and `grok-oss limits --json` | Detail panel + JSON | **Wired** from slash / CLI. No user-guide page. |
| Doctor | Dual-principal / poll honesty | **Unproven** this turn; doctor modules exist. |
| Shared `grok-rate-limit` | Cross-process 429 cooldown files | **Present.** Not on-screen limits chrome. |

**Proof the compact meter is missing in source, not only in an old binary**

1. `credit_bar_line_for_session` / `compact_meter_text_for_live_identity*` still produce `free SuperGrok period · N%` (and extras / console strings). Unit tests in the same file still assert those strings.
2. Those functions are **only** called from `credit_bar.rs` itself. No `status.push("credits", …)` anywhere in the pager.
3. Status paint in `app/agent_view/render.rs` pushes link, background tasks, plan, goal, MCP, workspace mode, **context**, queue, todo badge. Then it does `hit_credits.rect = areas.get("credits")`. That key is never inserted, so the hit rect is always empty.
4. `hit_credits` is hover-only. There is no click → `Action::ShowLimits`. `ShowLimits` exists only from `/limits`.
5. Billing fetch still writes `credit_balance`. Data can be warm while the status row stays mute.

This is **not** “the live TUI is old, source is fine.” An old binary can hide a later restore. Here source itself has the dead path. A successful `/rebuild` after this diagnosis would still ship a TUI with no compact included SuperGrok period limits chip.

The 2026-08-13 theme report said the compact meter was “present in `credit_bar.rs`” and left it untouched. That was true of the **helper**. It was not true of the operator surface. This postmortem does not contradict that rail restore; it names the paint hole that report did not check.

**Language residual (separate):** host law wants **included SuperGrok period limits · N%**. Chrome and FORK still paint/say “free SuperGrok period.” SuperGrok is paid. Board `feat:supergrok-period-limits-language`. Restoring paint should use the included-period name, not revive “free SuperGrok.”

## 4. Already-known restack losses (confirm only)

| Known item | Status now |
|------------|------------|
| Human/agent rails + `accent_model` + DOGE Object Property | **Restored in source** (theme report). Live TUI old until rebuild. |
| SIGUSR1 listener + arm-on-exit + exec + fail-does-not-signal | **Restored in source** (rebuild report). Peers that already died on default SIGUSR1 stay dead until someone starts them. |
| `--version` ENXIO on install verify | **Restored in source** (ENXIO report). `~/.cargo/bin/grok-oss` can still be the pre-fix file. |
| Five-CTA plan panel vs 1.0.3 placeholder | **Restored in source** (2026-08-13 implementer). Test `plan_approval_footer_paints_five_cta_vocabulary` exists. Old catalog names `soft_park_draw_paints_panel_*` are still **absent**. Residual `residual:plan-five-cta-after-103`. Live TUI old until rebuild. |
| Dual-auth hop chain empty after included SuperGrok period limits are full | **Confirmed.** `sampling_config_for_model` always empty failover. Residual `residual:dual-auth-spend-order-after-103`. Rank helpers still green. |
| Keyring helper leak + history-search thread | **Fixed in source.** Live TUI still old. |
| Limits language still says “free SuperGrok period” | **Confirmed** in `ActiveSpendDriver::as_human` and compact helpers. |

## 5. Why assert / catalog / recon missed it

FORK already says this: product seams inside `xai-grok-*` survive only by cherry-pick plus **cargo tests**. Assert proves files exist, not contracts. That warning was correct and still insufficient.

| Loss | Filter that would have caught it | What actually happened |
|------|----------------------------------|------------------------|
| Status compact meter gone | **None.** Catalog has `show_limits` / `format_supergrok_session` / `footer_names_live_principal` and **credit_bar helper** tests. No test that the status bar **pushes** `"credits"` or that click opens `/limits`. | Helper tests stay green. Operator loses the glanceable chip. |
| Hop chain empty | Catalog listed `sampling_config_auto_use_omits_console` / `sampling_config_auto_use` wire-up tests. Those names are **gone** from `config_tests.rs`. Rank-only tests in `supergrok_identity_rank.rs` still pass. | **Green-but-lying.** Ranking is not the hop list on `SamplerConfig`. |
| Five-CTA panel | Catalog had `soft_park_draw_paints_panel_approval_footer_chrome` (red on 2026-08-11). Those names stayed **deleted** through restack. A later implementer added `plan_approval_footer_paints_five_cta_vocabulary`. | Red became silent until a human-visible bug. Catalog cheat sheet still lists the old name, not the new one. |
| Auto-compact wipes todos | Catalog named `auto_compact_completed_preserves_todo_board`. Test is **gone**. Wipe line is back. | Green-but-lying by deletion. |
| `hide_header` paint | Catalog named `hide_header_zeroes_*` + settings_e2e. Paint tests gone. Serde default tests remain. | **Green-but-lying.** Config field is dead. |
| Window-title / `grok-oss` identity | Catalog named `window_title_always_manages_*`, `product_cli_name_is_grok_oss`. Both gone. | Identity/title contracts unenforced. |
| User-guide `/limits` | Not a cargo filter. Guide is not in `FORK_PATHS`. | Onto took xAI copy. `/limits` vanished from docs. |
| Rails / DOGE | Catalog **did** have `user_prompt_block_accent_*`. 2026-08-11 mop said PASS. Later restack dropped paint; 2026-08-13 theme slice restored tests + rails. | Filter works when the tests still exist. |
| SIGUSR1 / `--version` | **No catalog filter** before 2026-08-13. Rebuild/version tests exist **now** after the bugs. | Would not have been caught by the 08-11 cheat sheet. |
| Keyring / history-search threads | **No catalog filter** until the leak tests landed. | Same. |
| Process pins | `assert-process-pins` **did** run and **did** pass. | Expected. It cannot see these seams. |

**Process sequence that produced the miss**

1. 2026-08-11 catalog mop: assert PASS; many helper filters PASS; five-CTA / interject / usage.jsonl left red; **`just check` skipped**.
2. 2026-08-12 restack onto 1.0.3: compile mop restored 1.0.3 cores and “re-applied seams.” Residual still warned about the plan panel and empty hop chain.
3. Red catalog tests that blocked 1.0.3 chrome were dropped or never re-homed. Helper tests kept the catalog looking healthy.
4. FORK cheat sheet and `just upstream-assert-process-pins` never listed “status bar must paint the compact included SuperGrok period limits meter” or “`failover_api_keys` must be filled after included period is full.”
5. Land step in git-recon says assert **and** filters **or** `just check`. `just check` cannot fail a test that no longer exists.

## 6. How to improve the next onto / put-history / join

Numbered, high-signal only. Do not dump this into `AGENTS.md`. Implementers should treat this as recon land law until FORK / git-recon absorb the proposed paragraphs.

1. **Dogfood screenshot surfaces before calling a restack done.** After assert + catalog, someone must open a rebuilt TUI (or a render test that dumps the status row) and look at: Human/agent rails, plan five CTAs, **included SuperGrok period limits compact meter**, `/limits` from that click, SIGUSR1 fleet still alive after a **failed** install. Compile green is not dogfood.

2. **Add catalog filters that fail if the test name is missing.** Owed tests (do not write them in this diagnosis):
   - Status bar paint: a draw test that `status` contains `"credits"` and the compact string (included SuperGrok period limits · N%, or today’s `free SuperGrok period · N%` until language lands).
   - Click: `hit_credits` click dispatches `ShowLimits`.
   - Hop wire: `sampling_config_for_model` / `prepare_sampling_config_for_model` after included period is full has a non-empty console failover (restore the deleted `sampling_config_auto_use_*` names).
   - Plan: keep `plan_approval_footer_paints_five_cta_vocabulary` in the catalog cheat sheet (old `soft_park_draw_paints_panel_*` names are gone).
   - Auto-compact: restore `auto_compact_completed_preserves_todo_board`.
   - `hide_header_zeroes_status_bar_height` + welcome + dashboard.
   - Rebuild: keep `failed_install_must_not_replace_or_signal_peers` and `version_without_tty` in the catalog cheat sheet.

3. **Never delete a catalog-red test to finish a compile mop.** If 1.0.3 chrome cannot satisfy the Surmount contract, the test stays red and the restack is not landed. Helper-only green is a lie.

4. **Keep the assert vs catalog split, and add a third “paint” line.** Assert = paths. Catalog = named cargo contracts. Paint = the screenshot / draw list in (1). FORK already has the first two. The third is what this restack skipped.

5. **User-guide conflict resolve is part of land.** Shared guide is not in `FORK_PATHS`. Onto must re-check `/limits`, DOGE default, titles, Grok OSS branding. A guide with zero `/limits` hits is a failed land.

**Proposed FORK paragraph** (do not edit FORK unless this becomes standing law):

> Product seams inside `xai-grok-*` are not restored by `FORK_PATHS`. After onto, assert plus helper unit tests are not enough. A restack is not done until the catalog **paint** filters exist and pass: status compact included SuperGrok period limits meter is pushed and clickable, plan panel shows Approve / Notes / Clarify / Revise / Quit, `sampling_config` hop keys are filled when included SuperGrok period limits are full, AutoCompact does not wipe the todo board, and `hide_header` actually zeros chrome. Deleting a red catalog test is not a restore.

**Proposed git-recon land paragraph** (host skill, same rule):

> Land step 9: run assert, then the catalog cheat sheet, then confirm the catalog **test names still exist** (`rg` the filter identifiers). If a named filter has no matching `fn`, treat land as failed. Then run the dogfood screenshot list (rails, five-CTA, included SuperGrok period limits meter, SIGUSR1 fleet after a failed install). Do not accept “compile mop re-applied seams” without those.

## 7. Leftover honesty

- Live TUIs in this session are **old binaries**. Source restores for rails, SIGUSR1, `--version`, and keyring do not show until a successful install plus full quit/reopen.
- A successful rebuild **will not** bring back the compact included SuperGrok period limits meter. That paint is missing in source.
- Windows that already died on default SIGUSR1 stay dead until someone starts them again.
- Five-CTA paint is restored in source (`plan_approval_footer_paints_five_cta_vocabulary`). This diagnosis did not race that writer. Catalog cheat sheet still needs the new test name. Live TUI still old until rebuild.
- Dual-auth rank helpers can stay green while every live request has an empty hop list.
- This slice did not run `just check` or the catalog cargo blocks. Status above is from code and existing reports, not a fresh nextest pass.
- Diagnosis only. No product restore here.

## Paths (absolute)

- `/home/hunter/Projects/surmount/grok-build/FORK.md`
- `/home/hunter/Projects/surmount/grok-build/RESIDUAL.md`
- `/home/hunter/Projects/surmount/grok-build/doc/dev/upstream-regression-filters.md`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/credit_bar.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/agent/config.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/views/file_search/line_viewer.rs`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-theme-chrome-and-line-color.md`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-rebuild-stopped-after-fail.md`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/bug-install-verify-enxio.md`
- `/home/hunter/Projects/surmount/grok-build/.agents/reports/impl-upstream-catalog-filters-2026-08-11.md`
