//! Economic mode and session sampling windows.
//!
//! Grok 4.5 doubles input / output / cache-read prices once a request exceeds
//! [`GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS`] (200K; catalog context remains
//! 500K). Nested L2/L3 sessions use that 200k count as their sampling window.
//! The main (L1) session uses the catalog window so AUTO compact does not fire
//! at the old 200k L1 knee. Economic mode still gates implement-effort policy.
//!
//! **Implement-loop effort** (skill 1–5 thoroughness, not model reasoning
//! effort, and not how many Review rows to launch) is a separate Token
//! Economy policy under `[token_economy]`: optional lock and min floor always
//! apply when set; when economic mode is on and
//! `cap_implement_effort_when_economic` is true, the product also applies a hard
//! ceiling (default 3) and desired inject when missing (default 2). See
//! [`crate::token_economy`].
//!
//! Default: **on** (`None` in `[ui].economic_mode`). Override globally via
//! settings / `config.toml`, or per conversation with `/economic-mode`.

use toml::Value as TomlValue;

/// Pricing-tier soft cap (tokens). Same value as
/// [`xai_grok_compaction::GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS`] and the
/// settings modal's `200k` auto-compact preset — one source of truth for the
/// cliff. Nested L2/L3 sampling uses this count; L1 uses the catalog window.
pub const ECONOMIC_CONTEXT_CAP: u64 =
    xai_grok_compaction::GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS;

/// Nested L2/L3 sessions never exceed this sampling window.
pub const NESTED_SESSION_CONTEXT_CAP: u64 = xai_grok_compaction::NESTED_SESSION_CONTEXT_CAP;

/// Keep-near attention target as a percent of this session's sampling window.
pub const SESSION_ATTENTION_TARGET_PERCENT: u8 =
    xai_grok_compaction::SESSION_ATTENTION_TARGET_PERCENT;

/// Sampling/compaction window for the session that is running.
///
/// L1 (parent TUI) uses the catalog window (500k on Grok 4.5). Nested L2/L3
/// never exceed [`NESTED_SESSION_CONTEXT_CAP`].
pub fn session_sampling_window(catalog: u64, is_nested: bool) -> u64 {
    xai_grok_compaction::session_sampling_window(catalog, is_nested)
}

/// 40% of `sampling_window` (attention target for the running session).
pub fn session_attention_target_tokens(sampling_window: u64) -> u64 {
    xai_grok_compaction::session_attention_target_tokens(sampling_window)
}

/// Client default when `[ui].economic_mode` is unset.
pub const ECONOMIC_MODE_DEFAULT: bool = true;

/// Resolve economic mode from an optional config value (`None` → default on).
pub fn resolve_economic_mode(user: Option<bool>) -> bool {
    user.unwrap_or(ECONOMIC_MODE_DEFAULT)
}

/// Cap `context_window` at [`ECONOMIC_CONTEXT_CAP`] when economic mode is on.
pub fn apply_economic_context_cap(context_window: u64, economic_mode: bool) -> u64 {
    if economic_mode {
        context_window.min(ECONOMIC_CONTEXT_CAP)
    } else {
        context_window
    }
}

/// Read `[ui].economic_mode` from disk-merged config. Default on when unset.
pub fn economic_mode_from_disk() -> bool {
    let root = match crate::config::load_effective_config() {
        Ok(v) => v,
        Err(_) => return ECONOMIC_MODE_DEFAULT,
    };
    economic_mode_from_toml(&root)
}

