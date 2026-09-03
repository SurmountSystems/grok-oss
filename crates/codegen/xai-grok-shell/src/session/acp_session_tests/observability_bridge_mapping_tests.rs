use super::*;
use crate::session::events::ToolOutcome;
use xai_tool_protocol::session_event::ToolCallOutcome;
use xai_tool_protocol::turn_hook::TurnHookOutcome;
#[test]
fn map_tool_outcome_success() {
    assert_eq!(
        map_tool_outcome(ToolOutcome::Success),
        ToolCallOutcome::Success
    );
}
#[test]
fn map_tool_outcome_errors() {
    assert_eq!(map_tool_outcome(ToolOutcome::Error), ToolCallOutcome::Error);
    assert_eq!(
        map_tool_outcome(ToolOutcome::InvalidTool),
        ToolCallOutcome::Error
    );
}
#[test]
fn map_tool_outcome_cancellations() {
    for variant in [
        ToolOutcome::PermissionRejected,
        ToolOutcome::PermissionCancelled,
        ToolOutcome::Followup,
        ToolOutcome::HookDenied,
        ToolOutcome::Cancelled,
    ] {
        assert_eq!(
            map_tool_outcome(variant),
            ToolCallOutcome::Cancelled,
            "expected Cancelled for {variant:?}",
        );
    }
}
#[test]
fn turn_result_completed() {
    let result: Result<TurnOutcome, acp::Error> = Ok(TurnOutcome::Completed {
        snapshot: Box::new(None),
        tools_called: vec![],
        structured_output: None,
        refusal: None,
    });
    assert_eq!(
        turn_result_to_hook_outcome(&result),
        TurnHookOutcome::Completed
    );
}
#[test]
fn turn_result_cancelled() {
    let result: Result<TurnOutcome, acp::Error> = Ok(TurnOutcome::Cancelled {
        category: None,
        context: None,
    });
    assert_eq!(
        turn_result_to_hook_outcome(&result),
        TurnHookOutcome::Cancelled
    );
}
#[test]
fn turn_result_stationarity_ended_is_completed() {
    let result: Result<TurnOutcome, acp::Error> = Ok(TurnOutcome::StationarityEnded {
        snapshot: Box::new(None),
    });
    assert_eq!(
        turn_result_to_hook_outcome(&result),
        TurnHookOutcome::Completed
    );
}
#[test]
fn turn_result_error() {
    let result: Result<TurnOutcome, acp::Error> = Err(acp::Error::internal_error());
    assert_eq!(turn_result_to_hook_outcome(&result), TurnHookOutcome::Error);
}
#[test]
fn is_remote_image_url_classifies_schemes() {
    assert!(is_remote_image_url("https://example.com/x.png"));
    assert!(is_remote_image_url("http://example.com/x.png"));
    assert!(!is_remote_image_url("file:///Users/me/x.png"));
    assert!(!is_remote_image_url("data:image/png;base64,AAAA"));
    assert!(!is_remote_image_url(""));
    assert!(!is_remote_image_url("FILE:///Users/me/x.png"));
}
#[test]
fn pick_image_url_persists_file_uri_not_the_data_url_crate() {
    let img = agent_client_protocol::ImageContent::new("AAAA", "image/png").uri(Some(
        "file:///Users/me/.grok/sessions/s/images/image-1.png".into(),
    ));
    assert_eq!(
        pick_user_image_url(&img),
        "file:///Users/me/.grok/sessions/s/images/image-1.png"
    );
}
#[test]
fn pick_image_url_persists_https_uri_not_inline_bytes() {
    let img = agent_client_protocol::ImageContent::new("BBBB", "image/jpeg")
        .uri(Some("https://example.com/x.jpg".into()));
    assert_eq!(pick_user_image_url(&img), "https://example.com/x.jpg");
}
#[test]
fn pick_image_url_falls_back_to_https_uri_when_data_empty() {
    let img = agent_client_protocol::ImageContent::new(String::new(), "image/png")
        .uri(Some("https://example.com/x.png".into()));
    assert_eq!(pick_user_image_url(&img), "https://example.com/x.png");
}
#[test]
fn pick_image_url_keeps_file_uri_when_data_empty() {
    let img = agent_client_protocol::ImageContent::new(String::new(), "image/png")
        .uri(Some("file:///Users/me/missing.png".into()));
    assert_eq!(pick_user_image_url(&img), "file:///Users/me/missing.png");
}
#[test]
fn persist_inline_data_url_writes_session_file_and_drops_crate() {
    let dir = std::env::temp_dir().join(format!("grok-persist-img-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let crate_url = format!("data:image/png;base64,{}", "A".repeat(200_000));
    let handle = persist_inline_data_url(crate_url.clone(), Some(&dir));
    assert!(
        handle.starts_with("file://"),
        "must persist a file handle, got {handle}"
    );
    assert!(
        !handle.contains("base64"),
        "must not keep the data URL crate"
    );
    let json = serde_json::to_string(&handle).unwrap();
    assert!(
        json.len() < 1_000,
        "handle JSON must not copy the crate, got {}",
        json.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
