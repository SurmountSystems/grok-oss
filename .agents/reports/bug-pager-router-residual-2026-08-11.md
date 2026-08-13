# Pager router residual — green (2/2)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Inventory:** `.agents/reports/bug-pager-residual-inventory-2026-08-11.md` (router cluster: 2)

## Result

```text
cargo test -p xai-grok-pager --lib 'app::dispatch::tests::router' -- --test-threads=8
→ ok. 103 passed; 0 failed
```

Both previously red contracts green. `cargo fmt -p xai-grok-pager` clean.

## Failures (red) → product fix

### 1. `deferred_switch_overwritten_by_second_switch`

**Red:** second pre-session `SwitchModel` stashed
`prev_model_id: Some("model-a")` instead of `None`.

**Cause (half-merge):**
```rust
.take().and_then(|prior| prior.prev_model_id).or(prev_model)
```
When a prior deferred stash had a deliberate `prev_model_id: None`, `.and_then` produced `None` and `.or` filled in the intermediate optimistic `models.current` (model-a).

**Fix:** `dispatch/router.rs` `Action::SwitchModel` no-session arm — match on prior stash:

- `Some(prior)` → keep `prior.prev_model_id` (including `None`)
- `None` → first stash uses displayed `prev_model`

### 2. `slash_hooks_opens_modal`

**Red:** `effects.len()` was **7**, expected **6**. Modal opened.

**Cause (half-merge):** `extensions_modal_tab_fetches` in `dispatch/transcript.rs` listed **two** identical `Effect::FetchWorkflowsList` entries, then marketplace → 7.

**Fix:** drop the duplicate workflows fetch. Canonical list is hooks + plugins + mcps + skills + workflows + marketplace = **6**. Sibling `count_extension_fetches` (excludes workflows) still expects **5**.

## Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` | deferred overwrite keeps original rollback prev |
| `crates/codegen/xai-grok-pager/src/app/dispatch/transcript.rs` | remove duplicate `FetchWorkflowsList` |

No test expectation edits. No git commit/add/push.

## Verify

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib 'app::dispatch::tests::router' -- --test-threads=8
nice -n 19 ionice -c3 cargo fmt -p xai-grok-pager
```
