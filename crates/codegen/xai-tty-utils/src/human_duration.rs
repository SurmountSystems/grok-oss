//! Compact operator-facing duration text.
//!
//! Times at or above 60 seconds never print as a raw second count.

use std::time::Duration;

/// Compact duration: `5.2s`, `32s`, `15m43s`, `1h2m`.
///
/// Under 10 seconds uses one decimal. Under 60 seconds uses whole seconds.
/// Under 60 minutes uses minutes plus leftover seconds. Longer waits use
/// hours plus leftover minutes.
pub fn format_human_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    if total_secs < 10 {
        return format!("{:.1}s", d.as_secs_f64());
    }
    if total_secs < 60 {
        return format!("{total_secs}s");
    }
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins < 60 {
        return format!("{mins}m{secs}s");
    }
    let hours = mins / 60;
    let remaining_mins = mins % 60;
    format!("{hours}h{remaining_mins}m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_human_duration_943_seconds_is_minutes() {
        assert_eq!(format_human_duration(Duration::from_secs(943)), "15m43s");
    }

    #[test]
    fn format_human_duration_buckets_seconds_minutes_hours() {
        let cases = [
            (Duration::from_millis(500), "0.5s"),
            (Duration::from_secs_f64(5.23), "5.2s"),
            (Duration::from_secs(10), "10s"),
            (Duration::from_secs(59), "59s"),
            (Duration::from_secs(60), "1m0s"),
            (Duration::from_secs(943), "15m43s"),
            (Duration::from_secs(3600), "1h0m"),
            (Duration::from_secs(3725), "1h2m"),
        ];
        for (d, expected) in cases {
            assert_eq!(format_human_duration(d), expected, "{d:?}");
        }
    }
}
