# Fix: queued / interjected prompts doubling in transcript (2026-08-09)

## Contract

One human send (queue or soft interject) must produce **one** green human rail
in scrollback. Not two identical `UserPrompt` bubbles for the same text.

## Root cause

Path: soft interject of a typed or **queued** follow-up, then the running turn
ends before the shell drains `pending_interjections`.

1. **First paint** — `dispatch_interject` (local optimistic + id) and/or shell
   `x.ai/session/interjection` broadcast paints
   `RenderBlock::interjection_prompt` (same green human rail as a normal
   user bubble; `is_interjection: true`).
2. **Shell fallback** — if the interjection misses the turn, the shell queues
   an `interject-fallback-*` prompt (`queue_interjection_fallback_prompt`).
   Live user-echo is **persist-only** (no second ACP text chunk on the wire).
3. **Second paint (bug)** — when that fallback becomes the running turn,
   `apply_turn_start_shim` paints a **new** `user_prompt` from `running_text`.
   Trailing reuse deliberately **skips** `is_interjection` blocks (so a
   mid-turn steer is not stolen by the next ordinary turn), so the interjection
   bubble was never claimed → **two identical green rails**.

This matches dogfood with **Enter:interject**, queue chips still showing other
follow-ups, and image-bearing text (`[Image #1]`): soft interject of a local
queued row uses `Action::Interject` (paint once), then a turn-end race turns
it into `interject-fallback-*` and the shim doubled it.

Related (already guarded, not the bug here):

- FIFO handoff `expect_user_echo` arm at stash (`fifo_handoff_user_echo_not_duplicated`)
- PTY `queued_message_renders_once_not_twice` (held queue row, not interject-fallback)

## Fix

In `apply_turn_start_shim` (`crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs`):

- For `prompt_id` starting with `interject-fallback-`, also claim a trailing
  **interjection** bubble with the same text (new helper
  `trailing_interjection_matching`).
- Clear `expect_user_echo` for that prefix (shell never broadcasts a live
  echo for these turns; a stuck skip would swallow the next real turn).

Ordinary prompt ids still do **not** claim interjection bubbles (guard test).

## TDD

| Step | Test | Result |
|------|------|--------|
| Red contract | `shim_reuses_interjection_bubble_for_interject_fallback_turn` | fails without reuse (len +1, two rails) |
| Green | same test after fix | pass |
| Guard | `shim_does_not_claim_interjection_for_ordinary_prompt_id` | pass |
| Related | `fifo_handoff_user_echo_not_duplicated`, `shim_reuses_*`, `interject_contract_*` | pass |

```bash
cargo test -p xai-grok-pager --lib -- \
  shim_reuses_interjection_bubble_for_interject_fallback_turn \
  shim_does_not_claim_interjection_for_ordinary_prompt_id \
  fifo_handoff_user_echo_not_duplicated interject_contract
cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings   # clean
```

`--all-targets` still hits pre-existing clippy noise in other tests/benches
(not introduced by this change).

## Dogfood

After rebuild:

1. Mid-turn: queue a follow-up (with or without image), then soft Interject it
   (empty Enter / row `[Interject]` / chord) near the end of the turn so the
   shell may convert it to a fallback turn.
2. Expect **one** green rail for that text; no twin bubble when the next turn
   starts.
3. Direct mid-turn Interject that stays in the same turn should still paint
   once (unchanged path).
4. Footer `Enter:interject` with a different `#N` queue row is independent;
   that row should still appear once as a queue chip, not as a second scrollback
   copy of a prior message.

## Files

- `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` — reuse + skip clear + tests

No git commit/stage/push.
