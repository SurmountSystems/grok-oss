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
Job: one sentence, the real product outcome this session is trying to finish right now. \
State: running, waiting, blocked, or done. Name the real file, command, crate, or test. Translate leftover jargon into ordinary words. \
Operator: the operator action, or the word nothing if they do not need to act. Then say why. Name the evidence. Do not leave a bare nothing. \
Next: the next concrete agent step. \
Prefer Operator and Agent as speaker labels. Do not say You or Human for the operator. Do not say Me or Grok as the speaker label for the machine. \
Painted chrome and user-guide call the composer the Operator box. \
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
        "Restate Job, State, Operator, and Next"
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

    // Grok OSS: /what injects the in-tree default skill and requires Job, State, Operator, Next plus Name the evidence. This diverges from upstream xAI because FORK.md /what restatement pins those four complete thoughts. Speaker-label sentences live on what_instruction_prefers_operator_and_agent_speaker_labels.
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
        assert!(text.contains("Job: one sentence"), "{text}");
        assert!(
            text.contains("State: running, waiting, blocked, or done"),
            "{text}"
        );
        assert!(text.contains("Operator: the operator action"), "{text}");
        assert!(
            text.contains("Name the evidence"),
            "Operator must require evidence; got {text}"
        );
        assert!(
            text.contains("Next: the next concrete agent step"),
            "{text}"
        );
        assert!(text.contains("not /recap"), "{text}");
        assert!(text.contains("not /finish"), "{text}");
        assert!(text.contains("not /reports"), "{text}");
        assert!(
            !text.contains('\u{2014}') && !text.contains(" -- "),
            "operator-facing instruction must not use em dashes; got {text}"
        );
    }

    fn assert_operator_agent_speaker_labels(text: &str) {
        assert!(
            text.contains("Prefer Operator and Agent as speaker labels"),
            "prefer Operator and Agent; got {text}"
        );
        assert!(
            text.contains("Do not say You or Human for the operator"),
            "forbid You or Human for the operator; got {text}"
        );
        assert!(
            text.contains("Do not say Me or Grok as the speaker label for the machine"),
            "forbid Me or Grok as the speaker label; got {text}"
        );
        assert!(
            text.contains("call the composer the Operator box"),
            "painted chrome names the Operator box; got {text}"
        );
        assert!(
            !text.contains("Do not rename the product composer Human box"),
            "old Human box rename ban is superseded; got {text}"
        );
        assert!(text.contains("Job: one sentence"), "{text}");
        assert!(
            text.contains("State: running, waiting, blocked, or done"),
            "{text}"
        );
        assert!(text.contains("Operator: the operator action"), "{text}");
        assert!(
            text.contains("Next: the next concrete agent step"),
            "{text}"
        );
    }

    // Grok OSS: speaker labels are Operator / Agent. This diverges from upstream xAI You/Human / Me/Grok copy because the Operator said so.
    #[test]
    fn what_instruction_prefers_operator_and_agent_speaker_labels() {
        let text = what_instruction("");
        assert_operator_agent_speaker_labels(&text);
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let injected_result = WhatCommand.run(&mut ctx, "");
        let injected = text_of(&injected_result);
        assert_operator_agent_speaker_labels(injected);
        let skill_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../xai-grok-bundle/skills/what/SKILL.md");
        let skill = std::fs::read_to_string(&skill_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", skill_path.display()));
        // Load path is the in-tree bundle skill (CARGO_MANIFEST_DIR sibling
        // xai-grok-bundle/skills/what/SKILL.md). Nix quality uses that same
        // source. Speaker-label contracts live in the markdown body after
        // YAML frontmatter, not in the `---` block.
        let skill_body = skill
            .split_once("\n---\n")
            .map(|(_, body)| body)
            .unwrap_or(skill.as_str());
        assert!(
            skill_body.contains("Prefer Operator and Agent as speaker labels"),
            "what skill must prefer Operator and Agent; got {skill}"
        );
        assert!(
            skill_body.contains("Do not say You or Human for the operator"),
            "what skill must forbid You or Human; got {skill}"
        );
        assert!(
            skill_body.contains("Do not say Me or Grok as the speaker label for the machine"),
            "what skill must forbid Me or Grok as the speaker label; got {skill}"
        );
        assert!(
            skill_body.contains("Job / State / Operator / Next"),
            "what skill must keep Job / State / Operator / Next; got {skill}"
        );
        assert!(
            skill_body.contains("call the composer the Operator box"),
            "what skill must name the Operator box; got {skill}"
        );
        assert!(
            !skill_body.contains("Do not rename the product composer Human box"),
            "what skill must not keep the old Human box rename ban; got {skill}"
        );
    }

    // Grok OSS: leftover remaining-work said This debugger is Grok Build 1.0.13
    // and that line was treated as this window's grok-oss version. Isolated
    // Preview and plan chrome are grok-oss unless this process was launched as
    // grok from downloads. Probe this turn. Do not reuse leftover 1.0.13.
    #[test]
    fn what_skill_does_not_mix_grok_build_version_with_grok_oss() {
        let skill_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../xai-grok-bundle/skills/what/SKILL.md");
        let skill = std::fs::read_to_string(&skill_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", skill_path.display()));
        let skill_body = skill
            .split_once("\n---\n")
            .map(|(_, body)| body)
            .unwrap_or(skill.as_str());
        assert!(
            skill_body.contains("Never mix Grok Build version with grok-oss product version"),
            "what skill must keep grok-oss and Grok Build versions distinct; got {skill}"
        );
        assert!(
            skill_body.contains("grok-oss --version"),
            "what skill must name grok-oss --version; got {skill}"
        );
        assert!(
            skill_body.contains("grok --version"),
            "what skill must name grok --version; got {skill}"
        );
        assert!(
            skill_body.contains("Isolated Preview and plan chrome are grok-oss"),
            "what skill must name Isolated Preview chrome as grok-oss unless launched as grok; got {skill}"
        );
        assert!(
            skill_body.contains("probe this turn"),
            "what skill must probe this turn instead of leftover 1.0.13; got {skill}"
        );
        assert!(
            !skill_body.contains("This debugger is Grok Build 1.0.13"),
            "what skill must not teach leftover Grok Build 1.0.13 as grok-oss; got {skill}"
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

    // Grok OSS: /what is registered among pager builtins. This diverges from upstream xAI because FORK.md lists what_registered_in_builtins as a /what inventory named test.
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
