# Plan 0 — L1 Subagents view is L2-only; L3s never leak into L1

> L3 specialists currently show up in L1's subagent view. Wrong. L1 sees only its L2s. Each L2 sees only its own L3s and their context. L3s never appear in, or leak context into, L1.

**North star:** the Operator at L1 sees only L2 coordinator rows, plus a live L3 count on each L2 row (`1 specialist` / `N specialists`), never specialist names as their own L1 rows. Opening an L2 overlay shows only that L2's L3s and those L3s' transcripts. L2-A cannot see L2-B's L3s. L3 content never lands in the L1 transcript, the L1 context chip, or L1 watching counts. Product must match GitHub #224 / host `~/.grok/AGENTS.md`: Keep L1 list L2-only plus a live L3 count. Do not flatten L2/L3 into one list.

**Baseline (what exists today):**
- GitHub #224 deep home is `FORK.md` § *Hierarchical subagents* (`FORK.md:1998`). The L1-list law is checked as landed: "the L1 Subagents list, watching counts, and similar live chrome show only L2 coordinators. Each L2 row may append a live L3 count (`1 specialist` / `N specialists`). L3 specialists do not get their own L1 rows or names." (`FORK.md:1506-1509`). User-guide: "The L1 Subagents list, watching counts, and similar live chrome show only L2 coordinators." (`crates/codegen/xai-grok-pager/docs/user-guide/16-subagents.md:448`). Host process: "Keep L1 list L2-only plus a live L3 count. Do not flatten L2/L3 into one list." (`~/.grok/AGENTS.md:1573`). In-tree `docs/github-tracking.md` is the GitHub process pin, not the issue-224 body.
- Tree model is `SubagentAttemptInfo.parent_session_id` plus `depth` (`crates/codegen/xai-grok-pager/src/app/subagent.rs:57-60`). `1` is an L2. `2` is an L3. There is no separate `SubagentTree` type (that path is MISSING). `is_l2_list_row` (`subagent.rs:1356-1368`) returns false when `depth >= 2`, or when `parent_session_id` is another listed child. `live_l3_count` (`subagent.rs:1395-1406`) counts running non-workflow rows whose parent session is that L2.
- L1 dock already claims L2-only: "L3 specialists stay in the registry and are not their own rows." (`panes.rs:342-347`). `dock_subagent_rows` filters `is_l2_list_row` (`panes.rs:348-369`) and appends `format_live_l3_count` (`panes.rs:404-412`).
- Tasks pane `sync` also filters `is_l2_list_row` (`tasks_pane.rs:1017-1025`). Then a fallback: if that filter is empty, it lists every running parented row (`tasks_pane.rs:1026-1038`). That fallback is how an L2 overlay names specialists. It is not scoped to "this L2's session id." On L1, if no row classifies as L2, it lists L3s.
- Live nested specialists are registered on the L1 registry, not on the L2 child view (`render.rs:7937-7939`). Overlay draw mirrors parented L3s onto the L2 view for one paint, then drops them (`subagent_takeover.rs:729-762`). Mirror keys on `parent_session_id == this L2`. Overlay wait count uses the same parent filter (`subagent_takeover.rs:937-944`).
- **Leak 1, live spawn depth.** The L1-addressed `SubagentSpawned` path always writes `depth: None` (`session_notification.rs:813-814`). A later nested path writes depth 2 when the notification is addressed to the L2 (`session_notification.rs:2132-2143`). The crate already names the miss: "That path leaves `SubagentAttemptInfo.depth` as `None`." (`session_notification.rs:2922-2924`). `is_l2_list_row` only rejects `depth >= 2`. `None` plus a parent that is not in the child-id set classifies an L3 as an L2, so L1 lists it.
- **Leak 2, L1 scrollback.** After inserting the child view, L3 skip of the L1 `Subagent` block is `parent_session_id` in `subagent_sessions` (`session_notification.rs:978-984`). If the wire parent is L1, that check is false and L1 scrollback gets an L3 lifecycle block (`session_notification.rs:986-997`). That is L3 context in the L1 transcript.
- **Leak 3, watching counts.** `watchers().subagents` counts every running non-workflow row (`queue.rs:304-308`). It does not call `is_l2_list_row`. FORK watching-counts law (`FORK.md:1506-1508`) is not this function.
- **Leak 4, `/tasks` block.** `tasks_block_text` walks every `subagent_sessions` value (`status_blocks.rs:155-168`). `is_l2_list_row` is imported and unused (`status_blocks.rs:12`). L3 names print in the L1 `/tasks` body.
- **Leak 5, flattened views.** L3 `AgentView`s are inserted into L1 `subagent_views` before the L3 scrollback skip (`session_notification.rs:976`). The registry is one flat map. L1 list helpers must keep filtering. `running_live_specialists` / `running_foreground_subagents` do not (`session.rs:1985-2004`).
- **Leak 6, L2 overlay scope.** Tasks-pane empty-L2 fallback lists every parented running row, not only rows whose parent is this overlay's L2 (`tasks_pane.rs:1026-1038`). If an L2 child registry is over-copied, L2-A sees L2-B's L3s. `dock_subagent_rows` has no L2-scoped L3 list at all (`panes.rs:348-369`).
- Context chip law already exists: L1 `239K / 500K` must not add nested windows (`context_bar.rs:598-621`). Nested stamp must not copy the L1 sampling window onto the overlay (`session_notification.rs:101-104`, `subagent_takeover.rs:220-223`). Opening an L3 must show that specialist's transcript, not the L1 window (`task_icon_mouse_tests.rs:756-758`).
- Named FORK extras `live_subagent_list_shows_only_l2_and_reports_live_l3_count` and `running_count_matches_listed_live_l2_not_l3` (`FORK.md:3661-3671`) are **MISSING** from pager `*.rs` (searched this turn). Do not claim they exist. Existing tests that already encode the list law: `tasks_pane_and_dock_list_l2_rows_and_a_specialist_count` (`tasks_pane.rs:4523-4611`), `l2_row_shows_live_l3_count_not_specialist_names` (`tasks_pane.rs:3209`), `enter_on_l2_opens_the_l3_view_and_the_l1_list_stays_l2_rows_plus_a_live_l3_count` (`subagent_takeover_tests.rs:1559`), `an_l3_context_is_visible_when_that_l3_is_opened` (`task_icon_mouse_tests.rs:761`), `live_spawn_marks_l3_depth_so_the_overlay_stays_unaddressable` (`session_notification.rs:2926`), `parent_context_chip_is_l1_window_and_does_not_add_nested_windows` (`context_bar.rs:601`). Those must not be weakened.

