# Plan: Running grok-oss sessions

## Context

The operator wants to see other running grok-oss TUI windows on this machine. Inventory on 2026-08-16 confirms that no such surface exists. The operator asked about seeing other running grok-oss processes. There is no dedicated list of live product windows today.

Existing nearby surfaces do other jobs and must stay distinct. Agent Dashboard is this pager process. Slash `/tasks` is this session. `grok-oss sessions` is disk history. `grok-oss leader list` is optional leader daemons. `/rebuild` can print PIDs as a one-shot side effect of signaling live product processes. There is no `/proc` walk of every grok-oss binary.

The current registry already writes `$GROK_HOME/active_sessions.json`. Each row is an `ActiveSession` with only `session_id`, `pid`, `cwd`, and `opened_at`. Register identity is `session_id` alone, so a second window on the same conversation overwrites the first. Unregister also keys on `session_id` only. `list` and `list_in` are unlocked reads and still include dead PIDs. Headless processes register only when `GROK_TRACK_HEADLESS` is already set. Default headless stays unlisted.

Constraints for this work: reuse that one registry; do not attach ACP into sibling PIDs; do not census `/proc` for every grok-oss; do not steal reserved slash names; never publish prompts, tool arguments, tokens, JWTs, file contents, or message text; leave default headless unlisted; keep leader daemons on `grok-oss leader list`; stay plan-only until the operator Approves.

Non-goals: merging this into Agent Dashboard; using the leader fleet roster as the source of truth; workspace-server or ptyctl; listing foreign Claude, Codex, or Cursor sessions; wiring the missing `grok-oss rebuild` CLI (the user-guide claims `grok-oss rebuild`, but clap has no `Rebuild` variant); expanding `GROK_TRACK_HEADLESS`; adding a new Unix socket bus; attaching to sibling PIDs.

Assumptions: one `$GROK_HOME` is one registry, and two homes do not see each other. Recycled PID safety stays `is_grok_process`. On Linux the process cmdline contains `"grok"`.

This plan is implementation guidance only. Do not start product edits until the operator Approves.

## Approach

The user-facing name is **Running grok-oss sessions**. It is not Agent Dashboard, not Tasks, and not the leader list.

Both surfaces show the same filtered data.

- TUI slash `/running` with the unused plain alias `/windows`. Do not steal `/dashboard`, `/sessions`, `/tasks`, `/resume`, or `/start`.
- CLI `grok-oss running`, with a human table and `--json`.

Reuse `$GROK_HOME/active_sessions.json`. Do not invent a second registry. Do not walk `/proc` for every grok-oss.

List only rows that pass `is_pid_alive` and `is_grok_process`. Show PID, short session id, cwd, `opened_at`, a this-window marker, busy or idle if known, and a safe short activity line.

Change register and unregister identity to `(pid, session_id)` so two windows on the same conversation both appear. Rebuild SIGUSR1 must still find every live product PID and keep dedupe by pid. Add a named test for that. On process exit, remove all rows for this pid, or each `(pid, session_id)` this process registered. If unregister stayed session-id-only after the key change, closing one window would drop siblings.

Each live TUI publishes a heartbeat into the same registry: `updated_at`, `activity` (`working` | `idle` | `unknown`), an optional session title from the existing on-disk session summary (not the latest user prompt), and an optional short safe activity line (model name, "turn running", "paused", subagent count). Never publish prompts, tool arguments, tokens, JWTs, file contents, or message text. If a sibling is live but has no heartbeat (old binary), still list it with activity `unknown`. That is honest, not fake idle.

Default headless stays unlisted unless `GROK_TRACK_HEADLESS` is already set. Do not expand that here. Leader daemons stay on `grok-oss leader list`. This view is standalone TUI windows. Do not attach ACP into sibling PIDs for v1. No new Unix socket bus.

`/running` prints a text report in the transcript, like `/tasks` or `/session-info`. Prefer that over a new fullscreen that competes with Agent Dashboard. Do not merge this into `/dashboard`. Refresh on open. Optional periodic refresh only if it is cheap (re-read a flock-safe list). A transcript report must not keep appending on a timer. Refresh-on-open is the v1 default. The CLI human table uses the same columns. `--json` is the same filtered rows, safe fields only.

Keep flock-safe list, composite key, and heartbeat update in `xai-grok-active-sessions`. Compose `is_grok_process` in a thin pager or CLI helper that both slash and CLI call, which is the same filter rebuild already uses. Do not add a shell-base dependency to the active-sessions crate unless a later review finds a strong reason.

Keep the on-disk file as a pretty JSON array. Do not rewrite it into a map unless a later step proves that is required.

## Critical files

