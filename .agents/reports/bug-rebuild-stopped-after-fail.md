# Process-wide `/rebuild` after a failed `just install`

**Date:** 2026-08-13  
**Crates:** `xai-grok-update`, `xai-grok-pager`  
**Operator sequence:** `/rebuild` ran `just install`; release compile and copy of `~/.cargo/bin/grok-oss` succeeded; verify `grok-oss --version` failed with ENXIO (os error 6) at justfile line 386; then “everything’s stopped” and process-wide rebuild looked broken.

`--version` dispatch was already fixed in **source** (`.agents/reports/bug-install-verify-enxio.md`). This slice does **not** redo that flag work. The installed binary on disk is still the pre-fix copy until the next successful install.

---

## Named contracts

1. If `just install` / `--version` verify fails, `/rebuild` reports the failure in this session and must not leave other grok-oss sessions permanently stopped.
2. Process-wide rebuild only replaces or re-execs peers after a successful install (or an explicit already-installed relaunch-only path if one exists). There is no “relaunch only, skip install” path in this crate; fleet replace is gated on install + verify success.
3. After a failed rebuild, `/rebuild` is still invokable (progress bar cleared; `RebuildAndRelaunch` still emits `RunRebuild`).
4. Do not Unlock the keyring. Do not kill live TUIs from the implementer.

---

## Hypothesis: SIGUSR1 before install

**False in current source.**

`rebuild_and_relaunch_with_progress` in `crates/codegen/xai-grok-update/src/rebuild.rs`:

1. Resolve source root and start the progress bar.
2. Spawn blocking `just install` (`run_command_captured`: stdin `/dev/null`, stdout/stderr piped, `setsid`, no controlling TTY).
3. On install `Err`, **return immediately**. No leader signal. No request file. No peer `SIGUSR1`.
4. On install `Ok`, run `verify_installed_identity` (`binary --version`, parse identity). On verify `Err`, **return immediately** with context “not signaling peers or leaders.”
5. Only then `RebuildFleetPlan::after_install(true)` → soft-signal leaders (`RelaunchForUpdate`), write `rebuild_relaunch_request.json`, `SIGUSR1` other live product windows.
6. Invoker arms self re-exec in the pager (`handle_rebuild_done` Ok path) and quits to `exec` the new binary.

So a failed verify, like today’s ENXIO, does **not** ask peers to quit. This invoker stays in the TUI (fail path returns empty effects, no quit).

The operator still saw a stopped fleet. That is a different hole (below), plus leftover stopped peers from an earlier successful signal without a listener.

---

## Root cause (what actually left sessions dead)

Three restack losses plus one swallowed verify:

### 1. Default `SIGUSR1` = terminate

Unix default for `SIGUSR1` is terminate. The cooperative listener (`mark_peer_rebuild_relaunch_from_sigusr1` + graceful quit) was missing from `signal_handler.rs`. After a **successful** process-wide rebuild, peers died on the signal and never armed re-exec.

That matches prior work `bug:rebuild-peers-quit-no-restart`. It also explains “everything’s stopped” after any successful signal from a newer leader into an older peer that still has no handler.

Today’s failed install should **not** have sent that signal. Peers that were already dead from an earlier successful `/rebuild` stay dead until someone starts them again.

### 2. Event loop did not arm re-exec on the way out

Even with the flag, the loop used to quit on leader IPC cancel (biased `select!` above quit-notify) or SIGUSR1 quit-notify **without** calling `arm_peer_rebuild_before_exit`. Peers exited cleanly and never came back.

Restored:

- `connection_cancel` → `arm_peer_rebuild_before_exit(LeaderDisconnect)`
- `quit_notify` → `arm_peer_rebuild_before_exit(SignalOrFlag)` then `Quit`
- `finish_run` safety net + `RunResult.rebuild_relaunch`

### 3. Quit tail did not `exec` the new binary

`app::run` after restore used to ignore `rebuild_relaunch`. `post_restore_relaunch_action` now prefers rebuild exec (or a blocked hint if restore failed) over screen-mode relaunch.

### 4. Swallowed `--version` verify used to still signal

`verify_installed_identity` used to fall back to a cargo-format identity and continue into fleet replace. A binary that cannot print `--version` (ENXIO / TUI start under captured stdio) would still SIGUSR1 peers onto that copy.

That fallback is gone. Verify is a hard gate. Fail returns `Err` before `RebuildFleetPlan::after_install(true)`.

---

## Signal order (success vs fail)

| Step | Success | Install or verify fail |
|------|---------|------------------------|
| `just install` | copy + recipe verify | `Err`, return |
| `verify_installed_identity` | parse identity | `Err`, return (no fallback) |
| `RebuildFleetPlan` | both flags true | both flags false |
| Leader `RelaunchForUpdate` | yes | no |
| Request file + peer `SIGUSR1` | yes | no |
| Invoker `handle_rebuild_done` | toast + arm `rebuild_relaunch` + quit | toast + system block, **stay in session** |
| `/rebuild` again | N/A (re-exec) | progress cleared; `RunRebuild` still emitted |

There is no “already installed, relaunch only” skip in this path.

---

