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
Follow the host skill at ~/.agents/skills/what/SKILL.md (slash /what). \
The operator cannot parse the last agent chat. Restate. Do not apologize. \
Do not write a file. Do not spawn. \
Reply with this shape only, four labeled lines, nothing fluffier: \
Job: one sentence, what this session is trying to do right now. \
State: what is actually happening (running, waiting, blocked, done). Name the real product thing. No process jargon unless the operator already used it. \
You: what the operator must do, or the word nothing if they do not need to do anything. Then say why. Name the evidence. Do not use an unexplained heuristic. \
Next: the next concrete step the agent will take. \
Plain American English. Short sentences. \
No residual codes, board ids, or implement-run hex as the body. \
No say the word if you want me to continue when the next step is already clear. \
If the last assistant message was jargon (nix_retry, quality-deps, NAR, L2/L3 unless they asked), translate it. \
This is not /recap (chat recap), not /finish (post-mortem), and not /reports (checkpoint file)."
    )
}

/// Restate this session in four labeled lines.
pub struct WhatCommand;

impl SlashCommand for WhatCommand {
    fn name(&self) -> &str {
        "what"
    }

    fn description(&self) -> &str {
        "Restate job, state, what you must do, and next"
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
            text.contains("~/.agents/skills/what/SKILL.md"),
            "inject host overlay skill, not repo .agents/skills/what; got {text}"
        );
        assert!(
            text.contains("host skill"),
            "must name the host overlay, not a repo skill pack; got {text}"
        );
        assert!(
            !text.contains("product skill at .agents/skills/what"),
            "must not point at the repo skill pack; got {text}"
        );
        assert!(text.contains("Job:"), "{text}");
        assert!(text.contains("State:"), "{text}");
        assert!(text.contains("You:"), "{text}");
        assert!(
            text.contains("Name the evidence"),
            "You must require evidence, not a bare heuristic; got {text}"
        );
        assert!(text.contains("Next:"), "{text}");
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
