# FORK docs finish write

**Date:** 2026-08-15  
**Role:** L3 docs finisher. Docs only. No product `*.rs`. No new cargo tests.

Map: `.agents/reports/fork-docs-finish-map.md`

User-guide paths exist exactly where the map named them under `crates/codegen/xai-grok-pager/docs/user-guide/`. No path surprise.

---

## Leftovers 1–8

| Item | Status | What changed |
|------|--------|--------------|
| 1. `03-keyboard-shortcuts.md` FORK pin | **Done** | Added plan keys `a` / `A` / `?` / `s` / `q` and "Empty Enter never approves a plan." Named the composer footer Enter cue as send / queue / interject. Relabeled `Ctrl+Enter` / Apple `Ctrl+O` / VS Code `Ctrl+L` as soft interject (inject, never cancel). Cancel is Esc / `[stop]` only. Removed the old "Send now (cancels the current turn)" wording for those keys. |
| 2. `16-subagents.md` soft interject | **Done** | One pin next to the worktree-default paragraph: mid-turn interject injects and never cancels. Cancel is Esc / `[stop]` only. Points at `03-keyboard-shortcuts.md`. |
| 3. `22-permissions-and-safety.md` plan Approve | **Done** | Next to the Always-approve definition: always-approve skips tool-permission prompts only. It does not click plan Approve. Links `19-plan-mode.md`. |
| 4. FORK class 5 `/limits` mix | **Done** | Moved `show_limits`, `format_supergrok_session`, `footer_names_live_principal`, `limits_json_lists_two_supergrok_principals_when_both_slots_exist`, `limits_json_honest_single_supergrok_session_cannot_see_team_plan` out of `# 5.` into the neighbor cargo block. Class 5 pager line is hop 5b compact-meter names only. |
| 5. FORK dead identifiers | **Done** | Same-batch Product bullet now names `same_batch_plan_write_before_exit_plan_mode_returns_new_body`. Soft-interject bullet dropped `enter_prompt_mode`; footer cue is shipped in code with no named footer `fn`; never-cancel stays `interject_contract_*`. Dogfood cargo deleted no-`fn` names (`enter_prompt_mode_matrix`, `ctrl_c_dismisses_rewind`, `split_tool_batch_before_exit_plan_mode`, `credentials_rejected`, and the other dead plan-panel identifiers). Kept live / prefix-safe wave filters. Snapshot stays dated and is not required land. |
| 6. Catalog extra honesty | **Done** | Extra `from_config` now says empty `models_cache.json` is a code miss with no named test. Extra Nucleo now says `NUM_NUCLEO_THREADS = 2` is shipped in code and no `fn` asserts `Some(2)`. No cargo lines added. |
| 7. Catalog operator class 3 heading | **Done** | Operator sheet `# 3.` is now `Token Economy ledger /spend (extra SQL, not SuperGrok dollar credits)`. Required land heading `### 3. grok-oss SQL extras` is unchanged. |
| 8. Language on touched sentences | **Done** | New sentences use ASCII (comma, period). No "free SuperGrok." No "out-of-allowance mark." No media-player pause metaphor. Did not wholesale-scrub em dashes in the shared user-guide. |

---

## Files touched

- `FORK.md`
- `doc/dev/upstream-regression-filters.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/22-permissions-and-safety.md`

---

## What stayed UNPROVEN (labeled, not enrolled as land)

These honesty labels were already on disk. This write did not add required-land rows or invent `fn`s.

| Seam | Status after this write |
|------|-------------------------|
| rustc 1.97.1 / fenix match | File pin only. Still not cargo land. |
| Empty `models_cache.json` miss | Still a code miss. Catalog extra now says so. No dedicated `fn`. |
| Nucleo pool `Some(2)` | Constant only. Catalog extra now says so. Reuse-per-root remains the proven cargo. |
| User-guide `/limits` hit-count; last-session guide sentences; three-layer guide paragraph | Text exists. No dedicated cargo pin. |
| Stuck-retry **pager** chrome (`retry_chrome_*`, `clip_retry_reason_*`, `retrying_*`) | No matching `fn`. Neighbor comment still forbids adding those identifiers. |
| `shell_collision` / pager `SHELL_RESERVED` | `fn` gone. Not re-listed. |
| `default_title_items_include_agents`, `title_escape_never_empty_payload`, `title_updates_gated_only_by_title_enabled` | No matching `fn`. Neighbor comment still forbids them. |
| Lower-left throbber **color** | Absent. Not enrolled. |
| Token Economy / economic-mode / auto-run `/settings` GUI rows | Not re-proven. |
| Session recap / cancel-subagents Settings e2e; `[subagents] allow_worktree` actually changing spawn isolation | Copy `fn` only. Unchanged. |
| Host `~/.agents/skills` as a product land class | It is not. |
| Live TUI / dogfood of a rebuilt `grok-oss` | Operator-gated. Dogfood section still says so. |
| Composer footer Enter cue (`enter_prompt_mode` / `enter_prompt_mode_matrix`) | UNPROVEN as a named test. FORK now says shipped in code, no named footer `fn`. Soft interject never-cancel is proven by `interject_contract_*` only. |
| Dead dogfood identifiers from leftover 5 | Removed from the dated snapshot. Not put in Required land. |

---

## Path surprise

None. The three user-guide pages live at the paths the map named.
