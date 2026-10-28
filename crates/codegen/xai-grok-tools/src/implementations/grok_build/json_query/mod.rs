//! `json_query`: JSON Pointer over a JSON value, TOON out.
//!
//! In-process [`serde_json::Value::pointer`] (RFC 6901). Default agent-facing
//! output is TOON via [`crate::util::toon::encode`]. Does not spawn `jq` or
//! any other process.

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::util::toon;
use serde_json::Value;

/// Stable client-facing tool id.
pub const JSON_QUERY_TOOL_NAME: &str = "json_query";

/// JSON value plus a JSON Pointer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct JsonQueryInput {
    /// JSON to query.
    #[schemars(description = "JSON value to query.")]
    pub json: Value,
    /// RFC 6901 JSON Pointer (for example `/user/name`).
    #[schemars(description = "JSON Pointer (RFC 6901), for example /user/name.")]
    pub pointer: String,
}

/// Model-facing result. Default encoding is TOON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JsonQueryOutput {
    pub stdout: String,
}

impl xai_tool_runtime::ToolOutput for JsonQueryOutput {}

impl From<JsonQueryInput> for ToolInput {
    fn from(input: JsonQueryInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({
            "json": input.json,
            "pointer": input.pointer,
        }))
    }
}

impl From<JsonQueryOutput> for ToolOutput {
    fn from(o: JsonQueryOutput) -> Self {
        ToolOutput::Text(TextOutput::from(o.stdout))
    }
}

/// JSON Pointer query. Default output is TOON. In-process; never `jq`.
pub fn query_pointer(json: &Value, pointer: &str) -> Result<String, String> {
    if !is_valid_json_pointer(pointer) {
        return Err(format!(
            "invalid JSON Pointer {pointer:?}: must be empty or start with /"
        ));
    }
    let Some(found) = json.pointer(pointer) else {
        return Err(format!("JSON Pointer {pointer:?} did not match any value"));
    };
    toon::encode(found).map_err(|e| format!("TOON encode failed: {e}"))
}

fn is_valid_json_pointer(pointer: &str) -> bool {
    pointer.is_empty() || pointer.starts_with('/')
}

/// Agent tool: JSON Pointer, TOON stdout.
#[derive(Debug, Default)]
pub struct JsonQueryTool;

impl crate::types::tool_metadata::ToolMetadata for JsonQueryTool {
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
        r#"Query a JSON value with a JSON Pointer and print TOON. In-process serde_json; does not invoke jq."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for JsonQueryTool {
    type Args = JsonQueryInput;
    type Output = JsonQueryOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(JSON_QUERY_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            JSON_QUERY_TOOL_NAME,
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
        input: JsonQueryInput,
    ) -> Result<JsonQueryOutput, xai_tool_runtime::ToolError> {
        let stdout = query_pointer(&input.json, &input.pointer)
            .map_err(xai_tool_runtime::ToolError::invalid_arguments)?;
        Ok(JsonQueryOutput { stdout })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_query_pointer_toon() {
        let v = json!({"user": {"name": "Ada", "age": 36}});
        let out = query_pointer(&v, "/user").expect("pointer");
        assert!(
            out.contains("name:"),
            "JSON Pointer result must be TOON keys, got: {out}"
        );
        assert!(
            !out.trim_start().starts_with('{'),
            "must not be a JSON object start: {out}"
        );
    }

    #[test]
    fn json_query_does_not_invoke_jq() {
        let src = include_str!("mod.rs");
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("product source precedes tests");
        assert!(
            !prod.contains("Command::new("),
            "json_query must not spawn jq or any Command"
        );
        assert!(
            !prod.contains("std::process") && !prod.contains("tokio::process"),
            "json_query must not spawn a process"
        );
        let _ = query_pointer(&json!({"a": 1}), "/a");
    }

    #[test]
    fn json_query_invalid_pointer_errors() {
        let err = query_pointer(&json!({"a": 1}), "not-a-pointer")
            .expect_err("invalid JSON Pointer must error");
        let lower = err.to_ascii_lowercase();
        assert!(
            lower.contains("pointer") || lower.contains("invalid"),
            "clear pointer error, got: {err}"
        );
    }
}
