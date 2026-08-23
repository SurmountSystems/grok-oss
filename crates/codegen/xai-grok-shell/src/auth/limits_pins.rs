//! Stay-SuperGrok and meter-source pins in a Surmount sidecar file.
//!
//! Path: `$GROK_HOME/limits_pins.json`, sibling of the `exhausted_credits/`
//! directory. Missing or unreadable file is fail-open (no stay pin, no
//! meter-source pin). Not `[auth]`, not `[token_economy]`, not `grok_oss.db`.
//! Same words on TUI `/limits` and CLI `grok-oss limits`. Stock
//! `preferred_method = "api_key"` still pins console. A client 100% /
//! remaining 0 / $0 printout must not mark SuperGrok used up. Sampler
//! reconstruct honors `stay_supergrok` / `use_console` from this sidecar.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Filename under `$GROK_HOME`. Sibling of `exhausted_credits/`.
pub const LIMITS_PINS_FILE: &str = "limits_pins.json";

/// Meter chrome / `/limits` emphasis. Wire strings for G3 later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeterSource {
    Included,
    DollarCredits,
    Console,
    Combined,
}

impl MeterSource {
    /// Plain American English name for compact `/limits` chrome.
    ///
    /// Combined is only honest when remaining is across distinct SuperGrok
    /// identities. grok-oss limits JSON is a client printout, not xAI billing
    /// truth. Do not invent remaining. Do not call any pool used up.
    pub fn as_human(self) -> &'static str {
        match self {
            Self::Included => "included SuperGrok period limits",
            Self::DollarCredits => "SuperGrok dollar credits",
            Self::Console => "console team prepaid / console API credits",
            Self::Combined => "combined",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LimitsPins {
    /// When true, reconstruct keeps SuperGrok as sampler. Missing file or key = false.
    #[serde(default)]
    pub stay_supergrok: bool,
    /// When true, the operator asked for the console key (sidecar, not `[auth]`).
    #[serde(default)]
    pub use_console: bool,
    /// Which meter chrome should emphasize. None = no pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meter_source: Option<MeterSource>,
}

fn grok_home_path() -> PathBuf {
    if let Ok(v) = std::env::var("GROK_HOME") {
        return PathBuf::from(v);
    }
    #[allow(deprecated)]
    let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".grok")
}

/// `$GROK_HOME/limits_pins.json`. Reads `GROK_HOME` each call (not OnceLock).
pub fn limits_pins_path() -> PathBuf {
    grok_home_path().join(LIMITS_PINS_FILE)
}

/// Missing, empty, or unreadable file → [`LimitsPins::default`]. Never errors.
pub fn load_limits_pins() -> LimitsPins {
    load_limits_pins_under(&grok_home_path())
}

/// Same as [`load_limits_pins`], under an explicit home (unit tests).
pub fn load_limits_pins_under(grok_home: &Path) -> LimitsPins {
    let path = grok_home.join(LIMITS_PINS_FILE);
    let Ok(mut file) = File::open(&path) else {
        return LimitsPins::default();
    };
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return LimitsPins::default();
    }
    if buf.trim().is_empty() {
        return LimitsPins::default();
    }
    serde_json::from_str(buf.trim()).unwrap_or_default()
}

/// Persist stay SuperGrok and clear a false exhaust memo so SuperGrok is used
/// again without requiring console credits. Does not fight stock
/// `[auth] preferred_method = "api_key"`.
pub fn apply_stay_supergrok() -> Result<StaySupergrokApply, std::io::Error> {
    if disk_preferred_is_console_primary() {
        return Ok(StaySupergrokApply::BlockedByPreferredApiKey);
    }
    let mut pins = load_limits_pins();
    pins.stay_supergrok = true;
    pins.use_console = false;
    save_limits_pins(&pins)?;
    xai_grok_sampler::clear_all_including_durable();
    Ok(StaySupergrokApply::Applied)
}

/// Outcome of [`apply_stay_supergrok`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaySupergrokApply {
    /// Sidecar `stay_supergrok` is true and exhaust memos were cleared.
    Applied,
    /// Stock `[auth] preferred_method = "api_key"` pins console. Command does
    /// not override that key.
    BlockedByPreferredApiKey,
}

