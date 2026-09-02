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
