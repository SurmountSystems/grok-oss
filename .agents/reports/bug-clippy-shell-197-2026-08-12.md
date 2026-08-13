# Clippy fixes: `xai-grok-shell` (`-D warnings`)

**Date:** 2026-08-12
**Package:** `xai-grok-shell`
**Scope:** surgical clippy cleanups from CI list (13 sites). No behavior intent change.

## Verify

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-shell` | 0 |
| `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` | 0 |

Forced recheck after `touch` on a touched source file also finished with exit 0 (Checking + Finished).

## Files touched

All under `crates/codegen/xai-grok-shell/`:

| File | Lint / change |
|------|----------------|
| `src/agent/roster.rs` | `sort_by` → `sort_by_key(\|b\| Reverse(...))` |
| `src/agent/update_chunk_merge.rs` | if-let-else-return → `prev?` |
| `src/auth/secret_store_progress.rs` | zero-check + `/` → `checked_div(...).unwrap_or(width)` |
| `src/extensions/billing.rs` | drop redundant `&` on `auth.key` in `format!` |
| `src/extensions/prompt_history.rs` | `sort_by` → `sort_by_key(\|a\| a.updated_at)` |
| `src/extensions/suggest/history_provider.rs` | `sort_by` → `sort_by_key(\|b\| Reverse(b.1))` |
| `src/extensions/suggest/mod.rs` | `sort_by` → `sort_by_key(\|b\| Reverse(b.priority))` (stable sort kept) |
| `src/remote/agent.rs` | drop redundant `&` on `auth.key` |
| `src/remote/client.rs` | drop redundant `&` on `auth.key` (two sites) |
| `src/session/acp_session_impl/tool_calls.rs` | collapsible match: fold `if final_result.is_none()` into match arm guard |
| `src/session/storage/jsonl/mod.rs` | `sort_unstable_by` → `sort_unstable_by_key(\|b\| Reverse(b.1))` |
| `src/session/workflow/manager.rs` | manual Option zip → `session_dir.as_ref().zip(journal_path.as_ref())` |
| `src/util/subprocess.rs` | `loop` + `let Some ... else break` → `while let Some(remaining) = ...` |

## Notes

- Suggest merge path still uses **stable** `sort_by_key` (not unstable); only the comparator form changed.
- No git commit or stage.
- No extra clippy failures surfaced beyond the listed set.
