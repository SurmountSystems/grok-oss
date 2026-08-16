# Plan map: agent file writes today (ACP / product tools)

Read-only inventory for automatic fmt + clippy + tests on files just written, at the tool layer. No leftover product implementation of `feat:agentic-fmt-clippy-acp`. Residual only.

## 1. Which tools write files

Default Grok toolset (`default_grok_build_toolset` in `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-agent/src/config.rs`) has **no `write` tool**. Create/overwrite is `search_replace` with empty `old_string`.

| Surface | Crate path | Entry | Disk write | `FileWritten` |
|---|---|---|---|---|
| **Grok edit (primary)** | `crates/codegen/xai-grok-tools/src/implementations/grok_build/search_replace/mod.rs` | `SearchReplaceTool::run` → `run_search_replace` | `AsyncFileSystem::write_file` | yes (create ~364, replace ~717) |
| **Grok concise edit** | `.../grok_build_concise/search_replace.rs` | wraps same tool | same | same |
| **Codex patch** | `.../codex/apply_patch/tool.rs` | `ApplyPatchTool::run` | add/update/move via `fs.write_file`; delete via `delete_file` | yes (per file, ~355–460) |
| **Hashline edit** | `.../grok_build_hashline/edit/mod.rs` | `HashlineEditTool` ops include `"write"` | `fs.write_file` after `apply::apply_edits` | via `to_search_replace` output; not a separate `write` tool in default set |
| **OpenCode write** | `.../opencode/write/mod.rs` | `WriteTool::run` (`id` `"write"`) | `fs.write_file` | yes (~145) |
| **OpenCode edit** | `.../opencode/edit/mod.rs` | `OpenCodeEditTool` | `fs.write_file` | yes (~310, ~477) |
| **Bash / shell** | `.../grok_build/bash/mod.rs` | `BashTool::run` | any shell redirect; **not** a structured file tool | no |
| **Workspace RPC** | `crates/codegen/xai-grok-workspace/src/file_system/ext_fs.rs` | `FsWriteFileReq::execute` (`workspace.fs_write_file`) | `std::fs::write` | no |
| **ACP transport** | not a model tool | `write_text_file` | client disk | n/a |

Registry registration: `ToolRegistryBuilder::new` in `crates/codegen/xai-grok-tools/src/registry/types.rs` (~681–761).

Toolsets:

- `grok-build` / `grok-build-plan`: bash + `search_replace` (no apply_patch, no write).
- `codex`: `ApplyPatchTool` instead of `search_replace`.
- `explore` / `plan` presets: no edit, no shell.
- `hashline`: `hashline_edit` instead of `search_replace`.
- OpenCode write/edit: registered, not in default grok-build preset.

FS backends (`spawn.rs` ~635–643): if ACP client advertises fs write and a gateway exists, `AcpFsAdapter::write_file` (`crates/codegen/xai-grok-workspace/src/file_system/adapter.rs`) sends `acp::WriteTextFileRequest`. Else `LocalFs::write_file` (`crates/codegen/xai-grok-tools/src/computer/local/file_system.rs`). Overlay path: `AcpSessionFs` in `.../file_system/acp_fs.rs`. Gateway forward: `xai-acp-lib/src/gateway.rs` `write_text_file`.

Tests: `SearchReplaceTool` `basic_replacement`, `new_file_creation`, `empty_old_string_*` in `search_replace/mod.rs`. ApplyPatch `add_file_creates_with_correct_content`, `update_file_modifies_content`, `multiple_files_in_one_patch` in `apply_patch/tool.rs`. `file_written_round_trip_includes_previous_content` in `xai-tool-runtime/tests/notification_serde.rs`. `fs_injection_regression_tests.rs` (must use injected FS, not raw disk).

## 2. After a successful write: hooks / notifications

**In-product, already on the write path:**

- `FileWritten` (`xai-grok-tools/src/notification/types.rs`, send via `ToolNotificationHandle::send_file_written`). Shell `notification_bridge.rs` ~352 forwards to hunk tracker + `file_state_tracker`. Best attach point for “this path just landed on disk.”
- Cross-cutting **reminders** after every tool (`ToolRegistryBuilder::register_reminder`): `LspDiagnosticsReminder` (SearchReplace only; `notify_file_changed` + drain), `SkillDiscoveryReminder` (paths from SearchReplace / every apply_patch file), `TaskCompletionReminder`. `LspDiagnosticsReminder` does **not** cover apply_patch.
- Shell dispatch: `execute_tool_calls` → `prepare_tool_call` → `dispatch_tool` → success → `PostToolUse` (`tool_calls.rs` ~774–798). Failure → `PostToolUseFailure`. **Per tool call, not per file, not batched.**
- `PreToolUse` runs in `prepare_tool_call` before execute (~1083).

