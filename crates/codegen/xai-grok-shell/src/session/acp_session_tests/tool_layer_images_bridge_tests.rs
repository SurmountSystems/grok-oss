//! Wiring tests for MCP tool-layer images through `handle_bridge_tool_success`.
use super::support::*;
use super::*;
use xai_grok_sampling_types::{ContentPart, ConversationItem};
use xai_grok_tools::types::output::{MCPOutput, ToolOutput, ToolRunResult};
use xai_grok_tools::util::base64_images::{ExtractedImage, IMAGE_CONTENT_PLACEHOLDER};
/// 32×32 solid PNG — above vision min side/area so normalize keeps it.
fn vision_ok_png_b64() -> String {
    use base64::Engine;
    use image::{ImageBuffer, Rgba};
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(32, 32, Rgba([128, 64, 32, 255]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode png");
    base64::engine::general_purpose::STANDARD.encode(buf)
}
fn mcp_screenshot_result(payload_b64: &str) -> ToolRunResult {
    let mut mcp = MCPOutput::okay_output(
        "browser_screenshot".into(),
        "browser-use".into(),
        IMAGE_CONTENT_PLACEHOLDER.into(),
    );
    mcp.extracted_images = vec![ExtractedImage {
        data: payload_b64.to_owned(),
        mime_type: "image/png".into(),
    }];
    ToolRunResult {
        output: ToolOutput::MCP(mcp),
        prompt_text: IMAGE_CONTENT_PLACEHOLDER.into(),
        effective_tool_name: None,
    }
}
fn tool_result_text(item: &ConversationItem) -> &str {
    match item {
        ConversationItem::ToolResult(tr) => tr.content.as_ref(),
        other => panic!("expected ToolResult, got {other:?}"),
    }
}
fn item_contains_data_image_url(item: &ConversationItem) -> bool {
    match item {
        ConversationItem::User(u) => u.content.iter().any(|p| match p {
            ContentPart::Image { url } => url.starts_with("data:"),
            ContentPart::Text { text } => text.contains("data:image"),
        }),
        ConversationItem::ToolResult(tr) => {
            tr.content.contains("data:image")
                || tr.images.iter().any(|p| match p {
                    ContentPart::Image { url } => url.starts_with("data:"),
                    ContentPart::Text { text } => text.contains("data:image"),
                })
        }
        _ => false,
    }
}

fn item_has_image_part(item: &ConversationItem) -> bool {
    match item {
        ConversationItem::User(u) => u
            .content
            .iter()
            .any(|p| matches!(p, ContentPart::Image { .. })),
        ConversationItem::ToolResult(tr) => tr
            .images
            .iter()
            .any(|p| matches!(p, ContentPart::Image { .. })),
        _ => false,
    }
}

fn parent_followup_reminder_text(item: &ConversationItem) -> Option<String> {
    match item {
        ConversationItem::User(u) => {
            let text = u
                .content
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_ref()),
                    ContentPart::Image { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            text.contains("Image extracted from tool result")
                .then_some(text)
        }
        _ => None,
    }
}

/// Owed: parent (`subagent_depth == 0`) multimodal MCP image is persist-plus-text.
/// Placeholder stays in tool text. Follow-up is path / persist-miss text, never
/// `ContentPart::Image` or `data:image`. Session `images/` has a file on persist.
#[tokio::test(flavor = "current_thread")]
async fn handle_bridge_tool_success_multimodal_mcp_image_deferred_followup() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.session_info.id =
                acp::SessionId::new(format!("bridge-mcp-img-parent-{}", std::process::id()));
            assert!(!actor.is_cursor_harness());
            assert_eq!(
                actor.tool_context.subagent_depth, 0,
                "this contract is the parent (main) session, not nested attach"
            );
            let session_dir = xai_grok_shared::session::session_dir(&actor.session_info);
            let images_dir = session_dir.join("images");
            let _ = std::fs::remove_dir_all(&session_dir);

            let payload = vision_ok_png_b64();
            let parsed_args = serde_json::json!({});
            let followups = actor
                .handle_bridge_tool_success(
                    &acp::ToolCallId::new("tc-mcp-img"),
                    "tc-mcp-img",
                    "browser_screenshot",
                    "browser_screenshot",
                    DrainedToolSuccess::new(mcp_screenshot_result(&payload)),
                    0,
                    "test-model",
                    &parsed_args,
                )
                .await
                .expect("bridge success");
            assert!(
                !followups.iter().any(item_contains_data_image_url),
                "parent follow-ups must not store data:image: {followups:?}"
            );
            assert!(
                !followups.iter().any(item_has_image_part),
                "parent follow-ups must not include image content parts: {followups:?}"
            );
            let reminder_text = followups
                .iter()
                .find_map(parent_followup_reminder_text)
                .expect("parent must get a short extracted-image reminder");
            let persist_ok = reminder_text.contains(&images_dir.display().to_string())
                || reminder_text.contains("Saved to");
            let persist_miss = reminder_text.contains("could not be saved");
            assert!(
                persist_ok || persist_miss,
                "parent follow-up must be persist-plus-path or persist-miss text: {reminder_text}"
            );

            let conv = actor.chat_state_handle.get_conversation().await;
            assert!(
                !conv.iter().any(item_contains_data_image_url),
                "parent conversation must not store data:image: {conv:?}"
            );
            assert!(
                !conv.iter().any(item_has_image_part),
                "parent conversation must not include image content parts: {conv:?}"
            );
            let tool = conv
                .iter()
                .rev()
                .find(|i| matches!(i, ConversationItem::ToolResult(_)))
                .expect("tool result pushed");
            let text = tool_result_text(tool);
            assert!(
                text.contains(IMAGE_CONTENT_PLACEHOLDER),
                "placeholder stays in tool text: {text}"
            );
            assert!(
                !text.contains("data:image"),
                "tool text must not reinject data URI: {text}"
            );
            assert!(
                !text.contains("image omitted"),
                "no budget-omit copy: {text}"
            );

            if persist_ok {
                let saved: Vec<_> = std::fs::read_dir(&images_dir)
                    .expect("session images directory must exist after persist")
                    .filter_map(|e| e.ok())
                    .collect();
                assert!(
                    !saved.is_empty(),
                    "persist must write a file under {}",
                    images_dir.display()
                );
            }
            let _ = std::fs::remove_dir_all(&session_dir);
        })
        .await;
}
