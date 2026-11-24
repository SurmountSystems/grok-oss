//! Distinct sentences that share an opening are not one repeated sentence.
//! Wired from `stream/mod.rs` as `distinct_advisory_urls`.

use super::*;
use crate::events::SamplingChannel;
use xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE;

const ADVISORY_URLS: [&str; 9] = [
    "https://rustsec.org/advisories/RUSTSEC-2026-0574",
    "https://rustsec.org/advisories/RUSTSEC-2026-0575",
    "https://rustsec.org/advisories/RUSTSEC-2026-0576",
    "https://rustsec.org/advisories/RUSTSEC-2026-0577",
    "https://rustsec.org/advisories/RUSTSEC-2026-0578",
    "https://rustsec.org/advisories/RUSTSEC-2026-0579",
    "https://rustsec.org/advisories/RUSTSEC-2026-0580",
    "https://rustsec.org/advisories/RUSTSEC-2026-0581",
    "https://rustsec.org/advisories/RUSTSEC-2026-0582",
];

const DRY_RUN_STATUS: &str =
    "The dry-run failed. Exit code 1, after 49 seconds. The generation was not switched.";

const AUDIT_STATUS: &str = "The audit also failed, for a separate reason. cargo-audit exited 1 with nine unsound warnings. Those advisories stay visible:";

fn all_share_opening(lines: &[&str], n: usize) -> bool {
    let Some(first) = lines.first().copied() else {
        return false;
    };
    let head: String = first.chars().take(n).collect();
    head.chars().count() == n && lines.iter().all(|line| line.starts_with(&head))
}

fn none_is_a_prefix_of_another(lines: &[&str]) -> bool {
    for (i, left) in lines.iter().enumerate() {
        for (j, right) in lines.iter().enumerate() {
            if i != j && (right.starts_with(left) || left.starts_with(right)) {
                return false;
            }
        }
    }
    true
}

fn numbered_lines(bodies: &[&str]) -> String {
    let mut out = String::new();
    for (i, body) in bodies.iter().enumerate() {
        let n = i + 1;
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&n.to_string());
        out.push_str(". ");
        out.push_str(body);
    }
    out
}

fn advisory_list_reply() -> String {
    let mut out = String::from(DRY_RUN_STATUS);
    out.push('\n');
    out.push_str(AUDIT_STATUS);
    out.push('\n');
    out.push_str(&numbered_lines(&ADVISORY_URLS));
    out
}

/// Two status paragraphs, then nine advisory URLs. The URLs share
/// `https://rustsec.org/advi` and then differ. Item 9 is fed through the
/// guard cut at `https://rustsec.org/`, which is where the live reply
/// stopped, and then the rest of `RUSTSEC-2026-0582` is fed. The whole
/// reply stays. The stop sentence is not part of it.
#[test]
fn distinct_advisory_urls_are_kept_and_the_stop_sentence_is_absent() {
    assert_ne!(DRY_RUN_STATUS, AUDIT_STATUS);
    assert!(
        all_share_opening(&ADVISORY_URLS, 24),
        "the nine URLs must share a 24-character opening"
    );
    let first = ADVISORY_URLS.first().copied().expect("first advisory");
    let head: String = first.chars().take(24).collect();
    assert_eq!(head, "https://rustsec.org/advi");
    assert!(
        none_is_a_prefix_of_another(&ADVISORY_URLS),
        "a finished advisory URL is not a prefix of another"
    );

    let full = advisory_list_reply();
    let host = "https://rustsec.org/";
    let Some(host_at) = full.rfind(host) else {
        panic!("item 9 must contain {host}");
    };
    let (before_host, from_host) = full.split_at(host_at);
    let Some(rest) = from_host.strip_prefix(host) else {
        panic!("item 9 must start with {host}");
    };
    assert_eq!(rest, "advisories/RUSTSEC-2026-0582");
    assert!(
        before_host.ends_with("9. "),
        "the cut must be item 9, not an earlier URL"
    );

    let mut guard = StreamRepetitionGuard::default();
    guard.append(SamplingChannel::Text, before_host);
    assert!(
        !guard.is_looping(SamplingChannel::Text),
        "the reply through item 8 must not stop"
    );
    guard.append(SamplingChannel::Text, host);
    assert!(
        !guard.is_looping(SamplingChannel::Text),
        "item 9 cut at {host} must not stop"
    );
    guard.append(SamplingChannel::Text, rest);
    assert!(
        !guard.is_looping(SamplingChannel::Text),
        "the finished ninth advisory URL must not stop"
    );
    assert!(
        !is_repetitive(&full),
        "two status paragraphs and nine distinct advisory URLs are not one repeated sentence"
    );

    assert!(
        !full.contains(REPETITIVE_GENERATION_USER_MESSAGE),
        "the kept reply must not contain the stop sentence"
    );
    for url in ADVISORY_URLS {
        assert!(
            full.contains(url),
            "the kept reply must include {url}, stop sentence must stay absent"
        );
    }
    assert!(
        full.contains("RUSTSEC-2026-0582"),
        "item 9 must keep the full advisory id"
    );
    assert!(full.contains(DRY_RUN_STATUS));
    assert!(full.contains(AUDIT_STATUS));
}

