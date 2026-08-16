# Mop plan extract: human-message bubble copy click

Source: `.agents/reports/bug-copy-human-message.md` and `.agents/reports/bug-copy-human-message-impl.md`.

## Files changed (crate names)

Crate: **`xai-grok-pager`** only.

- `crates/codegen/xai-grok-pager/src/scrollback/types.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/blocks/mod.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/selection.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/render.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `crates/codegen/xai-grok-pager/src/app/mouse.rs`

## Product `*.rs` edited

**Yes.** Process mop must run.

## Test filter used

Primary contract:

```
cargo test --offline -p xai-grok-pager --lib -- clicking_human_bubble_copy
```

Named test: `app::mouse::tests::clicking_human_bubble_copy_copies_the_prompt`

Related (implementer, green): `clicking_human_bubble_copy` / `bubble_copy_` (paint on/off plus click). Also `block_line_exhaustive_literal_keeps_legacy_shape`.

## fmt / clippy already run? Exit codes listed

| Step | Command | Exit (from implementer) |
|------|---------|-------------------------|
| fmt | `cargo fmt -p xai-grok-pager` | 0 |
| clippy lib | `cargo clippy --offline -p xai-grok-pager --lib -- -D warnings` | 0 |
| clippy all-targets | `cargo clippy --offline -p xai-grok-pager --all-targets -- -D warnings` | 101 |
| contract red | same test filter, click path disabled | 101 |
| contract green | same filter after restore | 0 |
| related | `clicking_human_bubble_copy bubble_copy_` | 0 |

Implementer claimed clippy `--all-targets` red is pre-existing (bench `needless_range_loop`, clear-finished `expect(&format!(...))`, diagnostics `Path::canonicalize`, clear-finished `0 + 40 - 1`). Product `--lib` clippy was clean. Mop still re-runs fmt, `--all-targets` clippy, and targeted tests.
