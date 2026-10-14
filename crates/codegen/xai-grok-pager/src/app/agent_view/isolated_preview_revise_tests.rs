//! Isolated Preview Revise contracts.
//!
//! Operator: Revise on the secondary plan must revise `secondary-plan.md`.
//! It must not toast "No comments to send" and stop. Still-working chrome
//! stays up while that rewrite runs. The secondary plan must not rewrite
//! the primary `plan.md`. A screenshot paste stays an image chip. A click
//! on the session directory opens the system file manager and shows a
//! brief toast.
//!
//! Red before the helpers existed: `park_isolated_preview_revise_decision`
//! and `isolated_preview_revise_notes_from_comments` were missing (compile
//! break), empty Revise took the casual path and toasted "No comments to
//! send", and `send_plan_feedback` told the nested implementer to update
//! `plan.md` even while Isolated Preview showed the secondary plan.

use super::super::AgentView;
use super::super::test_fixtures::make_agent;
use crate::app::app_view::InputOutcome;
use crate::views::file_search::line_viewer::{LineViewerKind, LineViewerState, SelectedPlanCta};
use crate::views::plan_approval_view::{PLAN_REWRITE_WAIT_STATUS, PlanFeedbackInFlight};
use xai_grok_shell::grok_oss::SECONDARY_PLAN_IDENTITY;

fn docked_isolated_preview() -> AgentView {
    let mut view = make_agent();
    view.isolated_preview_shows_secondary_plan = true;
    view.session.cwd = std::path::PathBuf::from("/tmp/isolated-preview-session");
    let body = "# Secondary plan\n\nKeep the primary plan separate.\n".to_string();
    view.latest_inline_plan_content = Some(body.clone());
    if let Some(mut viewer) =
        LineViewerState::open_markdown_content("secondary-plan.md", body, None)
    {
        viewer.kind = LineViewerKind::PlanPreview;
        viewer.title_override = Some("secondary-plan.md".to_string());
        view.line_viewer = Some(viewer);
    }
    view
}

fn footer_revise(view: &mut AgentView) -> bool {
    // Same branch the Isolated Preview footer already takes for Revise.
    // Exclusive hard-plan view is a sibling contract and is not this path.
    let outcome = if view.plan_approval_view.is_some() {
        view.click_plan_cta(SelectedPlanCta::Revise)
    } else if view.isolated_preview_shows_secondary_plan {
        view.park_local_idle_plan_decision_if_needed();
        view.park_isolated_preview_revise_decision();
        let notes = view.isolated_preview_revise_notes_from_comments();
        let feedback = if notes.trim().is_empty() {
            None
        } else {
            Some(notes)
        };
        view.send_plan_feedback(feedback)
    } else {
        view.send_casual_plan_comments()
    };
    matches!(outcome, InputOutcome::Action(_))
}

fn feedback_text(view: &AgentView) -> String {
    view.isolated_preview_last_plan_feedback_for_test()
        .unwrap_or_default()
}

fn toast_blob(view: &AgentView) -> String {
    view.isolated_preview_toast_blob_for_test()
}

fn chrome_blob(view: &AgentView) -> String {
    view.isolated_preview_footer_chrome_for_test()
}

/// Operator: "let's see if revise works sensibly... crap... not working... no comments to send? nonsense."
///
/// Empty comments are not a reason to no-op. Isolated Preview Revise still
/// sends feedback that revises `secondary-plan.md`.
#[test]
fn isolated_preview_revise_no_comments_to_send_is_nonsense() {
    let mut view = docked_isolated_preview();
    view.plan_approval_view = None;
    view.clear_plan_comments_for_test();
    assert!(
        view.isolated_preview_revise_notes_from_comments()
            .is_empty(),
        "this contract is the empty-comment Revise, not a comment submit"
    );

    let sent = footer_revise(&mut view);
    let toasts = toast_blob(&view);
    let feedback = feedback_text(&view);

    assert!(
        sent,
        "Isolated Preview Revise must send feedback when there are no comments"
    );
    assert!(
        !toasts.contains("No comments to send"),
        "toasting \"No comments to send\" and stopping is the nonsense path; toasts were: {toasts}"
    );
    assert!(
        feedback.contains("secondary-plan.md"),
        "feedback must name secondary-plan.md, got: {feedback}"
    );
    assert_eq!(
        view.plan_feedback_in_flight_for_test(),
        Some(PlanFeedbackInFlight::Revising),
        "Revise parks Isolated Preview as revising"
    );
}

