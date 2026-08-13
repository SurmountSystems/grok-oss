# Thread leak fix: grok-keyring-op + history-search

## Root cause

### Leak A: `grok-keyring-op` (main hog, ~3,200 threads per process)

`CredentialsStore::run_keyring_op` spawned a new OS thread named `grok-keyring-op` for every Secret Service get/set/delete. On timeout it **abandoned the wait and the thread**. The login collection is Locked, so every `get_password` blocks until the 3s budget, then the sleeper stays parked in D-Bus forever.

The resolve-only circuit breaker (15s) only skips `read`. After that TTL, the next resolve probe spawned **two** more sleepers (primary + fallback). `read_for_update` and writes always probe. Status/billing paths call `collect_dual_auth_status` → `store.read` on a timer. Overnight: ~2 new abandoned workers every 15s → thousands of sleeping `grok-keyring-op` threads. Tokio stayed normal (~20–50). This was not a runtime explosion.

### Leak B: `history-search` (fat on dragon-npu / iso)

Each `HistorySearchState` (one per `PromptWidget`, including every subagent child view) spawned its **own** matcher thread on first activate. Reuse was only per widget. Many live composers, or many widgets that had opened history, meant one parked `history-search` thread each. `Daemon::drop` sent `Stop` and did not join. Construction was already lazy (subagent storm test still holds).

`FuzzySearchManager` reuse-per-root and nucleo `Some(2)` (never `None` as the requested pool size) still hold after the 1.0.3 restack. Verified in source (`Nucleo::new(..., Some(NUM_NUCLEO_THREADS), 1)` with `NUM_NUCLEO_THREADS = 2`) and `file_system::tests`.

## Named TDD contracts

1. **Keyring:** a locked or timed-out Secret Service call does not leave an extra sleeping `grok-keyring-op` worker. At most 2 in-flight helpers (one primary, one fallback). Repeated resolve, RMW, and write timeouts reuse those helpers.
2. **History search:** a second (and twentieth) live `HistorySearchState` reuses one matcher thread and does not grow `history-search` workers without bound. Each state still keeps its own results.

## Red (before product fix)

```
nice -n 19 ionice -c3 cargo test -p xai-grok-shell --lib \
  credentials_store::tests::timed_out_secret_service_does_not_leave_unbounded_sleeping_workers \
  -- --nocapture --test-threads=1
```

Fail: `this burst grew 42 (before=0 after=42)` expected `<= 2`.

```
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  views::history_search::tests::many_live_states_share_one_history_search_thread \
  -- --nocapture --test-threads=1
```

Fail: `20 live history searches must reuse one matcher thread, spawned 20`.

## Green (same filters after product fix)

Same two commands: **ok**.

Also green:

- `cargo test -p xai-grok-shell --lib credentials_store::tests -- --test-threads=1` (16 passed)
- `cargo test -p xai-grok-pager --lib views::history_search::tests -- --test-threads=1` (16 passed)
- `subagent_spawn_storm_spawns_no_matcher_daemons`
- `cargo test -p xai-grok-workspace --lib file_system::tests -- --test-threads=1` (reuse-per-root still holds)

## Product fix

**Keyring:** one long-lived `grok-keyring-op` helper per backend (primary + fallback). Ops go through a capacity-1 channel. Timeout still fails loud (`KeyringTimeout`) and does **not** wait forever. The same helper stays in flight. A later op queues on that helper or times out without spawning another sleeper. Hang tests capture “should hang” at enqueue so leftover jobs cannot call the real keyring after the hook clears. Still no Secret Service `Unlock` / `Prompt`. Locked keyring remains a credentials-store miss.

**History search:** one process-wide matcher thread. Each `HistorySearchState` is a client (own items + snapshot). Drop forgets that client; the thread stays for the next composer. Same-client drain still coalesces queries so two composers cannot drop each other’s updates.

## Files touched

- `crates/codegen/xai-grok-shell/src/auth/credentials_store.rs`
- `crates/codegen/xai-grok-pager/src/views/history_search.rs`

## Verify

- `cargo fmt -p xai-grok-shell -p xai-grok-pager`
- `cargo clippy -p xai-grok-shell --lib --bins -- -D warnings` (ok)
- `cargo clippy -p xai-grok-pager --lib --bins -- -D warnings` (ok)
- Nucleo still `Some(NUM_NUCLEO_THREADS)` with `NUM_NUCLEO_THREADS = 2` in `xai-fuzzy-file-search`. `None` is only the “pool could not be spawned” degrade path, not the requested size.

## What this work did not do

- Did not Unlock the login collection, send Secret Service Unlock/Prompt/Dismiss, or tell anyone to type the login password.
- Did not kill `gnome-keyring-daemon`, live grok-oss TUIs, Brave, System Monitor, rustc, or file transfers.
- Did not diagnose with `busctl` / `secret-tool` / keyring process walks.
- Did not `git add` / commit / push.

## Leftover honesty

- Already-running grok-oss processes still hold the leaked threads until those processes are quit and replaced with a rebuild. This change does not shrink live PIDs.
- The two helpers can stay blocked in D-Bus for the process lifetime while the collection is Locked. That is the bound (2), not a join of the kernel wait. A slow `set_password` can still land after a reported timeout (same inherent race as before, now on the reused helper).
- Distinct fuzzy-search roots still each own one nucleo pool (2 workers). History-search keeps one parked matcher thread for the process once anyone has opened history.
- Resolve still uses the 15s circuit breaker. That is latency, not the thread bound.
