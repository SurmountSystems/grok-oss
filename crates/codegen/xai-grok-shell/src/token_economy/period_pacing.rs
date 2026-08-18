//! Included SuperGrok **billing period** linear-burn pacing.
//!
//! Compares included SuperGrok period **used percent** to time elapsed through the
//! billing period. Never dollar-izes SuperGrok period %. Missing bounds → omit
//! (never invent).

use chrono::{DateTime, Duration, Utc};

/// Ahead / behind relative to linear burn through the included SuperGrok period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeriodPacing {
    /// Included SuperGrok period used percent (0–100+).
    pub usage_pct: f64,
    /// Expected used percent if burn were linear in time (0–100).
    pub expected_pct: f64,
    /// `usage_pct - expected_pct`. Positive → ahead of linear burn (using faster).
    pub delta_pct: f64,
}

impl PeriodPacing {
    /// Rounded absolute delta for display (integer percent points).
    pub fn abs_delta_rounded(self) -> i64 {
        self.delta_pct.abs().round() as i64
    }

    /// Compact chip: `"12% ahead of linear burn"` / `"8% behind linear burn"` /
    /// `"on linear burn"`.
    pub fn compact_label(self) -> String {
        let d = self.abs_delta_rounded();
        if d == 0 {
            "on linear burn".to_string()
        } else if self.delta_pct > 0.0 {
            format!("{d}% ahead of linear burn")
        } else {
            format!("{d}% behind linear burn")
        }
    }

    /// Full sentence for `/limits` and `/usage`.
    pub fn full_sentence(self) -> String {
        let d = self.abs_delta_rounded();
        if d == 0 {
            "Included SuperGrok period burn is on linear pace for this billing period.".to_string()
        } else if self.delta_pct > 0.0 {
            format!(
                "Included SuperGrok period burn is {d}% ahead of linear burn for this billing period \
(using included SuperGrok period limits faster than time share)."
            )
        } else {
            format!(
                "Included SuperGrok period burn is {d}% behind linear burn for this billing period \
(using included SuperGrok period limits slower than time share; more left than time share)."
            )
        }
    }

    /// Console-live honesty: SuperGrok pacing is not the live console principal.
    pub fn full_sentence_console_live(self) -> String {
        format!(
            "{} (SuperGrok period, not live principal).",
            self.full_sentence().trim_end_matches('.')
        )
    }

    /// Compact chip when live sampling is console.
    pub fn compact_label_console_live(self) -> String {
        format!("{} (SuperGrok period)", self.compact_label())
    }
}