/// Persist that the operator wants the console key (sidecar, not `[auth]`).
///
/// Fail loud when no console API key is stored (credentials store, env, or
/// config). Does not write `[auth] preferred_method`. Does not mark SuperGrok
/// used up. A stored key is enough even when it is not already in live
/// failover.
pub fn apply_use_console() -> Result<(), std::io::Error> {
    if stored_console_keys_for_use_console().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No console API key is stored. Store a console key in the credentials store or config, then retry use-console.",
        ));
    }
    let mut pins = load_limits_pins();
    pins.use_console = true;
    pins.stay_supergrok = false;
    save_limits_pins(&pins)
}

/// Honor sidecar `stay_supergrok` / `use_console` on a reconstructed sampler
/// config. Does not write `[auth] preferred_method`. Stock
/// `preferred_method = "api_key"` still pins console and wins over stay.
/// `use-console` switches live identity without that stock key. A stored
/// console key is enough even when it is not already in live failover.
pub fn apply_limits_pins_to_sampler_config(config: &mut xai_grok_sampler::SamplerConfig) {
    if disk_preferred_is_console_primary() {
        return;
    }
    let pins = load_limits_pins();
    if pins.stay_supergrok {
        xai_grok_sampler::prefer_supergrok_identity_for_stay_pin(config);
        return;
    }
    if pins.use_console {
        inject_stored_console_keys_into_failover(config);
        xai_grok_sampler::prefer_console_identity_for_use_console_pin(config);
    }
}

/// Env, credentials store, then `auth.json` API key. Unique, first-seen order.
/// Uses [`grok_home_path`] (reads `GROK_HOME` each call) so tests isolate.
fn stored_console_keys_for_use_console() -> Vec<String> {
    let mut keys = Vec::new();
    let mut push = |part: String| {
        if !part.is_empty() && !keys.iter().any(|k| k == &part) {
            keys.push(part);
        }
    };
    if let Ok(raw) = crate::agent::auth_method::read_xai_api_key_env() {
        for part in crate::agent::config::split_api_key_list(&raw) {
            push(part);
        }
    }
    let home = grok_home_path();
    let store = super::credentials_store::CredentialsStore::at_grok_home(&home);
    if let Ok(stored) = super::xai_console::load_stored_console_api_keys(&store) {
        for part in stored {
            push(part);
        }
    }
    if let Some(disk) = super::read_api_key(&home) {
        for part in crate::agent::config::split_api_key_list(&disk) {
            push(part);
        }
    }
    keys
}

fn failover_has_console_candidate(config: &xai_grok_sampler::SamplerConfig) -> bool {
    let active = config.api_key.as_deref().unwrap_or("").trim();
    let sess = config
        .session_identity_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    config.failover_api_keys.iter().any(|k| {
        let t = k.trim();
        !t.is_empty() && t != active && (sess.is_empty() || t != sess)
    })
}

/// Put stored console keys onto the sampler failover list when live failover
/// has no console candidate, so use-console does not require a prior dual-auth
/// failover entry.
fn inject_stored_console_keys_into_failover(config: &mut xai_grok_sampler::SamplerConfig) {
    if failover_has_console_candidate(config) {
        return;
    }
    let keys = stored_console_keys_for_use_console();
    if keys.is_empty() {
        return;
    }
    let active = config.api_key.as_deref().unwrap_or("").trim().to_owned();
    let sess = config
        .session_identity_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_owned();
    let mut to_add = Vec::new();
    for key in keys {
        let t = key.trim();
        if t.is_empty() || t == active || (!sess.is_empty() && t == sess) {
            continue;
        }
        if config.failover_api_keys.iter().any(|k| k.trim() == t) {
            continue;
        }
        to_add.push(key);
    }
    for (i, key) in to_add.into_iter().enumerate() {
        config.failover_api_keys.insert(i, key);
    }
    if config.failover_base_url.is_none() {
        config.failover_base_url = Some(super::xai_console::XAI_CONSOLE_API_URL.to_owned());
    }
}

/// Persist `/limits meter` / `grok-oss limits meter` source pin.
pub fn apply_meter_source(source: MeterSource) -> Result<(), std::io::Error> {
    let mut pins = load_limits_pins();
    pins.meter_source = Some(source);
    save_limits_pins(&pins)
}

