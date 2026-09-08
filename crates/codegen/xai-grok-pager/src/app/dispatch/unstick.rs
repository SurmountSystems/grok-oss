//! `/unstick`: resend the last L1 prompt as if the network dropped it.
//!
//! Not `/resume` (session picker). Does not cancel nested agents, rewind,
//! compact, or reset sampler usage meters.

use std::path::Path;

use crate::app::actions::Effect;
use crate::app::agent_view::AgentView;
use crate::app::app_view::{ActiveView, AppView};
use crate::scrollback::block::RenderBlock;
use agent_client_protocol as acp;
use xai_grok_shell::session::prompt_wal::{self, PromptWalImage, PromptWalKind};

/// Last L1 operator text plus WAL image file ids (never data URLs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LastL1Prompt {
    pub text: String,
    pub images: Vec<PromptWalImage>,
}

/// Short toast when there is no last parent prompt. Do not invent text.
pub(crate) const NO_LAST_PROMPT_TOAST: &str = "No last parent prompt to resend.";

/// Keep `[Image #N]` tokens. Never re-inline `data:` URLs.
pub(crate) fn keep_image_tokens_not_data_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix("data:") {
            let skip = stripped
                .find(|c: char| c.is_whitespace())
                .unwrap_or(stripped.len());
            rest = &stripped[skip..];
            continue;
        }
        let ch = rest.chars().next().expect("rest non-empty");
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

fn wal_kind_is_parent_send(kind: PromptWalKind) -> bool {
    matches!(kind, PromptWalKind::Send | PromptWalKind::Interject)
}

/// Last L1 operator text from `prompt_wal.jsonl`, if that file exists.
pub(crate) fn last_l1_prompt_from_wal(cwd: &str, session_id: &str) -> Option<LastL1Prompt> {
    let rows = prompt_wal::load_prompt_wal(cwd, session_id).ok()?;
    for rec in rows.iter().rev() {
        if !wal_kind_is_parent_send(rec.kind) {
            continue;
        }
        let kept = keep_image_tokens_not_data_urls(&rec.text);
        let trimmed = kept.trim();
        if !trimmed.is_empty() {
            let images = rec
                .images
                .iter()
                .filter(|img| prompt_wal::image_file_id_is_safe(&img.file_id))
                .cloned()
                .collect();
            return Some(LastL1Prompt {
                text: trimmed.to_string(),
                images,
            });
        }
    }
    None
}

fn last_parent_user_prompt(agent: &AgentView) -> Option<LastL1Prompt> {
    let len = agent.scrollback.len();
    for idx in (0..len).rev() {
        let Some(entry) = agent.scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::UserPrompt(block) = &entry.block {
            let kept = keep_image_tokens_not_data_urls(&block.text);
            let trimmed = kept.trim();
            if !trimmed.is_empty() {
                return Some(LastL1Prompt {
                    text: trimmed.to_string(),
                    images: Vec::new(),
                });
            }
        }
    }
    None
}

/// Prefer WAL L1 text when the file exists; else last parent Human send.
/// Never reads nested overlay scrollback.
pub(crate) fn last_l1_prompt(agent: &AgentView) -> Option<LastL1Prompt> {
    let sid = agent.session.session_id.as_ref().map(|s| s.0.to_string());
    let cwd = agent.session.cwd.to_string_lossy();
    if let Some(sid) = sid.as_deref()
        && let Some(from_wal) = last_l1_prompt_from_wal(&cwd, sid)
    {
        return Some(from_wal);
    }
    last_parent_user_prompt(agent)
}

fn mime_for_wal_image(file_id: &str) -> Option<&'static str> {
    let ext = Path::new(file_id)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())?;
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

