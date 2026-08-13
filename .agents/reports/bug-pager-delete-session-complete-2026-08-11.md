# DeleteSessionComplete honors `after` (pager residual 2)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Parent diagnosis:** `.agents/reports/bug-pager-mass-fail-root-2026-08-11.md` § Residual 2
**Agent:** L2 implementer

---

## Status

**Green** for the DeleteSessionComplete / lifecycle delete / dashboard stop cluster.

---

## Root cause

`TaskResult::DeleteSessionComplete` was toast-only and ignored `after`:

```rust
TaskResult::DeleteSessionComplete { source, session_id, after: _ } => {
    remove_session_from_pickers(app, &source, &session_id);
    app.show_toast("Session deleted");
    vec![]
}
```

So Welcome/Dashboard complete never removed agents, never switched view, never unregistered, never moved dashboard selection.

Related half-merges on the same cluster:

1. `/delete` answered set `after` from `active_view == AgentDashboard` instead of attached-agent (monorepo `after_delete_current_session`).
2. `dispatch_dashboard_stop` still used legacy `stop_confirm` + immediate `dispatch_sessions_confirm_close` instead of monorepo `delete_confirm` + `Effect::DeleteSession` (agent removed only on complete).

---

## Product fix

### 1. `task_result.rs` — full `AfterSessionDelete` handler

Port monorepo logic against current 3-arg `remove_session_from_pickers` (no 4th match-id-only flag):

| `after` | Behavior |
|---------|----------|
| **Stay** | Drop picker rows + roster rows for that session id; toast; no agent remove |
| **Dashboard** | Remove matching agents; clear attach if needed; focus neighbor / new-agent button when selected row closed; if foreground agent, land on `AgentDashboard`; `UnregisterActiveSession` |
| **Welcome** | Remove matching agents; if foreground, `dispatch_exit_session` (welcome + unregister path); else just unregister |

Also: `dashboard_neighbor_row` is `pub(super)` so the complete handler can pick the next row before agents are removed.

### 2. `session/lifecycle.rs` — `/delete` aftermath

Restored monorepo `after_delete_current_session(app, id)`:

- `Dashboard` when `dashboard.attached_agent == Some(id)`
- else `Welcome`

Used for question copy and for `Effect::DeleteSession.after`.

### 3. `dashboard.rs` — Ctrl+X stop → arm/delete history

Restored monorepo path:

- Idle top-level / eligible roster: `arm_or_delete` → second press `delete_dashboard_row` → `DeleteSession { after: Dashboard }`
- Busy top-level: `stop_top_level_activity` (cancel turn / kill bg / drop loops+queue); never arm
- Subagent: `KillSubagent` (unchanged)

Agents leave the roster only when `DeleteSessionComplete` runs (neighbor focus lives there).

### 4. Tests (surgical, monorepo contract)

Dashboard stop tests still asserted legacy `stop_confirm` + immediate agent drop. Updated those specs to monorepo:

- arm `delete_confirm`
- second press emits `DeleteSession`
- selection / agent removal after `DeleteSessionComplete`

Lifecycle complete/confirm tests needed no expect changes.

---

## Files touched

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs` | Full `DeleteSessionComplete` + `remove_agent_and_cleanup` import |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/lifecycle.rs` | `after_delete_current_session` for `/delete` |
| `crates/codegen/xai-grok-pager/src/app/dispatch/dashboard.rs` | `dashboard_neighbor_row` visibility; monorepo `dispatch_dashboard_stop` + helpers |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/dashboard.rs` | stop tests → delete_confirm + complete |

No git commit/add/push.

---

## Evidence (red → green)

**Red (before product fix):**

```
delete_current_session_complete_returns_to_dashboard — active_view not AgentDashboard
delete_current_session_complete_welcome_and_guard — active_view not Welcome
```

**Green filters (after):**

```text
cargo test -p xai-grok-pager --lib -- \
  delete_current_session_ \
  lifecycle::dashboard_stop \
  dashboard::dashboard_stop_double \
  dashboard::dashboard_stop_moves \
  dashboard::dashboard_stop_last \
  dashboard::dashboard_stop_does_not \
  dashboard::dashboard_stop_subagent \
  task_result::delete_ \
  delete_session_action

# 14 + 5 = 19 related tests ok
```

Named contracts:

| Test | Result |
|------|--------|
| `delete_current_session_complete_returns_to_dashboard` | ok |
| `delete_current_session_complete_welcome_and_guard` | ok |
| `delete_current_session_confirm_*` (Welcome/Dashboard after) | ok |
| `dashboard_stop_*` (lifecycle + dashboard modules) | ok |
| Stay-path picker identity deletes (`task_result::delete_*`) | ok |

`cargo fmt -p xai-grok-pager` run. Package-wide `clippy -D warnings` still fails on **pre-existing** dead_code across slash/session_picker (not introduced by this slice). Touched product paths compile under the same unit test build.

---

## Out of scope / residual

- Full session lifecycle CreateSession / deferred switch (mass-fail residual 4)
- `dispatch_dashboard_delete` (`y` confirm) still only referenced from tests/docs; monorepo wires it from the dashboard key path — separate if CI complains about dead_code on that symbol under stricter gates
- Footer render still mentions `stop_confirm` in places; product arm is `delete_confirm` for idle Ctrl+X

---

## 8-line summary

1. Complete handler ignored `after` (toast + pickers only).
2. Restored Stay / Dashboard / Welcome monorepo behavior with 3-arg pickers.
3. `/delete` after now follows attached-agent, not bare `AgentDashboard` view.
4. Dashboard Ctrl+X arms delete and emits `DeleteSession`; complete removes agents and moves selection.
5. Stale stop_confirm dashboard tests updated to monorepo delete contract.
6. 19 related unit tests green; no git mutation.
