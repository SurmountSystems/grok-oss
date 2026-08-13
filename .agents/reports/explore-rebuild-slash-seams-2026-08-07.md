# Explore: `/rebuild` slash seams (2026-08-07)

Inventory for a product slash that rebuilds `grok-oss` and relaunches live
instances. Complements
[`.agents/reports/plan-rebuild-reboot-graceful-2026-08-07.md`](plan-rebuild-reboot-graceful-2026-08-07.md).

---

## 1. Slash registration / handling

| Piece | Path | Role |
|-------|------|------|
| Trait + `CommandResult` | `crates/codegen/xai-grok-pager/src/slash/command.rs` — `SlashCommand`, `CommandResult` | Sync `run()`; async via `Action` / special variants |
| Registry | `…/slash/registry.rs` — `CommandRegistry::new`, `set_acp_commands` | Builtin + ACP; panics on name/alias collision |
| Builtin list (SoT) | `…/slash/commands/mod.rs` — `builtin_commands()` | Append new `Arc::new(rebuild::…)` here |
| Dispatch | `…/app/dispatch/prompt.rs` | Maps `CommandResult` → scrollback / `dispatch(Action)` / `dispatch_doctor` |
| ACP skills | `…/slash/acp_command.rs` | Skills → `InjectSkill` / `PassThrough`; not for local rebuild |

**No `/update` slash.** Update is CLI only on `grok-oss update` in
`xai-grok-pager-bin`. Install-ish product UX: `/plugin` (modal), doctor fix
effects, not cargo install.

Pattern for a new command: module under `slash/commands/`, implement
`SlashCommand`, register in `builtin_commands()`, return
`CommandResult::Action(…)` (or a dedicated result) so the event loop owns work.

---

## 2. Relaunch protocol (usable without xAI updater)

| Symbol | Path | Role |
|--------|------|------|
| `ControlCommand::RelaunchForUpdate { to_version }` | `xai-grok-shell/src/leader/protocol.rs` | Ask leader: drain → flush → `ShutdownReason::AutoUpdate` |
| `relaunch_v1` | same; `LeaderCapabilities` | Cap flag; old leaders decline gracefully |
| `decide_relaunch_for_update` / `spawn_relaunch_drain` | `leader/server.rs` | Strictly-newer semver guard; ~5s idle grace + 5s flush; force if busy |
| `leader_is_older_than` | `leader/mod.rs` | Parseable **semver** only; unparseable → no relaunch |
| `discover_leaders` | `leader/mod.rs` | Scan `~/.grok/leader*.sock` (+ locks); `LeaderDiscoveryState::Reachable` |
| `signal_leaders_to_relaunch` | `xai-grok-pager-bin/src/main.rs` | Best-effort: discover → connect → `RelaunchForUpdate` |
| Client vacate | `leader/mod.rs` — `request_leader_vacate` | Same control when client is newer; else SIGTERM |

**Call site today:** only after `GROK_OSS_ENABLE_XAI_UPDATER=1` path succeeds
inside `run_update_command` (xAI binary install). OSS default never calls it.

**Without xAI updater:** protocol is independent. Callers can reuse
`discover_leaders` + `LeaderClient::send_control(RelaunchForUpdate { … })` (or
factor `signal_leaders_to_relaunch` out of pager-bin). **Hard constraint:**
`to_version` must be **strictly greater parseable semver** than
`leader_binary_version`. Same package version + new git SHA alone will
**decline**. OSS may need a policy (bump version, or relax/alternate guard for
git-SHA rebuilds).

Also: `Action::QuitForUpdate` / Ctrl+U finish-update path is xAI-oriented; OSS
disables background auto-update unless the env flag is set.

---

## 3. `active_sessions` (live PIDs)

`crates/codegen/xai-grok-shell/src/active_sessions.rs`

| API | Role |
|-----|------|
| `ActiveSession { session_id, pid, cwd, opened_at }` | Row shape |
| `register` / `register_in` | Idempotent by session_id |
| `unregister` / `try_unregister` | Clean exit; non-blocking for signals |
| `collect_crashed` | Dead-PID orphans |
| `list_in(root)` | Read JSON (tests / injectable root) |

**No public `list()`** for default grok home; thin wrapper over
`list_in(&grok_home())` is easy. Store: `~/.grok/active_sessions.json` + flock.

