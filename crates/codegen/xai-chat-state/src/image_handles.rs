//! Session image handles vs inference data URLs.
//!
//! Conversation items persist a `file://` handle under session `images/`.
//! Inflate to a `data:` URL only when building the inference HTTP body.
//! Compact/history must not re-inline the crate. The model still receives a
//! content part (`input_image` / `image_url`), not `view_image`.
//!
//! The API accepts only a base64 data URL or an `http(s)` URL as `image_url`.
//! A local session asset path, an `[Image #N]` token, or an empty value 400s
//! (`invalid_image`). Request clones must convert or omit those; they must
//! not send them.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use xai_grok_sampling_types::{ContentPart, ConversationItem, image_url_is_api_accepted};

/// Placeholder when a user image cannot be sent as an API `image_url`.
const OMITTED_IMAGE_PLACEHOLDER: &str = "[image]";

/// Counts from [`repair_conversation_images_for_api`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImageRepairStats {
    pub converted: usize,
    pub omitted: usize,
}

/// True when `url` is an inline `data:image/...;base64,...` crate.
pub fn is_inline_data_url(url: &str) -> bool {
    let Some((header, payload)) = url.split_once(',') else {
        return false;
    };
    header.starts_with("data:") && header.ends_with(";base64") && !payload.is_empty()
}

/// Inflate session image handles to data URLs on a request clone, and omit
/// values the API would reject as `image_url`.
///
/// Stored conversation is left unchanged. HTTP(S) and existing data URLs
/// pass through. A missing file, `[Image #N]` token, empty value, or other
/// non-URL is omitted from this clone rather than sent.
pub fn inflate_conversation_images_for_inference(items: &mut [ConversationItem]) {
    let _ = repair_conversation_images_for_api(items);
}

/// Convert local image handles to data URLs, or omit them, on a request clone.
///
/// Compact, recap, and turn HTTP must not send `file://`, a raw session
/// asset path, an `[Image #N]` token, or an empty `image_url`.
pub fn repair_conversation_images_for_api(items: &mut [ConversationItem]) -> ImageRepairStats {
    let mut stats = ImageRepairStats::default();
    for item in items {
        match item {
            ConversationItem::User(user) => repair_user_parts(&mut user.content, &mut stats),
            ConversationItem::ToolResult(tool) => repair_tool_images(&mut tool.images, &mut stats),
            ConversationItem::System(_)
            | ConversationItem::Assistant(_)
            | ConversationItem::BackendToolCall(_)
            | ConversationItem::Reasoning(_) => {}
        }
    }
    stats
}

fn repair_user_parts(parts: &mut [ContentPart], stats: &mut ImageRepairStats) {
    for part in parts.iter_mut() {
        let ContentPart::Image { url } = part else {
            continue;
        };
        match api_image_url_or_omit(url) {
            Some(repaired) if repaired.as_str() == url.as_ref() => {}
            Some(repaired) => {
                *url = repaired.into();
                stats.converted += 1;
            }
            None => {
                *part = ContentPart::Text {
                    text: OMITTED_IMAGE_PLACEHOLDER.into(),
                };
                stats.omitted += 1;
            }
        }
    }
}

fn repair_tool_images(parts: &mut Vec<ContentPart>, stats: &mut ImageRepairStats) {
    let mut kept = Vec::with_capacity(parts.len());
    for part in parts.drain(..) {
        let ContentPart::Image { url } = &part else {
            kept.push(part);
            continue;
        };
        match api_image_url_or_omit(url) {
            Some(repaired) if repaired.as_str() == url.as_ref() => kept.push(part),
            Some(repaired) => {
                kept.push(ContentPart::Image {
                    url: repaired.into(),
                });
                stats.converted += 1;
            }
            None => {
                stats.omitted += 1;
            }
        }
    }
    *parts = kept;
}

