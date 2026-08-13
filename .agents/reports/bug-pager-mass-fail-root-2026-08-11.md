# Pager mass-fail root (xai-grok-pager)

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Prior cluster:** `.agents/reports/bug-ci-239-test-cluster-2026-08-11.md` (pager 148 of 239)
**Agent:** L2 explore+fix implementer

---

## Executive status

| Item | Value |
|------|--------|
| **CI baseline (pager lib)** | **148** fails (instant asserts) |
| **Local after largest-root fix** | **120** fails / **8689** pass (`xai-grok-pager` lib unit) |
| **Largest root fixed** | Project-picker re-entry on every plain `SendPrompt` in tests (and product footgun for already-bound sessions) |
| **Verdict** | **Not one root for all 148.** Multiple onto half-merge roots. Largest single systemic slice fixed; residual clusters below. |

---

## Sample panics (before fix)

| Filter | First panic |
|--------|-------------|
| `key_owner::…::a_parked_card_contributes_one_route_back` | `hint_labels` missing `"next answer"` |
| `dispatch::…::send_prompt_produces_effect_and_clears_input` | `effects.len()` **3** vs 1 |
| `lifecycle::…::delete_current_session_complete_returns_to_dashboard` | `active_view` not `AgentDashboard` |
| `acp_handler::…::settings_update_sharing_enabled_true_stays_forced_off` | typed `/share` does not `get_for_dispatch` |
| billing suite | **79/79 green** (not part of mass pager fail) |

### DIAG: the three send effects

```text
SetWorkingDir { path: "/tmp" }
CreateSession { agent_id: 0, cwd: "/tmp", … }
SendPrompt { session_id: "test-session", text: "hello", … }
```

Root path:

1. `test_app()` set `project_picker_shown: false` (cwd `/tmp` is not a project dir).
2. Plain `SendPrompt("hello")` hit `needs_project_picker()` → `open_project_question`.
3. No Tokio runtime / empty recent dirs → `resolved_paths.len() <= 1` auto-selects cwd.
4. `dispatch_project_selected` always emits **`SetWorkingDir` + `CreateSession`** then re-dispatches send.

Historical monorepo test fixture used **`project_picker_shown: true`** so pre-bound agents never re-entered the picker. After monorepo removed the picker fields then Surmount re-added them in the onto half-merge, the fixture default flipped to **false** and mass-broke plain send/drain tests.

---

## Fix landed (largest single root)

### Files changed

