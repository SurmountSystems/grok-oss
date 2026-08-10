# Fix: typed prompt goes nowhere while session is busy (2026-08-09)

## Operator pain

Typed a follow-up (always-expand thinking). Composer cleared. Message felt
swallowed for a long time. Operator sent a second message in panic. First
ask was eventually received and an implementer ran.

## Diagnosis (code)

Send-while-busy already **enqueued** correctly (server-authoritative immediate
queue or local FIFO). The void was **feedback**, not drop.

| Path | What happened | Visible ACK before fix |
|------|----------------|------------------------|
| Mid-turn plain Enter | `immediate_server_send` / local enqueue; composer cleared | Queue pane (if noticed), `+N` badge, ephemeral tip **"Queued · Enter to interject"** (seen-capped ×3, config-gated, unrenderable when tip row blocked) |
| Mid-turn **thinking / tools** (not sendable wait) | Same queue | Status **`N queued` was 0** — `held_queue_count` only counted sendable wait or bg-subagent hold |
| Status while tools run | `is_tool` branch | Queue suffix never painted on tool rows at all |
| Idle + bg subagent hold | Local enqueue, drain blocked | Status force hint after first item; **no tip**, no reliable toast |
| Cancel-and-send (parked empty wait) | Unblock | Correct human rail / send-now (not this bug) |
| Soft interject | Human rail paint | Already OK |

**Not root cause here:** double human rail (`bug:queued-prompts-double-up`, fixed
separately); plan panel focus (Enter still routes `SendPrompt` when prompt
focused). Host parent multi-wait without interim chat ACK is process UX, not
this TUI path — product still needed the busy-queue ACK.

## Contract

When operator sends while busy (turn running, subagents holding, queue mode):

1. **Immediate** visible feedback (toast and/or status and/or queue pane)
2. Composer clear must not feel like a void
3. Pending item stays in UI until drained
4. Fail loud on enqueue refuse (reconnect toast already); restore not needed when queue succeeds

## Fix (minimal)

1. **`held_queue_count`** — also non-zero while a **primary turn is running**
   (thinking / tools / streaming), not only sendable wait or bg hold.
   File: `crates/codegen/xai-grok-pager/src/app/agent_view/queue.rs`

2. **Turn status** — paint ` · N queued` / ` · N queued — Enter to interject`
   for **any** running-turn activity with held rows (thinking, tools, waits).
   Reserve suffix width so tool labels truncate cleanly.
   File: `crates/codegen/xai-grok-pager/src/views/turn_status.rs`

3. **Toast ACK** — `ack_followup_queued` → toast **`Queued`** after a successful
   busy queue (server immediate path and local path when the row stays held).
   Not shown for clean idle drain (human rail is enough) or cancel-and-send.
   File: `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs`

Ephemeral tip kept as optional education; toast + status no longer depend on tip
cap.

## TDD

| Test | Role | Result |
|------|------|--------|
| `send_while_busy_acks_with_queued_toast_and_held_count` | Server mid-turn: toast + held_count + clear composer | green |
| `send_while_running_local_path_acks_queued_toast` | Local FIFO mid-turn: toast + held_count | green |
| `running_thinking_shows_queued_hint_outside_sendable_wait` | Status outside wait | green |
| `running_tool_shows_queued_hint` | Status on tool row | green |
| Related: `send_prompt_while_running_*`, `held_queue_count_matches_*`, `interject_contract_*`, `queued_hint_renders_after_phase_timer`, full `views::turn_status::tests` (61) | regression | green |

```bash
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # exit 0
cargo test -p xai-grok-pager --lib -- \
  send_while_busy_acks_with_queued_toast_and_held_count \
  send_while_running_local_path_acks_queued_toast \
  running_thinking_shows_queued_hint \
  running_tool_shows_queued_hint
# + related filters above
```

## Dogfood

1. Mid-turn (thinking or tool): type follow-up, Enter → toast **Queued**, status
   ` · 1 queued — Enter to interject` (plain top), queue pane row, composer empty.
2. Idle + live background subagent: Enter queues → toast **Queued**, status force
   suffix after hold.
3. Clean idle send: still starts turn with human rail; **no** Queued toast.
4. Parked empty wait + Enter with text: cancel-and-send (no Queued toast).

## Files

- `crates/codegen/xai-grok-pager/src/app/agent_view/queue.rs`
- `crates/codegen/xai-grok-pager/src/views/turn_status.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/prompt.rs`

No git commit/stage/push.

## Host process note

Long parent multi-wait on subagents without interim operator ACK is separate
HITL process debt. Product busy-queue ACK is fixed above; do not rely on host
chat silence as the only signal.
