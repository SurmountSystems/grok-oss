//! grok-build compaction configuration.
//!
//! Holds the [`FullReplaceConfig`] tunables struct (mirroring
//! [`IntraCompactionConfig`](crate::intra_compaction::IntraCompactionConfig) /
//! [`InterCompactionConfig`](crate::inter_compaction::InterCompactionConfig),
//! which also live in their module's `config.rs`) plus the shared default
//! values. Trigger *wiring* (pre-sampling checks, preflight overflow,
//! model-switch, suppression) stays per-host.

/// Default auto-compact threshold (% of context window) when no other source
/// (env var, user config, remote per-model/global flags) sets it. Shared by
/// grok-build and Grok chat (~95% trigger on both sides).
pub const DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT: u8 = 95;

// ---------------------------------------------------------------------------
// Grok 4.5 model-card reference (docs.x.ai, mid-2026).
//
// Used as the token-count preset baseline in settings and docs. The live gate
// still uses each session's actual `context_window`; these constants only
// label the presets and the long-context price cliff.
// ---------------------------------------------------------------------------

/// Grok 4.5 maximum context window (tokens).
pub const GROK_45_CONTEXT_WINDOW_TOKENS: u64 = 500_000;

/// Prompt length (tokens) above which Grok 4.5 bills the **entire** request at
/// long-context rates (2× input / cached-input / output). Staying at or below
/// this cliff keeps short-context pricing.
pub const GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS: u64 = 200_000;

/// Nested L2/L3 catalog window is this count, even when the model catalog is
/// larger (500k on Grok 4.5) or smaller (grok-4.6-build ~32k). Same token
/// count as the long-context price cliff. Nested live equals this cap; 40% is
/// the keep-near attention target, not a second live cap. The main (L1)
/// session does not use this.
pub const NESTED_SESSION_CONTEXT_CAP: u64 = GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS;

/// Keep-near attention target as a percent of a catalog/sampling window.
/// Nested live is the nested catalog cap (200k), not 40% of it.
/// (`session_attention_target_tokens(200_000) == 80_000` stays keep-near.)
/// L1 live stays the catalog window (500k today).
pub const SESSION_ATTENTION_TARGET_PERCENT: u8 = 40;

/// 40% of `sampling_window`. One place for that percent.
pub fn session_attention_target_tokens(sampling_window: u64) -> u64 {
    sampling_window.saturating_mul(u64::from(SESSION_ATTENTION_TARGET_PERCENT)) / 100
}

/// Nested L2/L3 catalog window. Always [`NESTED_SESSION_CONTEXT_CAP`] (200k).
///
/// Nested spawn must not store a smaller model catalog (grok-4.6-build ~32k)
/// as live 32k or 40% of 32k (12800). GitHub #233 / #221.
pub fn nested_catalog_window(_catalog: u64) -> u64 {
    NESTED_SESSION_CONTEXT_CAP
}

/// Nested L2/L3 live sampling window.
///
/// Always [`NESTED_SESSION_CONTEXT_CAP`] (200k). Nested live is the nested
/// catalog cap, not 40% of it. A 32k grok-4.6-build catalog still floors to
/// 200k live, not 12800 and not 32k.
pub fn nested_live_sampling_window(_nested_catalog: u64) -> u64 {
    NESTED_SESSION_CONTEXT_CAP
}

/// Stored catalog window for the session that is running.
///
/// L1 uses the model catalog. Nested L2/L3 always store
/// [`NESTED_SESSION_CONTEXT_CAP`]. Live sampling is
/// [`session_sampling_window`].
pub fn session_catalog_window(catalog: u64, is_nested: bool) -> u64 {
    if is_nested {
        nested_catalog_window(catalog)
    } else {
        catalog
    }
}

/// Live sampling/compaction window for the session that is running.
///
/// L1 uses the catalog window. Nested L2/L3 live sampling is
/// [`nested_live_sampling_window`]: always the 200k nested catalog cap.
/// Ordinary L2 still AUTO compact at 95% of that live window (190k).
/// L3 never compact-and-continue.
pub fn session_sampling_window(catalog: u64, is_nested: bool) -> u64 {
    if is_nested {
        nested_live_sampling_window(catalog)
    } else {
        catalog
    }
}

/// 95% of [`GROK_45_CONTEXT_WINDOW_TOKENS`], the token equivalent of the
/// default percent threshold on the Grok 4.5 card.
pub const GROK_45_DEFAULT_AUTO_COMPACT_TOKENS: u64 =
    GROK_45_CONTEXT_WINDOW_TOKENS * (DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT as u64) / 100;

/// How the user expresses the auto-compact trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoCompactThreshold {
    /// Fraction of the active model's context window (0–100).
    Percent(u8),
    /// Absolute token count (independent of window size).
    Tokens(u64),
}

impl AutoCompactThreshold {
    /// Built-in default: 95% of the context window.
    pub const fn default_percent() -> Self {
        Self::Percent(DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT)
    }

