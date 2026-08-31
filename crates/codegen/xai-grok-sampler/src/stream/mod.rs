//! Layer-2 stream transforms: turn raw HTTP chunk streams into
//! [`SamplingEvent`](crate::events::SamplingEvent) streams.
//!
//! Each backend has its own transform because the raw chunk types
//! differ; backend dispatch happens in M4's
//! [`actor::request_task`](crate::actor::request_task), which knows
//! the API backend from `SamplerConfig.api_backend` and calls the
//! matching `SamplingClient::conversation_stream*` method before
//! handing the result to the corresponding transform here.

pub mod chat_completions;
pub mod collect;
pub mod messages;
pub mod responses;

pub use chat_completions::stream_chat_completions;
pub use collect::collect_response;
pub use messages::stream_messages;
pub use responses::stream_responses;

use std::time::{Duration, Instant};

use futures_util::Stream;
use futures_util::StreamExt;
use xai_grok_sampling_types::SamplingError;

/// Until the model makes progress, wait at most the headers / first-token
/// budget (default 120s), never the full post-token idle (default 300s).
pub(crate) fn first_token_wait(idle_timeout: Duration) -> Duration {
    idle_timeout.min(crate::client::stream_headers_timeout())
}

pub(crate) fn first_token_timeout_error(budget: Duration) -> SamplingError {
    SamplingError::EventStreamError(format!(
        "timed out waiting for the first token after {}",
        xai_tty_utils::format_human_duration(budget)
    ))
}

pub(crate) enum ChunkWait<T> {
    Item(T),
    Ended,
    FirstTokenTimeout,
    IdleTimeout,
}

/// First wait uses the first-token budget; later waits use idle timeout.
pub(crate) async fn next_or_timeout<S, T>(
    stream: &mut S,
    saw_progress: bool,
    first_token_deadline: Instant,
    idle_timeout: Duration,
) -> ChunkWait<T>
where
    S: Stream<Item = T> + Unpin,
{
    let wait = if saw_progress {
        idle_timeout
    } else {
        let left = first_token_deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return ChunkWait::FirstTokenTimeout;
        }
        left
    };
    match tokio::time::timeout(wait, stream.next()).await {
        Ok(Some(item)) => ChunkWait::Item(item),
        Ok(None) => ChunkWait::Ended,
        Err(_) if saw_progress => ChunkWait::IdleTimeout,
        Err(_) => ChunkWait::FirstTokenTimeout,
    }
}
