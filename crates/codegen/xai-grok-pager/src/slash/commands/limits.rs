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
        "/limits [--json | stay-supergrok | use-console | meter included|dollar-credits|console|combined | refresh]"
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
                description: "Stay on SuperGrok and clear a false exhaust memo".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_USE_CONSOLE.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_USE_CONSOLE.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_USE_CONSOLE.into(),
                description: "Ask for the console key (sidecar pin)".into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_METER.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_METER.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_METER.into(),
                description: "Pin meter chrome: included | dollar-credits | console | combined"
                    .into(),
            },
            ArgItem {
                display: crate::limits_cmd::LIMITS_WORD_REFRESH.into(),
                match_text: crate::limits_cmd::LIMITS_WORD_REFRESH.into(),
                insert_text: crate::limits_cmd::LIMITS_WORD_REFRESH.into(),
                description: "Force-refresh live meters".into(),
            },
        ])
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        match crate::limits_cmd::parse_limits_named_args(args) {
            Ok(crate::limits_cmd::LimitsNamedAction::Show)
            | Ok(crate::limits_cmd::LimitsNamedAction::Refresh) => {
                CommandResult::Action(Action::ShowLimits)
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
        let result = LimitsCommand.run(&mut ctx, "extra");
        assert!(
            matches!(result, CommandResult::Error(ref e) if e.contains("--json")
                && e.contains("stay-supergrok")
                && e.contains("use-console")
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

        let stay = crate::limits_cmd::LIMITS_WORD_STAY_SUPERGROK;
        let use_console = crate::limits_cmd::LIMITS_WORD_USE_CONSOLE;
        let meter = crate::limits_cmd::LIMITS_WORD_METER;
        let refresh = crate::limits_cmd::LIMITS_WORD_REFRESH;
        assert_eq!(stay, "stay-supergrok");
        assert_eq!(use_console, "use-console");
        assert_eq!(meter, "meter");
        assert_eq!(refresh, "refresh");

        let models = ModelState::default();
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
}
