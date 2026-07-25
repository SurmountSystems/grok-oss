/// Default auto-compact threshold (% of context window) when no source sets it.
///
/// Kept in lockstep with
/// [`xai_grok_compaction::DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT`].
pub const DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT: u8 =
    xai_grok_compaction::DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT;

// Re-export Grok 4.5 card reference constants for settings/docs callers.
pub use xai_grok_compaction::{
    AutoCompactThreshold, GROK_45_CONTEXT_WINDOW_TOKENS, GROK_45_DEFAULT_AUTO_COMPACT_TOKENS,
    GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompactionToolChoice {
    #[default]
    Auto,
    None,
}

impl std::str::FromStr for CompactionToolChoice {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "none" => Ok(Self::None),
            _ => Err(()),
        }
    }
}

pub(crate) const ENV_COMPACTION_TOOL_CHOICE: &str = "GROK_COMPACTION_TOOL_CHOICE";

pub(crate) fn resolve_compaction_tool_choice_from(
    env: Option<&str>,
    config: Option<&str>,
    remote: Option<&str>,
) -> CompactionToolChoice {
    env.and_then(|s| s.parse().ok())
        .or_else(|| config.and_then(|s| s.parse().ok()))
        .or_else(|| remote.and_then(|s| s.parse().ok()))
        .unwrap_or_default()
}

pub(crate) const ENV_AUTO_COMPACT_THRESHOLD_PERCENT: &str = "GROK_AUTO_COMPACT_THRESHOLD_PERCENT";

/// Precedence (highest first): env `GROK_AUTO_COMPACT_THRESHOLD_PERCENT` user TOML `[model.<id>].auto_compact_threshold_percent` (`cfg.config_models`, the merge of user and managed `[model.<id>]` sections) user TOML `[session].auto_compact_threshold_percent` remote settings per-model `ModelInfo.auto_compact_threshold_percent` (kept out of `ConfigModelOverride::apply` so the user and remote per-model tiers stay distinct) remote settings global `RemoteSettings.auto_compact_threshold_percent` default `DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT`
pub(crate) fn resolve_auto_compact_threshold_percent(
    cfg: &crate::agent::config::Config,
    model_id: &str,
    model: Option<&crate::agent::config::ModelInfo>,
) -> u8 {
    resolve_auto_compact_threshold_percent_from_tiers(
        cfg.config_models
            .get(model_id)
            .and_then(|m| m.auto_compact_threshold_percent),
        cfg.session.auto_compact_threshold_percent,
        model.and_then(|m| m.auto_compact_threshold_percent),
        cfg.remote_settings
            .as_ref()
            .and_then(|r| r.auto_compact_threshold_percent),
    )
}

/// [`resolve_auto_compact_threshold_percent`] for callers without a `Config`, e.g. subagent spawn paths that pass the parent's tiers explicitly.
/// There the per-model tier uses the subagent's resolved model id, not the parent's.
pub(crate) fn resolve_auto_compact_threshold_percent_from_tiers(
    user_per_model: Option<u8>,
    user_global: Option<u8>,
    gb_per_model: Option<u8>,
    gb_global: Option<u8>,
) -> u8 {
    fn clamp_env(raw: i64) -> Option<u8> {
        if (0..=100).contains(&raw) {
            Some(raw as u8)
        } else {
            tracing::debug!(
                source = "env",
                value = raw,
                "auto_compact_threshold_percent out of range 0..=100; ignoring"
            );
            None
        }
    }
    let from_env = || -> Option<u8> {
        std::env::var(ENV_AUTO_COMPACT_THRESHOLD_PERCENT)
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .and_then(clamp_env)
    };

    from_env()
        .or(user_per_model)
        .or(user_global)
        .or(gb_per_model)
        .or(gb_global)
        .unwrap_or(DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT)
}

/// Fleet p99 of successful compactions is ~181s (≈225s at 400K+ input).
/// So 300s clears the legit tail with margin while cutting a runaway from the ~600s deadline.
pub const DEFAULT_COMPACTION_WALL_CLOCK_BUDGET_SECS: u64 = 300;

/// Below this, a configured budget is almost certainly a misconfig (fleet success p99 ~181s); logged at `warn`, not clamped.
const COMPACTION_WALL_CLOCK_BUDGET_WARN_SECS: u64 = 120;

const ENV_COMPACTION_WALL_CLOCK_BUDGET_SECS: &str = "GROK_COMPACTION_WALL_CLOCK_SECS";

