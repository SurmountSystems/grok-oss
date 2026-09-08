//! Named saved-image handles for nested spawn / fork.
//!
//! Spawn and fork background strings name session files (`[Image #N]` plus
//! an absolute path). Nested first user turns attach those files as
//! `file://` `ContentPart::Image`. Bytes and `data:image` URLs stay out of
//! the prompt string.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use xai_grok_sampling_types::conversation::{ContentPart, ConversationItem};

/// A parent conversation image that already lives on disk (session `images/`
/// or `assets/`). Spawn prompt strings name these; nested turns attach
/// `file://` handles. Never a `data:` URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSavedImage {
    /// 1-based `[Image #N]` index in parent encounter order.
    pub number: usize,
    /// Absolute saved file path.
    pub path: PathBuf,
}

impl NamedSavedImage {
    /// Conversation / inflate handle. Persist writes `file://{path}` without encoding.
    pub fn file_url(&self) -> String {
        format!("file://{}", self.path.display())
    }

    pub(crate) fn matches_url(&self, url: &str) -> bool {
        saved_local_image_path(url).is_some_and(|p| p == self.path)
    }
}

/// Collect saved local images from parent items, numbered in encounter order.
///
/// Walks `ContentPart::Image` and parent text: `<image_files>` envelopes
/// (absolute paths under session `assets/` or `images/`) and
/// `[Image #N] <absolute path>` tokens. Skips `data:` URLs, bare
/// `[Image #N]` tokens, and remote `http(s)` URLs so the spawn string
/// never has to copy bytes.
pub fn collect_named_saved_images<'a>(
    items: impl IntoIterator<Item = &'a ConversationItem>,
) -> Vec<NamedSavedImage> {
    let mut catalog = Vec::new();
    for item in items {
        match item {
            ConversationItem::User(u) => {
                for part in &u.content {
                    match part {
                        ContentPart::Text { text } => {
                            harvest_saved_paths_from_text(&mut catalog, text);
                        }
                        ContentPart::Image { url } => {
                            push_named_saved(&mut catalog, url);
                        }
                    }
                }
            }
            ConversationItem::ToolResult(tr) => {
                harvest_saved_paths_from_text(&mut catalog, &tr.content);
                for part in &tr.images {
                    if let ContentPart::Image { url } = part {
                        push_named_saved(&mut catalog, url);
                    }
                }
            }
            _ => {}
        }
    }
    catalog
}

fn push_named_saved(catalog: &mut Vec<NamedSavedImage>, url: &str) {
    let Some(path) = saved_local_image_path(url) else {
        return;
    };
    push_named_saved_path(catalog, path);
}

fn push_named_saved_path(catalog: &mut Vec<NamedSavedImage>, path: PathBuf) {
    if catalog.iter().any(|img| img.path == path) {
        return;
    }
    catalog.push(NamedSavedImage {
        number: catalog.len() + 1,
        path,
    });
}

/// Harvest session-saved paths from parent text in appearance order.
fn harvest_saved_paths_from_text(catalog: &mut Vec<NamedSavedImage>, text: &str) {
    let mut hits: Vec<(usize, PathBuf)> = Vec::new();
    collect_image_files_envelope_paths(text, &mut hits);
    collect_named_image_token_paths(text, &mut hits);
    hits.sort_by_key(|(start, _)| *start);
    for (_, path) in hits {
        push_named_saved_path(catalog, path);
    }
}

/// `<image_files>` numbered lines from `render_image_files_block`.
fn collect_image_files_envelope_paths(text: &str, hits: &mut Vec<(usize, PathBuf)>) {
    const OPEN: &str = "<image_files>";
    const CLOSE: &str = "</image_files>";
    let mut search = 0;
    while let Some(open_rel) = text[search..].find(OPEN) {
        let interior_start = search + open_rel + OPEN.len();
        let Some(close_rel) = text[interior_start..].find(CLOSE) else {
            break;
        };
        let interior = &text[interior_start..interior_start + close_rel];
        let mut line_start = interior_start;
        for line in interior.split('\n') {
            let trimmed = line.trim();
            if let Some(path) = numbered_envelope_path(trimmed)
                && is_under_session_saved_dir(&path)
            {
                hits.push((line_start, path));
            }
            line_start += line.len() + 1;
        }
        search = interior_start + close_rel + CLOSE.len();
    }
}

/// `N. <absolute path>` as written by the parent `<image_files>` envelope.
fn numbered_envelope_path(line: &str) -> Option<PathBuf> {
    let dot = line.find(". ")?;
    if dot == 0 || !line[..dot].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    saved_local_image_path(line[dot + 2..].trim())
}

fn is_under_session_saved_dir(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(
            c,
            std::path::Component::Normal(name) if name == "assets" || name == "images"
        )
    })
}

