# bug: cargo fmt missing reparked mod (2026-08-11)

## Cause

1. **Missing mod target:** `tests/pty_e2e_persistence.rs` still declared
   `pty_e2e/reparked_wait_repushes_buried_marker.rs`, but that file is gone.
   Upstream renamed / replaced it with `reparked_wait_stays_markerless.rs` when
   stacked "Worked for" markers changed to markerless parks (one close marker
   per turn). The sibling test body is on disk; only the `#[path]` / `mod`
   lines were stale after onto mop. Restoring the deleted *repushes buried
   marker* body would fight the current product contract and the existing
   `reparked_wait_stays_markerless` coverage.

2. **rustfmt diffs:** unformatted (or mop-dirtied) sources in sampling-types,
   test-support, and tools packages blocked `cargo fmt --all -- --check`.

## Fix

| Action | Detail |
|--------|--------|
| **Mod rewired** (not file restored) | Point persistence harness at existing `reparked_wait_stays_markerless.rs` |
| **Format** | `cargo fmt --all` (edition 2024 workspace) |

## Files changed

- `crates/codegen/xai-grok-pager/tests/pty_e2e_persistence.rs` — mod path:
  `reparked_wait_repushes_buried_marker` → `reparked_wait_stays_markerless`
- `crates/codegen/xai-grok-sampling-types/src/error.rs` (fmt)
- `crates/codegen/xai-grok-sampling-types/src/lib.rs` (fmt)
- `crates/codegen/xai-grok-test-support/src/env.rs` (fmt)
- `crates/codegen/xai-grok-tools/src/computer/local/terminal.rs` (fmt)
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/bash/mod.rs` (fmt)

## Why not restore the deleted test body

- History: present under old names through `c368b4d7`; monorepo sync
  `47348d13` / rename path landed `reparked_wait_stays_markerless.rs` and
  switched the mod there. Current tree already has that file and the matching
  endline park/wakeup mods.
- Old contract asserted re-park **repushes** a buried marker; new contract is
  **markerless** parks + single turn-end marker. Fake empty green or resurrected
  repush test would be wrong.

## Verify

```bash
cargo fmt --all -- --check
```

**Exit code: 0**
