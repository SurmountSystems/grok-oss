//! Parent (main) conversation must not store `data:image/...;base64` from
//! tool-extracted images. Persist a session file. Parent sees a path or a
//! short system reminder.
use super::support::*;
use super::*;
use xai_chat_state::estimate_item_tokens;
use xai_grok_sampling_types::{ContentPart, ConversationItem};
use xai_grok_tools::types::output::{MCPOutput, ReadFileOutput, ToolOutput, ToolRunResult};
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

fn read_file_image_result(payload_b64: &str) -> ToolRunResult {
    ToolRunResult {
        output: ToolOutput::ReadFile(ReadFileOutput::ImageContent(
            xai_grok_tools::types::output::ImageContent {
                data: payload_b64.to_owned(),
                mime_type: "image/png".into(),
                annotations: None,
                uri: None,
                meta: None,
            },
        )),
        prompt_text: "image".into(),
        effective_tool_name: None,
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

/// Owed: a tool-extracted image must not leave `data:image/...;base64` on the
/// parent (main) conversation. Persist a session `images/` file. Parent sees
/// a path or a short system reminder, never an image content part.
#[tokio::test(flavor = "current_thread")]
async fn tool_extracted_image_does_not_leave_data_url_on_parent_conversation() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.session_info.id =
                acp::SessionId::new(format!("tool-extracted-parent-{}", std::process::id()));
            assert_eq!(
                actor.tool_context.subagent_depth, 0,
                "this contract is the parent (main) session"
            );
            let session_dir = xai_grok_shared::session::session_dir(&actor.session_info);
            let images_dir = session_dir.join("images");
            let _ = std::fs::remove_dir_all(&session_dir);

            let payload = vision_ok_png_b64();
            let parsed_args = serde_json::json!({});
            let followups = actor
                .handle_bridge_tool_success(
                    &acp::ToolCallId::new("tc-parent-img"),
                    "tc-parent-img",
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
                .find_map(|item| match item {
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
                })
                .expect("parent must get a short extracted-image reminder");
            assert!(
                reminder_text.contains(&images_dir.display().to_string())
                    || reminder_text.contains("Saved to"),
                "parent reminder must name the saved session path: {reminder_text}"
            );

            let conv = actor.chat_state_handle.get_conversation().await;
            assert!(
                !conv.iter().any(item_contains_data_image_url),
                "parent conversation must not store data:image: {conv:?}"
            );
            let tool = conv
                .iter()
                .rev()
                .find(|i| matches!(i, ConversationItem::ToolResult(_)))
                .expect("tool result pushed");
            assert!(
                !item_has_image_part(tool),
                "parent tool result must not attach image parts: {tool:?}"
            );
            match tool {
                ConversationItem::ToolResult(tr) => {
                    assert!(
                        !tr.content.contains("data:image"),
                        "tool text must not reinject data URI: {}",
                        tr.content
                    );
                }
                other => panic!("expected ToolResult, got {other:?}"),
            }

            let saved: Vec<_> = std::fs::read_dir(&images_dir)
                .expect("session images directory must exist after persist")
                .filter_map(|e| e.ok())
                .collect();
            assert!(
                !saved.is_empty(),
                "persist must write a file under {}",
                images_dir.display()
            );
            let _ = std::fs::remove_dir_all(&session_dir);
        })
        .await;
}

