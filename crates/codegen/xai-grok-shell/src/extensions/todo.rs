//! `x.ai/todo/clear_completed` — operator clear of finished board rows.
//!
//! Archives completed + cancelled items on the live `TodoState`, persists
//! Resources + plan.json, and re-emits ACP `Plan` so the todo pane stays in
//! sync. Not the same as pane `h` (view-only hide) or agent `merge: false`.

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use super::{ExtResult, parse_params, to_raw_response};
use crate::agent::MvpAgent;
use crate::session::SessionCommand;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearCompletedRequest {
    session_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearCompletedResponse {
    /// Number of completed/cancelled items archived off the active board.
    cleared: usize,
}

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "x.ai/todo/clear_completed" => handle_clear_completed(agent, args).await,
        _ => Err(acp::Error::method_not_found()),
    }
}

async fn handle_clear_completed(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: ClearCompletedRequest = parse_params(args)?;
    let not_found_err = format!("session not found: {}", req.session_id);
    let session_handle = {
        let sessions = agent.sessions.borrow();
        sessions.get(&req.session_id.into()).cloned()
    };
    let Some(session) = session_handle else {
        return Err(acp::Error::invalid_params().data(not_found_err));
    };
    let (tx, rx) = oneshot::channel();
    let _ = session
        .cmd_tx
        .send(SessionCommand::ClearCompletedTodos { respond_to: tx });
    let cleared = rx
        .await
        .map_err(|_| acp::Error::internal_error().data("session failed to respond"))?;
    to_raw_response(&ClearCompletedResponse { cleared })
}
