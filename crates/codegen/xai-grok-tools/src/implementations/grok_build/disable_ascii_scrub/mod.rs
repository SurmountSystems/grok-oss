//! `disable_ascii_scrub` — agent request to turn off assistant punctuation scrub.
//!
//! The agent cannot silently flip hygiene off. Shell owns the permission UX:
//! when this tool is invoked, the session layer runs
//! `session/request_permission` with scrub-specific options (Allow once /
//! Allow always / Reject) **before** the tool body runs. Reject keeps scrub
//! on and the tool is not executed. On allow, the shell applies the override
//! (session and, for Always, durable settings write) and then this tool
//! confirms the outcome to the model.
//!
//! This tool body is intentionally a no-op status report — disable logic lives
//! only in the shell permission path.

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;

/// Stable client-facing tool id (and registry id suffix).
pub const DISABLE_ASCII_SCRUB_TOOL_NAME: &str = "disable_ascii_scrub";

/// Empty input — binary gate (ask user to leave fancy punctuation alone).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DisableAsciiScrubInput {}

/// Model-facing confirmation after the permission UX applied (or short-circuit
/// when scrub was already off).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisableAsciiScrubOutput {
    pub message: String,
}

impl xai_tool_runtime::ToolOutput for DisableAsciiScrubOutput {}

impl From<DisableAsciiScrubInput> for ToolInput {
    fn from(_: DisableAsciiScrubInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({}))
    }
}

impl From<DisableAsciiScrubOutput> for ToolOutput {
    fn from(o: DisableAsciiScrubOutput) -> Self {
        ToolOutput::Text(TextOutput::from(o.message))
    }
}

/// Agent tool: request that assistant AI text keep fancy punctuation.
#[derive(Debug, Default)]
pub struct DisableAsciiScrubTool;

impl crate::types::tool_metadata::ToolMetadata for DisableAsciiScrubTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        r#"Request that assistant AI text keep fancy punctuation (curly quotes, em dashes) instead of ASCII scrubbing.

You cannot silently disable scrubbing. Calling this tool always goes through the session permission prompt (Allow once / Allow always / Reject). Reject keeps scrub on. Allow once turns it off for the rest of this session. Allow always also writes the durable settings preference off.

Prefer this only when the user explicitly wants fancy punctuation preserved. Users can also turn scrub off via /settings → Appearance or config without this tool."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for DisableAsciiScrubTool {
    type Args = DisableAsciiScrubInput;
    type Output = DisableAsciiScrubOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(DISABLE_ASCII_SCRUB_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            DISABLE_ASCII_SCRUB_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        // Not auto-allowlisted as Read: shell always routes this through the
        // scrub-specific permission UX (never silent YOLO / Read auto-allow).
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.disable_ascii_scrub", skip_all)]
    async fn run(
        &self,
        _ctx: xai_tool_runtime::ToolCallContext,
        _input: DisableAsciiScrubInput,
    ) -> Result<DisableAsciiScrubOutput, xai_tool_runtime::ToolError> {
        // Shell applies the disable *before* this body runs (permission UX).
        // If we got here, the user allowed it (or scrub was already off).
        Ok(DisableAsciiScrubOutput {
            message: "ASCII-safe assistant punctuation scrub is off for this session. \
Fancy punctuation in assistant text will pass through. \
Re-enable via /settings → Appearance → ASCII-safe assistant punctuation, \
or set [ui] scrub_ascii_punct = true."
                .to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_tool_runtime::Tool;

    #[test]
    fn tool_id_is_stable() {
        assert_eq!(
            DisableAsciiScrubTool.id().as_str(),
            DISABLE_ASCII_SCRUB_TOOL_NAME
        );
    }

    #[test]
    fn input_maps_to_dynamic_tool_input() {
        let ti: ToolInput = DisableAsciiScrubInput {}.into();
        assert!(matches!(ti, ToolInput::Dynamic(_)));
    }

    #[test]
    fn output_maps_to_text_tool_output() {
        let to: ToolOutput = DisableAsciiScrubOutput {
            message: "ok".into(),
        }
        .into();
        match to {
            ToolOutput::Text(t) => assert_eq!(t.text, "ok"),
            other => panic!("expected Text, got {other:?}"),
        }
    }
}
