# PR #36 local nextest mop

Date: 2026-08-13  
Branch: `onto-xai/b13fa526f511`  
Start HEAD: `a036327e6151398f7c46b79948256b24b2ae1832`  
Origin moved during mop to `48f0bf1a6307d25cb30561295de6e89aa37d59c5` (`ci: regenerate xai-grok-agent encrypted prompt templates`). Product fixes landed on that tip. No force-push.  
rustc: 1.97.1

## Commands and exits

| Step | Command | Exit |
|------|---------|------|
| First locked nextest (4 crates, nice 19, `--test-threads=2 --build-jobs 2`) | `cargo nextest run -p xai-grok-shell -p xai-grok-pager -p xai-grok-sampler -p xai-grok-pager-minimal --locked --test-threads=2` | 100 |
| First run summary | 16199 run, 16109 passed (1 flaky recovered), **90 failed**, 391 skipped, 622.7s | 100 |
| Re-run original 90 after first product wave | same nextest `-E` over `/tmp/pr36-fails.txt` | 100: **42 pass / 48 fail** |
| Targeted 7 after remaining product fixes | `cargo nextest run -p xai-grok-shell --locked --test-threads=2 --build-jobs 2 -E 'test(update_stores_team_token…) + … + expired_external…'` | **0** (7/7 pass) |
| `team_sync_writes_files` (confirm remaining) | `-E 'test(team_sync_writes_files)'` | 100 (wrote=false) |
| `cargo fmt -p` pager, pager-minimal, pager-render, shell, sampler | | **0** |
| `cargo clippy -p … --lib --bins --locked -- -D warnings` | same five crates | **0** |

`--jobs` is a nextest alias for `--test-threads`. Use `--build-jobs` for compile parallelism.

Known flake `xai-grok-shell terminal::pty_session::tests::close_pty_kills_a_background_grandchild` (**SLOW**): **PASSED** on the first full run. Report only; not "fixed."

Flaky recovered on first run: `test_timeout_kills_grandchildren_and_returns_promptly`.

## What we fixed (product, tests stay spec)

Did **not** weaken asserts. Did **not** touch Nucleo `Some(2)` / `FuzzySearchManager` / `last_activity`. Did **not** invent five-CTA, spend-order, or PTY work.

### First wave (42 of 90 green on re-run)

- Soft interject is buffer-only (`handle_interject_queued_prompt` always `false`); send-now is `queue_input(send_now: true)`.
- Queue drain returns empty when `global_work_pause` or `soft_stop.blocks_drain`.
- Overlay question input: invert cell, hardware `cursor_pos` `None`.
- Theme hermetic: `pin_theme()` on teal / palette / max-thoughts tests; mermaid e2e resets live cache to `Auto`.
- Doctor theme counts: Doge default → 3/6 and 2/6 as each fixture names.
- Browser toast: URL-first one line.
- 429 user copy strips `API error 429:` prefix; generic API arm keeps status.
- Generic `Retry-After` > 30s → `jitter_around(30s)`.
- Implement effort uses shared helper.
- Marketplace fixture git: local `commit.gpgsign false` + `ALLOW_UNSIGNED_COMMIT=1` only inside dummy repo spawn.
- Economic 200k cap on model metadata.
- StreamStarted → `RetryState::StreamResumed`; channel token scrub.
- `usage.jsonl` via `usage_log::record_model_call`.
- Workflow restore: collect/sort/filter **then** cap 128.
- Remote `auto_compact` undercut dropped when baked is `None`.
- Doom-loop header: clamped window tokens (default **1024**), not `"true"`.
- Skill wrap assert requires `run_id=`.

### Second wave (7 more, targeted green)

- `AuthManager::update` / `save_without_enrichment` / `persist_and_swap` persist SuperGrok session via `upsert_supergrok_session` (base + `{base}::personal` / `{base}::team::{id}`).
- `SessionActor` turn tests wrap 16 MiB stack + current-thread `LocalSet` (`start_paused` kept on exhaust).
- Terminal inference 401 on a live External provider records `ProviderInteractiveRequired` then applies `auth_remedy` so the message names the provider and `/login`, not "wait it out."

## Remaining fails (do not burn GHA on these without a plan)

### Leave (named contract conflict / Surmount hard-off / residual)

- `xai-grok-sampler retry::tests::cloudflare_edge_range_is_transient` — 525 is **Fatal**; sibling `classify_cloudflare_525_is_fatal` is the SoT. Do not weaken range-is-transient.
- `xai-grok-shell session::unified_list::tests::parse_list_req_forces_kind_under_process_chat_mode_only` — `process_chat_mode_enabled()` is hardcoded `false` (Surmount). `conversations_lane_active` already expects false even with `GROK_CHAT_MODE=1`.
- Five-CTA plan panel restore: **not this mop**.

### Local-env / process-global `GROK_HOME` OnceLock (likely clean on GHA)

- Entire `xai-grok-shell::team_managed_config` binary (~30 tests). `team_sync_writes_files` still `wrote=false` in ~50ms (`NoPrincipal` / `grok_home()` OnceLock can lock to operator `~/.grok` before `test_home()` sets `GROK_HOME`). Integration binary has no `cfg(test)` home ctor.
- Auth/env (operator keyring / `XAI_API_KEY` / session):
  - `agent::auth_method::tests::enterprise_byok_config_does_not_require_login`
  - `agent::auth_method::tests::env_key_probe_unusable_suppresses_advertise_without_byok`
  - `agent::config::tests::has_own_credentials_guards_session_vs_external_key`
  - `agent::config::tests::resolve_credentials_openrouter_does_not_use_xai_session`
  - `agent::mvp_agent::tests::post_auth_settings_failure_resolves_gate_onto_local_policy`
  - `agent::mvp_agent::tests::post_auth_settings_non_xai_keeps_local_but_still_emits`
  - `agent::mvp_agent::tests::post_auth_settings_xai_upgrades_writeback_emits_and_opens_gate`
  - `agent::mvp_agent::tests::settings_not_cached_when_identity_logs_out_during_fetch`
  - `agent::subagent::tests::resolve_model_override_api_key_pin_keeps_console_primary`

Do not invent product to paper over a live SuperGrok / console session on this machine.

## SHA

Product mop tip: `1a38c1f8842dd4d6ac16a1b4f426598806ad6dc4`  
Parent: `48f0bf1a6307d25cb30561295de6e89aa37d59c5`  
This report SHA pin is a follow-up commit-tree on that parent.
