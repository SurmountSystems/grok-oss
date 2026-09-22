//! Text block that sits beside the status line. It only formats numbers
//! the caller already read. It does not write Parquet and it does not call xAI.

/// Short window, in words, for the status-line block.
pub const SHORT_WINDOW_WORDS: &str = "15 minutes";

/// Longer window, in words, for the status-line block.
pub const LONG_WINDOW_WORDS: &str = "24 hours";

/// Short window length in unix milliseconds.
pub const SHORT_WINDOW_MS: i64 = 15 * 60 * 1000;

/// Longer window length in unix milliseconds.
pub const LONG_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;

/// Counts for one time range. Token totals are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowStats {
    pub duration_words: &'static str,
    pub observation_count: u64,
    pub succeeded_count: u64,
    pub http_500_count: u64,
    pub measured_latency_sum_ms: i128,
    pub measured_latency_count: u64,
    /// True when at least one row stored tokens as not fetched.
    pub tokens_not_fetched: bool,
}

impl WindowStats {
    pub fn empty(duration_words: &'static str) -> Self {
        Self {
            duration_words,
            observation_count: 0,
            succeeded_count: 0,
            http_500_count: 0,
            measured_latency_sum_ms: 0,
            measured_latency_count: 0,
            tokens_not_fetched: false,
        }
    }
}

/// The two ranges the status line can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPair {
    pub last_15_minutes: WindowStats,
    pub last_24_hours: WindowStats,
}

impl WindowPair {
    pub fn empty() -> Self {
        Self {
            last_15_minutes: WindowStats::empty(SHORT_WINDOW_WORDS),
            last_24_hours: WindowStats::empty(LONG_WINDOW_WORDS),
        }
    }
}

/// Shown when the installed DuckDB shared library cannot be loaded.
/// Not an error dialog and not a made-up sample.
pub fn format_tracking_off() -> String {
    "uptime tracking is off because DuckDB is not installed".to_string()
}

/// Small text block meant to sit beside the status line.
/// Not a second dashboard and not a replacement for token chrome.
pub fn format_uptime_beside_status(windows: &WindowPair) -> String {
    format!(
        "{}\n{}",
        format_one(&windows.last_15_minutes),
        format_one(&windows.last_24_hours)
    )
}

fn format_one(stats: &WindowStats) -> String {
    let duration = stats.duration_words;
    if stats.observation_count == 0 {
        return format!("last {duration}: no observations in the last {duration}");
    }
    let share = share_text(stats.succeeded_count, stats.observation_count);
    let latency = latency_text(stats);
    let mut line = format!(
        "last {duration}: {share}, HTTP 500 count {}, {latency}",
        stats.http_500_count
    );
    if stats.tokens_not_fetched {
        line.push_str(", tokens not fetched");
    }
    line
}

fn share_text(succeeded: u64, total: u64) -> String {
    let tenths = succeeded.saturating_mul(1000) / total.max(1);
    format!(
        "share succeeded {succeeded}/{total} ({}.{}%)",
        tenths / 10,
        tenths % 10
    )
}

fn latency_text(stats: &WindowStats) -> String {
    if stats.measured_latency_count == 0 {
        return "no measured latency".to_string();
    }
    let count = i128::from(stats.measured_latency_count);
    let tenths = stats.measured_latency_sum_ms.saturating_mul(10) / count;
    let whole = tenths.div_euclid(10);
    let frac = tenths.rem_euclid(10);
    format!("measured mean latency {whole}.{frac} ms")
}