## What today’s operator run did

1. Invoker `/rebuild` compiled release (~17m), stripped, copied `~/.cargo/bin/grok-oss`.
2. Recipe `grok-oss --version` (justfile 386) failed ENXIO because the **installed** binary still starts the TUI on `--version`. Source dispatch is fixed; the file on disk is not.
3. Current `rebuild_and_relaunch_with_progress` returns `Err` here. This session should have stayed up with a fail toast. Peers should not have been signaled **this** run.
4. “Everything stopped” is still real if:
   - older installed peers died on default `SIGUSR1` from a **previous** successful process-wide rebuild, and
   - this session sat idle after the fail (no relaunch; progress bar would have stuck if fail handling did not clear it).

This slice: fail does not signal; fail reports and clears progress; peers that **are** signaled later re-exec instead of dying.

---

## Product changes (smallest)

**`xai-grok-update`**

- `RebuildFleetPlan::{after_install, should_replace_fleet}`
- `orchestrate_order_on_install_result(install_ok, leader, peer)` — three-arg test helper; fail does not mark signals
- `rebuild_and_relaunch_with_progress`: install/verify `Err` returns before fleet; success gated on `plan.signal_leaders` / `plan.write_request_and_signal_peers`
- No cargo-identity fallback after verify fail

**`xai-grok-pager`**

- Fail toast/scrollback: “Rebuild failed (no other sessions were asked to quit or re-exec)”
- Clear `rebuild_progress` on fail so `/rebuild` is not stuck
- `post_restore_relaunch_action` + `app::run` exec/block
- `RunResult.rebuild_relaunch`; `finish_run` copies it; skip exit-timeout when set
- SIGUSR1 listener sets the peer flag and graceful-quits (does not default-terminate)
- Event-loop cancel/quit + `finish_run` call `arm_peer_rebuild_before_exit`

Did not Unlock the keyring. Did not kill live TUIs.

---

## TDD

### Red (before product)

Compile-red, then one assert miss:

1. `cargo test -p xai-grok-update --lib rebuild::failed_install`  
   `E0061` `orchestrate_order_on_install_result` 3 args vs 2; `E0433` `RebuildFleetPlan` missing.

2. Pager tests: missing `post_restore_relaunch_action` / `PostRestoreRelaunch`; `RunResult` had no `rebuild_relaunch`; missing `mark_peer_rebuild_relaunch_from_sigusr1`.

3. `app/mod.rs` `E0382`: `if let Err(cleanup_error)` moved `restore_result`. Fixed with `ref`.

4. `handle_rebuild_done_failure_reports_and_does_not_relaunch` failed: asserted `"not asked to quit"` which is not a substring of `"were asked to quit"`. Assert changed to `"asked to quit"` (same contract).

### Green (same filters)

```
cargo fmt -p xai-grok-update -p xai-grok-pager
cargo clippy -p xai-grok-update --lib --bins -- -D warnings   # exit 0
cargo clippy -p xai-grok-pager --lib --bins -- -D warnings    # exit 0

cargo test -p xai-grok-update --lib --offline rebuild::
# 27 passed, including failed_install_must_not_replace_or_signal_peers
# and build_fail_does_not_signal_leaders

cargo test -p xai-grok-pager --lib --offline dispatch::rebuild
# 17 passed, including
# handle_rebuild_done_failure_reports_and_does_not_relaunch
# rebuild_still_invokable_after_failed_rebuild_done
# post_restore_prefers_rebuild_relaunch_only_when_armed

cargo test -p xai-grok-pager --lib --offline finish_run_carries_rebuild_relaunch_when_armed
# 1 passed

cargo test -p xai-grok-pager --lib --offline sigusr1_sets_peer_rebuild_flag_once
# 1 passed
```

---

## Leftover honesty

- **`~/.cargo/bin/grok-oss` is still the pre-`--version` binary.** The next `/rebuild` or `just install` will compile the source fix and then verify can pass. Until then, recipe line 386 will fail the same way if you install from this tree with the old binary as the verify target after copy (copy replaces it with a newly built binary that has the flag fix). A rebuild **from current source** should install a binary that prints `--version` without a TTY.
- Peers that already exited on default `SIGUSR1` will not come back by themselves. This session can `/rebuild` again after a successful install; dead windows need a manual start.
- Running **this** TUI is still the old installed inode until a successful rebuild re-execs it. The source in the working tree has the SIGUSR1 listener and fail-does-not-signal order; the process in memory may not until relaunch.
- No explicit “already installed, relaunch only” slash path was added. Not invented.
- `just install` recipe verify and `--version` dispatch were not re-done here; see the ENXIO report.

---

## Files

- `crates/codegen/xai-grok-update/src/rebuild.rs`
- `crates/codegen/xai-grok-update/src/lib.rs` (export `RebuildFleetPlan`)
- `crates/codegen/xai-grok-pager/src/app/dispatch/rebuild.rs`
- `crates/codegen/xai-grok-pager/src/app/event_loop.rs`
- `crates/codegen/xai-grok-pager/src/app/signal_handler.rs`
- `crates/codegen/xai-grok-pager/src/app/mod.rs`