    /// Absolute token count at which compaction should fire for `context_window`.
    ///
    /// Percent mode scales with the window; tokens mode uses the configured
    /// count (clamped to the window so a 200k preset still fires on a smaller
    /// model before overflow).
    pub fn absolute_tokens(self, context_window: u64) -> u64 {
        match self {
            Self::Percent(p) => {
                if context_window == 0 {
                    0
                } else {
                    context_window.saturating_mul(u64::from(p.min(100))) / 100
                }
            }
            Self::Tokens(t) => {
                if context_window == 0 {
                    t
                } else {
                    t.min(context_window)
                }
            }
        }
    }

    /// Effective percent of `context_window` (for UIs that only show %).
    pub fn as_percent_of(self, context_window: u64) -> u8 {
        match self {
            Self::Percent(p) => p.min(100),
            Self::Tokens(t) => t
                .saturating_mul(100)
                .checked_div(context_window)
                .map(|p| p.min(100) as u8)
                .unwrap_or(0),
        }
    }
}

impl Default for AutoCompactThreshold {
    fn default() -> Self {
        Self::default_percent()
    }
}

#[cfg(test)]
mod auto_compact_threshold_tests {
    use super::*;

    /// Product contract: the built-in default is 95% of the context window.
    #[test]
    fn default_auto_compact_threshold_is_95_percent() {
        assert_eq!(DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT, 95);
        assert_eq!(
            AutoCompactThreshold::default(),
            AutoCompactThreshold::Percent(95)
        );
        assert_eq!(
            AutoCompactThreshold::default_percent(),
            AutoCompactThreshold::Percent(95)
        );
        // Grok 4.5 card: 95% of 500k = 475k tokens.
        assert_eq!(GROK_45_CONTEXT_WINDOW_TOKENS, 500_000);
        assert_eq!(GROK_45_DEFAULT_AUTO_COMPACT_TOKENS, 475_000);
        assert_eq!(
            AutoCompactThreshold::Percent(95).absolute_tokens(GROK_45_CONTEXT_WINDOW_TOKENS),
            GROK_45_DEFAULT_AUTO_COMPACT_TOKENS
        );
    }

    #[test]
    fn tokens_mode_clamps_to_window_and_preserves_price_cliff() {
        assert_eq!(GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS, 200_000);
        let cliff = AutoCompactThreshold::Tokens(GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS);
        // On Grok 4.5, 200k is 40% of the 500k window.
        assert_eq!(cliff.as_percent_of(GROK_45_CONTEXT_WINDOW_TOKENS), 40);
        assert_eq!(
            cliff.absolute_tokens(GROK_45_CONTEXT_WINDOW_TOKENS),
            200_000
        );
        // On a smaller window the absolute preset still fires before overflow.
        assert_eq!(cliff.absolute_tokens(128_000), 128_000);
    }

    #[test]
    fn percent_mode_scales_with_window() {
        let t = AutoCompactThreshold::Percent(90);
        assert_eq!(t.absolute_tokens(100_000), 90_000);
        assert_eq!(t.absolute_tokens(500_000), 450_000);
        assert_eq!(t.as_percent_of(500_000), 90);
    }

    /// Nested stored catalog is always 200k. Nested live equals that catalog.
    #[test]
    fn session_catalog_window_is_catalog_on_l1_and_200k_when_nested() {
        assert_eq!(session_catalog_window(500_000, false), 500_000);
        assert_eq!(session_catalog_window(500_000, true), 200_000);
        assert_eq!(session_catalog_window(128_000, true), 200_000);
        assert_eq!(nested_catalog_window(1_000_000), NESTED_SESSION_CONTEXT_CAP);
        assert_eq!(
            session_catalog_window(GROK_45_CONTEXT_WINDOW_TOKENS, false),
            GROK_45_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            session_catalog_window(GROK_45_CONTEXT_WINDOW_TOKENS, true),
            NESTED_SESSION_CONTEXT_CAP
        );
    }

    /// L1 live sampling is the catalog window. Nested live is always the
    /// nested catalog cap (200k), including a raw 32k or 128k catalog.
    #[test]
    fn session_sampling_window_is_catalog_on_l1_and_nested_catalog_cap_when_nested() {
        assert_eq!(session_sampling_window(500_000, false), 500_000);
        assert_eq!(session_sampling_window(500_000, true), 200_000);
        assert_eq!(session_sampling_window(128_000, true), 200_000);
        assert_eq!(
            session_sampling_window(GROK_45_CONTEXT_WINDOW_TOKENS, false),
            GROK_45_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            session_sampling_window(GROK_45_CONTEXT_WINDOW_TOKENS, true),
            200_000
        );
    }

