# Shell / pager compile mop — onto-xai land

**Date:** 2026-08-10
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior:** `.agents/reports/impl-upstream-filters-land-2026-08-10.md`
**Catalog:** `doc/dev/upstream-regression-filters.md`

---

## Executive status

| Item | State |
|------|--------|
| **`cargo check -p xai-grok-shell --lib`** | **GREEN** (warnings only) |
| **`cargo check -p xai-grok-shell --tests` / `--all-targets`** | **RED** (~297 lib-test errors: missing testkit, tip registry APIs in tests, deps) |
| **`cargo check -p xai-grok-pager --lib`** | **RED** (~290 errors: half-merge UI, Btw overlay, rewind, slash trait, dups) |
| **Stashes** | `recon-temp-work-b-wip-2026-08-10`, `recon-resume-local-dirt-2026-08-10` **kept** |
| **Shell/pager catalog filters** | **Blocked** (need shell cfg(test) + pager lib) |
| **Non-shell catalog filters** | Still green (re-run sample: sampling-types, pager-render DOGE, sampler retry) |
| **Push** | **Not done** |

**Bottom line:** Product shell **library** compiles on the onto tip after surgical tip/main alignment. Shell unit/integration tests and the full pager crate still do not compile. Prior filter-agent mop kept and extended.

---

## Strategy used

Half-merges (tip call sites vs main structs) were the main fail mode. Working alignment:

| Layer | Choice |
|-------|--------|
| Session actor / spawn / run_loop / slash_exec / sampler_turn | Tip monorepo APIs (`spawn_session_on_thread` 105-arg, `Shutdown(ShutdownKind)`, dual-auth `SamplerConfig`, economic mode, SetSessionModel tokens) |
| Agent stack (`MvpAgent`, HashMap sessions, no session_registry) | Main-shaped agent; call sites adapted (`sessions.borrow_mut()`, no `with_resident_mut`) |
| Product seams | Dual-auth credentials → sampler, Btw `attempts`, managed MCP merge arity, todo command arms, SettingsFetch `.into_option()` |

---

## Shell compile fixes (this pass)

Surgical only (no bulk replace):

1. **`spawn_session_on_thread` arity (105):** agent_ops + handle_request: drop tip-unwanted extras (`auto_compact_threshold_tokens`, managed MCP expires), insert `subagents_max_depth` + `workflow_max_concurrent_agents`, append `is_chat_kind`.
2. **`SessionSpawnOptions`:** add `is_chat_kind`; plumb through agent_ops / acp_agent / chat spawn options / session_setup.
3. **SettingsFetch:** `fetch_settings_blocking` → `.into_option()` (agent_ops, models).
4. **UnblockResult:** tip is tier-only; drop `unblocked.settings` apply path.
5. **lookup_session_model:** tip 2-arg API (model id + default).
6. **Telemetry:** `SubagentLaunched` / `SubagentCompleted` owner + workflow_run_id + queued/session fields.
7. **Coordinator / ChildRunRequest:** `limits` / `limit_sink`; ignore `queued_for` / `session_running`.
8. **Shutdown:** `SessionCommand::Shutdown(ShutdownKind::Graceful)`.
9. **CompactionConfig spawn:** `threshold_tokens`, `economic_mode` (from disk), `model_context_window`.
10. **slash_exec:** full `/economic-mode` arm (status / toggle / global persist).
11. **sampler_turn:** dual-auth fields + stashed/session bearer resolvers from credentials.
12. **run_loop:** SetSessionModel tokens; `save_mcp_server_enabled_in` arg order; arms for SetAutoCompactThreshold / RestoreTodoBoard / ClearCompletedTodos.
13. **MCP:** tip `start_mcp_servers` 5-arg + `McpSpawnCtx::session_less`; hooks re-merge with managed configs from handle.
14. **Misc:** RosterEntry `last_turn_summary`, BtwEntry `attempts`, turn slash `resolve` 5-arg (no LoopFireMode), model_switch resident mut via HashMap.

Kept prior mop: proxy-types, test-support, tools, sampling-types, sampler, fast-worktree, pager-render DOGE.

---

## Residual

### Shell lib tests (~297)

Classes seen:

- Missing `session::testkit`, `pretty_assertions`, `ctor` (dev-dep / feature wiring)
- Tests still call tip registry APIs (`insert_resident`, `with_resident_mut` patterns, CloseOutcome constants)
- Managed MCP merge arity in tests; missing product helpers (`startup_hints_from_meta`, `subagent_override_auth_rank_flags`, `PROACTIVE_MIN_SLEEP`)

### Pager lib (~290)

Top classes:

- Missing methods/fields (E0599/E0609) — product/tip UI drift
- Unresolved imports / modules (E0432/E0433)
- Btw overlay state field renames; RewindPhase / PermissionFocus exhaustiveness
- Duplicate derives on `McpSetupFormState` / `McpSetupOutcome`
- SlashCommand trait methods `mode_support` / `provenance` not on trait
- Missing `cli_known_mcp_server_names`, kitty helpers, roster `last_turn_summary`

### Catalog filters needing shell/pager

| Filter area | Blocked by |
|-------------|------------|
| shell_collision, titles, plan soft-park, dual SuperGrok in shell/pager | pager + shell cfg(test) |
| stream_resumed shell test | shell lib test compile |
| settings_e2e | pager |

Non-shell packages (rate-limit, shared hide_header, sampling-types, sampler unit, tools densify, pager-render DOGE) remain the green gate from the prior report.

---

## Commands log

```text
cargo check -p xai-grok-shell --lib          # GREEN (46 warnings)
cargo check -p xai-grok-shell --tests        # RED ~297
cargo check -p xai-grok-pager --lib          # RED ~290
./scripts/assert-process-pins.sh HEAD        # (re-run with commit)
# catalog shell/pager: not run (compile blocked)
```

---

## Commit / stashes

- Recon-unsigned commit of mop on tool branch when authorized (this report accompanies it).
- **No push.**
- Stashes **not** dropped.

---

## Suggested next mop (not done)

1. Shell **test** compile: restore `session::testkit` + dev-deps; adapt tests to HashMap sessions or restore minimal registry test helpers.
2. Pager: whole-side restore of conflicted UI modules from main **or** tip (avoid half-merge); then product DOGE/titles/shell_collision filters.
3. Re-run full catalog + optional `just check`.

---

## Success criteria

| Criterion | Met? |
|-----------|------|
| Shell lib green | **Yes** |
| Shell all-targets / lib+tests green | **No** |
| Pager lib green | **No** |
| Catalog shell/pager filters | **No** (blocked) |
| Stashes kept | **Yes** |
| No push | **Yes** |
| Report path | **This file** |