**Host hooks (`~/.grok/hooks`, project `.grok/hooks`, config.toml):** `xai-grok-hooks`. `afterFileEdit` aliases to `PostToolUse` (`event.rs` ~112–120). Gate is **Observe** (cannot block). Matcher can filter by `tool_name`. Docs already sell “run `cargo fmt` after edits” (`xai-grok-pager/docs/user-guide/10-hooks.md`). That is **operator-configured scripts**, fail-open, not product fmt.

**Turn end:** `Stop` hook can block the turn. No product post-turn fmt.

**No in-product wrapper** today that runs rustfmt/clippy/tests after write.

## 3. Existing auto-format / clippy / cargo-check after edit?

**No.** Residual only:

- `RESIDUAL.md` Open: “Agent process: cargo fmt / post-impl verify via ACP”. Board `feat:agentic-fmt-clippy-acp`. Deferred until after dogfood. Explicitly not a chat-scold.
- `FORK.md` still-open: “Agentic fmt/clippy ACP” points at residual.
- Host / project `AGENTS.md` 3a/3b: process mop (`cargo fmt -p`, clippy `--all-targets`, targeted tests). That is **agent process**, not a tool.

Closest product transforms (not rustfmt):

- `util/trailing_ws.rs` `prepare_for_write` (default ON, `GROK_STRIP_TRAILING_WHITESPACE`). Used by **OpenCode edit** and **hashline edit** only. **Grok `search_replace` and `apply_patch` do not strip.**
- `LspDiagnosticsReminder`: rust-analyzer diagnostics after SearchReplace, not format-on-save. LSP module has no `textDocument/formatting`.
- `bulk_edit_policy.rs`: **pre**-edit storm deny for `search_replace`, not format.

## 4. Bash intercept of named cargo today

`BashTool::run` (`bash/mod.rs` ~2015–2035) intercepts **only** three allowlisted Python skill CLIs, in-process:

- `util/implement_memory::{try_parse_memory_intercept, execute_intercept}` (`memory.py`)
- `util/plan_validate::{try_parse_plan_validate_intercept, execute_intercept}` (`validate-plan.py`)
- `util/session_reader::{try_parse_session_reader_intercept, execute_intercept}`

Tests: `implement_memory_snapshot_intercept_does_not_spawn_shell`, `plan_validate_intercept_does_not_spawn_shell`, `session_reader_list_intercept_does_not_spawn_shell`.

**There is no cargo / rustfmt / clippy intercept.** The model must type `cargo fmt` for that to run. A write tool **could** spawn scoped cargo the same way (in-process or `Command` after `write_file`) without the model choosing the command. No `package_from_path` / crate-for-file helper exists. Tight scope would be `rustfmt`/`cargo fmt -- <paths>` on the written files, then clippy/test **`-p` that crate** (still a crate compile, not a 2M-line workspace `--all`).

## 5. Dedup / debounce if 8 files in one turn

Existing batch patterns (none are a fmt flush):

- **Same-turn parallel tools:** `execute_tool_calls` prepares all, then `dispatch_futures` in parallel (`tool_calls.rs` ~528–587). Per-path mutex via `lock_path_for_args` so two edits of the same file serialize. Tests: `lock_path_for_args_*` in `parallel_dispatch_tests.rs`.
- **`apply_patch`:** many files, one tool call, one `PostToolUse`, many `FileWritten`.
- **`split_exit_plan_tail`:** `exit_plan_mode` runs after the rest of the batch so `plan.md` writes land first. Test: `same_batch_plan_write_before_exit_plan_mode_returns_new_body`.
- **`FileOperationLockManager`:** `editor_infra/file_operation_lock.rs` (OpenCode-style exclusive write lock).
- **fsnotify debounce:** file watcher only, not tool writes.
- **`bulk_edit_policy`:** 5 paths / 120s storm window for identical replacements.

No turn-end “flush edited paths and format once.” Eight `search_replace` calls = eight `FileWritten` + eight `PostToolUse`. A product mop should **collect paths for the turn** (or debounce on `FileWritten`) rather than spawn cargo per call.

## 6. Plan-mode write exception

Session plan file is the **only** allowed edit while plan mode is Active, in every permission mode (including always-approve).

