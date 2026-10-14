//! `x.ai/interject` extension handler.
//!
//! Queues a mid-turn interjection into the active session's pending interjection buffer.
//! The session actor drains it at the next safe point in `process_conversation_turn`.

use agent_client_protocol as acp;
use xai_grok_tools::implementations::grok_build::task::backend::{ChannelBackend, SubagentBackend};
use xai_grok_tools::implementations::grok_build::task::types::SubagentFollowUpOutcome;

use super::{ExtResult, parse_params};
use crate::agent::MvpAgent;
use crate::session::SessionCommand;

pub const INTERJECT_METHOD: &str = "x.ai/interject";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterjectRequest {
    pub session_id: String,
    pub text: String,
    #[serde(default)]
    pub interjection_id: Option<String>,
    /// Optional structured blocks (text and images) from image-capable clients.
    /// Absent means the legacy text-only wire shape (empty after default).
    #[serde(default)]
    pub content: Vec<acp::ContentBlock>,
}

fn queued_status() -> ExtResult {
    super::to_ext_response(Ok(serde_json::json!({
        "status": "queued",
    })))
}

fn interjection_not_sent(detail: &str, session_id: &str) -> acp::Error {
    acp::Error::invalid_params().data(format!("interjection not sent ({detail}): {session_id}"))
}

/// Handle `x.ai/interject`: queue a mid-turn user interjection.
///
/// A resident session, including one still loading after reconnect, gets one
/// `SessionCommand::Interject`. A running nested session that is not a
/// resident handle is delivered through follow-up. That path already enqueues
/// one Interject. Do not send a second one. A real missing session fails with
/// "session is gone".
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: InterjectRequest = parse_params(args)?;
    let sid: acp::SessionId = req.session_id.clone().into();
    let (text_override, images) = super::content::split_content(req.content);
    let text = text_override.unwrap_or(req.text);
    if let Some(session) = agent.session_handle_waiting_for_load(&sid).await {
        let _ = session.cmd_tx.send(SessionCommand::Interject {
            text,
            id: req.interjection_id,
            images,
        });
        return queued_status();
    }

    match ChannelBackend::from_coordinator(agent.subagent_event_tx.clone())
        .follow_up(&req.session_id, &text)
        .await
    {
        SubagentFollowUpOutcome::Queued { .. } => queued_status(),
        SubagentFollowUpOutcome::NotFound => {
            Err(acp::Error::invalid_params().data(format!("session is gone: {}", req.session_id)))
        }
        SubagentFollowUpOutcome::Disabled => {
            Err(interjection_not_sent("follow-up is off", &req.session_id))
        }
        SubagentFollowUpOutcome::NotRunning => Err(interjection_not_sent(
            "session is not running",
            &req.session_id,
        )),
        SubagentFollowUpOutcome::LiveL3Unbothered => Err(interjection_not_sent(
            "live specialist was not targeted",
            &req.session_id,
        )),
        SubagentFollowUpOutcome::NotThisParentsL2 => Err(interjection_not_sent(
            "not this nested session",
            &req.session_id,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Legacy wire shape (no `content`) parses byte-identically: text-only, zero images, no text override.
    #[test]
    fn parse_without_content_is_legacy_text_only() {
        let req: InterjectRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "s1",
            "text": "steer left",
            "interjectionId": "i1",
        }))
        .expect("legacy params must parse");
        assert_eq!(req.text, "steer left");
        assert_eq!(req.interjection_id.as_deref(), Some("i1"));
        let (text_override, images) = super::super::content::split_content(req.content);
        assert_eq!(text_override, None);
        assert!(images.is_empty());
    }

    /// `content` with text and image blocks parses; the images are extracted.
    /// The Text block (the client's rewritten, path-stripped text) overrides the raw `text` param.
    #[test]
    fn parse_with_content_extracts_images_and_prefers_block_text() {
        let req: InterjectRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "s1",
            "text": "look at [Image #1: /tmp/x.png]",
            "content": [
                { "type": "text", "text": "look at [Image #1]" },
                { "type": "image", "data": "aGVsbG8=", "mimeType": "image/png" },
            ],
        }))
        .expect("content params must parse");
        let (text_override, images) = super::super::content::split_content(req.content);
        assert_eq!(
            text_override.as_deref(),
            Some("look at [Image #1]"),
            "rewritten block text must win over the raw text param"
        );
        assert_eq!(images.len(), 1);
        let Some(image) = images.first() else {
            panic!("expected one image: {images:?}");
        };
        assert_eq!(image.mime_type, "image/png");
        assert_eq!(image.data, "aGVsbG8=");
    }

    /// Garbage `content` fails the whole parse (strict, like other params) instead of silently dropping attachments.
    #[test]
    fn parse_with_garbage_content_is_an_error() {
        let result: Result<InterjectRequest, _> = serde_json::from_value(serde_json::json!({
            "sessionId": "s1",
            "text": "steer",
            "content": "not an array",
        }));
        assert!(result.is_err(), "garbage content must be rejected");
    }
}
