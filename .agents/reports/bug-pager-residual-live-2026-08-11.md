# Live pager residual resample — 2026-08-11

Live full resample of remaining `xai-grok-pager` unit fails after today's mop wave.
Prior explore agent had no shell and could only synthesize; this run used real
`cargo test` under max nice.

## Commands and logs

| Step | Command | Log |
|------|---------|-----|
| 1 | `nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib 'app::dispatch::tests' -- --test-threads=8` | `/tmp/pager-dispatch-live.txt` |
| 2 | `nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8` | `/tmp/pager-lib-live.txt` |

Compile: clean. No product edits in this report wave.

## Exact pass / fail numbers

### Dispatch filter (`app::dispatch::tests`)

```
test result: ok. 1384 passed; 0 failed; 0 ignored; 0 measured; 7436 filtered out; finished in 2.59s
```

**1384 passed, 0 failed.** Dispatch is fully green after the mop wave.

### Full lib (`--lib`)

```
test result: FAILED. 8771 passed; 38 failed; 11 ignored; 0 measured; 0 filtered out; finished in 11.48s
```

| Metric | Count |
|--------|------:|
| Passed | **8771** |
| Failed | **38** |
| Ignored | **11** |
| Filtered out | 0 |
| Wall time | 11.48s |

## Remaining failed test names (38)

### A. `app::agent_view::key_owner` — **16**

1. `app::agent_view::key_owner::tests::a_parked_card_contributes_one_route_back`
2. `app::agent_view::key_owner::tests::anything_parked_in_the_overlay_keeps_an_esc_route_to_the_dashboard`
3. `app::agent_view::key_owner::tests::cancel_turn_panel_parks_and_returns_like_the_others`
4. `app::agent_view::key_owner::tests::esc_on_a_later_question_parks_before_it_leaves_the_overlay`
5. `app::agent_view::key_owner::tests::permission_ctrl_tab_is_not_a_walk`
6. `app::agent_view::key_owner::tests::permission_hints_follow_focus`
7. `app::agent_view::key_owner::tests::permission_shift_tab_walks_backwards_in_every_encoding`
8. `app::agent_view::key_owner::tests::permission_tab_walks_the_options_and_wraps`
9. `app::agent_view::key_owner::tests::plan_approval_takes_the_bar_wherever_it_takes_the_keys`
10. `app::agent_view::key_owner::tests::the_bar_follows_the_router_when_two_cards_are_open`
11. `app::agent_view::key_owner::tests::the_overlay_owns_the_park_rung_and_the_bar_says_so`
12. `app::agent_view::key_owner::tests::the_permission_esc_ladder_steps_out_one_rung_at_a_time`
13. `app::agent_view::key_owner::tests::the_plan_preview_names_tab_the_way_its_viewer_does`
14. `app::agent_view::key_owner::tests::the_route_back_never_names_a_card_the_plan_approval_outranks`
15. `app::agent_view::key_owner::tests::vim_mode_focused_card_keeps_the_tab_contract`
16. `app::agent_view::key_owner::tests::vim_mode_permission_tab_and_esc_match_default`

### B. `app::agent_view::plan::approve_plan_flush_tests` — **7**

17. `app::agent_view::plan::approve_plan_flush_tests::idle_plan_decision_draw_paints_approve_and_revise_ctas`
18. `app::agent_view::plan::approve_plan_flush_tests::idle_plan_view_only_panel_draw_self_heals_to_approval_ctas`
19. `app::agent_view::plan::approve_plan_flush_tests::plan_preview_copy_button_click_copies_whole_plan_body`
20. `app::agent_view::plan::approve_plan_flush_tests::soft_park_draw_paints_panel_approval_footer_chrome`
21. `app::agent_view::plan::approve_plan_flush_tests::soft_park_draw_resyncs_approval_ctas_when_feedback_active_was_cleared`
22. `app::agent_view::plan::approve_plan_flush_tests::soft_park_fullscreen_draw_paints_approval_ctas`
23. `app::agent_view::plan::approve_plan_flush_tests::view_plan_while_plan_mode_awaiting_decision_parks_ctas_not_view_only`

### C. `app::acp_handler` — **4**

24. `app::acp_handler::tests::queue_and_adoption::viewer_prompt_complete_error_pushes_turn_failed_marker`
25. `app::acp_handler::tests::settings::loop_fire_mode_adopts_the_loaded_session_value`
26. `app::acp_handler::tests::settings::settings_update_sharing_enabled_true_stays_forced_off`
27. `app::acp_handler::tests::turn_completion::failed_wake_turn_keeps_markerless_shape`

