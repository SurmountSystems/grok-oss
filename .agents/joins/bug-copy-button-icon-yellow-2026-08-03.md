# Join: copy button icon consistent yellow (2026-08-03)

## Outcome

Idle always-on bubble `⧉` and selection-box copy/view chrome now paint with
`theme.gray` (secondary informational chrome) instead of
`theme.selection_border` (bright white on DOGE).

On DOGE, `gray` is pure yellow (same token as timestamps and draft/plan `⧉`).
Hover still brightens to `text_primary`. Timestamp non-overlap is unchanged.

## Paint path

| Site | Before | After |
|------|--------|--------|
| `render_bubble_copy_buttons` | `selection_border` | `gray` |
| `render_selection_buttons` | `selection_border` | `gray` |
| Prompt draft `⧉` / plan top-bar `⧉` | already `gray` | no change |

File: `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs`

## Why `theme.gray`

DOGE semantic roles: yellow is dates/times/secondary chrome. Timestamps already
use `theme.gray`. Draft and plan copy buttons already use `theme.gray`. Bubble
and selection copy were the odd white-border outliers.

## Tests

```bash
cargo fmt -p xai-grok-pager
cargo test -p xai-grok-pager --lib bubble_copy
```

12 passed, including:

- **new** `bubble_copy_idle_uses_secondary_gray_not_white_border` (DOGE hermetic:
  idle `fg == theme.gray`, not `selection_border` / `text_primary`; hover →
  `text_primary`)
- existing `bubble_copy_does_not_overlap_timestamp` still green

## Out of scope

- No full `just check`
- No git add/commit
- Prompt/plan copy already matched; not re-touched
