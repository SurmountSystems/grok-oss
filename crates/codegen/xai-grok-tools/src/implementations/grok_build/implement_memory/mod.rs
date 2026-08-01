//! `implement_memory` — first-class tool wrapping `util::implement_memory`.
//!
//! Bash intercept of allowlisted `python3 …/memory.py …` remains the fallback
//! for host skill dual-pin. This named tool is for model discovery without
//! requiring the skill bash form.

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::types::tool_metadata::{ToolMetadata, resolve_cwd, shared_resources};
use crate::util::implement_memory::{
    MemoryIntercept, MemorySubcommand, UpdateStdinSource, execute_intercept,
};

/// Stable client-facing tool id.
pub const IMPLEMENT_MEMORY_TOOL_NAME: &str = "implement_memory";

/// Input: subcommand + optional update JSON body.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ImplementMemoryInput {
    /// One of: `path` | `read` | `snapshot` | `update`.
    #[schemars(
        description = "Subcommand: path (memory file path), read (raw markdown), snapshot (JSON state), update (merge JSON patch)."
    )]
    pub op: String,
    /// JSON object/array text for `update` only (merged into memory). Ignored for other ops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "For op=update: JSON merge patch as a string (object).")]
    pub update_json: Option<String>,
}

/// Model-facing result (stdout body + exit code parity with host memory.py).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImplementMemoryOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl xai_tool_runtime::ToolOutput for ImplementMemoryOutput {}

impl From<ImplementMemoryInput> for ToolInput {
    fn from(input: ImplementMemoryInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({
            "op": input.op,
            "update_json": input.update_json,
        }))
    }
}

impl From<ImplementMemoryOutput> for ToolOutput {
    fn from(o: ImplementMemoryOutput) -> Self {
        // Surface stdout primarily; include stderr/exit when non-success.
        if o.exit_code == 0 && o.stderr.is_empty() {
            ToolOutput::Text(TextOutput::from(o.stdout))
        } else {
            let body = serde_json::json!({
                "stdout": o.stdout,
                "stderr": o.stderr,
                "exit_code": o.exit_code,
            });
            ToolOutput::Text(TextOutput::from(
                serde_json::to_string_pretty(&body).unwrap_or_else(|_| o.stdout.clone()),
            ))
        }
    }
}

fn parse_subcommand(op: &str, update_json: Option<&str>) -> Result<MemorySubcommand, String> {
    match op.trim().to_ascii_lowercase().as_str() {
        "path" => Ok(MemorySubcommand::Path),
        "read" => Ok(MemorySubcommand::Read),
        "snapshot" => Ok(MemorySubcommand::Snapshot),
        "update" => {
            let stdin = match update_json {
                Some(s) if !s.trim().is_empty() => UpdateStdinSource::Literal(s.to_owned()),
                _ => UpdateStdinSource::Empty,
            };
            Ok(MemorySubcommand::Update { stdin })
        }
        other => Err(format!(
            "unknown op '{other}'; expected path | read | snapshot | update"
        )),
    }
}

/// Agent tool: implement-skill workspace memory (path/read/snapshot/update).
#[derive(Debug, Default)]
pub struct ImplementMemoryTool;

impl ToolMetadata for ImplementMemoryTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        r#"Read or update implement-skill workspace memory (path / read / snapshot / update).

Ops:
- path: print the memory file path for this workspace
- read: raw markdown body (empty if missing)
- snapshot: JSON state (common_issues, recent_runs, …)
- update: merge `update_json` (JSON object string) into memory

Same in-process handler as the bash intercept of host
`python3 …/implement/scripts/memory.py …`. Prefer this named tool when available;
bash form remains a fallback for skill dual-pin."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for ImplementMemoryTool {
    type Args = ImplementMemoryInput;
    type Output = ImplementMemoryOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(IMPLEMENT_MEMORY_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            IMPLEMENT_MEMORY_TOOL_NAME,
            ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        // `update` mutates disk under `~/.grok/implement-memory/`. Write scope
        // so multi-agent hub routes to the leader (peers use Write for mutators).
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.implement_memory", skip_all)]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: ImplementMemoryInput,
    ) -> Result<ImplementMemoryOutput, xai_tool_runtime::ToolError> {
        let sub = parse_subcommand(&input.op, input.update_json.as_deref())
            .map_err(xai_tool_runtime::ToolError::invalid_arguments)?;
        let resources = shared_resources(&ctx)?;
        let cwd = resolve_cwd(&ctx, &resources).await?;
        let intercept = MemoryIntercept {
            script_path: "implement_memory".into(),
            subcommand: sub,
        };
        let h = execute_intercept(&intercept, &cwd, None);
        Ok(ImplementMemoryOutput {
            stdout: h.stdout,
            stderr: h.stderr,
            exit_code: h.exit_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::resources::{Cwd, Resources};
    use crate::types::tool_metadata::test_ctx;
    use xai_tool_runtime::Tool;

    #[test]
    fn tool_id_is_stable() {
        assert_eq!(
            ImplementMemoryTool.id().as_str(),
            IMPLEMENT_MEMORY_TOOL_NAME
        );
    }

    #[test]
    fn capabilities_are_write_scope() {
        let caps = ImplementMemoryTool.capabilities();
        assert!(!caps.is_read_only);
        assert_eq!(
            caps.tool_scope,
            Some(xai_tool_protocol::ToolScope::Write),
            "update mutates disk; hub must treat as Write (not Read)"
        );
    }

    #[test]
    fn parse_ops() {
        assert!(matches!(
            parse_subcommand("path", None).unwrap(),
            MemorySubcommand::Path
        ));
        assert!(matches!(
            parse_subcommand("SNAPSHOT", None).unwrap(),
            MemorySubcommand::Snapshot
        ));
        match parse_subcommand("update", Some(r#"{"x":1}"#)).unwrap() {
            MemorySubcommand::Update {
                stdin: UpdateStdinSource::Literal(s),
            } => assert!(s.contains('1')),
            other => panic!("unexpected: {other:?}"),
        }
        assert!(parse_subcommand("nope", None).is_err());
    }

    #[tokio::test]
    async fn snapshot_runs_in_process() {
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        let tool = ImplementMemoryTool;
        let out = tool
            .run(
                test_ctx(resources.into_shared()),
                ImplementMemoryInput {
                    op: "snapshot".into(),
                    update_json: None,
                },
            )
            .await
            .expect("snapshot ok");
        assert_eq!(out.exit_code, 0, "stderr={}", out.stderr);
        let v: serde_json::Value = serde_json::from_str(out.stdout.trim()).expect("snapshot JSON");
        assert!(v.get("exists").is_some());
    }

    #[test]
    fn output_success_maps_to_text() {
        let to: ToolOutput = ImplementMemoryOutput {
            stdout: "ok\n".into(),
            stderr: String::new(),
            exit_code: 0,
        }
        .into();
        match to {
            ToolOutput::Text(t) => assert_eq!(t.text, "ok\n"),
            other => panic!("expected Text, got {other:?}"),
        }
    }
}
