//! `/limits` — SuperGrok included / dollar extras / console path detail.
//!
//! Multi-line detail for spend meters. Session token/cost stays on `/usage`
//! (`/cost`). Footer stays one-line; this is the full snapshot.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

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
        "/limits"
    }

    /// Works once an agent view exists (billing cache is app/agent scoped).
    fn session_scoped(&self) -> bool {
        true
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if !args.trim().is_empty() {
            return CommandResult::Error("Usage: /limits (no arguments)".to_string());
        }
        CommandResult::Action(Action::ShowLimits)
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
    fn limits_command_rejects_args() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let result = LimitsCommand.run(&mut ctx, "extra");
        assert!(matches!(result, CommandResult::Error(_)));
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
