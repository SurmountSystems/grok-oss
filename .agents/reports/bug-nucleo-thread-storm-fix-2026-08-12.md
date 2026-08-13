# Nucleo thread storm fix (2026-08-12)

## Named contract

Opening many workspace fuzzy searches without closing them must not grow an unbounded set of nucleo worker threads. One grok-oss process must keep a small constant number of nucleo workers (2 per live root), not two new workers per `fuzzy_open`.

Observed dogfood: long-lived grok-oss PIDs grew ~3500 and ~3300 threads named `nucleo worker`.

## Red (before product edit)

Command:

```
nice -n 19 ionice -c3 cargo test -p xai-grok-workspace --lib file_system::tests -- --nocapture --test-threads=1
```

Fail reason (all three new tests, same command):

- `file_system::tests::repeated_open_without_close_keeps_one_search_per_root` — `20 opens of the same root without close must keep 1 live search, not 20` (left 20, right 1)
- `file_system::tests::distinct_roots_each_keep_one_search` — `10 opens each must not grow past 2` (left 20, right 2)
- `file_system::tests::get_results_does_not_keep_a_stale_search_alive` — poll-only `get_results` refreshed `last_activity`, so `cleanup_stale` kept the search (left 1, right 0)

## Green (same tests after product edit)

Command (same filter):

```
nice -n 19 ionice -c3 cargo test -p xai-grok-workspace --lib file_system::tests -- --nocapture --test-threads=1
```

Result: 3 passed, 0 failed.

Also green: `execute_fuzzy_open_returns_search_id`, `execute_fuzzy_open_close_parity`.

`cargo fmt -p xai-grok-workspace` and `cargo clippy -p xai-grok-workspace --all-targets -- -D warnings` both succeeded.

## Product fix

`FuzzySearchManager::open` now reuses the existing matcher and search id when the same root is already live. It does not call `FuzzySearchContext::new` (and therefore does not call `Nucleo::new(..., Some(2), 1)`) again for that root. A reused open bumps `query_version` so old poll loops go stale, updates routing metadata, and restarts the walk only if `hidden` changed.

`get_results` / `get_results_filtered` no longer write `last_activity`. Query `change` and a reused `open` still do. Nucleo still takes `Some(2)`, never `None`.

## Files changed

- `crates/codegen/xai-grok-workspace/src/file_system/mod.rs` — reuse-per-root `open`, poll does not keep searches alive, unit tests
- `crates/codegen/xai-grok-workspace/src/handle.rs` — drop unused `mut` on poll/get-results locks after getters became `&self`
- `RESIDUAL.md` — one shipped Open bullet

## Leftover

- Distinct roots still each own one matcher (2 nucleo workers). There is no process-wide share of a single pool.
- Root identity is the path as passed, not a canonicalized filesystem id. Two spellings of the same directory can still create two matchers.
- `cleanup_stale` still runs on the next `open`, not on a timer. Poll no longer prevents expiry; nothing sweeps if no later `open` happens. One leftover matcher per abandoned root is bounded.
- TUI `@` (`FileSearchState`) was already one daemon after first `@` and was not rewritten. Many live composers that each used `@` still mean 2 nucleo workers each.
- Operator must rebuild and fully quit old `grok-oss` processes for the live PIDs to drop the leaked workers.
