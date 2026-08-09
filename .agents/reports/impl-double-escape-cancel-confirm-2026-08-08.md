# Double-Escape cancel confirm (2026-08-08)

## Root cause of accidental cancel

While a turn was running, a bare `Esc` cancelled immediately (`try_handle_esc_policy` mid-turn branch → `Action::CancelTurn` on the first press).

Overlays (settings, slash/model/effort dropdowns, etc.) correctly **steal** Esc first and close without canceling. After the overlay closed, a second Esc (bounce, habit, or “one more Esc to be sure”) hit the bare policy and **cancelled the turn**. That is what happened when raising effort in the options UI: Esc left the dialog, then another Esc stopped the agent.

## Exact behavior (after fix)

| State | Esc |
|-------|-----|
| Turn running, gate ON (minimal, or fullscreen with vim scrollback off) | **1st Esc:** arm confirm only. Shortcuts bar: “press again to cancel”. Turn keeps running. **2nd Esc** within ~800ms (`esc_double_press_ttl` / `GROK_ESC_DOUBLE_PRESS_MS`): fires `CancelTurn`. Draft preserved. |
| Turn running, fullscreen vim mode | Still swallow (no cancel). Use Ctrl+C. |
| Turn already cancelling | Immediate cancel **retry** (no double-Esc arm). |
| Settings / modal / slash dropdown open | Esc closes that surface only. Same press does **not** arm cancel or cancel. Next Esc after close starts confirm-arm if the turn is still running. |
| Idle | Unchanged: clear / rewind / swallow paths (no cancel invent). |
| Ctrl+C | Unchanged: still cancels when the prompt is empty (clear-first with draft). Not double-Esc. |

Side effects on **confirm** only (second Esc): `cancel_trigger_hint = Esc`, post-cancel rewind grace. First Esc does not set those.

Any non-Esc key (or expired window) disarms the arm without canceling.

## Implementation

- `AgentView::try_handle_esc_policy` (`prompt.rs`): running + gate → `ArmPending { CancelTurn, label: "cancel", ttl }` instead of immediate cancel. Cancelling path still immediate.
- `AppView::handle_input` (`app_view.rs`): when the second press fires pending `CancelTurn`, set Esc trigger + rewind grace (policy never sees the confirm press).
- Docs: user-guide `03-keyboard-shortcuts`, tutorial `02-first-prompt`, CancelTurn long_help, input-flow comments.

## Tests + commands

```bash
cargo test -p xai-grok-pager --lib -- \
  esc_from_prompt_pane_running_turn \
  esc_from_scrollback_pane_running_turn \
  esc_running_turn_minimal \
  running_turn_esc_once_then_other \
  running_turn_settings_esc \
  esc_cancel_grace \
  overlay_esc_running_turn_non_vim \
  idle_non_empty_double_esc \
  idle_empty_no_messages_esc \
  running_slash_dropdown_esc \
  cancel_turn esc_cancels_turn_gate overlay_esc_cancelling esc_while_cancelling \
  ctrl_c_running stale_idle_clear stale_idle_rewind

cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
just install
```

New / updated contracts:

- First Esc while running → arm “cancel”, turn still running
- Second Esc → `CancelTurn` + Esc trigger
- Esc then other key → disarm, turn still running
- Settings open + Esc → modal closes, no arm, no cancel; next Esc arms only
- Idle Esc paths unchanged
- Ctrl+C cancel unchanged

PTY e2e (ignored by default) updated for 2× Esc + “press again to cancel”:

- `esc_cancels_running_turn_from_prompt_preserves_draft`
- `esc_cancels_running_turn_from_scrollback`
- `minimal_esc_cancels_running_turn`

## Dogfood steps

1. `just install` already ran; `grok-oss --version` shows the new binary.
2. Start a long turn (or any streaming response).
3. Open Settings or `/effort` / `/model` while it runs. Press **Esc** once: dialog closes, turn still running, no “Turn cancelled”.
4. Press **Esc** once more with no dialog: footer “press again to cancel”; turn still running.
5. Press **Esc** again quickly: turn cancels; draft in the composer is kept.
6. Or after step 4, type a letter: arm clears, turn keeps going.
7. Idle: Esc Esc still clear/rewind as before. Ctrl+C still cancels without double-Esc.

## Verify notes

- `cargo clippy -p xai-grok-pager --lib -- -D warnings`: green.
- `--all-targets` still has pre-existing clippy failures in unrelated benches/tests (not introduced by this change).
