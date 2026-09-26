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

use std::collections::HashMap;
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
// A numbered list's periods split one checklist into many units, and a
// later smear is not byte-identical, so that unit streak resets. A long
// block that still sits inside the smear is the same streak, not a
// second breaker.

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
/// Block that still counts when a smear drops words or pastes the list
/// onto itself. Distinct numbered steps share a shorter tail (the unique
/// step list's repeated clause is 43 characters) and must not stop.
const SMEAR_BLOCK_CHARS: usize = 64;

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
        let channel_name = channel.as_str();
        append_repetition_stop_log(channel_name, chunk_index);
        SamplingError::RepetitiveGeneration {
            channel: channel_name.to_string(),
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

/// True when `text` ends in an obvious looping sentence, a two-sentence
/// cycle, or a long block that still repeats after a smear.
pub(crate) fn is_repetitive(text: &str) -> bool {
    if identical_character_run_hit_256(text) {
        return true;
    }
    let tail = tail_window(text);
    if tail.len() < MIN_PHRASE_CHARS * MIN_LONG_REPEATS {
        return false;
    }
    trailing_units_loop(split_sentences(tail))
        || trailing_units_loop(split_nonempty_lines(tail))
        || repeated_block_survives_smear(tail)
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
    if cycles >= MIN_LONG_REPEATS {
        return true;
    }
    // A 3- or 4-line cycle smears inside one line. Compare trimmed text
    // with collapsed whitespace, drop a leading "L3." or "N." marker, and
    // split on " N. ". The same step matches on its first 24 characters.
    // A shorter smear matches when it is still a prefix of that step.
    let step_pieces = |unit: &str| -> Vec<String> {
        let mut collapsed = String::new();
        let mut prev_space = false;
        for ch in unit.trim().chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    collapsed.push(' ');
                    prev_space = true;
                }
            } else {
                collapsed.push(ch);
                prev_space = false;
            }
        }
        let mut body = collapsed.as_str();
        if let Some(after) = body.strip_prefix("L3.") {
            body = after.trim_start();
        } else {
            let raw = body.as_bytes();
            let mut k = 0;
            while k < raw.len() && raw[k].is_ascii_digit() {
                k += 1;
            }
            if k > 0 && k < raw.len() && raw[k] == b'.' {
                body = body[k + 1..].trim_start();
            }
        }
        let normalized = body.to_string();
        let bytes = normalized.as_bytes();
        let mut pieces = Vec::new();
        let mut start = 0usize;
        let mut at = 0usize;
        while at < bytes.len() {
            if bytes[at] == b' ' {
                let mut end_num = at + 1;
                while end_num < bytes.len() && bytes[end_num].is_ascii_digit() {
                    end_num += 1;
                }
                if end_num > at + 1
                    && end_num + 1 < bytes.len()
                    && bytes[end_num] == b'.'
                    && bytes[end_num + 1] == b' '
                {
                    let piece = normalized[start..at].trim();
                    if !piece.is_empty() {
                        pieces.push(piece.to_string());
                    }
                    start = end_num + 2;
                    at = start;
                    continue;
                }
            }
            at += 1;
        }
        let tail = normalized[start..].trim();
        if !tail.is_empty() {
            pieces.push(tail.to_string());
        }
        pieces
    };
    let same_step = |left: &str, right: &str| -> bool {
        let left_n = left.chars().count();
        let right_n = right.chars().count();
        if left_n >= 24 && right_n >= 24 {
            let left_head: String = left.chars().take(24).collect();
            let right_head: String = right.chars().take(24).collect();
            return left_head == right_head;
        }
        if left_n < 24 && right_n < 24 {
            return left == right;
        }
        let (short, long) = if left_n < right_n {
            (left, right)
        } else {
            (right, left)
        };
        short.chars().count() >= 20 && long.starts_with(short)
    };
    let same_unit = |left: &str, right: &str| -> bool {
        let left_pieces = step_pieces(left);
        let right_pieces = step_pieces(right);
        !left_pieces.is_empty()
            && left_pieces.len() == right_pieces.len()
            && left_pieces
                .iter()
                .zip(right_pieces.iter())
                .all(|(left_piece, right_piece)| same_step(left_piece, right_piece))
    };
    for period in [3usize, 4] {
        let need = period * MIN_LONG_REPEATS;
        if units.len() < need {
            continue;
        }
        let start = units.len() - need;
        let mut matched = true;
        for offset in 0..period {
            let base = units[start + offset];
            for rep in 1..MIN_LONG_REPEATS {
                if !same_unit(base, units[start + rep * period + offset]) {
                    matched = false;
                    break;
                }
            }
            if !matched {
                break;
            }
        }
        if matched {
            return true;
        }
    }
    false
}

