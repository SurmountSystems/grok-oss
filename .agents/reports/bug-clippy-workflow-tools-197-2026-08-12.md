# Clippy fix: xai-workflow + xai-grok-tools (-D warnings)

Date: 2026-08-12
Scope: surgical fixes only for listed clippy items. No git add/commit.

## Files touched

### xai-workflow
- `crates/codegen/xai-workflow/src/engine.rs`
  Replaced Ok/Err match on `value_to_dynamic` with `results.push(value_to_dynamic(&value)?);`

### xai-grok-tools
- `crates/codegen/xai-grok-tools/src/implementations/web_search/client.rs`
  Float literals: `0.1` / `0.95` → `0.1_f32` / `0.95_f32` (both request builders)
- `crates/codegen/xai-grok-tools/src/computer/local/terminal.rs`
  `for (_, process) in self.processes.iter_mut()` → `for process in self.processes.values_mut()`
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/search_replace/mod.rs`
  Removed redundant `&` on `input.file_path` in format! strings
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/todo/mod.rs`
  `pct()` uses `checked_div(...).unwrap_or(0)` instead of manual zero-check + `/`
- `crates/codegen/xai-grok-tools/src/implementations/opencode/edit/mod.rs`
  Removed redundant `&` on `input.file_path` in format! strings
- `crates/codegen/xai-grok-tools/src/types/output.rs`
  Removed redundant `&` on `mcp_output.tool_name` in format!
- `crates/codegen/xai-grok-tools/src/util/command_display.rs`
  Last strip_prefix chain rewritten with `?` via `or_else`

## Verify

```
cargo clippy -p xai-workflow --all-targets -- -D warnings
```
Exit code: **0**

```
cargo clippy -p xai-grok-tools --all-targets -- -D warnings
```
Exit code: **0**

Note: xai-grok-tools run printed a build-script / workspace `clippy.toml` advisory about `tokio::process::Command::spawn` disallowed path not being reachable; it did not fail under `-D warnings`.

## Status

Done. Both packages clean under `-D warnings`.
