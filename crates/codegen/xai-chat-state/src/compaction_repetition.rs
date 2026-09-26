//! Detect high-repetition assistant / reasoning blocks for compact recovery.
//!
//! Same thresholds as the live-stream breaker in `xai-grok-sampler` (copied,
//! not imported): a short sentence four times, a two-sentence cycle three
//! times, or a 64-character block three times when a smear keeps that block.
//! Compact must drop that wall so summarizer input and reseed are not
//! `Context compacted: 75.2k → 75.2k tokens`. Not a second stream stop.

use std::collections::HashMap;

use xai_grok_sampling_types::{ConversationItem, synthesized_reasoning_item};

/// Replaces a looping assistant or reasoning block in compact input / seed.
pub const REPETITIVE_ASSISTANT_OMITTED: &str = "[repetitive assistant output omitted]";

/// Tail inspected for a repeating unit. Long enough for three copies of a
/// short two-sentence cycle (the dest-encoder-skip loop is about 70 characters).
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
/// onto itself. Distinct numbered steps share a shorter tail and must
/// not be stripped. Matches the live-stream breaker.
const SMEAR_BLOCK_CHARS: usize = 64;

/// True when `text` is an obvious looping sentence, a two-sentence cycle,
/// or a long block that still repeats after a smear.
pub fn is_repetitive_generation(text: &str) -> bool {
    let tail = tail_window(text);
    if tail.len() < MIN_PHRASE_CHARS * MIN_LONG_REPEATS {
        return false;
    }
    trailing_units_loop(split_sentences(tail))
        || trailing_units_loop(split_nonempty_lines(tail))
        || repeated_block_survives_smear(tail)
}

/// Replace looping assistant and reasoning content with
/// [`REPETITIVE_ASSISTANT_OMITTED`]. User prompts and tool results are kept.
pub fn strip_repetitive_generation(conversation: Vec<ConversationItem>) -> Vec<ConversationItem> {
    conversation
        .into_iter()
        .map(stub_looping_conversation_item)
        .collect()
}

pub(crate) fn stub_looping_conversation_item(item: ConversationItem) -> ConversationItem {
    match item {
        ConversationItem::Assistant(mut a) => {
            if is_repetitive_generation(&a.content) {
                a.content = REPETITIVE_ASSISTANT_OMITTED.into();
            }
            ConversationItem::Assistant(a)
        }
        ConversationItem::Reasoning(r) => {
            if is_repetitive_generation(&xai_grok_sampling_types::reasoning_item_text(&r)) {
                ConversationItem::Reasoning(synthesized_reasoning_item(
                    REPETITIVE_ASSISTANT_OMITTED,
                ))
            } else {
                ConversationItem::Reasoning(r)
            }
        }
        other => other,
    }
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

/// Copied from the live-stream breaker. Not a second detector.
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
            if window.contains(char::is_whitespace) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Operator screenshot 2026-09-20 Isolated Preview: the stream looped this
    /// pair until cancel (GitHub #133). Copied here; do not import from the
    /// sampler crate. Compact drop is recovery after the live Fatal breaker,
    /// not a second stream stop.
    const DEST_ENCODER_SKIP_LOOP: &str =
        "Spawn dests of dest encoder skip. I'll spawn dests of dest encoder skip.";

    #[test]
    fn dest_encoder_skip_loop_is_repetitive() {
        let one = format!("{DEST_ENCODER_SKIP_LOOP} ");
        assert!(
            !is_repetitive_generation(&one.repeat(2)),
            "two copies of the dest-encoder-skip pair must not strip ordinary restatement"
        );
        assert!(
            is_repetitive_generation(&one.repeat(3)),
            "three copies of `{DEST_ENCODER_SKIP_LOOP}` must count as a loop"
        );
    }

    #[test]
    fn dest_encoder_skip_single_sentence_four_times_is_repetitive() {
        let sentence = "Spawn dests of dest encoder skip. ";
        assert!(!is_repetitive_generation(&sentence.repeat(3)));
        assert!(is_repetitive_generation(&sentence.repeat(4)));
    }

    #[test]
    fn ordinary_word_repetition_is_not_a_loop() {
        assert!(!is_repetitive_generation("the the the the the the the the"));
    }

    #[test]
    fn unique_step_list_is_not_a_loop() {
        let list = (0..8)
            .map(|i| format!("Step {i}: skip the dest encoder for this unique item."))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!is_repetitive_generation(&list));
    }

    #[test]
    fn thought_line_loop_is_repetitive() {
        let line = "Spawn dests of dest encoder skip. I'll spawn dests of dest encoder skip.\n";
        assert!(is_repetitive_generation(&line.repeat(3)));
    }

    #[test]
    fn strip_replaces_looping_assistant_keeps_user_and_tool() {
        let one = format!("{DEST_ENCODER_SKIP_LOOP} ");
        let out = strip_repetitive_generation(vec![
            ConversationItem::user("Keep Isolated Preview open"),
            ConversationItem::tool_result("c1", "Wrote /home/hunter/.agents/reports/dest.md"),
            ConversationItem::assistant(one.repeat(3)),
        ]);
        assert_eq!(out[0].text_content(), "Keep Isolated Preview open");
        assert!(out[1].text_content().contains("dest.md"));
        assert_eq!(out[2].text_content(), REPETITIVE_ASSISTANT_OMITTED);
    }
}