| Path | Why |
|------|-----|
| `crates/codegen/xai-grok-active-sessions/src/lib.rs` | `ActiveSession` fields, register, unregister, list, lock, liveness helpers, and tests live here. |
| `crates/codegen/xai-grok-active-sessions/tests/smoke.rs` | Crate smoke coverage for the registry file and lock path. |
| `crates/codegen/xai-grok-pager/src/slash/commands/mod.rs` | Builtin slash registry. New `/running` must be added here the same way `/start` is. |
| `crates/codegen/xai-grok-pager/src/slash/commands/running.rs` | New slash command module. Name `running`, alias `windows`, transcript report. |
| `crates/codegen/xai-grok-pager/src/slash/commands/tasks.rs` | Transcript text-report pattern to copy (`status_blocks::tasks_block_text`), not `Action::OpenDashboard`. |
| `crates/codegen/xai-grok-pager/src/app/cli.rs` | Clap `Command` enum on `PagerArgs.command`. Add `Running { json: bool }` next to `Leader`, `Sessions`, and `DiskUsage`. |
| `crates/codegen/xai-grok-pager-bin/src/main.rs` | CLI dispatch match. Leader is near line 1834. Sessions is near line 1853. Do not overload `Command::Sessions`. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/start.rs` | Slash wiring mirror for registration. Copy the dispatch shape, not `/start` meaning. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/start.rs` | Slash dispatch test pattern to mirror for `/running`. |
| `crates/codegen/xai-grok-pager/src/app/actions.rs` | New action for the transcript report, next to `Action::StartPausedOrInterruptedWork`. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` | Route the new action to a small dispatch module. |
| `crates/codegen/xai-grok-shell/src/session/slash_commands.rs` | Must add `"running"` and `"windows"` to `PAGER_COMMAND_KEYS`, or `pager_builtin_triggers_are_reserved_in_shell` fails. Also `BUILTIN_COMMANDS` if that list advertises built-ins. |
| `crates/codegen/xai-grok-shell-base/src/util/mod.rs` | `is_grok_process` (Linux `/proc/{pid}/cmdline` contains `"grok"`). Compose this in the pager or CLI helper, not inside the active-sessions crate. |
| `crates/codegen/xai-grok-update/src/rebuild.rs` | `signal_active_sessions_to_relaunch` lists, dedupes by PID, skips self, dead, and non-grok, then SIGUSR1. Keep PID dedupe after the composite key. |
| `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` | TUI `Effect::RegisterActiveSession` and the heartbeat write seam. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/lifecycle.rs` | Session bind register path. Same four fields today. Heartbeat starts here on bind. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | Session load register path. Same bind-time register and later heartbeat. |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` | Document `/running` and the `/windows` alias. |
| `crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md` | Document live windows versus disk session history. |
| `crates/codegen/xai-grok-pager/docs/user-guide/23-dashboard.md` | Cite only to say this is not the Agent Dashboard and must not merge into `/dashboard`. |
| `FORK.md` | Short hierarchical ship note for the new surface. |
| `RESIDUAL.md` | Residual honesty so open work matches this plan. Do not invent non-goals. |

## Reuse

| Symbol / module | Path | How |
|-----------------|------|-----|
| `ActiveSession` | `crates/codegen/xai-grok-active-sessions/src/lib.rs` | Keep the four existing fields. Add optional heartbeat fields with serde defaults so old four-field JSON still loads. |
| `list` / `list_in` | same crate | Keep unlocked reads for callers that already use them. Add a flock-safe list API for UI and CLI. |
| `register_in` | same crate | Change retain-and-push identity from `session_id` alone to `(pid, session_id)`. |
| `unregister_in` / `try_unregister` | same crate | Unregister by pid (all rows for this process) or by each `(pid, session_id)` this process registered. Do not leave session-id-only remove. |
| `collect_crashed` | same crate | Already partitions with `is_pid_alive`. Keep that for crash cleanup. Live UI list adds `is_grok_process` in the composed helper. |
| `is_pid_alive` | same crate | Unix `kill(pid, 0)`, with EPERM still treated as alive. Reuse as the first liveness gate. |
| `DATA_FILENAME`, lock, tmp rename | same crate | Stay on `active_sessions.json`, `active_sessions.lock`, and `active_sessions.json.tmp` under `xai_grok_config::grok_home()`. |
| `is_grok_process` | `crates/codegen/xai-grok-shell-base/src/util/mod.rs` | Compose in a thin pager or CLI helper that slash, CLI, and rebuild already share in spirit. Do not add shell-base as a dependency of the active-sessions crate. |
| `signal_active_sessions_to_relaunch` | `crates/codegen/xai-grok-update/src/rebuild.rs` | Keep skip self, skip dead, skip non-grok, and `BTreeSet` PID dedupe after the composite key. Add a named test. |
| `StartCommand` | `crates/codegen/xai-grok-pager/src/slash/commands/start.rs` | Copy slash wiring only (`name()`, `builtin_commands()`, action, router, tests). Do not copy "continue paused work" behavior. |
| `TasksCommand` | `crates/codegen/xai-grok-pager/src/slash/commands/tasks.rs` | Copy the transcript text-report shape, not a fullscreen. |
| `Command` clap enum | `crates/codegen/xai-grok-pager/src/app/cli.rs` | Add `Running { json: bool }` beside `Leader`, `Sessions`, and `DiskUsage`. Do not overload `Sessions`. |
| `run_leader_mgmt` / `disk_usage_cmd` | `crates/codegen/xai-grok-pager-bin/src/main.rs` | Copy human table plus `--json` style. Product CLI name is `grok-oss`. |
| `Effect::RegisterActiveSession` | pager effects and session lifecycle or load | Keep bind-time register. Extend the same effect path or a sibling effect to write the heartbeat into the same file. |
| Session summary title | existing on-disk session summary (`summary.json` or equivalent) | Optional title field comes from that summary, never from the latest user prompt. |
| `PAGER_COMMAND_KEYS` | `crates/codegen/xai-grok-shell/src/session/slash_commands.rs` | Reserve `"running"` and `"windows"` so the shell reservation test stays green. |

## Steps

1. **Registry.** Add a flock-safe list API in `xai-grok-active-sessions` for UI and CLI. Add a composed live-list helper in the pager or CLI layer that keeps only rows passing `is_pid_alive` and `is_grok_process`. Change `register_in` and unregister identity to `(pid, session_id)` so two windows on the same conversation both stay in the array. On process exit, remove every row for this pid (or each `(pid, session_id)` this process registered). Keep the on-disk format as a pretty JSON array. Add optional heartbeat fields with serde defaults so old four-field JSON still loads, and missing heartbeat means activity `unknown`. Do not rewrite the file into a map unless a later review proves it is required. Named crate tests in this crate own the composite key, dead-pid drop, sibling-on-same-session, and old-JSON load. This step is the foundation for heartbeat, slash, CLI, and rebuild.

2. **Heartbeat.** After the registry key change, each live TUI writes `updated_at`, `activity` (`working` | `idle` | `unknown`), an optional title from the existing on-disk session summary, and an optional short safe activity line (model name, "turn running", "paused", subagent count) on bind and on turn state change. Write into the same JSON file with the same exclusive flock and atomic tmp rename as register. Never write prompts, tool arguments, tokens, JWTs, file contents, or message text. A sibling that is live but has no heartbeat (old binary) stays listable as `unknown`. Default headless still registers only when `GROK_TRACK_HEADLESS` is already set. This step depends on the registry step.

3. **Slash `/running` plus tests (failing tests first).** Add `running.rs` with `name()` `"running"` and unused alias `"windows"`. Wire `pub mod running` and `Arc::new(running::RunningCommand)` in `slash/commands/mod.rs`. Add an action and a small dispatch module, mirroring `/start` wiring only. Print a transcript table like `/tasks`, including a this-window marker. Refresh on open. Do not append the same report on a timer. Add `"running"` and `"windows"` to `PAGER_COMMAND_KEYS` (and `BUILTIN_COMMANDS` if that list advertises built-ins). Write fixture tests that plant a sibling row and assert the transcript lists it. Observe the named slash test fail before the product command exists, then land the smallest wiring that makes the same test pass. This step depends on the registry step, and on heartbeat for activity columns.

4. **CLI `grok-oss running` plus `--json` plus tests (failing tests first).** Add `Running { json: bool }` on the clap `Command` enum. Dispatch it in `xai-grok-pager-bin` next to Leader and Sessions. Do not overload `Command::Sessions`. Human table columns match the slash report. `--json` emits the same filtered rows and safe fields only. Observe the named CLI test fail first, then land the smallest dispatch that makes the same test pass. This step depends on the registry step, and on heartbeat for activity columns.

5. **Docs.** Update user-guide `04-slash-commands.md` and `17-sessions.md` with complete American English thoughts. In `23-dashboard.md`, say only that **Running grok-oss sessions** is not the Agent Dashboard and must not merge into `/dashboard`. Add a short `FORK.md` note. Update `RESIDUAL.md` so open work matches this plan. Do not invent non-goals. Do not mention billing meters. This step can follow slash and CLI.

6. **Rebuild.** After the composite key change, `signal_active_sessions_to_relaunch` must still signal each live product pid once. Keep skip self, skip dead, skip non-grok, and `BTreeSet` PID dedupe. Add the named rebuild test and run it red, then green, in `xai-grok-update`. This step depends on the registry step. It does not wire a `grok-oss rebuild` clap variant.

## Risks

- Two windows share one `session_id` and the current register overwrites the first row. Mitigation. Change identity to `(pid, session_id)` and add `list_live_includes_two_windows_on_the_same_session_id`.
- Unregister that still keys on `session_id` would drop every window on that conversation when one exits. Mitigation. Unregister by pid (all rows for this process) or by each `(pid, session_id)` this process registered, and test that closing one window leaves the sibling.
- Rebuild could signal the same pid twice after two rows share a pid, or miss a pid after the key change. Mitigation. Keep `BTreeSet` PID dedupe and add `rebuild_signals_each_pid_after_composite_key`.
- Old four-field JSON and old binaries have no heartbeat. Mitigation. Serde defaults; list those live rows as `unknown`, not fake idle. Named test for old JSON.
- Publishing a title from the latest user prompt would leak private text. Mitigation. Title comes only from the existing on-disk session summary. `heartbeat_omits_prompt_text` and CLI `--json` must not contain prompt text.
- A recycled PID could look alive. Mitigation. Keep `is_grok_process` in the composed live filter, the same way rebuild already does.
- A new fullscreen would compete with Agent Dashboard and invite a merge. Mitigation. Transcript report only, refresh on open, do not steal `/dashboard` or `/sessions`.
- Adding `xai-grok-shell-base` to the active-sessions crate would thicken a small registry crate. Mitigation. Keep flock, key, and heartbeat in active-sessions. Compose `is_grok_process` in the pager or CLI helper.
- Expanding headless tracking would list batch jobs the operator did not ask to see. Mitigation. Leave `GROK_TRACK_HEADLESS` unchanged.
- A second registry or a Unix socket bus would split truth and add process coupling. Mitigation. One JSON file, flock, no ACP attach, no new bus.
- Forgetting `PAGER_COMMAND_KEYS` breaks `pager_builtin_triggers_are_reserved_in_shell`. Mitigation. Add `"running"` and `"windows"` in the same slash step.
- Overloading `Command::Sessions` would mix disk history with live windows. Mitigation. New `Running` variant and `grok-oss running` only.

## Verification

Proof order is failing tests first, then the smallest product fix that makes the same test pass. Do not reshape asserts to match code. Use package-scoped `cargo test` filters. Do not run crate-wide clippy or `just check` as proof of this slice.

Required named tests, or equivalent names in the right crates:

- `list_live_includes_two_windows_on_the_same_session_id`
- `list_live_drops_dead_pid`
- `running_slash_lists_sibling_fixture_row`
- `heartbeat_omits_prompt_text`
- `rebuild_signals_each_pid_after_composite_key`

Recommended size-1 leaves if they fit cleanly:

- Unregister one window leaves the sibling on the same session id.
- Old JSON without heartbeat fields lists as `unknown`.
- CLI `--json` omits prompt text.

Suggested commands (adjust the filter string to the final test name):

- Red then green for the composite-key live list: `cargo test -p xai-grok-active-sessions --lib list_live_includes_two_windows_on_the_same_session_id`
- Red then green for dead-pid drop: `cargo test -p xai-grok-active-sessions --lib list_live_drops_dead_pid`
- Red then green for old JSON: `cargo test -p xai-grok-active-sessions --lib` with the old-JSON `unknown` test name
- Red then green for heartbeat privacy: `cargo test -p xai-grok-active-sessions --lib heartbeat_omits_prompt_text` (or the pager crate if the write seam test lives there)
- Red then green for slash: `cargo test -p xai-grok-pager --lib running_slash_lists_sibling_fixture_row`
- Red then green for CLI JSON privacy: `cargo test -p xai-grok-pager-bin` (or the crate that owns the running CLI test) with the `--json` omits-prompt-text filter
- Red then green for rebuild: `cargo test -p xai-grok-update --lib rebuild_signals_each_pid_after_composite_key`
- After reserving the slash names: `cargo test -p xai-grok-shell --lib pager_builtin_triggers_are_reserved_in_shell`

Each implementer log must name the test, the command, and the fail reason before the product edit. The same filter must pass after the fix.

Manual check after green tests: open two TUI windows on one conversation, run `/running` in one, and confirm both rows. Run `grok-oss running` and `grok-oss running --json` and confirm the same filtered, safe fields. Confirm `/dashboard`, `/sessions`, `/tasks`, `/resume`, and `/start` are unchanged. Confirm `grok-oss leader list` is unchanged.

## Open questions

- Slash name. Default is `/running` with unused alias `/windows`, not `/ps`. Approve may keep this default.
- Report shape. Default is a transcript table like `/tasks`, not a second fullscreen dashboard. Approve may keep this default.
- Heartbeat transport. Default is the same `$GROK_HOME/active_sessions.json` file with flock, not a new Unix socket. Approve may keep this default.
- Old binaries. Default is to list live siblings that have no heartbeat as activity `unknown`, not fake idle. Approve may keep this default.
