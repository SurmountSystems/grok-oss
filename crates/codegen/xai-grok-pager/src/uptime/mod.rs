//! Local uptime observations beside the status line.
//!
//! Each observation is one new Parquet piece. The installed DuckDB shared
//! library reads those pieces. This module does not call xAI, does not run a
//! synthetic probe, and does not add these tokens into the session ledger.
//! A missing shared library leaves tracking off. Process start does not
//! require the library.

mod shared_library;
mod store;
mod window;

#[cfg(test)]
mod tests;

pub use store::{
    BANNER_DATACENTER_INCIDENT, BANNER_MODEL_SERVING_ISSUES, Observation, Outcome, StoreError,
    StoredRow, UptimeStore, column_encodings, hide_announcement_keeps_row,
    read_rows_through_duckdb, record_announcement_if_recognized, record_completed_observation,
    text_beside_status, text_beside_status_using, uptime_dir, writer_properties,
};
pub use window::{
    LONG_WINDOW_MS, LONG_WINDOW_WORDS, SHORT_WINDOW_MS, SHORT_WINDOW_WORDS, WindowPair,
    WindowStats, format_tracking_off, format_uptime_beside_status,
};

use std::sync::atomic::{AtomicU64, Ordering};

static EXTRA_API_REQUESTS: AtomicU64 = AtomicU64::new(0);

/// Requests this series made to xAI. Recording local observations does not
/// increment this. There is no probe sender in this module.
pub fn extra_api_requests() -> u64 {
    EXTRA_API_REQUESTS.load(Ordering::Relaxed)
}

/// Synthetic probes are off. Nothing in this module arms a timer or sends one.
pub fn synthetic_probe_enabled() -> bool {
    false
}
