# pager-minimal dim thinking rail + plan insert APIs (2026-08-12)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer
**Blocked by:** `.agents/reports/bug-non-shell-oneshots-2026-08-12.md` § `xai-grok-pager-minimal`

## Summary

Restored two public pager APIs that a half-merge had dropped, unblocking
`xai-grok-pager-minimal` compile and greening the dim thinking-rail contract.

| API | Location | Callers |
|-----|----------|---------|
| `EntryRenderer::with_dim_accent` | `xai-grok-pager` `scrollback/wrappers/entry_renderer.rs` | minimal `commit.rs` `minimal_renderer` |
| `ScrollbackState::insert_block_before` | `xai-grok-pager` `scrollback/state/mod.rs` | minimal `plan.rs` + plan commit tests |

**Live:** `cargo test -p xai-grok-pager-minimal --all-targets` → **86 passed, 0 failed**.
Includes `committed_thinking_paints_a_dim_rail_in_column_zero` and plan
anchor insert tests.

## Compile errors (before)

```
error[E0599]: no method named `with_dim_accent` … EntryRenderer
error[E0599]: no method named `insert_block_before` … ScrollbackState  (×5)
```

## Product fix

### 1. `with_dim_accent` (dim thinking rail)

Lost in an earlier pager rewrite; restored from pre-removal behavior:

- Field `dim_accent: bool` (default `false`).
- Builder `with_dim_accent(self, dim: bool)`.
- Helper `accent_paint_style` adds `Modifier::DIM` on solid accent branches
  (pending freeze, animated wave, collapsed, static). Striped rails unchanged.
- Height-neutral: does not change `chrome_width`.

Minimal mode always sets `.with_dim_accent(true)` so a thinking rail that
resolves to `Color::Reset` under terminal-native palette stays chrome, not
full-brightness default fg.

### 2. `insert_block_before` (plan body above parked tool)

Public method on `ScrollbackState`:

- Inserts a finalized block immediately before `anchor` via `IndexMap::shift_insert`.
- Missing anchor → falls back to `push_block`.
- `debug_assert` that anchor is not already committed (native scrollback is append-only).
- Arms structural scroll anchor; applies same Edit default + always-expand-thinking
  materialize policy as `push`; bumps selection and clamps `commit_scan_cursor`
  to the insert index; rebuilds turns / invalidates layout.

Used when plan approval commits the plan body above a still-running
`exit_plan_mode` tool row so the plan prints before the tool frontier.

## Verify

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager-minimal --all-targets
# 86 passed; 0 failed

nice -n 19 ionice -c3 cargo fmt -p xai-grok-pager -p xai-grok-pager-minimal
nice -n 19 ionice -c3 cargo check -p xai-grok-pager --lib
# ok (lib test binary measures 8824 unit tests; no new compile break)
```

## Files touched

- `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/state/mod.rs`

No pager-minimal product rewrites; tests stayed as the contract.
