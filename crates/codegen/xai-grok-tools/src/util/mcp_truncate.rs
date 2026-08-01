//! Shared size-bounding for MCP/text tool output.
//!
//! Large payloads (e.g. Sentry attachment base64 resources) must not land
//! fully in chat state: they inflate the token estimate and trigger premature
//! auto-compact.
//!
//! # Configurable limit
//!
//! Default [`MCP_MAX_OUTPUT_BYTES`] (20_000). Effective limit (highest first):
//!
//! 1. [`TruncationCfg`](crate::types::resources::TruncationCfg) per-tool /
//!    MCP-specific (`mcp_max_output_bytes` — e.g. a winning repo-level
//!    `[mcp] max_output_bytes`, seeded per session by the shell) / default,
//!    when present in resources
//! 2. Host-seeded effective limit via [`set_mcp_max_output_bytes`] (host
//!    resolves requirements > env > config > remote config > default once at
//!    bootstrap / remote-config refresh and stores the result)
//! 3. When host has not seeded (`0`): env
//!    [`ENV_GROK_MAX_MCP_OUTPUT_BYTES`] / [`ENV_MAX_MCP_OUTPUT_BYTES`]
//! 4. Built-in default

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use xai_tool_runtime::ToolCallContext;

use crate::types::output::{MCPOutputDetails, ToolOutput};
use crate::types::tool::ToolKind;
use crate::util::query_tools::{QueryTools, examples_clause};
use crate::util::truncate::format_bytes;

/// Default inline limit for MCP tool output in chat state (bytes, not tokens).
pub const MCP_MAX_OUTPUT_BYTES: usize = 20_000;

/// Env override for the MCP inline output cap (bytes).
/// Some agents use `MAX_MCP_OUTPUT_TOKENS`; we bound by **bytes** because
/// truncation is byte-oriented (`truncate_str`).
pub const ENV_MAX_MCP_OUTPUT_BYTES: &str = "MAX_MCP_OUTPUT_BYTES";

/// Grok-native env override for the MCP inline output cap (bytes).
pub const ENV_GROK_MAX_MCP_OUTPUT_BYTES: &str = "GROK_MAX_MCP_OUTPUT_BYTES";

/// Process-wide effective limit. `0` = host has not seeded; fall through to
/// env / default. The shell writes the *fully resolved* stack here so free-
/// function tool dispatch (no live `Config`) sees the same value.
static EFFECTIVE_MCP_MAX_OUTPUT_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Host (shell) sets the fully-resolved MCP output cap in bytes.
///
/// Pass the already-resolved limit (requirements > env > config > remote config >
/// default). Pass `0` only in tests to clear and fall through to env / default.
pub fn set_mcp_max_output_bytes(bytes: usize) {
    EFFECTIVE_MCP_MAX_OUTPUT_BYTES.store(bytes, Ordering::Relaxed);
}

/// Parse a positive byte limit from an env var. Zero / unparseable → `None`.
fn parse_positive_bytes_env(name: &str) -> Option<usize> {
    let raw = std::env::var(name).ok()?;
    let n = raw.trim().parse::<u64>().ok()?;
    usize::try_from(n).ok().filter(|n| *n > 0)
}

/// Env tier: `GROK_MAX_MCP_OUTPUT_BYTES` then `MAX_MCP_OUTPUT_BYTES`.
///
/// Grok-native wins when both are set. Positive integers only. Used by the
/// shell resolver and as the standalone fallback when the host has not called
/// [`set_mcp_max_output_bytes`].
pub fn mcp_max_output_bytes_from_env() -> Option<usize> {
    parse_positive_bytes_env(ENV_GROK_MAX_MCP_OUTPUT_BYTES)
        .or_else(|| parse_positive_bytes_env(ENV_MAX_MCP_OUTPUT_BYTES))
}

/// Effective MCP inline output cap for this process.
///
/// Host-seeded value if set; otherwise env; otherwise [`MCP_MAX_OUTPUT_BYTES`].
pub fn mcp_max_output_bytes() -> usize {
    match EFFECTIVE_MCP_MAX_OUTPUT_BYTES.load(Ordering::Relaxed) {
        0 => mcp_max_output_bytes_from_env().unwrap_or(MCP_MAX_OUTPUT_BYTES),
        n => n,
    }
}

