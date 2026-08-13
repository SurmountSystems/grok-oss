# Implement: Ctrl+C plan, killall resume, multi-track also-guard

**Date:** 2026-08-07
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Plan:** `.agents/plans/plan-multi-track-also-guard-2026-08-07.md`
**Boards:** `bug:plan-approval-ctrl-c-still`, `bug:killall-no-graceful-resume`, `feat:multi-track-also-guard`

## Status

**Done** for this vertical. No invent free SuperGrok period debit. No git mutate.

---

## 1. Ctrl+C plan approval

**Already green in source** (prior fix). Re-verified unit tests:

| Test | Result |
|------|--------|
| `soft_park_empty_ctrl_c_abandons_plan_approval` | pass |
| `plan_panel_empty_ctrl_c_abandons_plan_approval` | pass |
| `plan_approval_ctrl_c_clears_draft_then_second_abandons` | pass |

Empty-composer Ctrl+C abandons plan approval (same as panel Quit). Non-empty clears draft; second empty abandons. No additional product edit required this pass. Dogfood on installed binary remains operator-side after rebuild.

---

## 2. SIGTERM / killall graceful resume

### Gap (pre-fix)

`Action::Quit` (first SIGTERM / `killall` default / `/exit`) did not write `canceled_turn_resume.json`, so reopen never re-queued mid-turn.

### Ship

| Piece | Change |
|-------|--------|
| Shared write | `write_cancel_resume_marker_for_session` in `dispatch/turn.rs` (same marker shape as Esc) |
| Graceful quit | `persist_cancel_resume_on_graceful_quit` called from `Action::Quit`, `QuitConfirmed`, `QuitForUpdate` |
| Esc / rebuild | Still use `do_cancel_turn_for(..., allow_local_rewind: true)` which now calls the shared writer |
| Load path | Unchanged: existing `resume_canceled_turn_on_restart` re-queues once + toast |
| SIGKILL | Documented only — no userspace handler can write a marker |

### Tests (red→green intent)

| Test | Contract |
|------|----------|
| `quit_mid_turn_writes_canceled_turn_resume_marker` | mid-turn + Quit writes marker; auto-resume eligible |
| `quit_idle_does_not_write_canceled_turn_resume_marker` | idle Quit invents nothing |
| `process_shutdown_class_marker_is_auto_resume_eligible` | shell unit: same marker shape |

### Docs

- Module docs on `canceled_turn_resume.rs`
- User-guide `17-sessions.md` (writes vs does not write; kill -9 honesty)
- `FORK.md` inventory updated

---

## 3. Multi-track also-guard (first cut)

| Piece | Change |
|-------|--------|
| Bind field | `meta.taskId` (camelCase) documented on `TodoUpdate` schema + `todo_bound_task_id` |
| Guard | `check_live_demote_guard`: reject `in_progress`→`pending` when bound id is still Running |
| Wire | `TodoWrite` execute queries `SubagentBackendResource` for live status before merge/replace |
| Allow | complete, cancel, unbound demote, finished subagent demote |
| Teach | `prompt.md` Planning + `TodoWriteTool` description; encrypted templates regenerated |
| Optional sticky | **Not invented** — title Agents + queue-hold chrome already exist; full sticky-on-new-message stays soft residual |

### Tests

| Test | Result |
|------|--------|
| `live_demote_guard_rejects_bound_running_to_pending` | pass |
| `live_demote_guard_allows_complete_and_cancel_while_bound` | pass |
| `live_demote_guard_allows_unbound_demote` | pass |
| `live_demote_guard_allows_bound_when_subagent_finished` | pass |
| `todo_bound_task_id_reads_camel_case_task_id` | pass |
| `test_base_template_plan_present_includes_planning` (taskId) | pass |

---

## 4. Residual / FORK honesty

- `FORK.md`: resume-after-kill, Ctrl+C plan, multi-track first cut checked in
- `RESIDUAL.md` §2i: first cut shipped; soft remainders (auto-bind, sticky-on-new-message, full UI)
- `AGENTS.md`: product bind first cut note under multi-track bullet

---

## Commands run

```bash
cargo fmt -p xai-grok-pager -p xai-grok-tools -p xai-grok-shell -p xai-grok-agent
python3 crates/codegen/xai-grok-agent/scripts/encrypt_templates.py

cargo test -p xai-grok-tools --lib -- live_demote_guard todo_bound_task_id
cargo test -p xai-grok-pager --lib -- \
  quit_mid_turn_writes_canceled quit_idle_does_not_write \
  soft_park_empty_ctrl_c_abandons plan_panel_empty_ctrl_c_abandons \
  plan_approval_ctrl_c_clears_draft
cargo test -p xai-grok-shell --lib -- canceled_turn_resume process_shutdown_class
cargo test -p xai-grok-agent --lib -- test_base_template_plan_present_includes_planning

cargo clippy -p xai-grok-pager -p xai-grok-tools -p xai-grok-shell -p xai-grok-agent --lib -- -D warnings
```

All listed filters green; clippy clean (`-D warnings`).

---

## Key paths

| Concern | Path |
|---------|------|
| Quit marker write | `crates/codegen/xai-grok-pager/src/app/dispatch/turn.rs` |
| Quit arm | `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` |
| Marker module | `crates/codegen/xai-grok-shell/src/session/canceled_turn_resume.rs` |
| Load re-queue | `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` (unchanged) |
| Demote guard | `crates/codegen/xai-grok-tools/src/implementations/grok_build/todo/mod.rs` |
| Prompt teach | `crates/codegen/xai-grok-agent/templates/prompt.md` + `prompt_encrypted.rs` |
| Sessions docs | `crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md` |

---

## Out of scope / not done

- SIGKILL magic (impossible)
- Free SuperGrok period debit invention
- Auto-bind every Task without agent meta
- New sticky toast on every user message while multi-track live
- Git add / commit / push
- Operator dogfood on installed binary after rebuild
