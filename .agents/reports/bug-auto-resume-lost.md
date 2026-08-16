# Bug: last session on start was dropped

**Date:** 2026-08-13  
**Named product:** auto-open the last session when you start `grok-oss`. Not continue interrupted turn. Not `/resume`. Not cancel-resume re-fire.

Explore: [explore-last-session-on-start.md](explore-last-session-on-start.md)

## Operator-visible contract

1. Cold start of `grok-oss` with a remembered last session for this working directory opens that session. No welcome screen. No session picker.
2. First-ever use, or no last session here, stays welcome (or picker). Fine.
3. Headless `-p` still starts a fresh session unless `-c` / `--continue` or `--resume`.
4. This is not continue interrupted turn (`canceled_turn_resume.json`).

## First read vs after

**First read:** dropped. Interactive flags with no `-c`/`-r` were `NewAuto`. `materialize_startup_for_cwd` left `NewAuto` as `NewAuto`. The event loop mapped that to no `LoadSession`, so welcome. User-guide said launch always shows welcome. FORK did not pin this seam by name. Source was wrong; live binaries being old was not the only issue.

**After:** interactive `MaterializeCtx::from_pager_args` sets `open_last_session_on_start`. `NewAuto` looks up the most recent local session for cwd (`list_summaries`, same order as `--continue`). Hit → `Resume` → existing `LoadSession` path. Miss or unlistable → welcome. Headless ctx stays false.

## Red (observed before product branch)

```text
cargo test -p xai-grok-pager --lib open_last_session_on_start -- --nocapture
```

```text
thread '...materialize_new_auto_opens_last_session_when_one_exists' panicked:
expected last session to open, got NewAuto
test result: FAILED. 2 passed; 1 failed
```

Welcome-when-none and headless-stays-fresh were already green.

## Green (same filter)

```text
cargo test -p xai-grok-pager --lib open_last_session_on_start
```

```text
ok. 3 passed
```

Also green: `from_pager_args_opens_last_session_on_start`, `intent_default_is_new_auto`, `headless_materialize_ctx_stays_non_chat`.

`cargo fmt -p xai-grok-pager` and `cargo clippy -p xai-grok-pager --lib --bins -- -D warnings` exited 0.

## Files

- `crates/codegen/xai-grok-pager/src/app/session_startup.rs` (flag, lookup, NewAuto branch, tests)
- `crates/codegen/xai-grok-pager/src/headless.rs` (flag false)
- `crates/codegen/xai-grok-pager/src/headless_tests.rs`
- `crates/codegen/xai-grok-pager/src/app/session_title_resolve_tests.rs` (ctx field)
- `crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md`
- `FORK.md` (pin)

## Wrong-product half-edits (previous L2)

Uncommitted `dispatch/session/load.rs` was continue-interrupted-turn / error-idle auto-resume. It did **not** compile (`last_primary_user_turn_completed_in_replay` and `has_in_flight_mid_turn_activity` missing). Restored that file to HEAD so the crate compiles. Did not finish that product. Other dirty rebuild/`--version` files were left alone.

This L2 could not launch a workflow (host rejects workflows from a subagent). Explore report is on disk; implement stayed on this named product.

## Leftover

Live TUI stays the old binary until rebuild/install. After install, quit old TUIs and start `grok-oss` from a directory that already has a session.
