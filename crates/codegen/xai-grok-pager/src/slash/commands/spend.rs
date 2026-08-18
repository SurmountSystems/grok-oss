//! `/spend` — double-entry local vs remote spend books (Token Economy).

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Show local calculated spend vs Management remote book + gap honesty.
pub struct SpendCommand;

impl SlashCommand for SpendCommand {
    fn name(&self) -> &str {
        "spend"
    }

    fn aliases(&self) -> &[&str] {
        &["double-entry", "ledger"]
    }

    fn description(&self) -> &str {
        "View local vs Management spend (double-entry)"
    }

    fn usage(&self) -> &str {
        "/spend"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let arg = args.trim();
        if arg.is_empty() {
            CommandResult::Action(Action::ShowSpend)
        } else {
            CommandResult::Error(format!("Unknown argument: {arg}. Use /spend (no args)."))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
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
    fn spend_command_emits_show_spend() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let result = SpendCommand.run(&mut ctx, "");
        assert!(
            matches!(result, CommandResult::Action(Action::ShowSpend)),
            "{result:?}"
        );
    }

    #[test]
    fn spend_registered_in_builtins() {
        let names: Vec<_> = crate::slash::commands::builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "spend"),
            "expected /spend in builtins, got {names:?}"
        );
    }
}
