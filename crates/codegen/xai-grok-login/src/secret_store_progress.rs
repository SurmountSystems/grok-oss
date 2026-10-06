//! Stderr progress while interactive login blocks on OS secret-store RMW+write.
//!
//! Budget is dual-backend worst case: 2 times [`KEYRING_OP_TIMEOUT`] (6s).

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::credentials_store::KEYRING_OP_TIMEOUT;

/// Wall-clock progress budget for one interactive store op family.
pub fn secret_store_progress_budget() -> Duration {
    KEYRING_OP_TIMEOUT.saturating_mul(2)
}

/// Whether interactive login should show a secret-store progress line.
pub fn should_show_secret_store_progress() -> bool {
    io::stderr().is_terminal()
}

/// Format a single progress line (no trailing newline). No secrets.
pub fn format_secret_store_progress(elapsed: Duration, budget: Duration) -> String {
    let budget_secs = budget.as_secs().max(1);
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
    format!("Saving to OS secret store... [{bar}] {elapsed_secs}s / {budget_secs}s")
}

/// Clear the current progress line on stderr.
pub fn clear_secret_store_progress_line() {
    eprint!("\r\x1b[K");
    let _ = io::stderr().flush();
}

/// Run `op` while optionally showing a stderr second-counter up to the budget.
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
            loop {
                if stop_tick.load(Ordering::Relaxed) {
                    break;
                }
                let line = format_secret_store_progress(started.elapsed(), budget);
                eprint!("\r{line}");
                let _ = io::stderr().flush();
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