/// Parse `[ui].economic_mode` from a TOML root. Default on when missing/invalid.
pub fn economic_mode_from_toml(root: &TomlValue) -> bool {
    match root
        .get("ui")
        .and_then(|u| u.get("economic_mode"))
        .and_then(|v| v.as_bool())
    {
        Some(b) => b,
        None => ECONOMIC_MODE_DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_defaults_on() {
        assert!(resolve_economic_mode(None));
        assert!(resolve_economic_mode(Some(true)));
        assert!(!resolve_economic_mode(Some(false)));
    }

    #[test]
    fn apply_cap_when_on_clamps_above_threshold() {
        assert_eq!(
            apply_economic_context_cap(500_000, true),
            ECONOMIC_CONTEXT_CAP
        );
        assert_eq!(
            apply_economic_context_cap(200_000, true),
            ECONOMIC_CONTEXT_CAP
        );
        assert_eq!(apply_economic_context_cap(100_000, true), 100_000);
    }

    #[test]
    fn apply_cap_when_off_is_identity() {
        assert_eq!(apply_economic_context_cap(500_000, false), 500_000);
        assert_eq!(apply_economic_context_cap(1, false), 1);
    }

    #[test]
    fn from_toml_defaults_and_reads_bool() {
        let empty: TomlValue = toml::from_str("").unwrap();
        assert!(economic_mode_from_toml(&empty));

        let on: TomlValue = toml::from_str("[ui]\neconomic_mode = true\n").unwrap();
        assert!(economic_mode_from_toml(&on));

        let off: TomlValue = toml::from_str("[ui]\neconomic_mode = false\n").unwrap();
        assert!(!economic_mode_from_toml(&off));
    }

    #[test]
    fn economic_cap_matches_grok_45_price_cliff_and_auto_compact_200k_preset() {
        assert_eq!(
            ECONOMIC_CONTEXT_CAP,
            xai_grok_compaction::GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS
        );
        assert_eq!(ECONOMIC_CONTEXT_CAP, 200_000);
        // Absolute auto-compact at the cliff is a no-op once economic mode
        // already soft-caps the window to the same value.
        let cliff = xai_grok_compaction::AutoCompactThreshold::Tokens(ECONOMIC_CONTEXT_CAP);
        assert_eq!(
            cliff.absolute_tokens(ECONOMIC_CONTEXT_CAP),
            ECONOMIC_CONTEXT_CAP
        );
        assert_eq!(
            cliff.absolute_tokens(xai_grok_compaction::GROK_45_CONTEXT_WINDOW_TOKENS),
            ECONOMIC_CONTEXT_CAP
        );
    }

    /// L1 uses the catalog window. Nested L2/L3 never exceed 200k even when
    /// the catalog is 500k. Economic mode must not recap L1 to the nested budget.
    #[test]
    fn session_sampling_window_is_catalog_on_l1_and_200k_when_nested() {
        assert_eq!(session_sampling_window(500_000, false), 500_000);
        assert_eq!(session_sampling_window(500_000, true), 200_000);
        assert_eq!(session_sampling_window(128_000, true), 128_000);
        assert_eq!(
            session_sampling_window(500_000, false),
            xai_grok_compaction::GROK_45_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            session_sampling_window(500_000, true),
            NESTED_SESSION_CONTEXT_CAP
        );
    }

    #[test]
    fn attention_target_is_forty_percent_of_the_running_session_window() {
        assert_eq!(SESSION_ATTENTION_TARGET_PERCENT, 40);
        assert_eq!(session_attention_target_tokens(500_000), 200_000);
        assert_eq!(session_attention_target_tokens(200_000), 80_000);
    }

    /// AUTO compact on L1 is 95% of 500k (475k), not the old 200k nested cap.
    #[test]
    fn main_session_auto_compact_knee_is_not_the_old_200k_l1_cap() {
        let window = session_sampling_window(500_000, false);
        assert_eq!(window, 500_000);
        assert!(!xai_token_estimation::exceeds_threshold(
            200_000, window, 95
        ));
        assert!(!xai_token_estimation::exceeds_threshold(
            190_000, window, 95
        ));
        assert!(!xai_token_estimation::exceeds_threshold(
            474_999, window, 95
        ));
        assert!(xai_token_estimation::exceeds_threshold(475_000, window, 95));
        let nested = session_sampling_window(500_000, true);
        assert_eq!(nested, 200_000);
        assert!(!xai_token_estimation::exceeds_threshold(
            189_999, nested, 95
        ));
        assert!(xai_token_estimation::exceeds_threshold(190_000, nested, 95));
    }
}
