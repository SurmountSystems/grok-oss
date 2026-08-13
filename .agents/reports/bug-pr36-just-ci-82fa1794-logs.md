# PR #36 first product fail, run 31668960010

Read-only log pull. No product edits. No GitHub write. `gh run view` hung (CLI token lives in the login keyring). Logs came from authenticated GitHub MCP `get_job_logs` plus the signed Actions logs zip.

## Run

| Item | Value |
|------|--------|
| Run | [31668960010](https://github.com/SurmountSystems/grok-oss/actions/runs/31668960010) (CI #53, `pull_request`) |
| Job | [94349508279](https://github.com/SurmountSystems/grok-oss/actions/runs/31668960010/job/94349508279) `just ci` |
| SHA | `82fa1794a8f1751045da6eb85b3e43d902972a69` |
| Branch | `onto-xai/b13fa526f511` |
| Conclusion | **failure** |
| Failed step | `just ci-prep && just test` (step 6) |
| Recipe that died | `test-unit` → `just cargo-ci cargo nextest run --workspace --locked` |
| Exit | **101** (`cargo test --no-run --workspace --jobs 2 --locked` while nextest was compiling) |

Ignored noise: Nix cache 400, Node 20 deprecation.

`cargo fmt --all -- --check` passed. `cargo clippy --workspace --lib --bins --locked -- -D warnings` passed (`Finished dev` at 05:11:50Z). Nextest never executed a product test.

## Classification

**test-compile**

Not fmt. Not clippy. Not test-runtime. Not infra.

## First real fail

- Crate: `xai-grok-shell` (`lib test`)
- rustc: `error[E0433]: cannot find module or crate ctor in this scope`
- File: `crates/codegen/xai-grok-shell/src/test_support/mod.rs:23`
- End: `could not compile xai-grok-shell (lib test) due to 117 previous errors`

```
error[E0433]: cannot find module or crate `ctor` in this scope
  --> crates/codegen/xai-grok-shell/src/test_support/mod.rs:23:3
   |
23 | #[ctor::ctor]
   |   ^^^^ use of unresolved module or unlinked crate `ctor`
```

At this SHA, `xai-grok-shell/Cargo.toml` has no `ctor` entry. Workspace already pins `ctor = "0.4"` (`xai-grok-telemetry` uses it). `lib.rs` loads `test_support` only under `#[cfg(test)]`. Nextest downloaded `ctor v0.4.3` for other crates; this crate still did not link it.

## Named contract (do not weaken)

Unit tests in `xai-grok-shell` must compile. The pre-main `#[ctor::ctor]` hook in `test_support` must keep redirecting the unified log to a temp file so the unit-test binary does not write synthetic events into the operator's real unified log. Keep the hook. Do not delete tests to make rustc quiet.

## Smallest product fix (first hole only)

Add under `[dev-dependencies]` in `crates/codegen/xai-grok-shell/Cargo.toml`:

```toml
# Pre-main unified-log redirect in `src/test_support/mod.rs` (`#[ctor::ctor]`).
ctor = { workspace = true }
```

That unblocks E0433 only. The same compile then still has **116** more rustc errors in the same crate (onto restack: missing Surmount fields, renamed helpers, stale test fixtures). Walk those next. Do not rewrite test expectations to match the broken fixtures.

## Nextest runtime tests

None. No `FAIL` lines. No test names to list.

## Other rustc sites in this same compile (not nextest names)

18 files, 117 errors. Largest piles first:

1. `session/acp_session_tests/cancel_running_task_tests.rs` (53)
2. `agent/config_tests.rs` (14)
3. `session/acp_session_impl/tasks_cancel.rs` (9)
4. `agent/subagent/tests/mod.rs` (9)
5. `session/persistence_tests.rs` (6)
6. `session/acp_session_tests/prompt_mode_transition_tests.rs` (6)
7. `session/acp_conversion.rs` (6)
8. `sampling/error.rs` (6)
9. `auth/manager_tests.rs` (6)
10. `session/acp_session_tests/auth_error_no_retry_tests.rs` (3)
11. `util/config/persist_tests.rs` (2)
12. `test_support/mod.rs` (1) **first**
13. `test_support/lsp_runtime.rs` (1)
14. `session/usage_log.rs` (1)
15. `session/helpers/session_compact_reasoning_compaction_regression_tests.rs` (1)
16. `session/compaction_inline_auto_compact_flow_tests.rs` (1)
17. `session/acp_session_tests/tool_layer_images_bridge_tests.rs` (1)
18. `session/acp_session_impl/model_switch.rs` (1)

Second error (same compile, after ctor):

```
error[E0425]: cannot find value `WORK_ULID_SESSION_FILE` in module `super`
  --> crates/codegen/xai-grok-shell/src/agent/subagent/tests/mod.rs:32:45
```
