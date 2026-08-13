# Pager residual resample after mop wave (2026-08-11)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 explore (read-only). **No shell tool** in this worker, so the requested live

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib 'app::dispatch::tests' -- --test-threads=8
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8
```

**did not run here.** Numbers below are a **claim synthesis + code residual probe** after today's mop reports, not a fresh cargo exit code. Parent (or any shell-capable mop) must re-run those commands to hard-confirm.

**Prior:**
- CI baseline: 148 pager lib fails (prompt `436` / `bug-ci-239-test-cluster-2026-08-11.md`)
- After project-picker root: ~8689 pass / **120 fail** (`bug-pager-mass-fail-root-2026-08-11.md`)
- Mid-wave inventory: dispatch **1339 pass / 45 fail** named list (`bug-pager-residual-inventory-2026-08-11.md`)
- Mop claims closed those 45 (session, settings, prompt, billing, router, lifecycle, key_owner, share)

---

## 1. Claimed green (module filters; implementer reports)

| Module / filter | Claimed result | Report |
|-----------------|---------------:|--------|
| `app::dispatch::tests::session::` (fork+load+lifecycle) | **277 / 0** | `bug-pager-session-fork-load-2026-08-11.md` |
| `…::session::lifecycle::` alone | **88 / 0** | `bug-pager-lifecycle-dashboard-stop-2026-08-11.md` |
| `…::prompt` | **127 / 0** | `bug-pager-prompt-residual-2026-08-11.md` |
| `…::billing` | **79 / 0** | `bug-pager-billing-residual-2026-08-11.md` |
| `…::router` | **103 / 0** | `bug-pager-router-residual-2026-08-11.md` |
| `…::settings` | **129 / 0** | `bug-pager-status-turn-settings-2026-08-11.md` |
| `…::status` | **58 / 0** | same |
| `…::turn` | **85 / 0** | same |
| `app::agent_view::key_owner::tests` | **30 / 0** | `bug-pager-key-owner-hints-2026-08-11.md` |
| related `question_answer_focus_tests` | **8 / 0** | same |
| `slash::registry` + share kill-switch | **28 / 0** + named ok | `bug-pager-share-menu-hidden-2026-08-11.md` |
| project-picker fixture / bound session | closed | `bug-pager-mass-fail-root-2026-08-11.md` |

**If every claim still holds on one tree,** the inventory **45 dispatch fails are zero**. Full `app::dispatch::tests` should be on the order of **~1380+ pass / 0 fail** (session 277 + prompt 127 + billing 79 + router 103 + settings 129 + status 58 + turn 85 + queue/voice/dashboard/task_result siblings).

**Caveat:** those verifies were **filter-scoped**, often concurrent. A single full `--lib` re-run is the only honest gate against thrash/regressions.

---

## 2. Remaining residual clusters (not closed by mop claims)

Cross-check: original CI pager fail list (`prompt_436`) minus modules with green mop reports, plus prompt mop **out-of-scope** notes + product/test code probe.

### Cluster A — queue parked look vs held occupancy (**3**, high confidence still red)

| Test | Module |
|------|--------|
| `app::dispatch::queue::tests::parked_wait_holds_queue_and_explains_itself` | `dispatch/queue.rs` |
| `app::dispatch::queue::tests::parked_wait_clears_progress_bar_notification` | (empty-queue park path; may be green if no held row) |
| `app::dispatch::queue::tests::local_delete_of_last_held_row_flips_parked_look_on` | same |

**Probe:**

- Tests with a held row assert `!agent.renders_parked()` then flip to parked after last delete.
- Product `AgentView::renders_parked` is occupancy-independent:

```432:434:crates/codegen/xai-grok-pager/src/app/agent_view/queue.rs
    pub(crate) fn renders_parked(&self) -> bool {
        self.is_parked_on_sendable_wait() && !self.is_waiting_on_subagent()
    }