fn api_image_url_or_omit(url: &str) -> Option<String> {
    if image_url_is_api_accepted(url) {
        return Some(url.to_owned());
    }
    if is_image_token(url) {
        return None;
    }
    let path = local_path_from_image_url(url)?;
    if !path.is_file() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let mime = mime_from_path(&path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

fn is_image_token(url: &str) -> bool {
    let t = url.trim();
    t.starts_with("[Image #") || t.eq_ignore_ascii_case("[image]")
}

fn local_path_from_image_url(url: &str) -> Option<PathBuf> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    // Persist writes `file://{path.display()}` and does not URL-encode.
    // Session cwd folders contain literal `%2F` in the name. Do not
    // percent-decode: `%2F` would become `/` and miss the file.
    if let Some(rest) = url.strip_prefix("file://") {
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        if rest.is_empty() {
            return None;
        }
        return Some(PathBuf::from(rest));
    }
    let path = Path::new(url);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        None
    }
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

    fn user_with_image_url(url: &str) -> Vec<ConversationItem> {
        vec![ConversationItem::user_with_parts(vec![
            ContentPart::Text { text: "see".into() },
            ContentPart::Image { url: url.into() },
        ])]
    }

    fn image_url_of(items: &[ConversationItem]) -> Option<String> {
        match items.first()? {
            ConversationItem::User(u) => u.content.iter().find_map(|p| match p {
                ContentPart::Image { url } => Some(url.as_ref().to_owned()),
                _ => None,
            }),
            _ => None,
        }
    }

    #[test]
    fn inflate_reads_session_file_and_does_not_require_stored_data_url() {
        let dir = std::env::temp_dir().join(format!("grok-image-handle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("image-test.png");
        std::fs::write(&path, b"png-bytes").unwrap();
        let handle = format!("file://{}", path.display());
        let mut items = user_with_image_url(&handle);
        inflate_conversation_images_for_inference(&mut items);
        let url = image_url_of(&items).expect("image part");
        assert!(
            url.starts_with("data:image/png;base64,"),
            "inference clone must inflate the handle, got {url}"
        );
        assert!(!url.contains("file://"), "must not send file:// to the API");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inflate_leaves_https_and_data_urls() {
        let https = "https://example.com/x.png";
        let data = "data:image/png;base64,AAAA";
        assert_eq!(api_image_url_or_omit(https).as_deref(), Some(https));
        assert_eq!(api_image_url_or_omit(data).as_deref(), Some(data));
    }

    /// Operator contract: compact/summarize/turn HTTP must send `image_url` as
    /// a base64-encoded image or a URL. A local session asset path must be
    /// encoded, not forwarded.
    #[test]
    fn repair_encodes_raw_session_asset_path_as_data_url() {
        let dir = std::env::temp_dir().join(format!("grok-image-asset-{}", std::process::id()));
        let assets = dir.join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        let path = assets.join("image-operator.jpg");
        std::fs::write(&path, b"jpeg-bytes").unwrap();
        let mut items = user_with_image_url(&path.to_string_lossy());
        let stats = repair_conversation_images_for_api(&mut items);
        assert_eq!(stats.converted, 1);
        assert_eq!(stats.omitted, 0);
        let url = image_url_of(&items).expect("encoded image");
        assert!(
            url.starts_with("data:image/jpeg;base64,"),
            "raw session asset path must become a data URL, got {url}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repair_omits_image_token_empty_and_missing_path() {
        let mut token = user_with_image_url("[Image #1]");
        let mut empty = user_with_image_url("");
        let mut missing = user_with_image_url("/tmp/grok-missing-image-does-not-exist.jpg");
        assert_eq!(repair_conversation_images_for_api(&mut token).omitted, 1);
        assert_eq!(repair_conversation_images_for_api(&mut empty).omitted, 1);
        assert_eq!(repair_conversation_images_for_api(&mut missing).omitted, 1);
        assert!(
            image_url_of(&token).is_none(),
            "[Image #N] must not be sent as image_url"
        );
        assert!(image_url_of(&empty).is_none());
        assert!(image_url_of(&missing).is_none());
    }

    /// Persist-shaped handle: session cwd folders contain literal `%2F`.
    /// Inflate must not percent-decode that name into `/`.
    #[test]
    fn repair_inflates_file_handle_with_literal_percent_2f_session_dir() {
        let dir = std::env::temp_dir().join(format!(
            "grok-image-%2Fhome%2Fsession-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("image-test.png");
        std::fs::write(&path, b"png-bytes").unwrap();
        assert!(
            path.to_string_lossy().contains("%2F"),
            "fixture dir name must contain literal %2F like sessions_cwd_dir"
        );
        let handle = format!("file://{}", path.display());
        assert!(
            handle.contains("%2F"),
            "persist writes path.display() without URL-decoding %2F"
        );
        let mut items = user_with_image_url(&handle);
        let stats = repair_conversation_images_for_api(&mut items);
        assert_eq!(
            stats.converted, 1,
            "literal %2F in the session dir must still inflate, omitted={}",
            stats.omitted
        );
        let url = image_url_of(&items).expect("encoded image");
        assert!(
            url.starts_with("data:image/png;base64,"),
            "persist file:// handle must become a data URL, got {url}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
