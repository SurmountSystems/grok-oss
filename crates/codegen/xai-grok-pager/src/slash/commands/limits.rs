//! `/limits`. SuperGrok included period limits, SuperGrok dollar credits, and
//! console path detail.
//!
//! Multi-line detail for spend meters. Session token/cost stays on `/usage`
//! (`/cost`). Footer stays one-line; this is the full snapshot.
//!
//! - `/limits` — dismissible popup modal (live countdown).
//! - `/limits --json` — pretty JSON into conversation scrollback (same shape
//!   as CLI `grok limits --json`; bypasses the modal).

use crate::app::actions::Action;
use crate::slash::command::{AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand};

/// Show SuperGrok + console limits detail from cached billing.
pub struct LimitsCommand;

impl SlashCommand for LimitsCommand {
    fn name(&self) -> &str {
        "limits"
    }

    fn description(&self) -> &str {
        "View included SuperGrok period limits, SuperGrok dollar credits, and console limits"
    }

    fn usage(&self) -> &str {
        "/limits [--help | --json | stay-supergrok | use-limits | use-console | use-personal | use-business | --use-credits | meter included|dollar-credits|console|combined | refresh]"
    }

    /// Works once an agent view exists (billing cache is app/agent scoped).
    fn session_scoped(&self) -> bool {
        true
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn suggest_args(&self, _ctx: &AppCtx, _args_query: &str) -> Option<Vec<ArgItem>> {
        Some(vec![
            ArgItem {
                display: "--json".into(),
                match_text: "--json".into(),
                insert_text: "--json".into(),
                description: "Print JSON to chat (same as grok-oss limits --json)".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_STAY_SUPERGROK.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_STAY_SUPERGROK.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_STAY_SUPERGROK.into(),
                description: "Stay on the SuperGrok session. This does not override preferred_method api_key.".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_USE_LIMITS.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_USE_LIMITS.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_USE_LIMITS.into(),
                description: "Spend included SuperGrok period limits on the next request."
                    .into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_USE_CONSOLE.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_USE_CONSOLE.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_USE_CONSOLE.into(),
                description: "Spend console API credits with the stored console inference key. No management key.".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_USE_PERSONAL.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_USE_PERSONAL.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_USE_PERSONAL.into(),
                description: "The next request uses the personal SuperGrok session.".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_USE_BUSINESS.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_USE_BUSINESS.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_USE_BUSINESS.into(),
                description: "The next request uses the Business SuperGrok session.".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_METER.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_METER.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_METER.into(),
                description: "Included SuperGrok period limits, SuperGrok dollar credits, console API credits, or combined. meter included spends included limits the same way use-limits does.".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_REFRESH.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_REFRESH.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_REFRESH.into(),
                description: "Force-refresh live meters".into(),
            },
            ArgItem {
                display: "--help".into(),
                match_text: "--help".into(),
                insert_text: "--help".into(),
                description: "List /limits named words and hyphenated aliases".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_USE_CREDITS.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_USE_CREDITS.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_USE_CREDITS.into(),
                description: "Spend SuperGrok dollar credits, not console API credits.".into(),
            },
        ])
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        match crate::limits_cmd::parse_limits_named_args(args) {
            Ok(crate::limits_cmd::LimitsNamedAction::Show)
            | Ok(crate::limits_cmd::LimitsNamedAction::Refresh) => {
                CommandResult::Action(Action::ShowLimits)
            }
            Ok(crate::limits_cmd::LimitsNamedAction::Help) => {
                CommandResult::Message(crate::limits_cmd::limits_help_text())
            }
            Ok(crate::limits_cmd::LimitsNamedAction::Json) => {
                CommandResult::Action(Action::ShowLimitsJson)
            }
            Ok(action) => match crate::limits_cmd::apply_limits_named_action(action) {
                Ok(msg) => CommandResult::Message(msg),
                Err(e) => CommandResult::Error(e),
            },
            Err(e) => CommandResult::Error(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::actions::Action;
    use crate::slash::command::{CommandExecCtx, CommandResult};

    static DEFAULT_BUNDLE_STATE: crate::app::bundle::BundleState =
        crate::app::bundle::BundleState {
            has_cache: false,
            version: String::new(),
            personas: Vec::new(),
            roles: Vec::new(),
            agents: Vec::new(),
            skills: Vec::new(),
            persona_details: Vec::new(),
            role_details: Vec::new(),
        };

    fn make_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn limits_command_emits_show_limits_action() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let result = LimitsCommand.run(&mut ctx, "");
        assert!(
            matches!(result, CommandResult::Action(Action::ShowLimits)),
            "expected ShowLimits, got {result:?}"
        );
    }

    #[test]
    fn limits_json_flag_emits_show_limits_json_action() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        for args in ["--json", "json", "  --json  "] {
            let result = LimitsCommand.run(&mut ctx, args);
            assert!(
                matches!(result, CommandResult::Action(Action::ShowLimitsJson)),
                "expected ShowLimitsJson for {args:?}, got {result:?}"
            );
        }
    }

    #[test]
    fn limits_command_rejects_unknown_args() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let usage = LimitsCommand.usage();
        assert!(
            usage.contains("use-console")
                && usage.contains("use-personal")
                && usage.contains("use-business"),
            "slash usage must list use-personal and use-business next to use-console: {usage}"
        );
        let result = LimitsCommand.run(&mut ctx, "extra");
        assert!(
            matches!(result, CommandResult::Error(ref e) if e.contains("--json")
                && e.contains("stay-supergrok")
                && e.contains("use-console")
                && e.contains("use-personal")
                && e.contains("use-business")
                && e.contains("meter")
                && e.contains("refresh")),
            "expected usage error listing the named words, got {result:?}"
        );
    }

    /// Pin words confirm host/key identity in plain English and must not dump JSON.
    #[test]
    #[serial_test::serial]
    fn limits_pin_words_confirm_switch_not_json_dump() {
        use tempfile::TempDir;
        use xai_grok_test_support::EnvGuard;

        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(xai_grok_shell::auth::credentials_store::FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let store =
            xai_grok_shell::auth::credentials_store::CredentialsStore::at_grok_home(home.path());
        xai_grok_shell::auth::store_console_api_key(&store, "slash-pin-console-key")
            .expect("store console key");

        let models = ModelState::default();
        let mut ctx = make_ctx(&models);

        let stay = LimitsCommand.run(&mut ctx, crate::limits_cmd::LIMITS_WORD_STAY_SUPERGROK);
        match stay {
            CommandResult::Message(msg) => {
                assert!(
                    msg.contains("SuperGrok session") && msg.contains("cli-chat-proxy"),
                    "stay-supergrok must confirm SuperGrok session host: {msg}"
                );
                assert!(
                    !msg.contains("schemaVersion") && !msg.trim_start().starts_with('{'),
                    "pin confirmation must not dump JSON: {msg}"
                );
            }
            other => panic!("stay-supergrok must be a short confirmation, not {other:?}"),
        }

        let use_console = LimitsCommand.run(&mut ctx, crate::limits_cmd::LIMITS_WORD_USE_CONSOLE);
        match use_console {
            CommandResult::Message(msg) => {
                assert!(
                    msg.contains("console API key") && msg.contains("api.x.ai"),
                    "use-console must confirm console key host: {msg}"
                );
                assert!(
                    msg.to_ascii_lowercase().contains("operator asked"),
                    "use-console must say the operator asked, not a printout hop: {msg}"
                );
                assert!(
                    !msg.contains("schemaVersion") && !msg.trim_start().starts_with('{'),
                    "pin confirmation must not dump JSON: {msg}"
                );
            }
            other => panic!("use-console must be a short confirmation, not {other:?}"),
        }

        let bare = LimitsCommand.run(&mut ctx, "");
        assert!(
            matches!(bare, CommandResult::Action(Action::ShowLimits)),
            "bare /limits stays collect, got {bare:?}"
        );
    }

    /// TUI `/limits` and CLI `grok-oss limits` share the same named words.
    #[test]
    #[serial_test::serial]
    fn limits_slash_and_cli_share_stay_supergrok_words() {
        use clap::Parser;
        use tempfile::TempDir;
        use xai_grok_test_support::EnvGuard;

        let home = TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(xai_grok_shell::auth::credentials_store::FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        let store =
            xai_grok_shell::auth::credentials_store::CredentialsStore::at_grok_home(home.path());
        xai_grok_shell::auth::store_console_api_key(&store, "slash-share-words-console-key")
            .expect("store console key so use-console can pin");

        let stay = crate::limits_cmd::LIMITS_WORD_STAY_SUPERGROK;
        let use_console = crate::limits_cmd::LIMITS_WORD_USE_CONSOLE;
        let use_personal = crate::limits_cmd::LIMITS_WORD_USE_PERSONAL;
        let use_business = crate::limits_cmd::LIMITS_WORD_USE_BUSINESS;
        let meter = crate::limits_cmd::LIMITS_WORD_METER;
        let refresh = crate::limits_cmd::LIMITS_WORD_REFRESH;
        assert_eq!(stay, "stay-supergrok");
        assert_eq!(use_console, "use-console");
        assert_eq!(use_personal, "use-personal");
        assert_eq!(use_business, "use-business");
        assert_eq!(meter, "meter");
        assert_eq!(refresh, "refresh");

        let usage = LimitsCommand.usage();
        assert!(
            usage.contains(use_console)
                && usage.contains(use_personal)
                && usage.contains(use_business),
            "slash usage must list use-personal and use-business next to use-console: {usage}"
        );
        let models = ModelState::default();
        let app_ctx = crate::slash::command::AppCtx {
            models: &models,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: false,
            screen_mode: crate::app::ScreenMode::Inline,
            current_title: None,
        };
        let suggested: Vec<String> = LimitsCommand
            .suggest_args(&app_ctx, "")
            .expect("slash /limits suggestions")
            .into_iter()
            .map(|item| item.insert_text)
            .collect();
        assert!(
            suggested.iter().any(|w| w == use_personal)
                && suggested.iter().any(|w| w == use_business)
                && suggested.iter().any(|w| w == use_console),
            "suggest_args must offer use-personal and use-business next to use-console, got {suggested:?}"
        );

        let mut ctx = make_ctx(&models);
        for (args, label) in [
            (stay, "stay-supergrok"),
            (use_console, "use-console"),
            (refresh, "refresh"),
            ("meter included", "meter included"),
            ("meter dollar-credits", "meter dollar-credits"),
            ("meter console", "meter console"),
            ("meter combined", "meter combined"),
        ] {
            let result = LimitsCommand.run(&mut ctx, args);
            assert!(
                !matches!(result, CommandResult::Error(_)),
                "slash /limits {label} must share the CLI word, got {result:?}"
            );
        }

        for args in [
            vec!["grok-oss", "limits", stay],
            vec!["grok-oss", "limits", use_console],
            vec!["grok-oss", "limits", use_personal],
            vec!["grok-oss", "limits", use_business],
            vec!["grok-oss", "limits", refresh],
            vec!["grok-oss", "limits", meter, "included"],
            vec!["grok-oss", "limits", meter, "dollar-credits"],
            vec!["grok-oss", "limits", meter, "console"],
            vec!["grok-oss", "limits", meter, "combined"],
        ] {
            crate::app::cli::PagerArgs::try_parse_from(&args).unwrap_or_else(|e| {
                panic!("CLI {:?} must parse the same words as slash: {e}", args)
            });
        }
    }

    #[test]
    fn limits_registered_in_builtins() {
        let names: Vec<_> = crate::slash::commands::builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "limits"),
            "expected /limits in builtin_commands, got {names:?}"
        );
        // Prefer dedicated /limits over overloading /usage for session tokens.
        assert!(
            names.iter().any(|n| n == "usage"),
            "/usage must remain for session tokens"
        );
    }

    #[test]
    fn limits_description_names_supergrok_dollar_credits_not_extras() {
        let desc = LimitsCommand.description();
        assert!(
            desc.contains("SuperGrok dollar credits"),
            "slash picker must name SuperGrok dollar credits: {desc}"
        );
        assert!(
            desc.contains("included SuperGrok period limits"),
            "slash picker must name included SuperGrok period limits: {desc}"
        );
        assert!(
            !desc.to_ascii_lowercase().contains("extras"),
            "slash picker must not teach extras as a nickname: {desc}"
        );
    }

    /// Operator: "would be nice if the limits command took help". `/limits
    /// --help` used to print `Unknown argument: --help`. Help must list the
    /// named words and hyphenated aliases.
    #[test]
    fn limits_help_lists_named_words_and_hyphenated_aliases() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        for args in ["--help", "help", "-h"] {
            let result = LimitsCommand.run(&mut ctx, args);
            match result {
                CommandResult::Message(msg) => {
                    assert!(
                        msg.contains("stay-supergrok")
                            && msg.contains("--stay-supergrok")
                            && msg.contains("use-console")
                            && msg.contains("--use-console")
                            && msg.contains("use-personal")
                            && msg.contains("use-business")
                            && msg.contains("--use-credits")
                            && msg.contains("meter")
                            && msg.contains("refresh")
                            && msg.contains("included SuperGrok period limits")
                            && msg.contains("SuperGrok dollar credits"),
                        "/limits {args} help must list named words and hyphen aliases: {msg}"
                    );
                    assert!(
                        !msg.contains("Unknown argument"),
                        "/limits {args} must not print Unknown argument: {msg}"
                    );
                    assert!(
                        !msg.contains("free SuperGrok")
                            && !msg.to_ascii_lowercase().contains("extras"),
                        "help must not call SuperGrok free or teach extras: {msg}"
                    );
                }
                other => panic!("/limits {args} must print help, got {other:?}"),
            }
        }
    }

