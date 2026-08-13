# Fix: unused `kind_filter` in pager session list dispatch

**Date:** 2026-08-11  
**Crate:** `xai-grok-pager`  
**Error:** `unused variable: kind_filter` at `foreign.rs:174`

## Root cause

`dispatch_fetch_session_list` already computed `welcome_history_kind_filter(app)` (welcome Sandbox → `chat`, Local → `build`) but never passed it into `Effect::FetchSessionList`. The effect variant only had `query` + `seq`. Lifecycle tests already expected a `kind_filter` field on the effect.

## Fix (wire product behavior, not discard)

1. **`Effect::FetchSessionList`** (`actions.rs`): add  
   `kind_filter: Option<Vec<String>>`  
   (wire: `_meta["x.ai/facetFilters"]["kind"]` on `x.ai/session/list`).

2. **`dispatch_fetch_session_list`** (`foreign.rs`): pass `kind_filter` into the effect; log it via `log_history_source` (also fixed missing 4th arg `source`).

3. **Search/refetch paths** (`load.rs`): same filter on chat search empty/forced fetch and debounce expiry so welcome history stays kind-scoped.

4. **Effect runner** (`effects/mod.rs`): when `kind_filter` is `Some`, set  
   `params["_meta"]["x.ai/facetFilters"]["kind"]`  
   so shell `parse_list_req` can honor client kind under `local-workspace` instead of force-rewriting to chat.

5. **Effects unit test** constructors: supply `kind_filter: None`.

## Verify

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

- **This error class:** gone (no `unused variable: kind_filter` in clippy output).
- **Full `--lib -D warnings`:** still fails with many pre-existing `dead_code` / `unused_imports` under lib-only (test-gated APIs). Out of scope for this fix.

## Files touched

- `crates/codegen/xai-grok-pager/src/app/actions.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/session/foreign.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/tests.rs`
