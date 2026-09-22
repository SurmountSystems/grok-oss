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

/// Counts for one time range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowStats {
    pub duration_words: &'static str,
    pub observation_count: u64,
    pub succeeded_count: u64,
    pub http_500_count: u64,
    pub measured_latency_sum_ms: i128,
    pub measured_latency_count: u64,
    /// Sum of stored token counts when every model observation in the window
    /// has one. `None` omits the token clause. This is not an estimate.
    pub fetched_token_sum: Option<i128>,
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
            fetched_token_sum: None,
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
        "{} · {}",
        format_one(&windows.last_15_minutes),
        format_one(&windows.last_24_hours)
    )
}

/// Status-line cap. Keeps SuperGrok period limits on the one-line bar.
pub fn cap_uptime_status_segment(text: &str) -> String {
    const TRACKING_OFF: &str = "uptime tracking is off because DuckDB is not installed";
    const MAX_COLS: usize = 24;
    if text == TRACKING_OFF {
        return "uptime off, no DuckDB".to_string();
    }
    let width = unicode_width::UnicodeWidthStr::width(text);
    if width <= MAX_COLS {
        return text.to_string();
    }
    let mut out = String::new();
    let mut cols = 0usize;
    for ch in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if cols + w > MAX_COLS.saturating_sub(3) {
            break;
        }
        out.push(ch);
        cols += w;
    }
    out.push_str("...");
    out
}

fn short_window_label(duration_words: &str) -> &str {
    if duration_words == SHORT_WINDOW_WORDS {
        "15m"
    } else if duration_words == LONG_WINDOW_WORDS {
        "24h"
    } else {
        duration_words
    }
}

fn format_one(stats: &WindowStats) -> String {
    let duration = stats.duration_words;
    if stats.observation_count == 0 {
        return format!("{} none", short_window_label(duration));
    }
    let share = share_text(stats.succeeded_count, stats.observation_count);
    let latency = latency_text(stats);
    let mut line = format!(
        "last {duration}: {share}, HTTP 500 count {}, {latency}",
        stats.http_500_count
    );
    if let Some(sum) = stats.fetched_token_sum {
        line.push_str(&format!(", tokens {sum}"));
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
