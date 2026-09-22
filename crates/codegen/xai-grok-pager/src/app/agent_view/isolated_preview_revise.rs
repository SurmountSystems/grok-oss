//! Isolated Preview Revise: rewrite secondary-plan.md with still-working chrome.

use super::PlanFeedbackInFlight;
#[cfg(test)]
use crate::views::plan_approval_view::PLAN_REWRITE_WAIT_STATUS;

impl super::AgentView {
    pub(crate) fn park_isolated_preview_revise_decision(&mut self) {
        self.enter_isolated_preview_rewrite_wait(PlanFeedbackInFlight::Revising);
    }

    pub(crate) fn plan_comment_draft(&self) -> String {
        self.plan_comments
            .iter()
            .map(|comment| comment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn isolated_preview_revise_notes_from_comments(&self) -> String {
        self.plan_comment_draft()
    }

    #[cfg(test)]
    pub(crate) fn isolated_preview_last_plan_feedback_for_test(&self) -> Option<String> {
        self.last_isolated_preview_plan_feedback.clone()
    }

    #[cfg(test)]
    pub(crate) fn isolated_preview_toast_blob_for_test(&self) -> String {
        self.toast
            .as_ref()
            .map(|(message, _)| message.clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn isolated_preview_footer_chrome_for_test(&self) -> String {
        let mut blob = String::new();
        if self.plan_feedback_in_flight.is_some() {
            blob.push_str(PLAN_REWRITE_WAIT_STATUS);
        }
        if self.isolated_preview_idle_cta_row_visible_for_test() {
            blob.push_str("approve|comment|revise|exit");
        }
        blob
    }

    #[cfg(test)]
    pub(crate) fn plan_feedback_in_flight_for_test(&self) -> Option<PlanFeedbackInFlight> {
        self.plan_feedback_in_flight
    }

    #[cfg(test)]
    pub(crate) fn isolated_preview_feedback_rewrites_primary_plan_md(&self) -> bool {
        !self.isolated_preview_shows_secondary_plan
    }

    #[cfg(test)]
    pub(crate) fn arm_exclusive_parked_plan_for_test(&mut self) {
        self.isolated_preview_shows_secondary_plan = false;
        self.plan_mode_active = true;
        self.plan_mode_pending = None;
        self.plan_decision_resolved = false;
        self.plan_feedback_in_flight = None;
        self.latest_inline_plan_content =
            Some("# Primary plan\n\nTighten the steps.\n".to_string());
        self.park_local_idle_plan_decision_if_needed();
    }

    #[cfg(test)]
    pub(crate) fn isolated_preview_idle_cta_row_visible_for_test(&self) -> bool {
        self.plan_feedback_in_flight.is_none()
    }

    #[cfg(test)]
    pub(crate) fn clear_plan_feedback_in_flight_for_test(&mut self) {
        self.plan_feedback_in_flight = None;
        self.isolated_preview_rewrite_wait_prompt = None;
    }

    #[cfg(test)]
    pub(crate) fn paste_screenshot_into_isolated_preview_for_test(
        &mut self,
    ) -> IsolatedPreviewPasteChip {
        IsolatedPreviewPasteChip {
            stayed_image_chip: self.plan_overlay_owns_composer_paste(),
            fell_through_to_line_viewer_search: !self.plan_overlay_owns_composer_paste(),
        }
    }

    #[cfg(test)]
    pub(crate) fn isolated_preview_composer_has_image_chip_for_test(&self) -> bool {
        self.plan_overlay_owns_composer_paste()
    }

    #[cfg(test)]
    pub(crate) fn click_isolated_preview_cwd_for_test(&mut self) {
        let cwd = self.session.cwd.clone();
        self.open_path(&cwd);
    }

    #[cfg(test)]
    pub(crate) fn clear_plan_comments_for_test(&mut self) {
        self.plan_comments.clear();
    }

    #[cfg(test)]
    pub(crate) fn set_plan_comment_for_test(&mut self, text: &str) {
        self.plan_comments.clear();
        self.plan_comments.push(super::PlanComment {
            id: self.plan_next_comment_id,
            line_range: 1..2,
            text: text.to_string(),
        });
    }
}

#[cfg(test)]
pub(crate) struct IsolatedPreviewPasteChip {
    pub stayed_image_chip: bool,
    pub fell_through_to_line_viewer_search: bool,
}
