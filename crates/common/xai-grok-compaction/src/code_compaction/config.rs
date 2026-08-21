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

/// Nested L2/L3 sessions never exceed this sampling/compaction window, even
/// when the model catalog is larger (500k on Grok 4.5). Same token count as
/// the long-context price cliff. The main (L1) session does not use this cap.
pub const NESTED_SESSION_CONTEXT_CAP: u64 = GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS;

/// Keep-near attention target as a percent of this session's sampling window.
/// Main session: 40% of 500k = 200k. Nested: 40% of 200k = 80k.
pub const SESSION_ATTENTION_TARGET_PERCENT: u8 = 40;

/// Sampling/compaction window for the session that is running.
///
/// L1 (parent TUI) uses the catalog window. Nested L2/L3 never exceed
/// [`NESTED_SESSION_CONTEXT_CAP`].
pub fn session_sampling_window(catalog: u64, is_nested: bool) -> u64 {
    if is_nested {
        catalog.min(NESTED_SESSION_CONTEXT_CAP)
    } else {
        catalog
    }
}

/// 40% of `sampling_window` (attention target for the running session).
pub fn session_attention_target_tokens(sampling_window: u64) -> u64 {
    sampling_window.saturating_mul(u64::from(SESSION_ATTENTION_TARGET_PERCENT)) / 100
}

/// 95% of [`GROK_45_CONTEXT_WINDOW_TOKENS`] — the token equivalent of the
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

    /// L1 uses the catalog window. Nested L2/L3 never exceed 200k even when
    /// the catalog is 500k.
    #[test]
    fn session_sampling_window_is_catalog_on_l1_and_200k_when_nested() {
        assert_eq!(session_sampling_window(500_000, false), 500_000);
        assert_eq!(session_sampling_window(500_000, true), 200_000);
        assert_eq!(session_sampling_window(128_000, true), 128_000);
        assert_eq!(
            session_sampling_window(GROK_45_CONTEXT_WINDOW_TOKENS, false),
            GROK_45_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            session_sampling_window(GROK_45_CONTEXT_WINDOW_TOKENS, true),
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