fn is_loop_phrase(unit: &str) -> bool {
    unit.len() >= MIN_PHRASE_CHARS && unit.contains(char::is_whitespace)
}

fn identical_character_run_hit_256(text: &str) -> bool {
    // The counter is a u8. The 256th identical character stops, and it never wraps.
    let mut run: u8 = 0;
    let mut prev: Option<char> = None;
    for ch in text.chars() {
        if prev == Some(ch) {
            if run == 255 {
                return true;
            }
            run += 1;
        } else {
            prev = Some(ch);
            run = 1;
        }
    }
    false
}

fn append_repetition_stop_log(channel: &str, chunk_index: u64) {
    let dir = "/home/hunter/.agents/logs";
    let path = "/home/hunter/.agents/logs/repetition-stops.log";
    let _ = std::fs::create_dir_all(dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let line = format!("repetition stop channel={channel} chunk={chunk_index}\n");
        let _ = std::io::Write::write_all(&mut file, line.as_bytes());
    }
}

fn smear_block_is_mostly_spaces(block: &str) -> bool {
    // A short run of padding spaces is a fixed-width table, not a smear.
    // A block that is more than half spaces is not a smear either.
    let mut spaces = 0usize;
    let mut total = 0usize;
    let mut run: u8 = 0;
    let mut longest: u8 = 0;
    for ch in block.chars() {
        total += 1;
        if ch == ' ' {
            spaces += 1;
            run = run.saturating_add(1);
            if run > longest {
                longest = run;
            }
        } else {
            run = 0;
        }
    }
    total > 0 && (spaces * 2 > total || longest >= 3)
}

