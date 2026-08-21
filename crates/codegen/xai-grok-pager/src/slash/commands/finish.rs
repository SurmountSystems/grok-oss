//! `/finish`: structured session post-mortem. Not `/dream`, not `/recap`.

use agent_client_protocol as acp;

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::queue_schedule::{queue_later_skill, split_schedule_token};

/// Instruction the agent follows when the operator types `/finish`.
pub fn finish_instruction(focus: &str) -> String {
    let focus_line = if focus.is_empty() {
        String::new()
    } else {
        format!("Optional focus from the operator: {focus}\n\n")
    };
    format!(
        "{focus_line}\
Follow the host skill at ~/.agents/skills/finish/SKILL.md (slash /finish). \
Write a structured post-mortem for this session. Work continues. \
A wrap often reveals more features worth adding. The product is not finished forever. \
Document what shipped, leftover, and useful next features as first-class sections. \
Write one markdown file under ~/.agents/reports/ named finish-YYYY-MM-DD.md \
(or finish-YYYY-MM-DD-<short-session>.md if that dated name already exists). \
Complete American English. No secrets. No em dashes. \
This is not /dream (memory consolidation) and not /recap (chat recap). \
Do not git add, commit, push, or rebuild."
    )
}

/// Wrap a long session: inject the finish skill prompt.
pub struct FinishCommand;

impl SlashCommand for FinishCommand {
    fn name(&self) -> &str {
        "finish"
    }

    fn description(&self) -> &str {
        "Write a post-mortem; leftover and next features stay first-class"
    }

    fn usage(&self) -> &str {
        "/finish [queue|later] [optional focus]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("optional focus")
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let (hold, focus) = split_schedule_token(args);
        let display_text = if focus.is_empty() {
            "/finish".to_string()
        } else {
            format!("/finish {focus}")
        };
        if hold {
            return queue_later_skill(
                display_text,
                vec![acp::ContentBlock::Text(acp::TextContent::new(
                    finish_instruction(focus),
                ))],
            );
        }
        CommandResult::InjectSkill {
            display_text,
            prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
                finish_instruction(focus),
            ))],
            display_as_skill: true,
            scheduled_task_preview: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;

    fn text_of(result: &CommandResult) -> &str {
        match result {
            CommandResult::InjectSkill { prompt_blocks, .. } => match &prompt_blocks[0] {
                acp::ContentBlock::Text(t) => &t.text,
                _ => panic!("expected Text block"),
            },
            other => panic!("expected InjectSkill, got {other:?}"),
        }
    }

    #[test]
    fn finish_command_name() {
        assert_eq!(FinishCommand.name(), "finish");
        assert!(FinishCommand.aliases().is_empty());
    }

    #[test]
    fn finish_empty_args_injects_postmortem_skill() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let result = FinishCommand.run(&mut ctx, "");
        match &result {
            CommandResult::InjectSkill {
                display_text,
                display_as_skill,
                ..
            } => {
                assert_eq!(display_text, "/finish");
                assert!(
                    *display_as_skill,
                    "/finish is a skill wrap, not a fake builtin prompt"
                );
            }
            other => panic!("expected InjectSkill, got {other:?}"),
        }
        let text = text_of(&result);
        assert!(text.contains("post-mortem"), "{text}");
        assert!(text.contains("~/.agents/reports/"), "{text}");
        assert!(text.contains("finish-YYYY-MM-DD.md"), "{text}");
        assert!(text.contains("not /dream"), "{text}");
        assert!(text.contains("not /recap"), "{text}");
        assert!(
            !text.contains('\u{2014}') && !text.contains(" -- "),
            "operator-facing instruction must not use em dashes; got {text}"
        );
    }

    #[test]
    fn finish_optional_focus_is_in_injected_prompt() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let result = FinishCommand.run(&mut ctx, "  pager slashes  ");
        match &result {
            CommandResult::InjectSkill { display_text, .. } => {
                assert_eq!(display_text, "/finish pager slashes");
            }
            other => panic!("expected InjectSkill, got {other:?}"),
        }
        let text = text_of(&result);
        assert!(text.contains("pager slashes"), "{text}");
    }

    #[test]
    fn finish_registered_in_builtins() {
        let names: Vec<_> = crate::slash::commands::builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "finish"),
            "expected /finish in builtins, got {names:?}"
        );
    }

    #[test]
    fn finish_skill_copy_does_not_say_work_is_closed_forever() {
        let text = finish_instruction("");
        let lower = text.to_lowercase();
        assert!(text.contains("leftover"), "{text}");
        assert!(
            text.contains("next features") || text.contains("more features"),
            "leftover and next features stay first-class; got {text}"
        );
        assert!(
            lower.contains("work continues")
                || lower.contains("not finished forever")
                || lower.contains("not closed forever"),
            "wrap must not treat the session as finished forever; got {text}"
        );
        assert!(
            !lower.contains("closed forever") && !lower.contains("the project is done"),
            "must not say the work is closed forever; got {text}"
        );
        assert!(
            !lower.contains("ties the bow"),
            "must not frame the wrap as a final bow; got {text}"
        );
    }
}