| Path | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/mod.rs` | `test_app` default `project_picker_shown: **true**` (monorepo historical test default; comment why) |
| `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs` | Product guard: skip project-picker intercept when active agent **already has `session_id`** (bound session must never get a second `CreateSession` from picker auto-select) |

### Sample green (after)

| Suite / filter | Result |
|----------------|--------|
| `send_prompt_produces_effect_and_clears_input` | **ok** |
| `app::dispatch::tests::prompt` | **113 pass / 14 fail** (was 101/26) |
| `app::dispatch::tests::router` | **103 pass / 0 fail** |
| `app::dispatch::tests::billing` | **79 pass / 0 fail** |
| Full `xai-grok-pager` lib unit | **8689 pass / 120 fail** (was ~148 CI fails) |

Rough win: on the order of **~25–30** pager unit fails cleared by this one root (CI 148 → local 120; prompt alone −12).

No git commit/add/push.

---

## Residual clusters (not fixed this turn)

Multiple independent onto half-merge roots remain. Do **not** mass-rewrite tests; restore monorepo product paths or fix intentional Surmount contracts with dual-pin.

### 1. Key-owner shortcut bar (~12 fails)

**Symptom:** bar missing `"next answer"` / `"next choice"` / `"next option"`; parked card does not pin route-back `"question"` / `"permission"`.

**Root:** monorepo `current_shortcut_hints` used `key_owner()` + `question_shortcut_hints` / `permission_shortcut_hints` / `card_esc_hint` / `ShortcutsBarContent` + `build_hints(..., focus_hint)`. Surmount/onto left a simplified non-`key_owner` path in `render.rs` that hardcodes Tab as `"scrollback"` and never emits walk labels.

**Evidence:** `"next answer"` exists only in tests; product string was removed in cherry-pick `01327f98` (impl #7). Monorepo tip `a5589e95` still has the full path.

**Fix direction:** restore monorepo `shortcuts_bar_content` / card hint helpers into `render.rs`, keep Surmount plan-approval P1/Q2 empty-freeform rules; pass `parked_card().map_or_else(prompt_focus_hint, BlockingCard::focus_hint)` into `build_hints` (may need monorepo `focus_hint` arg on `agent::build_hints` if still missing).

**Paths:**
`app/agent_view/render.rs`, `views/agent.rs` (`build_hints` / `prompt_focus_hint`), `app/agent_view/key_owner.rs` (already present).

### 2. DeleteSessionComplete stub (~lifecycle delete + dashboard stop)

**Symptom:** `delete_current_session_complete_returns_to_dashboard` → still `ActiveView::Agent`, agents not removed.

**Root:** current handler is toast-only and **ignores `after`**:

```rust
TaskResult::DeleteSessionComplete { source, session_id, after: _ } => {
    remove_session_from_pickers(app, &source, &session_id);
    app.show_toast("Session deleted");
    vec![]
}
```

Monorepo `a5589e95` has full `AfterSessionDelete::{Stay, Dashboard, Welcome}` handling (remove agents, neighbor focus, `AgentDashboard`, `UnregisterActiveSession`, …).

**Note:** monorepo `remove_session_from_pickers` took a 4th `clear` bool; current API is 3-arg. Port carefully against current `remove_agent_and_cleanup` / `dashboard_neighbor_row` / `dispatch_exit_session`.

**Path:** `app/dispatch/task_result.rs` (~1054).

### 3. Share kill-switch / menu_hidden (~settings)

**Symptom:** `settings_update_sharing_enabled_true_stays_forced_off` — after kill-switch, `get_for_dispatch("share")` is `None`.

**Root:** monorepo contract: `/share` is **menu_hidden** (still dispatchable) until sharing enabled. Current `set_share_visible` uses hard **`hidden`** (blocks dispatch too). Registry `new()` no longer seeds `menu_hidden` with `"share"`.

**Fix direction:** restore monorepo `set_share_visible` (always `hidden.remove("share")`; toggle `menu_hidden`) + default `menu_hidden.insert("share")` in `CommandRegistry::new`.

**Path:** `slash/registry.rs` `set_share_visible` / `new`.

### 4. Session lifecycle / new-session / deferred switch (~33 in `dispatch::tests::session`)

Separate from delete stub: CreateSession model_id, chat_kind, dashboard attach repoint, deferred switch stash, pre-session cycle mode, cwd fallback, MCP init seed, etc. Likely several half-merge holes in `dispatch/session/lifecycle.rs` + dashboard attach helpers.

### 5. Status / privacy / turn / settings dispatch (~20+20+9+8)

`dispatch::tests::status`, `turn`, `settings` — still red. Not shared with project-picker. Diagnose per panic after clusters 1–3.

### 6. Scrollback layout (~5)

`scrollback::state::layout::tests` — structural scroll anchor / layout half-merge (dead methods already warn).

### 7. Small oneshots

Plan approve flush, links, interactions answer focus, picker cursor, mode refusals, command palette cursor, a few acp_handler turn/subagent/queue.

---

## What this is **not**

- Not DOGE theme / privacy banner as the global pager fail.
- Not signed-policy dark build (that is config/shell `team_managed`, separate CI cluster).
- Not one AppView default beyond project-picker (billing was already green).
- Not fixed by compile-only mops.

---

## Recommended follow-up order

| Order | Scope | Est. impact |
|------:|-------|-------------|
| **Done** | Project-picker fixture + bound-session guard | ~25–30 fails |
| **1** | Restore key_owner-aligned shortcut bar | ~12 + related interactions/pty |
| **2** | Restore `DeleteSessionComplete` monorepo handler | lifecycle delete/dashboard stop slice |
| **3** | `set_share_visible` → menu_hidden contract | share kill-switch + registry tests |
| **4** | Session lifecycle CreateSession / attach / deferred switch | remaining session module |
| **5** | status / turn / settings / layout / oneshots | rest of ~120 |

---

## 10-line summary

1. Re-ran diverse fails; **not one panic string** for all 148.
2. **Largest shared root:** project picker re-entered on every plain send in tests (`project_picker_shown: false` + auto CreateSession).
3. DIAG: SendPrompt effects were `SetWorkingDir` + `CreateSession` + `SendPrompt`.
4. Fixed fixture default to monorepo `shown: true` + product skip when session already bound.
5. Local pager lib: **120 fails** remaining (was ~148 CI).
6. Residual: key_owner bar, DeleteSessionComplete stub, share menu_hidden, session lifecycle, status/turn/settings, layout.
7. Tests are still the spec; do not mass-reshape expects.
8. No git commit/add/push.
