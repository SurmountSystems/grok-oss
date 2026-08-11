# Implement: rebuild must restart all active product processes (2026-08-09)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `fixes-2`
**Board:** `bug:rebuild-not-restarting-all-processes`

## Operator report

Multi-session dogfood (bitmagi / iso / surmount-server / colibri / fixes-2): after
`/rebuild`, only the invoking window looked updated. Other live windows kept old
chrome / behavior (stale or `(deleted)` binary images).

## What rebuild did today (pre-fix)

Evidence from code + prior reports
(`.agents/reports/impl-rebuild-slash-2026-08-07.md`,
`plan-rebuild-reboot-graceful-2026-08-07.md`, `rebuild.rs`):

| Step | Behavior |
|------|----------|
| Install | `just install` / fixed cargo argv → `~/.cargo/bin/grok-oss` |
| Leaders | `signal_leaders_to_relaunch` → `RelaunchForUpdate` (graceful drain) |
| Invoker TUI | Arm `RebuildRelaunch` + quit + `exec` onto installed path |
| **Other live TUIs** | **Inventory only** + “may still need reattach” summary lines |

Explicit v1 residual (impl report 2026-08-07):

> Other standalone TUIs (not leaders, not the invoking process): v1 reports
> live PIDs + reattach hints only. No cooperative quit marker / SIGUSR in this cut.

So this was **not** a silent regression from a broader multi-process restart that
used to ship. The original cut never restarted peer windows. Operator
expectation for multi-session dogfood is that **all** active product windows
pick up the new binary after one `/rebuild`.

### Why old behavior sticks on peers

1. Peer TUI keeps the old process image after install overwrites the cargo-bin
   path (Linux: `/proc/<pid>/exe` shows `…/grok-oss (deleted)`).
2. Leader spawn uses `current_exe()` for non-managed installs under
   `~/.cargo/bin` (not under `~/.grok`), so a stale client can re-spawn a stale
   leader.
3. Client reconnect after leader AutoUpdate does **not** re-exec the client.

## Root cause

Incomplete product path: rebuild restarted leaders + invoker only; peers left
running on the replaced binary.

## Product fix

After successful install + leader signal:

1. **Write** `$GROK_HOME/rebuild_relaunch_request.json`
   (`installed_exe`, `installed_identity`, `requested_at_unix_secs`).
2. **Signal every other live product PID** in `active_sessions` with
   `SIGUSR1` (`signal_process_rebuild_relaunch`), skipping self / dead / non-grok.
3. **Peer TUI** (`SIGUSR1` handler): set flag + graceful quit notify.
4. **Event loop** on quit notify: if peer-rebuild flag, arm
   `rebuild_relaunch` from the request (when identity is older **or** running
   exe is deleted/different path, request is fresh ≤15 min, exe exists), then
   `Action::Quit` (cancel-resume still on mid-turn).
5. **Post-restore** path already `exec`s `rebuild_relaunch` (same as invoker).

Invoker still self re-execs via `/rebuild` TaskResult (excluded from SIGUSR1).

### Key paths

| Concern | Path |
|---------|------|
| Peer PID filter, request file, signal, summary | `crates/codegen/xai-grok-update/src/rebuild.rs` |
| `SIGUSR1` / User1 kill helper | `crates/codegen/xai-grok-shell-base/src/util/mod.rs` |
| Peer arm + pure re-exec plan | `crates/codegen/xai-grok-pager/src/app/dispatch/rebuild.rs` |
| SIGUSR1 → flag + quit notify | `…/app/signal_handler.rs` |
| Quit arm before Quit | `…/app/event_loop.rs` |
| Docs | user-guide `04-slash-commands.md`, `FORK.md` |

## Red → green proof

Named contract: rebuild schedules restart of **all** active product instances of
this install, not only the invoking session.

| Test | Contract |
|------|----------|
| `peer_pids_to_signal_excludes_self_dead_and_non_grok` | Signal only live grok peers |
| `peer_relaunch_accepts_same_semver_different_sha` | SHA rebuild accepted |
| `peer_relaunch_declines_equal_identity_on_same_path` | No thrash after re-exec |
| `peer_relaunch_accepts_deleted_inode_even_when_identity_equal` | `(deleted)` forces re-exec |
| `peer_relaunch_declines_stale_request` | >15 min ignored |
| `rebuild_relaunch_request_round_trips_on_disk` | Request serde |
| `format_rebuild_summary_includes_peer_signals` | Summary reports peers; no “may still need reattach” |
| `peer_rebuild_relaunch_if_applicable_arms_when_older_and_exe_exists` | Pager arms re-exec |
| `peer_rebuild_relaunch_if_applicable_arms_deleted_inode` | Deleted path arms |
| `peer_rebuild_relaunch_if_applicable_skips_*` | Equal path / missing exe skip |

```bash
cargo test -p xai-grok-update --lib rebuild::
# 25 passed

cargo test -p xai-grok-pager --lib dispatch::rebuild::
# 12 passed

cargo test -p xai-grok-shell-base --lib kill_process
# 3 passed

cargo fmt -p xai-grok-shell-base -p xai-grok-update -p xai-grok-pager
cargo clippy -p xai-grok-shell-base -p xai-grok-update --all-targets -- -D warnings
cargo clippy -p xai-grok-pager --lib -- -D warnings
# clean
```

## Dogfood expectation

After `/rebuild` (or `grok-oss rebuild`) from one window:

1. That window re-execs onto the new binary (same session; mid-turn resume toast when applicable).
2. Other live product windows receive `SIGUSR1`, quit gracefully, and re-exec onto
   the same installed path with their session id.
3. Leaders drain via `RelaunchForUpdate`; clients that re-exec spawn/connect with
   the new binary.
4. `ps` / `/proc/<pid>/exe` should not show multi-pid lag on `(deleted)` grok-oss
   for sessions that were registered in `active_sessions` at rebuild time.

Manual check:

```bash
# while several grok-oss TUIs are open:
ls -l /proc/*/exe 2>/dev/null | rg 'grok-oss'
# after /rebuild from one window: no (deleted) rows for those sessions
```

## Limits / not claimed

- Windows: `SIGUSR1` maps to terminate (no graceful peer re-exec on Windows in this cut).
- Headless / PIDs not in `active_sessions` are not signaled.
- Official SpaceXAI `grok` binary is out of scope.
- SIGKILL peers still cannot re-exec themselves.

## No git

No `git add` / commit / push (operator-only).
