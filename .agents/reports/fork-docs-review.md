# Independent review: FORK defense inventory and process/tool write

**Date:** 2026-08-15  
**Role:** L3 independent reviewer. Read-only on product files. This review did not run cargo.  
**Sources of truth:** `.agents/reports/fork-docs-seams.md` (proven vs unproven), plus the map/gaps reports, current `FORK.md`, `doc/dev/upstream-regression-filters.md`, host `git-recon` land, `docs/upstream-history.md` land bits, `scripts/assert-process-pins.sh`, `scripts/recon-status.sh`, `justfile` `upstream-land-filters`.

SuperGrok is a **paid** product. Meters stay distinct: included SuperGrok period limits, SuperGrok dollar credits, console team prepaid / console API credits.

## Verdict

**pass-with-nits**

The write matches the seams inventory on honesty, seven product class numbering, helper-green bans, and chrome-only / paint-only bubble / skills-Python failed-land rules. Spot-checked `fn` names exist. Catalog-required identifiers with no `fn` are no longer treated as land filters. One real land-sheet hole remains: the catalog **operator cheat sheet** class 5 block is thinner than the same file’s Required land class 5 plus 5b cargo.

## What is good

- **FORK is a defense inventory again.** `What Grok OSS adds` now requires a named `fn` or an explicit “shipped in code, no named test” / “file pin only” / “FORK claims” label. Chrome moved out of the Process dump into `### Chrome` one-liners. Land **Rules** vs **Steps** vs **Seven product classes** no longer collide. Class 5 through 7 match the catalog (hop, last-session, skills-not-Python). Class 3 FORK title is Token Economy ledger `/spend` (extra SQL, not SuperGrok dollar credits).
- **Proven seams from the seams report have FORK lines and catalog rows.** Class 1 welcome / tutorial / hero badges and leftover `grok-oss` CLI guide tests; class 2 click-to-copy plus wrap; class 3 ingest pair; class 4 rails, caret, compact included SuperGrok period limits meter, titled composer, five-CTA; class 5 hop, Business / Team included before personal, sibling included before SuperGrok dollar credits, flock, combined remaining; class 6 last-session siblings; class 7 sanitize / extract / roots / guide sentence / three Rust intercepts. Extra neighbors enrolled, not as a second numbered board: plan present is not Approve, SHA-aware `/rebuild`, nucleo reuse-per-root, `from_config` no-prefetch usable catalog, always-three-layer product prompt, pause / Clear finished, user-guide hop and spend-order pins.
- **Honesty labels are in place.** rustc 1.97.1 is file pin only; `rust-toolchain.toml` is not in `FORK_PATHS`. Empty `models_cache.json` miss is not cargo-proven. No live TUI dogfood. Nucleo `Some(2)` is a constant. `/limits` hit-count is prose only. Token Economy `/settings` table rows were not re-proven. Stuck-retry **pager** chrome is not fully proven. Catalog lies with no `fn` are honesty leftovers, not required land.
- **Language in FORK and the catalog body is clean.** No em dashes. SuperGrok is paid. Never “free SuperGrok.” Compact meter is included SuperGrok period limits. Class 3 body says extra SQL is the Token Economy ledger, not SuperGrok dollar credits.
- **Land procedure would refuse a chrome-only closeout.** FORK, catalog, host `git-recon` `recon:land`, `upstream-export-import`, import post-restore NOTE, `docs/upstream-history.md` review checklist, `recon-status.sh` next-action, and `just upstream-land-filters` all name seven product classes and say `just check` cannot fail a deleted catalog test. Paint-only bubble copy and skills Python reintroduced are failed land. Helper-green bans match the 1.0.3 failure modes.
- **Assert stays files plus titles.** Catalog file is required. Seven class title markers are sniffed (worktree and tree-ish). No cargo inside assert. FORK says that is not contract proof. No second inventory file. No cheat-sheet novel dumped into D1 `AGENTS.md` (Survive recon stays a pointer).
- **Spot-check (`rg fn <name>`).** Claimed land names exist in this tree, including the newly enrolled extras (`welcome_badge_brands_grok_oss`, `exit_plan_mode_present_is_not_operator_approve`, `from_config_without_prefetch_produces_usable_catalog`, `peer_relaunch_accepts_same_semver_different_sha`, `repeated_open_without_close_keeps_one_search_per_root`, `child_task_description_is_concise`, `default_max_allows_l2_to_spawn_l3`, hop / flock / spend / skills names). Honesty leftovers have **no** matching `fn`: `retry_chrome_soft_reconnects_when_retry_stream_starts`, `stream_resumed_without_prior_retry_clears_activity`, `shell_collision_contract_covers_every_pager_command_and_alias`, `default_title_items_include_agents`, `title_escape_never_empty_payload`, `title_updates_gated_only_by_title_enabled`, `doge_idle_subagent_still_running`, `doge_tool_running_spinner`. `child_task_description_is_concise` actually asserts three-layer text and forbids “many greps” / “half the window.”

