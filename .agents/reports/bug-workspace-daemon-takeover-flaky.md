# Flake glance: `take_over_declines_when_lock_is_never_released`

## What the test encodes

`xai-grok-workspace-daemon` `daemonize::tests::take_over_declines_when_lock_is_never_released` (Linux only) encodes this contract:

1. A predecessor workspace-server stand-in (`sleep`) is named in the pidfile.
2. The advisory `flock` is still held by someone else (an in-process `PidFile` guard that is never dropped).
3. `PidFile::acquire_or_take_over_matching` must still terminate that predecessor.
4. It must then **decline** (`Ok(None)`). It must not proceed without the lock, and it must not rewrite the pidfile.

Product path: `PidFile::acquire_or_take_over` → `acquire_or_take_over_matching` in `crates/codegen/xai-grok-workspace-daemon/src/daemonize.rs`. After SIGTERM grace and SIGKILL grace, if `poll_acquire` still cannot win the flock, it returns `None`. Identification of the predecessor is `PredecessorTarget::open`, which requires `/proc/<pid>/cmdline` argv0 basename to match the injected fragment (`"sleep"` in this test).

## Run results

Env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-build-target`, `TMPDIR=/home/hunter/.cache/grok-oss-tmp`.

| Command | Result |
|---------|--------|
| `cargo test -p xai-grok-workspace-daemon --lib daemonize::tests::take_over_declines_when_lock_is_never_released -- --exact --nocapture` | 1 pass (first compile + run, 1.31s; logs show SIGTERM wait, SIGKILL, decline) |
| Same test binary, 20 sequential repeats (before harden) | 20 pass, 0 fail |
| Same test binary, 8 concurrent copies (before harden) | 8 pass, 0 fail |
| Tight host probe: spawn `sleep 30` and immediately read `/proc/<pid>/cmdline` | **2 misses in 3000** (empty cmdline `[]`) |
| After harden: `cargo test -p … --lib daemonize::tests::take_over` | 7 pass (all takeover tests) |
| After harden: same test binary, 25 sequential repeats of the named test | 25 pass, 0 fail |
| After harden: sibling fixture users (`take_over_declines_non_matching_holder`, `take_over_acquires_cleanly_when_predecessor_releases`, `predecessor_target_pins_verifies_and_signals`) | 3 pass |
| `cargo clippy -p xai-grok-workspace-daemon --all-targets -- -D warnings` | pass |
| `cargo fmt -p xai-grok-workspace-daemon` | applied |

The named cargo test did **not** fail locally (53 named-test runs green: 1 + 20 + 8 + 1 + 25). The flake is still real: empty `/proc` cmdline right after `spawn` is observed, and that is exactly the product gate.

If cmdline is empty, `process_name_matches` is false, `PredecessorTarget::open` returns `None`, takeover declines **without signaling**, and the test fails on `"the predecessor must be terminated"`. The sibling test `predecessor_target_pins_verifies_and_signals` already documents this: `/proc/<pid>/cmdline` can lag after spawn under remote CI.

## Whether implemented, and why

**Implemented.** Cheap fixture harden. The lock contract is unchanged.

`spawn_predecessor` now waits until `process_name_matches(pid, "sleep")` (2s bound, 10ms poll) before returning. That is a condition wait for the same identity check the product uses, not a longer takeover grace. The in-process flock still stays held; a declined takeover still must not rewrite the pidfile.

No product lock/takeover timing was changed (`TAKEOVER_GRACE`, `TAKEOVER_KILL_GRACE`, `TAKEOVER_POLL` untouched).

## Files changed

- `crates/codegen/xai-grok-workspace-daemon/src/daemonize.rs`
  - `spawn_predecessor` waits for a live `"sleep"` cmdline match.
  - New test-only helper `wait_until_process_name_matches`.

No red/green of the named cargo test locally (it was already green here). Evidence for the harden is the 2/3000 empty-cmdline probe plus the existing sibling comment, not a local fail of this test.