```

- Marker push **does** withhold on held queue (`maybe_push_parked_marker` gates on `has_held_user_queue()`), but **parked chrome** does not.
- Prompt mop explicitly left this as not owned (`bug-pager-prompt-residual-2026-08-11.md` § Out of scope).

**Fix direction (choose one contract, dual-pin if Surmount differs from monorepo):**

1. **Monorepo product:** park look independent of queue; **rewrite** the three expects to `renders_parked() == true` with held rows (and adjust delete-flip test to only check marker push). Or
2. **Live-test contract:** gate `renders_parked` (and notifications) on `!has_held_user_queue()` so held rows suppress stopped chrome.

Do not half-merge both.

### Cluster B — slash mode_support pin / dashboard visibility (**2**, high confidence still red)

| Test | Issue |
|------|--------|
| `slash::mode_support::tests::mode_specific_builtin_refusals_are_pinned` | Pin list expects `jump` + `timeline` refusals via `mode_support().refusal` |
| `slash::commands::dashboard::tests::visible_everywhere_except_minimal` | Mode/visibility surface for dashboard |

**Probe:** `JumpCommand` / `TimelineCommand` only override `available_in_minimal() -> false` and leave default `mode_support() -> Both`. The pin filter uses **`mode_support()` only**, so jump/timeline never contribute refusals while the expected vec still lists them.

**Fix:** give jump/timeline (and any other denylisted builtins) proper `ModeSupport::FullscreenOnly(Remedy::…)` with the pinned `why` strings; align `available_in_minimal` with that or drop the legacy flag path. Dashboard command already uses `FullscreenOnly`.

### Cluster C — scrollback structural layout (**5**, CI still listed; no mop)

```
scrollback::state::layout::tests::growing_entry_above_manual_viewport_keeps_marker_at_screen_row_zero
scrollback::state::layout::tests::removal_above_wrapped_park_keeps_row_inside_wrapping_entry
scrollback::state::layout::tests::removing_entry_above_manual_viewport_keeps_marker_at_screen_row_zero
scrollback::state::layout::tests::resize_defers_warm_above_across_a_frames_extra_layout_passes
scrollback::state::layout::tests::resize_defers_warm_above_until_the_width_settles
```

Half-merge structural anchor / warm-measure defer. Paths: `scrollback/state/layout.rs` only. Disjoint from dispatch mops.

### Cluster D — acp_handler / oneshots (**~4–6**, no full-module green claim)

From CI list; share kill-switch one was fixed in isolation:

| Test | Notes |
|------|--------|
| `acp_handler::…::viewer_prompt_complete_error_pushes_turn_failed_marker` | queue_and_adoption |
| `acp_handler::…::loop_fire_mode_follows_session_not_later_settings_push` | settings acp |
| `acp_handler::…::settings_update_sharing_enabled_true_stays_forced_off` | **claimed ok** by share mop |
| `acp_handler::…::live_update_closes_late_replay_grace` | subagents |
| `acp_handler::…::failed_wake_turn_keeps_markerless_shape` | turn_completion |
| `agent_view::links::…::privacy_banner_owns_slot_and_clicks_dispatch` | links |
| `agent_view::plan::…::plan_preview_copy_button_click_copies_whole_plan_body` | plan copy |
| `dispatch::tests::task_result::session_success_arms_finish_startup_obligation` | task_result |
| `dispatch::tests::voice::voice_submit_*` (**2**) | voice |
| `dispatch::tests::dashboard::dashboard_*` (**3**) | may already be green after lifecycle/picker; **unverified** |

### Cluster E — sibling crates (not `xai-grok-pager` lib, still CI)

| Package | Tests |
|---------|--------|
| `xai-grok-pager-minimal` | `committed_thinking_paints_a_dim_rail_in_column_zero` |
| `xai-grok-pager-render` | auto dark → DOGE (2) |

Out of this crate's `--lib` filter but still on the 239 board.

---

## 3. Estimated pass/fail totals (inference only)

| Scope | Est. pass | Est. fail | Confidence |
|-------|----------:|----------:|------------|
| **Inventory 45 dispatch names** | all | **0** | Medium (per-module claims; no unified re-run) |
| **Full `app::dispatch::tests`** | ~all module totals | **~0–8** if voice/dashboard/task_result still red | Low–medium |
| **Full `xai-grok-pager --lib`** | ~8800+ | **~15–25** | Low–medium |
| Est. fail mix if claims hold | | layout **5** + parked queue **2–3** + mode pin **1–2** + acp/oneshots **~5–10** + maybe voice/dashboard | |

**Do not treat this as a cargo summary.** Next mop with shell must replace this section with live `test result:` lines and a named fail list.

---

## 4. Suggested next mop (priority)

Parallel-safe; product restore first unless Surmount intentional contract is dual-pinned.

| Order | Scope | Est. | Paths | Notes |
|------:|-------|-----:|-------|--------|
| **0** | **Live full resample (shell)** | — | `cargo test -p xai-grok-pager --lib` + `app::dispatch::tests` | Replace this report's estimates; `tee` to `/tmp/pager-*-resample.txt` |
| **1** | **Parked-look contract** | **2–3** | `agent_view/queue.rs` `renders_parked` **or** `dispatch/queue.rs` tests | Decide monorepo vs held-suppress; one writer |
| **2** | **mode_support pin + jump/timeline** | **1–2** | `slash/commands/{jump,timeline}.rs`, maybe dashboard visibility | Wire `ModeSupport::FullscreenOnly` with pinned copy |
| **3** | **scrollback layout structural** | **5** | `scrollback/state/layout.rs` only | Disjoint; monorepo anchor/warm path |
| **4** | **acp_handler oneshots** | **~4** | `acp_handler` tests + handlers | One filter each: failed_wake, subagent grace, loop_fire, viewer error |
| **5** | **plan copy / privacy link / voice / task_result / dashboard** | small | per-test | After 0 names remaining |
| **6** | **pager-minimal + pager-render** | **3** | sibling crates | Dim rail + DOGE auto theme |

**Already skip for pager unit residual (claimed):** lifecycle, session fork/load, settings, status, turn, prompt, billing ShowUsage chain, router deferred/hooks, key_owner bar, share registry, project-picker fixture.

---

## 5. Parent verify commands (must run with shell)

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib 'app::dispatch::tests' \
  -- --test-threads=8 2>&1 | tee /tmp/pager-dispatch-resample.txt | tail -80

nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  -- --test-threads=8 2>&1 | tee /tmp/pager-lib-resample.txt | tail -80

# cluster extract
rg 'FAILED$|^failures:|test result:' /tmp/pager-lib-resample.txt
rg 'app::|scrollback::|slash::' /tmp/pager-lib-resample.txt | rg 'FAILED|panicked' || true
```

