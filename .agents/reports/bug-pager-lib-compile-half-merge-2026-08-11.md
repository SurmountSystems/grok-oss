# bug: pager lib compile half-merge (2026-08-11)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Goal:** restore `cargo check -p xai-grok-pager --lib` (and ideally `--tests`) to GREEN after half-merge thrash.

---

## Final status

| Check | Result |
|-------|--------|
| `cargo check -p xai-grok-pager --lib` | **GREEN** (exit 0, warnings only) |
| `cargo check -p xai-grok-pager --lib --tests` | **GREEN** (exit 0, all targets including integration harnesses) |
| Lifecycle smoke `cargo test -p xai-grok-pager --lib 'app::dispatch::tests::session::lifecycle::' -- --test-threads=8` | **86 passed, 2 failed** (compile ok; residual product arms) |
| `cargo fmt -p xai-grok-pager` | exit 0 |

Primary goal (**lib compiles**) met. Secondary (**lib tests compile**) met. Lifecycle runtime is almost green; two dashboard-stop cases remain product residual.

---

## Baseline (this session / prior thrash)

| Stage | Errors (approx) | Notes |
|-------|-----------------|-------|
| Prior status/turn thrash | ~321 lib errors | half-merge: dups, missing mods, rewind exhaustiveness, slash trait, telemetry, project_picker, MCP types |
| Start of this mop | lib **already green** after earlier queue.rs / type-seam work | `cargo check --lib` exit 0 |
| Mid mop (lib `--tests`) | ~8 remaining error sites | DeferredModelSwitch tuples, AppView fields, git_info arity, privacy e2e export, missing reparked mod |
| After this mop | **0** lib / **0** `--lib --tests` compile errors | |

### Top clusters fixed this continuation

| Class | Symptom | Fix |
|-------|---------|-----|
| **DeferredModelSwitch** | `Option<(ModelId, Option<Effort>)>` vs struct | Assert / assign `DeferredModelSwitch { model_id, effort, prev_model_id }` in router + dashboard tests |
| **AppView test seed** | missing `project_picker_*`, `privacy_banner_accept_inflight` | Fields on `dispatch/tests/mod.rs::test_app`; `project_picker_shown: true` so unit tests skip Tokio picker |
| **AgentSession seed** | half-closed struct (`},` before `session_notes`) | Close struct correctly with `session_notes` |
| **git_info arity** | `update_from_notification` 4th `is_worktree` | `top_bar` unit test passes `false` |
| **privacy e2e** | `parse_privacy_arg` removed (product: open settings only) | E2E documents export removal; unit tests own contract |
| **pty reparked mod** | missing `reparked_wait_repushes_buried_marker.rs` | Rewire to existing `reparked_wait_stays_markerless` (markerless park product) |
| **Project picker in unit tests** | CreateSession empty / no Tokio reactor | Fixture marks picker already shown |

Earlier same-day mop (see `impl-upstream-pager-lib-compile-2026-08-11.md` and session-lifecycle report): queue.rs FetchBilling delimiter corruption, suppress_code_restore / begin_frame / late_replay, RewindMode / Btw Done, marketplace dups, privacy rect names, TaskSnapshot / CreditBalance fields, dashboard delete restore, etc.

---

## Files touched this continuation (compile mop)

- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/router.rs` — DeferredModelSwitch asserts
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/dashboard.rs` — DeferredModelSwitch asserts
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/mod.rs` — AppView fields, AgentSession seed, picker fixture
- `crates/codegen/xai-grok-pager/src/views/welcome/top_bar.rs` — git_info 4th arg
- `crates/codegen/xai-grok-pager/tests/pty_e2e_persistence.rs` — reparked mod rewire
- `crates/codegen/xai-grok-pager/tests/settings_e2e.rs` — privacy export residual test

Product session-lifecycle / key_owner / delete / share / project_picker paths were **not** undone; only type seams and fixtures.

---

## Lifecycle smoke detail

```text
test result: FAILED. 86 passed; 2 failed; 0 ignored; 0 measured; 8732 filtered out
```

### Still failing (product residual, not compile)

1. `dashboard_stop_double_press_via_handle_key_deletes_top_level` — first Ctrl+X must arm `delete_confirm`
2. `dashboard_stop_with_peek_open_moves_selection_and_peek_down_one` — missing map key after stop/peek path

Likely: half-merge incomplete wire for dashboard stop / delete_confirm / key_owner. Related prior reports: `bug-pager-key-owner-hints`, `bug-pager-delete-session-complete`, `bug-pager-session-lifecycle`.

### Fixed by fixture (were failing, now pass)

CreateSession / slash new / chat_mode / mcp_init_progress / send_prompt queue / session_created drain / agent_type_mismatch — unblocked once unit fixtures skip project picker Tokio path.

---

## Residual for follow-on agents

| Area | Residual |
|------|----------|
| **status / turn / settings** | Runtime catalog filters and status/turn thrash paths not re-verified beyond compile; clippy mop optional |
| **session fork / load** | Compile green; fork/load product paths need targeted filter smoke after dashboard-stop |
| **layout** | Structural scroll-anchor methods still dead_code warnings; layout half-merge not this mop |
| **oneshots / e2e** | PTY families compile; behavior not re-run (ignored under normal cargo). `settings_e2e` privacy unit ownership only |
| **dashboard stop** | Two lifecycle failures above |
| **warnings** | ~42 lib dead_code / unused (session_picker PendingDelete helpers, privacy height, etc.) — mop later, not blocking compile |
| **clippy** | Not run this pass (time-boxed); fmt only |

---

## Commands

```bash
nice -n 19 ionice -c3 cargo check -p xai-grok-pager --lib
# exit 0

nice -n 19 ionice -c3 cargo check -p xai-grok-pager --lib --tests
# exit 0

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::tests::session::lifecycle::' -- --test-threads=8
# 86 passed; 2 failed (dashboard_stop_*)

cargo fmt -p xai-grok-pager
# exit 0
```

---

## Related reports

- `.agents/reports/impl-upstream-pager-lib-compile-2026-08-11.md`
- `.agents/reports/impl-upstream-pager-tests-compile-2026-08-11.md`
- `.agents/reports/bug-pager-session-lifecycle-2026-08-11.md`
- `.agents/reports/bug-fmt-missing-reparked-mod-2026-08-11.md`
- `.agents/reports/bug-parse-privacy-arg-e2e-2026-08-11.md` (if present; privacy export removal)
