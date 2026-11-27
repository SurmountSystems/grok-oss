//! Turn-end on `run_shell_child_body`. This calls `apply_handle_request_turn_end`,
//! the function that body calls before `end_local_session`.

use super::HandleRequestTurnEnd;
use xai_grok_tools::implementations::grok_build::task::types::SubagentEvent;

fn channels() -> (
    tokio::sync::mpsc::UnboundedSender<crate::session::SessionCommand>,
    tokio::sync::mpsc::UnboundedReceiver<crate::session::SessionCommand>,
    tokio::sync::mpsc::UnboundedSender<SubagentEvent>,
    tokio::sync::mpsc::UnboundedReceiver<SubagentEvent>,
) {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (events_tx, events_rx) = tokio::sync::mpsc::unbounded_channel();
    (cmd_tx, cmd_rx, events_tx, events_rx)
}

/// An L2 whose output has no own report must not receive Shutdown.
/// The same session must still accept a later Interject.
#[tokio::test]
async fn handle_request_l2_without_own_report_keeps_the_session_open() {
    let (cmd_tx, mut cmd_rx, events_tx, mut events_rx) = channels();
    let shut_down = super::apply_handle_request_turn_end(HandleRequestTurnEnd {
        cmd_tx: &cmd_tx,
        events: &events_tx,
        subagent_id: "l2",
        parent_depth: 0,
        child_depth: 1,
        output: "the turn returned without a report",
        cancelled: false,
        spawned_child_still_running: false,
    });
    assert!(
        !shut_down,
        "an L2 with no own report must stay open on the handle_request turn end"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "Shutdown must not be sent when the L2 has no own report"
    );
    assert!(
        events_rx.try_recv().is_err(),
        "a missing report must not be recorded as RecordOwnReport"
    );
    cmd_tx
        .send(crate::session::SessionCommand::Interject {
            text: "write the L2 report".to_owned(),
            id: None,
            images: Vec::new(),
        })
        .expect("the open session must accept Interject");
    let next = cmd_rx
        .try_recv()
        .expect("Interject queued on the open session");
    assert!(
        matches!(next, crate::session::SessionCommand::Interject { .. }),
        "the queued command must be Interject"
    );
}

/// An L2 that names its report shuts down only after RecordOwnReport.
#[tokio::test]
async fn handle_request_l2_with_own_report_records_it_before_shutdown() {
    let (cmd_tx, mut cmd_rx, events_tx, mut events_rx) = channels();
    let shut_down = super::apply_handle_request_turn_end(HandleRequestTurnEnd {
        cmd_tx: &cmd_tx,
        events: &events_tx,
        subagent_id: "l2",
        parent_depth: 0,
        child_depth: 1,
        output: "wrote ~/.agents/reports/l2.md\n",
        cancelled: false,
        spawned_child_still_running: false,
    });
    assert!(shut_down, "an L2 that wrote its report may shut down");
    match events_rx.try_recv() {
        Ok(SubagentEvent::RecordOwnReport { subagent_id, .. }) => {
            assert_eq!(subagent_id, "l2");
        }
        Ok(_) => panic!("RecordOwnReport must be sent before Shutdown"),
        Err(_) => panic!("RecordOwnReport must be sent before Shutdown"),
    }
    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(crate::session::SessionCommand::Shutdown(_))
        ),
        "Shutdown follows the recorded report"
    );
}

/// An L3 with no report still shuts down. The hold is for an L2.
#[tokio::test]
async fn handle_request_l3_without_own_report_still_shuts_down() {
    let (cmd_tx, mut cmd_rx, events_tx, mut events_rx) = channels();
    let shut_down = super::apply_handle_request_turn_end(HandleRequestTurnEnd {
        cmd_tx: &cmd_tx,
        events: &events_tx,
        subagent_id: "l3",
        parent_depth: 1,
        child_depth: 2,
        output: "specialist finished",
        cancelled: false,
        spawned_child_still_running: false,
    });
    assert!(shut_down, "an L3 still shuts down when its turn ends");
    assert!(
        events_rx.try_recv().is_err(),
        "an L3 must not record an L2 report"
    );
    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(crate::session::SessionCommand::Shutdown(_))
        ),
        "an L3 turn end sends Shutdown"
    );
}