/// Owed: `estimate_item_tokens` charges 765 only for `ContentPart::Image`.
/// A parent user item that is text-only after persist (path / short
/// description) must not add that image-token charge.
#[tokio::test(flavor = "current_thread")]
async fn parent_text_only_user_item_after_describe_does_not_add_image_token_charge() {
    let text =
        "[Image extracted from tool result above. Saved to /tmp/sessions/id/images/image-uuid.png]";
    let text_only = ConversationItem::system_reminder(text);
    let with_image = {
        let mut item = ConversationItem::system_reminder(text);
        item.add_image("data:image/png;base64,AAAA");
        item
    };
    let text_tokens = estimate_item_tokens(&text_only);
    let image_tokens = estimate_item_tokens(&with_image);
    assert_eq!(
        image_tokens - text_tokens,
        xai_token_estimation::IMAGE_TOKEN_ESTIMATE,
        "ContentPart::Image still charges IMAGE_TOKEN_ESTIMATE (765); do not change that constant"
    );
    assert!(
        text_tokens < xai_token_estimation::IMAGE_TOKEN_ESTIMATE,
        "text-only parent reminder must not add the 765 image-token charge, got {text_tokens}"
    );

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.session_info.id = acp::SessionId::new(format!(
                "tool-extracted-parent-tokens-{}",
                std::process::id()
            ));
            assert_eq!(actor.tool_context.subagent_depth, 0);
            let session_dir = xai_grok_shared::session::session_dir(&actor.session_info);
            let _ = std::fs::remove_dir_all(&session_dir);

            let payload = vision_ok_png_b64();
            let followups = actor
                .handle_bridge_tool_success(
                    &acp::ToolCallId::new("tc-parent-tokens"),
                    "tc-parent-tokens",
                    "browser_screenshot",
                    "browser_screenshot",
                    DrainedToolSuccess::new(mcp_screenshot_result(&payload)),
                    0,
                    "test-model",
                    &serde_json::json!({}),
                )
                .await
                .expect("bridge success");
            let reminder = followups
                .iter()
                .find(|item| matches!(item, ConversationItem::User(_)))
                .expect("parent reminder follow-up");
            let tokens = estimate_item_tokens(reminder);
            assert!(
                !item_has_image_part(reminder),
                "parent reminder after persist must be text-only: {reminder:?}"
            );
            assert!(
                tokens < xai_token_estimation::IMAGE_TOKEN_ESTIMATE,
                "parent text-only follow-up must not add 765 image tokens, got {tokens}"
            );
            let _ = std::fs::remove_dir_all(&session_dir);
        })
        .await;
}

/// Owed: `read_file` of an image on the parent session persists a session
/// file and must not leave `data:image` on the parent conversation item.
#[tokio::test(flavor = "current_thread")]
async fn read_file_image_does_not_leave_data_url_on_parent_conversation() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.session_info.id =
                acp::SessionId::new(format!("read-file-image-parent-{}", std::process::id()));
            assert_eq!(actor.tool_context.subagent_depth, 0);
            let session_dir = xai_grok_shared::session::session_dir(&actor.session_info);
            let images_dir = session_dir.join("images");
            let _ = std::fs::remove_dir_all(&session_dir);

            let payload = vision_ok_png_b64();
            let parsed_args = serde_json::json!({ "target_file": "/tmp/shot.png" });
            let followups = actor
                .handle_bridge_tool_success(
                    &acp::ToolCallId::new("tc-read-img"),
                    "tc-read-img",
                    "read_file",
                    "read_file",
                    DrainedToolSuccess::new(read_file_image_result(&payload)),
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

            let conv = actor.chat_state_handle.get_conversation().await;
            assert!(
                !conv.iter().any(item_contains_data_image_url),
                "parent conversation must not store data:image after read_file image: {conv:?}"
            );
            let tool = conv
                .iter()
                .rev()
                .find(|i| matches!(i, ConversationItem::ToolResult(_)))
                .expect("tool result pushed");
            assert!(
                !item_has_image_part(tool),
                "parent read_file image must not attach image parts: {tool:?}"
            );
            match tool {
                ConversationItem::ToolResult(tr) => {
                    assert!(
                        tr.content.contains("/tmp/shot.png")
                            || tr.content.contains(&images_dir.display().to_string())
                            || tr.content.contains("Saved to"),
                        "parent tool text must name the source or saved path: {}",
                        tr.content
                    );
                    assert!(
                        !tr.content.contains("data:image"),
                        "tool text must not carry a data URI: {}",
                        tr.content
                    );
                }
                other => panic!("expected ToolResult, got {other:?}"),
            }
            let saved: Vec<_> = std::fs::read_dir(&images_dir)
                .expect("session images directory must exist after persist")
                .filter_map(|e| e.ok())
                .collect();
            assert!(
                !saved.is_empty(),
                "read_file image must persist under {}",
                images_dir.display()
            );
            let _ = std::fs::remove_dir_all(&session_dir);
        })
        .await;
}
