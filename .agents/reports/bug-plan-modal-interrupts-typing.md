# Plan present must not interrupt typing

**Date:** 2026-08-14
**Package:** `xai-grok-pager`
**Host spawn:** this L2 session had no `spawn_subagent` tool. Work stayed on L2.

Operator report (additive): while mid-thought in plan mode, present / the
plan.md viewer / the 1.0.3 plan modal stole keys. Human line was "oh you
interrupted my typing." Standing law: L1 is modal-free. Plan review is on
demand (`/view-plan`, status click, panel CTAs). Empty freeform Enter never
approves. SuperGrok is paid. This slice says **included SuperGrok period
limits**, never "free SuperGrok."

## Named contract

1. Plan present, the isolated `plan.md` viewer, and approval park must not
   steal composer focus or swallow ordinary typing.
2. Keys `a` / `A` / `?` / `s` / `q` are accelerators only when the plan panel
   has empty prompt focus. If the composer has text, or is the focused typer,
   those keys are text.
3. The Thinking overlay, "You asked to revise" chrome, and a side-panel open
   must not take exclusive keyboard from an in-progress compose.
4. No questionnaire modal. Enter still never approves.
5. Do not undo token-economy, five-CTA vocabulary, or the limits hub.

## Source already correct vs leftover capture

Docs and FORK already said L1 stays the typer. After the 1.0.3 restack, the
**source did not**. Live TUIs stay on the old binary until `/rebuild` and a
full quit/reopen. This was a source bug, not only a stale binary.

| Path | Before this slice | After |
|------|-------------------|--------|
| `handle_plan_feedback_key` in `agent_view/plan.rs` | Already gated `a`/`A`/`?`/`s`/`q` on empty prompt. Enter never approves. **Already correct.** | Unchanged. |
| `handle_exit_plan_mode` in `acp_handler/interactions.rs` | Always `stash()` then `set_text("")`. Left Preview when `plan.md` auto-opened. Wiped a live draft. **Capture.** | Keeps a non-empty draft and cursor. Sets Prompt focus when a draft exists. |
| `reopen_plan_approval` in `agent_view/plan.rs` | Same wipe. **Capture.** | Same keep-draft / Prompt-when-draft rule. |
| `handle_line_viewer_key` in `agent_view/viewer.rs` | Isolated `plan.md` treated `a`/`A`/`?`/`s`/`q`/`c` as CTAs with **no** empty-prompt check. Mid-compose `a` Approved. Empty `h` never reached the composer. **Capture.** | Bare `Char` / Backspace / Delete during approval forward to `handle_plan_feedback_key`. Chords stay with the viewer. |
| Thinking overlay / turn status | Scrollback and status paint only. Not a `KeyOwner`. **Already correct.** | Unchanged. |
| "You asked to revise" | Human-line / loop chrome. Not exclusive key capture. **Already correct.** | Unchanged. |
| Modal park (`[ui] plan_approval_park = modal`) | Paint-only fullscreen on the same viewer. Same unguarded `a`/`q` as soft park. **Capture via viewer, not a second modal.** | Same printable forward. Fullscreen paint stays. |
| Five-CTA footer, token-economy, limits hub | Out of this slice. | Not touched. |

## TDD

### Red (tests first, product still wiping / capturing)

```bash
CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-modal-typing-target \
TMPDIR=/home/hunter/.cache/grok-oss-tmp \
cargo test -p xai-grok-pager --offline --lib -- \
  exit_plan_mode_keeps_mid_compose_draft_and_a_types \
  exit_plan_mode_modal_park_does_not_steal_mid_compose_keys \
  exit_plan_mode_empty_present_printable_goes_to_composer \
  plan_md_preview_mid_compose_a_types_does_not_approve \
  plan_md_preview_empty_a_still_approves \
  plan_md_preview_empty_printable_goes_to_composer
```

**First run after adding tests, before the product edit:** exit **101**.
1 passed, 5 failed.

