# Clippy: enroll std spawns in `xai-grok-pager-render`

**Date:** 2026-08-11
**Package:** `xai-grok-pager-render`
**Status:** fixed; clippy green

## Problem

`cargo clippy -p xai-grok-pager-render --all-targets -- -D warnings` failed on three disallowed `std::process::Command::spawn` sites (unenrolled children can outlive the session):

1. `src/clipboard/mod.rs` — `write_tmux_buffer` (`tmux load-buffer -`)
2. `src/link_opener.rs` — `open_url` (`open` / `xdg-open` / `cmd`)
3. `src/link_opener.rs` — `open_path` non-Windows path (`open` / `xdg-open`)

Also hit on test target: `src/glyphs.rs` `manual_range_contains` in `cursor_box_blink_alternates_filled_and_hollow`.

## Approach

Match established std enroll (tokio `ProcessScope::enroll` is not available for `std::process::Child`):

- `ProcessGroup::new` → `attach_std` → `Arc` → `global_process_scope().register`
- `#[allow(clippy::disallowed_methods)]` only on the enrolled spawn line, not file-wide

Patterns referenced: `xai-grok-tools` implement_memory workspace, `xai-grok-workspace` envrc, pager notification hooks, same-crate `tmux_probe`.

Package already depended on `xai-tty-utils`; no Cargo.toml change.

## Changes

### `clipboard/mod.rs` — waited short-lived helper

After spawn, enroll in the global scope. On closed scope, kill + wait and fail. Bounded wait via existing `wait_with_deadline`; drop the group `Arc` after the child is reaped so the scope's `Weak` cannot `killpg` a recycled PID.

### `link_opener.rs` — fire-and-forget OS helpers

Added `spawn_enrolled_os_helper`:

1. Spawn with site-local allow
2. Attach + register (kill if scope already closed)
3. Background thread `wait`s the child while holding the group `Arc` (so `kill_all` can still upgrade the `Weak`, and Windows job-object drop does not kill a still-running helper)

Wired into `open_url`, non-Windows `open_path`, and Windows `reveal_in_explorer` (same pattern; not in the original three-site list but same API surface).

### `glyphs.rs`

`(500..=800).contains(&half)` for the caret blink half-period assert.

## Verify

```bash
cargo fmt -p xai-grok-pager-render
cargo clippy -p xai-grok-pager-render --all-targets -- -D warnings
```

**Exit code:** `0`
(Only pre-existing workspace warning: `clippy.toml` tokio spawn path not reachable in this package's dependency graph.)
