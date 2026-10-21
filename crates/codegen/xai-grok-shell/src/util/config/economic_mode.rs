//! Economic mode and session sampling windows.
//!
//! Grok 4.5 doubles input / output / cache-read prices once a request exceeds
//! [`GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS`] (200K; catalog context remains
//! 500K). Nested L2/L3 catalog is that 200k cap, including when the model
//! catalog is smaller (grok-4.6-build ~32k). Nested live sampling is that same
//! 200k cap (not 40% of 200k, not 400k of a 1M catalog, not 32k or 12800 of a
//! grok-4.6-build catalog). 40% is the keep-near attention target
//! (`session_attention_target_tokens(200_000) == 80_000`), not a second live
//! cap. The main (L1) session uses the catalog window so AUTO compact does not
//! fire at the old 200k L1 knee. Economic mode still gates implement-effort
//! policy.
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
/// cliff. Nested L2/L3 catalog is this count; nested live sampling is this
/// same 200k cap. 40% is keep-near attention, not a second live cap. L1 uses
/// the catalog window.
pub const ECONOMIC_CONTEXT_CAP: u64 =
    xai_grok_compaction::GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS;

/// Nested L2/L3 catalog is this cap. Nested live sampling is this same cap,
/// not 40% of it and not 40% of a smaller model catalog.
pub const NESTED_SESSION_CONTEXT_CAP: u64 = xai_grok_compaction::NESTED_SESSION_CONTEXT_CAP;

/// Keep-near attention target as a percent of a catalog/sampling window.
/// Nested live is the nested catalog cap (200k), not this percent of it.
pub const SESSION_ATTENTION_TARGET_PERCENT: u8 =
    xai_grok_compaction::SESSION_ATTENTION_TARGET_PERCENT;

/// Nested L2/L3 catalog window. Always [`NESTED_SESSION_CONTEXT_CAP`].
pub fn nested_catalog_window(catalog: u64) -> u64 {
    xai_grok_compaction::nested_catalog_window(catalog)
}

/// Nested L2/L3 live sampling: always [`NESTED_SESSION_CONTEXT_CAP`] (200k).
/// A 32k grok-4.6-build catalog still floors to 200k live, not 12800 and not
/// 32k.
pub fn nested_live_sampling_window(nested_catalog: u64) -> u64 {
    xai_grok_compaction::nested_live_sampling_window(nested_catalog)
}

/// Stored catalog window. L1 uses the model catalog. Nested L2/L3 always store
/// [`NESTED_SESSION_CONTEXT_CAP`].
pub fn session_catalog_window(catalog: u64, is_nested: bool) -> u64 {
    xai_grok_compaction::session_catalog_window(catalog, is_nested)
}

/// Live sampling/compaction window for the session that is running.
///
/// L1 (parent TUI) uses the catalog window (500k on Grok 4.5). Nested L2/L3
/// live sampling is [`nested_live_sampling_window`].
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

    /// Nested stored catalog is always 200k. Nested live equals that catalog.
    #[test]
    fn session_catalog_window_is_catalog_on_l1_and_200k_when_nested() {
        assert_eq!(session_catalog_window(500_000, false), 500_000);
        assert_eq!(session_catalog_window(500_000, true), 200_000);
        assert_eq!(session_catalog_window(128_000, true), 200_000);
        assert_eq!(
            session_catalog_window(500_000, true),
            NESTED_SESSION_CONTEXT_CAP
        );
    }

    /// L1 uses the catalog window. Nested live sampling is always the nested
    /// catalog cap (200k), including a raw 32k or 128k catalog. Economic mode
    /// must not recap L1 to the nested budget.
    #[test]
    fn session_sampling_window_is_catalog_on_l1_and_nested_catalog_cap_when_nested() {
        assert_eq!(session_sampling_window(500_000, false), 500_000);
        assert_eq!(session_sampling_window(500_000, true), 200_000);
        assert_eq!(session_sampling_window(128_000, true), 200_000);
        assert_eq!(
            session_sampling_window(500_000, false),
            xai_grok_compaction::GROK_45_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(session_sampling_window(500_000, true), 200_000);
    }

    /// GitHub #238 / #233 / #221. Nested spawn with a 32k model catalog still
    /// gets nested catalog 200k and live 200k. Do not apply 40% to 32k.
    #[test]
    fn nested_spawn_32k_model_catalog_still_gets_200k_catalog_and_200k_live() {
        let grok_46_build_catalog = 32_000_u64;
        assert_eq!(
            nested_catalog_window(grok_46_build_catalog),
            NESTED_SESSION_CONTEXT_CAP
        );
        assert_eq!(session_catalog_window(grok_46_build_catalog, true), 200_000);
        assert_eq!(
            session_sampling_window(session_catalog_window(grok_46_build_catalog, true), true),
            200_000
        );
        assert_eq!(nested_live_sampling_window(grok_46_build_catalog), 200_000);
        assert_ne!(session_sampling_window(grok_46_build_catalog, true), 12_800);
        assert_ne!(
            session_sampling_window(session_catalog_window(grok_46_build_catalog, true), true),
            80_000,
            "must not apply 40% to the 200k nested catalog as a second live cap"
        );
    }

    /// Nested L2 and L3 live sampling is the nested catalog cap (200k).
    /// A 1M nested catalog still caps at 200k, not 400k. L1 stays on the
    /// catalog window. GitHub issue 238 / 221.
    #[test]
    fn nested_l2_and_l3_live_sampling_is_nested_catalog_cap() {
        let nested_cases = [(1_000_000_u64, 200_000_u64), (200_000, 200_000)];
        for (catalog, live) in nested_cases {
            assert_eq!(session_sampling_window(catalog, true), live);
            assert_eq!(nested_live_sampling_window(catalog), live);
        }
        assert_eq!(session_sampling_window(500_000, false), 500_000);
        assert_eq!(session_sampling_window(1_000_000, false), 1_000_000);
        let nested_live = session_sampling_window(200_000, true);
        assert_eq!(nested_live, 200_000);
        assert!(!xai_token_estimation::exceeds_threshold(
            189_999,
            nested_live,
            95
        ));
        assert!(xai_token_estimation::exceeds_threshold(
            190_000,
            nested_live,
            95
        ));
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
