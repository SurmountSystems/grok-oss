# impl:ci20-router-shell

Board: `impl:ci20-router-shell` under `bug:ci-20-unit-fails`.

Isolated compile: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ci20-router-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`. Tests were not weakened.

Product CLI name stays `grok-oss`. Cancel-resume / last-session contracts were not undone.

## 1. `pager_registry_default_matches_agent_view_new_initializer`

**Red.** The Pager owner loop panics on any key with no alignment arm. A new Pager setting `bubble_copy_buttons` landed without an arm, so the test failed with:

> PAGER setting `bubble_copy_buttons` has no arm in pager_registry_default_matches_agent_view_new_initializer

**Product change.** None to defaults. The registry default already matches `ScrollbackDisplayConfig::default()` / `AgentView::new` (true). Added the missing triangle arm in `app/dispatch/tests/router.rs` that pins the registry bool against `agent.scrollback.appearance().scrollback.display.bubble_copy_buttons`.

**Green.** Same filter, pass.

## 2. `session_loaded_applies_cancel_resume_marker_and_toasts`

**Red.** `handle_session_loaded` never read `canceled_turn_resume.json`. After `SessionLoaded` the toast was empty, no `Effect::SendPrompt`, state stayed idle, queue empty.

**Product change.** In `app/dispatch/session/load.rs`:

- Read `resume_canceled_turn_on_restart_enabled()` before the agent mut borrow.
- After `pending_first_prompt` enqueue, if this load is **not** adopting a live `running_prompt_id`, call `apply_canceled_turn_resume_on_load`.
- Helper loads the marker, checks `should_auto_resume_on_restart`, shows toast **Continuing interrupted turn...** (`auto_resume_toast()`), `enqueue_prompt_front`, then clears the one-shot marker.

`maybe_drain_queue` then emits `Effect::SendPrompt`. This is continue-interrupted-turn, not `/resume` session pick. Adopting a live running prompt still skips auto-resume.

**Green.** Same filter, pass. Toast contains `Continuing interrupted turn`, `SendPrompt` text matches the marker, turn is running, queue drained.

## 3. `idle_with_all_watcher_kinds_lists_all`

**Red.** Idle row at 72 cols with commands + monitors + loops + subagents. Work B `[pause]` / `[stop]` stole the right side, so the clipped cue lost `3 subagents still running`. Contract: one cue lists every nonzero kind.

**Product change.** In `views/turn_status.rs` idle/parked render: if `icon + cue + pause/stop` would overflow `area.width`, drop pause/stop (and the gap) so the kinds list stays complete. Narrow-area tail-clip of the cue itself is unchanged (`narrow_area_clips_cue_tail_keeping_counts`).

**Green.** Same filter, pass. Full string present:

`1 command · 2 monitors · 1 loop · 3 subagents still running`

## 4. `env_keys_resolve_skips_whitespace_only_value`

**Red.** `EnvKeys::resolve_value_with` routed through `split_api_key_list` / `push_unique_key`, which trim. Whitespace-only primary did not fall through to fallback. Padded real token `"  tok  "` was returned trimmed.

**Product change.** In `agent/config.rs`, `resolve_value_with` iterates configured names, skips `trim().is_empty()`, returns the **raw** first non-blank value. Multi-value `resolve_all_values_with` still splits and trims list parts.

**Green.** Same filter, pass. `"   "` falls through to `"real"`; all-whitespace is `None`; `"  tok  "` stays padded.

## 5. `from_config_without_prefetch_produces_usable_catalog`

**Red.** Zero-network `ModelsManager::from_config(&Config::default(), None, …)` could produce an empty catalog (custom `GROK_MODELS_BASE_URL` / empty disk cache treated as a real prefetch). Default key missing; `has_fetched_real_catalog` could be claimed on a hollow cache.

**Product change.** In `agent/models.rs`:

- Treat an empty disk-cache map as a miss (`.filter(|models| !models.is_empty())`).
- After `resolve_default_model`, if the resolved default key is not in the catalog, insert it.

Cold boot still must not claim a fetched catalog (`has_fetched_real_catalog` stays false when there was no non-empty prefetch).

**Green.** Same filter, pass. Catalog nonempty, contains `current_model_id()`, `has_fetched_real_catalog() == false`.

## Verify

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ci20-router-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
cargo fmt -p xai-grok-pager -p xai-grok-shell -- --check   # FMT_CHECK:0
cargo clippy -p xai-grok-pager --lib -- -D warnings       # PAGER_CLIPPY:0
cargo clippy -p xai-grok-shell --lib -- -D warnings       # SHELL_CLIPPY:0
cargo test -p xai-grok-pager --lib -- \
  pager_registry_default_matches_agent_view_new_initializer \
  session_loaded_applies_cancel_resume_marker_and_toasts \
  idle_with_all_watcher_kinds_lists_all
# 3 passed; 8877 filtered; 0.02s
cargo test -p xai-grok-shell --lib -- \
  env_keys_resolve_skips_whitespace_only_value \
  from_config_without_prefetch_produces_usable_catalog
# 2 passed; 6590 filtered; 0.01s
```

Foreground cargo at 120s was killed during the first compile; later runs used a long background wait. Isolated target stayed warm.

## Files touched

- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/router.rs` (bubble_copy_buttons alignment arm)
- `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` (cancel-resume on SessionLoaded)
- `crates/codegen/xai-grok-pager/src/views/turn_status.rs` (cue-first overflow)
- `crates/codegen/xai-grok-shell/src/agent/config.rs` (`resolve_value_with`)
- `crates/codegen/xai-grok-shell/src/agent/models.rs` (`from_config`)

Stayed off credit_bar, limits_honesty, allowance_exhaust, settings_e2e, prompt_widget, dashboard peek.

## Leftovers

- A new Pager-owned setting still needs an arm in `pager_registry_default_matches_agent_view_new_initializer` or that test will panic again. That is the test's contract.
- Cancel-resume is skipped when the load adopts a live `running_prompt_id`. Intentional.
- Idle work chrome is dropped only when it would clip the kinds list. Wide rows still show `[pause]` / `[stop]`.
- `has_fetched_real_catalog` is still set on any **non-empty** prefetch, including a populated disk cache. The named test only requires false on cold/empty cache. Left as-is.
- `Config::default()` still reads `GROK_MODELS_BASE_URL`. Custom endpoint plus empty remote catalog is why the default-insert is required.

Stop.
