//! An unfinished L2 is not ended when the reply repeats the same sentence.

use xai_grok_sampling_types::REPETITIVE_GENERATION_USER_MESSAGE;
use xai_grok_tools::implementations::grok_build::task::repetitive_l2_session_end::{
    OwedL2Work, REPETITIVE_STOP_SENTENCE, RepetitiveGenerationSessionEnd,
    blocker_report_names_path_and_quoted_error, owed_l2_work_from_session_record,
    repetitive_generation_session_end, session_is_ended,
};

fn unfinished_spawn_only() -> OwedL2Work {
    OwedL2Work {
        edit_landed: false,
        reviewer_passed: false,
        spawned_specialist: true,
        blocker_report_names_path_and_quoted_error: false,
    }
}

#[test]
fn unfinished_l2_is_not_ended_when_the_reply_repeats_the_same_sentence() {
    let stop = REPETITIVE_GENERATION_USER_MESSAGE;
    assert_eq!(stop, REPETITIVE_STOP_SENTENCE);
    assert!(
        stop.contains("repeating the same sentence"),
        "the user-facing stop sentence stays, got {stop}"
    );

    let owed = unfinished_spawn_only();
    assert!(
        !owed.succeeded(),
        "a specialist spawn is not success, and a repeated sentence is not success"
    );
    let decision = repetitive_generation_session_end(true, owed);
    assert_eq!(decision, RepetitiveGenerationSessionEnd::StayUp);
    assert!(
        !session_is_ended(decision),
        "unfinished L2 (spawn only, no edit, no blocker report) plus RepetitiveGeneration does not end the session"
    );

    let echoed = format!("{stop} see ~/.agents/reports/l2.md");
    assert!(
        !blocker_report_names_path_and_quoted_error(&echoed, stop),
        "the repetitive-stop sentence is not a blocker report, even beside a path"
    );
    let echoed_owed = OwedL2Work {
        blocker_report_names_path_and_quoted_error: blocker_report_names_path_and_quoted_error(
            &echoed, stop,
        ),
        ..unfinished_spawn_only()
    };
    let echoed_decision = repetitive_generation_session_end(true, echoed_owed);
    assert!(
        !session_is_ended(echoed_decision),
        "a path mention plus the stop sentence does not end an unfinished L2"
    );

    let edit_without_review = OwedL2Work {
        edit_landed: true,
        reviewer_passed: false,
        spawned_specialist: true,
        blocker_report_names_path_and_quoted_error: false,
    };
    assert!(
        !session_is_ended(repetitive_generation_session_end(true, edit_without_review)),
        "an edit without a reviewer pass is still unfinished"
    );

    assert!(
        session_is_ended(repetitive_generation_session_end(false, owed)),
        "a session that is not an ordinary L2 still ends on this stop"
    );

    let blocker = "blocked on crates/foo.rs: \"cannot find type Bar\"";
    assert!(
        blocker_report_names_path_and_quoted_error(blocker, stop),
        "a path and a quoted error is a real blocker report"
    );
    assert!(stop.contains("Stopped:"));

    let recorded = owed_l2_work_from_session_record(&["spawn_subagent".to_string()], "");
    assert!(
        recorded.spawned_specialist && !recorded.edit_landed && !recorded.succeeded(),
        "a recorded specialist spawn, with no edit and no blocker report, is not success"
    );
    assert!(
        !session_is_ended(repetitive_generation_session_end(true, recorded)),
        "that recorded unfinished L2 is not ended"
    );
    let edit_only = owed_l2_work_from_session_record(
        &["spawn_subagent".to_string(), "search_replace".to_string()],
        "",
    );
    assert!(
        !session_is_ended(repetitive_generation_session_end(true, edit_only)),
        "an edit without a reviewer pass is still unfinished"
    );
}