/// Precedence: env `GROK_COMPACTION_WALL_CLOCK_SECS`, then remote `RemoteSettings.compaction_wall_clock_budget_secs`, then the client default. `0` **disables** it.
/// Low values are warned, not clamped: any "safe" clamp (e.g. 30s) would itself cut legit compactions, trading one silent failure for another. Ops own the value.
pub(crate) fn resolve_compaction_wall_clock_budget_secs(gb_global: Option<u64>) -> u64 {
    let from_env = std::env::var(ENV_COMPACTION_WALL_CLOCK_BUDGET_SECS)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());
    let resolved = from_env
        .or(gb_global)
        .unwrap_or(DEFAULT_COMPACTION_WALL_CLOCK_BUDGET_SECS);
    if resolved > 0 && resolved < COMPACTION_WALL_CLOCK_BUDGET_WARN_SECS {
        tracing::warn!(
            budget_secs = resolved,
            "compaction wall-clock budget {resolved}s is below {COMPACTION_WALL_CLOCK_BUDGET_WARN_SECS}s \
             and may cut legitimate compactions (fleet success p99 ~181s); set 0 to disable"
        );
    }
    resolved
}

#[cfg(test)]
mod compaction_wall_clock_budget_tests {
    use super::resolve_compaction_wall_clock_budget_secs as resolve;

    // Assumes GROK_COMPACTION_WALL_CLOCK_SECS is unset in the test env.
    #[test]
    fn default_global_disable_and_no_clamp() {
        assert_eq!(resolve(None), 300); // client default
        assert_eq!(resolve(Some(450)), 450); // server global wins
        assert_eq!(resolve(Some(0)), 0); // 0 explicitly disables (no clamp)
        assert_eq!(resolve(Some(5)), 5); // low values pass through (warned, not clamped)
    }
}


#[cfg(test)]
mod compaction_tool_choice_tests {
    use super::{CompactionToolChoice, resolve_compaction_tool_choice_from as resolve};

    #[test]
    fn default_is_auto() {
        assert_eq!(resolve(None, None, None), CompactionToolChoice::Auto);
    }

    #[test]
    fn precedence_env_over_config_over_remote() {
        assert_eq!(
            resolve(Some("none"), Some("auto"), Some("auto")),
            CompactionToolChoice::None
        );
        assert_eq!(
            resolve(None, Some("none"), Some("auto")),
            CompactionToolChoice::None
        );
        assert_eq!(
            resolve(None, None, Some("none")),
            CompactionToolChoice::None
        );
    }

    #[test]
    fn garbage_falls_through() {
        assert_eq!(
            resolve(Some("garbage"), None, Some("none")),
            CompactionToolChoice::None
        );
        assert_eq!(
            resolve(Some("garbage"), Some("also-bad"), None),
            CompactionToolChoice::Auto
        );
    }

    #[test]
    fn from_str_case_insensitive() {
        assert_eq!("AUTO".parse(), Ok(CompactionToolChoice::Auto));
        assert_eq!(" None ".parse(), Ok(CompactionToolChoice::None));
        assert!("required".parse::<CompactionToolChoice>().is_err());
    }
}

#[cfg(test)]
mod resolve_auto_compact_threshold_dual_mode_tests {
    use super::*;
    use crate::agent::config::{Config, SessionConfig};

    fn bare_cfg() -> Config {
        Config::default()
    }

    /// Product contract: when nothing is configured, preference is 95%.
    #[test]
    fn default_is_95_percent() {
        let cfg = bare_cfg();
        assert_eq!(DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT, 95);
        assert_eq!(
            resolve_auto_compact_threshold(&cfg, "any", None),
            AutoCompactThreshold::Percent(95)
        );
        assert_eq!(GROK_45_DEFAULT_AUTO_COMPACT_TOKENS, 475_000);
        assert_eq!(GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS, 200_000);
    }

    #[test]
    fn session_tokens_win_over_session_percent() {
        let mut cfg = bare_cfg();
        cfg.session = SessionConfig {
            auto_compact_threshold_percent: Some(98),
            auto_compact_threshold_tokens: Some(GROK_45_LONG_CONTEXT_PRICE_THRESHOLD_TOKENS),
            load_envrc: None,
        };
        assert_eq!(
            resolve_auto_compact_threshold(&cfg, "any", None),
            AutoCompactThreshold::Tokens(200_000)
        );
    }

    #[test]
    fn session_percent_when_tokens_unset() {
        let mut cfg = bare_cfg();
        cfg.session = SessionConfig {
            auto_compact_threshold_percent: Some(90),
            auto_compact_threshold_tokens: None,
            load_envrc: None,
        };
        assert_eq!(
            resolve_auto_compact_threshold(&cfg, "any", None),
            AutoCompactThreshold::Percent(90)
        );
    }
}
