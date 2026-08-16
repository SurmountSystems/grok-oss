# from_config without prefetch produces a usable catalog

## Named contract

`ModelsManager::from_config` with no prefetch argument is a zero-network
boot. It must put at least one model in the internal catalog, the resolved
default must be one of those keys, and that boot must not claim a real
fetched catalog. An empty models disk cache is a miss, not a fetch. SuperGrok
is a paid product; this path does not invent included SuperGrok period
limits.

## Red (before any product edit)

Command (GHA-like: no console API key, so Session auth can match a grok-home
session cache):

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-prefetch-catalog-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
env -u XAI_API_KEY -u GROK_CODE_XAI_API_KEY \
  cargo test -p xai-grok-shell --offline --lib \
  agent::models::tests::from_config_without_prefetch_produces_usable_catalog \
  -- --exact --nocapture
```

Exit: 101

Fail reason: `cold-cache boot must not claim a real catalog` at
`crates/codegen/xai-grok-shell/src/agent/models/tests.rs:1201`.

With a console API key still in the environment the same test was green,
because `ModelFetchAuth` became `ApiKey` and missed the session cache. With
the key unset, `from_config(None)` loaded a fresh non-empty
`$GROK_HOME/models_cache.json` (session, `cli-chat-proxy.grok.com`, two
models) and set `has_fetched_real_catalog`.

## What changed and why

The named test was not rewritten.

1. `ModelsCacheManager::load_fresh` now returns `None` when `models` is
   empty. An empty disk file is a miss for prefetch, reload, and any other
   loader. `resolve_model_list(Some(empty))` would otherwise replace the
   bundled catalog with nothing.

2. `ModelsManager::from_config` no longer treats the grok-home disk cache as
   a silent prefetch. Only an explicit non-empty prefetch argument is a real
   catalog. Disk hits stay on `prefetch_models_blocking` and
   `reload_from_disk_cache`. A no-prefetch boot builds the bundled catalog
   (and still inserts the resolved default if it is missing).

## Green (same named test)

Same command as red, with and without `XAI_API_KEY`:

```
env -u XAI_API_KEY -u GROK_CODE_XAI_API_KEY \
  cargo test -p xai-grok-shell --offline --lib \
  agent::models::tests::from_config_without_prefetch_produces_usable_catalog \
  -- --exact --nocapture
```

Exit: 0

With `XAI_API_KEY` still set: exit 0.

`cargo fmt -p xai-grok-shell`: exit 0.

`cargo clippy -p xai-grok-shell --offline --lib -- -D warnings`: exit 0.

Nearby contracts still green: `disk_cache_reload_applies_without_fetching`,
the `reload_from_disk_cache_*` set (8 tests),
`sign_out_clears_catalog_rebuilds_bundled_without_fetching`,
`resolve_model_list_empty_prefetch_yields_empty_base`.

## Leftovers that are real

- `apply_catalog` still sets `has_fetched_real_catalog` if a caller hands it
  an empty map. Network fetch already refuses an empty list. `load_fresh`
  now refuses an empty disk file. A direct empty `apply_catalog` is still a
  fetched claim.
- `from_config` no longer copies a disk etag onto a no-prefetch manager.
  Warm start from disk is the prefetch or reload path.
