# Restore Surmount user-guide pages

**Date:** 2026-08-13  
**Tree:** `/home/hunter/Projects/surmount/grok-build`  
**Scope:** product guide only, `crates/codegen/xai-grok-pager/docs/user-guide/`

Surgical restore after the 1.0.3 restack left xAI copy. Product tree is the source of truth. No second home was invented.

SuperGrok is paid. The shipped guide says **included SuperGrok period limits**. It does not say "free SuperGrok."

## What was already in place (this restore wave)

`README.md`, `01`, `02`, `03`, `04`, and `05` already named Grok OSS / `grok-oss`, `/limits` plus click the compact meter, Token Economy, the 16 leftover Settings rows, ASCII scrub, economic mode, `/screenshot` / F9 / plan auto-attach, `[pause]` / `[resume]` / `[stop]`, soft-stop as chord-only, last-session-on-start, `canceled_turn_resume.json` as a different thing, and `[subagents] allow_worktree` default false. `grok_oss_database_path` is documented as **toml-only** (no Settings row). Automatic hop to the console API after included SuperGrok period limits are full is **not** documented as shipped.

## What this pass finished

| Page | Restore |
|------|---------|
| `06-theming.md` | Six themes. **DOGE** is the default, not GrokNight. Auto dark maps to DOGE. `doge.tmTheme`. **Hide header** vs **window titles** on by default. The `grok` title slot renders as `grok-oss`. |
| `16-subagents.md` | Worktree isolation off by default (`[subagents] allow_worktree = false`). Opt-in toml + Settings row. Token Economy / `/limits` pointer. |
| `17-sessions.md` | Last-session-on-start for bare `grok-oss`. Distinct **Continue interrupted turn** (`canceled_turn_resume.json`). Storage list includes that file and `resources_state.json`. Token Economy DB is not in the session tree. |
| `19-plan-mode.md` | Present is not Approve. Five CTAs. Empty Enter never approves. `/screenshot` / F9 auto-attach. Freeform questions, not the questionnaire modal as the operator path. |
| `24-monitoring-usage.md` | This page is org OpenTelemetry only. Spend meters live on `/limits` and click the compact meter. |

No new `25-limits` page. FORK expected `/limits` on authentication and slash commands. Adding a file would require a `docs.rs` `USER_GUIDE` entry (Rust). That was out of scope.

## Named contracts

1. `/limits` and click the compact meter. Included SuperGrok period limits stay distinct from SuperGrok dollar credits and console team prepaid. (`02`, `04`, `24`)
2. Product name Grok OSS / `grok-oss`. (`README`, `01`, `02`, install, CLI examples)
3. DOGE default. Titles on by default. `[ui] hide_header` is in-app headers only. (`05`, `06`)
4. `/screenshot`, F9, plan approval auto-attach. (`03`, `04`, `19`)
5. Status `[pause]` / `[resume]` / `[stop]`. Soft-stop is **chord-only** (`Ctrl+Shift+S`), even if status-row paint is still landing. (`03`, `17`)
6. Bare `grok-oss` opens last session for this cwd. Not the Welcome picker. (`01`, `17`)
7. Continue interrupted turn (`canceled_turn_resume.json`) is a different thing. (`17`, `03`, `05`)
8. Token Economy, economic mode, ASCII scrub, and the 16 leftover Settings rows. No Settings row for `[token_economy] grok_oss_database_path`. (`05`, `04`)
9. `[subagents] allow_worktree` default false. (`05`, `16`)
10. Plan five CTAs. Empty Enter never approves. Present is not Approve. (`19`)

Did not invent hop-after-period-full as shipped.

Did not dump residual queue codes or implement-run hex into the guide.

Did not claim dropped plan chrome ("Revising plan...", sticky already-decided re-arm) as shipped.

## Tests

Docs-only: TDD exception for prose. Existing include tests:

```
cargo test -p xai-grok-pager --lib -- user_guide
```

3 passed (`user_guide_entries_are_valid`, `user_guide_entries_have_no_duplicates`, `default_howto_entries_includes_all_user_guide_docs`). No test asserted an xAI-only GrokNight default. No Rust was edited.

## Host copy

`~/.grok/docs/user-guide/` is an extract target (`docs.rs` `extract_user_guide_docs`). It is **stale**: 06 still says Grok Build, there is no host README, and a search of the host copy has no `/limits`. The next product launch that extracts the guide will refresh it. Do not treat the host tree as a second source of truth.

## Did not

- Edit `render.rs` or `settings/defs.rs`
- `git add` / `git commit`
- Bulk find-and-replace
- Touch Rust
