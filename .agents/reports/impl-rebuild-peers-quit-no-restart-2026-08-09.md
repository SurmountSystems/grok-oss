# Implement: rebuild peers quit without restart (2026-08-09)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Board:** `bug:rebuild-peers-quit-no-restart`
**Prior:** `.agents/reports/impl-rebuild-not-restarting-all-processes-2026-08-09.md`

## Operator report (fact)

After multi-process `/rebuild` work, overall better, but **peers shut down and
do not all come back** on the new binary. This used to work well for the
invoker / leader path; multi-process SIGUSR1 made peers quit without a reliable
re-exec.

## Intended flow

1. Install (`just install` / cargo fixed argv) → `~/.cargo/bin/grok-oss`
2. Soft-signal leaders (`RelaunchForUpdate`) → drain → re-exec leader
3. Write `$GROK_HOME/rebuild_relaunch_request.json`
4. `SIGUSR1` every other live product PID in `active_sessions`
5. Each peer: arm `rebuild_relaunch` → graceful quit → terminal restore →
   `exec` same session onto installed binary
6. Invoker: TaskResult path arms self re-exec (not SIGUSR1)

## Root cause (plain English)

Two bugs stacked:

### 1. Leader disconnect wins the race (main failure)

Event loop `tokio::select!` is **biased**. Leader IPC cancel is listed **above**
quit-notify:

```text
connection_cancel  →  break   // no rebuild_relaunch arm
quit_notify        →  take SIGUSR1 flag, arm, Quit, break
```

`/rebuild` signals leaders **before** peers. Leaders accept `RelaunchForUpdate`,
drain, then drop the client IPC. Peers also get `SIGUSR1` (flag + quit notify).

When both are ready, **connection_cancel runs first** and breaks the loop
**without** reading the rebuild request or setting `rebuild_relaunch`. The peer
process exits cleanly. No re-exec. Window stays dead.

That matches: instances shut down, not all restart.

### 2. SIGUSR1 arm gates too strict for same-commit rebuild

Even on the quit-notify path, arm used identity/path gates. Same package + same
git SHA (common dogfood rebuild) needs a deleted-inode or different path. When
gates failed, the code still quit (`Action::Quit`) with **no** re-exec.

Operator contract: **SIGUSR1 means re-exec**, not “maybe quit only.”

## What used to work

| Path | Before multi-process cut | After multi-process (buggy) | After this fix |
|------|--------------------------|-----------------------------|----------------|
| Invoker `/rebuild` | re-exec | re-exec | re-exec |
| Leaders | `RelaunchForUpdate` re-exec | same | same |
| Other TUI peers | stayed up on old binary | SIGUSR1 → quit, often **no** re-exec | arm on disconnect + force arm on SIGUSR1 → re-exec |

## Fix

### Event loop (`event_loop.rs`)

- **Leader disconnect:** call `arm_peer_rebuild_before_exit(..., LeaderDisconnect)`
  before break (covers cancel-vs-SIGUSR1 race + request file without flag).
- **Quit notify:** call `arm_peer_rebuild_before_exit(..., SignalOrFlag)`.
- **Loop end safety net:** drain leftover SIGUSR1 flag only (no opportunistic
  re-exec on plain `/exit` / SIGTERM).

### Peer arm (`dispatch/rebuild.rs`)

- `peer_rebuild_relaunch_if_applicable(..., signaled: bool)`:
  - `signaled == true`: fresh request + exe exists + session id is enough
    (skip identity/path anti-thrash).
  - `signaled == false`: keep identity/path gates (opportunistic leader path).
- `try_arm_peer_rebuild_relaunch_from_request(app, signaled)` with warn logs
  when SIGUSR1 cannot arm.
- `arm_peer_rebuild_before_exit(app, reason)` single chokepoint.

### Update crate (`rebuild.rs`)

- `peer_rebuild_request_is_actionable` shared freshness check (empty identity /
  age > 15 min).

## Tests (red → green)

| Test | Contract |
|------|----------|
| `peer_rebuild_signaled_arms_even_when_identity_and_path_equal` | SIGUSR1 forces re-exec on same-commit rebuild |
| `peer_rebuild_signaled_skips_stale_request` | Stale request still ignored when signaled |
| `peer_rebuild_request_is_actionable_when_fresh` | Freshness helper |
| Existing identity/path/deleted-inode tests | Unchanged opportunistic gates |

```bash
cargo test -p xai-grok-update --lib rebuild::
# 26 passed; exit 0

cargo test -p xai-grok-pager --lib dispatch::rebuild::
# 14 passed; exit 0

cargo fmt -p xai-grok-update -p xai-grok-pager
# exit 0

cargo clippy -p xai-grok-update --all-targets -- -D warnings
# exit 0

cargo clippy -p xai-grok-pager --lib -- -D warnings
# exit 0
```

## Dogfood steps

1. Open several product windows (different repos / sessions) so
   `~/.grok/active_sessions.json` lists multiple live PIDs.
2. From one window: `/rebuild` (or `grok-oss rebuild` from a checkout).
3. Expect:
   - Invoker re-execs onto new binary (same session).
   - Other windows receive SIGUSR1 (and/or leader disconnect), arm re-exec,
     come back with same session on `~/.cargo/bin/grok-oss`.
4. Check:

```bash
# After rebuild settles: no multi-pid lag on deleted images for sessions
# that were registered at rebuild time
ls -l /proc/*/exe 2>/dev/null | rg 'grok-oss'
cat ~/.grok/active_sessions.json
# Each live window cmdline should look like rebuild re-exec:
#   grok-oss --resume <session> --fullscreen   (or --minimal)
# not a dead terminal with no process
```

5. Same-commit rebuild (no version bump): peers should still re-exec (signaled path).

## Limits / not claimed

- Windows: SIGUSR1 still maps to forceful terminate; no graceful peer re-exec there.
- PIDs not in `active_sessions` are not signaled.
- If request file missing and no SIGUSR1 flag, leader disconnect alone only
  arms when identity/path gates say the process is behind.
- Official SpaceXAI `grok` binary out of scope.

## Key paths

| Concern | Path |
|---------|------|
| Peer arm + exit chokepoint | `crates/codegen/xai-grok-pager/src/app/dispatch/rebuild.rs` |
| Leader cancel + quit notify | `crates/codegen/xai-grok-pager/src/app/event_loop.rs` |
| Request freshness helper | `crates/codegen/xai-grok-update/src/rebuild.rs` |
| SIGUSR1 flag | `crates/codegen/xai-grok-pager/src/app/signal_handler.rs` |

## No git

No `git add` / commit / push (operator-only).
