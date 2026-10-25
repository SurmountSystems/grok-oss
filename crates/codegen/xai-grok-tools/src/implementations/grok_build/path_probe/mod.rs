//! `path_probe`: batch exists/missing/type as TOON.
//!
//! Replaces a bash `test -f` loop. Default agent-facing output is TOON via
//! [`crate::util::toon::encode`]. Missing paths report `MISSING`. Does not
//! spawn a shell.

use std::path::{Path, PathBuf};

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::util::toon;
use serde_json::{Value, json};

/// Stable client-facing tool id.
pub const PATH_PROBE_TOOL_NAME: &str = "path_probe";

/// Paths to probe.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PathProbeInput {
    /// Absolute or relative paths.
    #[schemars(description = "Paths to probe (absolute or relative).")]
    pub paths: Vec<String>,
}

/// Model-facing result. Default encoding is TOON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PathProbeOutput {
    pub stdout: String,
}

impl xai_tool_runtime::ToolOutput for PathProbeOutput {}

impl From<PathProbeInput> for ToolInput {
    fn from(input: PathProbeInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({ "paths": input.paths }))
    }
}

impl From<PathProbeOutput> for ToolOutput {
    fn from(o: PathProbeOutput) -> Self {
        ToolOutput::Text(TextOutput::from(o.stdout))
    }
}

/// Probe paths. Default output is TOON. Missing is the token `MISSING`.
pub fn probe_paths(paths: &[impl AsRef<Path>]) -> String {
    let mut rows = Vec::with_capacity(paths.len());
    for p in paths {
        let path = p.as_ref();
        let path_s = path.to_string_lossy().to_string();
        let (exists, kind) = match std::fs::symlink_metadata(path) {
            Ok(meta) => {
                let kind = if meta.is_dir() {
                    "directory"
                } else if meta.is_file() {
                    "file"
                } else {
                    "other"
                };
                (true, kind)
            }
            Err(_) => (false, "MISSING"),
        };
        rows.push(json!({
            "path": path_s,
            "exists": exists,
            "kind": kind,
        }));
    }
    encode_paths_toon(json!({ "paths": rows }))
}

fn encode_paths_toon(value: Value) -> String {
    toon::encode(&value).unwrap_or_else(|_| toon::compact_json(&value))
}

/// Agent tool: batch path exists/missing/type.
#[derive(Debug, Default)]
pub struct PathProbeTool;

impl crate::types::tool_metadata::ToolMetadata for PathProbeTool {
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
        r#"Probe whether paths exist (file, directory, or MISSING) and print TOON. Replaces a bash test -f loop."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for PathProbeTool {
    type Args = PathProbeInput;
    type Output = PathProbeOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(PATH_PROBE_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            PATH_PROBE_TOOL_NAME,
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
        input: PathProbeInput,
    ) -> Result<PathProbeOutput, xai_tool_runtime::ToolError> {
        let paths: Vec<PathBuf> = input.paths.into_iter().map(PathBuf::from).collect();
        Ok(PathProbeOutput {
            stdout: probe_paths(&paths),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_probe_missing_is_missing_toon() {
        let missing = std::env::temp_dir().join("grok-path-probe-missing-no-such-file-xyz");
        let _ = std::fs::remove_file(&missing);
        let out = probe_paths(&[missing.as_path()]);
        assert!(
            out.contains("MISSING") || out.contains("missing:"),
            "missing path must report MISSING in TOON, got: {out}"
        );
        assert!(
            !out.trim_start().starts_with('{'),
            "path_probe default output must be TOON, not JSON: {out}"
        );
    }

    #[test]
    fn path_probe_existing_file_toon() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("exists.txt");
        std::fs::write(&f, "ok").unwrap();
        let out = probe_paths(&[f.as_path()]);
        assert!(
            out.contains("file") || out.contains("exists"),
            "existing file must be typed in TOON, got: {out}"
        );
        assert!(
            !out.trim_start().starts_with('{'),
            "path_probe default output must be TOON, not JSON: {out}"
        );
    }

    #[test]
    fn path_probe_output_is_not_json_object_start() {
        let out = probe_paths(&[Path::new("/tmp")]);
        assert!(
            !out.trim_start().starts_with('{'),
            "path_probe default output must not be a JSON object: {out}"
        );
    }
}