## Must-fix

### 1. Catalog operator cheat sheet class 5 is thinner than Required land §5 plus §5b

**File:** [`doc/dev/upstream-regression-filters.md`](../../doc/dev/upstream-regression-filters.md)  
**Where:** Operator cheat sheet block under `# 5. Dual-auth hop after included SuperGrok period limits are full` (the two `cargo test` lines that currently end at `limits_snapshot_second_process_reads_file_and_does_not_http` and `compact_meter_stays_included_while_sibling_pool_has_remaining`).

The Required land class 5 table plus 5b, and the FORK cheat sheet class 5 block, keep after-burner, Business / Team pick and credential order, stale-flock / never-writes-tokens / billing hub, both combined-remaining tests, and `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`. The operator sheet is labeled “Minimum after import restore or onto tip land” and “Same seven classes.” A land agent who copy-pastes only that minimum class 5 block will not run those filters. That is the incomplete-inventory failure mode, even though hop prefix `sampling_config_auto_use` still runs.

**Exact change:** replace those two class 5 `cargo test` lines with the same commands already printed in this file’s Required land inventory class 5 cargo plus the 5b cargo:

```bash
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use sampling_config_hops_to_sibling_included_before_extras \
  resolve_model_to_sampling_config_auto_use \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  order_credentials_business_included_before_personal_when_both_have_room \
  limits_snapshot_second_process_reads_file_and_does_not_http \
  limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once \
  limits_snapshot_never_writes_access_tokens \
  billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http \
  combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool
cargo test -p xai-grok-pager --lib -- compact_meter_stays_included_while_sibling_pool_has_remaining \
  active_spend_driver_stays_included_while_any_distinct_pool_has_remaining
```

Do not delete the Required land class 5 / 5b tables. Do not invent new identifiers. Do not add rustc or empty-cache as cargo land.

## Nits

- **Catalog class 3 heading** is still `### 3. grok-oss SQL extras (`/spend` ingest)`. The next sentences correctly say this is extra SQL in the Token Economy ledger, not SuperGrok dollar credits. A tired reader of headings only can still mash meters. Safer heading that keeps the assert sniff (`### 3. grok-oss SQL extras` is a prefix match): `### 3. grok-oss SQL extras (Token Economy ledger /spend; not SuperGrok dollar credits)`. Leave `scripts/assert-process-pins.sh` `LAND_CLASS_MARKERS` unchanged if the heading still starts with `### 3. grok-oss SQL extras`.
- **After-burner catalog contract** still says “out-of-allowance mark.” Prefer “after-burner skip mark” or “out of included SuperGrok period limits mark” after the plain thought. Wire names may stay.
- **Land extras parentheticals** in host `git-recon` `recon:land`, `justfile` `upstream-land-filters`, and `docs/upstream-history.md` review item name bubble click, plan present, SHA-aware `/rebuild`, nucleo, and `from_config`. The catalog extra section and the operator extra cargo also list pause / Clear finished, always-three-layer product prompt, and user-guide hop / spend-order. Add those three phrases to the parentheticals so a land agent who only reads the reminder still walks what FORK extra list calls seam loss.
- **Catalog extra plan table** omits `empty_enter_on_revise_prompt_does_not_approve`, `soft_park_empty_ctrl_c_abandons_plan_approval`, and `exit_plan_mode_shows_overlay_even_in_yolo`. FORK Chrome and the FORK extra cargo already name them. Residual `plan` / `soft_park` prefixes still hit them. Optional: add the three exact names to the catalog extra plan table so it matches FORK.
- **Catalog extra SHA table** omits `build_fail_does_not_signal_leaders`. FORK Product and FORK extra cargo include it. Optional enroll next to `failed_install_must_not_replace_or_signal_peers`.
- **Catalog extra pause table** omits `idle_with_subagents_paints_pause_and_stop_hits` and `global_paused_idle_paints_resume_not_stop`. FORK extra cargo includes them.
- **FORK cheat sheet class 5** also runs residual `show_limits` / `format_supergrok_session` / `footer_names_live_principal` / dual `/limits` JSON honesty in the hop block. Those `fn`s exist. They are not hop keys. Prefer leaving them in the residual neighbor block so class 5 stays hop plus flock plus 5b.