pub(crate) const LONG_LINE_BYTES: usize = 2_000;

/// How a truncated MCP payload is saved and described to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum McpDumpKind {
    LongLineJson,
    Json,
    LongLineText,
    /// Structured JSON densified to TOON (or other non-JSON model format).
    /// Dump body is densified text; extension `.txt` with shell search steer
    /// (not jq — body is no longer JSON).
    DensifiedStructured,
    Other,
}

impl McpDumpKind {
    pub(crate) fn classify(text: &str) -> Self {
        let trimmed = text.trim();
        let is_json = matches!(trimmed.as_bytes().first(), Some(b'{' | b'['))
            && serde_json::from_str::<serde::de::IgnoredAny>(trimmed).is_ok();
        let has_long_line = text.lines().map(str::len).max().unwrap_or(0) > LONG_LINE_BYTES;
        match (is_json, has_long_line) {
            (true, true) => Self::LongLineJson,
            (true, false) => Self::Json,
            (false, true) => Self::LongLineText,
            (false, false) => Self::Other,
        }
    }

    /// Dump kind after densify: keep JSON/long-line kinds when the densified
    /// body still classifies that way; if densify turned JSON into TOON
    /// (non-JSON), use [`Self::DensifiedStructured`] so the operator still
    /// gets a non-empty “query the saved file” steer under default `auto`.
    pub(crate) fn after_densify(pre_kind: Self, densified_text: &str) -> Self {
        let densified_kind = Self::classify(densified_text);
        match (pre_kind, densified_kind) {
            (_, densified @ (Self::Json | Self::LongLineJson | Self::LongLineText)) => densified,
            (Self::Json | Self::LongLineJson, Self::Other) => Self::DensifiedStructured,
            (_, densified) => densified,
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::LongLineJson | Self::Json => "json",
            Self::LongLineText | Self::DensifiedStructured | Self::Other => "txt",
        }
    }

    pub(crate) fn steer(self, shell: &str, tools: QueryTools) -> String {
        match self {
            Self::LongLineJson => format!(
                " The full output is valid JSON with a very long line, so \
                 grep/read_file are ineffective on it — use `{shell}` to query the \
                 saved file{eg}.",
                eg = examples_clause(&tools.json_tools()),
            ),
            Self::Json => format!(
                " The full output is valid JSON saved to the file above; use \
                 `{shell}` to query it{eg}.",
                eg = examples_clause(&tools.json_tools()),
            ),
            Self::LongLineText => format!(
                " The full output has a very long line, so grep/read_file are \
                 ineffective on it — use `{shell}` to slice/search the saved \
                 file{eg}.",
                eg = examples_clause(&tools.text_tools()),
            ),
            Self::DensifiedStructured => format!(
                " The full output is densified structured text (model format from \
                 JSON); use `{shell}` to search/slice the saved file{eg}.",
                eg = examples_clause(&tools.text_tools()),
            ),
            Self::Other => String::new(),
        }
    }
}

/// Resolved settings for truncating one MCP payload (inline limit, dump dir,
/// shell tool name, call id). Build with [`McpTruncateContext::from_tool_ctx`].
pub struct McpTruncateContext {
    pub(crate) max_output_bytes: usize,
    pub(crate) session_folder: Option<PathBuf>,
    pub(crate) shell_tool: String,
    pub(crate) call_id: String,
}

impl McpTruncateContext {
    pub async fn from_tool_ctx(ctx: &ToolCallContext, tool_key: &str) -> Self {
        let call_id = ctx.call_id.as_str().to_string();
        let resolved_default = mcp_max_output_bytes();
        match crate::types::tool_metadata::shared_resources(ctx) {
            Ok(res) => {
                let guard = res.lock().await;
                let max_output_bytes = guard
                    .get::<crate::types::resources::TruncationCfg>()
                    .map(|cfg| cfg.0.mcp_max_output_bytes_for(tool_key, resolved_default))
                    .unwrap_or(resolved_default);
                let session_folder = guard
                    .get::<crate::types::resources::SessionFolder>()
                    .map(|f| f.0.clone());
                let shell_tool = guard
                    .get::<crate::types::template_renderer::TemplateRenderer>()
                    .and_then(|r| r.tool_for_kind(ToolKind::Execute))
                    .map(str::to_string)
                    .unwrap_or_else(|| "bash".to_string());
                Self {
                    max_output_bytes,
                    session_folder,
                    shell_tool,
                    call_id,
                }
            }
            Err(_) => Self {
                max_output_bytes: resolved_default,
                session_folder: None,
                shell_tool: "bash".to_string(),
                call_id,
            },
        }
    }
}