| Test | Fail reason |
|------|-------------|
| `exit_plan_mode_keeps_mid_compose_draft_and_a_types` | `present must keep the live composer draft, got ""` |
| `exit_plan_mode_modal_park_does_not_steal_mid_compose_keys` | same wipe: modal present cleared `still typing a thought` |
| `exit_plan_mode_empty_present_printable_goes_to_composer` | `printable keys go to the composer after present, got ""` |
| `plan_md_preview_mid_compose_a_types_does_not_approve` | `plan.md Preview must not Approve while the composer has a draft` |
| `plan_md_preview_empty_printable_goes_to_composer` | `printable keys go to the composer while plan.md is open, got ""` |
| `plan_md_preview_empty_a_still_approves` | **already green** (unguarded Preview `a` still Approved) |

This is a real red, not a fake red. Present wiped the draft. Isolated
`plan.md` swallowed printables and treated mid-compose `a` as Approve.

### Green (same six filters after the product edit)

Same command, isolated target: **6 passed**, exit **0**.

Nearby empty-prompt accelerators still green (exit **0**):

- `a_on_empty_revise_prompt_approves`
- `s_on_empty_prompt_decisively_revises`
- `question_mark_on_empty_prompt_focuses_clarify`
- `capital_a_on_empty_prompt_focuses_notes`
- `empty_enter_on_revise_prompt_does_not_approve`

## What changed

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-pager/src/app/acp_handler/interactions.rs` | `handle_exit_plan_mode` keeps a mid-compose draft and cursor. Only clears when the composer was already empty, so empty-prompt `a`/`s`/`q` stay accelerators. Prompt focus when a draft exists. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs` | `reopen_plan_approval` uses the same keep-draft / Prompt-when-draft rule. |
| `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` | `plan_preview_key_is_composer_text` plus an early forward of bare letters and delete keys into `handle_plan_feedback_key`. |
| `crates/codegen/xai-grok-pager/src/app/acp_handler/tests/plan_mode.rs` | Three present-path tests (soft park, modal park, empty present printable). |
| `crates/codegen/xai-grok-pager/src/app/agent_view/viewer_tests.rs` | Three isolated-`plan.md` tests (mid-compose `a`, empty `a` still Approves, empty printable types). |

No questionnaire modal. Enter still does not approve. Five-CTA labels,
token-economy, and the limits hub were not edited.

Preview `c` during approval now types `c` (it is a bare letter). Enter still
opens a line comment. That is the L1 modal-free rule, not a regression of
the five CTAs.

## Leftovers

- **Live binary is still old** until `/rebuild` and a full quit/reopen. A
  1.0.3 TUI will keep interrupting typing until that happens.
- The later `a`/`A`/`?`/`s`/`q`/`c`/`x`/`y` arms in `handle_line_viewer_key`
  are now unreachable for unmodded letters while approval is open, because
  those chars return earlier. Smallest fix left that dead code in place.
- Empty-prompt Preview `x` (delete comment) and `y` (copy) no longer fire
  during approval. Those letters type. Mouse and Enter still comment. Casual
  `plan.md` (no approval) still uses `x`/`y`.
- Thinking overlay and "You asked to revise" were not exclusive owners. No
  product change there.
- `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` still fails
  on **pre-existing** files this slice did not touch (`doctor_early_dispatch.rs`
  canonicalize, `settings_e2e.rs` unnecessary_min_or_max, `render.rs`
  expect_fun_call, `diagnostics/fix_tests.rs` canonicalize,
  `scrollback/selection.rs` identity_op). Not mopped.

## Commands and exit codes

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager` | 0 |
| Isolated `cargo test -p xai-grok-pager --offline --lib --` (six new tests, **red**, tests only) | 101 (1 passed / 5 failed) |
| Same six after product | 0 (6 passed) |
| Isolated related empty-prompt / Enter filters (five tests) | 0 |
| Isolated `cargo clippy -p xai-grok-pager --offline --lib -- -D warnings` | 0 |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` (earlier, pre-existing) | 101 |

Isolated env: `CARGO_TARGET_DIR=/home/hunter/.cache/grok-oss-modal-typing-target`,
`TMPDIR=/home/hunter/.cache/grok-oss-tmp`. rustc 1.97.1.

No `git add` / `git commit` / push.