    /// Operator: `/limits use credits` printed unknown argument. Hyphenated
    /// options must match the unhyphenated words. `--use-credits` pins
    /// SuperGrok dollar credits.
    #[test]
    fn limits_hyphenated_aliases_match_unhyphenated_words() {
        use crate::limits_cmd::{LimitsMeterWord, LimitsNamedAction, parse_limits_named_args};

        let pairs = [
            ("stay-supergrok", "--stay-supergrok"),
            ("use-console", "--use-console"),
            ("use-personal", "--use-personal"),
            ("use-business", "--use-business"),
            ("refresh", "--refresh"),
        ];
        for (bare, hyphen) in pairs {
            let a = parse_limits_named_args(bare).expect(bare);
            let b = parse_limits_named_args(hyphen).expect(hyphen);
            assert_eq!(a, b, "{bare} must match {hyphen}");
        }
        assert_eq!(
            parse_limits_named_args("meter included").unwrap(),
            parse_limits_named_args("--meter included").unwrap()
        );
        for credits in ["--use-credits", "use-credits", "use credits"] {
            assert_eq!(
                parse_limits_named_args(credits).expect(credits),
                LimitsNamedAction::Meter(LimitsMeterWord::DollarCredits),
                "{credits} must pin SuperGrok dollar credits"
            );
        }
    }

