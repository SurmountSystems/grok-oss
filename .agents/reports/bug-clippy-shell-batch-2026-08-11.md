# Report: `xai-grok-shell` clippy-clean (`-D warnings`)

**Date:** 2026-08-11
**Package:** `xai-grok-shell`
**Verify:**

```bash
cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --all-targets -- -D warnings
```

**Exit codes:** fmt 0, clippy 0.

## Operator lib errors (49) fixed

### A. private-interfaces

- `ModelByok` was `pub(crate)` while `ModelAuthFacts::byok` is a `pub` field on a `pub` struct under `pub mod agent::config`.
- **Fix:** promote `ModelByok` to `pub` in `agent/auth_method.rs` (matches public API exposure of `ModelAuthFacts`).

### B. disallowed_methods (`tokio::process::Command::spawn`)

- `auth/auth_provider.rs` `run_capped`: site-local `#[allow(clippy::disallowed_methods)]` at enroll boundary (already had `ProcessGroup::attach`).
- `auth/flow.rs` external provider path: allow + best-effort `ProcessGroup` enroll; on timeout kill the group so grandchildren do not outlive the reported failure.

### C. needless_borrow

- `terminal/streaming_local_terminal.rs`: `spawn_with_argv(program, …)` (`default_shell_path()` already returns `&'static str`).

### D. unreachable_pub

Demoted crate-private symbols to `pub(crate)` (no file-wide allow):

| Area | Items |
|------|--------|
| `agent/handlers/session.rs` | `handle` |
| `agent/mvp_agent/session_lifecycle.rs` | `RegistrySnapshot` |
| `agent/subagent/mod.rs` | `capture`, resolve_* helpers, `SubagentSessionMetadata` + `SCHEMA_VERSION` + `from_meta` |
| `auth/flow.rs` | `StderrCallback`, `run_auth_flow*`, `run_auth_flow_interactive` |
| `auth/model.rs` | `is_supergrok_session_mode`, `SUPERGROK_PERSONAL_MULTI_SLOT`, `lookup_supergrok_session_for_base` |
| `auth/oidc/protocol.rs` | `is_configured` |
| `extensions/chat_conversation_history.rs` | `handle` |
| `extensions/session_admin.rs` | `handle` |
| `session/slash_commands.rs` | `allows`, `all_enabled` (test helper) |
| `session/usage_log.rs` | constants, types, constructors, record helpers |

Still re-exported as public crate API (kept `pub` on definition where applicable): e.g. flow items under `pub use flow::{ AuthUrlInfo, … ensure_authenticated*, run_cli_login, … }`, model items under `pub use model::{ AuthMode, GrokAuth, upsert_supergrok_session, multi_slot_scope_for_auth, … }`.

### Intentional public-API exceptions kept as `pub`

- **`agent::auth_method::ModelByok`** — public because it is the type of `pub` field `ModelAuthFacts::byok` on the public `agent::config::ModelAuthFacts` type.
- Existing public re-exports from `auth/mod.rs` and other public modules were left alone (not in the clippy list).

## Extra work from `--all-targets` (tests)

After lib was green, tests added more lints; fixed until package green:

- `field_reassign_with_default` → struct update syntax (`GrokComConfig` / `Config` fixtures).
- `len_zero` → `!api.is_empty()`.
- `single_match` → `if let Some(...)`.
- `if_same_then_else` → collapsed identical branches in unified list test.
- `items_after_test_module` → moved `is_reserved_slash_name` and `shell_c` above `mod tests`.
- `await_holding_lock` → site `#[allow]` on tests that intentionally hold env locks across await.
- `unused_must_use` → `let _ =` for `handle_interject_queued_prompt`, `cancel_running_task`, oneshot flush in tests.

### Optional clippy.toml

- Added `allow-invalid = true` on the `tokio::process::Command::spawn` disallowed-methods entry (clippy warned the path was not reachable for validation).

## Scope

Surgical edits under `crates/codegen/xai-grok-shell` + one-line `clippy.toml` tweak. No git commit/add/push. No bulk sed across unrelated crates.
