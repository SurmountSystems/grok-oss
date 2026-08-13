//! `[token_economy]` config: implement-effort caps, pacing, ledger, store path.
//!
//! Economic **context** soft-cap stays under `[ui] economic_mode`. This table
//! owns Token Economy knobs only.
//!
//! Implement-loop effort policy order (see `implement_effort`):
//! 1. `lock_implement_effort` when set (ignores prompt and desired)
//! 2. else missing → inject `desired` when economic caps are active; present stays
//! 3. floor `min_implement_effort` (always when set above 1 or present below min)
//! 4. ceiling `max_implement_effort` when economic mode + cap master

use std::path::PathBuf;

use toml::Value as TomlValue;

/// Default hard ceiling for implement-loop effort when economic mode caps it.
pub const DEFAULT_MAX_IMPLEMENT_EFFORT: u8 = 3;

/// Default desired implement-loop effort when the product would omit `--effort`.
pub const DEFAULT_DESIRED_IMPLEMENT_EFFORT: u8 = 2;

/// Default floor for implement-loop effort (1 = no extra floor beyond scale min).
pub const DEFAULT_MIN_IMPLEMENT_EFFORT: u8 = 1;

/// Minimum implement-loop effort (1–5 scale).
pub const MIN_IMPLEMENT_EFFORT: u8 = 1;

/// Maximum implement-loop effort (1–5 scale).
pub const MAX_IMPLEMENT_EFFORT: u8 = 5;

/// Validated Token Economy settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenEconomyConfig {
    /// When true and `[ui] economic_mode` is on, apply implement-effort ceiling
    /// and desired inject when missing.
    pub cap_implement_effort_when_economic: bool,
    /// Hard ceiling (1–5). Default 3. Applied only when economic caps are active.
    pub max_implement_effort: u8,
    /// Floor (1–5). Default 1. Always applied (not economic-only).
    pub min_implement_effort: u8,
    /// When `Some(1–5)`, always force this effort (ignores prompt and desired).
    /// `None` = unlocked. Still subject to economic ceiling at runtime if the
    /// live max is lower; config validation requires `min ≤ lock ≤ max`.
    pub lock_implement_effort: Option<u8>,
    /// Injected when effort is missing under economic caps (1–5, must be ≤ max).
    /// Default 2.
    pub desired_implement_effort: u8,
    /// Show free SuperGrok period linear-burn pacing in chrome.
    pub show_period_pacing: bool,
    /// Write local spend ledger rows into `grok_oss.db`.
    pub local_spend_ledger: bool,
    /// Store Management samples and show remote book on reconcile.
    pub reconcile_management_usage: bool,
    /// Override path for `grok_oss.db`. Empty / unset → `$GROK_HOME/grok_oss.db`.
    pub grok_oss_database_path: Option<PathBuf>,
}

impl Default for TokenEconomyConfig {
    fn default() -> Self {
        Self {
            cap_implement_effort_when_economic: true,
            max_implement_effort: DEFAULT_MAX_IMPLEMENT_EFFORT,
            min_implement_effort: DEFAULT_MIN_IMPLEMENT_EFFORT,
            lock_implement_effort: None,
            desired_implement_effort: DEFAULT_DESIRED_IMPLEMENT_EFFORT,
            show_period_pacing: true,
            local_spend_ledger: true,
            reconcile_management_usage: true,
            grok_oss_database_path: None,
        }
    }
}

/// Why `[token_economy]` failed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenEconomyConfigError {
    /// Effort out of 1–5.
    EffortOutOfRange { field: &'static str, value: i64 },
    /// `desired_implement_effort` greater than `max_implement_effort`.
    DesiredAboveMax { desired: u8, max: u8 },
    /// `min_implement_effort` greater than `max_implement_effort`.
    MinAboveMax { min: u8, max: u8 },
    /// `lock_implement_effort` outside `[min, max]`.
    LockOutOfBounds { lock: u8, min: u8, max: u8 },
}

impl std::fmt::Display for TokenEconomyConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EffortOutOfRange { field, value } => write!(
                f,
                "[token_economy].{field} must be an integer from {MIN_IMPLEMENT_EFFORT} to {MAX_IMPLEMENT_EFFORT} (got {value})"
            ),
            Self::DesiredAboveMax { desired, max } => write!(
                f,
                "[token_economy].desired_implement_effort ({desired}) must be ≤ max_implement_effort ({max})"
            ),
            Self::MinAboveMax { min, max } => write!(
                f,
                "[token_economy].min_implement_effort ({min}) must be ≤ max_implement_effort ({max})"
            ),
            Self::LockOutOfBounds { lock, min, max } => write!(
                f,
                "[token_economy].lock_implement_effort ({lock}) must satisfy min_implement_effort ({min}) ≤ lock ≤ max_implement_effort ({max})"
            ),
        }
    }
}

