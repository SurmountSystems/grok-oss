//! `session_reader`: first-class tool wrapping `util::session_reader`.
//!
//! Default agent-facing list output is TOON via [`crate::util::toon::encode`].
//! `json: true` / host `--json` stays pretty JSON for host parity with
//! `session_reader.py --json`. Bash intercept of the Python CLI form is
//! unchanged.

use std::path::Path;

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::util::session_reader::{
    SessionAction, SessionReaderIntercept, SessionTool, execute_intercept,
};
use crate::util::toon;
use serde_json::Value;

/// Stable client-facing tool id.
pub const SESSION_READER_TOOL_NAME: &str = "session_reader";

/// List or show a foreign-session transcript.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SessionReaderInput {
    /// `claude` | `codex` | `cursor`.
    #[schemars(description = "Session tool: claude, codex, or cursor.")]
    pub tool: String,
    /// `list` | `show`.
    #[schemars(description = "Action: list or show.")]
    pub action: String,
    /// Host-parity JSON. Default (unset/false) is TOON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "If true, print JSON for host parity. Default is TOON.")]
    pub json: Option<bool>,
}

/// Model-facing result. Default encoding is TOON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionReaderOutput {
    pub stdout: String,
}

impl xai_tool_runtime::ToolOutput for SessionReaderOutput {}

impl From<SessionReaderInput> for ToolInput {
    fn from(input: SessionReaderInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({
            "tool": input.tool,
            "action": input.action,
            "json": input.json,
        }))
    }
}

impl From<SessionReaderOutput> for ToolOutput {
    fn from(o: SessionReaderOutput) -> Self {
        ToolOutput::Text(TextOutput::from(o.stdout))
    }
}

/// Default list (agent-facing TOON). Wraps [`execute_intercept`] JSON then encodes.
#[cfg(test)]
fn list_tool_default(cwd: &Path) -> String {
    list_as_toon(cwd, SessionTool::Claude)
}

fn list_as_toon(cwd: &Path, tool: SessionTool) -> String {
    match list_as_json_value(cwd, tool) {
        Ok(v) => toon::encode(&v).unwrap_or_else(|_| toon::compact_json(&v)),
        Err(_) => {
            let fallback = serde_json::json!({
                "tool": tool.as_str(),
                "cwd": cwd.display().to_string(),
                "sessions": [],
                "warnings": [],
            });
            toon::encode(&fallback).unwrap_or_else(|_| toon::compact_json(&fallback))
        }
    }
}

fn list_as_json_value(cwd: &Path, tool: SessionTool) -> Result<Value, String> {
    let stdout = list_json_stdout(cwd, tool);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err("empty list stdout".into());
    }
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

fn list_json_stdout(cwd: &Path, tool: SessionTool) -> String {
    let intercept = SessionReaderIntercept {
        script_path: "session_reader".into(),
        tool,
        action: SessionAction::List,
        ref_arg: None,
        cwd: Some(cwd.to_string_lossy().into_owned()),
        within_min: 0,
        json: true,
        max_tool_chars: 300,
    };
    execute_intercept(&intercept, cwd).stdout
}

/// `json: true` / `--json` stays JSON via the existing util intercept.
#[cfg(test)]
fn list_tool_json_flag(cwd: &Path) -> String {
    list_json_stdout(cwd, SessionTool::Claude)
}

/// Agent tool wrapping `util::session_reader`.
#[derive(Debug, Default)]
pub struct SessionReaderTool;

impl crate::types::tool_metadata::ToolMetadata for SessionReaderTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn description_template(&self) -> &str {
        r#"List or show a foreign-session transcript (claude/codex/cursor). Default output is TOON. json: true stays JSON for host parity with session_reader.py --json."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for SessionReaderTool {
    type Args = SessionReaderInput;
    type Output = SessionReaderOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(SESSION_READER_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            SESSION_READER_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    async fn run(
        &self,
        _ctx: xai_tool_runtime::ToolCallContext,
        input: SessionReaderInput,
    ) -> Result<SessionReaderOutput, xai_tool_runtime::ToolError> {
        let cwd = Path::new(".");
        let tool = parse_session_tool(&input.tool)
            .map_err(xai_tool_runtime::ToolError::invalid_arguments)?;
        let action = parse_session_action(&input.action)
            .map_err(xai_tool_runtime::ToolError::invalid_arguments)?;
        let json_flag = input.json.unwrap_or(false);
        let intercept = SessionReaderIntercept {
            script_path: "session_reader".into(),
            tool,
            action,
            ref_arg: None,
            cwd: Some(cwd.to_string_lossy().into_owned()),
            within_min: 0,
            json: true,
            max_tool_chars: 300,
        };
        let result = execute_intercept(&intercept, cwd);
        if result.exit_code != 0 {
            return Err(xai_tool_runtime::ToolError::execution(
                self.id(),
                result.stderr,
            ));
        }
        let stdout = if json_flag {
            result.stdout
        } else {
            match serde_json::from_str::<Value>(result.stdout.trim()) {
                Ok(v) => toon::encode(&v).unwrap_or_else(|_| toon::compact_json(&v)),
                Err(_) => list_as_toon(cwd, tool),
            }
        };
        Ok(SessionReaderOutput { stdout })
    }
}

fn parse_session_tool(raw: &str) -> Result<SessionTool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "claude" => Ok(SessionTool::Claude),
        "codex" => Ok(SessionTool::Codex),
        "cursor" => Ok(SessionTool::Cursor),
        other => Err(format!(
            "unknown tool '{other}'; expected claude | codex | cursor"
        )),
    }
}

fn parse_session_action(raw: &str) -> Result<SessionAction, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "list" => Ok(SessionAction::List),
        "show" => Ok(SessionAction::Show),
        other => Err(format!("unknown action '{other}'; expected list | show")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_reader_tool_list_is_toon() {
        let out = list_tool_default(Path::new("/tmp"));
        assert!(
            !out.trim_start().starts_with('{'),
            "default session_reader list must be TOON, not JSON: {out}"
        );
        assert!(
            out.contains("tool:") || out.contains("sessions:"),
            "TOON list keys, got: {out}"
        );
    }

    #[test]
    fn session_reader_json_flag_still_json_for_host_parity() {
        let out = list_tool_json_flag(Path::new("/tmp"));
        let trimmed = out.trim();
        assert!(
            trimmed.starts_with('{'),
            "json: true must stay JSON for host parity, got: {out}"
        );
        let v: serde_json::Value =
            serde_json::from_str(trimmed).expect("json flag must parse as JSON");
        assert!(v.is_object(), "host-parity list is a JSON object: {out}");
        assert!(
            v.get("tool").is_some() || v.get("sessions").is_some(),
            "host-parity JSON keeps tool/sessions, got: {out}"
        );
    }
}
