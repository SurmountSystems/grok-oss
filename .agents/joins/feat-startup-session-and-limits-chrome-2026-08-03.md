# Join: startup conversation defaults + always-relevant limits chrome

**Date:** 2026-08-03
**Scope:** `xai-grok-pager` (session_startup, credit_bar, billing fetch mapping, user-guide)

## Done

### 1. Default open folder → last conversation

- Interactive TUI `MaterializeCtx::from_pager_args` sets `auto_resume_last_for_cwd` when not worktree and not `--chat`.
- Bare `NewAuto` materialization lists cwd sessions (newest first) and:
  - **Empty:** `NewAuto { new_folder_notice: true }` → soft yellow welcome banner ("This is a new folder with no prior conversations yet.").
  - **One:** resume latest, no sibling toast.
  - **Two+:** resume latest; toast with plain relative age of next-oldest ("Other conversations exist… Next most recent was 3 hours ago.").
- Headless / worktree keep fresh sessions (`auto_resume_last_for_cwd: false`).
- Pure helpers + tests: `pick_default_startup_session`, `format_plain_relative_ago`, toast/new-folder copy.

### 2. Limits tracker always relevant + no silent 0%

- `CreditBalance.included_usage_known` aligned with shell `included_usage_and_period_end` (honest absence).
- Mapping in `credit_balance_from_config` / `credit_balance_from_billing_config` sets the flag.
- Status chrome: unknown → `...%` (same as cold), never a silent `0%`. True wire `0%` still paints `0%`.
- `/usage` summary: "not yet available" when unknown.
- Console live unchanged: prepaid $ or honest gap (not SuperGrok %).
- `billing_poll_wanted` stays true when included is unknown (or no config yet), so chrome keeps polling until warm.

### 3. Docs

- `docs/user-guide/17-sessions.md` — opening a folder defaults.
- `docs/user-guide/04-slash-commands.md` — `/limits` status meter honesty (live principal, `...%` vs true `0%`).

## Verify (package-scoped)

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell
cargo test -p xai-grok-pager --lib -- pick_default_startup other_conversations new_folder plain_relative auto_resume_last unknown_included true_zero_included usage_warning_console_live usage_summary_unknown credit_balance_empty_config credit_balance_explicit_zero credit_balance_prefers_credit_usage intent_default
# 16 passed
cargo test -p xai-grok-pager --lib -- session_startup:: credit_bar:: credit_balance_
# 124 passed
```

## Not touched

- `token_economy/implement_effort.rs` / lock/min config (other agent).
- No git add/commit.

## Key paths

- `crates/codegen/xai-grok-pager/src/app/session_startup.rs`
- `crates/codegen/xai-grok-pager/src/app/event_loop.rs` (toast + new-folder banner)
- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/billing.rs`
- `crates/codegen/xai-grok-pager/src/limits_cmd.rs`
