//! Nested New/Prompt image attach: ACP blocks with `file://` URIs for
//! session files named in the spawn prompt. The prompt string never
//! contains `data:image`.

use super::*;
use base64::Engine as _;
use xai_grok_sampling_types::conversation::ContentPart;
use xai_grok_subagent_resolution::{
    collect_named_saved_images, file_urls_for_named_spawn_prompt, saved_local_image_path,
};

/// Drop leftover parent `ContentPart::Image` from a verbatim fork copy.
/// Nested first user turns attach named `file://` parts from the spawn
/// prompt instead. Spawn text is not rewritten here.
pub(super) fn drop_parent_image_parts_for_spawn_fork(items: &mut [ConversationItem]) {
    for item in items {
        match item {
            ConversationItem::User(u) => {
                u.content
                    .retain(|p| !matches!(p, ContentPart::Image { .. }));
            }
            ConversationItem::ToolResult(tr) => {
                tr.images
                    .retain(|p| !matches!(p, ContentPart::Image { .. }));
            }
            _ => {}
        }
    }
}

/// Nested Prompt blocks: task text plus ACP image blocks with `file://`
/// URIs for session files named in that text.
pub(super) fn nested_spawn_prompt_blocks(
    spawn_prompt: &str,
    parent_items: &[ConversationItem],
) -> Vec<acp::ContentBlock> {
    let catalog = collect_named_saved_images(parent_items);
    let urls = file_urls_for_named_spawn_prompt(spawn_prompt, &catalog);
    let mut blocks = Vec::with_capacity(1 + urls.len());
    blocks.push(acp::ContentBlock::Text(acp::TextContent::new(
        spawn_prompt.to_string(),
    )));
    for url in urls {
        blocks.push(acp_image_block_for_named_file_url(&url));
    }
    blocks
}

fn acp_image_block_for_named_file_url(url: &str) -> acp::ContentBlock {
    let path = saved_local_image_path(url);
    let mime = path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(mime_for_image_extension)
        .unwrap_or("image/png");
    // ACP ingest decodes `data`; empty data is dropped. Bytes stay off the
    // prompt string. `uri` is the conversation handle `add_image` uses.
    let data = path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
        .unwrap_or_default();
    acp::ContentBlock::Image(
        acp::ImageContent::new(data, mime.to_string()).uri(Some(url.to_string())),
    )
}

fn mime_for_image_extension(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
}

/// Nested first user turn from those Prompt blocks: text plus
/// `ContentPart::Image` `file://` handles, matching `add_image` when
/// `user_images` is nonempty.
pub(super) fn nested_first_user_turn_from_prompt_blocks(
    prompt_blocks: &[acp::ContentBlock],
) -> ConversationItem {
    let mut text = String::new();
    let mut image_parts = Vec::new();
    for block in prompt_blocks {
        match block {
            acp::ContentBlock::Text(t) => text.push_str(&t.text),
            acp::ContentBlock::Image(img) => {
                if let Some(uri) = img.uri.as_deref().filter(|u| !u.is_empty()) {
                    image_parts.push(ContentPart::Image { url: uri.into() });
                }
            }
            _ => {}
        }
    }
    let mut parts = vec![ContentPart::Text { text: text.into() }];
    parts.extend(image_parts);
    ConversationItem::user_with_parts(parts)
}

