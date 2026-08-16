# Process mop: user-guide Clear finished `[−]`

**Date:** 2026-08-13
**Tree:** `/home/hunter/Projects/surmount/grok-build`
**Primary:** `.agents/reports/bug-user-guide-clear-finished.md`
**Product SoT:** `crates/codegen/xai-grok-pager/docs/user-guide/`

Docs mop only. No Rust. No new guide file. Did not edit `~/.grok/docs/user-guide/`.

SuperGrok is paid. This pass did not invent hop-after-period-full and did not call SuperGrok free.

## Tests

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-ug-clear-finished-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
cargo --offline test -p xai-grok-pager --lib -- user_guide
```

Cold mop target needed a full compile (two killed resumes, then finished). Final run:

**Exit code: 0**

```
running 3 tests
test docs::tests::user_guide_entries_are_valid ... ok
test docs::tests::user_guide_entries_have_no_duplicates ... ok
test docs::tests::default_howto_entries_includes_all_user_guide_docs ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 8857 filtered out; finished in 0.00s
```

No new `USER_GUIDE` file. Disk still has 01–24 plus `README.md` (25 files).

## Spot-check (pages 03, 04, 17)

All three product pages still name Clear finished **`[−]`** (U+2212 minus, UTF-8 `e2 88 92`) and `/clear-completed-todos`. No ASCII `[-]` false-friends.

| Page | Present |
|------|---------|
| `03-keyboard-shortcuts.md` | Agent-level `X` (line 218). Clear finished paragraph after Agent-Level notes (line 235). Mouse bullet for click `[−]` (line 398). |
| `04-slash-commands.md` | `/clear-completed-todos` under Session Management (lines 127–133). |
| `17-sessions.md` | Section **The session todo board** (`resources_state.json` live snapshot, `plan.json` fallback, lines 104–110). Tips line (line 385). |

Primary-claimed sentences are on disk. No restore.

Host overlay copies at `~/.grok/docs/user-guide/{03,04,17}*` have no Clear finished / `/clear-completed-todos` text. Untouched.

## Did not

- Touch any `.rs`
- Edit `~/.grok/docs/user-guide/`
- Invent pages
- Restore missing copy (none missing)
- Call SuperGrok free
- `git add` / `git commit`