## Would 1.0.3 chrome-only land now fail? Why?

**Yes, if the land agent follows FORK / catalog Required land / `git-recon` `recon:land`.**

A chrome-only 1.0.3 pass kept `FORK_PATHS`, some DOGE / rail helpers, and `just check` after deleted catalog reds. That is now an explicit failed land:

1. All seven product classes must be proven by named cargo `fn`s. Paint is class 4 only. Hop, `/spend` ingest, `/settings` plus readers, first-token `grok-oss`, last-session, and skills-not-Python are required.
2. Helper-green is forbidden: substring `grok` on `--version`, theme file exists, schema without ingest, serde without a `/settings` row, rank helpers without `sampling_config` hop keys, bundle still has junk `.py`.
3. Paint-only bubble copy is a failed land (click-to-copy `fn`s must exist). Skills Python reintroduced is a failed land.
4. `rg` each required identifier. A named filter with no matching `fn` is a failed land. Deleting a red catalog test is not a restore.
5. `just check` is quality only. `just upstream-land-filters` is assert plus reminder; it does not run cargo and does not replace the cheat sheet.

**What would still pass on a chrome-only tree:** `scripts/assert-process-pins.sh` (files plus seven catalog titles). That is honest. FORK and the history doc say assert is not contract proof. Closeout that stops at assert plus `just check` is now written as a failed land, not as success.

## Honesty leftovers still undocumented because unproven

These remain unproven. The write **does** document them. Do not enroll them as cargo land until a named `fn` or sniff exists.

| Leftover | Where it is already labeled |
|----------|-----------------------------|
| rustc 1.97.1 / fenix match (file pin; `rust-toolchain.toml` not in `FORK_PATHS`) | FORK Packaging; FORK land “Not a cargo land class” |
| Empty `models_cache.json` miss (`load_fresh` returns `None`) | FORK Product `from_config` bullet; catalog extra `from_config` note |
| Live TUI / dogfood of a rebuilt `grok-oss` | FORK Dogfood snapshot; land step 5 |
| Nucleo pool `Some(2)` | FORK Product nucleo bullet |
| User-guide `/limits` hit-count | FORK user-guide table; catalog extra guide prose |
| Last-session guide sentences; three-layer guide paragraph | FORK user-guide table |
| Stuck-retry **pager** chrome (`retry_chrome_*`, `clip_retry_reason_*`, `retrying_*`) | FORK Chrome honesty; catalog Stuck retry honesty; operator “do not `rg`” list |
| `shell_collision` / pager `SHELL_RESERVED` | FORK Clear finished bullet; catalog shell_collision honesty |
| `default_title_items_include_agents`, `title_escape_never_empty_payload`, `title_updates_gated_only_by_title_enabled` | FORK window-titles bullet; catalog titles honesty |
| Lower-left throbber **color** `fn`s (`doge_idle_subagent_still_running`, `doge_tool_running_spinner`) | FORK Chrome activity bullet; catalog class 4 neighbor note |
| Token Economy / economic-mode / auto-run `/settings` GUI rows | FORK Product economic / Token Economy bullets |
| Session recap / cancel-subagents Settings e2e rows | FORK Chrome `/settings` bullet |
| `[subagents] allow_worktree` actually changing spawn isolation | FORK Chrome `/settings` (copy-only `fn`) |
| Host `~/.agents/skills` as a product land class | FORK Process skills; catalog class 7 |

Nothing in that table is silently claimed as cargo-proven. Nothing in the required-land identifier list lacks a matching `fn`.

End of review.