impl std::error::Error for TokenEconomyConfigError {}

/// Parse and validate `[token_economy]` from a TOML root. Missing table → defaults.
pub fn token_economy_from_toml(
    root: &TomlValue,
) -> Result<TokenEconomyConfig, TokenEconomyConfigError> {
    let Some(table) = root.get("token_economy") else {
        return Ok(TokenEconomyConfig::default());
    };

    let mut cfg = TokenEconomyConfig::default();

    if let Some(b) = table
        .get("cap_implement_effort_when_economic")
        .and_then(|v| v.as_bool())
    {
        cfg.cap_implement_effort_when_economic = b;
    }
    if let Some(b) = table.get("show_period_pacing").and_then(|v| v.as_bool()) {
        cfg.show_period_pacing = b;
    }
    if let Some(b) = table.get("local_spend_ledger").and_then(|v| v.as_bool()) {
        cfg.local_spend_ledger = b;
    }
    if let Some(b) = table
        .get("reconcile_management_usage")
        .and_then(|v| v.as_bool())
    {
        cfg.reconcile_management_usage = b;
    }
    if let Some(s) = table.get("grok_oss_database_path").and_then(|v| v.as_str()) {
        let t = s.trim();
        if !t.is_empty() {
            cfg.grok_oss_database_path = Some(PathBuf::from(t));
        }
    }

    if let Some(v) = table.get("max_implement_effort") {
        cfg.max_implement_effort = parse_effort(v, "max_implement_effort")?;
    }
    if let Some(v) = table.get("min_implement_effort") {
        cfg.min_implement_effort = parse_effort(v, "min_implement_effort")?;
    }
    if let Some(v) = table.get("desired_implement_effort") {
        cfg.desired_implement_effort = parse_effort(v, "desired_implement_effort")?;
    }
    if let Some(v) = table.get("lock_implement_effort") {
        // 0 / false-ish null via integer 0 → unlocked; 1–5 → locked.
        cfg.lock_implement_effort = parse_lock_effort(v)?;
    }

    if cfg.min_implement_effort > cfg.max_implement_effort {
        return Err(TokenEconomyConfigError::MinAboveMax {
            min: cfg.min_implement_effort,
            max: cfg.max_implement_effort,
        });
    }
    if cfg.desired_implement_effort > cfg.max_implement_effort {
        return Err(TokenEconomyConfigError::DesiredAboveMax {
            desired: cfg.desired_implement_effort,
            max: cfg.max_implement_effort,
        });
    }
    if let Some(lock) = cfg.lock_implement_effort {
        if lock < cfg.min_implement_effort || lock > cfg.max_implement_effort {
            return Err(TokenEconomyConfigError::LockOutOfBounds {
                lock,
                min: cfg.min_implement_effort,
                max: cfg.max_implement_effort,
            });
        }
    }

    Ok(cfg)
}

fn parse_effort(v: &TomlValue, field: &'static str) -> Result<u8, TokenEconomyConfigError> {
    let n = v
        .as_integer()
        .ok_or(TokenEconomyConfigError::EffortOutOfRange { field, value: -1 })?;
    if n < i64::from(MIN_IMPLEMENT_EFFORT) || n > i64::from(MAX_IMPLEMENT_EFFORT) {
        return Err(TokenEconomyConfigError::EffortOutOfRange { field, value: n });
    }
    Ok(n as u8)
}

/// Parse `lock_implement_effort`: missing handled by caller; `0` → unlocked;
/// `1–5` → locked; other integers → out of range.
fn parse_lock_effort(v: &TomlValue) -> Result<Option<u8>, TokenEconomyConfigError> {
    let n = v
        .as_integer()
        .ok_or(TokenEconomyConfigError::EffortOutOfRange {
            field: "lock_implement_effort",
            value: -1,
        })?;
    if n == 0 {
        return Ok(None);
    }
    if n < i64::from(MIN_IMPLEMENT_EFFORT) || n > i64::from(MAX_IMPLEMENT_EFFORT) {
        return Err(TokenEconomyConfigError::EffortOutOfRange {
            field: "lock_implement_effort",
            value: n,
        });
    }
    Ok(Some(n as u8))
}

/// Process-wide live Token Economy config.
///
/// Seeded from disk on first [`token_economy_from_disk`] call. Settings
/// dispatch updates this optimistically (same pattern as pager appearance
/// caches) so `current_value_for` / implement-effort policy see the new
/// value before async persist finishes. Tests pin product defaults via
/// [`reset_token_economy_live_to_defaults`] so a developer `$GROK_HOME`
/// cannot break settings registry e2e.
fn live_token_economy_slot() -> &'static std::sync::Mutex<Option<TokenEconomyConfig>> {
    static LIVE: std::sync::OnceLock<std::sync::Mutex<Option<TokenEconomyConfig>>> =
        std::sync::OnceLock::new();
    LIVE.get_or_init(|| std::sync::Mutex::new(None))
}