---

## Goals

### G1 — View
L1 Subagents dock, tasks pane, header `tasks N/M`, watching counts, and `/tasks` text show only L2 rows plus a live L3 count. L3 session ids and L3 descriptions do not appear as L1 rows. Opening an L2 still shows that L2's specialists inside the L2 overlay. A left click on `1 specialist` still opens that running L3 (`clicking_the_specialist_count_opens_that_l3`). Empty Enter on the L2 floor still opens the live L3 (`enter_on_l2_opens_the_l3_view_and_the_l1_list_stays_l2_rows_plus_a_live_l3_count`).

### G2 — Tree model
Every live L3 row on the L1 registry has `depth == Some(2)` and `parent_session_id` equal to that L2's `child_session_id`. `is_l2_list_row` stays the L1 list predicate. Add a single helper for "L3 rows whose parent is this L2" and use it in overlay mirror, overlay wait count, tasks-pane overlay list, and any L2 dock list. Occupancy restore already stores `parent_session_id` and `depth` (`session.rs:82-87`, `session.rs:116-117`). Live spawn must write the same fields. Do not flatten L2 and L3 into one list.

### G3 — Context scoping
L3 spawn does not push an L1 scrollback `Subagent` block. L3 transcript lines do not paint on the L1 floor. The L1 context chip stays the L1 window. Nested overlay context is that nested session's own used/window pair, or omitted. It is not the L1 `152K/500K` copied down, and it is not printed as `0`. L2-A overlay paint, mirrored `subagent_sessions`, and wait chrome contain L2-A's L3 transcripts only.

---

## Workstreams
| # | Work | Done when |
|---|---|---|
| W0 | This plan | on disk at `plans/l2-a-subagent-visibility.md` |
| W1 | Failing tests first (the four required sentences, plus the two MISSING FORK names if they still fit those sentences) | named tests exist, observed red on today's code, existing dock tests untouched |
| W2 | Smallest product so the same tests pass: live spawn depth/parent, L1 list filters, L2-scoped L3 helper, L1 scrollback skip, watching counts and `/tasks` | W1 tests green; `tasks_pane_and_dock_list_l2_rows_and_a_specialist_count` still green |
| W3 | One reviewer | one review L3. No second reviewer unless the Operator asked |

Serial: W1 tests, then W2 product, then W3 review. Do not wait on a remote check to start W2 after W1 is red. Do not start a second `just check-remote`. Horizon does not run cargo.

