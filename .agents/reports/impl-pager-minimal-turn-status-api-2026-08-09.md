# Fix: `xai-grok-pager-minimal` turn_status API wire-up

**Date:** 2026-08-09
**Scope:** Surgical compile fix only (Work B pause/stop `global_paused` field).

## Problem

After Work B added process-level global pause to turn status:

- `turn_status::should_show` takes a 6th arg: `global_paused: bool`
- `TurnStatusArgs` requires field `global_paused: bool`

`xai-grok-pager-minimal/src/live.rs` still used the old 5-arg / field set, so `just check` / clippy failed with E0061 and E0063.

## Change

File: `crates/codegen/xai-grok-pager-minimal/src/live.rs` (`render_minimal_status`)

- Pass `global_paused` into `should_show` and `TurnStatusArgs`.
- Value is **`false`**.

### Why not full-pager semantics

- Full pager uses `self.global_work_paused` (set from `AppRenderParams::global_paused`).
- That field is **`pub(crate)`** on `AgentView`, so the minimal crate cannot read it.
- Minimal is already keyboard-only (`buttons: None`), so pause/resume mouse chrome is suppressed anyway.
- Wiring real pause state would need a public accessor / `minimal_api` seam; out of scope for this wire-up break.

Short comment at the call site documents the choice.

## Other call sites

Workspace grep of `should_show` / `TurnStatusArgs` for the turn-status API:

| Location | Status |
|----------|--------|
| `xai-grok-pager/src/app/agent_view/render.rs` | Already passes `self.global_work_paused` |
| `xai-grok-pager/src/views/turn_status.rs` (tests) | Already pass `false` / set `global_paused` |
| `xai-grok-pager-minimal/src/live.rs` | **Fixed this turn** |

No other product call sites of this API were broken.

## Verify

```text
cargo fmt -p xai-grok-pager-minimal
cargo check -p xai-grok-pager-minimal          # ok
cargo clippy -p xai-grok-pager-minimal --lib -- -D warnings   # ok
cargo clippy -p xai-grok-pager-minimal --all-targets -- -D warnings  # ok
```

No new unit test: pure compile wire-up; full pager already owns pause chrome contracts.

## Not done

- Exposing `global_work_paused` to minimal for idle-under-pause resume row visibility.
- Any pause/stop behavior redesign.
