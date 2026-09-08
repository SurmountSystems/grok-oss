//! Tool-layer extracted-image helpers for the session tool pipeline.
//!
//! Drain / harness split plus persist-to-session-dir for parent leak stop.

use super::*;
use std::path::{Path, PathBuf};
use xai_grok_sampling_types::ContentPart;
use xai_grok_tools::util::base64_images::ExtractedImage;

/// Persist extracted tool-image bytes under session `images/`.
///
/// Returns the written path when the crate became a `file://` handle.
/// Parent conversation must never keep a `data:` URL; a persist miss
/// yields `None` so the caller can emit text only.
pub(super) fn persist_extracted_tool_image(
    mime_type: &str,
    base64_data: &str,
    session_images_dir: &Path,
) -> Option<PathBuf> {
    let data_url = format!("data:{mime_type};base64,{base64_data}");
    let handle = persist_inline_data_url(data_url, Some(session_images_dir));
    let path = handle.strip_prefix("file://")?;
    let path = PathBuf::from(path);
    path.is_file().then_some(path)
}

/// Parent follow-up is text (saved path). Nested may attach a `file://` handle.
pub(super) fn extracted_image_followup(saved_path: &Path, parent: bool) -> ConversationItem {
    if parent {
        ConversationItem::system_reminder(format!(
            "[Image extracted from tool result above. Saved to {}]",
            saved_path.display()
        ))
    } else {
        let mut item =
            ConversationItem::system_reminder("[Image extracted from tool result above]");
        item.add_image(format!("file://{}", saved_path.display()));
        item
    }
}

/// Parent persist miss: text only. Nested persist miss: keep a data-URL attach
/// so the nested session can still see the image until nested-attach owns it.
pub(super) fn extracted_image_persist_miss_followup(
    mime_type: &str,
    base64_data: &str,
    parent: bool,
) -> ConversationItem {
    if parent {
        ConversationItem::system_reminder(
            "[Image extracted from tool result above could not be saved to the session images directory]",
        )
    } else {
        let mut item =
            ConversationItem::system_reminder("[Image extracted from tool result above]");
        item.add_image(format!("data:{mime_type};base64,{base64_data}"));
        item
    }
}

/// Parent must not attach image parts on the tool result. Nested may attach
/// a `file://` handle (inflate on the inference HTTP clone).
pub(super) fn parent_or_nested_tool_image_part(
    saved_path: &Path,
    parent: bool,
) -> Option<ContentPart> {
    if parent {
        None
    } else {
        Some(ContentPart::Image {
            url: std::sync::Arc::<str>::from(format!("file://{}", saved_path.display())),
        })
    }
}

/// Drain pre-truncate image captures off the tool output for session vision.
pub(super) fn drain_tool_layer_extracted_images(
    output: &mut ToolsToolOutput,
) -> Vec<ExtractedImage> {
    match output {
        ToolsToolOutput::ReadFile(ReadFileOutput::FileContent(fc)) => {
            std::mem::take(&mut fc.extracted_images)
        }
        ToolsToolOutput::MCP(mcp) => std::mem::take(&mut mcp.extracted_images),
        _ => Vec::new(),
    }
}

/// `ToolRunResult` with tool-layer images already drained off `output`.
///
/// Construct only via [`Self::new`] so PostToolUse serialize and bridge success
/// handling cannot skip the harvest.
pub(super) struct DrainedToolSuccess {
    result: ToolRunResult,
    tool_layer_images: Vec<ExtractedImage>,
}

impl DrainedToolSuccess {
    #[must_use]
    pub(super) fn new(mut result: ToolRunResult) -> Self {
        let tool_layer_images = drain_tool_layer_extracted_images(&mut result.output);
        Self {
            result,
            tool_layer_images,
        }
    }

    /// Output after drain — for PostToolUse / hook-facing serialize.
    pub(super) fn output(&self) -> &ToolsToolOutput {
        &self.result.output
    }

    pub(super) fn into_parts(self) -> (ToolRunResult, Vec<ExtractedImage>) {
        (self.result, self.tool_layer_images)
    }
}

