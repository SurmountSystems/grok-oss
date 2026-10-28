//! `session_usage`: read session `usage.jsonl` and print TOON.
//!
//! File reader only. Does not open the token ledger, SQLite books, or
//! `l2_token_tracking`. On-disk JSONL stays JSONL. Null `output_tokens` is
//! omitted, not printed as `0`. Default agent-facing output is TOON via
//! [`crate::util::toon::encode`].

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::util::toon;
use serde_json::{Value, json};

/// Stable client-facing tool id.
pub const SESSION_USAGE_TOOL_NAME: &str = "session_usage";

/// Path to a `usage.jsonl` file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SessionUsageInput {
    /// Path to `usage.jsonl`.
    #[schemars(description = "Path to a session usage.jsonl file.")]
    pub path: String,
}

/// Model-facing result. Default encoding is TOON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionUsageOutput {
    pub stdout: String,
}

impl xai_tool_runtime::ToolOutput for SessionUsageOutput {}

impl From<SessionUsageInput> for ToolInput {
    fn from(input: SessionUsageInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({ "path": input.path }))
    }
}

impl From<SessionUsageOutput> for ToolOutput {
    fn from(o: SessionUsageOutput) -> Self {
        ToolOutput::Text(TextOutput::from(o.stdout))
    }
}

/// Render JSONL usage events as TOON. Null `output_tokens` is omitted, not `0`.
pub fn usage_toon_from_jsonl(jsonl: &str) -> String {
    let mut events = Vec::new();
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        events.push(omit_null_output_tokens(parsed));
    }
    let wrapped = json!({ "events": events });
    toon::encode(&wrapped).unwrap_or_else(|_| toon::compact_json(&wrapped))
}

fn omit_null_output_tokens(value: Value) -> Value {
    let Value::Object(mut map) = value else {
        return value;
    };
    if matches!(map.get("output_tokens"), Some(Value::Null)) {
        map.remove("output_tokens");
    }
    Value::Object(map)
}

/// Agent tool: session usage.jsonl reader (not the SQLite ledger).
#[derive(Debug, Default)]
pub struct SessionUsageTool;

impl crate::types::tool_metadata::ToolMetadata for SessionUsageTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn lock_path_param(&self) -> Option<&'static str> {
        Some("path")
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn description_template(&self) -> &str {
        r#"Read a session usage.jsonl file and print TOON. Null output_tokens is omitted, not printed as 0. Does not open the token ledger."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for SessionUsageTool {
    type Args = SessionUsageInput;
    type Output = SessionUsageOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(SESSION_USAGE_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            SESSION_USAGE_TOOL_NAME,
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
        input: SessionUsageInput,
    ) -> Result<SessionUsageOutput, xai_tool_runtime::ToolError> {
        let text = std::fs::read_to_string(&input.path).map_err(|e| {
            xai_tool_runtime::ToolError::execution(
                self.id(),
                format!("failed to read usage.jsonl at {}: {e}", input.path),
            )
        })?;
        Ok(SessionUsageOutput {
            stdout: usage_toon_from_jsonl(&text),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        r#"{"event":"model_call","input_tokens":10,"output_tokens":5}"#,
        "\n",
        r#"{"event":"model_call","input_tokens":3,"output_tokens":null}"#,
        "\n",
    );

    #[test]
    fn session_usage_reads_fixture_jsonl_toon() {
        let out = usage_toon_from_jsonl(FIXTURE);
        assert!(
            out.contains("input_tokens") || out.contains("input_tokens:"),
            "must read fixture events as TOON, got: {out}"
        );
        assert!(
            !out.trim_start().starts_with('{') && !out.trim_start().starts_with('['),
            "session_usage default output must be TOON, not JSON: {out}"
        );
    }

    #[test]
    fn session_usage_null_output_tokens_not_printed_as_zero() {
        let out = usage_toon_from_jsonl(FIXTURE);
        assert!(
            out.contains("input_tokens") || out.contains('3'),
            "must include the null-output event from the fixture, got: {out}"
        );
        assert!(
            !out.contains("output_tokens: 0")
                && !out.contains("\"output_tokens\": 0")
                && !out.contains("\"output_tokens\":0"),
            "null output_tokens must not print as zero, got: {out}"
        );
    }
}