fn load_token_economy_from_disk_uncached() -> TokenEconomyConfig {
    let root = match crate::config::load_effective_config() {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, "token_economy: load config failed; defaults");
            return TokenEconomyConfig::default();
        }
    };
    match token_economy_from_toml(&root) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(error = %e, "token_economy: invalid [token_economy]; using defaults");
            TokenEconomyConfig::default()
        }
    }
}

/// Live Token Economy config (seeded from disk-merged effective config).
///
/// On parse/load failure or invalid table, logs and returns defaults
/// (fail-open for live TUI; validation errors are loud in unit tests via
/// [`token_economy_from_toml`]). After the first call, returns the process
/// live copy until [`set_token_economy_live`] / field setters update it or
/// [`clear_token_economy_live`] drops the cache.
pub fn token_economy_from_disk() -> TokenEconomyConfig {
    let mut guard = live_token_economy_slot()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(cfg) = guard.as_ref() {
        return cfg.clone();
    }
    let cfg = load_token_economy_from_disk_uncached();
    *guard = Some(cfg.clone());
    cfg
}

/// Replace the process-wide live Token Economy config.
pub fn set_token_economy_live(cfg: TokenEconomyConfig) {
    let mut guard = live_token_economy_slot()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    *guard = Some(cfg);
}

/// Pin live Token Economy to product defaults (hermetic settings tests).
pub fn reset_token_economy_live_to_defaults() {
    set_token_economy_live(TokenEconomyConfig::default());
}

/// Drop the live cache so the next [`token_economy_from_disk`] reloads disk.
pub fn clear_token_economy_live() {
    let mut guard = live_token_economy_slot()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    *guard = None;
}

/// Optimistic live update for a Token Economy boolean field.
pub fn set_token_economy_live_bool(field: &str, value: bool) {
    let mut cfg = token_economy_from_disk();
    match field {
        "cap_implement_effort_when_economic" => cfg.cap_implement_effort_when_economic = value,
        "show_period_pacing" => cfg.show_period_pacing = value,
        "local_spend_ledger" => cfg.local_spend_ledger = value,
        "reconcile_management_usage" => cfg.reconcile_management_usage = value,
        _ => return,
    }
    set_token_economy_live(cfg);
}

/// Optimistic live update for a Token Economy integer effort field.
///
/// `lock_implement_effort` uses `0` for unlocked (`None`). Out-of-range
/// values are ignored (UI stepper / persist validation are the gates).
pub fn set_token_economy_live_int(field: &str, value: i64) {
    let mut cfg = token_economy_from_disk();
    let in_effort = |n: i64| -> Option<u8> {
        if n < i64::from(MIN_IMPLEMENT_EFFORT) || n > i64::from(MAX_IMPLEMENT_EFFORT) {
            None
        } else {
            Some(n as u8)
        }
    };
    match field {
        "max_implement_effort" => {
            let Some(v) = in_effort(value) else {
                return;
            };
            cfg.max_implement_effort = v;
        }
        "min_implement_effort" => {
            let Some(v) = in_effort(value) else {
                return;
            };
            cfg.min_implement_effort = v;
        }
        "desired_implement_effort" => {
            let Some(v) = in_effort(value) else {
                return;
            };
            cfg.desired_implement_effort = v;
        }
        "lock_implement_effort" => {
            if value == 0 {
                cfg.lock_implement_effort = None;
            } else {
                let Some(v) = in_effort(value) else {
                    return;
                };
                cfg.lock_implement_effort = Some(v);
            }
        }
        _ => return,
    }
    set_token_economy_live(cfg);
}

/// Resolved path for uniquely grok-oss durable state (`grok_oss.db`).
pub fn resolve_grok_oss_database_path(cfg: &TokenEconomyConfig) -> PathBuf {
    if let Some(p) = &cfg.grok_oss_database_path {
        return p.clone();
    }
    xai_grok_config::grok_home().join("grok_oss.db")
}