/// Operator: "I do like that this is a secondary plan, so it's not rewriting the primary plan."
///
/// Isolated Preview Revise rewrites `secondary-plan.md` only. Exclusive
/// parked `/plan` still updates `plan.md`. The secondary path must not.
#[test]
fn isolated_preview_revise_rewrites_secondary_plan_not_primary() {
    let mut secondary = docked_isolated_preview();
    secondary.plan_approval_view = None;
    secondary.set_plan_comment_for_test("keep the secondary plan separate");
    assert!(footer_revise(&mut secondary));
    let secondary_feedback = feedback_text(&secondary);
    assert!(
        secondary_feedback.contains(SECONDARY_PLAN_IDENTITY)
            || secondary_feedback.contains("secondary-plan.md"),
        "Isolated Preview feedback must name the secondary plan, got: {secondary_feedback}"
    );
    assert!(
        secondary_feedback.contains("Update secondary-plan.md"),
        "Interject must say Update secondary-plan.md, got: {secondary_feedback}"
    );
    assert!(
        !secondary_feedback.contains("Update plan.md"),
        "secondary revise must not tell the implementer to update primary plan.md, got: {secondary_feedback}"
    );
    assert!(
        !secondary.isolated_preview_feedback_rewrites_primary_plan_md(),
        "secondary-plan.md must not rewrite primary plan.md"
    );

    let mut exclusive = make_agent();
    exclusive.isolated_preview_shows_secondary_plan = false;
    exclusive.arm_exclusive_parked_plan_for_test();
    let notes = "tighten the primary plan".to_string();
    assert!(matches!(
        exclusive.send_plan_feedback(Some(notes)),
        InputOutcome::Action(_)
    ));
    let primary_feedback = feedback_text(&exclusive);
    assert!(
        primary_feedback.contains("Update plan.md"),
        "exclusive parked /plan still updates plan.md, got: {primary_feedback}"
    );
    assert!(
        !primary_feedback.contains("Update secondary-plan.md"),
        "exclusive parked /plan must not retarget secondary-plan.md, got: {primary_feedback}"
    );
}

/// Operator: "btw, this is better... but there's no indication that there's work being done to make the plan complete"
///
/// While Isolated Preview completes or revises, the footer is still-working
/// / rewriting-wait chrome. Idle approve, comment, revise, and exit are not
/// that chrome.
#[test]
fn isolated_preview_no_indication_work_is_being_done_to_make_the_plan_complete() {
    let mut view = docked_isolated_preview();
    view.plan_approval_view = None;

    view.enter_isolated_preview_rewrite_wait(PlanFeedbackInFlight::Updating);
    let updating = chrome_blob(&view);
    assert!(
        updating.contains(PLAN_REWRITE_WAIT_STATUS),
        "completing the secondary plan must show rewriting-wait, got: {updating}"
    );
    assert!(
        !view.isolated_preview_idle_cta_row_visible_for_test(),
        "idle approve|comment|revise|exit must hide while the secondary plan is updating"
    );
    assert!(
        !updating.contains("approve|comment|revise|exit"),
        "idle CTA chrome is not still-working chrome, got: {updating}"
    );

    view.enter_isolated_preview_rewrite_wait(PlanFeedbackInFlight::Revising);
    let revising = chrome_blob(&view);
    assert_eq!(
        view.plan_feedback_in_flight_for_test(),
        Some(PlanFeedbackInFlight::Revising)
    );
    assert!(
        revising.contains(PLAN_REWRITE_WAIT_STATUS),
        "revise must show rewriting-wait, not a quiet idle footer, got: {revising}"
    );
    assert!(
        !view.isolated_preview_idle_cta_row_visible_for_test(),
        "idle approve|comment|revise|exit must hide while Revise is in flight"
    );

    view.clear_plan_feedback_in_flight_for_test();
    assert!(
        view.isolated_preview_idle_cta_row_visible_for_test(),
        "after the rewrite finishes, idle CTAs may return; they are not the working chrome"
    );
    let idle = chrome_blob(&view);
    assert!(
        !idle.contains(PLAN_REWRITE_WAIT_STATUS),
        "idle footer must not keep rewriting-wait chrome, got: {idle}"
    );
}

/// Operator: "let's see if screenshots work here... nice, it seems that they do!"
///
/// A screenshot paste in Isolated Preview stays an image chip. It must not
/// fall through into line-viewer search.
#[test]
fn isolated_preview_screenshots_work_here_stay_image_chip() {
    let mut view = docked_isolated_preview();
    assert!(
        view.plan_overlay_owns_composer_paste(),
        "Isolated Preview owns composer paste, including screenshot paste"
    );
    let pasted = view.paste_screenshot_into_isolated_preview_for_test();
    assert!(
        pasted.stayed_image_chip,
        "screenshot paste must stay an image chip"
    );
    assert!(
        !pasted.fell_through_to_line_viewer_search,
        "screenshot paste must not become line-viewer search"
    );
    assert!(
        view.isolated_preview_composer_has_image_chip_for_test(),
        "the composer keeps the image chip after the paste"
    );
}

/// Operator: a click on the session directory opens the system file manager,
/// with a brief toast.
///
/// Cwd text is not a dead label. The open uses the same path the session
/// is in, and the toast says the directory was opened.
#[test]
fn isolated_preview_cwd_directory_click_opens_file_manager_with_toast() {
    let mut view = docked_isolated_preview();
    let cwd = view.session.cwd.clone();
    view.click_isolated_preview_cwd_for_test();
    let opened = crate::app::mouse::last_opened_path_for_test();
    assert_eq!(
        opened.as_deref(),
        Some(cwd.as_path()),
        "cwd click must open the session directory in the file manager"
    );
    let toasts = toast_blob(&view);
    assert!(
        toasts.contains("Opened the session directory."),
        "cwd click must toast that the session directory opened, got: {toasts}"
    );
}