    /// `/limits` offers use-limits. That command spends included SuperGrok
    /// period limits. With preferred_method = api_key, the next request key
    /// is the SuperGrok session.
    #[test]
    #[serial_test::serial]
    fn limits_menu_offers_use_limits_and_that_command_spends_included_limits() {
        use xai_grok_shell::auth::limits_pins::{
            MeterSource, apply_limits_pins_to_sampler_config, apply_meter_source, load_limits_pins,
        };
        use xai_grok_shell::sampling::SamplerConfig;
        use xai_grok_test_support::EnvGuard;

        let models = ModelState::default();
        let app_ctx = crate::slash::command::AppCtx {
            models: &models,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: false,
            screen_mode: crate::app::ScreenMode::Inline,
            current_title: None,
        };
        let suggested = LimitsCommand
            .suggest_args(&app_ctx, "")
            .expect("slash /limits suggestions");
        let blob = suggested
            .iter()
            .map(|item| {
                format!(
                    "{} {} {} {}",
                    item.display, item.match_text, item.insert_text, item.description
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            suggested
                .iter()
                .any(|item| item.insert_text == "use-limits"),
            "suggestion list must contain use-limits: {blob}"
        );
        let use_limits = suggested
            .iter()
            .find(|item| item.insert_text == "use-limits")
            .expect("use-limits row");
        assert_eq!(
            use_limits.description,
            "Spend included SuperGrok period limits on the next request."
        );
        assert!(
            !blob.contains("sidecar pin"),
            "suggestion list must not say sidecar pin: {blob}"
        );
        assert!(
            !blob.contains("pin meter chrome"),
            "suggestion list must not say pin meter chrome: {blob}"
        );

        let home = tempfile::TempDir::new().expect("temp grok home");
        let _env = EnvGuard::set("GROK_HOME", home.path());
        let _force = EnvGuard::set(xai_grok_shell::auth::credentials_store::FORCE_FILE_ENV, "1");
        let _xai = EnvGuard::unset("XAI_API_KEY");
        let _legacy = EnvGuard::unset("GROK_CODE_XAI_API_KEY");
        std::fs::write(
            home.path().join("config.toml"),
            "[auth]\npreferred_method = \"api_key\"\n",
        )
        .expect("write preferred_method = api_key");
        apply_meter_source(MeterSource::Console).expect("start on console API credits");

        let session = "included-period-session-token";
        let console = "console-inference-key";
        let mut ctx = make_ctx(&models);
        let result = LimitsCommand.run(&mut ctx, "use-limits");
        let msg = match result {
            CommandResult::Message(msg) => msg,
            other => panic!("use-limits must confirm, not {other:?}"),
        };
        assert!(
            !msg.contains("still pins console") && !msg.contains("api_key still pins"),
            "use-limits confirmation must not say that api_key still pins console: {msg}"
        );
        assert_eq!(
            load_limits_pins().meter_source,
            Some(MeterSource::Included),
            "use-limits must set MeterSource::Included"
        );

        let mut config = SamplerConfig {
            api_key: Some(console.into()),
            failover_api_keys: vec![session.into()],
            base_url: "https://api.x.ai/v1".into(),
            model: "grok-4".into(),
            session_identity_key: Some(session.into()),
            failover_base_url: Some("https://api.x.ai/v1".into()),
            session_base_url: Some("https://cli-chat-proxy.grok.com/v1".into()),
            ..Default::default()
        };
        apply_limits_pins_to_sampler_config(&mut config);
        assert_eq!(
            config.api_key.as_deref(),
            Some(session),
            "use-limits with preferred_method api_key must make the next request key the SuperGrok session"
        );
        assert_ne!(config.api_key.as_deref(), Some(console));
        assert!(
            config.base_url.contains("cli-chat-proxy"),
            "included limits stay on the SuperGrok session host: {}",
            config.base_url
        );
    }
}