### D. `scrollback::state::layout` — **3**

28. `scrollback::state::layout::tests::growing_entry_above_manual_viewport_keeps_marker_at_screen_row_zero`
29. `scrollback::state::layout::tests::removal_above_wrapped_park_keeps_row_inside_wrapping_entry`
30. `scrollback::state::layout::tests::removing_entry_above_manual_viewport_keeps_marker_at_screen_row_zero`

### E. Slash / mode — **3**

31. `slash::commands::dashboard::tests::visible_everywhere_except_minimal`
32. `slash::commands::tests::shell_collision_contract_covers_every_pager_command_and_alias`
33. `slash::mode_support::tests::mode_specific_builtin_refusals_are_pinned`

### F. Singletons — **5**

34. `app::agent_view::interactions::question_answer_focus_tests::shortcut_hints_name_the_answer_walk`
35. `app::agent_view::links::link_click_tests::privacy_banner_owns_slot_and_clicks_dispatch`
36. `app::modals::command_palette_vim_input_tests::command_palette_search_bar_cursor_only_when_focused`
37. `app::turn_completion::tests::turn_end_after_park_pushes_single_marker`
38. `views::settings_modal::tests::picker_highlights_current_choice`

## Clusters by module (count)

| Cluster | N | Shared symptom (first panic sample) |
|---------|--:|-------------------------------------|
| **key_owner** (bar / Esc / Tab / park) | 16 | Hints show `["unselect", "scrollback", "dismiss"]` instead of card walk labels (`next answer`, `next choice`, option walk, Esc route). Tab walk off-by-one (`[1,2,3,0,1]` vs `[0,1,2,3,0]`). Parked Esc does not leave overlay. |
| **approve_plan_flush** (plan CTAs) | 7 | Footer paints casual `c-comment` only; five approval CTA hits missing; `⧉` copy hit target unset. |
| **acp_handler** | 4 | Three distinct settings/queue/turn contracts (see below). |
| **scrollback layout** (manual viewport pin) | 3 | Marker not held at screen row 0 on growth/removal (`row 40`, `row -15`); wrap re-pin measure fail. |
| **slash** | 3 | Dashboard offered in minimal; unreserved key `undo`; mode refusal pin missing `jump` + `timeline`. |
| **interactions** (question walk bar) | 1 | Same bar family: expected answer walk, got unselect/scrollback/dismiss. |
| **links** (privacy banner) | 1 | `banner copy painted` |
| **modals** (command palette cursor) | 1 | Search bar renders cursor when not `search_active`. |
| **turn_completion** | 1 | `agent.renders_parked()` false after park + turn end. |
| **settings_modal** | 1 | Focused row `bg` is `DarkGray` not `Reset` (`bg_visual`). |

**Related bar/hint family (key_owner + interactions + possibly turn_completion park):** ~17–18 tests. One product fix in key-owner / shortcut-hint routing likely collapses most of A + F.interactions.

## First panic per cluster (representative)

### key_owner / interactions (bar + Esc + Tab)

- **File:** `crates/codegen/xai-grok-pager/src/app/agent_view/key_owner_tests.rs` / `interactions.rs`
- Samples:
  - `permission: parked behind the scrollback, the next Esc leaves the overlay`
  - `hint_labels(...).contains("next answer"|"next choice")` fails
  - `the bar names the option walk, got ["select", "always-approve", "cancel"]`
  - `the bar names where Esc actually goes, got ["unselect", "scrollback", "dismiss"]`
  - Tab wrap: `left: [1, 2, 3, 0, 1]` / `right: [0, 1, 2, 3, 0]`
  - interactions: `step 0: the bar advertises the answer walk, got ["unselect", "scrollback", "dismiss"]`

### approve_plan_flush

- **File:** `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs`
- Samples:
  - `must not paint casual c-comment as the only footer while decision is pending`
  - `panel footer must expose all five approval CTA hit targets after soft-park draw`
  - `painted plan top bar must set ⧉ hit target`

### acp_handler (three sub-symptoms, one module)

| Test | Panic |
|------|-------|
| `viewer_prompt_complete_error_pushes_turn_failed_marker` | `agentResult must propagate into the marker` — left `"Request failed — boom. Try sending again."` right `"boom"` |
| `loop_fire_mode_adopts_the_loaded_session_value` | `/loop must enqueue an instruction and drain it` |
| `settings_update_sharing_enabled_true_stays_forced_off` | `typed /share still resolves so the disable path can run` |
| `failed_wake_turn_keeps_markerless_shape` | `non-completion wake terminals push nothing` — left `1` right `0` |