/// `[Image #N] <absolute path>` as written by [`format_named_image_token`].
fn collect_named_image_token_paths(text: &str, hits: &mut Vec<(usize, PathBuf)>) {
    let marker = "[Image #";
    let mut search = 0;
    while let Some(idx) = text[search..].find(marker) {
        let token_start = search + idx;
        let after_marker = token_start + marker.len();
        let rest = &text[after_marker..];
        let digit_end = rest
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_digit())
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        if digit_end == 0 || !rest[digit_end..].starts_with(']') {
            search = after_marker;
            continue;
        }
        let after_bracket = after_marker + digit_end + 1;
        let after = &text[after_bracket..];
        if !after.starts_with(' ') {
            search = after_bracket;
            continue;
        }
        let path_start = after_bracket + 1;
        let line_end = text[path_start..]
            .find('\n')
            .map(|i| path_start + i)
            .unwrap_or(text.len());
        let candidate = text[path_start..line_end].trim();
        if let Some(path) = saved_local_image_path(candidate) {
            hits.push((token_start, path));
        }
        search = after_bracket;
    }
}

/// Absolute path for a session-saved image handle. `None` for data URLs,
/// image tokens, and remote URLs.
pub fn saved_local_image_path(url: &str) -> Option<PathBuf> {
    let url = url.trim();
    if url.is_empty() || url.starts_with("data:") || url.starts_with("[Image #") {
        return None;
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return None;
    }
    if let Some(rest) = url.strip_prefix("file://") {
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        if rest.is_empty() {
            return None;
        }
        let path = PathBuf::from(rest);
        return path.is_absolute().then_some(path);
    }
    let path = Path::new(url);
    path.is_absolute().then_some(path.to_path_buf())
}

/// `[Image #N]` display numbers named in a spawn / task prompt string.
pub fn image_numbers_named_in_text(text: &str) -> Vec<usize> {
    let mut numbers = Vec::new();
    let mut rest = text;
    let marker = "[Image #";
    while let Some(idx) = rest.find(marker) {
        rest = &rest[idx + marker.len()..];
        let digit_end = rest
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_digit())
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        if digit_end == 0 || !rest[digit_end..].starts_with(']') {
            continue;
        }
        if let Ok(n) = rest[..digit_end].parse::<usize>()
            && n > 0
            && !numbers.contains(&n)
        {
            numbers.push(n);
        }
        rest = &rest[digit_end..];
    }
    numbers
}

/// `ContentPart::Image` file:// handles for images named in `spawn_prompt`.
///
/// Nested sessions pass these as `user_images` / extra image parts. Unnamed
/// parent images are omitted.
pub fn image_parts_for_named_spawn_prompt(
    spawn_prompt: &str,
    catalog: &[NamedSavedImage],
) -> Vec<ContentPart> {
    image_parts_from_numbers(&image_numbers_named_in_text(spawn_prompt), catalog)
}

/// `file://` handles for images named in `spawn_prompt`, for nested `user_images`.
pub fn file_urls_for_named_spawn_prompt(
    spawn_prompt: &str,
    catalog: &[NamedSavedImage],
) -> Vec<String> {
    image_parts_for_named_spawn_prompt(spawn_prompt, catalog)
        .into_iter()
        .filter_map(|part| match part {
            ContentPart::Image { url } => Some(url.as_ref().to_owned()),
            ContentPart::Text { .. } => None,
        })
        .collect()
}

pub(crate) fn image_parts_from_numbers(
    numbers: &[usize],
    catalog: &[NamedSavedImage],
) -> Vec<ContentPart> {
    let mut seen = BTreeSet::new();
    let mut parts = Vec::new();
    for number in numbers {
        if !seen.insert(*number) {
            continue;
        }
        let Some(img) = catalog.iter().find(|i| i.number == *number) else {
            continue;
        };
        parts.push(ContentPart::Image {
            url: img.file_url().into(),
        });
    }
    parts
}

