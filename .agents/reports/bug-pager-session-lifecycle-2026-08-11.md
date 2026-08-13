# Session lifecycle / CreateSession / attach / deferred switch

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Parent diagnosis:** `.agents/reports/bug-pager-mass-fail-root-2026-08-11.md` § Residual 4
**Agent:** L2 implementer

---

## Status

| Item | Value |
|------|--------|
| **Sample baseline (session module)** | **249 pass / 28 fail** (before product edit; dirty compiling WIP) |
| **Lifecycle-only red (11)** | CreateSession attach, deferred switch, chat_kind stamp, pre-session cycle, sticky worktree branch, orphan attach restore |
| **Product fix** | Landed monorepo path restores (lifecycle + attach + deferred + chat + cycle + worktree branch + type seams) |
| **Lifecycle module compile** | Product paths for this slice no longer error (`conversation_entry` field + `deferred_model_switch_from_cli` type restored) |
| **Verify re-run** | **Blocked** — pager lib still fails compile (~321 errors: half-merge dups, missing modules, rewind exhaustiveness, telemetry, slash trait, …) |
| **Remaining red (session module)** | **Unknown** until lib compiles; expected lifecycle 11 → 0 if product paths hold; fork (5) + load (12) still separate residual |

---

## Sample red (lifecycle cluster)

| Filter | First panic / assert |
|--------|----------------------|
| `switch_model_deferred_when_no_session_id` | expected persist-only, got `[]` |
| `deferred_switch_threads_stash_prev_into_effect` | `SwitchModel.prev_model_id` not stash prev |
| `deferred_switch_prefers_authoritative_current_as_prev` | prev not session `models.current` |
| `dispatch_new_session_repoints_dashboard_attached_agent` | `attached_agent` stayed `Some(0)` not `Some(1)` |
| `dispatch_new_session_repoints_attach_while_subagent_view_open` | attach did not follow top-level parent |
| `session_failed_orphan_restores_dashboard_attach_to_survivor` | attach never moved onto orphan first |
| `session_failed_last_orphan_clears_dashboard_attach` | Welcome recovery left attach set |
| `chat_mode_new_session_creates_with_chat_kind` | `conversation_entry` not stamped under sticky `--chat` |
| `dispatch_cycle_mode_pre_session_cycles_locally` | CycleMode emitted `CreateSession` |
| `worktree_session_created_clears_sticky_branch_from_main_repo` | `current_branch` survived worktree cwd switch |

Also red outside pure lifecycle (same session module sample): fork (5) and load hydration/title/worktree (12). Those are separate residual slices.

---

## Roots fixed (product)

Restored monorepo contracts (`b13fa526` tip paths) into Surmount half-merge holes. **Tests encode product contracts** — no mass expect rewrites.

### 1. Dashboard attach follows New/Fork (`switch_to_agent`)

**Root:** New/worktree `/new` switched `ActiveView` but never re-pointed `DashboardState.attached_agent`, so overlay back-out (Left/Esc) stayed on the prior agent.

**Fix:**

- `views/dashboard/state.rs`: `repoint_attach_if_on(previous, new_id)` (only when attach already names `previous`; also `focus_row`).
- `dispatch/ctx.rs` `switch_to_agent`: for `SwitchCause::New | Fork`, capture **top-level** `ActiveView::Agent(id)` (never subagent placeholder from `get_active_agent`), then `repoint_attach_if_on`.

Covers plain `/new`, worktree `/new`, and subagent-view open (parent top-level id).

### 2. Orphan create failure restores or clears attach

**Root:** `handle_session_failed` / `handle_worktree_session_failed` removed the placeholder but left dashboard attach on the dead id (or never cleared on Welcome).

**Fix:** `restore_dashboard_attach_after_orphan_remove` — survivor → `repoint_attach_if_on`; no survivor → `close_popup()`.

### 3. Deferred model switch preserves `prev_model_id`

**Root:**

- Pre-session `Action::SwitchModel` only stashed with `prev_model_id: None` and returned no effects (no optimistic current + no `PersistPreferredModel`).
- `SessionCreated` / worktree / load emitted `Effect::SwitchModel` with `prev_model_id: None` even when stash or authoritative catalog had a prev.
- Half-merge: `deferred_model_switch_from_cli` returned a tuple while `AgentSession.deferred_model_switch` is `Option<DeferredModelSwitch>`.

**Fix:**

- `app_view.rs`: `deferred_model_switch_from_cli` → `Option<DeferredModelSwitch>` with `prev_model_id: None`.
- `router.rs`: pre-session path — optimistic `set_current`, stash with rollback prev (prior stash prev or display current), `PersistPreferredModel` when changed.
- `lifecycle.rs`: `DeferredSwitchOutcome.switch` is `Option<DeferredModelSwitch>`; `apply_deferred_model_switch` fills `prev_model_id` from authoritative `models.current` when different from switch target; create handlers thread `switch.prev_model_id`.
- `load.rs`: same SwitchModel prev threading.
- `take_deferred` tests: `switch()` helper returns `DeferredModelSwitch`.