/// Map a `call_id` to safe filename chars so a `/` or `..` in a wire-supplied
/// id (only validated as non-empty) cannot escape the session `mcp/` dir.
fn sanitized_stem(call_id: &str) -> String {
    call_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// If `text` is structured JSON (object or array), re-encode for the model under
/// the same TOON policy as [`crate::util::toon::maybe_encode_for_llm_from_env`].
///
/// Free text, bare scalars, and invalid JSON are left unchanged. Call this
/// **before** the MCP byte cap so denser TOON can avoid truncation.
///
/// Delegates to [`crate::util::toon::densify_structured_text_in_place`] (single
/// policy parser; no second independent densify path).
///
/// No-op without allocate when the body is not structured JSON.
pub fn densify_mcp_result_text_in_place(text: &mut String) {
    crate::util::toon::densify_structured_text_in_place(text);
}

/// Convenience: densify a borrowed slice (allocates only when rewriting).
pub fn densify_mcp_result_text(text: &str) -> String {
    crate::util::toon::densify_structured_text(text)
}

/// Truncate `text` in place when over the limit, dumping the full payload to
/// the session `mcp/` dir (when available) with a pointer appended.
///
/// Structured JSON is densified via [`densify_mcp_result_text_in_place`] first
/// (T3 UDAX). Dump kind is chosen with [`McpDumpKind::after_densify`] so
/// JSON→TOON over-cap dumps still get a shell steer under default `auto`.
async fn truncate_mcp_text(text: &mut String, trunc_ctx: &McpTruncateContext) {
    // Classify before densify so JSON→TOON still gets structured dump steer.
    let pre_kind = McpDumpKind::classify(text.as_str());
    // Encode structured→text under TOON policy before the byte cap.
    densify_mcp_result_text_in_place(text);

    if text.len() <= trunc_ctx.max_output_bytes {
        return;
    }

    let total_bytes = text.len();
    let kind = McpDumpKind::after_densify(pre_kind, text.as_str());

    let output_file_path = trunc_ctx.session_folder.as_ref().map(|folder| {
        folder.join("mcp").join(format!(
            "{}.{}",
            sanitized_stem(&trunc_ctx.call_id),
            kind.extension()
        ))
    });

    let file_hint = if let Some(ref path) = output_file_path {
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        match tokio::fs::write(path, text.as_bytes()).await {
            Ok(()) => format!(" Full output written to: {}.", path.to_string_lossy()),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to write full MCP output to file"
                );
                String::new()
            }
        }
    } else {
        String::new()
    };

    let truncated =
        crate::util::truncate::truncate_str(text.as_str(), trunc_ctx.max_output_bytes).to_owned();
    let steer = if file_hint.is_empty() {
        String::new()
    } else {
        kind.steer(&trunc_ctx.shell_tool, QueryTools::detect())
    };
    *text = format!(
        "{}\n\n[MCP output truncated: showing first {} of {}.{}{}]",
        truncated,
        format_bytes(trunc_ctx.max_output_bytes),
        format_bytes(total_bytes),
        file_hint,
        steer,
    );
}

