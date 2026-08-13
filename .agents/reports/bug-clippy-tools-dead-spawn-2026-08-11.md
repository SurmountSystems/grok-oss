# Clippy `-D warnings` in `xai-grok-tools` (2026-08-11)

## Cause

Onto mop left two operator-pasted failures plus several more that showed up under `--all-targets` on a clean host clippy run:

1. **`dead_code`:** `LocalTerminalActor::ensure_persistent_shell_initialized` was unused. Persistent shell init is already inlined at the start of `spawn_persistent_command` (the only live path). The helper was leftover after product changes, not a missing wire-up.
2. **`clippy::disallowed_methods` (`std::process::Command::spawn`):** implement-memory `workspace::git` spawned a short-lived git probe without process-group enrollment. Clippy policy requires enrolled children so session/process exit can reap them.
3. **Fallout under the same verify command:** lifecycle test used bare `tokio::process::Command::spawn`; several async tests held process-env `MutexGuard`s across `.await` (`await_holding_lock`); two `match`/`len` style nits (`single_match`, `len_zero`).

## Fix

| Area | Change |
|------|--------|
| `computer/local/terminal.rs` | Removed unused `ensure_persistent_shell_initialized` (logic remains in `spawn_persistent_command`). |
| `util/implement_memory/workspace.rs` | Detach std command; spawn with allow only as the enroll site; `ProcessGroup::attach_std` + `global_process_scope().register`; kill via group on timeout. (`ProcessScope::enroll` is tokio-only; this is the established std enroll path used by envrc / restore_fetch / LSP.) |
| `computer/local/lifecycle.rs` | Test spawn via `ProcessScope::spawn` (enroll primitive). |
| `shared_http_rate_limit.rs`, `image_gen/mod.rs` | `single_match` → `if let`; deliberate `await_holding_lock` allow on env-serializing async test. |
| `opencode/edit/mod.rs`, `use_tool/mod.rs` | Same env-lock allow on async bulk/toon tests. |
| `util/session_reader/mod.rs` | `turns.len() >= 1` → `!turns.is_empty()`. |

No `#[allow(dead_code)]`. Disallowed-spawn allow only at the std enroll site with a clear comment.

## Files changed

- `crates/codegen/xai-grok-tools/src/computer/local/terminal.rs`
- `crates/codegen/xai-grok-tools/src/util/implement_memory/workspace.rs`
- `crates/codegen/xai-grok-tools/src/computer/local/lifecycle.rs`
- `crates/codegen/xai-grok-tools/src/shared_http_rate_limit.rs`
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/image_gen/mod.rs`
- `crates/codegen/xai-grok-tools/src/implementations/opencode/edit/mod.rs`
- `crates/codegen/xai-grok-tools/src/implementations/use_tool/mod.rs`
- `crates/codegen/xai-grok-tools/src/util/session_reader/mod.rs`

## Verify

```bash
cargo fmt -p xai-grok-tools
cargo clippy -p xai-grok-tools --all-targets -- -D warnings
```

**Exit code: 0**

(Host still prints a build-script warning that `tokio::process::Command::spawn` is not a reachable path in `clippy.toml`; that is pre-existing and does not fail `-D warnings`.)
