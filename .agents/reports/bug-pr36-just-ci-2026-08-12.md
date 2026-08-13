# PR #36 `just ci` fail on restack tip

Date: 2026-08-12. Branch: `onto-xai/b13fa526f511`. Shared cwd. No worktree. No GitHub write.

## Fail excerpt

- Check: `just ci` (job `quality` / display name `just ci`)
- Run: https://github.com/SurmountSystems/grok-oss/actions/runs/31657229971
- Job: https://github.com/SurmountSystems/grok-oss/actions/runs/31657229971/job/94314261283
- Head: `755521df88eab2da16b36e189b7ac0329e73c859`
- Duration: ~4m 11s. Stopped in `just ci-prep && just test` during `test-fmt`.

First product error:

```
==> cargo fmt --all -- --check
just cargo-ci cargo fmt --all -- --check
...
Diff in /home/runner/work/grok-oss/grok-oss/crates/codegen/xai-grok-pager-bin/src/main.rs:2305:
     return Ok(());
 }

-    let _ = (force_reinstall, version, channel_switch, trigger, base_update_config);
+    let _ = (
+        force_reinstall,
+        version,
+        channel_switch,
+        trigger,
+        base_update_config,
+    );
error: recipe `cargo-ci` failed with exit code 1
error: recipe `test-fmt` failed on line 291 with exit code 1
##[error]Process completed with exit code 1.
```

Nix cache restore/save warnings and Node 20 deprecation were present. They were not the fail.

## Root cause

`test-fmt` runs `cargo fmt --all -- --check` on rustfmt 1.97.1. After the 1.0.3 restack compile mop, `run_update_command` in `xai-grok-pager-bin` kept unused update args in one tuple so the no-auto-install path would still compile. That line was wider than rustfmt's wrap. CI never reached clippy or tests.

## Files changed

- `crates/codegen/xai-grok-pager-bin/src/main.rs`: wrap the unused-binding tuple the way rustfmt 1.97.1 wants. No behavior change.

Nucleo contracts were not touched. rustc / fenix 1.97.1 pin unchanged.

## Local command + exit

```
cargo fmt --all -- --check    # exit 0 (rustc 1.97.1 / rustfmt 1.9.0-stable 8bab26f4f)
./scripts/assert-process-pins.sh HEAD    # OK: 24 files + 5 dirs, exit 0
```

## New tip + push

| Item | Value |
|------|--------|
| New tip | `4df59dac8e70ce339236350bf00288f1fe8adf47` |
| Tree | `7260b19c705b4a0ad81f585b59159ab5827f37df` |
| Parent | `755521df88eab2da16b36e189b7ac0329e73c859` |
| Commit path | `git add` the pager-bin file, `git write-tree`, `git commit-tree`, `git update-ref HEAD`. No `commit.gpgsign=false`, no `--no-gpg-sign`, no fake `gpg.program`. |
| Push | `git push origin onto-xai/b13fa526f511` ff: `755521df..4df59dac`. No force. No new PR. No `gh` write. |

## New CI run

Yes. `just ci` started on the new SHA.

- Run: https://github.com/SurmountSystems/grok-oss/actions/runs/31658516974
- Job: https://github.com/SurmountSystems/grok-oss/actions/runs/31658516974/job/94318181973 (`just ci`, in_progress at report time; checkout done, freeing disk)
- Run number 49, event `pull_request`
- Head SHA: `4df59dac8e70ce339236350bf00288f1fe8adf47`