/// Token written into spawn / fork background text. Never a data URL.
pub fn format_named_image_token(image: &NamedSavedImage) -> String {
    format!("[Image #{}] {}", image.number, image.path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::conversation::{ConversationItem, ConversationRequest};

    fn user_with_saved_image(text: &str, path: &Path) -> ConversationItem {
        ConversationItem::user_with_parts(vec![
            ContentPart::Text { text: text.into() },
            ContentPart::Image {
                url: format!("file://{}", path.display()).into(),
            },
        ])
    }

    fn image_urls(item: &ConversationItem) -> Vec<String> {
        match item {
            ConversationItem::User(u) => u
                .content
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Image { url } => Some(url.as_ref().to_owned()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Same envelope grammar as `render_image_files_block`.
    fn image_files_envelope(paths: &[&Path]) -> String {
        let mut out = String::from(
            "<image_files>\nThe following images were provided by the user and saved to the workspace for future use:\n",
        );
        for (i, p) in paths.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, p.display()));
        }
        out.push_str("\nThese images can be copied for use in other locations.\n</image_files>");
        out
    }

    #[test]
    fn spawn_prompt_string_names_path_and_omits_data_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("images").join("shot-1.png");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"png-bytes").unwrap();
        let items = vec![user_with_saved_image("look", &path)];
        let catalog = collect_named_saved_images(&items);
        let token = format_named_image_token(&catalog[0]);
        assert!(token.contains("[Image #1]"));
        assert!(token.contains(&path.display().to_string()));
        assert!(!token.contains("data:image"));
        assert!(!token.contains("png-bytes"));
        let data_item = ConversationItem::user_with_parts(vec![ContentPart::Image {
            url: "data:image/png;base64,QUJDRA==".into(),
        }]);
        let data_catalog = collect_named_saved_images(std::slice::from_ref(&data_item));
        assert!(data_catalog.is_empty());
    }

    #[test]
    fn nested_user_turn_attaches_file_image_not_data_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assets").join("nested.png");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"png-bytes").unwrap();
        let parent = vec![user_with_saved_image("see screenshot", &path)];
        let catalog = collect_named_saved_images(&parent);
        let spawn = format!("Inspect [Image #1] {}\nDo not copy bytes.", path.display());
        assert!(!spawn.contains("data:image"));
        let parts = image_parts_for_named_spawn_prompt(&spawn, &catalog);
        let mut user_parts = vec![ContentPart::Text { text: spawn.into() }];
        user_parts.extend(parts);
        let request =
            ConversationRequest::from_items(vec![ConversationItem::user_with_parts(user_parts)]);
        let urls = image_urls(&request.items[0]);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], format!("file://{}", path.display()));
        assert!(!urls[0].contains("data:"));
        match &request.items[0] {
            ConversationItem::User(u) => {
                let text: String = u
                    .content
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_ref()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                assert!(!text.contains("data:image"));
                assert!(text.contains("[Image #1]"));
                assert!(text.contains(&path.display().to_string()));
            }
            _ => panic!("expected user item"),
        }
    }

    #[test]
    fn extra_parent_images_are_not_attached_unless_named_in_spawn_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let job = dir.path().join("images").join("job.png");
        let extra = dir.path().join("images").join("unrelated.png");
        std::fs::create_dir_all(job.parent().unwrap()).unwrap();
        std::fs::write(&job, b"job").unwrap();
        std::fs::write(&extra, b"extra").unwrap();
        let parent = vec![
            user_with_saved_image("job shot", &job),
            user_with_saved_image("unrelated shot", &extra),
        ];
        let catalog = collect_named_saved_images(&parent);
        assert_eq!(catalog.len(), 2);
        let spawn = format!("Use [Image #1] {} for this job only.", job.display());
        let parts = image_parts_for_named_spawn_prompt(&spawn, &catalog);
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            ContentPart::Image { url } => {
                assert_eq!(url.as_ref(), format!("file://{}", job.display()));
                assert!(!url.contains("unrelated.png"));
            }
            _ => panic!("expected image part"),
        }
        let urls = file_urls_for_named_spawn_prompt(&spawn, &catalog);
        assert_eq!(urls, vec![format!("file://{}", job.display())]);
    }

    #[test]
    fn text_only_parent_named_image_attaches_file_url_unnamed_extra_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        let job = assets.join("job.png");
        let extra = assets.join("unrelated.png");
        std::fs::write(&job, b"job").unwrap();
        std::fs::write(&extra, b"extra").unwrap();
        let parent_text = format!(
            "{}\nlook at [Image #1] {}",
            image_files_envelope(&[&job, &extra]),
            job.display()
        );
        let parent = vec![ConversationItem::user(parent_text)];
        match &parent[0] {
            ConversationItem::User(u) => {
                assert!(
                    !u.content
                        .iter()
                        .any(|p| matches!(p, ContentPart::Image { .. })),
                    "parent user item must be text-only (no ContentPart::Image)"
                );
            }
            _ => panic!("expected user item"),
        }
        let catalog = collect_named_saved_images(&parent);
        let spawn = format!("Inspect [Image #1] {} for this job only.", job.display());
        let urls = file_urls_for_named_spawn_prompt(&spawn, &catalog);
        assert_eq!(
            urls,
            vec![format!("file://{}", job.display())],
            "nested file_urls_for_named_spawn_prompt must return file:// for [Image #1] from text-only parent; unnamed extra must not attach"
        );
        assert!(
            !urls.iter().any(|u| u.contains("unrelated.png")),
            "unnamed extra path must not attach: {urls:?}"
        );
    }
}
