//! Whether a repetitive-generation stop ends the session.
//!
//! Detection of a repeated sentence stays in the stream guard. This module
//! only chooses end versus stay-up. Tools does not depend on the sampling
//! crate, so [`REPETITIVE_STOP_SENTENCE`] is the same text as
//! `REPETITIVE_GENERATION_USER_MESSAGE`. The shell test locks those together.

/// Operator-facing stop when the stream was repeating the same sentence.
pub const REPETITIVE_STOP_SENTENCE: &str = "Stopped: the reply was repeating the same sentence.";

/// End the session, or leave an unfinished L2 up so it can continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepetitiveGenerationSessionEnd {
    /// Fatal stop still ends the session. That is L1, L3, and any session
    /// that is not an unfinished ordinary L2.
    End,
    /// Unfinished ordinary L2 stays up and can continue.
    StayUp,
}

/// Facts that count as the L2's owed work succeeding.
///
/// A specialist spawn does not count. The repetitive-stop sentence does not
/// count. Success is the owed edit in the tree and a reviewer pass, or a
/// blocker report that names a path and a quoted error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwedL2Work {
    pub edit_landed: bool,
    pub reviewer_passed: bool,
    pub spawned_specialist: bool,
    pub blocker_report_names_path_and_quoted_error: bool,
}

impl OwedL2Work {
    pub fn succeeded(self) -> bool {
        let spawn_alone = self.spawned_specialist
            && !self.edit_landed
            && !self.reviewer_passed
            && !self.blocker_report_names_path_and_quoted_error;
        if spawn_alone {
            return false;
        }
        (self.edit_landed && self.reviewer_passed)
            || self.blocker_report_names_path_and_quoted_error
    }
}

/// Unfinished ordinary L2 plus this stop does not end the session.
/// Every other depth still ends. A succeeded L2 is not forced to stay by
/// this stop; the unfinished case is the one that must stay up.
pub fn repetitive_generation_session_end(
    is_ordinary_l2: bool,
    owed: OwedL2Work,
) -> RepetitiveGenerationSessionEnd {
    if is_ordinary_l2 && !owed.succeeded() {
        RepetitiveGenerationSessionEnd::StayUp
    } else {
        RepetitiveGenerationSessionEnd::End
    }
}

pub fn session_is_ended(decision: RepetitiveGenerationSessionEnd) -> bool {
    matches!(decision, RepetitiveGenerationSessionEnd::End)
}

/// Wire names of the subagent spawn tool. `spawn_subagent` is the L2 tool.
pub fn tool_name_records_specialist_spawn(name: &str) -> bool {
    super::is_task_tool_id(name)
}

/// Wire names of an edit that can land in the tree. A call is not a reviewer pass.
pub fn tool_name_records_edit(name: &str) -> bool {
    matches!(name, "search_replace" | "write" | "apply_patch")
}

/// Owed work read from tools the session recorded and from assistant or tool text.
/// A specialist spawn is not success. The stop sentence is not a blocker report.
pub fn owed_l2_work_from_session_record(tools_used: &[String], transcript: &str) -> OwedL2Work {
    let stop = REPETITIVE_STOP_SENTENCE;
    OwedL2Work {
        edit_landed: tools_used.iter().any(|name| tool_name_records_edit(name)),
        reviewer_passed: reviewer_passed_besides_stop(transcript, stop),
        spawned_specialist: tools_used
            .iter()
            .any(|name| tool_name_records_specialist_spawn(name)),
        blocker_report_names_path_and_quoted_error: blocker_report_names_path_and_quoted_error(
            transcript, stop,
        ),
    }
}

fn reviewer_passed_besides_stop(transcript: &str, stop_sentence: &str) -> bool {
    let stem = stop_stem(stop_sentence);
    let without = transcript.replace(stop_sentence, "").replace(stem, "");
    let lower = without.to_ascii_lowercase();
    lower.contains("reviewer passed") || lower.contains("review passed")
}

/// A blocker report names a path and a quoted error.
/// The repetitive-stop sentence is not that report, even if a path sits beside it.
pub fn blocker_report_names_path_and_quoted_error(report: &str, stop_sentence: &str) -> bool {
    let stem = stop_stem(stop_sentence);
    let trimmed = report.trim();
    if trimmed.is_empty() || trimmed == stop_sentence.trim() || trimmed == stem {
        return false;
    }
    let without_stop = report.replace(stop_sentence, "").replace(stem, "");
    if !without_stop.contains('/') {
        return false;
    }
    quoted_span_besides_stop(&without_stop, stem)
}

fn stop_stem(stop_sentence: &str) -> &str {
    stop_sentence.trim().trim_end_matches('.')
}

fn quoted_span_besides_stop(text: &str, stop_stem: &str) -> bool {
    for quote in ['"', '\''] {
        let mut rest = text;
        while let Some(start) = rest.find(quote) {
            rest = &rest[start + quote.len_utf8()..];
            let Some(end) = rest.find(quote) else {
                break;
            };
            let inner = rest[..end].trim();
            rest = &rest[end + quote.len_utf8()..];
            if inner.is_empty() {
                continue;
            }
            if inner.trim_end_matches('.') == stop_stem {
                continue;
            }
            return true;
        }
    }
    false
}

/// True when `error_text` or `output_text` is this stop and the owed work
/// has not succeeded. Callers that are not an ordinary L2 get false.
pub fn unfinished_l2_text_must_not_end_session(
    is_ordinary_l2: bool,
    error_text: &str,
    output_text: &str,
    owed: OwedL2Work,
) -> bool {
    let stem = stop_stem(REPETITIVE_STOP_SENTENCE);
    let hit = error_text.contains(stem) || output_text.contains(stem);
    hit && repetitive_generation_session_end(is_ordinary_l2, owed)
        == RepetitiveGenerationSessionEnd::StayUp
}

/// Owed work read from a child result whose error or output carries the stop.
/// The stop sentence is not proof that an edit landed or a reviewer passed.
pub fn owed_work_from_stop_texts(error_text: &str, output_text: &str) -> OwedL2Work {
    let stop = REPETITIVE_STOP_SENTENCE;
    OwedL2Work {
        edit_landed: false,
        reviewer_passed: false,
        spawned_specialist: false,
        blocker_report_names_path_and_quoted_error: blocker_report_names_path_and_quoted_error(
            output_text,
            stop,
        ) || blocker_report_names_path_and_quoted_error(
            error_text, stop,
        ),
    }
}
