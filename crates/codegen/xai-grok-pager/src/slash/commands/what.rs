//! `/what`: four-line restatement when the operator cannot parse agent chat.
//! Not `/recap`, not `/finish`, not `/reports`.

use agent_client_protocol as acp;

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Instruction the agent follows when the operator types `/what`.
pub fn what_instruction(focus: &str) -> String {
    let focus_line = if focus.is_empty() {
        String::new()
    } else {
        format!("Optional focus from the operator: {focus}\n\n")
    };
    format!(
        "{focus_line}\
Follow the default Grok OSS skill at crates/codegen/xai-grok-bundle/skills/what/SKILL.md \
(installed into ~/.grok/bundled/skills/what/). Slash /what. \
The live cache is not the source. Do not use a repo or host overlay skill pack for this slash. \
The operator cannot parse the last agent chat. Restate. Do not apologize. \
Do not write a file. Do not spawn. \
Reply with this shape only, four labeled complete thoughts, nothing fluffier: \
What we are doing: one sentence, the real product outcome this session is trying to finish right now. \
What is true right now: running, waiting, blocked, or done. Name the real file, command, crate, or test. Translate leftover jargon into ordinary words. \
What you need to do: the operator action, or the word nothing if they do not need to act. Then say why. Name the evidence. Do not leave a bare nothing. \
What I will do next: the next concrete agent step. \
Complete American English thoughts. Short sentences. \
No leftover board ids or hex run ids in the body. \
No say the word if you want me to continue when the next step is already clear. \
Follow Concise American Technical English as specified in Surmount 0005_CATE.md. \
This is not /recap (chat recap), not /finish (post-mortem), and not /reports (checkpoint file)."
    )
}

/// Restate this session in four complete thoughts.
pub struct WhatCommand;

impl SlashCommand for WhatCommand {
    fn name(&self) -> &str {
        "what"
    }

    fn description(&self) -> &str {
        "Restate what we are doing, what is true, what you need to do, and what I will do next"
    }

    fn usage(&self) -> &str {
        "/what [optional focus]"
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
        let focus = args.trim();
        let display_text = if focus.is_empty() {
            "/what".to_string()
        } else {
            format!("/what {focus}")
        };
        CommandResult::InjectSkill {
            display_text,
            prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
                what_instruction(focus),
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
    fn what_command_name() {
        assert_eq!(WhatCommand.name(), "what");
        assert!(WhatCommand.aliases().is_empty());
    }

    #[test]
    fn what_empty_args_injects_what_skill() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let result = WhatCommand.run(&mut ctx, "");
        match &result {
            CommandResult::InjectSkill {
                display_text,
                display_as_skill,
                ..
            } => {
                assert_eq!(display_text, "/what");
                assert!(
                    *display_as_skill,
                    "/what is a skill wrap, not a fake builtin prompt"
                );
            }
            other => panic!("expected InjectSkill, got {other:?}"),
        }
        let text = text_of(&result);
        assert!(
            text.contains("crates/codegen/xai-grok-bundle/skills/what"),
            "inject the in-tree Grok OSS skill, not repo .agents/skills/what; got {text}"
        );
        assert!(
            text.contains("bundled/skills/what"),
            "must name the bundled install path; got {text}"
        );
        assert!(
            text.contains("default Grok OSS skill"),
            "must name the default product skill; got {text}"
        );
        assert!(
            !text.contains(".agents/skills/what"),
            "must not point at repo or host .agents/skills/what as the grok-oss source; got {text}"
        );
        assert!(text.contains("What we are doing:"), "{text}");
        assert!(text.contains("What is true right now:"), "{text}");
        assert!(text.contains("What you need to do:"), "{text}");
        assert!(
            text.contains("Name the evidence"),
            "What you need to do must require evidence; got {text}"
        );
        assert!(text.contains("What I will do next:"), "{text}");
        assert!(text.contains("not /recap"), "{text}");
        assert!(text.contains("not /finish"), "{text}");
        assert!(text.contains("not /reports"), "{text}");
        assert!(
            !text.contains('\u{2014}') && !text.contains(" -- "),
            "operator-facing instruction must not use em dashes; got {text}"
        );
    }

    #[test]
    fn what_optional_focus_is_in_injected_prompt() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let result = WhatCommand.run(&mut ctx, "  the last status  ");
        match &result {
            CommandResult::InjectSkill { display_text, .. } => {
                assert_eq!(display_text, "/what the last status");
            }
            other => panic!("expected InjectSkill, got {other:?}"),
        }
        let text = text_of(&result);
        assert!(text.contains("the last status"), "{text}");
    }

    #[test]
    fn what_registered_in_builtins() {
        let names: Vec<_> = crate::slash::commands::builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "what"),
            "expected /what in builtins, got {names:?}"
        );
    }
}
