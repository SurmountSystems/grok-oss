//! Session image handles vs inference data URLs.
//!
//! Conversation items persist a `file://` handle under session `images/`.
//! Inflate to a `data:` URL only when building the inference HTTP body.
//! Compact/history must not re-inline the crate. The model still receives a
//! content part (`input_image` / `image_url`), not `view_image`.

use std::path::Path;

use base64::Engine as _;
use xai_grok_sampling_types::{ContentPart, ConversationItem};

/// True when `url` is an inline `data:image/...;base64,...` crate.
pub fn is_inline_data_url(url: &str) -> bool {
    let Some((header, _)) = url.split_once(',') else {
        return false;
    };
    header.starts_with("data:") && header.ends_with(";base64")
}

/// Inflate `file://` session image handles to data URLs on a request clone.
///
/// Stored conversation is left unchanged. HTTP(S) and existing data URLs
/// pass through. A missing file keeps the handle (the API will reject it
/// rather than invent bytes).
pub fn inflate_conversation_images_for_inference(items: &mut [ConversationItem]) {
    for item in items {
        match item {
            ConversationItem::User(user) => inflate_parts(&mut user.content),
            ConversationItem::ToolResult(tool) => inflate_parts(&mut tool.images),
            ConversationItem::System(_)
            | ConversationItem::Assistant(_)
            | ConversationItem::BackendToolCall(_)
            | ConversationItem::Reasoning(_) => {}
        }
    }
}

fn inflate_parts(parts: &mut [ContentPart]) {
    for part in parts {
        if let ContentPart::Image { url } = part {
            *url = inflate_image_url(url).into();
        }
    }
}

fn inflate_image_url(url: &str) -> String {
    if is_inline_data_url(url) || url.starts_with("http://") || url.starts_with("https://") {
        return url.to_owned();
    }
    let Some(path) = path_from_file_url(url) else {
        return url.to_owned();
    };
    if !path.is_file() {
        return url.to_owned();
    }
    let Ok(bytes) = std::fs::read(path) else {
        return url.to_owned();
    };
    let mime = mime_from_path(path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:{mime};base64,{b64}")
}

fn path_from_file_url(url: &str) -> Option<&Path> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    Some(Path::new(rest))
}

fn mime_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflate_reads_session_file_and_does_not_require_stored_data_url() {
        let dir = std::env::temp_dir().join(format!("grok-image-handle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("image-test.png");
        std::fs::write(&path, b"png-bytes").unwrap();
        let handle = format!("file://{}", path.display());
        let mut items = vec![ConversationItem::user_with_parts(vec![
            ContentPart::Text { text: "see".into() },
            ContentPart::Image {
                url: handle.clone().into(),
            },
        ])];
        inflate_conversation_images_for_inference(&mut items);
        match &items[0] {
            ConversationItem::User(u) => match &u.content[1] {
                ContentPart::Image { url } => {
                    assert!(
                        url.starts_with("data:image/png;base64,"),
                        "inference clone must inflate the handle, got {url}"
                    );
                    assert!(!url.contains("file://"), "must not send file:// to the API");
                }
                other => panic!("expected image, got {other:?}"),
            },
            other => panic!("expected user, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inflate_leaves_https_and_data_urls() {
        let https = "https://example.com/x.png";
        let data = "data:image/png;base64,AAAA";
        assert_eq!(inflate_image_url(https), https);
        assert_eq!(inflate_image_url(data), data);
    }
}