/// Stock `[auth] preferred_method = "api_key"` under `$GROK_HOME/config.toml`.
/// Campaign / overlay loaders are not consulted so a temp `$GROK_HOME` stays isolated.
fn disk_preferred_is_console_primary() -> bool {
    let path = grok_home_path().join("config.toml");
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = toml::from_str::<toml::Value>(&text) else {
        return false;
    };
    let method = value
        .get("auth")
        .or_else(|| value.get("grok_com_config"))
        .and_then(|t| t.get("preferred_method"))
        .and_then(|v| v.as_str());
    matches!(method, Some("api_key"))
}

/// Best-effort atomic write (temp + rename), like exhaust memos.
pub fn save_limits_pins(pins: &LimitsPins) -> std::io::Result<()> {
    save_limits_pins_under(&grok_home_path(), pins)
}

/// Same as [`save_limits_pins`], under an explicit home (unit tests).
pub fn save_limits_pins_under(grok_home: &Path, pins: &LimitsPins) -> std::io::Result<()> {
    fs::create_dir_all(grok_home)?;
    let path = grok_home.join(LIMITS_PINS_FILE);
    let data = serde_json::to_vec_pretty(pins)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // Same as exhaust memos: `.with_extension("json.tmp")` replaces `.json`.
    let tmp = path.with_extension("json.tmp");
    let write = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(&data)?;
        let _ = f.sync_all();
        drop(f);
        fs::rename(&tmp, &path)
    })();
    if write.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::Config;
    use crate::auth::{GrokComConfig, PreferredAuthMethod};
    use crate::grok_oss::GROK_OSS_DB_FILE;
    use std::fs;
    use tempfile::TempDir;
    use xai_grok_test_support::EnvGuard;

    fn stock_preferred_method_toml() -> toml::Value {
        toml::from_str(
            r#"
[auth]
preferred_method = "api_key"
"#,
        )
        .expect("stock preferred_method toml")
    }

    /// Stock `GrokComConfig` fields only: no stay-SuperGrok or meter-source pin
    /// on `[auth]`. Adding those fields here must fail this destructure.
    fn assert_grok_com_config_has_only_stock_fields(cfg: &GrokComConfig) {
        let GrokComConfig {
            grok_ws_origin: _,
            grok_ws_url: _,
            token_header: _,
            oidc: _,
            oauth2: _,
            auth_provider_command: _,
            auth_provider_label: _,
            auth_token_ttl: _,
            disable_api_key_auth: _,
            force_login_team_uuid: _,
            preferred_method: _,
            auto_use_included_limits: _,
            allow_spend_when_free_period_debit_unproven: _,
        } = cfg;
    }

    #[test]
    #[serial_test::serial]
    fn stock_preferred_method_config_loads_without_surmount_auth_keys() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());

        let cfg = Config::new_from_toml_cfg(&stock_preferred_method_toml())
            .expect("stock [auth] preferred_method = api_key must load");
        assert_eq!(
            cfg.grok_com_config.preferred_method,
            Some(PreferredAuthMethod::ApiKey)
        );
        assert_grok_com_config_has_only_stock_fields(&cfg.grok_com_config);

        assert!(
            !home.path().join(LIMITS_PINS_FILE).exists(),
            "sidecar must be missing for fail-open defaults"
        );
        let pins = load_limits_pins();
        assert!(!pins.stay_supergrok);
        assert_eq!(pins.meter_source, None);
        let under = load_limits_pins_under(home.path());
        assert!(!under.stay_supergrok);
        assert_eq!(under.meter_source, None);
    }

    #[test]
    #[serial_test::serial]
    fn surmount_pin_file_does_not_require_new_auth_keys() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        fs::write(
            home.path().join(LIMITS_PINS_FILE),
            r#"{"stay_supergrok":true,"meter_source":"dollar-credits"}"#,
        )
        .expect("write sidecar");

        let cfg = Config::new_from_toml_cfg(&stock_preferred_method_toml())
            .expect("stock toml still loads when sidecar is present");
        assert_eq!(
            cfg.grok_com_config.preferred_method,
            Some(PreferredAuthMethod::ApiKey)
        );
        assert_grok_com_config_has_only_stock_fields(&cfg.grok_com_config);

        let pins = load_limits_pins_under(home.path());
        assert!(pins.stay_supergrok);
        assert_eq!(pins.meter_source, Some(MeterSource::DollarCredits));
    }

    #[test]
    #[serial_test::serial]
    fn exhaust_memo_stays_under_exhausted_credits_pins_are_sibling_file() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let grok_home = home.path();

        let expected = grok_home.join(LIMITS_PINS_FILE);
        assert_eq!(limits_pins_path(), expected);
        assert_eq!(expected, grok_home.join("limits_pins.json"));
        assert!(
            !expected.starts_with(grok_home.join("exhausted_credits")),
            "pins must not live under exhausted_credits/"
        );
        assert_ne!(
            expected.file_name(),
            Some(std::ffi::OsStr::new(GROK_OSS_DB_FILE)),
            "pins must not be grok_oss.db"
        );

        let exhaust_dir = grok_home.join("exhausted_credits");
        fs::create_dir_all(&exhaust_dir).expect("exhaust dir");
        let memo = exhaust_dir.join("abc123.json");
        let original = "{\n  \"until_unix_ms\": 1\n}";
        fs::write(&memo, original).expect("seed exhaust memo");

        let pins = LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: Some(MeterSource::Included),
        };
        save_limits_pins(&pins).expect("save pins");
        save_limits_pins_under(grok_home, &pins).expect("save pins under home");

        assert_eq!(
            fs::read_to_string(&memo).expect("read exhaust memo"),
            original,
            "saving pins must not rewrite exhausted_credits/*.json bodies"
        );
        assert!(
            !exhaust_dir.join(LIMITS_PINS_FILE).exists(),
            "pins must not be a file inside exhausted_credits/"
        );
        assert!(
            expected.exists(),
            "pins must land at $GROK_HOME/limits_pins.json"
        );
    }

    #[test]
    #[serial_test::serial]
    fn missing_limits_pins_file_returns_defaults() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        assert!(!home.path().join(LIMITS_PINS_FILE).exists());
        let pins = load_limits_pins();
        assert!(!pins.stay_supergrok);
        assert_eq!(pins.meter_source, None);
        let under = load_limits_pins_under(home.path());
        assert_eq!(under, LimitsPins::default());
    }

    #[test]
    #[serial_test::serial]
    fn limits_pins_round_trip_save_then_load_under_temp_grok_home() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let pins = LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: Some(MeterSource::Console),
        };
        save_limits_pins_under(home.path(), &pins).expect("save under home");
        assert_eq!(load_limits_pins_under(home.path()), pins);
        save_limits_pins(&LimitsPins {
            stay_supergrok: false,
            use_console: false,
            meter_source: Some(MeterSource::Combined),
        })
        .expect("save via GROK_HOME");
        let loaded = load_limits_pins();
        assert!(!loaded.stay_supergrok);
        assert_eq!(loaded.meter_source, Some(MeterSource::Combined));
    }

    #[test]
    #[serial_test::serial]
    fn corrupt_limits_pins_json_fail_opens_to_defaults() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        fs::write(home.path().join(LIMITS_PINS_FILE), "this is not json {")
            .expect("write corrupt sidecar");
        let pins = load_limits_pins_under(home.path());
        assert!(!pins.stay_supergrok);
        assert_eq!(pins.meter_source, None);
        let via_env = load_limits_pins();
        assert_eq!(via_env, LimitsPins::default());
    }

    /// Stay SuperGrok must persist the sidecar pin and clear a false exhaust
    /// memo so SuperGrok is used again without requiring console credits.
    #[test]
    #[serial_test::serial]
    fn stay_supergrok_clears_false_exhaust_without_console_credits() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        xai_grok_sampler::clear_all_including_durable();

        xai_grok_sampler::mark_exhausted("fp-false-exhaust");
        assert!(
            xai_grok_sampler::is_exhausted("fp-false-exhaust"),
            "precondition: false exhaust memo is live"
        );
        assert!(
            !home.path().join("console").exists(),
            "this recovery must not require console credits on disk"
        );

        let outcome = apply_stay_supergrok().expect("apply stay SuperGrok");
        assert_eq!(outcome, StaySupergrokApply::Applied);

        let pins = load_limits_pins();
        assert!(
            pins.stay_supergrok,
            "sidecar must persist stay SuperGrok without a new [auth] key"
        );
        assert!(
            !xai_grok_sampler::is_exhausted("fp-false-exhaust"),
            "false exhaust memo must be cleared so SuperGrok is used again"
        );

        xai_grok_sampler::clear_all_including_durable();
    }

    fn dual_auth_sampler(session: &str, console: &str) -> xai_grok_sampler::SamplerConfig {
        xai_grok_sampler::SamplerConfig {
            api_key: Some(session.into()),
            failover_api_keys: vec![console.into()],
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        }
    }

    /// Named contract: sidecar `use_console` switches live identity to the
    /// console key without writing stock `[auth] preferred_method = "api_key"`.
    #[test]
    #[serial_test::serial]
    fn use_console_switches_sampler_without_editing_preferred_method() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(crate::auth::credentials_store::FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let session = "use-console-session-jwt";
        let console = "use-console-console-key";
        let store = crate::auth::credentials_store::CredentialsStore::at_grok_home(home.path());
        crate::auth::store_console_api_key(&store, console).expect("store console key");
        apply_use_console().expect("persist use-console pin");
        assert!(
            !home.path().join("config.toml").exists(),
            "precondition: no stock preferred_method on disk"
        );

        let mut config = dual_auth_sampler(session, console);
        apply_limits_pins_to_sampler_config(&mut config);

        assert_eq!(
            config.api_key.as_deref(),
            Some(console),
            "use-console sidecar must switch live identity to console without preferred_method=api_key"
        );
        assert!(
            config.base_url.contains("api.x.ai"),
            "use-console must switch host to console API: {}",
            config.base_url
        );
        assert!(
            config.bearer_resolver.is_none(),
            "console primary must not keep the SuperGrok bearer resolver"
        );
        assert!(
            !home.path().join("config.toml").exists()
                || !fs::read_to_string(home.path().join("config.toml"))
                    .unwrap_or_default()
                    .contains("preferred_method"),
            "use-console must not write [auth] preferred_method"
        );
        let pins = load_limits_pins();
        assert!(pins.use_console);
        assert!(!pins.stay_supergrok);
        assert!(
            !xai_grok_sampler::is_credential_exhausted(session),
            "use-console must not mark SuperGrok used up"
        );
    }

    /// Named contract: sidecar `stay_supergrok` restores SuperGrok as primary
    /// even when reconstruct currently has the console key first. A client
    /// 100% / remaining 0 / SuperGrok dollar credits $0 printout must not
    /// mark SuperGrok used up.
    #[test]
    #[serial_test::serial]
    fn stay_supergrok_keeps_sampler_primary_on_client_100_printout() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        xai_grok_sampler::clear_all_including_durable();
        apply_stay_supergrok().expect("persist stay SuperGrok pin");

        let session = "stay-pin-session-jwt";
        let console = "stay-pin-console-key";
        let marked =
            xai_grok_sampler::sync_allowance_exhaust_from_usage(100.0, Some(session), true);
        assert_eq!(
            marked,
            xai_grok_sampler::AllowanceExhaustAction::None,
            "client 100% printout must not mark SuperGrok used up"
        );
        assert!(
            !xai_grok_sampler::is_credential_exhausted(session),
            "fail-open: remaining 0 printout is not proof SuperGrok is used up"
        );

        let mut config = dual_auth_sampler(session, console);
        config.api_key = Some(console.into());
        config.failover_api_keys = vec![session.into()];
        config.base_url = "https://api.x.ai/v1".into();
        config.bearer_resolver = None;
        apply_limits_pins_to_sampler_config(&mut config);

        assert_eq!(
            config.api_key.as_deref(),
            Some(session),
            "stay-supergrok must keep SuperGrok primary"
        );
        assert!(
            config.base_url.contains("cli-chat-proxy"),
            "stay-supergrok must stay on SuperGrok host: {}",
            config.base_url
        );
        assert_eq!(
            xai_grok_sampler::prefer_live_identity_after_credit_exhaust(&mut config),
            None,
            "stay-supergrok fail-open must not hop to console from a client 100% printout"
        );
        assert_eq!(config.api_key.as_deref(), Some(session));
        xai_grok_sampler::clear_all_including_durable();
    }

    /// Stock `[auth] preferred_method = "api_key"` still pins console even
    /// when the sidecar has stay SuperGrok.
    #[test]
    #[serial_test::serial]
    fn stock_preferred_method_api_key_wins_over_stay_sidecar() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"api_key\"\n",
        )
        .expect("write stock preferred_method");
        save_limits_pins(&LimitsPins {
            stay_supergrok: true,
            use_console: false,
            meter_source: None,
        })
        .expect("write stay sidecar");

        let session = "api-key-pin-session";
        let console = "api-key-pin-console";
        let mut config = dual_auth_sampler(session, console);
        config.api_key = Some(console.into());
        config.failover_api_keys = vec![session.into()];
        config.base_url = "https://api.x.ai/v1".into();
        apply_limits_pins_to_sampler_config(&mut config);
        assert_eq!(
            config.api_key.as_deref(),
            Some(console),
            "stock preferred_method=api_key must keep console primary"
        );
        assert!(config.base_url.contains("api.x.ai"));
    }

    /// Named contract: `/limits use-console` / `grok-oss limits use-console`
    /// switches live identity to a stored console API key even when that key
    /// is not already in the sampler failover list. Does not write
    /// `[auth] preferred_method`. Does not mark SuperGrok used up.
    #[test]
    #[serial_test::serial]
    fn use_console_switches_to_stored_console_key_not_already_in_failover() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(crate::auth::credentials_store::FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");

        let session = "use-console-stored-session-jwt";
        let console = "use-console-stored-console-key";
        let store = crate::auth::credentials_store::CredentialsStore::at_grok_home(home.path());
        crate::auth::store_console_api_key(&store, console).expect("store console key");

        apply_use_console().expect("use-console with a stored console key");
        assert!(
            !home.path().join("config.toml").exists(),
            "precondition: no stock preferred_method on disk"
        );

        let mut config = dual_auth_sampler(session, "not-the-stored-key");
        config.failover_api_keys.clear();
        assert!(
            config.failover_api_keys.is_empty(),
            "precondition: stored console key is not in live failover"
        );
        apply_limits_pins_to_sampler_config(&mut config);

        assert_eq!(
            config.api_key.as_deref(),
            Some(console),
            "use-console must switch to the stored console key even when it was not in failover"
        );
        assert!(
            config.base_url.contains("api.x.ai"),
            "use-console must switch host to console API: {}",
            config.base_url
        );
        assert!(
            config.bearer_resolver.is_none(),
            "console primary must not keep the SuperGrok bearer resolver"
        );
        assert!(
            !home.path().join("config.toml").exists()
                || !fs::read_to_string(home.path().join("config.toml"))
                    .unwrap_or_default()
                    .contains("preferred_method"),
            "use-console must not write [auth] preferred_method"
        );
        let pins = load_limits_pins();
        assert!(pins.use_console);
        assert!(!pins.stay_supergrok);
        assert!(
            !xai_grok_sampler::is_credential_exhausted(session),
            "use-console must not mark SuperGrok used up"
        );
    }

    /// Named contract: use-console fails loud when no console API key is
    /// stored (credentials store / config). Does not write a pin that cannot
    /// switch live identity.
    #[test]
    #[serial_test::serial]
    fn use_console_fails_loud_when_no_console_key_is_stored() {
        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(crate::auth::credentials_store::FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");

        let err =
            apply_use_console().expect_err("use-console must fail when no console key is stored");
        let msg = err.to_string();
        assert!(
            msg.to_ascii_lowercase().contains("console")
                && (msg.to_ascii_lowercase().contains("stored")
                    || msg.to_ascii_lowercase().contains("store")),
            "fail-loud message must name the missing stored console key: {msg}"
        );
        assert!(
            !home.path().join(LIMITS_PINS_FILE).exists() || !load_limits_pins().use_console,
            "must not persist a use-console pin when no console key is stored"
        );
        assert!(
            !home.path().join("config.toml").exists(),
            "use-console must not write [auth] preferred_method"
        );
    }
}
