# Grok OSS fork notes

**Grok OSS** (`grok-oss`) is an **unofficial** open-source fork of
[xai-org/grok-build](https://github.com/xai-org/grok-build) (SpaceXAI’s Grok
Build CLI/TUI), maintained by [Surmount](https://github.com/SurmountSystems).

It is **not** affiliated with or endorsed by xAI / SpaceXAI. Trademarks and
product names belonging to xAI remain theirs.

**Why the fork exists:** upstream publishes under Apache-2.0 but **does not
accept external pull requests**. This repo accepts community PRs. If upstream
ever opens to outside contributions, Surmount intends to **open a PR** and try
to land the useful fork work there.

## Vision

| Pillar | Practice |
|--------|----------|
| **Faithful** | Absorb xAI monorepo exports after review; keep `xai-grok-*` paths for alignment |
| **Complete history** | Surmount `main` is the continuous product archive; xAI is a content feed |
| **Open** | Pull requests welcome **here** |
| **Distinct** | Product **Grok OSS**, binary **`grok-oss`**, clear unofficial labeling |
| **Compatible** | Config and sessions under **`~/.grok`** (shared with upstream if both installed) |
| **Superset** | Fork features sit **on top of** upstream behavior, never hollow out core agent logic |

## Git flow

Normal feature branches → pull request → **`main`**. Temporary tool branches
(`import/*`, `onto-xai/*`) are not a second main; they land via PR.

On **open PRs**, catch up with `main` by **merge**, not rebase (no force-push
while CI runs). Detail: [`docs/git-workflow.md`](docs/git-workflow.md).

## Remotes

```bash
git remote add xai-org https://github.com/xai-org/grok-build.git   # once
# origin → SurmountSystems/grok-oss
# xai-org → xai-org/grok-build
```

## Syncing with xAI

xAI publishes force-pushed snapshots (bot author, often orphan roots, sometimes
short “Synced from monorepo” chains). GitHub may say histories are “entirely
different.” **Expected.** Treat them as a **tree feed**, not shared ancestry.

**Maintainer jobs** (do not confuse them):

| Job | Script | Result |
|-----|--------|--------|
| **Import**: their tree into Surmount history | `./scripts/import-upstream-export.sh` | `import/*` review branch → PR to `main` |
| **Stack on tip**: our product commits on their tip | `./scripts/put-history-on-xai.sh` | `onto-xai/*` (real **cherry-pick**; no `MODE=overlay`) |
| **Join `main` into onto**: landable graph | `./scripts/join-main-into-onto.sh` | same tip; `main` becomes ancestor; **tree kept** (`-s ours`) → PR |

When histories keep breaking: **stack product on their tip**, then **join
Surmount `main`** (`-s ours`) so GitHub compare/PR works, then PR to `main`.
Detect: `./scripts/detect-upstream-export.sh` or `just upstream-detect`.

Full process: [`docs/upstream-history.md`](docs/upstream-history.md)
Import log: [`docs/upstream-import-log.md`](docs/upstream-import-log.md)
Onto log: [`docs/upstream-onto-log.md`](docs/upstream-onto-log.md)

**Never:** reset Surmount `main` to xAI; GitHub “Sync fork” that drops Surmount
commits; unsigned commits; bulk tree rewrites without review.

## What Grok OSS adds (divergence inventory)

Hierarchical: one complete sentence here, then a named `fn` plus crate, or a
linked doc. This list is the **defense inventory** for the next upstream merge.
A checkbox is not proof. Import restores `FORK_PATHS` (docs, scripts, packaging)
only. Product seams inside `xai-grok-*` survive onto only by cherry-pick plus
named cargo tests.

**When you ship a restack-droppable seam:** add the one-liner here with the
exact `fn` name; enroll that filter in
[`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md);
keep helper-green (substring `grok` on `--version`, theme file exists, schema
without `/spend` ingest, serde without a `/settings` row, rank helpers without
`sampling_config` hop keys) out of the land proof. Do not list a catalog
identifier that has no matching `fn`.

### Product

- [x] **UDAX JSON→TOON (T0-T6)**: model-facing structured JSON densifies via
  shared `util/toon` (`GROK_TOOL_RESULT_FORMAT=auto|toon|json`). Not a land
  class. Catalog residual-aligned filters: `toon`, `json_to_toon`,
  `densify_mcp`, `densify_structured`, `task_output_handoff`,
  `subagent_completed_handoff`. Detail:
  [`doc/dev/research/udax-json-toon-2026-07-26.md`](doc/dev/research/udax-json-toon-2026-07-26.md)
- [x] **ULID helper**: `xai_grok_tools::util::ulid` mints 26-char Crockford
  base32 ids for new work/log/tool artifacts; task UUID v7 unchanged. No land
  `fn`. Detail:
  [`doc/dev/research/ulid-helper-2026-07-25.md`](doc/dev/research/ulid-helper-2026-07-25.md)
- [x] **usage.jsonl append log**: fail-open per-session spend log at end of
  model turns (`session/usage_log.rs` ← `record_response_token_usage`). This
  is the **append log**, not `/spend` ingest. Catalog residual: `usage_log`,
  `record_response_token_usage`. Detail:
  [`doc/dev/research/usage-jsonl-2026-07-25.md`](doc/dev/research/usage-jsonl-2026-07-25.md)
- [x] **Last session on start**: interactive `grok-oss` with a remembered last
  session for this working directory opens that session, not Welcome.
  First-ever use stays Welcome. Headless does not steal last-session. Distinct
  from continue interrupted turn (`canceled_turn_resume.json`) and from
  `/resume`. Land: `materialize_new_auto_opens_last_session_when_one_exists`
  (`app/session_startup.rs`). Siblings (enroll so Welcome / headless cannot
  regress silently): `materialize_new_auto_stays_welcome_when_no_last_session`,
  `materialize_new_auto_does_not_open_last_when_headless`,
  `from_pager_args_opens_last_session_on_start`.
- [x] **Binary / branding is `grok-oss`**: `grok-oss --version` first token is
  **`grok-oss`**, not bare `grok` (substring `grok` is how `grok 1.0.3` stayed
  green). Resume and relaunch hints are `grok-oss --resume`. Welcome, tutorial,
  and hero chrome say **Grok OSS**. Crate: `xai-grok-pager`
  `client_identity.rs`, `app/mod.rs`, `views/welcome/`, `views/tutorial.rs`,
  `docs.rs`; bin: `xai-grok-pager-bin` `version_without_tty`. Tests:
  `product_cli_name_is_grok_oss`,
  `product_version_line_uses_grok_oss_not_bare_grok`,
  `resume_session_command_uses_grok_oss`,
  `print_exit_resume_hint_writes_expected_lines`,
  `user_guide_resume_and_version_examples_use_grok_oss`,
  `user_guide_operator_cli_examples_use_grok_oss`,
  `welcome_badge_brands_grok_oss`, `hero_subtitle_brands_grok_oss`,
  `tutorial_list_title_brands_grok_oss`; plus
  `cargo test -p xai-grok-pager-bin --test version_without_tty`.
- [x] **OpenRouter**: separate model option (`openrouter-grok-4.5`);
  login/logout; secret store; optional Zed credential probe (read-only).
  Neighbor (not class 1): `referer_is_surmount_*`, `title_is_grok_oss`.
- [x] **Multi-key OpenRouter**: comma lists / failover keys for credit +
  rate-limit rotation. FORK claims; not a land class.
- [x] **Dual-auth hop after included SuperGrok period limits are full**:
  SuperGrok is **paid**. While included SuperGrok period limits still have
  room, stay on SuperGrok session (`sampling_config` omits console failover).
  After those included limits are full, `sampling_config` fills console
  failover and also switches the API host (SuperGrok proxy ↔ `api.x.ai`).
  Rank helpers alone are not this class. Crate: `xai-grok-shell`
  `agent/config_tests.rs`. Tests:
  `sampling_config_auto_use_fills_console_hop_after_included_full`,
  `sampling_config_auto_use_omits_console`,
  `sampling_config_auto_use_omits_console_while_supergrok_included_headroom`,
  `resolve_model_to_sampling_config_auto_use`,
  `sampling_config_auto_use_extras_keep_session_console_failover`.
  Per-turn reconstruct:
  `prepare_sampler_for_turn_aligns_to_ranked_included_primary`
  (`session/acp_session_impl/sampler_turn.rs`).
- [x] **Any stored SuperGrok login with included remaining before SuperGrok dollar credits**:
  Any stored SuperGrok identity with remaining included SuperGrok period
  limits stays ahead of SuperGrok dollar credits and console. Business /
  Team included still ranks before personal when both have remaining. After
  both included pools are exhausted, SuperGrok dollar credits rank before
  console. Do **not** flatten remaining to zero from `usagePct` /
  `creditUsagePercent` 100 plus missing SuperGrok Heavy. Prior remaining
  stays. A memo without a usage reading still forces remaining 0. Used
  percent below 100 still sets remaining from the percent helper. The
  client must not invent used-up included SuperGrok period limits from a
  100% + missing Heavy snapshot. SuperGrok Heavy ranking optional label is
  **not** implemented. SuperGrok Heavy is a real distinct weekly pool. This
  file does not diagnose product usage of that pool. Rank helpers alone
  are still not hop. Crate: `xai-grok-shell` `agent/config_tests.rs`,
  `session/acp_session_impl/sampler_turn.rs`. Tests:
  `sampling_config_hop_team_remaining_personal_exhausted_not_dollars_or_console`,
  `sampling_config_hop_personal_remaining_team_exhausted`,
  `sampling_config_hop_both_remaining_team_first_then_personal`,
  `sampling_config_hop_both_included_exhausted_dollar_credits_before_console`,
  `sampling_config_hop_missing_heavy_false_100_keeps_sibling_included`,
  `sampling_config_hop_dollar_credits_on_both_missing_heavy_keeps_team`,
  `prepare_sampler_for_turn_does_not_flatten_missing_heavy_100_off_sibling`,
  `prepare_sampler_for_turn_does_not_flatten_dollar_credits_on_both`.
  Existing identifiers
  `sampling_config_hops_to_sibling_included_before_extras` and
  `sampling_config_auto_use_extras_keep_session_console_failover` stay `fn`
  names. Human prose next to them says SuperGrok dollar credits. This
  file does not claim live Business remaining or a live window hop.
- [x] **Business / Team included SuperGrok period limits before personal**:
  spend Business or Team included SuperGrok period limits before this login's
  personal included SuperGrok period limits. Crate: `xai-grok-shell`
  `auth/supergrok_identity_rank.rs`, `auth/manager_tests.rs`. Tests:
  `pick_prefers_business_included_before_personal_when_both_have_remaining`,
  `order_credentials_business_included_before_personal_when_both_have_room`,
  `align_after_billing_switches_sticky_personal_full_to_business_included`.
- [x] **Sibling included SuperGrok period limits before SuperGrok dollar credits**:
  another SuperGrok identity's remaining included SuperGrok period limits beat
  this login's SuperGrok dollar credits. After-burner skip only when every
  included pool is exhausted. Tests:
  `sampling_config_hops_to_sibling_included_before_extras`,
  `afterburner_does_not_skip_mark_when_sibling_has_included_remaining`
  (`auth/allowance_exhaust_from_billing.rs`). Compact meter stays on included
  SuperGrok period limits while a sibling pool has remaining:
  `compact_meter_stays_included_while_sibling_pool_has_remaining`,
  `active_spend_driver_stays_included_while_any_distinct_pool_has_remaining`
  (`views/credit_bar.rs`). Combined remaining sums distinct pools and does not
  double-count a unified pool:
  `combined_included_remaining_sums_distinct_personal_and_business_pools`,
  `combined_included_remaining_does_not_double_count_unified_pool`.
- [x] **One-process SuperGrok billing flock**: one `grok-oss` process fetches
  SuperGrok billing; others read `$GROK_HOME/limits_snapshot.json`. The
  snapshot never stores JWTs or API keys. Crate: `xai-grok-shell`
  `auth/limits_snapshot_hub.rs`, `extensions/billing.rs`. Tests:
  `limits_snapshot_second_process_reads_file_and_does_not_http`,
  `limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once`,
  `limits_snapshot_never_writes_access_tokens`,
  `billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http`.
- [x] **Dual-auth resolve, 429, and credit memo (FORK claims; residual-aligned)**:
  first-party resolve merge (session primary + console failover by default;
  `preferred_method=api_key` reverses). Identity switch on credit / SuperGrok
  Heavy usage-limit and plain 429 (FORK claim, not a land class, not live
  proof). SuperGrok Heavy ranking optional label is **not** implemented.
  SuperGrok Heavy is a real distinct weekly pool. This file does not
  diagnose product usage of that pool. Exhausted-fingerprint memo lives in process
  cache plus `$GROK_HOME/exhausted_credits/` (1h TTL; console-key success
  clears; session success does not). Rate-limit switch uses temporary shared
  `grok-rate-limit` cooldown, not the credit memo. `[auth] auto_use_included_limits`
  defaults true on a new/empty Grok home. Catalog residual: `resolve_credentials`,
  `fingerprint`, `hop_reason`, `live_rebind`, `credit_exhausted`, `dual_auth_hop_reason`.
  Plans: [`.agents/plans/plan-secure-key-failover.md`](.agents/plans/plan-secure-key-failover.md),
  [`.agents/plans/plan-rate-limit-failover.md`](.agents/plans/plan-rate-limit-failover.md),
  [`.agents/plans/plan-auth-preferred-roles-failover.md`](.agents/plans/plan-auth-preferred-roles-failover.md).
- [x] **Three distinct billing meters**: (1) **included SuperGrok period limits**
  (subscription-included quota for the current SuperGrok billing period; how
  much of that included quota is already used); (2) **SuperGrok dollar credits**
  (prepaid top-ups on the SuperGrok account); (3) **console team prepaid /
  console API credits**. SuperGrok is paid. Never call SuperGrok free. Desired
  spend order: included SuperGrok period limits first, then SuperGrok dollar
  credits, then console team prepaid / console API credits. Compact chrome
  paints `included SuperGrok period limits · N%` for the included-period meter;
  SuperGrok dollar credits paint `SuperGrok dollar credits · $N` (live chrome
  must not nickname that meter); console still `console · $N`. `/limits --json`
  `activeDriver` wire values (`supergrok_free_period` | `supergrok_extras` |
  `console_key`) stay wire labels after that plain thought. Land paint: class
  4 compact-meter tests plus
  `compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct` and
  `format_supergrok_session_with_weekly_and_extras`.
  Dual `/limits` honesty (neighbor, not hop):
  `limits_json_lists_two_supergrok_principals_when_both_slots_exist`,
  `limits_json_honest_single_supergrok_session_cannot_see_team_plan`.
  Explicit TUI `/limits` open and CLI `grok-oss limits` collect are
  ForceRefresh. Background FetchBilling is HonorTtl. ForceRefresh without
  a management key does not clear Management caches. First paint can still
  be a fresh-by-TTL HonorTtl snapshot. Do not invent live used percent
  from that file. Tests:
  `management_meter_cache_policy_collect_force_background_honor_ttl`,
  `should_clear_management_meter_caches_force_with_key_only`
  (`xai-grok-pager` `limits_cmd.rs`);
  `limits_snapshot_mode_for_get_billing_explicit_is_force_refresh`
  (`xai-grok-shell` `extensions/billing.rs`).
- [x] **C4 server included-period debit is not a land class**: the client
  must not invent included SuperGrok period used percent. Optional hard block
  is `[auth] allow_spend_when_free_period_debit_unproven = false` (or env
  `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=0`). Default allows sampler
  turns under included SuperGrok period limits with loud honesty when the
  server debit is unproven. Operator ticket:
  [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md).
- [x] **Keyring login time-box + fail-loud**: OS keyring get/set/delete wall
  clock budget (`KEYRING_OP_TIMEOUT`); interactive `grok login --api-key` /
  OpenRouter login require a secure backend. Only if all secure backends fail:
  clear error, no silent `provider_credentials.json` secret dump. FORK claims;
  not a land class. Diagnose with in-tree tests, not host D-Bus probes.
- [x] **Economic mode (cap shipped; slash leftover)**: soft-cap effective
  context at the Grok 4.5 long-context price cliff (~200k) at spawn, model
  switch, and header when `[ui] economic_mode` is on (settings default on
  for **new** sessions). The Settings setter applies to new sessions.
  `/economic-mode` is a pager command that queues that text only. The
  shell has no BuiltinAction arm. Do **not** list `/economic-mode` as a
  live slash or a cargo-proven BuiltinAction. Separate from Token Economy
  implement-effort caps. **Do not claim** a Token Economy or economic-mode
  `/settings` table row as cargo-proven (2026-08-15 seams walk did not
  re-prove those GUI rows).
- [x] **Token Economy (four pillars; `/spend` is the land class)**:
  (1) implement-loop effort 1-5 policy; (2) included SuperGrok billing-period
  linear-burn pacing on `/limits` and `/usage` (never dollar-ize period %);
  (3) **`/spend` ingest** of `usage.jsonl` into `local_usage_event` plus
  `reconciliation_run` (not `DoubleEntryReport::default()`); (4) extra SQL
  ledger `$GROK_HOME/grok_oss.db` (Token Economy ledger, not the session
  store, not SuperGrok dollar credits). Config table `[token_economy]`.
  Land class 3 tests: `spend_path_ingests_usage_jsonl_and_records_reconciliation`
  (`xai-grok-shell` `token_economy/mod.rs`),
  `show_spend_ingests_usage_jsonl_and_is_not_empty_default`
  (`xai-grok-pager` `app/dispatch/tests/status.rs`). Schema v1 without ingest
  is a failed land.
- [x] **Baked default is Grok 4.6 at medium reasoning effort** (fork
  contract change; enabled by default; `[models].default_reasoning_effort`
  is the operator override). Test:
  `baked_default_is_grok_46_medium_fork_contract`
  (`xai-grok-shell` `util/config/persist_tests.rs`).
- [x] **Auto-compact default 95% + live-apply**: stock Grok 4.5 catalog omits
  a per-model undercut; Settings commit live-applies to open sessions.
  FORK claims; not a land class. Detail:
  `docs/dev/research/rca-auto-compact-early-fire.md`
- [x] **Footer context chip names sampling vs catalog when they differ**:
  AUTO compact gates on the sampling window (economic cap 200k when that
  Cell is on). The chip must not paint unlabeled `207K / 500K` as if
  catalog 500k were that gate. Same honesty as the CompactionStarted
  banner. Test:
  `context_chip_names_sampling_window_when_catalog_differs`
  (`xai-grok-pager` `views/context_bar.rs`).
- [x] **Session sampling must not copy catalog 500k**: AUTO compact and
  the footer chip gate on the sampling window, not catalog 500k. Session
  sampling comes from GetSessionInfo / AutoCompactStarted.
  `refresh_context_used` must not copy catalog into that field. Spawn
  seeds the sampling window at the economic cap when disk economic is on.
  Footer sampling can stay empty until GetSessionInfo / AutoCompactStarted;
  then fallback is catalog plus pager economic cache (session field is no
  longer poisoned to 500k). Tests:
  `footer_chip_uses_session_sampling_window_when_economic_cache_is_off`
  (`views/context_bar.rs`),
  `refresh_context_used_does_not_copy_catalog_into_session_sampling`
  (`app/acp_handler/tests/session_events.rs`),
  `spawn_seeds_sampling_window_at_economic_cap_when_disk_economic_is_on`
  (`xai-grok-shell` `session/acp_session_impl/spawn.rs`).
- [x] **Parent ingest folds huge spawn prompts**: parent ingest folds
  spawn prompts over 40k into a pointer (description + size + report path
  if any). Live L2 execute still uses the full spawn prompt. Spawn
  **tool-call arguments** on the parent assistant item can still count
  until fold runs. Tests (filter `fold_spawn_prompt`):
  `huge_spawn_prompt_becomes_pointer_with_description_and_report`,
  `small_spawn_prompt_stays`, `read_file_args_are_not_folded`
  (`xai-grok-sampling-types` `fold_spawn_prompt_parent_ingest_tests`);
  `parent_estimated_tokens_omit_huge_spawn_prompt`
  (`xai-chat-state` `actor/tests.rs`).
- [x] **Parent ingest caps huge last answers**: parent ingest /
  completed-poll / blocking-spawn prompt format cap huge last answers
  (~40k) and point at an on-disk report if one exists. Stored child
  output can still be the full string. There is no automatic on-disk
  last-answer report. Tests:
  `to_model_text_caps_huge_last_answer_for_parent_ingest`
  (`xai-tool-types` `task.rs`),
  `completed_subagent_task_output_is_capped_or_points_at_report`
  (`xai-grok-tools` `task_output/mod.rs`),
  `blocking_spawn_subagent_completed_to_prompt_format_is_capped`
  (`xai-grok-tools` `task/mod.rs`).
- [x] **Auto-run `/implement`**: after a successful turn, queue a follow-up
  implement block when present; **appends** after any already-queued prompts.
  FORK claims; not a land class.
- [x] **Shared rate limits**: crate `grok-rate-limit` (Surmount name, not
  `xai-`); cooldowns under `~/.grok/rate_limits/`; optional
  `GROK_DISABLE_SHARED_RATE_LIMIT=1`. Path-restored (`FORK_PATHS`).
- [x] **Updates**: no xAI auto-update channel by default (wrong product).
  `grok-oss update --check` compares to Surmount `main`. Escape hatch:
  `GROK_OSS_ENABLE_XAI_UPDATER=1`. FORK claims; not a land class.
- [x] **`/rebuild` is SHA-aware peer relaunch**: local `just install`, not an
  xAI download. Verify package version plus git SHA. Same semver plus a
  different SHA is newer. Failed install must not replace the binary or
  `SIGUSR1` peers. Crate: `xai-grok-update` `rebuild.rs`;
  `xai-grok-shell` `leader/mod.rs`. Tests:
  `failed_install_must_not_replace_or_signal_peers`,
  `build_fail_does_not_signal_leaders`,
  `parse_version_output_extracts_identity`,
  `peer_relaunch_accepts_same_semver_different_sha`,
  `peer_relaunch_declines_equal_identity_on_same_path`,
  `peer_relaunch_accepts_deleted_inode_even_when_identity_equal`,
  `leader_is_older_than_same_semver_git_sha_identity`. Fail-does-not-signal
  alone is not this seam. TUI `/rebuild` is the wired operator path. CLI
  `grok-oss rebuild` is documented in user-guide and `xai-grok-update`
  docs; clap has no `Rebuild` variant; `run_rebuild_command` is unwired.
  Do not list a `Command::Rebuild` `fn`. Rebuild/reinstall by hand is the
  CLI path today.
- [x] **Running grok-oss sessions**: live TUI windows on this `$GROK_HOME`
  from `active_sessions.json`. Slash `/running` (alias `/windows`) and CLI
  `grok-oss running` / `grok-oss running --json`. Not Agent Dashboard, not
  `/sessions`, not `/tasks`, not `/resume`. Distinct from `/start` (that
  slash starts paused or interrupted work in this process). Identity is
  `(pid, session_id)` so two windows on the same conversation both appear.
  Missing heartbeat is activity `unknown`. Title is the on-disk session
  summary. Never stores prompts, tool arguments, tokens, JWTs, file
  contents, or message text. Default headless stays unlisted unless
  `GROK_TRACK_HEADLESS` is already set. Leader daemons stay on
  `grok-oss leader list`. `/rebuild` SIGUSR1 still dedupes by PID. Crates:
  `xai-grok-active-sessions`, `xai-grok-pager`, `xai-grok-pager-bin`,
  `xai-grok-update`. Tests: `list_live_includes_two_windows_on_the_same_session_id`,
  `list_live_drops_dead_pid`, `heartbeat_omits_prompt_text`,
  `running_slash_lists_sibling_fixture_row`,
  `running_cli_json_omits_prompt_text`,
  `rebuild_signals_each_pid_after_composite_key`. User-guide
  `04-slash-commands`, `17-sessions`, `23-dashboard` (cite only).
- [x] **`/start` starts paused or interrupted work**: pager builtin, not
  an alias of `/resume` (picker). Unpause if globally paused; else if a
  valid `canceled_turn_resume.json` exists, toast **Continuing interrupted
  turn...**, enqueue once, clear the marker, and drain. Soft-stop hold is
  released. An idle clean session does not invent a turn. Operator-typed
  `/start` applies even when `[ui] resume_canceled_turn_on_restart` is
  off. Files: `slash/commands/start.rs`, `app/dispatch/start.rs`. Tests:
  `start_while_globally_paused_continues_interrupted_turn_once`,
  `start_on_idle_clean_session_does_not_invent_a_turn`,
  `start_with_cancel_resume_marker_continues_interrupted_turn`.
- [x] **`from_config` no-prefetch usable catalog**:
  `ModelsManager::from_config` with no prefetch argument is a zero-network
  boot and must produce a usable bundled catalog. Test:
  `from_config_without_prefetch_produces_usable_catalog`
  (`xai-grok-shell` `agent/models/tests.rs`). **Empty `models_cache.json` is a
  miss in code (`load_fresh` returns `None` when `models` is empty). That
  empty-file branch has no named test. Do not claim it is cargo-proven.**
- [x] **Seeded custom model on `session/load` stays Chat Completions**:
  `session/load` keeps a seeded custom model id on Chat Completions instead
  of remapping it to the default grok-4.5 Responses catalog entry. grok-4.5
  itself still uses Responses. SuperGrok is paid. This is not last-session
  on start. Crate: `xai-grok-shell`. Tests:
  `keep_unverified_persisted_model_keeps_seeded_custom_slug`
  (`agent/models/tests.rs`),
  `seeded_test_model_keeps_chat_completions_backend`
  (`agent/mvp_agent/tests.rs`). Integration:
  `poisoned_image_session_recovers_within_the_failing_turn`
  (`--test test_image_strip_recovery`; in-turn strip after 400
  `invalid_image`).
- [x] **Nucleo reuse-per-root**: many workspace fuzzy searches without
  `close` keep one live matcher per root. Poll-only `get_results` must not
  refresh the stale timer. Crate: `xai-grok-workspace` `file_system/mod.rs`.
  Tests: `repeated_open_without_close_keeps_one_search_per_root`,
  `distinct_roots_each_keep_one_search`,
  `get_results_does_not_keep_a_stale_search_alive`. Per-matcher pool size
  `NUM_NUCLEO_THREADS = 2` is shipped in code (`xai-fuzzy-file-search`); no
  `fn` asserts `Some(2)`.
- [x] **Always-three-layer product prompt**: process law lives in
  [`AGENTS.md`](AGENTS.md) (D1; path-restored). Product also teaches it: L2
  task description must say three layers always and must spawn L3;
  default max depth must let depth-1 spawn L3. Tests:
  `child_task_description_is_concise` (`xai-grok-agent` `builder.rs`
  `CHILD_TASK_DESCRIPTION`),
  `default_max_allows_l2_to_spawn_l3` (`xai-grok-tools` `task/mod.rs`).
  User-guide `16-subagents.md` has the three-layer paragraph and names
  Hierarchical fast path (L1-only: one-command host question, one already
  named path, or the asked-for report). No dedicated cargo `fn` for that
  sentence. Do not put Hierarchical fast path into `CHILD_TASK_DESCRIPTION`.
  A restack can keep AGENTS via `FORK_PATHS` and still drop
  `CHILD_TASK_DESCRIPTION`.
- [x] **Soft interject only + Enter cue honesty**: mid-turn interject
  (Ctrl+Enter) injects into the current turn and **never cancels**. Cancel is
  Esc/stop only. Composer footer Enter cue (send / queue / interject) is
  shipped in code with no named footer `fn`. Proven never-cancel:
  `interject_contract_*`. User-guide `03-keyboard-shortcuts`,
  `16-subagents`.
- [x] **Todo board survives auto-compact**:
  `auto_compact_completed_preserves_todo_board`
  (`app/acp_handler/tests/subagents.rs`).
- [x] **plan.json honesty + resume board**: compact writes the live
  Resources `TodoState` to `plan.json`. FORK claims; not a land class.
  User-guide `17-sessions`.
- [x] **Auto-seed user asks as todos**: real user turns seed protected
  `ask:<prompt_id>`. FORK claims; helpers in `xai-grok-tools` todo module.
- [x] **Default agent uses the todo board**: base `prompt.md` teaches
  `todo_write`. FORK claims; not a land class.
- [x] **Same-batch plan write + `exit_plan_mode`**: mixed multi-tool batches
  run non-exit tools to completion first
  (`same_batch_plan_write_before_exit_plan_mode_returns_new_body`). Dated
  2026-08-09 wave filter; not one of the seven product land classes.
- [x] **Continue interrupted turn on restart**: `canceled_turn_resume.json`;
  distinct from last-session on start. Mid-turn `/rebuild` writes the
  marker. Idle completed turns do not write a marker and do not re-fire
  the last prompt. Load drops a leftover marker after a successful
  primary-turn finish. Tests:
  `handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn`,
  `handle_rebuild_done_idle_completed_turn_does_not_write_cancel_resume_or_refire_last_prompt`,
  `session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully`
  (`xai-grok-pager` `app/dispatch/rebuild.rs`). Still leftover (not
  shipped): auto-resume after an error-terminal turn with no marker;
  soft-stop button; mid-sample freeze without cancel. FORK claims plus
  these named tests; not a land class. User-guide `17-sessions`.
- [x] **OAuth 403 `bad-credentials` → auth path**: HTTP 403 with
  `unauthenticated:bad-credentials` classifies as auth, not included
  SuperGrok period limits. Dated 2026-08-09 wave filters on sampler types.
  Not a land class.
- [x] **Multi-track also-guard (first cut)**: `todo_write` accepts
  `meta.taskId`; demoting `in_progress` → `pending` is rejected while that
  subagent is still Running. FORK claims; not a land class.

### Chrome

Human chrome is **green** (`accent_user`: composer caret, human rails, OSC 12,
success). Agent activity is **magenta** (`accent_running` / `accent_model`:
active agent rails, tool spinner, lower-left still-running cue). Clear finished
is quiet secondary, not neon green and not magenta. Default theme is **DOGE**.
External role map:
[0001_DOGE.md](https://github.com/SurmountSystems/specs/blob/main/0001_DOGE.md).
User-guide [`06-theming`](crates/codegen/xai-grok-pager/docs/user-guide/06-theming.md).

- [x] **Unset theme is DOGE**: `xai-grok-pager-render` `theme/cache.rs`,
  `theme/system_appearance.rs`. Tests: `default_theme_is_doge`,
  `resolve_from_config_no_config_returns_doge`,
  `resolve_auto_dark_system_returns_doge`,
  `to_theme_kind_dark_defaults_to_doge`. This is **not** the models-catalog
  `from_config` empty-cache miss.
- [x] **DOGE human green / system cyan / role map**: `theme/doge.rs`. Tests:
  `doge_accent_user_is_pure_green_for_human`,
  `doge_accent_system_is_pure_cyan_for_system_limits_credits`,
  `doge_roles_green_cyan_no_blue_ui_no_gray_text`.
- [x] **Human left rail paints green**: `scrollback/blocks/user.rs`. Tests:
  `user_prompt_block_accent_is_static_human_rail`,
  `user_prompt_block_accent_is_green_rail_under_doge_default`,
  `user_prompt_entry_renderer_paints_green_rail`,
  `user_prompt_prefix_matches_human_rail_color`.
- [x] **Running agent rail paints magenta**:
  `agent_message_block_accent_is_magenta_rail_under_doge_while_running`
  (`scrollback/blocks/agent.rs`).
- [x] **Composer box caret is Human green, never agent magenta**:
  `views/prompt_widget/tests.rs`. Tests:
  `paint_composer_box_cursor_uses_human_green_not_agent_magenta`,
  `focused_composer_paints_human_green_box_caret_hides_terminal_cursor`.
- [x] **Model label uses `accent_model`**:
  `info_line_model_name_uses_accent_model_not_gray`.
- [x] **Titled composer frame is `prompt_border_active` (white); title only
  is yellow**:
  `titled_doge_composer_frame_is_prompt_border_not_context_yellow`.
- [x] **Compact included SuperGrok period limits meter**: status chip
  `included SuperGrok period limits · N%`; click opens `/limits`. Tests:
  `status_bar_pushes_credits_compact_included_supergrok_period_limits`,
  `hit_credits_click_dispatches_show_limits` (`app/agent_view/render.rs`).
- [x] **Plan footer CTAs**: idle footer is Approve / Comment / Revise / Exit
  (four CTAs). Clarify is only in the comment flow after Comment, not an
  idle top-level notes path. Notes is gone. Letter `a` / `A` type. Empty
  Enter never Approves. Revise arms the box and waits. The white plan
  prompt frame uses `theme.prompt_border_active`. Tests:
  `plan_approval_footer_paints_five_cta_vocabulary`,
  `plan_footer_exit_not_quit`, `plan_footer_has_no_notes_button`,
  `plan_prompt_letter_a_inserts_when_composing`
  (`views/file_search/line_viewer.rs`, `app/agent_view/plan.rs`).
- [x] **Plan present is not operator Approve + modal-free typing**:
  `exit_plan_mode` presents the plan. It does not click Approve.
  Always-approve permission mode does not auto-click the CTA. Empty Enter
  never Approves. Soft-park must not steal mid-compose keys. Crate:
  `xai-grok-pager` `app/agent_view/plan.rs`,
  `app/acp_handler/tests/plan_mode.rs`; `xai-grok-tools`
  `exit_plan_mode/mod.rs`. Tests:
  `exit_plan_mode_present_is_not_operator_approve`,
  `exit_plan_mode_tool_result_does_not_claim_operator_approval`,
  `empty_enter_on_revise_prompt_does_not_approve`,
  `soft_park_empty_ctrl_c_abandons_plan_approval`,
  `exit_plan_mode_keeps_mid_compose_draft_and_a_types`,
  `exit_plan_mode_modal_park_does_not_steal_mid_compose_keys`,
  `exit_plan_mode_empty_present_printable_goes_to_composer`,
  `exit_plan_mode_shows_overlay_even_in_yolo`. Settings park picker is class 2
  (`plan_approval_park_*`). Prefer these exact names over a vague
  `exit_plan_mode_soft` substring. User-guide `19-plan-mode`,
  `22-permissions-and-safety`.
- [x] **Soft plan present is a real right-side pane**: default soft park
  docks the existing plan list plus four idle CTAs (Approve / Comment /
  Revise / Exit) on the right, full overlay
  height, no dim of the transcript. Status **Plan ready. Side panel open**
  only when that viewer is actually open. A click on a plan row does not
  enter Commenting. `c` remains the explicit line-comment gesture. Tests:
  `plan_soft_park_docks_right_not_centered_overlay`,
  `plan_soft_park_draw_right_pane_matches_side_panel_status`,
  `plan_row_click_does_not_enter_commenting`,
  `plan_loop_status_does_not_claim_side_panel_when_viewer_closed`.
- [x] **Plan-review and Linux prompt screenshot paste**: `Event::Paste` and
  plan-review Ctrl+V run the clipboard image probe on every OS. Approve and
  Revise drain composer image chips. Tests:
  `event_paste_plan_commenting_empty_defers_clipboard_image_probe`,
  `plan_feedback_ctrl_v_defers_clipboard_image_probe`,
  `agent_empty_bracketed_paste_defers_probe_for_clipboard_image`,
  `approve_or_revise_drains_plan_composer_images`.
- [x] **No two live same-description Subagent rows**: product spawn
  **rejects** a second live Task-owned child with the same trimmed
  description on the same parent. It does not replace the first child.
  Unlimited retry paints `Retrying (1)`, never `Retrying (1/4294967295)`.
  Finite `Retrying (2/5)` stays. Token Economy implement-loop effort is
  thoroughness, not reviewer count (one reviewer unless the operator asked
  for more). Tests:
  `live_subagent_list_does_not_show_two_rows_with_the_same_description`,
  `task_spawn_rejects_or_replaces_second_live_same_description`,
  `format_activity_label_unlimited_retry_has_no_u32_max_fraction`,
  `implement_effort_two_does_not_spawn_two_review_rows_unless_operator_asked`.
- [x] **L1 Subagents list is L2-only plus a live L3 count**: the L1
  Subagents list, watching counts, and similar live chrome show only L2
  coordinators. Each L2 row may append a live L3 count (`1 specialist` /
  `N specialists`). L3 specialists do not get their own L1 rows or names.
  Opening an L2 still shows that L2's specialists inside the L2 view.
  Headless `ExtEvent::SubagentSpawned` is not the L1 list. Helpers:
  `live_subagent_list`, `is_l2_list_row`, `format_live_l3_count`
  (`xai-grok-pager` `app/subagent.rs`). Tests:
  `live_subagent_list_shows_only_l2_and_reports_live_l3_count`
  (`app/subagent.rs`),
  `l2_row_shows_live_l3_count_not_specialist_names`
  (`views/tasks_pane.rs`).
- [x] **Always-on bubble copy is paint plus click**: flag on paints `⧉`. A
  full-width first line still paints a hit. Click on the human glyph copies
  that prompt. Click on the assistant glyph copies that message. Paint-only
  bubble copy is a **failed land**. Tests:
  `bubble_copy_buttons_on_paints_copy_icon`,
  `bubble_copy_buttons_on_paints_copy_icon_when_first_line_is_full_width`
  (`scrollback/blocks/user.rs`);
  `append_bubble_copy_button_paints_when_first_line_fills_content_width`
  (`scrollback/blocks/mod.rs`);
  `clicking_human_bubble_copy_copies_the_prompt`,
  `clicking_assistant_bubble_copy_copies_the_message`,
  `clicking_wide_human_bubble_copy_still_paints_and_copies`
  (`app/mouse.rs`). Settings row: class 2 `bubble_copy_buttons_*`.
- [x] **Clear finished is quiet secondary**: compact `[−]` in the todo
  header when the board is open and finished rows exist. Never neon green or
  agent magenta. Hits must not open a subagent. Tests:
  `clear_finished_action_idle_is_quiet_not_neon_green_or_magenta`
  (`scrollback/selection.rs`);
  `clear_finished_only_when_open_with_finished_rows`,
  `clear_finished_hit_does_not_intersect_tasks_subagent_open_or_kill`,
  `clear_finished_click_does_not_open_subagent`,
  `clear_completed_todos_x_key_only_when_todo_pane_focused`. Slash
  `/clear-completed-todos` exists. The old pager `SHELL_RESERVED` /
  `shell_collision_contract_covers_every_pager_command_and_alias` `fn` is
  **gone**. Do not list that identifier as a land filter.
- [x] **Pause / resume / stop chips**: status `[pause]` / `[resume]`
  dispatch global pause, not cancel. `[stop]` is hard cancel only. Soft stop
  stays keyboard-only (`Ctrl+Shift+S`); no soft-stop button. Tests:
  `pause_button_click_dispatches_global_pause_not_cancel`
  (`app/agent_view/render.rs`);
  `work_control_chrome_matrix_pause_not_cancel_stop_not_pause`,
  `idle_with_subagents_paints_pause_and_stop_hits`,
  `global_paused_idle_paints_resume_not_stop` (`views/turn_status.rs`).
- [x] **Hide header zeros in-app chrome**: `[ui] hide_header` (default
  false) zeros the top agent status bar, welcome location top bar, and
  dashboard location header only. Not window titles. Tests:
  `hide_header_space_dispatches_typed_setter`,
  `hide_header_mouse_click_two_stage_toggles` (`settings_e2e.rs`);
  `hide_header_zeroes_status_bar_height`,
  `hide_header_zeros_welcome_top_bar_height`,
  `hide_header_zeroes_header_and_header_gap`. Serde-only
  `hide_header_defaults_false_and_parses` is **not** this class by itself.
- [x] **Window titles on by default**: product manages OSC titles when
  `[ui.notifications.title] enabled` (default true). Never emit an empty
  window-title OSC. Distinct from `hide_header`. Stale `[ui] hide_title_bar`
  is ignored (`stale_hide_title_bar_key_is_ignored`). Proven: 
  `window_title_always_manages_non_empty_branded_osc`,
  `titles_on_session_name_osc_is_non_empty_branded`,
  `window_title_osc_payload_never_empty_string`. Catalog names
  `default_title_items_include_agents`, `title_escape_never_empty_payload`,
  and `title_updates_gated_only_by_title_enabled` have **no matching `fn`**.
  Do not list those as land filters.
- [x] **Activity spinner is striped marquee, not braille**:
  `doge_activity_spinners_use_striped_down_marquee_not_braille`
  (`xai-grok-pager-render` `glyphs.rs`). Still-running cue:
  `idle_with_subagents_renders_still_running_cue` (`views/turn_status.rs`).
  Recap idle rail stays tool-white:
  `recap_accent_and_bullet_use_neutral_tool_color_when_idle`. **Do not
  claim** a dedicated lower-left throbber **color** `fn`
  (`doge_idle_subagent_still_running` and `doge_tool_running_spinner` are
  still absent).
- [x] **`/settings` unread restore set**: rows plus runtime readers for
  `hide_header`, `always_expand_thinking`, `scrub_ascii_punct`,
  `allow_worktree`, `bubble_copy_buttons`, `plan_approval_park`, and theme
  default **doge**. Tests: settings_e2e prefixes above;
  `theme_choices_include_doge_and_default_is_doge`;
  `always_expand_thinking_keeps_blocks_expanded`;
  `prime_applies_scrub_ascii_punct_from_ui`
  (`xai-grok-pager-render` `appearance/cache.rs`);
  `resolve_subagents_copies_allow_worktree` (`xai-grok-shell`; copy only,
  no named test that spawn isolation actually changes). Session recap and
  cancel-subagents Settings rows are **FORK claims, not re-proven** as
  `/settings` e2e filters on 2026-08-15.
- [x] **Stuck Retrying / StreamResumed (honesty)**: pager maps
  `RetryState::StreamResumed` in `session_notification.rs`. Shell emit
  exists: `stream_started_emits_retry_state_stream_resumed`. Sampler
  neighbors exist: `wait_before_attempt_aborts_on_cancel`,
  `retry_footer_reason_uses_short_transport_label`,
  `retry_footer_backoff_hint_appends_next_try_in`,
  `stream_headers_timeout_defaults_to_120_secs_when_env_unset`, plus
  `cargo test -p xai-grok-sampler --test stream_headers_timeout`. Catalog
  pager chrome names (`retry_chrome_soft_reconnects_when_retry_stream_starts`,
  `stream_resumed_without_prior_retry_clears_activity`, `clip_retry_reason_*`,
  `retrying_activity_label_*`, `retrying_label_shows_timeout_*`) have **no
  matching `fn`**. Do **not** claim stuck-retry pager chrome is fully proven.
- [x] **Click tasks chrome, Worked-for one live line, composer Ctrl+Home/End,
  rewind overlays, btw Done-panel, ASCII stream scrub, trailing-whitespace
  strip**: shipped product behavior (FORK claims / residual-aligned). Not
  seven-class land filters unless a named `fn` is enrolled later.

### Packaging and build

- [x] **AUR** sources under `packaging/aur/`
- [x] **Nix flake**: `nix build .#grok-oss`, dev shells (human packaging, not
  GHA release artifacts). `flake.nix` is in `FORK_PATHS`.
- [x] **Rust 1.97.1 (file pin only; not cargo-proven)**: project
  `rust-toolchain.toml` channel `1.97.1` plus matching fenix FOD in
  `flake.nix`. After an upstream export that still lists 1.94.x, keep
  Surmount **1.97.1** unless the operator chooses another channel.
  **`rust-toolchain.toml` is not in `FORK_PATHS`.** Import can keep the flake
  and take upstream's toolchain file. There is no cargo `fn` that asserts
  channel `1.97.1`. Do not add rustc 1.97.1 as a cargo land class until a
  named test or assert sniff exists. Report:
  [`.agents/reports/impl-toolchain-1971-2026-08-12.md`](.agents/reports/impl-toolchain-1971-2026-08-12.md)
- [x] **justfile**: `just check` / `just ci` full quality gate; `just test`
  for the cargo quality suite
- [x] **release-dist debug sidecar**: `just build-dist` / `just install-dist`
  build with `--profile release-dist` (strip=false, debug=1), extract DWARF to
  `grok-oss.debug` via `scripts/extract-debug-sidecar.sh`, strip the binary,
  embed GNU debuglink. Plain `just install` stays local `--release` + strip
  (no sidecar).

### Process

- [x] **Process docs hierarchy**: D0 residual open-only; D1 AGENTS; D2 logs
  under `docs/upstream-*` and `doc/dev/campaigns`; D3 research / skill
  `references/`
- [x] **Upstream tooling**: detect / import / put-history /
  **join-main-into-onto** / sync scripts; scheduled export watch workflow
- [x] **Onto land path**: after product is on their tip, join Surmount
  `main` with `merge -s ours` so the tip is PR-able
  (`docs/upstream-history.md`, `just upstream-join-main`)
- [x] **PRs accepted**: CONTRIBUTING / this fork
- [x] **Parent = HITL only; always three layers (2026-08-15) plus
  Hierarchical fast path (2026-08-16)**: process pin in [`AGENTS.md`](AGENTS.md)
  and host `~/.grok/AGENTS.md`. Whenever implement work, multi-file diagnosis,
  CI, or a regression needs tools, agents are three layers deep. Including
  implement loops. **Hierarchical fast path** (named): the main thread may do
  a one-command host question, a single known-path read already named, or
  read and quote the short on-disk report this thread asked for. That is not
  a license to diagnose or implement in the main thread. **Mention is in
  scope:** if the operator mentions work, that mention is in scope. **L1
  main:** status, spawn L2, wait, read short reports, board upsert,
  Hierarchical fast path. **L2:** parallelize, spawn L3s, throw context away
  after a report. **L3:** all actual tools and work. No L4. The older weaker
  law (L2 must spawn L3 only when many greps / half the window) is replaced.
  Product cargo pins for the prompt contract are under Product
  (`CHILD_TASK_DESCRIPTION`). Assert sniffs that AGENTS still contains the
  coordinator sentence; that is not the crate seam. Write new short reports
  under `~/.agents/reports/` on this machine. Do not add report files to the
  git tree. Historical `.agents/reports/foo.md` citations in this file are
  finished-note names only. Product `grok-oss limits multipoll` default out
  dir is `~/.agents/reports/limits-multipoll-<utc>/` (temp fallback if
  HOME is empty). Shipped in `default_multipoll_out_dir`. No named `fn`.
  Do not claim repo `.agents/reports/` is the live home. Fold helper
  `first_report_path` matches any `.agents/reports/` substring (home or
  leftover repo path). That is implementation, not a land class.
- [x] **Subagent worktree policy**: prefer isolation none; product default
  `[subagents] allow_worktree = false`. Class 2 copies the flag:
  `resolve_subagents_copies_allow_worktree`. User-guide `05-configuration` +
  `16-subagents`. Campaign:
  `doc/dev/campaigns/operator-orchestration-2026-07.md`
- [x] **`/execute-plan` honors `allow_worktree`**: host skill defaults to
  shared-cwd protocol. Report:
  `doc/dev/research/execute-plan-no-worktree-2026-07-24.md`
- [x] **Todo levels, fib leaves, cleared archive, session notes**: product
  `todo_write` surface (`priority`, `meta`, protected prefixes, fib `size`
  1|2, `cleared_todos`, `/note`). Not land classes. Reports under
  `doc/dev/research/todo-*.md` and `notes-channel-2026-07-24.md`.
- [x] **Git recon depth**: host skill `/git-recon`; product
  `scripts/recon-status.sh` + `just recon-status` (read-only probe); pin in
  `FORK_PATHS` + `assert-process-pins`.
- [x] **Prefer Rust tools; product skills are not a Python runtime**:
  standing preference plus land class 7. Sanitize rejects junk `.py`; archive
  extract skips junk `.py`; product skill roots have no junk `.py`. The three
  allowlisted CLI stubs are Rust intercepts (`memory.py`, `validate-plan.py`,
  `session_reader.py`). Exceptions: those stubs plus office/docx/pptx/xlsx/pdf
  scripts. Host `~/.agents/skills` is operator-owned and is **not** this
  class. Tests: `sanitize_rejects_non_excepted_skill_python`,
  `extract_archive_skips_non_excepted_skill_python`,
  `product_repo_skill_roots_have_no_non_excepted_python`
  (`xai-grok-bundle` `lib.rs`);
  `user_guide_skills_are_not_a_python_runtime` (`xai-grok-pager` `docs.rs`);
  `implement_memory_snapshot_intercept_does_not_spawn_shell`,
  `plan_validate_intercept_does_not_spawn_shell`,
  `session_reader_list_intercept_does_not_spawn_shell`
  (`xai-grok-tools` `bash/mod.rs`). A restack that reintroduces non-excepted
  Python, or that drops a Rust intercept, is a **failed land**. Research:
  [`doc/dev/research/python-to-rust-tools-2026-07-26.md`](doc/dev/research/python-to-rust-tools-2026-07-26.md)
- [x] **File-level infer-from-path verify** (ACP `search_replace` /
  `apply_patch` and the other structured edit tools): a written `.rs`
  file is formatted and linted as that file. Not `cargo clippy -p
  <crate> --lib`, not `cargo fmt -p`, not `just check`. Other extensions
  do not get Rust cargo. Kill switch: `GROK_SKIP_EDIT_VERIFY=1`. Helper:
  `xai-grok-tools` `util/rust_edit_verify.rs`. Named tests below. A
  restack that drops the helper or those tests is a **failed land**.
- [x] **ACP per-path write lock** (`search_replace`, `apply_patch`,
  `write`, OpenCode `edit`, `hashline_edit`): each tool takes the path
  automatically as part of the call. Happy path is silent. A held path
  is a tool error that names the holder and the file. The tool does
  not write, wait, or show a human steal, skip, or wait menu. Lock
  releases when the call finishes. File-level infer-from-path verify
  still runs under the same hold. Helper: `xai-grok-tools`
  `implementations/editor_infra/per_path_write_lock.rs`. Named tests
  below. A restack that drops the helper, the OpenCode `edit` lock
  acquire, the `hashline_edit` lock acquire, or those tests is a
  **failed land**.

### File-level infer-from-path verify

After ACP `search_replace` / `apply_patch` (and the other structured
edit tools), the **edit tool** infers from the path. A `.rs` file is
formatted and linted **as that file**. The format and lint argv must
include the written path. That is not crate or project cargo, not
`just check`, and not an AGENTS process slogan. Markdown, toml, and
other non-`.rs` paths stay quiet. The command-running tool still
**rejects** crate-wide cargo launches (`cargo fmt --all`, `cargo fmt
-p` without a file list, `cargo clippy -p ... --all-targets`,
`--workspace`). Kill switch if already in the plan:
`GROK_SKIP_EDIT_VERIFY=1`.

This is product behavior. Process law (do not prove the slice by
spawning crate-wide cargo through extra subagents) lives in
[`AGENTS.md`](AGENTS.md) hard constraint 3b.

**Named tests** (fixture and argv only; they must not clippy this
workspace). Module filter `rust_edit_verify` matches these `fn`s:

- `rustfmt_argv_edition_2024_config_and_absolute_files`
- `clippy_argv_lints_the_edited_file_not_crate_lib`
- `clippy_argv_includes_bin_path_not_package_lib`
- `clippy_argv_includes_integration_test_path_not_package_lib`
- `clippy_argv_is_file_level_not_package_lib`
- `several_rust_writes_run_file_level_clippy_per_file`

Command-tool reject (same crate, `dangerous_cargo` filter):

- `dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_fmt_package_without_file_list_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_clippy_all_targets_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_clippy_workspace_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_test_workspace_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_nextest_run_without_package_or_filter_is_refused_and_does_not_spawn_shell`
- `dangerous_cargo_test_package_lib_filter_is_not_refused`

Catalog:
[`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md)
§ *File-level infer-from-path verify*. Extra restack-droppable class, not
one of the seven numbered land classes.

### ACP per-path write lock

ACP `search_replace`, `apply_patch`, `write`, OpenCode `edit`, and
`hashline_edit` (`GrokBuildHashline:hashline_edit`) take a per-path
write lock automatically as part of the tool call. There is no lock
argument on the tool schema. A successful write does not mention the
lock.

When another agent already holds that path, the tool returns an error
that names the holder and the file. It does not write. It does not
overwrite silently. It does not wait inside the tool. It does not show
a human steal, skip, or wait menu. Agents resolve the conflict by
talking to each other: they can wait, hand off, or pick another path.

The lock is held through file-level infer-from-path verify on a written
`.rs` file. `GROK_SKIP_EDIT_VERIFY=1` still skips only that verify.

OpenCode `edit` (tool id `"edit"`) acquires after directory, same-string,
and bulk-edit checks, and before create or replace. The guard stays in
`run` so rustfmt and clippy-driver on the same `.rs` path stay under
the hold.

`hashline_edit` acquires on the joined path after `resolve_model_path`,
before canonicalize or any write. Existing-file edits and new-file
`Write` both take the lock. Same helper; no second table; no human menu.

**Named tests** (module filter `per_path_write_lock`):

- `two_agents_cannot_write_the_same_path_at_once`
- `happy_path_first_writer_succeeds_silently`
- `lock_releases_after_the_tool_call_so_a_later_call_can_write`
- `search_replace_apply_patch_and_write_all_take_the_lock`
- `held_path_error_names_holder_and_file_without_a_steal_skip_wait_menu`
- `hashline_edit_refuses_when_another_agent_holds_the_path`
- `hashline_edit_happy_path_does_not_mention_the_lock`

```bash
cargo test -p xai-grok-tools --lib per_path_write_lock
```

OpenCode `edit` fixture (not under that module filter):

- `opencode_edit_cannot_write_a_path_another_agent_already_holds`

```bash
cargo test -p xai-grok-tools --lib opencode_edit_cannot_write_a_path_another_agent_already_holds
```

### Skills (multi-source)

Skills are loaded from several places; the product on this branch owns the
machinery. Full map: `doc/dev/research/where-skills-come-from-2026-07-24.md`,
user-guide [`08-skills.md`](crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md).

| Source | Role |
|--------|------|
| Project `.agents/skills`, `.grok/skills` | Git-trackable on the branch (supported; may be empty) |
| `~/.agents/skills` then `~/.grok/skills` | Host operator overlay (agents wins) |
| `[skills].paths` / server inject / plugins | Config and managed dirs |
| `~/.grok/bundled/skills` | Platform cache from network bundle sync |

**Process pins that must survive recon** (import / onto): document in **FORK +
AGENTS + product user-guide** when product-facing; **dual-pin** host skills
(`~/.agents`) when operator-only. Host skill git alone does not ride product
history. Chat-only pins die at compaction.

### User-guide fork pins (one line + link)

The shared guide under `crates/codegen/xai-grok-pager/docs/user-guide/` is
**not** in `FORK_PATHS`. Onto takes the xAI guide unless conflict resolve
keeps Surmount pages. Do not paste those pages here.

| Page | Fork pin | Cargo pin |
|------|----------|-----------|
| [`01-getting-started`](crates/codegen/xai-grok-pager/docs/user-guide/01-getting-started.md) | Binary is `grok-oss`. Bare interactive open is last session for this cwd, not Welcome. | Last-session sentences shipped in code; no dedicated `fn`. |
| [`02-authentication`](crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md) | SuperGrok is paid. Distinct meters. `/limits` and compact chip. Hop after included SuperGrok period limits are full. | `user_guide_does_not_claim_automatic_host_hop_is_unshipped`. Zero `/limits` hits is a failed land in catalog prose; no cargo hit-count `fn`. |
| [`03-keyboard-shortcuts`](crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md) | Plan keys and Enter cue (send / queue / interject). Empty Enter never approves a plan. | Plan honesty `fn`s under Chrome. |
| [`04-slash-commands`](crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md) | `/running` (alias `/windows`) lists live grok-oss TUI windows. Not Agent Dashboard. `/start` starts paused or interrupted work in this process; not `/resume`. | `running_slash_lists_sibling_fixture_row`; `/start` cite `start_*` tests. No `user_guide_*start*` `fn`. Guide still documents `grok-oss rebuild`; that page is not cargo-proven for CLI rebuild. |
| [`05-configuration`](crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md) | `hide_header` is in-app only. Titles use `title.enabled`. `[subagents] allow_worktree` defaults false. | Class 2 readers. **Do not claim** Token Economy `/settings` table rows as proven. |
| [`06-theming`](crates/codegen/xai-grok-pager/docs/user-guide/06-theming.md) | Default theme is DOGE. Human green / agent magenta roles. | Class 4 theme + rail `fn`s. |
| [`08-skills`](crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md) | Product skills are not a Python runtime (allowlisted CLI stubs + office/docx/pptx/xlsx/pdf only). | `user_guide_skills_are_not_a_python_runtime` |
| [`16-subagents`](crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md) | Worktree isolation off by default. Soft interject never cancels. Three-layer paragraph. Hierarchical fast path (L1-only). L1 Subagents list is L2-only plus a live L3 count. New reports under `~/.agents/reports/`. | Three-layer / fast-path / L2-only guide text shipped in code; no dedicated user-guide `fn`. Cargo: `child_task_description_is_concise`, `live_subagent_list_shows_only_l2_and_reports_live_l3_count`. |
| [`17-sessions`](crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md) | Last-session on start vs `-c` / `--resume` vs `/start` vs leftover `canceled_turn_resume.json` drop after a successful primary-turn finish. Running grok-oss sessions vs disk `grok-oss sessions`. Resume examples use `grok-oss`. | `user_guide_resume_and_version_examples_use_grok_oss`; `/start` + marker-drop cite `start_*` and `session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully`. |
| [`19-plan-mode`](crates/codegen/xai-grok-pager/docs/user-guide/19-plan-mode.md) | Present is not Approve. Idle footer is Approve / Comment / Revise / Exit. Clarify only after Comment. Empty Enter never approves. Freeform questions, not the questionnaire modal. | Extra class B `fn`s. Keep identifier `plan_approval_footer_paints_five_cta_vocabulary`. |
| [`22-permissions-and-safety`](crates/codegen/xai-grok-pager/docs/user-guide/22-permissions-and-safety.md) | Always-approve is tool permissions only, not plan Approve. | `exit_plan_mode_shows_overlay_even_in_yolo` |
| [`23-dashboard`](crates/codegen/xai-grok-pager/docs/user-guide/23-dashboard.md) | Agent Dashboard is this pager. Running grok-oss sessions must not merge into `/dashboard`. | Cite only. No dedicated user-guide `fn`. |
| [`24-monitoring-usage`](crates/codegen/xai-grok-pager/docs/user-guide/24-monitoring-usage.md) | `/spend` ledger vs org metrics. Do not mash meters. | `user_guide_names_token_economy_spend_order` |

Also: `user_guide_operator_cli_examples_use_grok_oss` (leftover `grok login` /
`grok sessions` must not return).

### Dogfood snapshot (2026-08-09)

This section is a **dated operator handoff from 2026-08-09**. It does **not**
demote later shipped claims in Product, Chrome, or Land above. Source restore
is not live TUI dogfood. This file does **not** claim a rebuilt interactive
`grok-oss`. Do not start rebuild or quit from this inventory.

**Read first (next task, recon, or dogfood):**

1. **Operator install gate**: code can be green while old TUIs still run a
   deleted-inode binary. Checklist:
   [`.agents/reports/d0-dogfood-checklist-2026-08-09.md`](.agents/reports/d0-dogfood-checklist-2026-08-09.md).
   Package status for that wave:
   [`.agents/reports/impl-remaining-plan-wave-2026-08-09.md`](.agents/reports/impl-remaining-plan-wave-2026-08-09.md).
2. **Shipped in that wave (tree + reports; dogfood only after a later
   install the operator chooses)**: decisive plan Revise; same-batch
   `plan.md` write + `exit_plan_mode`; plan decision surface (empty Enter
   never approves; **Plan ready. Side panel open**); revise/clarify in-flight
   status; caret empty half `text_primary`; OAuth 403 bad-credentials path;
   rewind missing intermediate checkpoints; Ctrl+C dismisses rewind; status
   `[pause]`/`[resume]` + red `[stop]`; soft stop chord-only; composer Enter
   `send`/`queue`/`interject` cue; compact included SuperGrok period limits
   meter. Later Product / Chrome bullets and named `fn`s supersede this
   snapshot when they disagree. Later Product bullets for `/start` and
   leftover `canceled_turn_resume.json` drop supersede this snapshot. Do
   not revive idle Clarify as current leftover.
3. **Still not shipped (honesty leftovers, not a demotion of shipped chrome)**
   - **Auto-resume after error terminal on rebuild/reopen**: expected
     operator contract if the last terminal was an error (not only the
     cancel-resume marker). Document as shipped only when
     `.agents/reports/impl-rebuild-auto-resume-after-error-2026-08-09.md`
     (or equivalent) is green in tree. Distinct from continue interrupted
     turn (`canceled_turn_resume.json`). After any auto-resume, 403
     bad-credentials may still need `/login`.
   - **Soft-stop button**: not shipped; soft stop stays `Ctrl+Shift+S` only.
   - **Mid-sample freeze without cancel**: not shipped (global pause
     cancels turns; soft stop only stops queue drain after the current
     turn). Do not invent a media-player freeze metaphor.
   - **CLI `grok-oss rebuild`**: not clap-wired. TUI `/rebuild` is the
     wired path. Rebuild/reinstall by hand is the CLI path today.
   - **`/economic-mode` slash**: pager queues the text only. No
     BuiltinAction. Economic cap at spawn / model switch / header is
     shipped.
   - **SuperGrok Heavy ranking optional label**: not implemented. SuperGrok
     Heavy is a real distinct weekly pool. This file does not diagnose
     product usage of that pool.
4. **Still open (residual, not this install alone)**
   - Included SuperGrok period flat % / server C4 debit: paste-ready ticket
     [`.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md`](.agents/reports/c4-xai-ticket-paste-ready-2026-08-07.md);
     never invent included SuperGrok period used % on the client.
   - Thoughtful todo tracking process (session board hygiene; **not**
     file-level edit verify):
     [`RESIDUAL.md`](RESIDUAL.md) Open.
5. **Useful regression filters from that wave only** (not a substitute for
   the seven-class land list):

```bash
cargo test -p xai-grok-pager --lib -- after_revise re_present_after_revise \
  paint_composer_box_cursor_grapheme_phases_keep_letter \
  left_through_letters_empty_phase_not_neon
cargo test -p xai-grok-shell --lib -- same_batch_plan_write_before_exit_plan_mode_returns_new_body \
  replay_skips_missing_intermediate_checkpoint
cargo test -p xai-grok-sampling-types --lib -- forbidden_bad_credentials
cargo test -p xai-grok-sampler --lib -- api_403_bad_credentials classify_forbidden
cargo test -p xai-grok-pager --lib -- work_control_chrome_matrix_pause_not_cancel_stop_not_pause \
  pause_button_click_dispatches_global_pause_not_cancel

# File-level infer-from-path verify (2026-08-15; extra class, not that wave)
cargo test -p xai-grok-tools --lib rust_edit_verify
cargo test -p xai-grok-tools --lib dangerous_cargo
```

Process law (plain English, no bad metaphors): host + project `AGENTS.md`
§ Prose + tone / hard constraint 4. Not re-dumped here.

### What recon keeps / clobbers

| Path | Import | Put-history | Join (`-s ours`) |
|------|--------|-------------|------------------|
| Paths in `FORK_PATHS` (AGENTS, RESIDUAL, FORK, `docs/upstream-*`, join/hermetic/assert/`recon-status` scripts, `.grok/workflows`, `doc/dev`, `flake.nix`, ...) | **Restored** from base; post-restore `assert-process-pins` | Via cherry-picks | Tip tree kept |
| Product commits after seed | N/A (tree = xAI + restore) | Cherry-picked onto tip | Tip tree kept |
| Paths **not** in `FORK_PATHS` and absent from xAI | **Dropped** | Only if stacked | Cannot backfill missing |
| Shared user-guide / crate seams | xAI base | Conflict resolve | Tip tree only |
| Host `~/.agents/skills`, `~/.grok/AGENTS.md` | Untouched | Untouched | Untouched |
| `rust-toolchain.toml` | **Not** in `FORK_PATHS`; import can take upstream's file | Only if stacked | Tip tree only |

Assert: `./scripts/assert-process-pins.sh` or `just upstream-assert-process-pins`.
That command proves **files exist**. It does **not** prove product contracts
inside `xai-grok-*`. Detail: `doc/dev/research/fork-paths-hardening-2026-07-24.md`,
`doc/dev/research/skills-survive-upstream-recon-2026-07-24.md`,
[`docs/upstream-history.md`](docs/upstream-history.md).

Novel Surmount crates use the **`grok-*`** prefix (example: `grok-rate-limit`).
Upstream crate paths stay **`xai-grok-*`** for mergeability.

## How to defend a seam on the next merge

A restack-droppable seam is not defended by a FORK checkbox. Defense is four
things that stay aligned:

1. **FORK line**: one complete sentence in this inventory with the named
   `fn` and crate (or an explicit “shipped in code, no named test” /
   “file pin only” label).
2. **Named cargo test**: a `fn` that goes red if the seam is deleted.
3. **Catalog row**: enroll the filter in
   [`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md).
   Recon agents walk these classes. The process improver owns that catalog
   and the assert / git-recon wiring.
4. **Cherry-pick**: product seams inside `xai-grok-*` survive onto only via
   cherry-pick plus those tests.

**Import restores docs and scripts only** (`FORK_PATHS`). It does not restore
crate tests. `scripts/assert-process-pins.sh` proves files exist. It does not
prove contracts. **`just check` cannot fail a deleted catalog test.** A
chrome-only inventory is a failed land. Paint-only bubble copy is a failed
land. Reintroducing non-excepted Python under product skills is a failed land.

## Land checklist (after put-history / onto / import / join)

Do not claim "Surmount seams survived" until this list is done. `just check`
is quality only. It cannot fail a deleted catalog test.

**Rules (not product class numbers):**

- **`FORK_PATHS` restore is docs and scripts only.** Product seams inside
  `xai-grok-*` survive onto only via cherry-pick plus cargo tests.
  `scripts/assert-process-pins.sh` proves files exist. It does not prove
  contracts.
- **A chrome-only inventory is a failed land.** Paint screenshots of rails
  and four idle plan CTAs do not prove hop keys, `/spend` ingest, unread config,
  first-token `grok-oss`, last-session, or skills-not-Python.
- **Paint-only bubble copy is a failed land.** Click-to-copy tests must
  still exist.
- **Product skills reintroduced as a Python runtime is a failed land.**
- **Catalog hook (process improver):** recon agents walk these classes in
  [`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md).
  A sibling name-existence check (not this file, not `REQUIRED_FILES`) may
  later fail if a required-land identifier has no matching `fn`. Path assert
  stays files-only.

**Steps** (procedure; do not mix these into the 1-7 product count):

1. Run `just upstream-assert-process-pins` (or
   `./scripts/assert-process-pins.sh HEAD`). Files and light sniffs only.
2. Run the named cargo filters for the **seven product classes** below, plus
   the extra proven restack-droppable classes. Use existing test names. Do
   not invent a filter that is not in the tree. Do not list an identifier
   that has no matching `fn`.
3. `rg` each required identifier for a matching `fn`. A named filter with no
   matching `fn` is a failed land.
4. **Helper-green is a failed land.** Forbidden as proof: a `--version` test
   that only checks stdout contains the substring `grok`; catalog-exists
   without paint; schema-exists without `/spend` ingest; serde `hide_header`
   without a `/settings` row and a runtime reader; rank helpers without
   `sampling_config` hop keys; bundle still has `memory.py`.
5. Dogfood screenshots (rails, four idle plan CTAs, compact included SuperGrok period
   limits meter, SIGUSR1 after a failed install) stay an operator check
   after those `fn`s exist. They are not the only check. This inventory does
   not claim live TUI dogfood.

**Seven product classes** (must match the catalog; each proven by a named
cargo `fn`):

1. **CLI identity.** The product command is **grok-oss**. `grok-oss --version`
   first token is `grok-oss`, not bare `grok`. Resume and relaunch hints are
   `grok-oss --resume`. Welcome / tutorial badges say Grok OSS.
2. **Config is a surface, not a field.** A toml field that deserializes is
   not shipped if `/settings` has no row and no runtime reader. Restack lost
   unread keys (`hide_header`, always-expand thinking, plan park, worktrees,
   ASCII scrub at launch, bubble copy) and leftover `/settings` rows plus
   DOGE in the theme picker.
3. **Token Economy ledger `/spend` (extra SQL, not SuperGrok dollar credits).**
   `$GROK_HOME/grok_oss.db` is the Token Economy ledger, not the session
   store. Schema v1 surviving is not enough. `/spend` must ingest
   `usage.jsonl` and write `reconciliation_run` (not
   `DoubleEntryReport::default()`).
4. **DOGE / Surmount chrome.** A theme file existing is not paint. Land must
   keep paint/render tests for human green rails plus box caret, magenta
   model / running agent, the compact **included SuperGrok period limits**
   meter, the titled composer frame (`prompt_border_active` white, yellow
   title only), and the four-CTA idle plan panel (Clarify only after Comment).
5. **Dual-auth hop after included SuperGrok period limits are full.** Rank
   helpers are not hop. `sampling_config` must fill console failover after
   those included limits are full, and must omit it while they still have
   room. Includes Business / Team included before personal, any stored
   SuperGrok login with included remaining before SuperGrok dollar credits,
   sibling included before SuperGrok dollar credits, and the one-process
   limits flock. Do not flatten remaining to zero from usage percent 100
   plus missing SuperGrok Heavy. Never invent used-up included SuperGrok
   period limits. SuperGrok Heavy ranking optional label is not this class.
6. **Last-session on start.** Interactive `grok-oss` opens the remembered
   last session for this working directory. It does not land on Welcome
   first.
7. **Product skills are not a Python runtime.** A restack that installs
   non-excepted Python under product skills, or that drops the Rust intercept
   for `memory.py` / `validate-plan.py` / `session_reader.py`, is a failed
   land. Office/docx/pptx/xlsx/pdf scripts and those three allowlisted CLI
   stubs are the only exceptions. User-guide `08-skills.md` must keep that
   sentence.

After restack the required classes are **all seven** above: CLI branding,
`/settings` plus unread config, Token Economy ledger `/spend`, DOGE/chrome
paint, dual-auth hop after included SuperGrok period limits are full,
last-session on start, **and** product skills are not a Python runtime.

**Extra proven restack-droppable classes** (named cargo tests exist; a land
that drops them while keeping the seven is still a seam loss):

- Always-on bubble copy **click + wrap** (paint-only is a failed land).
- Plan present ≠ Approve + modal-free typing (four-CTA idle paint is not honesty).
- `/rebuild` SHA-aware peer relaunch (fail-does-not-signal is not enough).
- Nucleo reuse-per-root.
- Baked default is Grok 4.6 at medium reasoning effort
  (`baked_default_is_grok_46_medium_fork_contract`). Fork contract change.
- Soft plan present is a real right-side pane (not a 75% centered overlay).
  Named tests: `plan_soft_park_docks_right_not_centered_overlay`,
  `plan_soft_park_draw_right_pane_matches_side_panel_status`,
  `plan_row_click_does_not_enter_commenting`,
  `plan_loop_status_does_not_claim_side_panel_when_viewer_closed`.
- Plan-review and Linux prompt screenshot paste
  (`event_paste_plan_commenting_empty_defers_clipboard_image_probe`,
  `plan_feedback_ctrl_v_defers_clipboard_image_probe`,
  `agent_empty_bracketed_paste_defers_probe_for_clipboard_image`,
  `approve_or_revise_drains_plan_composer_images`).
- Live chrome names SuperGrok dollar credits, not a nickname
  (`compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct`,
  `format_supergrok_session_with_weekly_and_extras`).
- No two live same-description Subagent rows; unlimited retry is not a
  `u32::MAX` fraction
  (`live_subagent_list_does_not_show_two_rows_with_the_same_description`,
  `task_spawn_rejects_or_replaces_second_live_same_description`,
  `format_activity_label_unlimited_retry_has_no_u32_max_fraction`,
  `implement_effort_two_does_not_spawn_two_review_rows_unless_operator_asked`).
- `from_config` no-prefetch usable catalog
  (`from_config_without_prefetch_produces_usable_catalog`). Empty
  `models_cache.json` miss is **not** cargo-proven.
- Seeded custom model on `session/load` stays Chat Completions
  (`keep_unverified_persisted_model_keeps_seeded_custom_slug`,
  `seeded_test_model_keeps_chat_completions_backend`,
  `poisoned_image_session_recovers_within_the_failing_turn`).
- Always-three-layer product prompt (`child_task_description_is_concise`,
  `default_max_allows_l2_to_spawn_l3`).
- File-level infer-from-path verify after ACP structured edits
  (`rustfmt_argv_edition_2024_config_and_absolute_files`,
  `clippy_argv_lints_the_edited_file_not_crate_lib`,
  `clippy_argv_is_file_level_not_package_lib`,
  `dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell`).
  A restack that drops `util/rust_edit_verify.rs` or these tests is a
  failed land. Not one of the seven numbered classes.
- ACP per-path write lock after structured edits
  (`two_agents_cannot_write_the_same_path_at_once`,
  `search_replace_apply_patch_and_write_all_take_the_lock`,
  `held_path_error_names_holder_and_file_without_a_steal_skip_wait_menu`,
  `opencode_edit_cannot_write_a_path_another_agent_already_holds`,
  `hashline_edit_refuses_when_another_agent_holds_the_path`,
  `hashline_edit_happy_path_does_not_mention_the_lock`).
  A restack that drops `per_path_write_lock.rs`, the OpenCode `edit`
  lock acquire, the `hashline_edit` lock acquire, or these tests is a
  failed land. Not one of the seven numbered classes.
- Pause / resume chips and Clear finished quiet paint.
- User-guide cargo pins beyond skills + resume (`user_guide_operator_cli_examples_use_grok_oss`,
  `user_guide_does_not_claim_automatic_host_hop_is_unshipped`,
  `user_guide_names_token_economy_spend_order`).
- L1 Subagents list is L2-only plus a live L3 count
  (`live_subagent_list_shows_only_l2_and_reports_live_l3_count`,
  `l2_row_shows_live_l3_count_not_specialist_names`).
- `/start` plus leftover cancel-resume marker drop
  (`start_while_globally_paused_continues_interrupted_turn_once`,
  `start_on_idle_clean_session_does_not_invent_a_turn`,
  `start_with_cancel_resume_marker_continues_interrupted_turn`,
  `handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn`,
  `handle_rebuild_done_idle_completed_turn_does_not_write_cancel_resume_or_refire_last_prompt`,
  `session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully`).
- ForceRefresh on explicit `/limits`
  (`management_meter_cache_policy_collect_force_background_honor_ttl`,
  `should_clear_management_meter_caches_force_with_key_only`,
  `limits_snapshot_mode_for_get_billing_explicit_is_force_refresh`).
- Footer / session sampling window vs catalog
  (`context_chip_names_sampling_window_when_catalog_differs`,
  `context_chip_hover_percent_uses_sampling_window_when_catalog_differs`,
  `footer_chip_uses_session_sampling_window_when_economic_cache_is_off`,
  `refresh_context_used_does_not_copy_catalog_into_session_sampling`,
  `spawn_seeds_sampling_window_at_economic_cap_when_disk_economic_is_on`).
- Spawn-prompt fold plus last-answer caps
  (`huge_spawn_prompt_becomes_pointer_with_description_and_report`,
  `parent_estimated_tokens_omit_huge_spawn_prompt`,
  `to_model_text_caps_huge_last_answer_for_parent_ingest`,
  `completed_subagent_task_output_is_capped_or_points_at_report`,
  `blocking_spawn_subagent_completed_to_prompt_format_is_capped`).
- Any stored SuperGrok included remaining before SuperGrok dollar credits;
  do not flatten remaining from usage percent 100 plus missing SuperGrok
  Heavy (`sampling_config_hop_team_remaining_personal_exhausted_not_dollars_or_console`,
  `sampling_config_hop_personal_remaining_team_exhausted`,
  `sampling_config_hop_both_remaining_team_first_then_personal`,
  `sampling_config_hop_both_included_exhausted_dollar_credits_before_console`,
  `sampling_config_hop_missing_heavy_false_100_keeps_sibling_included`,
  `sampling_config_hop_dollar_credits_on_both_missing_heavy_keeps_team`,
  `prepare_sampler_for_turn_does_not_flatten_missing_heavy_100_off_sibling`,
  `prepare_sampler_for_turn_does_not_flatten_dollar_credits_on_both`).
  Rank `hop_*` helpers are still not hop.

**Not a cargo land class:** rustc 1.97.1 (file pin only;
`rust-toolchain.toml` not in `FORK_PATHS`). Stuck-retry **pager** chrome is
not fully proven. Token Economy `/settings` table rows were not re-proven on
2026-08-15. CLI `grok-oss rebuild` is not clap-wired. `/economic-mode` is
not a live BuiltinAction. SuperGrok Heavy ranking optional label is not
implemented. Empty `models_cache.json` miss has no named test. Live hop /
live Business remaining / live TUI dogfood are unknown.

## Upstream regression filters

**Process pins** survive import via `FORK_PATHS` restore +
`assert-process-pins` (path presence and light content sniffs). That gate does
**not** prove product behavior inside shared `xai-grok-*` crates.

**Product seams** live inside those crates. They survive onto only through
**cherry-picks / conflict resolve** and stay honest through **named cargo
tests**. After recon, run the assert, then the seven-class land checklist,
then the name-existence check. `just check` cannot fail a deleted catalog
test. Deleting a red catalog test is not a restore. Paint is one of seven
land classes, not the whole land.

Full filter catalog (why each exists + every residual Validate honesty block):
[`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md).
Open residual still points at the same commands under RESIDUAL § *Validate
honesty* (D0 can demote; the catalog is durable).

Operator cheat sheet (post-import / post-onto tip). `rg` each identifier for
a matching `fn` first:

```bash
just upstream-assert-process-pins
./scripts/assert-process-pins.sh HEAD   # or onto tip

# 1. CLI identity (first token grok-oss; substring "grok" is not enough)
cargo test -p xai-grok-pager --lib -- product_version_line_uses_grok_oss_not_bare_grok \
  resume_session_command_uses_grok_oss user_guide_resume_and_version_examples_use_grok_oss \
  product_cli_name_is_grok_oss print_exit_resume_hint_writes_expected_lines \
  user_guide_operator_cli_examples_use_grok_oss welcome_badge_brands_grok_oss \
  hero_subtitle_brands_grok_oss tutorial_list_title_brands_grok_oss
cargo test -p xai-grok-pager-bin --test version_without_tty

# 2. Config is a surface (/settings rows + readers + DOGE picker; serde-only is not enough)
cargo test -p xai-grok-pager --test settings_e2e -- hide_header always_expand_thinking \
  scrub_ascii_punct allow_worktree bubble_copy_buttons plan_approval_park
cargo test -p xai-grok-pager --lib -- theme_choices_include_doge_and_default_is_doge \
  hide_header_zeroes always_expand_thinking_keeps_blocks_expanded \
  bubble_copy_buttons_on append_bubble_copy_button_paints \
  clicking_human_bubble_copy clicking_assistant_bubble_copy \
  clicking_wide_human_bubble_copy
cargo test -p xai-grok-pager-render --lib -- prime_applies_scrub_ascii_punct_from_ui
cargo test -p xai-grok-shell --lib -- resolve_subagents_copies_allow_worktree

# 3. Token Economy ledger /spend (schema-only is not enough; extra SQL, not SuperGrok dollar credits)
cargo test -p xai-grok-shell --lib -- spend_path_ingests_usage_jsonl_and_records_reconciliation
cargo test -p xai-grok-pager --lib -- show_spend_ingests_usage_jsonl_and_is_not_empty_default

# 4. DOGE / Surmount chrome (theme file existing is not paint)
cargo test -p xai-grok-pager-render --lib -- default_theme_is_doge resolve_from_config_no_config \
  doge_accent_user_is_pure_green doge_accent_system_is_pure_cyan
cargo test -p xai-grok-pager --lib -- user_prompt_block_accent user_prompt_entry_renderer_paints_green_rail \
  paint_composer_box_cursor_uses_human focused_composer_paints_human_green_box_caret \
  agent_message_block_accent info_line_model_name_uses_accent_model \
  status_bar_pushes_credits_compact_included_supergrok_period_limits \
  hit_credits_click_dispatches_show_limits \
  titled_doge_composer_frame_is_prompt_border_not_context_yellow \
  plan_approval_footer_paints_five_cta_vocabulary \
  auto_compact_completed_preserves_todo_board

# 5. Dual-auth hop after included SuperGrok period limits are full (rank helpers are not hop)
cargo test -p xai-grok-shell --lib -- sampling_config_auto_use \
  sampling_config_hops_to_sibling_included_before_extras \
  afterburner_does_not_skip_mark_when_sibling_has_included_remaining \
  resolve_model_to_sampling_config_auto_use \
  align_after_billing_switches_sticky_personal_full_to_business_included \
  prepare_sampler_for_turn_aligns_to_ranked_included_primary \
  combined_included_remaining_sums_distinct_personal_and_business_pools \
  combined_included_remaining_does_not_double_count_unified_pool \
  pick_prefers_business_included_before_personal_when_both_have_remaining \
  order_credentials_business_included_before_personal_when_both_have_room \
  limits_snapshot_second_process_reads_file_and_does_not_http \
  limits_snapshot_stale_file_lets_waiter_become_leader_and_fetch_once \
  limits_snapshot_never_writes_access_tokens \
  billing_handler_uses_snapshot_hub_instead_of_unconditional_sibling_http
cargo test -p xai-grok-pager --lib -- compact_meter_stays_included_while_sibling_pool_has_remaining \
  active_spend_driver_stays_included_while_any_distinct_pool_has_remaining

# 6. Last-session on start
cargo test -p xai-grok-pager --lib -- materialize_new_auto_opens_last_session_when_one_exists \
  materialize_new_auto_stays_welcome_when_no_last_session \
  materialize_new_auto_does_not_open_last_when_headless \
  from_pager_args_opens_last_session_on_start

# 7. Product skills are not a Python runtime (non-excepted .py or dropped intercept is a failed land)
cargo test -p xai-grok-bundle --lib -- sanitize_rejects_non_excepted_skill_python \
  extract_archive_skips_non_excepted_skill_python \
  product_repo_skill_roots_have_no_non_excepted_python
cargo test -p xai-grok-pager --lib -- user_guide_skills_are_not_a_python_runtime
cargo test -p xai-grok-tools --lib -- implement_memory_snapshot_intercept_does_not_spawn_shell \
  plan_validate_intercept_does_not_spawn_shell session_reader_list_intercept_does_not_spawn_shell

# Extra: plan present != approve + modal-free typing
cargo test -p xai-grok-pager --lib -- exit_plan_mode_present_is_not_operator_approve \
  empty_enter_on_revise_prompt_does_not_approve \
  soft_park_empty_ctrl_c_abandons_plan_approval \
  exit_plan_mode_keeps_mid_compose_draft_and_a_types \
  exit_plan_mode_modal_park_does_not_steal_mid_compose_keys \
  exit_plan_mode_empty_present_printable_goes_to_composer \
  exit_plan_mode_shows_overlay_even_in_yolo
cargo test -p xai-grok-tools --lib -- exit_plan_mode_tool_result_does_not_claim_operator_approval

# Extra: pause / Clear finished (not paint-only)
cargo test -p xai-grok-pager --lib -- work_control_chrome_matrix_pause_not_cancel_stop_not_pause \
  pause_button_click_dispatches_global_pause_not_cancel \
  idle_with_subagents_paints_pause_and_stop_hits \
  global_paused_idle_paints_resume_not_stop \
  clear_finished_action_idle_is_quiet_not_neon_green_or_magenta \
  clear_finished_click_does_not_open_subagent

# Extra: /rebuild SHA-aware (fail-does-not-signal is not enough)
cargo test -p xai-grok-update --lib -- failed_install_must_not_replace_or_signal_peers \
  build_fail_does_not_signal_leaders parse_version_output_extracts_identity \
  peer_relaunch_accepts_same_semver_different_sha \
  peer_relaunch_declines_equal_identity_on_same_path \
  peer_relaunch_accepts_deleted_inode_even_when_identity_equal
cargo test -p xai-grok-shell --lib -- leader_is_older_than_same_semver_git_sha_identity

# Extra: from_config cold catalog (empty models_cache.json miss is NOT this filter)
cargo test -p xai-grok-shell --lib -- from_config_without_prefetch_produces_usable_catalog

# Extra: session/load keeps seeded custom model on Chat Completions (not last-session on start)
cargo test -p xai-grok-shell --lib -- keep_unverified_persisted_model_keeps_seeded_custom_slug \
  seeded_test_model_keeps_chat_completions_backend
cargo test -p xai-grok-shell --test test_image_strip_recovery -- \
  poisoned_image_session_recovers_within_the_failing_turn

# Extra: nucleo reuse-per-root
cargo test -p xai-grok-workspace --lib -- repeated_open_without_close_keeps_one_search_per_root \
  distinct_roots_each_keep_one_search get_results_does_not_keep_a_stale_search_alive

# Extra: always-three-layer product prompt
cargo test -p xai-grok-agent --lib -- child_task_description_is_concise
cargo test -p xai-grok-tools --lib -- default_max_allows_l2_to_spawn_l3

# Extra: file-level infer-from-path verify (not crate-wide cargo)
cargo test -p xai-grok-tools --lib rust_edit_verify
cargo test -p xai-grok-tools --lib -- rustfmt_argv_edition_2024_config_and_absolute_files \
  clippy_argv_lints_the_edited_file_not_crate_lib \
  clippy_argv_is_file_level_not_package_lib \
  dangerous_cargo_fmt_all_is_refused_and_does_not_spawn_shell \
  dangerous_cargo_clippy_package_all_targets_is_refused_and_does_not_spawn_shell \
  dangerous_cargo_test_package_lib_filter_is_not_refused

# Extra: L1 Subagents list is L2-only plus a live L3 count
cargo test -p xai-grok-pager --lib -- \
  live_subagent_list_shows_only_l2_and_reports_live_l3_count \
  l2_row_shows_live_l3_count_not_specialist_names

# Extra: /start + leftover cancel-resume marker drop
cargo test -p xai-grok-pager --lib -- \
  start_while_globally_paused_continues_interrupted_turn_once \
  start_on_idle_clean_session_does_not_invent_a_turn \
  start_with_cancel_resume_marker_continues_interrupted_turn \
  handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn \
  handle_rebuild_done_idle_completed_turn_does_not_write_cancel_resume_or_refire_last_prompt \
  session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully

# Extra: ForceRefresh on explicit /limits
cargo test -p xai-grok-pager --lib -- \
  management_meter_cache_policy_collect_force_background_honor_ttl \
  should_clear_management_meter_caches_force_with_key_only
cargo test -p xai-grok-shell --lib -- \
  limits_snapshot_mode_for_get_billing_explicit_is_force_refresh

# Extra: sampling window vs catalog (chip + session field + spawn seed)
cargo test -p xai-grok-pager --lib -- \
  context_chip_names_sampling_window_when_catalog_differs \
  context_chip_hover_percent_uses_sampling_window_when_catalog_differs \
  footer_chip_uses_session_sampling_window_when_economic_cache_is_off \
  refresh_context_used_does_not_copy_catalog_into_session_sampling
cargo test -p xai-grok-shell --lib -- \
  spawn_seeds_sampling_window_at_economic_cap_when_disk_economic_is_on

# Extra: spawn-prompt fold + last-answer caps
cargo test -p xai-grok-sampling-types --lib -- fold_spawn_prompt
cargo test -p xai-chat-state --lib -- parent_estimated_tokens_omit_huge_spawn_prompt
cargo test -p xai-tool-types --lib -- to_model_text_caps_huge_last_answer_for_parent_ingest
cargo test -p xai-grok-tools --lib -- \
  completed_subagent_task_output_is_capped_or_points_at_report \
  blocking_spawn_subagent_completed_to_prompt_format_is_capped

# Extra: hop flatten / any stored included remaining (rank hop_* is not this)
cargo test -p xai-grok-shell --lib -- \
  sampling_config_hop_team_remaining_personal_exhausted_not_dollars_or_console \
  sampling_config_hop_personal_remaining_team_exhausted \
  sampling_config_hop_both_remaining_team_first_then_personal \
  sampling_config_hop_both_included_exhausted_dollar_credits_before_console \
  sampling_config_hop_missing_heavy_false_100_keeps_sibling_included \
  sampling_config_hop_dollar_credits_on_both_missing_heavy_keeps_team \
  prepare_sampler_for_turn_does_not_flatten_missing_heavy_100_off_sibling \
  prepare_sampler_for_turn_does_not_flatten_dollar_credits_on_both

# Extra: user-guide fork pins beyond class 1 resume + class 7 skills
cargo test -p xai-grok-pager --lib -- user_guide_does_not_claim_automatic_host_hop_is_unshipped \
  user_guide_names_token_economy_spend_order

# Neighbors that still have a matching fn (titles / stream retry emit / rebuild fail / /limits).
# Do NOT add retry_chrome_soft_reconnects_*, shell_collision, default_title_items_include_agents:
# those identifiers have no matching fn. Stuck-retry pager chrome is not fully proven.
cargo test -p xai-grok-pager --lib -- window_title_always_manages_non_empty_branded_osc \
  titles_on_session_name_osc_is_non_empty_branded window_title_osc_payload_never_empty_string \
  show_limits format_supergrok_session footer_names_live_principal \
  limits_json_lists_two_supergrok_principals_when_both_slots_exist \
  limits_json_honest_single_supergrok_session_cannot_see_team_plan
cargo test -p xai-grok-shell --lib -- stream_started_emits_retry_state_stream_resumed
cargo test -p xai-grok-sampler --lib -- wait_before_attempt_aborts_on_cancel \
  retry_footer_reason_uses_short_transport_label \
  retry_footer_backoff_hint_appends_next_try_in \
  stream_headers_timeout_defaults_to_120_secs_when_env_unset
cargo test -p xai-grok-sampler --test stream_headers_timeout

just check   # full gate before push/PR; does not replace missing catalog fn names
```

## CI and local quality

**CI is for checks only**: never build a shippable release package in GitHub
Actions (supply-chain boundary). Humans package from a trusted tree when ready.

| Command | Role |
|---------|------|
| **`just check`** or **`just ci`** | Full local gate (flake-meta + prep + fmt/clippy/tests): **run before push** |
| **`just test`** | Quality suite without re-running full flake prep |
| **`just build` / install** | Optional release-style package (not CI) |

GHA quality job: flake-meta → ci-prep → `just test` (see `.github/workflows/ci.yml`).
There is **no** `ci-quick` or `ci-host` recipe.

**PATH hermeticity (CI / low-mem):** with `CI_LOW_MEM=1`, `cargo-ci` enters
`nix develop .#ci`, then `scripts/with-ci-hermetic-path.sh` rebuilds `PATH`
from **`/nix/store` bins only** (ci-tools + stdenv: rustc, nextest, mold, git,
python3, coreutils, ...). Host desktop tools (`pw-record` / `parec` / `arecord`,
...) are not visible to quality tests, matches headless GHA. Interactive
`just dev` / default shell keep impure host `PATH`. Audio recorders are
intentionally **not** in `ci-tools`; `python3` **is** (cgroup + mock LSP e2e
spawn it under scrubbed PATH). Escape hatch: `GROK_CI_ALLOW_HOST_PATH=1`.
Closest GHA repro: `CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci`.

## Versioning and “am I up to date?”

| Idea | Practice |
|------|----------|
| **Upstream owns the package version number** | Keep lockstep with the upstream tree we track (`CARGO_PKG_VERSION`) |
| **Our identity is the git revision** | Binary shows **upstream version + short git SHA** |
| **No second release train** | No Surmount stable/alpha channel mirroring SpaceXAI |
| **No default xAI auto-update** | Would advertise official `grok` builds |

Illustrative only (not necessarily this checkout):

```text
grok-oss <upstream-version> (<short-sha>)
```

```bash
grok-oss --version
grok-oss update --check          # vs github.com/SurmountSystems/grok-oss main
grok-oss update --check --json
```

`SOURCE_REV` at the repo root is a **monorepo export pin** (full upstream-side
SHA recorded for the tree we absorbed), not a substitute for “what is HEAD.”

If behind: from a checkout run TUI **`/rebuild`** (wired, SHA-aware peer
relaunch), or rebuild/reinstall by hand. CLI **`grok-oss rebuild`** is
documented in the user-guide; clap has no `Rebuild` variant;
`run_rebuild_command` is unwired. Do not present CLI rebuild as a wired
command. Do not use the official `curl https://x.ai/cli/install.sh` path
(that installs upstream **`grok`**).

## Multi-session rate limits

Concurrent `grok-oss` processes share cooldowns under `~/.grok/rate_limits/`
(`grok-rate-limit`). On HTTP 429-style limits, the strictest wait wins across
processes. Disable shared coordination with `GROK_DISABLE_SHARED_RATE_LIMIT=1`.

Product HTTP paths that wait before send and observe on 429 (403 only when a
retry hint such as `Retry-After` is present):

| Class | Provider key shape | Examples |
|-------|--------------------|----------|
| Chat / inference | host + key fingerprint | sampler (xAI, SuperGrok proxy, OpenRouter, BYOK base URLs) |
| SuperGrok billing | proxy host + session fingerprint | `GET .../billing?format=credits`, auto-topup |
| Management API | management host + management-key fingerprint | prepaid, postpaid, usage series, key validation |
| Imagine image | host + fingerprint + `imagine` | `image_gen`, `image_edit` |
| Imagine video | host + fingerprint + `video` | `video_gen` start + poll |
| Voice STT | host + fingerprint + `voice` | streaming `wss://.../v1/stt` |
| Responses | host + fingerprint + `responses` | `web_search` |
| GitHub | logical `github` | OSS update compare |

Waits prefer server headers (`Retry-After`, then `x-ratelimit-reset`) over
hardcoded tier tables. Public docs (accessed 2026-08-03):

- [xAI rate limits](https://docs.x.ai/developers/rate-limits) (per-model RPS/TPM;
  Imagine image/video have separate RPS; Voice/Imagine tier increases via sales)
- [OpenRouter limits](https://openrouter.ai/docs/api_reference/limits) (honor
  `Retry-After` / `X-RateLimit-*` on 429)
- [GitHub REST rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
  (primary + secondary; `Retry-After` / `x-ratelimit-reset`)

## Canonical repo

<https://github.com/SurmountSystems/grok-oss>

## License

Apache License 2.0: [`LICENSE`](LICENSE).
Third-party: [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES).