## Success criteria
1. L1 Subagents view does not include L3 rows (dock, tasks pane, header tasks count, watching counts, `/tasks` text).
2. L1 does not leak L3 context into L1 transcript or L1 context scoping (no L3 `Subagent` block on L1 scrollback; L1 chip is the L1 window; opened L3 is that L3's transcript).
3. L2 overlay sees only that L2's own L3s (names, transcripts, mirrored registry, wait chrome).
4. L2-A cannot see L2-B's L3s.
5. Existing dock tests in `render.rs` and `dock_input_tests.rs` are not weakened, including `dock_h_toggles_show_done`, `finished_only_subagents_do_not_paint_a_done_summary`, dock `[clear]` header tests, and `plan_panel_open_for_present_does_not_show_in_flight_explore_jobs_on_the_subagents_list`.

## Non-goals
`viewer.rs` clippy leftover (another L2). `token_economy/ledger.rs` and SQLite (L2-B). Token-report prompts (L2-C). `GROK_TOOL_RESULT_FORMAT` (L2-D). Git. Cargo, rustc, clippy, or `cargo test` on this laptop. `just check-remote` / `just test-remote` from this implement. Telling the Operator to `/login`. Changing `MAX_SUBAGENT_DEPTH`. Teaching L1 to do the implement work.

If a needed edit shares a file with those jobs, stop and name the path as BLOCKED shared file. Do not edit `viewer.rs`, `crates/codegen/xai-grok-shell/src/token_economy/ledger.rs`, or token-report prompt files.

## Rust tests

Named tests are contracts. Write them first. Observe red. Then change the product so the same tests pass. Do not fit the tests to the code. Do not weaken existing asserts.

Shared fixture (all four new tests, and the two restored FORK names): L1 `AgentView` with `session_id = "l1-session"`. L2-A `child_session_id = "l2-a"`, `depth = Some(1)`, `parent_session_id = Some("l1-session")`, description `"Coordinate A"`. L2-B `child_session_id = "l2-b"`, same depth/parent shape, description `"Coordinate B"`. L3-A `child_session_id = "l3-a"`, `depth = Some(2)`, `parent_session_id = Some("l2-a")`, description `"Prove A"`, and an L3 scrollback line `L3_A_TRANSCRIPT`. L3-B `child_session_id = "l3-b"`, `depth = Some(2)`, `parent_session_id = Some("l2-b")`, description `"Prove B"`, scrollback line `L3_B_TRANSCRIPT`. Insert L2 and L3 `AgentView`s on L1 `subagent_views`. Do not use `running_subagent_info` as-is for L3 rows: that fixture leaves `depth: None` and `parent_session_id: None` (`mod.rs:2529-2530`), which is today's leak.

| Test | File | Asserts |
|---|---|---|
| `l1_subagents_view_does_not_include_l3_rows` | `crates/codegen/xai-grok-pager/src/app/agent_view/dock_input_tests.rs` (dock + watchers) and a twin in `crates/codegen/xai-grok-pager/src/views/tasks_pane.rs` next to `tasks_pane_and_dock_list_l2_rows_and_a_specialist_count` | Sentence: L1 Subagents view does not include L3 rows. `dock_subagent_rows()` ids are `l2-a`, `l2-b` only. No row `description` or `meta` contains `"Prove A"` or `"Prove B"`. Each L2 row `meta` contains `1 specialist`. After `tasks.sync`, pane agent ids match the dock. `watchers().subagents == 2`. `tasks_block_text` contains the L2 descriptions and does not contain the L3 descriptions. A second arm with L3 `depth: None` and `parent_session_id = Some("l2-a")` (today's live spawn) still must not list that L3. |
| `l1_does_not_leak_l3_context_into_l1_transcript_or_context_scoping` | `crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs` in `live_l3_depth_tests`, plus a draw assert in `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Sentence: L1 does not leak L3 context into L1 transcript/context scoping. Deliver L3 spawn addressed to `l2-a` (existing `handle` + `test_subagent_spawned` path). L1 scrollback has no `Subagent` block whose `child_session_id` is `l3-a` and no `L3_A_TRANSCRIPT` line. L1 `context_state.used` stays the L1 figure (seed `152_000` / `500_000`); draw text is `152K / 500K` or `152K/500K`, not an L3 window, not `0`. Deliver the same L3 spawn addressed to L1 with wire parent `l2-a`: still no L1 `Subagent` block, and `attempt.depth == Some(2)`. Keep `live_spawn_marks_l3_depth_so_the_overlay_stays_unaddressable`. Do not reshape it. |
| `l2_overlay_sees_only_that_l2s_own_l3s` | `crates/codegen/xai-grok-pager/src/app/agent_view/subagent_takeover_tests.rs` next to `enter_on_l2_opens_the_l3_view_and_the_l1_list_stays_l2_rows_plus_a_live_l3_count` | Sentence: L2 overlay sees only that L2's own L3s. `open_subagent_fullscreen("l2-a")`, draw. Overlay text contains `Prove A` and/or `L3_A_TRANSCRIPT`. It does not contain `Prove B` or `L3_B_TRANSCRIPT`. After the draw (mirror dropped), L2-A child `subagent_sessions` does not keep L3-B. Empty Enter still opens L3-A, not L3-B. |
| `l2_a_cannot_see_l2_b_l3s` | same `subagent_takeover_tests.rs` file | Sentence: L2-A cannot see L2-B's L3s. Same two-L2 fixture. L2-A overlay paint, `mirror_parented_specialists_for_overlay` keys (inspect child map during draw, or a `pub(crate)` test helper that returns mirrored ids), overlay wait chrome, and L2-A tasks-pane items after a scoped sync: `l3-a` only. `l3-b` id, `"Prove B"`, and `L3_B_TRANSCRIPT` are absent. L1 `dock_subagent_rows` still lists `l2-a` and `l2-b` only. Repeat with L2-B overlay: `l3-b` only. |
| `live_subagent_list_shows_only_l2_and_reports_live_l3_count` | `crates/codegen/xai-grok-pager/src/app/subagent_tests.rs` (this name is MISSING today; FORK extra at `FORK.md:3663`) | `live_subagent_list` returns L2-A and L2-B only. `live_l3_count(..., "l2-a") == 1`. `format_live_l3_count(1) == Some("1 specialist")`. Do not list L3 names. |
| `running_count_matches_listed_live_l2_not_l3` | `dock_input_tests.rs` or `queue.rs` tests (this name is MISSING today; FORK extra at `FORK.md:3670`) | `watchers().subagents` equals `dock_subagent_rows().iter().filter(\|r\| r.2.spinning).count()` equals 2, not 4. Header `tasks N/M` uses that L2 count. |

Keep, do not weaken: `dock_h_toggles_show_done`, `finished_only_subagents_do_not_paint_a_done_summary`, `dock_clear_drops_ghost_rows_and_header_tasks_match_live_subagents_list_of_one`, `one_live_nested_goal_plan_writer_must_not_leave_header_tasks_at_330_of_347`, `plan_panel_open_for_present_does_not_show_in_flight_explore_jobs_on_the_subagents_list`, `l2_row_shows_live_l3_count_not_specialist_names`, `tasks_pane_and_dock_list_l2_rows_and_a_specialist_count`, `clicking_the_specialist_count_opens_that_l3`, `an_l3_context_is_visible_when_that_l3_is_opened`, `parent_context_chip_is_l1_window_and_does_not_add_nested_windows`.

## Product (W2, after red)

Smallest change that makes the same tests pass:

1. **Live spawn tree.** In the L1 `SubagentSpawned` constructor (`session_notification.rs:802-814`), if `parent_session_id` is already a row in `subagent_sessions`, set `depth` to that row's depth saturating-add 1, else `Some(1)` for an L1-spawned L2. Match `apply_nested_subagent_update` (`session_notification.rs:2132-2143`). Treat that nested parent as L3 for the L1 scrollback skip even when the wire parent string is L1. Do not push an L3 `Subagent` block onto L1.
2. **One L3-for-this-L2 helper** in `subagent.rs` next to `live_l3_count`. Use it from `mirror_parented_specialists_for_overlay`, `overlay_nested_specialist_count`, tasks-pane overlay listing, and any L2 dock list. Do not list another L2's L3s.
3. **L1 list filters.** `watchers().subagents` (`queue.rs:304-308`) and `tasks_block_text` (`status_blocks.rs:155-168`) use `is_l2_list_row` (or `live_subagent_list`). Use the unused import in `status_blocks.rs`. Tasks-pane fallback (`tasks_pane.rs:1026-1038`): on L1 (`child_link` none), never list L3s just because `l2_rows` is empty. On an L2 overlay, list only `l3_rows_for_l2` for this view's session id.
4. **Context.** Keep `stamp_nested_child_context` / `paint_opened_l3_context` omit-if-missing. Do not copy L1 `session_sampling_window` onto an L3. Do not edit `context_bar.rs` unless a new test proves the chip still sums nested windows after (1)-(3). Do not edit `l2_token_tracking.rs` unless that file is the only remaining leak; it is not L2-B's ledger, but keep the write set to pager view/tree/context.

Write set: `panes.rs`, `tasks_pane.rs`, `subagent.rs`, `subagent_tests.rs`, `session_notification.rs`, `status_blocks.rs`, `queue.rs`, `subagent_takeover.rs`, `dock_input_tests.rs`, `render.rs` (tests), `subagent_takeover_tests.rs`. Occupancy `session.rs` only if restore still drops `depth` on a vacant L3 insert (today it copies `row.depth`).