    /// GitHub #233 / #221. grok-4.6-build catalog ~32k must not become nested
    /// live 12800 (40% of 32k). Nested spawn seeds catalog 200k so live is 200k.
    #[test]
    fn nested_spawn_32k_model_catalog_still_gets_200k_catalog_and_200k_live() {
        let grok_46_build_catalog = 32_000_u64;
        assert_eq!(
            nested_catalog_window(grok_46_build_catalog),
            NESTED_SESSION_CONTEXT_CAP,
            "nested catalog floors at 200k even when the model catalog is 32k"
        );
        assert_eq!(session_catalog_window(grok_46_build_catalog, true), 200_000);
        let seeded = session_catalog_window(grok_46_build_catalog, true);
        assert_eq!(
            session_sampling_window(seeded, true),
            200_000,
            "nested live after seeding the 200k catalog is 200k, not 40% of 200k"
        );
        assert_eq!(
            session_sampling_window(grok_46_build_catalog, true),
            200_000,
            "a raw 32k catalog still floors to nested live 200k"
        );
        assert_ne!(
            session_sampling_window(grok_46_build_catalog, true),
            12_800,
            "must not apply 40% to a 32k catalog"
        );
        assert_ne!(
            session_sampling_window(seeded, true),
            80_000,
            "must not apply 40% to the 200k nested catalog as a second live cap"
        );
    }

    /// Nested live is always the 200k nested catalog cap. Do not apply 40%
    /// to a 32k catalog (12800) and do not leave 32k as live. GitHub #233.
    #[test]
    fn nested_live_does_not_take_forty_percent_of_a_catalog_already_under_200k() {
        assert_eq!(nested_live_sampling_window(32_000), 200_000);
        assert_eq!(session_sampling_window(32_000, true), 200_000);
        assert_eq!(nested_live_sampling_window(12_800), 200_000);
        assert_eq!(nested_live_sampling_window(128_000), 200_000);
        assert_eq!(nested_live_sampling_window(199_999), 200_000);
        assert_eq!(nested_live_sampling_window(200_000), 200_000);
        assert_eq!(nested_live_sampling_window(1_000_000), 200_000);
        assert_ne!(
            nested_live_sampling_window(32_000),
            12_800,
            "must not apply 40% to a 32k catalog"
        );
        assert_eq!(
            session_sampling_window(32_000, false),
            32_000,
            "L1 must not take the nested live path"
        );
    }

    /// Nested L2 and L3 live sampling is the nested catalog cap (200k).
    /// A 1M nested catalog still caps at 200k, not 400k. L1 stays on the
    /// catalog window. GitHub issue 221.
    #[test]
    fn nested_l2_and_l3_live_sampling_is_nested_catalog_cap() {
        // Nested catalog 1M → 200k live. Nested catalog 200k → 200k live.
        // is_nested=true is both L2 and L3 (same helper; L3 still must not
        // compact-and-continue).
        let nested_cases = [(1_000_000_u64, 200_000_u64), (200_000, 200_000)];
        for (catalog, live) in nested_cases {
            assert_eq!(
                session_sampling_window(catalog, true),
                live,
                "nested L2/L3 live sampling must be the 200k nested catalog cap, not 40% of {catalog}"
            );
            assert_eq!(nested_live_sampling_window(catalog), live);
        }
        assert_eq!(
            session_sampling_window(500_000, false),
            500_000,
            "L1 live sampling stays the 500k catalog window"
        );
        assert_eq!(
            session_sampling_window(1_000_000, false),
            1_000_000,
            "L1 must not take the nested live path"
        );
        let nested_live = session_sampling_window(200_000, true);
        assert_eq!(nested_live, 200_000);
        assert_eq!(
            AutoCompactThreshold::Percent(95).absolute_tokens(nested_live),
            190_000,
            "ordinary L2 must still AUTO compact at 95% of nested live sampling"
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
        assert_eq!(
            AutoCompactThreshold::Percent(95).absolute_tokens(window),
            475_000
        );
        assert!(
            AutoCompactThreshold::Percent(95).absolute_tokens(window) > 200_000,
            "L1 auto-compact must not fire at the old 200k nested cap"
        );
        let nested = session_sampling_window(500_000, true);
        assert_eq!(nested, 200_000);
        assert_eq!(
            AutoCompactThreshold::Percent(95).absolute_tokens(nested),
            190_000
        );
    }
}

/// Minimum character count for a cleaned summary seed.
///
/// grok-build retries when the cleaned summary is shorter than this — the
/// smallest healthy prod summary observed was ~3,242 chars; anything under
/// 500 is treated as degenerate and retried like a transient failure.
pub const MIN_SUMMARY_SEED_CHARS: usize = 500;

/// Tunables for the full-replace pass.
#[derive(Debug, Clone)]
pub struct FullReplaceConfig {
    /// Total LLM attempts (first try + retries) on transient failures.
    pub max_attempts: u32,
    /// Delay between transient retries.
    pub retry_delay_secs: u64,
    /// End-to-end timeout for each compaction LLM call.
    pub sampling_timeout_secs: u64,
}

impl Default for FullReplaceConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            retry_delay_secs: 3,
            sampling_timeout_secs: 120,
        }
    }
}