/// Same shape as the advisory list, on a different host and with letter
/// tails instead of advisory numbers. A host allowlist would not keep this.
#[test]
fn distinct_lines_that_share_an_opening_are_not_one_sentence() {
    let lines = [
        "Read the north sensor log for bay alpha today.",
        "Read the north sensor log for bay bravo today.",
        "Read the north sensor log for bay cedar today.",
        "Read the north sensor log for bay delta today.",
        "Read the north sensor log for bay ember today.",
        "Read the north sensor log for bay flint today.",
        "Read the north sensor log for bay grove today.",
        "Read the north sensor log for bay haven today.",
        "Read the north sensor log for bay ivory today.",
    ];
    assert!(
        all_share_opening(&lines, 24),
        "the nine sensor lines must share a 24-character opening"
    );
    assert!(
        none_is_a_prefix_of_another(&lines),
        "a finished sensor line is not a prefix of another"
    );
    assert!(
        lines.iter().all(|line| !line.contains("rustsec.org")),
        "this fixture must not depend on the rustsec.org host"
    );

    let mut full = String::from("The west dock is clear today.\n");
    full.push_str("The east dock is not clear today.\n");
    full.push_str(&numbered_lines(&lines));

    let opening = "Read the north sensor log ";
    let Some(at) = full.rfind(opening) else {
        panic!("item 9 must contain the shared opening");
    };
    let (before, from_opening) = full.split_at(at);
    assert!(before.ends_with("9. "));
    let Some(tail) = from_opening.strip_prefix(opening) else {
        panic!("item 9 must start at the shared opening");
    };
    assert_eq!(tail, "for bay ivory today.");

    let mut guard = StreamRepetitionGuard::default();
    guard.append(SamplingChannel::Text, before);
    guard.append(SamplingChannel::Text, opening);
    assert!(
        !guard.is_looping(SamplingChannel::Text),
        "a shared opening on item 9 must not stop distinct sensor lines"
    );
    guard.append(SamplingChannel::Text, tail);
    assert!(!guard.is_looping(SamplingChannel::Text));
    assert!(!is_repetitive(&full));
    assert!(!full.contains(REPETITIVE_GENERATION_USER_MESSAGE));
    for line in lines {
        assert!(full.contains(line));
    }
}

/// The same sentence, with no new number, URL, or fact, still stops.
/// Two copies do not. Eight copies do, and the stop sentence is produced.
#[test]
fn identical_sentence_repeated_still_stops_the_turn() {
    let sentence = "I am repeating this exact prose sentence. ";
    assert!(
        !sentence.chars().any(|ch| ch.is_ascii_digit()),
        "the repeated sentence must not smuggle a new number"
    );
    assert!(
        !sentence.contains("://"),
        "the repeated sentence must not smuggle a URL"
    );
    assert!(
        sentence.len() >= MIN_PHRASE_CHARS && sentence.len() < LONG_PHRASE_CHARS,
        "the repeated sentence must sit in the short-phrase band, got {}",
        sentence.len()
    );
    assert!(
        !is_repetitive(&sentence.repeat(2)),
        "two copies of one sentence must not stop"
    );

    let repeated = sentence.repeat(MIN_SHORT_REPEATS);
    assert!(
        is_repetitive(&repeated),
        "{MIN_SHORT_REPEATS} copies of one sentence must still stop"
    );
    let mut guard = StreamRepetitionGuard::default();
    guard.append(SamplingChannel::Text, &repeated);
    assert!(
        guard.is_looping(SamplingChannel::Text),
        "the guard must still stop an identical sentence repeated"
    );
    let err = guard.error(SamplingChannel::Text, 1);
    assert_eq!(err.to_string(), REPETITIVE_GENERATION_USER_MESSAGE);
    assert!(
        !repeated.contains(REPETITIVE_GENERATION_USER_MESSAGE),
        "the loop is the repeated sentence, not a copy of the stop line"
    );
}
