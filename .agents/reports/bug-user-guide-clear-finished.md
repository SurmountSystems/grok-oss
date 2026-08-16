# User-guide: Clear finished `[−]`

**Date:** 2026-08-13
**Tree:** `/home/hunter/Projects/surmount/grok-build`
**Product SoT:** `crates/codegen/xai-grok-pager/docs/user-guide/`
**Paint report:** `.agents/reports/bug-clear-finished-button-unpainted.md`

Docs only. No Rust. No new guide file. Did not edit `~/.grok/docs/user-guide/`.

The three named pages did **not** already name Clear finished `[−]`. This pass added complete American English thoughts. It did not bulk-rewrite those pages.

SuperGrok is paid. Existing `/limits` copy still says **included SuperGrok period limits**. This pass did not invent hop-after-period-full.

## Named contract (docs)

1. Compact **`[−]`** (U+2212 minus) in the todo header next to close when the board is open and finished rows exist, focused or unfocused.
2. Hidden board or no finished rows: no button.
3. Same action as `/clear-completed-todos` and optional focused `X`. Archives completed and cancelled rows. Not `h` hide-done. Not a `merge: false` wipe.
4. Hints still say **Clear finished**. The chrome is the compact minus, not the long words.
5. Quiet idle / stronger hover. Not Human green. Not agent magenta.
6. Tasks open/kill chrome wins z-order. Compact layout keeps one chrome row above the todo body.

## What changed

| Page | Added |
|------|--------|
| `03-keyboard-shortcuts.md` | Agent-level `X` when the todo pane is focused. A short Clear finished paragraph after the Agent-Level notes. A mouse bullet for click `[−]`. |
| `04-slash-commands.md` | `/clear-completed-todos` under Session Management. |
| `17-sessions.md` | Section **The session todo board** (`resources_state.json` live snapshot, `plan.json` fallback). One Tips line. |

Glyph in all three pages is U+2212 minus inside `[−]`, not ASCII `[-]`.

## Tests

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-ug-clear-finished-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
cargo --offline test -p xai-grok-pager --lib -- user_guide
```

Cold target needed a full compile (resumed). Final run:

**Exit code: 0**

```
running 3 tests
test docs::tests::user_guide_entries_are_valid ... ok
test docs::tests::user_guide_entries_have_no_duplicates ... ok
test docs::tests::default_howto_entries_includes_all_user_guide_docs ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 8857 filtered out
```

No new `USER_GUIDE` file. Disk still has 01–24 plus `README.md`.

## Did not

- Touch any `.rs`
- Edit `~/.grok/docs/user-guide/`
- Invent hop-after-period-full
- Call SuperGrok free
- `git add` / `git commit`
