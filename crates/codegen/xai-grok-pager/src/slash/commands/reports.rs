//! `/reports`: checkpoint report while work continues. Not `/finish`, not `/dream`, not `/recap`.

use agent_client_protocol as acp;

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::queue_schedule::{queue_later_skill, split_schedule_token};

/// Instruction the agent follows when the operator types `/reports`.
pub fn reports_instruction(focus: &str) -> String {
    let focus_line = if focus.is_empty() {
        String::new()
    } else {
        format!("Optional focus from the operator: {focus}\n\n")
    };
    format!(
        "{focus_line}\
Follow the host skill at ~/.agents/skills/reports/SKILL.md (slash /reports). \
Write a checkpoint of what landed so far, leftover, and useful next features. \
Work continues. This is not a wrap that says the project is done. \
Write one markdown file under ~/.agents/reports/ named reports-YYYY-MM-DD.md \
(or reports-YYYY-MM-DD-<short-session>.md if that dated name already exists). \
Complete American English. No secrets. No em dashes. \
This is not /finish (session post-mortem), not /dream (memory consolidation), and not /recap (chat recap). \
Do not git add, commit, push, or rebuild."
    )
}

fn inject_reports(focus: &str) -> CommandResult {
    let display_text = if focus.is_empty() {
        "/reports".to_string()
    } else {
        format!("/reports {focus}")
    };
    CommandResult::InjectSkill {
        display_text,
        prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
            reports_instruction(focus),
        ))],
        display_as_skill: true,
        scheduled_task_preview: None,
    }
}

/// Checkpoint report: inject the reports skill prompt.
pub struct ReportsCommand;

impl SlashCommand for ReportsCommand {
    fn name(&self) -> &str {
        "reports"
    }

    fn description(&self) -> &str {
        "Write a checkpoint report; work continues"
    }

    fn usage(&self) -> &str {
        "/reports [queue|later] [optional focus]"
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
        if hold {
            let display_text = if focus.is_empty() {
                "/reports".to_string()
            } else {
                format!("/reports {focus}")
            };
            return queue_later_skill(
                display_text,
                vec![acp::ContentBlock::Text(acp::TextContent::new(
                    reports_instruction(focus),
                ))],
            );
        }
        inject_reports(focus)
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
    fn reports_command_name() {
        assert_eq!(ReportsCommand.name(), "reports");
        assert!(ReportsCommand.aliases().is_empty());
    }

    #[test]
    fn reports_empty_args_injects_reports_skill() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let result = ReportsCommand.run(&mut ctx, "");
        match &result {
            CommandResult::InjectSkill {
                display_text,
                display_as_skill,
                ..
            } => {
                assert_eq!(display_text, "/reports");
                assert!(
                    *display_as_skill,
                    "/reports is a skill wrap, not a fake builtin prompt"
                );
            }
            other => panic!("expected InjectSkill, got {other:?}"),
        }
        let text = text_of(&result);
        assert!(text.contains("~/.agents/skills/reports/SKILL.md"), "{text}");
        assert!(text.contains("~/.agents/reports/"), "{text}");
        assert!(text.contains("reports-YYYY-MM-DD.md"), "{text}");
        assert!(
            text.contains("work continues") || text.contains("Work continues"),
            "{text}"
        );
        assert!(text.contains("not /finish"), "{text}");
        assert!(text.contains("not /dream"), "{text}");
        assert!(text.contains("not /recap"), "{text}");
        assert!(
            !text.contains('\u{2014}') && !text.contains(" -- "),
            "operator-facing instruction must not use em dashes; got {text}"
        );
    }

    #[test]
    fn reports_queue_token_holds_without_injecting_now() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        match ReportsCommand.run(&mut ctx, "queue") {
            CommandResult::QueueLater {
                text,
                as_command,
                display_as_skill,
                wire_blocks,
            } => {
                assert_eq!(text, "/reports");
                assert!(!as_command);
                assert!(display_as_skill);
                assert!(wire_blocks.is_some());
            }
            other => panic!("expected QueueLater, got {other:?}"),
        }
    }
}