Pager: `Effect::RegisterActiveSession` / `UnregisterActiveSession`
(`app/effects/mod.rs`, `actions.rs`). Live set for reboot ≈ alive rows here
**plus** reachable leaders (clients may share one leader).

---

## 4. OSS update messaging (not install)

| API | Path |
|-----|------|
| `how_to_update_message`, `check_against_main`, `print_oss_update_status`, `OssUpdateStatus` | `xai-grok-update/src/oss_update.rs` |
| `run_update_command` | `xai-grok-pager-bin/src/main.rs` |

Default: `--check` vs Surmount `main` SHA; bare `update` prints “no auto-install”
+ `just install` recipe. No multi-instance reboot on OSS path.

---

## 5. `just install`

`justfile` recipe `install` (~L373–388):

1. `cargo build --release -p xai-grok-pager-bin --locked` (rustflags override wild linker)
2. `strip --strip-unneeded target/release/grok-oss`
3. Install → **`${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss`**
4. `--version` + stripped check

Siblings: `install-dist`, `install-nix`. Source tree is the workspace with that
justfile (or operator-configured clone).

---

## 6. Can a slash shell out to `just install` / cargo?

| Concern | Finding |
|---------|---------|
| Slash sync model | `run()` must not block multi-minute builds; use `Action` → `Effect` → `JoinSet` + `TaskResult` |
| Agent shell permissions | N/A for a **builtin** effect that runs product `tokio::process` / `spawn_blocking`; not the agent `execute` tool path |
| Agent tool “rebuild” word | `auto_mode.rs` lists npm/yarn subcommand `"rebuild"` only; unrelated |
| Safety | Prefer fixed argv (`just install` / `cargo build -p xai-grok-pager-bin`) from known repo root; not freeform shell; surface cwd/source choice |
| Self-replace | Running process may rewrite the on-disk binary then relaunch/exec; same class as update restart |

---

## 7. Long-running work from slash (existing patterns)

| Pattern | Where | Notes |
|---------|--------|------|
| `CommandResult::Action` → dispatch | `prompt.rs` | Default async handoff |
| Doctor: `CommandResult::Doctor` → `Effect::PlanDoctorFix` / `ApplyDoctorFix` | `effects/mod.rs`, `task_result.rs` | `spawn_blocking` + `TaskResult::*` → scrollback/toast |
| Plugin CTA install | `Effect` + `TaskResult::CtaPluginInstallDone` | ACP round-trip async |
| Export / login / tasks | thin Action | Fast UI, not multi-minute |
| Toasts | `agent.show_toast`, dashboard `set_error_toast` | No dedicated slash spinner; turn/tool chrome exists for agent work |

**No** existing slash that runs a multi-minute host cargo build. Closest
template: doctor Effect/TaskResult + Message/toast progress. Slash system is
event-driven only (export comment: no tick polling for slash).

---

## 8. Is `/rebuild` free?

| Surface | Collision? |
|---------|------------|
| Builtin slash names/aliases | **No** `rebuild` in `commands/` or `builtin_commands()` |
| CLI `Command::` | No rebuild subcommand |
| Worktree | `x.ai/git/worktree/db/rebuild` (ext RPC / `worktree_cmd`), not a slash |
| Git slash | No `/rebuild` |
| auto_mode allowlist | `"rebuild"` = package-manager subcommand only |

**Name is free** for a new builtin slash. Prefer documenting distinction from
worktree DB rebuild in help text.

---

## Implementation sketch (seams only)

1. `slash/commands/rebuild.rs` + register in `builtin_commands()`.
2. New `Action` / `Effect` / `TaskResult` trio: resolve source root → run
   `just install` (or equivalent) async → on success, read new binary version
   → `signal_leaders_to_relaunch` (shared lib, not only pager-bin) → optional
   self `QuitForUpdate` / re-exec for this client.
3. Report live targets via `list_in` + `discover_leaders` before/after.
4. **Resolve semver guard** for same-version git SHA installs or relaunch will
   no-op.

---

## Key absolute paths

- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/slash/`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/leader/`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/active_sessions.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-update/src/oss_update.rs`
- `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager-bin/src/main.rs` (`run_update_command`, `signal_leaders_to_relaunch`)
- `/home/hunter/Projects/surmount/grok-build/justfile` (`install`)
