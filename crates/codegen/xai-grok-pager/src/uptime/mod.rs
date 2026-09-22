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

#[cfg(test)]
pub use store::{
    BANNER_DATACENTER_INCIDENT, BANNER_MODEL_SERVING_ISSUES, Observation, StoreError,
    column_encodings, writer_properties,
};
pub use store::{
    Outcome, UptimeStore, hide_announcement_keeps_row, text_beside_status, uptime_dir,
};
#[cfg(not(test))]
pub use store::{record_announcement_if_recognized, record_completed_observation};
pub use window::cap_uptime_status_segment;
#[cfg(test)]
pub use window::format_uptime_beside_status;
pub use window::{WindowPair, uptime_slash_output};

use std::sync::atomic::{AtomicU64, Ordering};

static EXTRA_API_REQUESTS: AtomicU64 = AtomicU64::new(0);

/// Requests this series made to xAI. Recording local observations does not
/// increment this. The probe path does not send, so this stays zero.
pub fn extra_api_requests() -> u64 {
    EXTRA_API_REQUESTS.load(Ordering::Relaxed)
}

/// Synthetic probes are off. Nothing in this module arms a timer or sends one.
pub fn synthetic_probe_enabled() -> bool {
    false
}

/// The path that would send a synthetic probe. The switch is off, so this
/// does not send a request. It still does not send if the switch is on.
fn send_synthetic_probe() {
    // The switch is off. This does not send a request when it is on, either.
    let _ = synthetic_probe_enabled();
}
