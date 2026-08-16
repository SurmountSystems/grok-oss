# Process mop — F9 TUI screenshot

**Date:** 2026-08-13  
**Repo:** `/home/hunter/Projects/surmount/grok-build`  
**Role:** process mop after `.agents/reports/bug-f9-screenshot-unbound.md`

No F9-slice product edits. `views/prompt_widget/**`, `settings/**`, `settings_writes.rs`, and `render.rs` were not touched. No git add / commit / push.

## Commands and exit codes

| Step | Command | Exit |
|------|---------|------|
| fmt | `cargo fmt -p xai-grok-pager` | **0** |
| clippy (preferred mop target) | `CARGO_TARGET_DIR=/tmp/grok-oss-f9-mop-target cargo clippy -p xai-grok-pager --lib -- -D warnings` | **101** (`No space left on device` writing fingerprints under `/tmp`) |
| clippy (implementer warm target, first) | `CARGO_TARGET_DIR=/tmp/grok-oss-f9-target cargo clippy -p xai-grok-pager --lib -- -D warnings` | killed on wait (compile still in `xai-grok-pager`) |
| clippy (implementer warm target, resume) | same | **101** (`No space left on device` writing `xai-grok-shell` incremental query cache) |
| clippy (workspace target, `/tmp` 100%) | `cargo clippy -p xai-grok-pager --lib -- -D warnings` | **101** (tools-api `build.rs` / `protoc` could not write `/tmp/.tmp*/debug-redact.pbbin`) |
| clippy (workspace + `TMPDIR` on home disk) | `TMPDIR=/home/hunter/.cache/tmp-f9-mop cargo clippy -p xai-grok-pager --lib -- -D warnings` | **101** (4 compile errors, none in F9-slice files) |
| tests (workspace + `TMPDIR` on home disk) | `TMPDIR=/home/hunter/.cache/tmp-f9-mop cargo test -p xai-grok-pager --lib -- capture_tui_screenshot_bound_to_f9_always try_attach_tui_screenshot_for_plan_when_approval_open try_attach_tui_screenshot_skips_when_no_plan_approval` | **101** (2 compile errors, none in F9-slice files) |

Preferred mop target `/tmp/grok-oss-f9-mop-target` is not usable: `/tmp` is a 45G tmpfs that sat at 89–100% during this mop. After the first ENOSPC, the finished implementer leftover `/tmp/grok-oss-f9-target` (5.1G) was removed so other writers could keep going. Live writer targets (`grok-oss-caret-target`, `grok-settings-rows-target`, `grok-oss-pause-chips-target`, and the other mop dirs) were left alone.

## Clippy errors (not F9 slice)

From `TMPDIR=... cargo clippy -p xai-grok-pager --lib -- -D warnings`:

1. `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs:533` — `ActionId::ToggleGlobalPause` missing. **Do not edit** (`render.rs` is the live pause-chip writer).
2. `crates/codegen/xai-grok-pager/src/app/dispatch/dashboard.rs:1403` — `PagerLocalSnapshot` missing `auto_compact_threshold_percent`, `auto_compact_threshold_tokens`, `features_session_recap`, and 2 other fields. Not an F9 file. Settings writer is live.
3. `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs:551` — same `PagerLocalSnapshot` initializer. Not an F9 file.
4. `crates/codegen/xai-grok-pager/src/views/agent.rs:1240` — `ActionId::ToggleGlobalPause` missing. Not an F9 file.

## Test compile errors (not F9 slice)

From the named `cargo test -p xai-grok-pager --lib` filter (same two `PagerLocalSnapshot` sites). Tests never linked; the three named F9 tests did not run.

`ToggleGlobalPause` did not appear on the test compile (clip vs test race with the pause-chip writer is possible). The settings snapshot errors did.

## F9-slice files (no mop)

Allowed mop paths from the implementer report, left unchanged:

- `crates/codegen/xai-grok-pager/src/actions/`
- `crates/codegen/xai-grok-pager/src/app/app_view.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/input.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs`
- `crates/codegen/xai-grok-pager/src/app/event_loop.rs`
- `crates/codegen/xai-grok-pager/src/views/dashboard/state.rs`

No fallout in those files. The red compile is mid-flight work in pause-chip and remaining-settings.

## Result

- **fmt:** clean (exit 0).
- **clippy / named tests:** not green on this tree. Failures are outside the F9 slice. Re-run after those writers land.

Stop.