/// Whether economic implement-effort **ceiling** and **desired inject** are
/// active (economic mode on + cap master). Min floor and lock always apply
/// when set, independent of this flag.
pub fn implement_effort_policy_active(economic_mode: bool, cfg: &TokenEconomyConfig) -> bool {
    economic_mode && cfg.cap_implement_effort_when_economic
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_plan() {
        let d = TokenEconomyConfig::default();
        assert!(d.cap_implement_effort_when_economic);
        assert_eq!(d.max_implement_effort, 3);
        assert_eq!(d.min_implement_effort, 1);
        assert_eq!(d.lock_implement_effort, None);
        assert_eq!(d.desired_implement_effort, 2);
        assert!(d.show_period_pacing);
        assert!(d.local_spend_ledger);
        assert!(d.reconcile_management_usage);
        assert!(d.grok_oss_database_path.is_none());
    }

    #[test]
    fn live_cache_overrides_disk_seed() {
        reset_token_economy_live_to_defaults();
        assert_eq!(token_economy_from_disk().min_implement_effort, 1);
        set_token_economy_live_int("min_implement_effort", 2);
        assert_eq!(token_economy_from_disk().min_implement_effort, 2);
        reset_token_economy_live_to_defaults();
        assert_eq!(token_economy_from_disk().min_implement_effort, 1);
        clear_token_economy_live();
    }

    #[test]
    fn missing_table_is_defaults() {
        let root: TomlValue = toml::from_str("").unwrap();
        assert_eq!(
            token_economy_from_toml(&root).unwrap(),
            TokenEconomyConfig::default()
        );
    }

    #[test]
    fn reads_knobs() {
        let root: TomlValue = toml::from_str(
            r#"
[token_economy]
cap_implement_effort_when_economic = false
max_implement_effort = 4
min_implement_effort = 2
lock_implement_effort = 2
desired_implement_effort = 3
show_period_pacing = false
local_spend_ledger = false
reconcile_management_usage = false
grok_oss_database_path = "/tmp/custom_grok_oss.db"
"#,
        )
        .unwrap();
        let cfg = token_economy_from_toml(&root).unwrap();
        assert!(!cfg.cap_implement_effort_when_economic);
        assert_eq!(cfg.max_implement_effort, 4);
        assert_eq!(cfg.min_implement_effort, 2);
        assert_eq!(cfg.lock_implement_effort, Some(2));
        assert_eq!(cfg.desired_implement_effort, 3);
        assert!(!cfg.show_period_pacing);
        assert!(!cfg.local_spend_ledger);
        assert!(!cfg.reconcile_management_usage);
        assert_eq!(
            cfg.grok_oss_database_path.as_deref(),
            Some(std::path::Path::new("/tmp/custom_grok_oss.db"))
        );
    }

    #[test]
    fn lock_zero_means_unlocked() {
        let root: TomlValue = toml::from_str(
            r#"
[token_economy]
lock_implement_effort = 0
"#,
        )
        .unwrap();
        let cfg = token_economy_from_toml(&root).unwrap();
        assert_eq!(cfg.lock_implement_effort, None);
    }

    #[test]
    fn rejects_desired_above_max() {
        let root: TomlValue = toml::from_str(
            r#"
[token_economy]
max_implement_effort = 2
desired_implement_effort = 4
"#,
        )
        .unwrap();
        let err = token_economy_from_toml(&root).unwrap_err();
        assert!(
            matches!(
                err,
                TokenEconomyConfigError::DesiredAboveMax { desired: 4, max: 2 }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_min_above_max() {
        let root: TomlValue = toml::from_str(
            r#"
[token_economy]
min_implement_effort = 4
max_implement_effort = 3
"#,
        )
        .unwrap();
        let err = token_economy_from_toml(&root).unwrap_err();
        assert!(
            matches!(err, TokenEconomyConfigError::MinAboveMax { min: 4, max: 3 }),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_lock_above_max() {
        // Prefer config validation: lock must be ≤ max_implement_effort.
        let root: TomlValue = toml::from_str(
            r#"
[token_economy]
max_implement_effort = 3
lock_implement_effort = 5
"#,
        )
        .unwrap();
        let err = token_economy_from_toml(&root).unwrap_err();
        assert!(
            matches!(
                err,
                TokenEconomyConfigError::LockOutOfBounds {
                    lock: 5,
                    min: 1,
                    max: 3
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_lock_below_min() {
        let root: TomlValue = toml::from_str(
            r#"
[token_economy]
min_implement_effort = 3
max_implement_effort = 5
lock_implement_effort = 2
"#,
        )
        .unwrap();
        let err = token_economy_from_toml(&root).unwrap_err();
        assert!(
            matches!(
                err,
                TokenEconomyConfigError::LockOutOfBounds {
                    lock: 2,
                    min: 3,
                    max: 5
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_effort_out_of_range() {
        let root: TomlValue = toml::from_str(
            r#"
[token_economy]
max_implement_effort = 9
"#,
        )
        .unwrap();
        let err = token_economy_from_toml(&root).unwrap_err();
        assert!(
            matches!(
                err,
                TokenEconomyConfigError::EffortOutOfRange {
                    field: "max_implement_effort",
                    value: 9
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn policy_active_requires_both() {
        let cfg = TokenEconomyConfig::default();
        assert!(implement_effort_policy_active(true, &cfg));
        assert!(!implement_effort_policy_active(false, &cfg));
        let mut off = cfg.clone();
        off.cap_implement_effort_when_economic = false;
        assert!(!implement_effort_policy_active(true, &off));
    }
}
