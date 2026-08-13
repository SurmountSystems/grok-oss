# Pager prompt dispatch residual — green

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Agent:** L2 implementer (prompt cluster only)

## Goal

Green the 14 (live: 15) `app::dispatch::tests::prompt` fails from
`.agents/reports/bug-pager-residual-inventory-2026-08-11.md`.

## Result

```text
cargo test -p xai-grok-pager --lib 'app::dispatch::tests::prompt' -- --test-threads=8
→ 127 passed; 0 failed
```

All inventory prompt names are green (suppress, mode slash, interject/send-now,
bash-before-bind).

## Product restore (monorepo contracts; tests as spec)

| Area | Root | Fix |
|------|------|-----|
| **PromptResponse suppress** | Half-merged `handle_prompt_response` dropped disk-full push, RequestFailed suppress, 402 status recovery from banner text, and broad `(401)` match | Restored `dedicated_ux_shown` (rate/free-usage/model/credit/reauth/overflow/**disk_full**/`RequestFailed`); recover status via `parse_http_status`; push `DiskFull` when ENOSPC and trailing banner missing |
| **Mode slash** | Gate used only `available_in_minimal` with wrong copy | Central gate uses `command.mode_support().refusal(token, screen_mode)` |
| **Plain mid-turn send** | Immediate-send path armed send-now cancel on parked wait | Plain image-free sends stay **unarmed**; shell `cancelTrigger` decides |
| **Goal Send Now paint** | Painted only when `expects_send_now_cancel` | `paint_send_now_and_maybe_arm` + `arm_send_now_and_paint_dispatched`: goal paints without arming |
| **Non-running PromptResponse retire** | Unconditional `retire_send_now_painted_block` dropped goal claim blocks | Skip retire when `is_send_now_awaiting_interjection_claim` |
| **Soft queue interject** | Missing `note_self_originated_prompt` | Stamp self-originated; still no cancel arm / no paint |
| **Bash before bind** | `skip_picker_and_create_session` on unbound session | Queue + `! ` history only; drain no-ops until session binds |

## Paths touched

- `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/interject.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` (paint helpers + soft interject stamp)
- `crates/codegen/xai-grok-pager/src/app/dispatch/auth.rs` (`scrollback_has_recent_disk_full` / `_request_failed` / `trailing_session_events`)
- `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs` + `mod.rs` (`is_disk_full_error` pub re-export)

## Verify

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-pager` | done |
| `cargo test -p xai-grok-pager --lib 'app::dispatch::tests::prompt'` | **127 / 0** |
| `cargo test … interject::tests` | **5 / 0** |
| Clippy `-p xai-grok-pager --lib -D warnings` | blocked by **pre-existing** dep noise in `xai-grok-tools` (dead method + disallowed `Command::spawn`); not introduced here |

## Out of scope (not prompt residual; noted live)

- `app::dispatch::queue::tests::parked_wait_*` (3) — monorepo asserts parked look is **queue-occupancy-independent** (`renders_parked() == true` with held row); live tree tests still assert the inverted half-merge. Not owned by this prompt mop.
- `slash::mode_support::tests::mode_specific_builtin_refusals_are_pinned` — pin list drift (`jump` / `timeline` inventory), not the dispatch gate restore.

## Honesty

- No mass expect rewrites on the prompt contracts.
- Prefer monorepo product path restore (suppress + mode gate + send-now paint + plain-send unarmed + bash bind).
- Did not rewrite session/settings/lifecycle product paths.
