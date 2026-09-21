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

use crate::events::SamplingChannel;
use futures_util::Stream;
use futures_util::StreamExt;
use xai_grok_sampling_types::SamplingError;

/// Until the model makes progress, wait at most the headers / first-token
/// budget (default 120s), never the full post-token idle (default 300s).
///
/// Retry attempt 2 must use this same cap. Scaffolding SSE (reasoning
/// item added, keepalives) must not stretch the wait toward 11 minutes.
pub fn first_token_wait(idle_timeout: Duration) -> Duration {
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

#[cfg(test)]
mod first_token_wait_tests {
    use super::*;
    use std::time::Duration;

    /// Operator: Isolated Preview retry attempt 2 sat on
    /// `Retrying the model request (attempt 2): waiting for first token`
    /// for 11m27s. The first-token budget is the headers wait (120s),
    /// not the 300s idle and not eleven minutes.
    #[test]
    fn first_token_wait_is_headers_budget_not_eleven_minutes() {
        let chrome = "Retrying the model request (attempt 2): waiting for first token";
        assert!(
            chrome.contains("Retrying the model request"),
            "chrome must name the retry, got {chrome}"
        );
        assert!(
            chrome.contains("waiting for first token"),
            "chrome must name the first-token wait, got {chrome}"
        );
        let idle = Duration::from_secs(300);
        let budget = first_token_wait(idle);
        let headers = crate::client::stream_headers_timeout();
        assert_eq!(
            budget,
            idle.min(headers),
            "first-token wait is min(idle, headers budget), got {budget:?}"
        );
        assert!(
            budget < Duration::from_secs(11 * 60),
            "must not hang 11 minutes waiting for the first token, got {budget:?}"
        );
        assert!(
            budget <= idle,
            "must not wait longer than post-token idle before the first token, got {budget:?}"
        );
    }
}

// Client-side breaker for obvious looping sentences in a streaming
// assistant or thought buffer.
//
// Complements the server `x-grok-doom-loop-check` path, which only
// resamples confident *thinking* loops on the Responses API and treats
// visible-output loops as the operator's to judge. Chat Completions
// never reports those triggers. When the same short sentence (or a
// two-sentence cycle) repeats in the stream, this abort is Fatal: stop
// the turn instead of resampling and then accepting the loop as-is.

/// Tail inspected on each push. Long enough for three copies of a short
/// two-sentence cycle (the dest-encoder-skip loop is about 70 characters).
const WINDOW_CHARS: usize = 768;
/// Shortest phrase that can count as a loop unit.
const MIN_PHRASE_CHARS: usize = 24;
/// Consecutive copies of a short sentence that mean a loop.
const MIN_SHORT_REPEATS: usize = 4;
/// Consecutive copies of a longer phrase (a two-sentence cycle).
const MIN_LONG_REPEATS: usize = 3;
/// Phrases this long use [`MIN_LONG_REPEATS`].
const LONG_PHRASE_CHARS: usize = 48;

/// Per-stream accumulator. Text and thought are scored separately so a
/// repeated heading in the answer cannot be blamed on a thought dump.
#[derive(Debug, Default)]
pub(crate) struct StreamRepetitionGuard {
    text: String,
    reasoning: String,
}

impl StreamRepetitionGuard {
    pub(crate) fn append(&mut self, channel: SamplingChannel, delta: &str) {
        let buf = self.buf_mut(channel);
        buf.push_str(delta);
        if buf.len() > WINDOW_CHARS * 2 {
            let mut start = buf.len() - WINDOW_CHARS;
            while start > 0 && !buf.is_char_boundary(start) {
                start -= 1;
            }
            buf.replace_range(..start, "");
        }
    }

    pub(crate) fn is_looping(&self, channel: SamplingChannel) -> bool {
        is_repetitive(self.buf(channel))
    }

    pub(crate) fn error(&self, channel: SamplingChannel, chunk_index: u64) -> SamplingError {
        SamplingError::RepetitiveGeneration {
            channel: channel.as_str().to_string(),
            aborted_at_chunk: Some(chunk_index),
        }
    }

    fn buf(&self, channel: SamplingChannel) -> &str {
        match channel {
            SamplingChannel::Text => &self.text,
            SamplingChannel::Reasoning => &self.reasoning,
        }
    }

    fn buf_mut(&mut self, channel: SamplingChannel) -> &mut String {
        match channel {
            SamplingChannel::Text => &mut self.text,
            SamplingChannel::Reasoning => &mut self.reasoning,
        }
    }
}

/// True when `text` ends in an obvious looping sentence or two-sentence cycle.
pub(crate) fn is_repetitive(text: &str) -> bool {
    let tail = tail_window(text);
    if tail.len() < MIN_PHRASE_CHARS * MIN_LONG_REPEATS {
        return false;
    }
    trailing_units_loop(split_sentences(tail)) || trailing_units_loop(split_nonempty_lines(tail))
}

fn tail_window(s: &str) -> &str {
    if s.len() <= WINDOW_CHARS {
        return s;
    }
    let mut start = s.len() - WINDOW_CHARS;
    while start > 0 && !s.is_char_boundary(start) {
        start -= 1;
    }
    &s[start..]
}

fn split_sentences(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if matches!(bytes[i], b'.' | b'?' | b'!') {
            let next = i + 1;
            if next == bytes.len() || bytes[next].is_ascii_whitespace() {
                let mut end = next;
                while end < bytes.len() && bytes[end].is_ascii_whitespace() {
                    end += 1;
                }
                let piece = s[start..end].trim();
                if !piece.is_empty() {
                    out.push(piece);
                }
                start = end;
                i = end;
                continue;
            }
        }
        i += 1;
        while i < bytes.len() && !s.is_char_boundary(i) {
            i += 1;
        }
    }
    let rest = s[start..].trim();
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

fn split_nonempty_lines(s: &str) -> Vec<&str> {
    s.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

fn trailing_units_loop(units: Vec<&str>) -> bool {
    if units.is_empty() {
        return false;
    }
    let last = units[units.len() - 1];
    if is_loop_phrase(last) {
        let mut k = 1usize;
        for unit in units.iter().rev().skip(1) {
            if *unit == last {
                k += 1;
            } else {
                break;
            }
        }
        let min_k = if last.len() >= LONG_PHRASE_CHARS {
            MIN_LONG_REPEATS
        } else {
            MIN_SHORT_REPEATS
        };
        if k >= min_k {
            return true;
        }
    }
    if units.len() < MIN_LONG_REPEATS * 2 {
        return false;
    }
    let a = units[units.len() - 2];
    let b = units[units.len() - 1];
    if a == b {
        return false;
    }
    if !is_loop_phrase(a) || b.len() < MIN_PHRASE_CHARS / 2 {
        return false;
    }
    if a.len() + b.len() < LONG_PHRASE_CHARS {
        return false;
    }
    let mut cycles = 1usize;
    let mut i = units.len() - 2;
    while i >= 2 {
        if units[i - 2] == a && units[i - 1] == b {
            cycles += 1;
            i -= 2;
        } else {
            break;
        }
    }
    cycles >= MIN_LONG_REPEATS
}

fn is_loop_phrase(unit: &str) -> bool {
    unit.len() >= MIN_PHRASE_CHARS && unit.contains(char::is_whitespace)
}

/// Operator screenshot 2026-09-20 Isolated Preview: the stream looped this
/// pair until cancel at 19m18s. GitHub #133. Named tests must reference this
/// const so quality `-D warnings` does not fail on unused.
#[cfg(test)]
pub(crate) const DEST_ENCODER_SKIP_LOOP: &str =
    "Spawn dests of dest encoder skip. I'll spawn dests of dest encoder skip.";

#[cfg(test)]
mod dest_encoder_skip_repetition_tests {
    use super::*;
    use xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE;

    /// Surmount fork of SpaceXAI stream handling. Upstream server
    /// `x-grok-doom-loop-check` resamples confident *thinking* loops on
    /// Responses and leaves visible-output loops for the operator to
    /// cancel. Isolated Preview Chat Completions never reports those
    /// triggers, so this client `StreamRepetitionGuard` is Fatal
    /// (`SamplingError::RepetitiveGeneration`, not retried). Do not add a
    /// second breaker. Upstream server resample stays
    /// `[doom_loop_recovery]`.
    #[test]
    fn dest_encoder_skip_loop_is_repetitive() {
        assert_eq!(
            DEST_ENCODER_SKIP_LOOP,
            "Spawn dests of dest encoder skip. I'll spawn dests of dest encoder skip."
        );
        let one = format!("{DEST_ENCODER_SKIP_LOOP} ");
        assert!(
            !is_repetitive(&one.repeat(2)),
            "two copies of the dest-encoder-skip pair must not stop ordinary restatement"
        );
        let looping = one.repeat(3);
        assert!(
            is_repetitive(&looping),
            "three copies of `{DEST_ENCODER_SKIP_LOOP}` must count as a loop"
        );
        assert!(
            REPETITIVE_GENERATION_USER_MESSAGE.contains("repeating the same sentence"),
            "user-facing stop must name repeating sentence, got {REPETITIVE_GENERATION_USER_MESSAGE}"
        );
    }

    #[test]
    fn dest_encoder_skip_single_sentence_four_times_is_repetitive() {
        let (first, _) = DEST_ENCODER_SKIP_LOOP
            .split_once(". ")
            .expect("fixture is two sentences");
        let sentence = format!("{first}. ");
        assert!(!is_repetitive(&sentence.repeat(3)));
        assert!(is_repetitive(&sentence.repeat(4)));
    }

    #[test]
    fn ordinary_word_repetition_does_not_stop() {
        assert!(!is_repetitive("the the the the the the the the"));
    }

    #[test]
    fn legitimate_list_does_not_stop() {
        let list = (0..8)
            .map(|i| format!("Step {i}: skip the dest encoder for this unique item."))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!is_repetitive(&list));
    }

    #[test]
    fn thought_line_loop_is_repetitive() {
        let line = format!("{DEST_ENCODER_SKIP_LOOP}\n");
        assert!(is_repetitive(&line.repeat(3)));
    }
}