fn image_urls_from_user(item: &ConversationItem) -> Vec<String> {
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

fn user_text(item: &ConversationItem) -> String {
    match item {
        ConversationItem::User(u) => u
            .content
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn user_with_saved_image(text: &str, path: &std::path::Path) -> ConversationItem {
    ConversationItem::user_with_parts(vec![
        ContentPart::Text { text: text.into() },
        ContentPart::Image {
            url: format!("file://{}", path.display()).into(),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nested first user turn has ContentPart::Image file:// for the named
    /// file and not for an unnamed extra.
    #[test]
    fn nested_first_user_turn_has_content_part_image_file_for_named_file_not_unnamed_extra() {
        let dir = tempfile::tempdir().unwrap();
        let images = dir.path().join("images");
        std::fs::create_dir_all(&images).unwrap();
        let job = images.join("job.png");
        let extra = images.join("unrelated.png");
        std::fs::write(&job, b"job-bytes").unwrap();
        std::fs::write(&extra, b"extra-bytes").unwrap();
        let parent = vec![
            user_with_saved_image("job shot", &job),
            user_with_saved_image("unrelated shot", &extra),
        ];
        let spawn = format!("Inspect [Image #1] {} for this job only.", job.display());
        let blocks = nested_spawn_prompt_blocks(&spawn, &parent);
        assert!(
            !spawn.contains("data:image"),
            "spawn prompt string must not contain data:image"
        );
        let text_blocks: Vec<&str> = blocks
            .iter()
            .filter_map(|b| match b {
                acp::ContentBlock::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text_blocks.join(""), spawn);
        assert!(!text_blocks.join("").contains("data:image"));
        let turn = nested_first_user_turn_from_prompt_blocks(&blocks);
        let urls = image_urls_from_user(&turn);
        assert_eq!(
            urls.len(),
            1,
            "nested first user turn has ContentPart::Image file:// for the named file and not for an unnamed extra"
        );
        assert_eq!(urls[0], format!("file://{}", job.display()));
        assert!(urls[0].starts_with("file://"));
        assert!(
            !urls.iter().any(|u| u.contains("unrelated.png")),
            "unnamed extra must not attach: {urls:?}"
        );
        let text = user_text(&turn);
        assert!(!text.contains("data:image"));
        assert!(text.contains("[Image #1]"));
        assert!(text.contains(&job.display().to_string()));
    }

    /// Nested first user turn has ContentPart::Image file:// for the named
    /// file and not for an unnamed extra. Summarized fork bootstrap uses
    /// `normalize_forked_context_for_job` with the spawn prompt.
    #[test]
    fn nested_fork_first_user_turn_has_content_part_image_file_for_named_file_not_unnamed_extra() {
        let dir = tempfile::tempdir().unwrap();
        let images = dir.path().join("images");
        std::fs::create_dir_all(&images).unwrap();
        let job = images.join("job.png");
        let extra = images.join("unrelated.png");
        std::fs::write(&job, b"job-bytes").unwrap();
        std::fs::write(&extra, b"extra-bytes").unwrap();
        let items = vec![
            ConversationItem::system("parent system"),
            user_with_saved_image("job shot", &job),
            ConversationItem::assistant("ok 1"),
            user_with_saved_image("unrelated shot", &extra),
            ConversationItem::assistant("ok 2"),
        ];
        let spawn = format!("Use [Image #1] {} for this job only.", job.display());
        let ctx = super::super::verbatim_or_normalize_fork(items, 1, Some(&spawn));
        assert!(!ctx.verbatim_fork);
        assert_eq!(ctx.conversation.len(), 2);
        let urls = image_urls_from_user(&ctx.conversation[1]);
        assert_eq!(
            urls.len(),
            1,
            "nested first user turn has ContentPart::Image file:// for the named file and not for an unnamed extra"
        );
        assert_eq!(urls[0], format!("file://{}", job.display()));
        assert!(
            !urls.iter().any(|u| u.contains("unrelated.png")),
            "unnamed extra must not attach: {urls:?}"
        );
        let text = user_text(&ctx.conversation[1]);
        assert!(!text.contains("data:image"));
    }

    /// Same envelope grammar as `render_image_files_block`.
    fn image_files_envelope(paths: &[&std::path::Path]) -> String {
        let mut out = String::from(
            "<image_files>\nThe following images were provided by the user and saved to the workspace for future use:\n",
        );
        for (i, p) in paths.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, p.display()));
        }
        out.push_str("\nThese images can be copied for use in other locations.\n</image_files>");
        out
    }

    /// Parent user item is text-only (`[Image #1]` plus an `<image_files>`
    /// path, no `ContentPart::Image`). Nested prompt blocks must return
    /// `file://` for `[Image #1]`. An unnamed extra path must not attach.
    #[test]
    fn nested_prompt_blocks_attach_named_file_from_text_only_parent() {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        let job = assets.join("job.png");
        let extra = assets.join("unrelated.png");
        std::fs::write(&job, b"job-bytes").unwrap();
        std::fs::write(&extra, b"extra-bytes").unwrap();
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
        let spawn = format!("Inspect [Image #1] {} for this job only.", job.display());
        let blocks = nested_spawn_prompt_blocks(&spawn, &parent);
        let turn = nested_first_user_turn_from_prompt_blocks(&blocks);
        let urls = image_urls_from_user(&turn);
        assert_eq!(
            urls,
            vec![format!("file://{}", job.display())],
            "nested prompt blocks must return file:// for [Image #1] from text-only parent; unnamed extra must not attach"
        );
        assert!(
            !urls.iter().any(|u| u.contains("unrelated.png")),
            "unnamed extra path must not attach: {urls:?}"
        );
    }
}