fn repeated_block_survives_smear(tail: &str) -> bool {
    if tail.len() < SMEAR_BLOCK_CHARS * MIN_LONG_REPEATS {
        return false;
    }
    let mut starts: HashMap<&str, Vec<usize>> = HashMap::new();
    let n = tail.len();
    let mut i = 0;
    while i + SMEAR_BLOCK_CHARS <= n {
        if tail.is_char_boundary(i) && tail.is_char_boundary(i + SMEAR_BLOCK_CHARS) {
            let window = &tail[i..i + SMEAR_BLOCK_CHARS];
            if window.contains(char::is_whitespace) && !smear_block_is_mostly_spaces(window) {
                starts.entry(window).or_default().push(i);
            }
        }
        i += 1;
    }
    starts.values().any(|at| {
        let mut count = 0usize;
        let mut last: Option<usize> = None;
        for &pos in at {
            let far = match last {
                Some(prev) => pos >= prev + SMEAR_BLOCK_CHARS,
                None => true,
            };
            if far {
                count += 1;
                last = Some(pos);
                if count >= MIN_LONG_REPEATS {
                    return true;
                }
            }
        }
        false
    })
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
    use crate::events::SamplingChannel;
    use xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE;
    use xai_grok_sampling_types::SamplingError;

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

    /// Operator screenshot Fri Sep 25, 2026, 12:58 PM. Thinking stayed on
    /// this sentence for 12 minutes 36 seconds, then the copies smeared.
    /// Words dropped, and the same list was concatenated onto itself.
    /// The scored tail is that smear, not three byte-identical copies, so
    /// a streak that resets on the smear does not stop the turn. Same
    /// `StreamRepetitionGuard` and `RepetitiveGeneration` stop.
    #[test]
    fn repeated_needle_update_sentence_and_smear_stops_the_turn() {
        const SENTENCE: &str = "Let me start with the needle updates: 1. Update the needles and shape checks 2. Update the header comment 3. Update the imports section 4. Update the fuel comment";
        const SMEAR: &str = "Let me start with the needle 1. Update the needles and shape checks 2. Update the header comment 3. Update the imports section 4. Update the fuel comment";
        const CONCAT: &str = "Let me start with the needle 1. Update the needles and shape checks 2. Update the header comment 3. Update the imports section 4. Update the 1. Update the needles and shape checks 2. Update the header comment 3. Update the imports section 4. Update the fuel comment";
        assert_eq!(
            SENTENCE,
            "Let me start with the needle updates: 1. Update the needles and shape checks 2. Update the header comment 3. Update the imports section 4. Update the fuel comment"
        );
        assert!(
            SMEAR.starts_with("Let me start with the needle 1. Update the needles"),
            "smear must drop words from the needle-update sentence"
        );
        assert!(
            CONCAT.contains(
                "4. Update the 1. Update the needles and shape checks 2. Update the header comment 3. Update the imports section 4. Update the fuel comment"
            ),
            "concatenation must paste the list onto itself"
        );

        let mut restatement = StreamRepetitionGuard::default();
        restatement.append(SamplingChannel::Reasoning, SENTENCE);
        restatement.append(SamplingChannel::Reasoning, " ");
        restatement.append(SamplingChannel::Reasoning, SENTENCE);
        assert!(
            !restatement.is_looping(SamplingChannel::Reasoning),
            "two copies of the needle-update sentence must not stop an ordinary restatement"
        );

        let mut stream = String::new();
        for _ in 0..4 {
            stream.push_str(SENTENCE);
            stream.push(' ');
        }
        for _ in 0..6 {
            stream.push_str(SMEAR);
            stream.push(' ');
            stream.push_str(CONCAT);
            stream.push(' ');
        }
        assert!(
            stream.len() > WINDOW_CHARS,
            "fixture must be longer than the breaker window"
        );
        let tail_at = stream.len() - WINDOW_CHARS;
        assert!(
            stream.is_char_boundary(tail_at),
            "window cut must be a char boundary"
        );
        let tail = &stream[tail_at..];
        assert!(
            !tail.contains("needle updates:"),
            "scored tail must be the smear, so a byte-identical sentence streak has reset"
        );
        assert!(
            tail.contains("Let me start with the needle 1. Update the needles"),
            "scored tail must still be this needle-update loop"
        );

        let mut guard = StreamRepetitionGuard::default();
        for chunk in stream.split_inclusive(' ') {
            guard.append(SamplingChannel::Reasoning, chunk);
        }
        assert!(
            guard.is_looping(SamplingChannel::Reasoning),
            "needle-update sentence plus smears must stop the turn"
        );
        match guard.error(SamplingChannel::Reasoning, 1) {
            SamplingError::RepetitiveGeneration {
                channel,
                aborted_at_chunk,
            } => {
                assert_eq!(channel, SamplingChannel::Reasoning.as_str());
                assert_eq!(aborted_at_chunk, Some(1));
                assert!(
                    REPETITIVE_GENERATION_USER_MESSAGE.contains("repeating the same sentence"),
                    "user-facing stop must name repeating sentence, got {REPETITIVE_GENERATION_USER_MESSAGE}"
                );
            }
            other => panic!("expected RepetitiveGeneration, got {other:?}"),
        }
    }

    /// Operator screenshot Sat Sep 26, 2026, 6:13 AM. Thinking stayed on
    /// this four-line list for 35 minutes 42 seconds, then the copies
    /// smeared. Each line is shorter than the 64-character smear block.
    /// The second line glues "hardening in AG" onto "3. Implement skill".
    /// Same `StreamRepetitionGuard` and `RepetitiveGeneration` stop. One
    /// pass of these different lines must not stop.
    #[test]
    fn repeated_four_line_hardening_list_and_smear_stops_the_turn() {
        const L1: &str = "L3. L1 interactions hardening in AGENTS.md";
        const L2: &str = "L3. Plan approval hardening in AGENTS.md";
        const L3LINE: &str = "L3. Implement skill hardening in AGENTS.md";
        const L4: &str = "L3. Compact pin hardening in host AGENTS.md";
        const CYCLE0: &str = "L3. Plan approval hardening in AGENTS.md 3. Implement skill hardening in AGENTS.md 4. Compact pin hardening in host AGENTS.md";
        const CYCLE1: &str = "L3. Plan approval hardening in AG 3. Implement skill hardening in AGENTS.md 4. Compact pin hardening in host AGENTS.md";
        // Cycle 2 smears the glue prefix and the copied tail. A single
        // prefix edit leaves "3. Implement..." and the following two lines
        // as a third 64-character copy. Extra spaces collapse back to the
        // same steps. L1, the implement line, and the compact line stay.
        const CYCLE2: &str = "L3.  Plan  approval  hardenin  3.  Implement  skill  hardening  in  AGENTS.md  4.  Compact  pin  hardening  in  host  AGENTS.md";

        let block = format!("{L1}\n{L2}\n{L3LINE}\n{L4}");
        let mut once = StreamRepetitionGuard::default();
        once.append(SamplingChannel::Reasoning, &block);
        assert!(
            !once.is_looping(SamplingChannel::Reasoning),
            "one joined block must not stop"
        );

        let mut twice = StreamRepetitionGuard::default();
        twice.append(SamplingChannel::Reasoning, &block);
        twice.append(SamplingChannel::Reasoning, "\n");
        twice.append(SamplingChannel::Reasoning, &block);
        assert!(
            !twice.is_looping(SamplingChannel::Reasoning),
            "two copies must not stop"
        );

        let mut guard = StreamRepetitionGuard::default();
        let mut stream = String::new();
        for (cycle_i, second) in [CYCLE0, CYCLE1, CYCLE2].into_iter().enumerate() {
            let lines = [L1, second, L3LINE, L4];
            for (line_i, line) in lines.into_iter().enumerate() {
                guard.append(SamplingChannel::Reasoning, line);
                stream.push_str(line);
                // Cycle 2's junctions must not rebuild the 64-character
                // pair of the implement line and the compact line.
                let sep = if cycle_i == 2 && (line_i == 1 || line_i == 2) {
                    " \n"
                } else {
                    "\n"
                };
                guard.append(SamplingChannel::Reasoning, sep);
                stream.push_str(sep);
            }
        }

        let width = 64usize;
        let mut starts: std::collections::HashMap<&str, Vec<usize>> =
            std::collections::HashMap::new();
        let n = stream.len();
        let mut i = 0usize;
        while i + width <= n {
            if stream.is_char_boundary(i) && stream.is_char_boundary(i + width) {
                let window = &stream[i..i + width];
                if window.contains(char::is_whitespace) {
                    starts.entry(window).or_default().push(i);
                }
            }
            i += 1;
        }
        let triple = starts.values().any(|at| {
            let mut count = 0usize;
            let mut last: Option<usize> = None;
            for &pos in at {
                let far = match last {
                    Some(prev) => pos >= prev + width,
                    None => true,
                };
                if far {
                    count += 1;
                    last = Some(pos);
                    if count >= 3 {
                        return true;
                    }
                }
            }
            false
        });
        assert!(
            !triple,
            "fewer than three identical 64-character windows at least 64 apart"
        );
        assert!(
            guard.is_looping(SamplingChannel::Reasoning),
            "four-line hardening list plus smear must stop the turn, got continue"
        );
        match guard.error(SamplingChannel::Reasoning, 1) {
            SamplingError::RepetitiveGeneration {
                channel,
                aborted_at_chunk,
            } => {
                assert_eq!(channel, SamplingChannel::Reasoning.as_str());
                assert_eq!(aborted_at_chunk, Some(1));
                assert!(
                    REPETITIVE_GENERATION_USER_MESSAGE.contains("repeating the same sentence"),
                    "user-facing stop must name repeating sentence, got {REPETITIVE_GENERATION_USER_MESSAGE}"
                );
            }
            other => panic!("expected RepetitiveGeneration, got {other:?}"),
        }
    }

    /// Markdown job table: header, dash row, and data rows. Each data row
    /// has a different name, then 24.0 minutes, 1.64m, just started, and
    /// still open. Space padding is real and well under 256 characters.
    /// Different padded rows are different sentences and must not stop.
    #[test]
    fn padded_job_table_with_different_names_does_not_stop_the_turn() {
        let table = "\
| job         | wall             | tokens   | state          | status       |
| ----------- | ---------------- | -------- | -------------- | ------------ |
| Adastria    | 24.0 minutes     | 1.64m    | just started   | still open   |
| Bellerophon | 24.0 minutes     | 1.64m    | just started   | still open   |
| Callisto    | 24.0 minutes     | 1.64m    | just started   | still open   |
";
        for name in ["Adastria", "Bellerophon", "Callisto"] {
            assert!(table.contains(name), "data row must name {name}");
        }
        assert!(
            table.contains("24.0 minutes")
                && table.contains("1.64m")
                && table.contains("just started")
                && table.contains("still open"),
            "data rows must carry the shared job cells"
        );
        assert!(
            table.lines().any(|line| line.contains("---")),
            "table must include a dash row"
        );
        let mut best = 0usize;
        let mut run = 0usize;
        let mut prev: Option<char> = None;
        for ch in table.chars() {
            if prev == Some(ch) {
                run += 1;
            } else {
                prev = Some(ch);
                run = 1;
            }
            if run > best {
                best = run;
            }
        }
        assert!(
            best < 64,
            "space padding must stay well under 256, longest identical run was {best}"
        );
        let tail = tail_window(table);
        assert!(
            !is_repetitive(table),
            "padded job table must not stop the turn; smear={} sentence_units={} line_units={} longest_run={best}",
            repeated_block_survives_smear(tail),
            trailing_units_loop(split_sentences(tail)),
            trailing_units_loop(split_nonempty_lines(tail)),
        );
    }

    /// This exact sentence three times must stop the turn.
    #[test]
    fn bellerophon_waiting_sentence_three_times_stops_the_turn() {
        const SENTENCE: &str = "I am Bellerophon. I am waiting for the L3s to complete their work.";
        assert!(!is_repetitive(SENTENCE), "one copy must not stop");
        let twice = format!("{SENTENCE}\n{SENTENCE}");
        assert!(!is_repetitive(&twice), "two copies must not stop");
        let thrice = format!("{SENTENCE}\n{SENTENCE}\n{SENTENCE}");
        assert!(
            is_repetitive(&thrice),
            "three copies of the Bellerophon waiting sentence must stop the turn"
        );
    }

    /// 255 spaces must not stop. The 256th identical character must stop.
    /// 200 spaces, one other character, then 200 spaces must not stop.
    #[test]
    fn character_run_stops_at_the_256th_identical_character() {
        assert!(
            !is_repetitive(&" ".repeat(255)),
            "255 spaces in a row must not stop"
        );
        assert!(
            is_repetitive(&" ".repeat(256)),
            "the 256th identical character must stop"
        );
        let mut broken = " ".repeat(200);
        broken.push('x');
        broken.push_str(&" ".repeat(200));
        assert!(
            !is_repetitive(&broken),
            "200 spaces, one other character, then 200 spaces must not stop"
        );
    }
}
