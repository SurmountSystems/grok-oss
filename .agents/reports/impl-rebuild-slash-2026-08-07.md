# Implement report: `/rebuild` + `grok-oss rebuild` (2026-08-07)

ASCII only. Complete vertical from approved plan.

## What shipped

### 1. SHA-aware local-rebuild identity

- `parse_binary_identity` + updated `leader_is_older_than` in
  `crates/codegen/xai-grok-shell/src/leader/mod.rs`.
- Same package semver + different git SHA → accept relaunch.
- Equal SHA / pure equal package → decline.
- Bare leader + SHA baseline → accept (upgrade path).
- SHA leader + bare baseline → decline (no thrash to less specific).
- Wired through existing `decide_relaunch_for_update`.

### 2. Shared rebuild core + CLI

- `xai_grok_update::rebuild` (`crates/codegen/xai-grok-update/src/rebuild.rs`):
  resolve source root, `just install` or fixed cargo argv, verify
  `--version`, `signal_leaders_to_relaunch`, live `active_sessions` inventory,
  summary lines.
- `xai_grok_shell::leader::signal_leaders_to_relaunch` (public, structured
  outcomes; used by CLI update path and rebuild).
- `active_sessions::list()` + public `is_pid_alive`.
- CLI: `grok-oss rebuild [--source DIR]` in pager-bin.

### 3. Builtin slash `/rebuild`

- `slash/commands/rebuild.rs` registered in `builtin_commands()`.
- `Action::RebuildAndRelaunch` → `Effect::RunRebuild` →
  `TaskResult::RebuildDone`.
- Progress toast + scrollback; on success: cancel mid-turn with
  `canceled_turn_resume` when a turn is running, arm `RebuildRelaunch`,
  quit, re-exec installed binary with `--resume` (same pattern as screen-mode
  relaunch).

### 4. Docs

- User-guide `04-slash-commands.md` (`/rebuild`).
- User-guide `01-getting-started.md` (OSS rebuild path; not SpaceXAI).
- `oss_update::how_to_update_message` mentions `/rebuild` and `grok-oss rebuild`.
- `FORK.md` short bullet + update section pointer.

## Red → green evidence

| Contract | Command | Result |
|----------|---------|--------|
| Same semver + different SHA accepts | `cargo test -p xai-grok-shell --lib leader_is_older_than` | ok (directional + SHA identity) |
| Parse identity | `cargo test -p xai-grok-shell --lib parse_binary` | ok |
| decide_relaunch SHA | `cargo test -p xai-grok-shell --lib decide_relaunch` | ok (accept + equal decline + existing directional) |
| Resolve root / parse version / build-fail no signal | `cargo test -p xai-grok-update --lib rebuild::` | ok (4 tests) |
| Slash registered + Action | `cargo test -p xai-grok-pager --lib slash::commands::rebuild` | ok |
| Rebuild relaunch struct | `cargo test -p xai-grok-pager --lib dispatch::rebuild` | ok |

### Verify

- `cargo fmt -p xai-grok-shell -p xai-grok-update -p xai-grok-pager -p xai-grok-pager-bin`
- `cargo clippy -p xai-grok-shell --lib -- -D warnings` → clean
- `cargo clippy -p xai-grok-update --all-targets -- -D warnings` → clean
- `cargo clippy -p xai-grok-pager --lib -- -D warnings` → clean
- `cargo clippy -p xai-grok-pager-bin --bin grok-oss -- -D warnings` → clean
- Note: `xai-grok-shell --all-targets` still has pre-existing test-only clippy
  failures unrelated to this work (`await_holding_lock` in other modules).

## Limits check (operator dogfood note)

Ran `grok-oss limits --json` after implementation work:

| Field | Value |
|-------|--------|
| `activeDriver` | `supergrok_free_period` ("Active: free SuperGrok period") |
| free SuperGrok period included used % | **6.0%** (both business and personal principals; shared pool) |
| included remaining % | 94 |
| live sampling | SuperGrok session (business) |

Compared to prior ~6%: **still ~6%**, not moving past 6% during this implement
pass. No SuperGrok free-period debit invented; observation only.

## Remaining soft items (real, not invent-park)

- **Other standalone TUIs** (not leaders, not the invoking process): v1 reports
  live PIDs + reattach hints only. No cooperative quit marker / SIGUSR in this
  cut.
- **Nix / AUR install backends** not first-class flags (still `just install` /
  cargo fixed argv).
- Full live dogfood of multi-session `/rebuild` (two TUIs + mid-turn resume
  toast) is operator manual; unit tests cover identity + registration + pure
  orchestration order without a full release build in CI.

## Key paths

- `crates/codegen/xai-grok-shell/src/leader/mod.rs` (identity + signal)
- `crates/codegen/xai-grok-shell/src/leader/server.rs` (decide tests)
- `crates/codegen/xai-grok-update/src/rebuild.rs`
- `crates/codegen/xai-grok-pager/src/slash/commands/rebuild.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/rebuild.rs`
- `crates/codegen/xai-grok-pager-bin/src/main.rs` (`Command::Rebuild`)
