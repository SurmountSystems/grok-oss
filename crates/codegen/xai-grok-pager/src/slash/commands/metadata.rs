//! `/metadata` — live session ids and context. Not `/session-info`.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Honest live fields for the `/metadata` transcript block.
#[derive(Debug, Clone, Default)]
pub struct SessionMetadataFields {
    pub session_ulid: Option<String>,
    pub session_uuid: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub started: Option<String>,
    pub pid: u32,
    /// When true, list the grok-oss ULID before the Grok Build UUID.
    pub ulid_primary: bool,
}

/// Format a system-block report. Omit unknown fields. Do not invent meters.
pub fn format_session_metadata(fields: &SessionMetadataFields) -> String {
    let mut lines = vec!["Session metadata".to_string()];
    let ulid_line = fields
        .session_ulid
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|id| format!("  grok-oss ULID: {id}"));
    let uuid_line = fields
        .session_uuid
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|id| format!("  Grok Build UUID: {id}"));
    if fields.ulid_primary {
        if let Some(line) = ulid_line {
            lines.push(line);
        }
        if let Some(line) = uuid_line {
            lines.push(line);
        }
    } else {
        if let Some(line) = uuid_line {
            lines.push(line);
        }
        if let Some(line) = ulid_line {
            lines.push(line);
        }
    }
    if let Some(cwd) = fields.cwd.as_deref().filter(|s| !s.is_empty()) {
        lines.push(format!("  cwd: {cwd}"));
    }
    if let Some(model) = fields.model.as_deref().filter(|s| !s.is_empty()) {
        lines.push(format!("  model: {model}"));
    }
    if let Some(started) = fields.started.as_deref().filter(|s| !s.is_empty()) {
        lines.push(format!("  started: {started}"));
    }
    lines.push(format!("  pid: {}", fields.pid));
    lines.join("\n")
}

/// Show live session metadata (ULID, UUID, cwd, model, started, pid).
pub struct MetadataCommand;

impl SlashCommand for MetadataCommand {
    fn name(&self) -> &str {
        "metadata"
    }

    fn description(&self) -> &str {
        "Show live session metadata including ULID and UUID"
    }

    fn usage(&self) -> &str {
        "/metadata"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let arg = args.trim();
        if arg.is_empty() {
            CommandResult::Action(Action::ShowSessionMetadata)
        } else {
            CommandResult::Error(format!("Unknown argument: {arg}. Use /metadata (no args)."))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;

    #[test]
    fn metadata_command_emits_show_session_metadata() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let result = MetadataCommand.run(&mut ctx, "");
        assert!(
            matches!(result, CommandResult::Action(Action::ShowSessionMetadata)),
            "{result:?}"
        );
    }

    #[test]
    fn metadata_unknown_args_are_an_error() {
        let models = ModelState::default();
        let mut ctx = crate::slash::commands::tests::make_ctx(&models);
        let result = MetadataCommand.run(&mut ctx, "json");
        assert!(matches!(result, CommandResult::Error(_)), "{result:?}");
    }

    #[test]
    fn metadata_registered_in_builtins() {
        let names: Vec<_> = crate::slash::commands::builtin_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "metadata"),
            "expected /metadata in builtins, got {names:?}"
        );
    }

    #[test]
    fn format_lists_ulid_before_uuid_when_primary() {
        let text = format_session_metadata(&SessionMetadataFields {
            session_ulid: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".into()),
            session_uuid: Some("018f1e2a-3b4c-7d8e-9f01-23456789abcd".into()),
            cwd: Some("/tmp/meta".into()),
            model: Some("grok-4".into()),
            started: Some("2026-08-20T12:00:00Z".into()),
            pid: 4242,
            ulid_primary: true,
        });
        let ulid_at = text.find("grok-oss ULID").expect("ulid line");
        let uuid_at = text.find("Grok Build UUID").expect("uuid line");
        assert!(ulid_at < uuid_at, "{text}");
        assert!(text.contains("cwd: /tmp/meta"), "{text}");
        assert!(text.contains("model: grok-4"), "{text}");
        assert!(text.contains("started: 2026-08-20T12:00:00Z"), "{text}");
        assert!(text.contains("pid: 4242"), "{text}");
        assert!(
            !text.contains('\u{2014}'),
            "no em dashes in operator-facing metadata; got {text}"
        );
    }

    #[test]
    fn format_lists_uuid_first_when_ulid_primary_is_off() {
        let text = format_session_metadata(&SessionMetadataFields {
            session_ulid: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".into()),
            session_uuid: Some("018f1e2a-3b4c-7d8e-9f01-23456789abcd".into()),
            cwd: None,
            model: None,
            started: None,
            pid: 7,
            ulid_primary: false,
        });
        let ulid_at = text.find("grok-oss ULID").expect("ulid line");
        let uuid_at = text.find("Grok Build UUID").expect("uuid line");
        assert!(uuid_at < ulid_at, "{text}");
        assert!(text.contains("pid: 7"), "{text}");
        assert!(!text.contains("cwd:"), "{text}");
        assert!(!text.contains("model:"), "{text}");
        assert!(!text.contains("started:"), "{text}");
    }

    #[test]
    fn format_omits_unknown_id_fields() {
        let text = format_session_metadata(&SessionMetadataFields {
            session_ulid: None,
            session_uuid: Some("018f1e2a-3b4c-7d8e-9f01-23456789abcd".into()),
            cwd: Some("/tmp/only-uuid".into()),
            model: None,
            started: None,
            pid: 9,
            ulid_primary: true,
        });
        assert!(!text.contains("grok-oss ULID"), "{text}");
        assert!(text.contains("Grok Build UUID"), "{text}");
        assert!(text.contains("pid: 9"), "{text}");
    }
}
