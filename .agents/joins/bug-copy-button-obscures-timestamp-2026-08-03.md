# Join: copy button overlaps message timestamps (2026-08-03)

## Root cause (overlap, not truncation)

Always-on bubble ⧉ (`render_bubble_copy_buttons` in `viewer.rs`) painted on the
**absolute right edge** of the message content area. The yellow/gray message
timestamp is right-aligned into that same edge (short form and expanded
`HH:mm:ss | MMM DD` on hover). Copy paints **after** the entry, so ⧉ covered
the last timestamp character(s). "Aug 0" was the copy control / overlap reading
as a zero, not a day-format bug.

## Fix

When both timestamps and bubble_copy are on for user/assistant messages:

1. Reserve **2 extra** content columns (`BUBBLE_COPY_TRAILING_INSET`: gap + ⧉).
2. Right-align the timestamp to end **left of that inset** (shared path in
   `EntryRenderer` and sticky headers).
3. Keep ⧉ at the content right edge (unchanged hit placement).
4. Wrap/content-width reserve uses `message_right_chrome_reserve` (10 + inset).

Helpers (exported from `scrollback::wrappers`):
- `TIMESTAMP_SHORT_RESERVE` (10)
- `BUBBLE_COPY_TRAILING_INSET` (2)
- `message_right_chrome_reserve` / `bubble_copy_trailing_inset`

Timestamp **format strings** unchanged.

## Tests

- **Red→green contract:**
  `bubble_copy_does_not_overlap_timestamp` (short + expanded hover; full
  timestamp string survives ⧉ paint; ⧉ still present at right edge).
- Updated existing timestamp position / content-width tests for trailing inset.
- Ran: all `bubble_copy_*`, timestamp short/expand/collapse, gutter, content-width.

## Files

- `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/wrappers/mod.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/render.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs`
- `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs`

## Acceptance

- Timestamp (time + date, short and expanded) fully readable with copy present
- Copy still paints and hits at content right edge
- Tests green; `cargo fmt -p xai-grok-pager` clean
- Root cause named as **overlap**, not truncation
