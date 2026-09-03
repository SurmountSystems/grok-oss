use super::*;
use xai_grok_sampling_types::ContentPart;

#[test]
fn compact_history_does_not_copy_the_data_url_crate() {
    let crate_url = format!("data:image/jpeg;base64,{}", "C".repeat(200_000));
    let source = vec![ConversationItem::user_with_parts(vec![
        ContentPart::Text { text: "see".into() },
        ContentPart::Image {
            url: crate_url.clone().into(),
        },
    ])];
    let prepared = build_compaction_chat_history(source, None, true, 0);
    let json = serde_json::to_string(&prepared.items).expect("serialize");
    assert!(
        !json.contains(&crate_url),
        "compact HTTP must not re-inline the data URL crate"
    );
    assert!(json.contains("[image]"));
    assert!(
        json.len() < 20_000,
        "compact history JSON must stay small, got {}",
        json.len()
    );
    assert_eq!(
        prepared.image_budget.inline_images, 0,
        "stripped compact history has no inline images for the 47MB budget"
    );
}

/// Operator contract: AUTO compact must send each `image_url` as a
/// base64-encoded image or a URL. A local session asset path, an
/// `[Image #N]` token, or an empty value must not reach the API.
#[test]
fn compact_request_must_not_send_session_asset_path_or_image_token_as_image_url() {
    let dir = std::env::temp_dir().join(format!("grok-compact-image-{}", std::process::id()));
    let assets = dir.join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    let asset = assets.join("image-operator.jpg");
    std::fs::write(&asset, b"jpeg-bytes").unwrap();
    let source = vec![
        ConversationItem::user_with_parts(vec![
            ContentPart::Text {
                text: "screenshot".into(),
            },
            ContentPart::Image {
                url: asset.to_string_lossy().as_ref().into(),
            },
        ]),
        ConversationItem::user_with_parts(vec![ContentPart::Image {
            url: "[Image #1]".into(),
        }]),
        ConversationItem::user_with_parts(vec![ContentPart::Image {
            url: format!("file://{}", asset.display()).into(),
        }]),
        ConversationItem::user_with_parts(vec![ContentPart::Image { url: "".into() }]),
    ];
    let prepared = build_compaction_chat_history(source, None, true, 0);
    let json = serde_json::to_string(&prepared.items).expect("serialize");
    assert!(
        !json.contains("image_url"),
        "compact HTTP must not include image_url after strip/repair, got {json}"
    );
    assert!(
        !json.contains(asset.to_string_lossy().as_ref()),
        "compact HTTP must not send the session asset path"
    );
    assert!(
        !json.contains("[Image #1]"),
        "compact HTTP must not send an [Image #N] token as image_url"
    );
    assert!(
        !json.contains("file://"),
        "compact HTTP must not send file:// as image_url"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_image_history_is_unchanged_before_prompt() {
    let source = vec![
        ConversationItem::system("system text"),
        ConversationItem::user("user text"),
        ConversationItem::assistant("assistant text"),
        ConversationItem::tool_result("call-1", "tool text"),
    ];
    let source_serialized = serde_json::to_value(&source).unwrap();
    let request = build_compaction_chat_history(source.clone(), None, true, 0);

    assert_eq!(request.image_budget.inline_images, 0);
    assert_eq!(
        serde_json::to_value(&request.items[..source.len()]).unwrap(),
        source_serialized
    );
}