/// Bound the `MCP`/`Text` variants to the inline size limit, keeping a preview
/// and dumping the full payload to disk. Other variants are returned untouched.
pub async fn truncate_tool_output(
    mut output: ToolOutput,
    trunc_ctx: &McpTruncateContext,
) -> ToolOutput {
    match &mut output {
        ToolOutput::MCP(mcp) => {
            let text = match mcp.output_mut() {
                MCPOutputDetails::OkayOutput(t) | MCPOutputDetails::Error(t) => t,
            };
            truncate_mcp_text(text, trunc_ctx).await;
        }
        ToolOutput::Text(text_out) => {
            truncate_mcp_text(&mut text_out.text, trunc_ctx).await;
        }
        _ => {}
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with_folder(folder: PathBuf, max: usize) -> McpTruncateContext {
        McpTruncateContext {
            max_output_bytes: max,
            session_folder: Some(folder),
            shell_tool: "bash".to_string(),
            call_id: "call-test".to_string(),
        }
    }

    /// Serialize tests that mutate the process-global effective limit / env.
    fn with_mcp_limit_lock<R>(f: impl FnOnce() -> R) -> R {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        f()
    }

    #[test]
    fn host_set_overrides_env_fallback() {
        with_mcp_limit_lock(|| {
            let prev = EFFECTIVE_MCP_MAX_OUTPUT_BYTES.load(Ordering::Relaxed);
            // Clear host seed; with no env, effective limit is the built-in default.
            set_mcp_max_output_bytes(0);
            let prev_max = std::env::var(ENV_MAX_MCP_OUTPUT_BYTES).ok();
            let prev_grok = std::env::var(ENV_GROK_MAX_MCP_OUTPUT_BYTES).ok();
            unsafe {
                std::env::remove_var(ENV_MAX_MCP_OUTPUT_BYTES);
                std::env::remove_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES);
            }
            assert_eq!(
                mcp_max_output_bytes(),
                MCP_MAX_OUTPUT_BYTES,
                "unset host + unset env → built-in default"
            );

            set_mcp_max_output_bytes(10_000);
            assert_eq!(mcp_max_output_bytes(), 10_000, "host seed wins over env");

            set_mcp_max_output_bytes(0);
            assert_eq!(
                mcp_max_output_bytes(),
                MCP_MAX_OUTPUT_BYTES,
                "cleared host falls through to default"
            );

            unsafe {
                match prev_max {
                    Some(v) => std::env::set_var(ENV_MAX_MCP_OUTPUT_BYTES, v),
                    None => std::env::remove_var(ENV_MAX_MCP_OUTPUT_BYTES),
                }
                match prev_grok {
                    Some(v) => std::env::set_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES, v),
                    None => std::env::remove_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES),
                }
            }
            set_mcp_max_output_bytes(prev);
        });
    }

    #[test]
    fn env_parser_rejects_zero_and_junk() {
        with_mcp_limit_lock(|| {
            let prev_max = std::env::var(ENV_MAX_MCP_OUTPUT_BYTES).ok();
            let prev_grok = std::env::var(ENV_GROK_MAX_MCP_OUTPUT_BYTES).ok();
            unsafe {
                std::env::remove_var(ENV_MAX_MCP_OUTPUT_BYTES);
                std::env::remove_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES);
            }
            assert_eq!(mcp_max_output_bytes_from_env(), None);

            unsafe { std::env::set_var(ENV_MAX_MCP_OUTPUT_BYTES, "0") };
            assert_eq!(mcp_max_output_bytes_from_env(), None);

            unsafe { std::env::set_var(ENV_MAX_MCP_OUTPUT_BYTES, "not-a-number") };
            assert_eq!(mcp_max_output_bytes_from_env(), None);

            unsafe { std::env::set_var(ENV_MAX_MCP_OUTPUT_BYTES, "12345") };
            assert_eq!(mcp_max_output_bytes_from_env(), Some(12_345));

            // GROK_* wins over MAX_* when both set.
            unsafe { std::env::set_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES, "99999") };
            assert_eq!(mcp_max_output_bytes_from_env(), Some(99_999));

            unsafe { std::env::remove_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES) };
            assert_eq!(mcp_max_output_bytes_from_env(), Some(12_345));

            unsafe {
                match prev_max {
                    Some(v) => std::env::set_var(ENV_MAX_MCP_OUTPUT_BYTES, v),
                    None => std::env::remove_var(ENV_MAX_MCP_OUTPUT_BYTES),
                }
                match prev_grok {
                    Some(v) => std::env::set_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES, v),
                    None => std::env::remove_var(ENV_GROK_MAX_MCP_OUTPUT_BYTES),
                }
            }
        });
    }

    #[tokio::test]
    async fn text_over_limit_truncates_and_dumps_full_payload() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = cfg_with_folder(dir.path().to_path_buf(), 100);
        let full = "x".repeat(5_000);

        let out = truncate_tool_output(ToolOutput::Text(full.clone().into()), &cfg).await;

        let ToolOutput::Text(t) = out else {
            panic!("expected Text");
        };
        assert!(t.text.len() < full.len());
        assert!(t.text.starts_with(&"x".repeat(100)), "preview prefix kept");
        assert!(t.text.contains("[MCP output truncated:"));
        assert!(t.text.contains("Full output written to:"));

        let dump = dir.path().join("mcp").join("call-test.txt");
        assert_eq!(tokio::fs::read_to_string(&dump).await.unwrap(), full);
    }

    #[tokio::test]
    async fn boundary_exact_limit_untouched_one_over_truncates() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = cfg_with_folder(dir.path().to_path_buf(), 100);

        let at = truncate_tool_output(ToolOutput::Text("a".repeat(100).into()), &cfg).await;
        let ToolOutput::Text(t) = at else {
            panic!("expected Text")
        };
        assert_eq!(t.text, "a".repeat(100), "exactly at limit is untouched");
        assert!(
            !dir.path().join("mcp").exists(),
            "no dump when not truncated"
        );

        let over = truncate_tool_output(ToolOutput::Text("b".repeat(101).into()), &cfg).await;
        let ToolOutput::Text(t) = over else {
            panic!("expected Text")
        };
        assert!(
            t.text.contains("[MCP output truncated:"),
            "one over truncates"
        );
    }

    #[tokio::test]
    async fn traversal_in_call_id_cannot_escape_session_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = McpTruncateContext {
            max_output_bytes: 100,
            session_folder: Some(dir.path().to_path_buf()),
            shell_tool: "bash".to_string(),
            call_id: "../../evil".to_string(),
        };

        let out = truncate_tool_output(ToolOutput::Text("x".repeat(5_000).into()), &cfg).await;

        let ToolOutput::Text(t) = out else {
            panic!("expected Text");
        };
        let mcp_dir = dir.path().join("mcp");
        assert!(!t.text.contains(".."), "no traversal sequence in pointer");
        let entries: Vec<_> = std::fs::read_dir(&mcp_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(entries.len(), 1, "exactly one dump file");
        assert!(entries[0].starts_with(&mcp_dir), "dump stayed inside mcp/");
    }

    #[tokio::test]
    async fn non_text_variant_passes_through() {
        let cfg = McpTruncateContext {
            max_output_bytes: 1,
            session_folder: None,
            shell_tool: "bash".to_string(),
            call_id: "call-test".to_string(),
        };

        let out = truncate_tool_output(
            ToolOutput::SearchTool(crate::types::output::SearchToolOutput {
                result_count: 1,
                content: "anything".to_string(),
            }),
            &cfg,
        )
        .await;

        let ToolOutput::SearchTool(s) = out else {
            panic!("expected SearchTool");
        };
        assert_eq!(s.content, "anything", "passthrough leaves content intact");
    }

    // ── T3: densify structured MCP JSON before byte cap ──

    #[test]
    fn densify_mcp_free_text_unchanged() {
        let plain = "hello world\nnot json at all";
        assert_eq!(densify_mcp_result_text(plain), plain);

        let scalar = "12345";
        assert_eq!(
            densify_mcp_result_text(scalar),
            scalar,
            "bare scalar is not object/array"
        );

        // In-place free text: no rewrite, no re-allocation of a densified body.
        let mut owned = plain.to_owned();
        let ptr_before = owned.as_ptr();
        densify_mcp_result_text_in_place(&mut owned);
        assert_eq!(owned, plain);
        assert_eq!(
            owned.as_ptr(),
            ptr_before,
            "free text densify-in-place must not reallocate"
        );
    }

    #[test]
    fn after_densify_preserves_json_kinds_and_marks_toon() {
        let json = r#"{"hits":[{"path":"a.rs","line":1,"text":"x"}]}"#;
        assert_eq!(McpDumpKind::classify(json), McpDumpKind::Json);
        // densified TOON is not JSON → DensifiedStructured (keeps shell steer).
        let toon_like = "hits[1]{path,line,text}:\n  a.rs,1,x";
        assert_eq!(
            McpDumpKind::after_densify(McpDumpKind::Json, toon_like),
            McpDumpKind::DensifiedStructured
        );
        assert_eq!(McpDumpKind::DensifiedStructured.extension(), "txt");
        let steer = McpDumpKind::DensifiedStructured.steer(
            "bash",
            QueryTools {
                jq: None,
                sed: Some("sed"),
                cut: Some("cut"),
            },
        );
        assert!(
            steer.contains("densified structured") && steer.contains("`bash`"),
            "TOON dump must steer to shell, got: {steer}"
        );
        // Still-JSON densified body keeps .json path.
        assert_eq!(
            McpDumpKind::after_densify(McpDumpKind::Json, r#"{"a":1}"#),
            McpDumpKind::Json
        );
    }

    #[test]
    fn densify_mcp_invalid_json_unchanged() {
        let junk = "{not valid json";
        assert_eq!(densify_mcp_result_text(junk), junk);
    }

    #[test]
    fn densify_mcp_tabular_auto_emits_toon() {
        use crate::util::toon::ENV_TOOL_RESULT_FORMAT;
        use crate::util::toon::test_env::{ENV_LOCK, EnvGuard};

        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, None)]); // auto

        let json = serde_json::json!({
            "hits": [
                {"path": "a.rs", "line": 1, "text": "fn main"},
                {"path": "b.rs", "line": 2, "text": "fn other"},
                {"path": "c.rs", "line": 3, "text": "fn third"},
            ]
        });
        let pretty = serde_json::to_string_pretty(&json).unwrap();
        let out = densify_mcp_result_text(&pretty);
        assert!(
            out.contains("hits[") && out.contains('{'),
            "auto should emit tabular TOON for uniform array, got: {out}"
        );
        assert!(
            !out.trim_start().starts_with('{'),
            "must not remain JSON object: {out}"
        );
        assert!(out.contains("a.rs") && out.contains("b.rs"));
    }

    #[test]
    fn densify_mcp_json_policy_is_compact_json() {
        use crate::util::toon::ENV_TOOL_RESULT_FORMAT;
        use crate::util::toon::test_env::{ENV_LOCK, EnvGuard};

        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, Some("json"))]);

        let value = serde_json::json!({
            "users": [
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"}
            ]
        });
        let pretty = serde_json::to_string_pretty(&value).unwrap();
        let out = densify_mcp_result_text(&pretty);
        assert_eq!(out, serde_json::to_string(&value).unwrap());
        assert!(out.starts_with('{'));
        assert!(!out.contains('\n'), "compact JSON is single-line: {out}");
    }

    #[tokio::test]
    async fn densify_before_truncate_can_avoid_byte_cap() {
        use crate::types::output::MCPOutput;
        use crate::util::toon::ENV_TOOL_RESULT_FORMAT;
        use crate::util::toon::test_env::{ENV_LOCK, EnvGuard};

        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, Some("toon"))]);

        // Pretty JSON with a uniform array — large enough that pretty form
        // exceeds a tight cap, but densified TOON fits under it.
        let rows: Vec<_> = (0..30)
            .map(|i| {
                serde_json::json!({
                    "path": format!("src/module_{i:03}.rs"),
                    "line": i * 10,
                    "text": "match found here"
                })
            })
            .collect();
        let value = serde_json::json!({"matches": rows});
        let pretty = serde_json::to_string_pretty(&value).unwrap();
        let densified = densify_mcp_result_text(&pretty);
        assert!(
            densified.len() < pretty.len(),
            "TOON denser than pretty: densified={} pretty={}",
            densified.len(),
            pretty.len()
        );
        // Cap between densified and pretty so densify-first avoids truncation.
        assert!(
            densified.len() < pretty.len(),
            "precondition: densified < pretty"
        );
        let max = densified.len() + (pretty.len() - densified.len()) / 2;
        assert!(
            densified.len() <= max && pretty.len() > max,
            "cap {max} should sit between densified {} and pretty {}",
            densified.len(),
            pretty.len()
        );

        let cfg = McpTruncateContext {
            max_output_bytes: max,
            session_folder: None,
            shell_tool: "bash".to_string(),
            call_id: "call-densify".to_string(),
        };
        let out = truncate_tool_output(
            ToolOutput::MCP(MCPOutput::okay_output(
                "server__tool".into(),
                "server".into(),
                pretty,
            )),
            &cfg,
        )
        .await;

        let ToolOutput::MCP(mcp) = out else {
            panic!("expected MCP");
        };
        let text = match mcp.output() {
            MCPOutputDetails::OkayOutput(t) | MCPOutputDetails::Error(t) => t,
        };
        assert!(
            !text.contains("[MCP output truncated:"),
            "densify-before-cap should keep payload under limit: {}",
            &text[text.len().saturating_sub(120)..]
        );
        assert!(
            text.contains("matches[") || text.contains("module_"),
            "should be densified TOON body, got: {text}"
        );
    }

    /// Under default auto/toon, over-cap densified JSON dumps as .txt with
    /// densified-structured steer (not empty Other).
    #[tokio::test]
    async fn densified_toon_over_cap_dumps_txt_with_structured_steer() {
        use crate::types::output::MCPOutput;
        use crate::util::toon::ENV_TOOL_RESULT_FORMAT;
        use crate::util::toon::test_env::{ENV_LOCK, EnvGuard};

        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, Some("toon"))]);

        let dir = tempfile::tempdir().unwrap();
        // Build a large uniform array that densifies to multi-line TOON but
        // still exceeds a small cap.
        let rows: Vec<_> = (0..80)
            .map(|i| {
                serde_json::json!({
                    "path": format!("src/file_{i:03}.rs"),
                    "line": i,
                    "text": "match found in source"
                })
            })
            .collect();
        let value = serde_json::json!({"matches": rows});
        let pretty = serde_json::to_string_pretty(&value).unwrap();
        let densified = densify_mcp_result_text(&pretty);
        assert!(
            !densified.trim_start().starts_with('{'),
            "precondition: toon densify leaves non-JSON"
        );
        let max = densified.len() / 3;
        assert!(max > 0 && densified.len() > max);

        let cfg = McpTruncateContext {
            max_output_bytes: max,
            session_folder: Some(dir.path().to_path_buf()),
            shell_tool: "bash".to_string(),
            call_id: "call-toon-dump".to_string(),
        };
        let out = truncate_tool_output(
            ToolOutput::MCP(MCPOutput::okay_output(
                "server__tool".into(),
                "server".into(),
                pretty,
            )),
            &cfg,
        )
        .await;

        let ToolOutput::MCP(mcp) = out else {
            panic!("expected MCP");
        };
        let text = match mcp.output() {
            MCPOutputDetails::OkayOutput(t) | MCPOutputDetails::Error(t) => t,
        };
        assert!(text.contains("[MCP output truncated:"));
        assert!(
            text.contains(".txt"),
            "densified TOON dump must use .txt, got: {}",
            &text[text.len().saturating_sub(200)..]
        );
        assert!(
            text.contains("densified structured") || text.contains("to slice/search"),
            "must keep a shell steer (not empty Other): {}",
            &text[text.len().saturating_sub(300)..]
        );

        let files: Vec<_> = std::fs::read_dir(dir.path().join("mcp"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].extension().and_then(|e| e.to_str()),
            Some("txt"),
            "dump extension .txt for densified body"
        );
    }

    #[tokio::test]
    async fn free_text_mcp_not_rewritten_by_toon_policy() {
        use crate::types::output::MCPOutput;
        use crate::util::toon::ENV_TOOL_RESULT_FORMAT;
        use crate::util::toon::test_env::{ENV_LOCK, EnvGuard};

        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env = EnvGuard::set(&[(ENV_TOOL_RESULT_FORMAT, Some("toon"))]);

        let plain = "issue created successfully\nno json here";
        let cfg = McpTruncateContext {
            max_output_bytes: 20_000,
            session_folder: None,
            shell_tool: "bash".to_string(),
            call_id: "call-plain".to_string(),
        };
        let out = truncate_tool_output(
            ToolOutput::MCP(MCPOutput::okay_output(
                "server__tool".into(),
                "server".into(),
                plain.to_owned(),
            )),
            &cfg,
        )
        .await;

        let ToolOutput::MCP(mcp) = out else {
            panic!("expected MCP");
        };
        let text = match mcp.output() {
            MCPOutputDetails::OkayOutput(t) | MCPOutputDetails::Error(t) => t,
        };
        assert_eq!(text, plain, "free text must not be rewritten");
    }
}