Optional tight reds if full suite is slow:

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'app::dispatch::queue::tests::parked_wait' -- --test-threads=8
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'mode_specific_builtin_refusals_are_pinned' -- --test-threads=8
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib \
  'scrollback::state::layout::tests' -- --test-threads=8
```

---

## 6. 10-line summary

1. **No live cargo** in this explore worker (no shell); report is mop-claim + code residual, not a re-sample exit code.
2. Inventory **45 dispatch fails** are **all claimed green** by today's mops (session 277/0, prompt 127/0, settings 129/0, billing 79/0, router 103/0, lifecycle 88/0, status/turn green).
3. Full `app::dispatch::tests` is **likely green or near-green** if claims hold on one tree.
4. Full lib residual **likely ~15–25**, not 120/45, concentrated **outside** the mopped dispatch clusters.
5. Strong remaining: **parked_wait / held-queue park look (2–3)**, **mode_support pin jump/timeline (1–2)**, **layout structural (5)**.
6. Medium remaining: **acp_handler oneshots**, plan copy, privacy link, voice, task_result; dashboard dispatch unconfirmed.
7. Sibling: pager-minimal dim rail, pager-render DOGE auto (not this crate's `--lib`).
8. Next: shell mop full `--lib` → fix parked contract → mode_support → layout → acp oneshots.
9. Do not mass-weaken expects; pick park-look contract deliberately.
10. No product edits from this agent.

---

## 7. Blocker for exact numbers

Spawn a **shell-capable** process mop (or general-purpose) whose only job is the two `cargo test` commands above and a short append to this file (or a new report) with:

- `test result: … passed; … failed`
- full `failures:` name list
- one panic sample per remaining cluster