### 4. Sticky `--chat` stamps `conversation_entry`

**Root:** `chat_kind` was set on CreateSession / agent but `conversation_entry` (rename kind) was missing on `AgentView` (half-merge hole) and not stamped on create.

**Fix:**

- `agent_view/mod.rs`: restore `pub conversation_entry: bool` (+ docs from monorepo tip).
- `agent_view/session.rs`: default `conversation_entry: false` in `AgentView::new`.
- `lifecycle.rs`: `agent.conversation_entry = chat_kind` on plain new-session create and worktree new-session create paths.

### 5. Pre-session CycleMode must not CreateSession

**Root:** Surmount half-merge called `skip_picker_and_create_session` at end of pre-session cycle arms.

**Fix:** Pre-session cycle only persists permission mode (and defers Plan via `deferred_session_mode`). `skip_picker_and_create_session` remains for real send/project-picker paths in `prompt.rs`.

### 6. Worktree create clears sticky main-repo git chrome

**Root:** `handle_worktree_session_created` set `session.is_worktree` but left agent `current_branch` / `main_repo` / `is_worktree` from the main repo.

**Fix:** clear `current_branch` + `main_repo`, set `agent.is_worktree = true` with session flag.

---

## Files changed (intentional product)

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/views/dashboard/state.rs` | `repoint_attach_if_on` |
| `crates/codegen/xai-grok-pager/src/app/dispatch/ctx.rs` | New/Fork attach follow |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/lifecycle.rs` | deferred switch type, chat stamp, attach restore, sticky branch, FetchBilling format |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | deferred SwitchModel prev |
| `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` | pre-session SwitchModel persist + stash prev |
| `crates/codegen/xai-grok-pager/src/app/dispatch/modes.rs` | drop CreateSession from pre-session cycle |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/session/take_deferred.rs` | outcome type helper |
| `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs` | restore `conversation_entry` field |
| `crates/codegen/xai-grok-pager/src/app/agent_view/session.rs` | default `conversation_entry: false` |
| `crates/codegen/xai-grok-pager/src/app/app_view.rs` | `deferred_model_switch_from_cli` → `DeferredModelSwitch` |

No FORK dual-pin (product intent matches monorepo tests; no fork-doc lie).

---

## Verify

### What ran green once

At first sample (dirty half-merge tree that compiled tests):

```text
cargo test -p xai-grok-pager --lib 'app::dispatch::tests::session::'
→ 249 passed; 28 failed
```

Lifecycle alone: **77 pass / 11 fail**.

### What blocked re-verify

1. Pager lib does **not** compile (~321 errors after this slice’s type seams). Failures span missing `project_picker` module, duplicate MCP types, unresolved rewind/queue/telemetry imports, slash trait methods, rewind exhaustiveness, extensions modal, etc. **Not** lifecycle product paths (those no longer appear in `cargo check` diagnostics).
2. Pure onto tip alone also fails lib compile (~290 errors). Concurrent WIP half-merge is required for a green tree; pure tip is not self-sufficient.
3. Product session-lifecycle modules were re-applied from `/tmp/session-lifecycle-keep/` after mid-turn recovery; type seams for `conversation_entry` + CLI deferred helper restored from monorepo tip text.

**Do not claim lifecycle 11 green without a re-run on a compiling tree.**

### When compile is restored

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::session::lifecycle::' -- --test-threads=8
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::session::' -- --test-threads=8
```

Expected: lifecycle 11 filters green; session module remaining red concentrated in **fork** + **load** (hydration / worktree / title), not attach/deferred/chat/cycle.

---

## Not in this slice

- Fork suppress retarget / sticky chat rename / worktree fork branch (5 fails).
- Load: `HydrateSessionMetaFromDisk`, title sanitize, standalone worktree mark, sticky chat restore (12 fails).
- DeleteSessionComplete `after` (already reported done; may need re-land if tip tree lacks `AfterSessionDelete` on TaskResult).
- Project-picker fixture (already fixed; do not regress).
- Broader pager half-merge compile mop (parent/other residual).
- FORK / user-guide dual-pin (not required).

---

## 10-line summary

1. Session module sample: **28** red (lifecycle **11**, fork 5, load 12).
2. Roots: attach not following New/Fork; orphan fail no attach restore; deferred switch lost prev + no pre-session persist; chat_kind without `conversation_entry`; pre-session cycle CreateSession; sticky branch on worktree create.
3. Product restore from monorepo: `repoint_attach_if_on` + switch_to_agent, orphan restore, DeferredModelSwitch prev, conversation_entry field + stamp, cycle no create, worktree branch clear, CLI deferred helper type.
4. Files: lifecycle, ctx, modes, router SwitchModel, load, dashboard state, agent_view, app_view, take_deferred helper.
5. No mass test rewrites; no FORK pin.
6. Lifecycle-specific compile diagnostics cleared; full lib still ~321 errors (half-merge outside this slice).
7. Re-run lifecycle filters when compile is green; remaining session red expected in fork + load only.
8. No git commit/add/push.