/// WAL `images/` file ids as ACP resource links. Never inline data URLs or blobs.
pub(crate) fn wal_image_resource_blocks(
    images_dir: &Path,
    images: &[PromptWalImage],
) -> Vec<acp::ContentBlock> {
    images
        .iter()
        .filter_map(|img| {
            if !prompt_wal::image_file_id_is_safe(&img.file_id) {
                return None;
            }
            let path = images_dir.join(&img.file_id);
            if !path.is_file() {
                return None;
            }
            let uri = format!("file://{}", path.display());
            if uri.to_ascii_lowercase().contains("data:") {
                return None;
            }
            let mut link = acp::ResourceLink::new(format!("[Image #{}]", img.n), uri);
            if let Some(mime) = mime_for_wal_image(&img.file_id) {
                link = link.mime_type(mime.to_string());
            }
            let mut meta = acp::Meta::new();
            meta.insert(
                "xai.dev/imageDisplayNumber".into(),
                serde_json::json!(img.n),
            );
            link = link.meta(meta);
            Some(acp::ContentBlock::ResourceLink(link))
        })
        .collect()
}

/// Resend the last L1 prompt without a second Human line and without unwind.
pub(super) fn dispatch_unstick_last_l1_prompt(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        app.show_toast(NO_LAST_PROMPT_TOAST);
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        app.show_toast(NO_LAST_PROMPT_TOAST);
        return vec![];
    };
    let Some(last) = last_l1_prompt(agent) else {
        agent.show_toast(NO_LAST_PROMPT_TOAST);
        return vec![];
    };
    let Some(session_id) = agent.session.session_id.clone() else {
        agent.show_toast(NO_LAST_PROMPT_TOAST);
        return vec![];
    };

    let already_has_human_line = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::UserPrompt(_))
        )
    });
    if !already_has_human_line {
        agent
            .scrollback
            .push_block(RenderBlock::user_prompt(last.text.clone()));
    }

    // New prompt id so an orphaned hung session/prompt RPC cannot end this retry.
    let prompt_id = uuid::Uuid::new_v4().to_string();
    agent.session.current_prompt_id = Some(prompt_id.clone());
    if agent.session.state.is_idle() {
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
    }

    let images_dir =
        crate::prompt_images::session_images_dir(Some(&session_id), &agent.session.cwd);

    vec![Effect::UnstickResendPrompt {
        agent_id: id,
        session_id,
        text: last.text,
        prompt_id,
        images: last.images,
        images_dir,
    }]
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn keep_image_tokens_drops_data_urls() {
        let raw = "see [Image #1] data:image/png;base64,AAAA more";
        let kept = keep_image_tokens_not_data_urls(raw);
        assert!(kept.contains("[Image #1]"));
        assert!(!kept.contains("data:image"));
        assert!(!kept.contains("AAAA"));
    }

    #[test]
    fn wal_parent_send_kinds() {
        assert!(wal_kind_is_parent_send(PromptWalKind::Send));
        assert!(wal_kind_is_parent_send(PromptWalKind::Interject));
        assert!(!wal_kind_is_parent_send(PromptWalKind::PlanNotes));
        assert!(!wal_kind_is_parent_send(PromptWalKind::Queue));
    }

    #[test]
    fn wal_image_resource_blocks_use_file_uri_not_data_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("img-1.png");
        std::fs::write(&path, b"png-bytes").unwrap();
        let blocks = wal_image_resource_blocks(
            dir.path(),
            &[PromptWalImage {
                n: 1,
                file_id: "img-1.png".into(),
            }],
        );
        assert_eq!(blocks.len(), 1, "expected one resource link: {blocks:?}");
        let acp::ContentBlock::ResourceLink(link) = &blocks[0] else {
            panic!("WAL image must be a resource link, got {blocks:?}");
        };
        assert!(
            link.uri.starts_with("file://") && link.uri.contains("img-1.png"),
            "resource uri must be file:// to the WAL file id, got {}",
            link.uri
        );
        assert!(
            !link.uri.to_ascii_lowercase().contains("data:"),
            "must not inline a data URL: {}",
            link.uri
        );
        assert_eq!(link.mime_type.as_deref(), Some("image/png"));
    }

    #[test]
    fn wal_image_resource_blocks_drop_data_url_file_ids() {
        let dir = tempfile::tempdir().unwrap();
        let blocks = wal_image_resource_blocks(
            dir.path(),
            &[PromptWalImage {
                n: 1,
                file_id: "data:image/png;base64,AAAA".into(),
            }],
        );
        assert!(
            blocks.is_empty(),
            "data URL file ids must not become resource blocks: {blocks:?}"
        );
    }
}