- Tracker: `crates/codegen/xai-grok-shell/src/session/plan_mode.rs`. `is_plan_file_write` (exact path). `should_auto_approve_edit`. Rejection template `plan_mode_edit_rejected_template`.
- Gate: `plan_mode_edit_gate` in `tool_calls.rs` (~237). Runs in `prepare_tool_call` **before** permission / PreToolUse (~1055). `AccessKind::Edit` must be the plan file. `apply_patch` uses placeholder `"apply_patch"` so it is **always rejected** in plan mode.
- Comment above the gate mentions a markdown carve-out for compat Write/StrReplace. **Implementation + tests do not allow arbitrary `.md`.** `write("/tmp/README.md")` is rejected.

Tests:

- `tool_calls.rs` `plan_mode_edit_gate_tests`: `grok_edits_outside_plan_file_rejected`, `plan_file_edit_allowed`, `apply_patch_rejected_in_plan_mode`.
- `acp_session_tests/plan_mode_edit_gate_tests.rs`: `plan_mode_rejects_grok_edit_outside_plan_file_despite_allow_all_permissions`, `plan_mode_allows_plan_file_edit`, `inactive_plan_mode_does_not_gate_edits`.
- `plan_mode.rs`: `is_plan_file_write_exact_match`, `is_markdown_file_path_recognizes_extensions`.

**Auto clippy/test must not fire on session `plan.md`.** Skip when `is_plan_file_write` or path is the tracker’s `plan_file_path()`. Markdown fmt-only is optional; rust clippy must not run.

Bash in plan mode is **not** blocked by this gate (comment ~233). Shell can still mutate files.

## 7. Non-Rust files

No product formatter matrix. `trailing_ws` is language-agnostic text. LSP diagnostics skip non-configured languages.

Suggested product policy (not implemented): `.rs` → rustfmt + scoped clippy + targeted tests; `.toml` / `.md` → skip clippy/test (optional trailing-ws already, or nothing); plan.md → skip rust pipeline.

## 8. Critical files for implementers

Attach at **tool layer after successful disk write**, not process mop.

1. `crates/codegen/xai-grok-tools/src/implementations/grok_build/search_replace/mod.rs` (`run_search_replace`, `SearchReplaceTool::run`)
2. `crates/codegen/xai-grok-tools/src/implementations/codex/apply_patch/tool.rs` (`ApplyPatchTool::run`)
3. `crates/codegen/xai-grok-tools/src/implementations/grok_build_hashline/edit/mod.rs`
4. `crates/codegen/xai-grok-tools/src/implementations/opencode/write/mod.rs` and `opencode/edit/mod.rs` (if those presets stay)
5. `crates/codegen/xai-grok-tools/src/notification/types.rs` + `handle.rs` (`FileWritten`)
6. `crates/codegen/xai-grok-shell/src/tools/notification_bridge.rs` (`ToolNotification::FileWritten`)
7. `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs` (`execute_tool_calls`, `prepare_tool_call`, `plan_mode_edit_gate`, PostToolUse)
8. `crates/codegen/xai-grok-shell/src/session/plan_mode.rs` (`is_plan_file_write`)
9. `crates/codegen/xai-grok-tools/src/reminders/lsp_diagnostics.rs` (pattern: post-edit side effect)
10. `crates/codegen/xai-grok-tools/src/implementations/grok_build/bash/mod.rs` (intercept pattern; also untracked file writes)
11. `crates/codegen/xai-grok-workspace/src/file_system/adapter.rs` + `acp_fs.rs` + `ext_fs.rs` (`FsWriteFileReq`)
12. `crates/codegen/xai-grok-agent/src/config.rs` (which toolset is live)
13. `crates/codegen/xai-grok-tools/src/util/trailing_ws.rs` (existing post-write transform)
14. `RESIDUAL.md` Open `feat:agentic-fmt-clippy-acp`

**Do not** hook only `PostToolUse` host scripts (Observe, per-call, operator-owned). Prefer `FileWritten` or a tools-crate helper called from each write success, then **debounce per turn / per path set**.

**Do not** run `cargo fmt -p` / `clippy --all-targets` on the whole workspace from this hook.

Suggested existing tests to extend: `basic_replacement`, `multiple_files_in_one_patch`, `plan_mode_allows_plan_file_edit`, `plan_mode_rejects_grok_edit_outside_plan_file_despite_allow_all_permissions`, `same_batch_plan_write_before_exit_plan_mode_returns_new_body`, `lock_path_for_args_*`, `file_written_round_trip_includes_previous_content`.
