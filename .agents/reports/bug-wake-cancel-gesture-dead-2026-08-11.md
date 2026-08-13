# bug: WakeCancelGesture Esc/StopClick dead_code (2026-08-11)

## Status
Fixed.

## Root cause
`WakeCancelGesture` lives in
`crates/codegen/xai-grok-pager/tests/pty_e2e/auto_wake_cancel_preserves_queued_user_prompt.rs`
with three variants: `CtrlC`, `Esc`, `StopClick`. Shared scenario body matches on all three.

Mirror tests that construct `Esc` and `StopClick` already existed:

- `auto_wake_cancel_via_esc_preserves_queued_user_prompt.rs`
- `auto_wake_cancel_via_stop_click_preserves_queued_user_prompt.rs`

…but `tests/pty_e2e_queue.rs` only `#[path]`-included the Ctrl+C module. Within that
integration binary only `CtrlC` was constructed, so rustc warned that `Esc` and
`StopClick` were never constructed (noise under `pty_e2e_queue`; risk under `-D warnings`).

## Fix
Wire the two mirror modules into the same test binary (gesture matrix as designed):

```rust
// tests/pty_e2e_queue.rs
#[path = "pty_e2e/auto_wake_cancel_preserves_queued_user_prompt.rs"]
mod auto_wake_cancel_preserves_queued_user_prompt;
#[path = "pty_e2e/auto_wake_cancel_via_esc_preserves_queued_user_prompt.rs"]
mod auto_wake_cancel_via_esc_preserves_queued_user_prompt;
#[path = "pty_e2e/auto_wake_cancel_via_stop_click_preserves_queued_user_prompt.rs"]
mod auto_wake_cancel_via_stop_click_preserves_queued_user_prompt;
```

No enum change, no `#[allow(dead_code)]`. Variants are used by construction.

## Verify
```bash
nice -n 19 ionice -c3 cargo test -p xai-grok-pager --test pty_e2e_queue --no-run
# EXIT:0
# Finished test profile … Executable tests/pty_e2e_queue.rs
# No warning about WakeCancelGesture / Esc / StopClick
# No warnings attributed to the test target (lib still has pre-existing dead_code noise)
```

## Files touched
- `crates/codegen/xai-grok-pager/tests/pty_e2e_queue.rs` (register two modules)
