//! `json_to_toon` — convert JSON text or a structured JSON value to TOON.
//!
//! Dogfood surface for UDAX (`util::toon`). Protocol envelopes (ACP/MCP) are
//! unchanged; this is an optional first-class agent tool only.

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::util::toon;
use serde_json::Value;

/// Stable client-facing tool id.
pub const JSON_TO_TOON_TOOL_NAME: &str = "json_to_toon";

/// Input: JSON string (parsed) or any JSON value.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct JsonToToonInput {
    /// JSON to encode as TOON.
    ///
    /// - Object / array / number / bool / null → encoded as-is.
    /// - String → treated as **JSON text** and parsed first. Invalid JSON text
    ///   returns a clear error (the string is not re-encoded as a TOON string
    ///   literal unless you wrap it in a JSON value yourself).
    #[schemars(
        description = "JSON value or a JSON text string to convert to TOON. Strings are parsed as JSON text first; invalid JSON text returns an error."
    )]
    pub json: Value,
}

/// Model-facing TOON text result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JsonToToonOutput {
    /// TOON encoding of the input JSON.
    pub toon: String,
}

impl xai_tool_runtime::ToolOutput for JsonToToonOutput {}

impl From<JsonToToonInput> for ToolInput {
    fn from(input: JsonToToonInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({ "json": input.json }))
    }
}

impl From<JsonToToonOutput> for ToolOutput {
    fn from(o: JsonToToonOutput) -> Self {
        // Prefer Dynamic so T2 policy can densify further if needed; body is
        // already TOON text, so surface as plain text for the model.
        ToolOutput::Text(TextOutput::from(o.toon))
    }
}

/// Resolve the logical JSON value from tool input (string = JSON text).
pub fn resolve_json_value(json: &Value) -> Result<Value, String> {
    match json {
        Value::String(s) => serde_json::from_str(s).map_err(|e| {
            format!(
                "invalid JSON text: {e} (pass a parseable JSON string or a structured JSON value)"
            )
        }),
        other => Ok(other.clone()),
    }
}

/// Encode resolved JSON to TOON (clear error on encode failure).
pub fn encode_json_to_toon(json: &Value) -> Result<String, String> {
    let value = resolve_json_value(json)?;
    toon::encode(&value).map_err(|e| format!("TOON encode failed: {e}"))
}

/// Agent tool: JSON → TOON via `util::toon`.
#[derive(Debug, Default)]
pub struct JsonToToonTool;

impl crate::types::tool_metadata::ToolMetadata for JsonToToonTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    /// Pure conversion: no workspace or external side-effects. Overrides the
    /// `Other` kind default (mutating) so wire metadata matches `capabilities`.
    fn is_read_only(&self) -> bool {
        true
    }

    fn description_template(&self) -> &str {
        r#"Convert JSON to TOON (Token-Oriented Object Notation) for denser model context.

Pass `json` as either:
- a structured JSON value (object/array/…), or
- a string containing JSON text (parsed first).

Invalid JSON text returns a clear error. Does not change ACP/MCP protocol envelopes — those stay JSON-RPC. Prefer this when you hold a large uniform JSON blob and want TOON before pasting into a prompt or handoff."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for JsonToToonTool {
    type Args = JsonToToonInput;
    type Output = JsonToToonOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(JSON_TO_TOON_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            JSON_TO_TOON_TOOL_NAME,
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

    #[tracing::instrument(name = "tool.json_to_toon", skip_all)]
    async fn run(
        &self,
        _ctx: xai_tool_runtime::ToolCallContext,
        input: JsonToToonInput,
    ) -> Result<JsonToToonOutput, xai_tool_runtime::ToolError> {
        let toon = encode_json_to_toon(&input.json)
            .map_err(xai_tool_runtime::ToolError::invalid_arguments)?;
        Ok(JsonToToonOutput { toon })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use xai_tool_runtime::Tool;

    #[test]
    fn tool_id_is_stable() {
        assert_eq!(JsonToToonTool.id().as_str(), JSON_TO_TOON_TOOL_NAME);
    }

    #[test]
    fn structured_object_encodes() {
        let v = json!({"name": "Ada", "age": 36});
        let out = encode_json_to_toon(&v).unwrap();
        assert!(out.contains("name:"), "got: {out}");
        assert!(out.contains("Ada"), "got: {out}");
        assert!(!out.trim_start().starts_with('{'));
    }

    #[test]
    fn json_text_string_is_parsed() {
        let v = json!("{\"tags\":[\"a\",\"b\"]}");
        let out = encode_json_to_toon(&v).unwrap();
        assert!(out.contains("tags"), "got: {out}");
        assert!(out.contains("a"), "got: {out}");
    }

    #[test]
    fn invalid_json_text_clear_error() {
        let v = json!("{not valid json");
        let err = encode_json_to_toon(&v).unwrap_err();
        assert!(
            err.contains("invalid JSON text"),
            "expected clear invalid-JSON error, got: {err}"
        );
    }

    #[test]
    fn tabular_array_encodes() {
        let v = json!([
            {"id": 1, "name": "a"},
            {"id": 2, "name": "b"}
        ]);
        let out = encode_json_to_toon(&v).unwrap();
        assert!(out.contains("id") && out.contains("name"), "got: {out}");
    }

    #[test]
    fn output_maps_to_text() {
        let to: ToolOutput = JsonToToonOutput {
            toon: "name: x".into(),
        }
        .into();
        match to {
            ToolOutput::Text(t) => assert_eq!(t.text, "name: x"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    /// Runtime path: `Tool::run` maps encode failures to
    /// `ToolErrorKind::InvalidArguments` (not a bare unit-path re-check).
    #[tokio::test]
    async fn run_rejects_invalid_json_text() {
        use xai_tool_runtime::error::ToolErrorKind;

        let tool = JsonToToonTool;
        let ctx = xai_tool_runtime::ToolCallContext::default();
        let err = tool
            .run(ctx, JsonToToonInput { json: json!("[") })
            .await
            .expect_err("invalid JSON text must fail at Tool::run");
        assert_eq!(
            err.kind,
            ToolErrorKind::InvalidArguments,
            "invalid JSON must surface as invalid_arguments, got {:?}",
            err.kind
        );
        let msg = err.to_string();
        assert!(
            msg.contains("invalid JSON text"),
            "runtime error should keep the clear message; got {msg}"
        );
    }
}