/// Compute period pacing from used % and absolute period bounds.
///
/// Returns `None` when start/end missing, end ≤ start, or usage is not finite.
pub fn compute_period_pacing(
    usage_pct: f64,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<PeriodPacing> {
    if !usage_pct.is_finite() {
        return None;
    }
    let length = period_end.signed_duration_since(period_start);
    if length <= Duration::zero() {
        return None;
    }
    let elapsed = now.signed_duration_since(period_start);
    // Before period start or after end: clamp elapsed into [0, length].
    let elapsed_clamped = if elapsed < Duration::zero() {
        Duration::zero()
    } else if elapsed > length {
        length
    } else {
        elapsed
    };
    let expected_pct =
        100.0 * (elapsed_clamped.num_milliseconds() as f64) / (length.num_milliseconds() as f64);
    let expected_pct = expected_pct.clamp(0.0, 100.0);
    let delta_pct = usage_pct - expected_pct;
    Some(PeriodPacing {
        usage_pct,
        expected_pct,
        delta_pct,
    })
}

/// Derive period start when wire only gives end + period type.
///
/// - Weekly → end − 7 days
/// - Monthly → end − 30 days
/// - Unknown type → `None` (omit pacing rather than invent)
pub fn derive_period_start_from_end_and_type(
    period_end: DateTime<Utc>,
    period_type: Option<&str>,
) -> Option<DateTime<Utc>> {
    let t = period_type.unwrap_or("");
    if t.contains("WEEKLY") {
        Some(period_end - Duration::days(7))
    } else if t.contains("MONTHLY") {
        Some(period_end - Duration::days(30))
    } else {
        None
    }
}

/// Resolve period start: prefer explicit start, else derive from end + type.
pub fn resolve_period_start(
    period_start: Option<DateTime<Utc>>,
    period_end: Option<DateTime<Utc>>,
    period_type: Option<&str>,
) -> Option<DateTime<Utc>> {
    if let Some(s) = period_start {
        return Some(s);
    }
    let end = period_end?;
    derive_period_start_from_end_and_type(end, period_type)
}

/// Parse RFC 3339 (or similar) into UTC. Fail → None.
pub fn parse_rfc3339_utc(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// High-level helper: usage % + optional start/end/type + now → pacing.
pub fn period_pacing_from_bounds(
    usage_pct: Option<f64>,
    period_start_raw: Option<&str>,
    period_end_raw: Option<&str>,
    period_type: Option<&str>,
    now: DateTime<Utc>,
) -> Option<PeriodPacing> {
    let usage = usage_pct?;
    let end = period_end_raw.and_then(parse_rfc3339_utc)?;
    let start_explicit = period_start_raw.and_then(parse_rfc3339_utc);
    let start = resolve_period_start(start_explicit, Some(end), period_type)?;
    compute_period_pacing(usage, start, end, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn dt(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn half_period_half_usage_is_on_pace() {
        let start = dt(2026, 8, 1, 0);
        let end = dt(2026, 8, 8, 0); // 7 days
        let now = dt(2026, 8, 4, 12); // 3.5 days = 50%
        let p = compute_period_pacing(50.0, start, end, now).unwrap();
        assert!((p.expected_pct - 50.0).abs() < 0.1, "{}", p.expected_pct);
        assert!(p.abs_delta_rounded() == 0);
        assert_eq!(p.compact_label(), "on linear burn");
        assert!(p.full_sentence().contains("on linear pace"));
    }

    #[test]
    fn ahead_of_linear_burn() {
        let start = dt(2026, 8, 1, 0);
        let end = dt(2026, 8, 8, 0);
        let now = dt(2026, 8, 4, 12); // 50% time
        let p = compute_period_pacing(62.0, start, end, now).unwrap();
        assert!(p.delta_pct > 0.0);
        assert_eq!(p.compact_label(), "12% ahead of linear burn");
        assert!(p.full_sentence().contains("ahead of linear burn"));
        assert!(!p.full_sentence().contains("winning"));
    }

    #[test]
    fn behind_linear_burn() {
        let start = dt(2026, 8, 1, 0);
        let end = dt(2026, 8, 8, 0);
        let now = dt(2026, 8, 4, 12);
        let p = compute_period_pacing(42.0, start, end, now).unwrap();
        assert!(p.delta_pct < 0.0);
        assert_eq!(p.compact_label(), "8% behind linear burn");
        assert!(p.full_sentence().contains("behind linear burn"));
        // Not the awkward "behind of".
        assert!(!p.compact_label().contains("behind of"));
    }

    #[test]
    fn missing_bounds_omit() {
        assert!(
            period_pacing_from_bounds(
                Some(50.0),
                None,
                None,
                Some("USAGE_PERIOD_TYPE_WEEKLY"),
                Utc::now()
            )
            .is_none()
        );
        assert!(
            period_pacing_from_bounds(
                Some(50.0),
                None,
                Some("2026-08-08T00:00:00Z"),
                None, // unknown type, no start
                Utc::now()
            )
            .is_none()
        );
    }

    #[test]
    fn weekly_derives_start_from_end() {
        let p = period_pacing_from_bounds(
            Some(50.0),
            None,
            Some("2026-08-08T00:00:00Z"),
            Some("USAGE_PERIOD_TYPE_WEEKLY"),
            dt(2026, 8, 4, 12),
        )
        .unwrap();
        assert!((p.expected_pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn zero_length_period_omits() {
        let t = dt(2026, 8, 1, 0);
        assert!(compute_period_pacing(10.0, t, t, t).is_none());
    }

    #[test]
    fn console_live_labels_mark_not_principal() {
        let p = PeriodPacing {
            usage_pct: 60.0,
            expected_pct: 50.0,
            delta_pct: 10.0,
        };
        assert!(p.compact_label_console_live().contains("SuperGrok period"));
        assert!(
            p.full_sentence_console_live()
                .contains("not live principal")
        );
    }

    #[test]
    fn never_dollarizes() {
        let p = PeriodPacing {
            usage_pct: 80.0,
            expected_pct: 50.0,
            delta_pct: 30.0,
        };
        assert!(!p.compact_label().contains('$'));
        assert!(!p.full_sentence().contains('$'));
    }
}
