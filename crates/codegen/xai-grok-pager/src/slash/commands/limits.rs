//! `/limits` — SuperGrok included / dollar extras / console path detail.
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
        "View SuperGrok included, dollar extras, and console limits"
    }

    fn usage(&self) -> &str {
        "/limits [--json]"
    }

    /// Works once an agent view exists (billing cache is app/agent scoped).
    fn session_scoped(&self) -> bool {
        true
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn suggest_args(&self, _ctx: &AppCtx, _args_query: &str) -> Option<Vec<ArgItem>> {
        Some(vec![ArgItem {
            display: "--json".into(),
            match_text: "--json".into(),
            insert_text: "--json".into(),
            description: "Print JSON to chat (same as grok limits --json)".into(),
        }])
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let arg = args.trim();
        match arg {
            "" => CommandResult::Action(Action::ShowLimits),
            "--json" | "json" => CommandResult::Action(Action::ShowLimitsJson),
            _ => CommandResult::Error(format!(
                "Unknown argument: {arg}. Use /limits or /limits --json"
            )),
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
            matches!(result, CommandResult::Error(ref e) if e.contains("--json")),
            "expected usage error mentioning --json, got {result:?}"
        );
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
}
