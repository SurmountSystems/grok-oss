# Process mop: Surmount user-guide restore

**Date:** 2026-08-13  
**Tree:** `/home/hunter/Projects/surmount/grok-build`  
**Primary:** `.agents/reports/bug-user-guide-surmount-pages.md`  
**Product SoT:** `crates/codegen/xai-grok-pager/docs/user-guide/`

Docs-only mop. No Rust edited. No product pages rewritten. No `25-limits` page invented.

SuperGrok is paid. Product guide says **included SuperGrok period limits**. The only "free SuperGrok" string is the prohibition on `05-configuration.md` line 3.

## Commands

```
export CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-mop-user-guide-target
export TMPDIR=/home/hunter/.cache/grok-oss-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
cargo --offline test -p xai-grok-pager --lib -- user_guide
```

Cold target needed a full compile. First two foreground runs were killed at the 300s wrapper. Incremental resume:

```
cargo --offline test -p xai-grok-pager --lib -- user_guide
```

**Exit code: 0**

```
running 3 tests
test docs::tests::user_guide_entries_are_valid ... ok
test docs::tests::user_guide_entries_have_no_duplicates ... ok
test docs::tests::default_howto_entries_includes_all_user_guide_docs ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 8851 filtered out
```

`USER_GUIDE` in `docs.rs` is 01–24 plus no extra page. Disk has those 24 numbered files plus `README.md`. Include tests match.

## Named-contract spot-check (product tree only)

All ten contracts the primary claimed finished are on the named pages.

| Contract | Where verified |
|----------|----------------|
| `/limits` and click the compact meter. Meters stay distinct. | `02-authentication.md`, `04-slash-commands.md`, `24-monitoring-usage.md` |
| Product name Grok OSS / `grok-oss` | `README.md`, `01-getting-started.md`, CLI examples |
| DOGE default. Titles on. `[ui] hide_header` is in-app only. `grok` title slot is `grok-oss`. | `05-configuration.md`, `06-theming.md` (six themes; auto dark → DOGE; `doge.tmTheme`) |
| `/screenshot`, F9, plan auto-attach | `03-keyboard-shortcuts.md`, `04-slash-commands.md`, `19-plan-mode.md` |
| Status `[pause]` / `[resume]` / `[stop]`. Soft-stop is chord-only `Ctrl+Shift+S`. | `03-keyboard-shortcuts.md`, `17-sessions.md` |
| Bare `grok-oss` opens last session for this cwd, not Welcome | `01-getting-started.md`, `17-sessions.md` |
| Continue interrupted turn (`canceled_turn_resume.json`) is a different thing | `17-sessions.md`, `03-keyboard-shortcuts.md`, `05-configuration.md` |
| Token Economy, economic mode, ASCII scrub (`scrub_ascii_punct`). `grok_oss_database_path` is toml-only. | `05-configuration.md`, `04-slash-commands.md` |
| `[subagents] allow_worktree` default false | `05-configuration.md`, `16-subagents.md` |
| Plan five CTAs. Empty Enter never approves. Present is not Approve. | `19-plan-mode.md` |

Automatic hop to the console API after included SuperGrok period limits are full is **not** documented as shipped (`02`, `04`, `24`).

`17-sessions.md` storage list includes `resources_state.json` and `canceled_turn_resume.json`. Token Economy DB is `$GROK_HOME/grok_oss.db`, not the session tree.

## Doc edits this mop

None. No missing named-contract sentence on a page the primary claimed finished.

## Leftover

`~/.grok/docs/user-guide/` is the extract target. It is **stale** until the next product launch that runs `extract_user_guide_docs`. Host has 24 files, no host `README.md`, host `06-theming.md` still says Grok Build, and a search of the host tree has **no** `/limits`. Do not treat the host tree as a second source of truth.

Unrestored xAI phrasing remains on pages this wave did not claim (`16-subagents.md` persona sections still say Grok Build; `21-terminal-support.md` and `23-dashboard.md` still lead with Grok Build). Out of mop scope.

Stop.