Wake-marker shape also ties to `app::turn_completion::tests::turn_end_after_park_pushes_single_marker` (`renders_parked()`).

### scrollback layout

- **File:** `crates/codegen/xai-grok-pager/src/scrollback/state/layout.rs`
- Growth: marker screen row **40** (expected 0); `virtual_y 2→42`, `scroll_offset` stuck at 2.
- Removal: marker row **-15**; prefix `virtual_y 15→0`, `scroll_offset` stuck at 15.
- Wrapped park: `anchor entry must be measured exactly before the re-pin clamps`.

### slash

- Dashboard: `must not be offered in minimal mode`
- Shell collision: `unreserved pager key undo`
- Mode refusals pin: expected set missing **`jump`** and **`timeline`** minimal-mode refusal strings (diff shows extra entries on the right / product side vs pin).

### Singletons

| Module | Panic one-liner |
|--------|-----------------|
| links | `banner copy painted` |
| modals | command palette search bar must not render cursor when not search_active |
| settings_modal | focused row must have `bg_visual`; left `DarkGray` right `Reset` |

## Next mop order (recommended)

Priority = (fail count × shared root) + operator-facing plan/bar contracts. Dispatch is already clear; do not re-mop dispatch.

1. **Mop A — key_owner bar / Esc / Tab / park (16 + 1 interactions)**
   Single largest cluster. Product focus: overlay key owner, hint labels (`next answer` / `next choice` / option walk / Esc route), Tab wrap index, parked Esc ladder. Filter:
   ```
   cargo test -p xai-grok-pager --lib 'agent_view::key_owner::tests' -- --test-threads=8
   cargo test -p xai-grok-pager --lib 'question_answer_focus_tests' -- --test-threads=8
   ```
   Expect ~17 green if root is shared.

2. **Mop B — plan approval footer / CTAs / copy hit (7)**
   Soft-park and idle draw still painting casual comment-only footer; `⧉` hit missing. Filter:
   ```
   cargo test -p xai-grok-pager --lib 'approve_plan_flush_tests' -- --test-threads=8
   ```

3. **Mop C — turn markers / park after complete (acp turn_completion + app turn_completion, 2)**
   Failed wake pushes a marker when tests want none; park state after turn end. Related to A if park chrome is wrong, but assertion text is marker-count / `renders_parked`. Filter:
   ```
   cargo test -p xai-grok-pager --lib 'failed_wake_turn_keeps_markerless_shape' -- --test-threads=8
   cargo test -p xai-grok-pager --lib 'turn_end_after_park_pushes_single_marker' -- --test-threads=8
   ```

4. **Mop D — scrollback manual viewport pin (3)**
   Layout re-pin / scroll_offset not tracking upstream growth/removal. Isolated from TUI chrome. Filter:
   ```
   cargo test -p xai-grok-pager --lib 'scrollback::state::layout::tests' -- --test-threads=8
   ```

5. **Mop E — slash registration / mode pin (3)**
   New commands (`undo`, `jump`, `timeline`) vs reserved-key and mode-refusal pin tables; dashboard visibility in minimal. Small catalog updates + product visibility. Filter:
   ```
   cargo test -p xai-grok-pager --lib 'slash::' -- --test-threads=8
   ```

6. **Mop F — acp settings leftovers (2)**
   `/loop` drain adopt + `/share` still resolves under forced-off sharing. Filter:
   ```
   cargo test -p xai-grok-pager --lib 'acp_handler::tests::settings' -- --test-threads=8
   ```

7. **Mop G — error marker text wrap (1)**
   `viewer_prompt_complete_error_pushes_turn_failed_marker`: product wraps to `"Request failed — boom. Try sending again."` while test expects raw `"boom"`. Decide product vs test intent before edit.

8. **Mop H — chrome singletons (3)**
   Privacy banner paint, command-palette cursor when unfocused, settings picker `bg_visual`. Independent; batch after A–E or parallel if disjoint files.

### Parallelism note

Safe parallel pairs after A starts (disjoint crates paths):

- D (scrollback) ‖ E (slash) ‖ H.modals / H.settings_modal
- B (plan.rs) may touch same agent_view surface as A; **serialize A then B** if same implementer, or queue writers.

## Not residual (this live pass)

- **All of `app::dispatch::tests`:** 1384/0 green.
- Full lib is 8771/38; residual is the 38 listed only.

## Reproduce

```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib 'app::dispatch::tests' -- --test-threads=8
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --lib -- --test-threads=8 2>&1 | tee /tmp/pager-lib-live.txt
rg "FAILED|test result:" /tmp/pager-lib-live.txt
```
