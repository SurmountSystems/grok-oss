//! Stderr progress while interactive login blocks on OS secret-store RMW+write.
//!
//! Budget is dual-backend worst case: **2 × [`KEYRING_OP_TIMEOUT`]** (~6s).
//! Pure format helpers are unit-tested; the ticker only runs when stderr is a
//! TTY and the caller opts in (after secret accept; never on env short-circuit).

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use super::credentials_store::KEYRING_OP_TIMEOUT;

/// Wall-clock progress budget for one interactive store op family (RMW read
/// and/or write, each dual-backend). Matches dual-backend worst case.
pub fn secret_store_progress_budget() -> Duration {
    KEYRING_OP_TIMEOUT.saturating_mul(2)
}

/// Whether interactive login should show a secret-store progress line.
///
/// Suppress for non-TTY stderr (pipes, CI capture, automation). Callers still
/// skip wrapping on env short-circuit paths that never touch the store.
pub fn should_show_secret_store_progress() -> bool {
    should_show_secret_store_progress_with(io::stderr().is_terminal())
}

/// Testable gate: show only when stderr is a terminal.
pub fn should_show_secret_store_progress_with(stderr_is_terminal: bool) -> bool {
    stderr_is_terminal
}

/// Format a single progress line (no trailing newline). No secrets.
///
/// Example: `Saving to OS secret store… [====----] 2s / 6s`
pub fn format_secret_store_progress(elapsed: Duration, budget: Duration) -> String {
    let budget_secs = budget.as_secs().max(1);
    // Cap the displayed elapsed at budget so the bar never overflows the label.
    let elapsed_secs = elapsed.as_secs().min(budget_secs);
    let width: u64 = 8;
    let filled = elapsed_secs
        .saturating_mul(width)
        .checked_div(budget_secs)
        .unwrap_or(width)
        .min(width);
    let empty = width.saturating_sub(filled);
    let bar: String = std::iter::repeat_n('=', filled as usize)
        .chain(std::iter::repeat_n('-', empty as usize))
        .collect();
    format!("Saving to OS secret store… [{bar}] {elapsed_secs}s / {budget_secs}s")
}

/// Clear the current progress line on stderr (`\r` + erase to end of line).
pub fn clear_secret_store_progress_line() {
    eprint!("\r\x1b[K");
    let _ = io::stderr().flush();
}

/// Run `op` while optionally showing a stderr second-counter up to `budget`.
///
/// When `show` is false, `op` runs with no I/O side effects. On finish (ok or
/// err), clears the progress line if one was drawn.
pub fn with_secret_store_progress<T>(show: bool, op: impl FnOnce() -> T) -> T {
    if !show {
        return op();
    }
    let budget = secret_store_progress_budget();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_tick = Arc::clone(&stop);
    let started = Instant::now();
    let ticker = thread::Builder::new()
        .name("grok-secret-store-progress".into())
        .spawn(move || {
            // Draw immediately so a fast hang still shows 0s.
            loop {
                if stop_tick.load(Ordering::Relaxed) {
                    break;
                }
                let line = format_secret_store_progress(started.elapsed(), budget);
                eprint!("\r{line}");
                let _ = io::stderr().flush();
                // ~4 Hz — enough for second boundaries without spam.
                thread::sleep(Duration::from_millis(250));
            }
        })
        .ok();

    let out = op();
    stop.store(true, Ordering::Relaxed);
    if let Some(handle) = ticker {
        let _ = handle.join();
    }
    clear_secret_store_progress_line();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_progress_at_zero_shows_empty_bar_and_budget() {
        let budget = Duration::from_secs(6);
        let line = format_secret_store_progress(Duration::ZERO, budget);
        assert_eq!(line, "Saving to OS secret store… [--------] 0s / 6s");
        // Never leak secret-like material.
        assert!(!line.to_ascii_lowercase().contains("key"));
        assert!(!line.contains("sk-"));
    }

    #[test]
    fn format_progress_mid_budget_fills_proportionally() {
        let budget = Duration::from_secs(6);
        let line = format_secret_store_progress(Duration::from_secs(3), budget);
        assert_eq!(line, "Saving to OS secret store… [====----] 3s / 6s");
    }

    #[test]
    fn format_progress_at_budget_is_full_bar() {
        let budget = Duration::from_secs(6);
        let line = format_secret_store_progress(Duration::from_secs(6), budget);
        assert_eq!(line, "Saving to OS secret store… [========] 6s / 6s");
    }

    #[test]
    fn format_progress_past_budget_caps_display() {
        let budget = Duration::from_secs(6);
        let line = format_secret_store_progress(Duration::from_secs(99), budget);
        assert_eq!(line, "Saving to OS secret store… [========] 6s / 6s");
    }

    #[test]
    fn progress_budget_is_two_times_keyring_op_timeout() {
        assert_eq!(
            secret_store_progress_budget(),
            KEYRING_OP_TIMEOUT.saturating_mul(2)
        );
        assert_eq!(secret_store_progress_budget(), Duration::from_secs(6));
    }

    #[test]
    fn tty_gate_suppresses_when_stderr_not_terminal() {
        assert!(
            !should_show_secret_store_progress_with(false),
            "non-TTY must suppress progress (automation / pipes)"
        );
        assert!(
            should_show_secret_store_progress_with(true),
            "TTY stderr may show progress"
        );
    }

    #[test]
    fn with_progress_false_is_passthrough() {
        let mut ran = false;
        let v = with_secret_store_progress(false, || {
            ran = true;
            42
        });
        assert!(ran);
        assert_eq!(v, 42);
    }
}
