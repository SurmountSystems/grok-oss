# Fix: unused `cli` / `full` in screen-mode relaunch hint test

**Date:** 2026-08-11
**File:** `crates/codegen/xai-grok-pager/src/app/screen_mode_relaunch.rs`

## Problem

`failed_relaunch_hint_includes_screen_mode_env` bound `cli` and `full` but never used them. Asserts hard-coded upstream binary name `grok`.

## Fix

1. **Product:** `screen_mode_relaunch_resume_hint` now uses `cli_hint_name()` instead of hard-coded `grok`, matching module docs and plain quit resume hints (`DEFAULT_CLI_HINT_NAME` / product install name).
2. **Test:** asserts use `full` and `format!(… {cli} …)`; also checks `cli == DEFAULT_CLI_HINT_NAME` and that the fullscreen hint does not contain bare ` grok `.

## Verify

| Command | Exit |
|---------|------|
| `cargo test -p xai-grok-pager --lib failed_relaunch_hint_includes_screen_mode_env -- --nocapture` | **0** (1 passed) |
| `cargo test -p xai-grok-pager --lib screen_mode_relaunch -- --nocapture` | **0** (25 passed) |
| `cargo clippy -p xai-grok-pager --lib -- -D unused-variables` | **0** (no unused-variable errors; unrelated dead_code warnings remain) |
| `cargo fmt -p xai-grok-pager` | applied |

## Intent

Sibling exit-resume tests already require `cli_hint_name()` and ban recommending upstream `grok`. The unused bindings were the unfinished half of that contract for the relaunch recovery string.