/// Multimodal harness: extend vision follow-ups. Text-only: discard (placeholders stay).
pub(super) fn split_tool_layer_for_harness(
    text_only_harness: bool,
    vision: &mut Vec<ExtractedImage>,
    tool_layer: Vec<ExtractedImage>,
) {
    if !text_only_harness {
        vision.extend(tool_layer);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DrainedToolSuccess, drain_tool_layer_extracted_images, extracted_image_followup,
        persist_extracted_tool_image, split_tool_layer_for_harness,
    };
    use std::path::PathBuf;
    use xai_grok_sampling_types::ContentPart;
    use xai_grok_tools::types::output::{
        FileContent, MCPOutput, ReadFileOutput, SearchToolOutput, ToolOutput, ToolRunResult,
    };
    use xai_grok_tools::util::base64_images::{ExtractedImage, IMAGE_CONTENT_PLACEHOLDER};

    fn img(data: &str, mime: &str) -> ExtractedImage {
        ExtractedImage {
            data: data.to_owned(),
            mime_type: mime.to_owned(),
        }
    }

    fn run_result(output: ToolOutput) -> ToolRunResult {
        ToolRunResult {
            output,
            prompt_text: "prompt".into(),
            effective_tool_name: None,
        }
    }

    #[test]
    fn drained_tool_success_new_drains_mcp_images() {
        let mut mcp = MCPOutput::okay_output(
            "browser_screenshot".into(),
            "browser-use".into(),
            IMAGE_CONTENT_PLACEHOLDER.into(),
        );
        let payload = "A".repeat(2000);
        mcp.extracted_images = vec![img(&payload, "image/png")];
        let drained = DrainedToolSuccess::new(run_result(ToolOutput::MCP(mcp)));

        let (result, images) = drained.into_parts();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime_type, "image/png");
        assert_eq!(images[0].data, payload);
        let ToolOutput::MCP(mcp) = result.output else {
            panic!("expected MCP");
        };
        assert!(mcp.extracted_images.is_empty());
    }

    #[test]
    fn drained_tool_success_new_drains_file_content_images() {
        let fc = FileContent {
            content: IMAGE_CONTENT_PLACEHOLDER.into(),
            content_concise: None,
            absolute_path: PathBuf::from("/tmp/x.png"),
            offset: None,
            limit: None,
            raw_output: String::new(),
            total_lines: 1,
            extracted_images: vec![img(&"B".repeat(3000), "image/jpeg")],
        };
        let drained = DrainedToolSuccess::new(run_result(ToolOutput::ReadFile(
            ReadFileOutput::FileContent(fc),
        )));

        assert!(matches!(
            drained.output(),
            ToolOutput::ReadFile(ReadFileOutput::FileContent(fc)) if fc.extracted_images.is_empty()
        ));
        let (result, images) = drained.into_parts();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime_type, "image/jpeg");
        assert_eq!(images[0].data, "B".repeat(3000));
        let ToolOutput::ReadFile(ReadFileOutput::FileContent(fc)) = result.output else {
            panic!("expected FileContent");
        };
        assert!(fc.extracted_images.is_empty());
    }

    #[test]
    fn harvests_mcp_images_and_drains() {
        let mut mcp = MCPOutput::okay_output(
            "browser_screenshot".into(),
            "browser-use".into(),
            IMAGE_CONTENT_PLACEHOLDER.into(),
        );
        mcp.extracted_images = vec![img(&"A".repeat(2000), "image/png")];
        let mut output = ToolOutput::MCP(mcp);

        let harvested = drain_tool_layer_extracted_images(&mut output);
        assert_eq!(harvested.len(), 1);
        assert_eq!(harvested[0].mime_type, "image/png");
        assert_eq!(harvested[0].data, "A".repeat(2000));

        let ToolOutput::MCP(mcp) = &output else {
            panic!("expected MCP");
        };
        assert!(
            mcp.extracted_images.is_empty(),
            "must drain so a second harvest cannot double-attach"
        );
    }

    #[test]
    fn harvests_file_content_images_and_drains() {
        let fc = FileContent {
            content: IMAGE_CONTENT_PLACEHOLDER.into(),
            content_concise: None,
            absolute_path: PathBuf::from("/tmp/x.png"),
            offset: None,
            limit: None,
            raw_output: String::new(),
            total_lines: 1,
            extracted_images: vec![img(&"B".repeat(3000), "image/jpeg")],
        };
        let mut output = ToolOutput::ReadFile(ReadFileOutput::FileContent(fc));

        let harvested = drain_tool_layer_extracted_images(&mut output);
        assert_eq!(harvested.len(), 1);
        assert_eq!(harvested[0].mime_type, "image/jpeg");
        assert_eq!(harvested[0].data, "B".repeat(3000));

        let ToolOutput::ReadFile(ReadFileOutput::FileContent(fc)) = &output else {
            panic!("expected FileContent");
        };
        assert!(fc.extracted_images.is_empty());
    }

    #[test]
    fn non_image_variants_yield_empty() {
        let mut output = ToolOutput::SearchTool(SearchToolOutput {
            result_count: 0,
            content: "no images".into(),
        });
        assert!(drain_tool_layer_extracted_images(&mut output).is_empty());
    }

    #[test]
    fn empty_mcp_extracted_images_yields_empty() {
        let mut output = ToolOutput::MCP(MCPOutput::okay_output(
            "t".into(),
            "s".into(),
            "plain text".into(),
        ));
        assert!(drain_tool_layer_extracted_images(&mut output).is_empty());
    }

    #[test]
    fn drained_tool_success_output_serialize_omits_extracted_images() {
        let mut mcp = MCPOutput::okay_output(
            "browser_screenshot".into(),
            "browser-use".into(),
            IMAGE_CONTENT_PLACEHOLDER.into(),
        );
        let payload = "P".repeat(80_000);
        mcp.extracted_images = vec![img(&payload, "image/png")];
        let before = serde_json::to_value(ToolOutput::MCP(mcp.clone())).unwrap();
        assert!(
            before.get("extracted_images").is_some(),
            "hub ToolDyn to_value must keep non-empty extracted_images key before drain"
        );
        assert!(
            before.to_string().contains(&payload),
            "hub ToolDyn to_value must keep non-empty extracted_images before drain"
        );

        let drained = DrainedToolSuccess::new(run_result(ToolOutput::MCP(mcp)));
        let after = serde_json::to_value(drained.output()).unwrap();
        assert!(
            after.get("extracted_images").is_none(),
            "PostToolUse path: DrainedToolSuccess::output must omit extracted_images"
        );
        assert!(
            !after.to_string().contains(&payload),
            "PostToolUse path: serialize must not peek multi-MB payload"
        );
        let (_result, images) = drained.into_parts();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].data, payload);
    }

    #[test]
    fn multimodal_split_extends_vision() {
        let mut vision = Vec::new();
        let tool_layer = vec![img(&"PNG".repeat(500), "image/png")];
        split_tool_layer_for_harness(false, &mut vision, tool_layer);
        assert_eq!(vision.len(), 1);
        assert_eq!(vision[0].mime_type, "image/png");
        assert_eq!(vision[0].data, "PNG".repeat(500));
    }

    #[test]
    fn multimodal_split_multiple_images_preserve_order() {
        let mut vision = Vec::new();
        let tool_layer = vec![
            img(&"AAA".repeat(100), "image/png"),
            img(&"BBB".repeat(100), "image/jpeg"),
        ];
        split_tool_layer_for_harness(false, &mut vision, tool_layer);
        assert_eq!(vision.len(), 2);
        assert_eq!(vision[0].mime_type, "image/png");
        assert_eq!(vision[0].data, "AAA".repeat(100));
        assert_eq!(vision[1].mime_type, "image/jpeg");
        assert_eq!(vision[1].data, "BBB".repeat(100));
    }

    #[test]
    fn multimodal_split_appends_after_existing_vision() {
        let mut vision = vec![img("from-text-extract", "image/webp")];
        let tool_layer = vec![img(&"tool".repeat(50), "image/png")];
        split_tool_layer_for_harness(false, &mut vision, tool_layer);
        assert_eq!(vision.len(), 2);
        assert_eq!(vision[0].mime_type, "image/webp");
        assert_eq!(vision[0].data, "from-text-extract");
        assert_eq!(vision[1].mime_type, "image/png");
        assert_eq!(vision[1].data, "tool".repeat(50));
    }

    #[test]
    fn text_only_split_discards_tool_layer_images() {
        let mut vision = Vec::new();
        let tool_layer = vec![img(&"JPG".repeat(500), "image/jpeg")];
        split_tool_layer_for_harness(true, &mut vision, tool_layer);
        assert!(vision.is_empty());
    }

    #[test]
    fn text_only_split_leaves_existing_vision_untouched() {
        let mut vision = vec![img("existing", "image/png")];
        let tool_layer = vec![img(&"JPG".repeat(500), "image/jpeg")];
        split_tool_layer_for_harness(true, &mut vision, tool_layer);
        assert_eq!(vision.len(), 1);
        assert_eq!(vision[0].mime_type, "image/png");
        assert_eq!(vision[0].data, "existing");
    }

    #[test]
    fn persist_extracted_tool_image_writes_session_file() {
        let dir =
            std::env::temp_dir().join(format!("grok-tool-img-persist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let saved = persist_extracted_tool_image("image/png", "AAAA", &dir)
            .expect("persist must write a session file");
        assert!(
            saved.is_file(),
            "saved path must exist: {}",
            saved.display()
        );
        assert!(
            saved.starts_with(&dir),
            "must persist under the session images dir"
        );
        let followup = extracted_image_followup(&saved, true);
        match &followup {
            xai_grok_sampling_types::ConversationItem::User(u) => {
                assert!(
                    u.content
                        .iter()
                        .all(|p| !matches!(p, ContentPart::Image { .. })),
                    "parent follow-up must be text-only: {followup:?}"
                );
                let text = u
                    .content
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_ref()),
                        ContentPart::Image { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    text.contains(&saved.display().to_string()),
                    "parent reminder must name the saved path: {text}"
                );
                assert!(!text.contains("data:image"), "must not leak data URL text");
            }
            other => panic!("expected user reminder, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
