//! Host-owned goal plan applied from Operator messages.
//!
//! Isolated `rustc --test` of this file is supported: no crate imports.

/// Reply text the Operator must never see from an empty give-up / replace.
pub(crate) const NO_CURRENT_GOALS: &str = "no current goals";

const GOAL_SLASH: &str = "/goal";

/// Durable goal list plus the one-shot post-plan retry latch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoalPlan {
    goals: Vec<String>,
    post_plan_retry_pending: bool,
    post_plan_retry_count: u32,
    extra: Vec<(String, String)>,
    /// Mill/todo tracker history for this goal's Progress. A new goal starts empty.
    progress_items: Vec<String>,
}

/// Result of applying a user message, giving up, or replacing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoalPlanReply {
    /// Host-turn text. Empty means no reply.
    pub text: String,
    /// True when [`GoalPlan::set_goals`] ran for this action.
    pub goals_set: bool,
    /// True when [`GoalPlan::post_plan_action`] should fire once after set.
    pub post_plan_retry: bool,
}

impl GoalPlan {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn goals(&self) -> &[String] {
        &self.goals
    }

    pub(crate) fn primary_goal(&self) -> Option<&str> {
        self.goals
            .iter()
            .map(String::as_str)
            .find(|goal| !goal.is_empty())
    }

    pub(crate) fn extra(&self) -> &[(String, String)] {
        &self.extra
    }

    pub(crate) fn progress_items(&self) -> &[String] {
        &self.progress_items
    }

    /// Record mill/todo tracker history for this in-flight goal's Progress.
    /// Creating a different new goal clears this list; same-prompt resume keeps it.
    pub(crate) fn record_progress_items(&mut self, items: Vec<String>) {
        self.progress_items = items;
    }

    pub(crate) fn post_plan_retry_count(&self) -> u32 {
        self.post_plan_retry_count
    }

    /// Replace the goal list. Arms the one-shot post-plan retry.
    /// A new goal must not carry prior mill/todo tracker history into Progress.
    pub(crate) fn set_goals(&mut self, goals: Vec<String>) {
        self.goals = goals
            .into_iter()
            .map(|goal| goal.trim().to_string())
            .filter(|goal| !goal.is_empty())
            .collect();
        self.post_plan_retry_pending = true;
        self.progress_items.clear();
    }

    /// After the planner writes PLAN_FILE, the current goal is the planned work.
    /// Does not arm post-plan retry: the plan already happened.
    pub(crate) fn adopt_planned_goal(&mut self, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        self.goals = vec![title.to_string()];
        self.post_plan_retry_pending = false;
    }

    /// Apply an Operator message. Goal-plan messages call [`Self::set_goals`].
    /// Other messages do not reply [`NO_CURRENT_GOALS`].
    /// The same prompt as the current goals does not reset or re-arm post-plan retry.
    pub(crate) fn apply_user_goal_message(&mut self, message: &str) -> GoalPlanReply {
        match parse_goal_plan_message(message) {
            Some(goals) => {
                let incoming: Vec<String> = goals
                    .into_iter()
                    .map(|goal| goal.trim().to_string())
                    .filter(|goal| !goal.is_empty())
                    .collect();
                if incoming.is_empty() || incoming == self.goals {
                    return empty_host_reply();
                }
                self.set_goals(incoming);
                let retry = self.post_plan_action();
                GoalPlanReply {
                    text: String::new(),
                    goals_set: true,
                    post_plan_retry: retry,
                }
            }
            None => empty_host_reply(),
        }
    }

    /// Abandon current goals. Empty and non-empty must not reply [`NO_CURRENT_GOALS`].
    pub(crate) fn give_up(&mut self) -> GoalPlanReply {
        self.goals.clear();
        self.post_plan_retry_pending = false;
        self.progress_items.clear();
        empty_host_reply()
    }

    /// Replace the goal list. Empty-on-empty must not reply [`NO_CURRENT_GOALS`].
    pub(crate) fn replace(&mut self, goals: Vec<String>) -> GoalPlanReply {
        let incoming: Vec<String> = goals
            .into_iter()
            .map(|goal| goal.trim().to_string())
            .filter(|goal| !goal.is_empty())
            .collect();
        if incoming.is_empty() && self.goals.is_empty() {
            return empty_host_reply();
        }
        if incoming.is_empty() {
            return self.give_up();
        }
        self.set_goals(incoming);
        let retry = self.post_plan_action();
        GoalPlanReply {
            text: String::new(),
            goals_set: true,
            post_plan_retry: retry,
        }
    }

    /// Fire the post-plan retry at most once after [`Self::set_goals`].
    pub(crate) fn post_plan_action(&mut self) -> bool {
        if !self.post_plan_retry_pending {
            return false;
        }
        self.post_plan_retry_pending = false;
        self.post_plan_retry_count = self.post_plan_retry_count.saturating_add(1);
        true
    }

    /// Session-update merge: other fields may change; goals stay.
    pub(crate) fn apply_session_update(&mut self, extra: Vec<(String, String)>) {
        self.extra = extra;
    }

    /// Conversation compaction must not drop the goal list.
    pub(crate) fn after_conversation_compaction(&mut self) {}

    pub(crate) fn snapshot(&self) -> GoalPlanSnapshot {
        GoalPlanSnapshot {
            goals: self.goals.clone(),
            post_plan_retry_pending: self.post_plan_retry_pending,
            post_plan_retry_count: self.post_plan_retry_count,
            extra: self.extra.clone(),
        }
    }

    pub(crate) fn restore(&mut self, snapshot: GoalPlanSnapshot) {
        self.goals = snapshot.goals;
        self.post_plan_retry_pending = snapshot.post_plan_retry_pending;
        self.post_plan_retry_count = snapshot.post_plan_retry_count;
        self.extra = snapshot.extra;
    }
}

/// Persistable GoalPlan. Survives session restore.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoalPlanSnapshot {
    pub goals: Vec<String>,
    pub post_plan_retry_pending: bool,
    pub post_plan_retry_count: u32,
    pub extra: Vec<(String, String)>,
}

fn parse_goal_plan_message(message: &str) -> Option<Vec<String>> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(inner) = extract_tagged(trimmed, "goal-plan") {
        let goals = inner
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        return (!goals.is_empty()).then_some(goals);
    }
    let rest = trimmed
        .strip_prefix(GOAL_SLASH)
        .or_else(|| trimmed.strip_prefix("/GOAL"))?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    if is_goal_subcommand(rest) {
        return None;
    }
    Some(vec![rest.to_string()])
}

fn is_goal_subcommand(rest: &str) -> bool {
    matches!(
        rest.split_whitespace().next().unwrap_or(""),
        "status" | "pause" | "resume" | "clear"
    ) && rest.split_whitespace().nth(1).is_none()
}

fn extract_tagged<'a>(message: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = message.find(&open)? + open.len();
    let end = message[start..].find(&close)? + start;
    Some(message[start..end].trim())
}

/// Empty host-turn reply. Must never contain [`NO_CURRENT_GOALS`].
fn empty_host_reply() -> GoalPlanReply {
    let reply = GoalPlanReply::default();
    debug_assert!(
        !reply_has_no_current_goals(&reply),
        "empty host reply must not contain {NO_CURRENT_GOALS:?}, got {:?}",
        reply.text
    );
    reply
}

fn reply_has_no_current_goals(reply: &GoalPlanReply) -> bool {
    reply.text.to_ascii_lowercase().contains(NO_CURRENT_GOALS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_plan_apply_sets_goals_from_user_message() {
        let mut plan = GoalPlan::new();
        let reply = plan.apply_user_goal_message("/goal ship the parser");
        assert!(
            reply.goals_set,
            "apply_user_goal_message must call set_goals"
        );
        assert_eq!(plan.goals(), &["ship the parser".to_string()]);
        assert_eq!(plan.primary_goal(), Some("ship the parser"));
        assert!(
            !reply_has_no_current_goals(&reply),
            "setting goals must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            reply.text
        );
    }

    #[test]
    fn test_goal_plan_give_up_empty_goals_does_not_reply_no_current_goals() {
        let mut plan = GoalPlan::new();
        let reply = plan.give_up();
        assert!(
            !reply_has_no_current_goals(&reply),
            "empty give_up must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            reply.text
        );
        assert!(plan.goals().is_empty());
        assert!(!reply.goals_set);
        assert!(!reply.post_plan_retry);
    }

    /// Operator 2026-10-21: Goals are still broken. `/goal` still replies
    /// "no current goals". Production builds a throwaway [`GoalPlan`] (no
    /// session store) for each `/goal` / give-up. That host line is forbidden.
    #[test]
    fn test_goal_slash_with_no_stored_plan_does_not_reply_no_current_goals() {
        let mut plan = GoalPlan::new();
        for message in [
            "/goal",
            "/goal ",
            "/GOAL",
            "/goal status",
            "/goal clear",
            "/goal pause",
            "/goal resume",
            "",
        ] {
            let reply = plan.apply_user_goal_message(message);
            assert!(
                !reply_has_no_current_goals(&reply),
                "Goals are still broken: /goal still replies no current goals. \
                 throwaway apply {message:?} must not reply {NO_CURRENT_GOALS:?}, got {:?}",
                reply.text
            );
        }
        let empty_give_up = plan.give_up();
        assert!(
            !reply_has_no_current_goals(&empty_give_up),
            "Goals are still broken: /goal still replies no current goals. \
             throwaway empty give_up must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            empty_give_up.text
        );
        let applied = plan.apply_user_goal_message("/goal ship the parser");
        assert!(applied.goals_set);
        let give_up = plan.give_up();
        assert!(
            !reply_has_no_current_goals(&give_up),
            "Goals are still broken: /goal still replies no current goals. \
             give_up after set_goals must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            give_up.text
        );
        assert!(plan.goals().is_empty());
        assert!(!give_up.goals_set);
        assert!(!give_up.post_plan_retry);
    }

    #[test]
    fn test_goal_plan_replace_empty_goals_does_not_reply_no_current_goals() {
        let mut plan = GoalPlan::new();
        let reply = plan.replace(Vec::new());
        assert!(
            !reply_has_no_current_goals(&reply),
            "empty replace must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            reply.text
        );
        assert!(plan.goals().is_empty());
        assert!(!reply.post_plan_retry);
    }

    #[test]
    fn test_goal_plan_user_message_without_goal_plan_does_not_reply_no_current_goals() {
        let mut plan = GoalPlan::new();
        let reply = plan.apply_user_goal_message("what is the weather in Austin");
        assert!(
            !reply.goals_set,
            "a plain user message is not a goal plan and must not call set_goals"
        );
        assert!(
            !reply_has_no_current_goals(&reply),
            "a plain user message must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            reply.text
        );
        assert!(plan.goals().is_empty());
        assert!(!reply.post_plan_retry);
    }

    #[test]
    fn test_goal_plan_empty_give_up_does_not_invoke_post_plan_retry() {
        let mut plan = GoalPlan::new();
        let reply = plan.give_up();
        assert!(
            !reply.post_plan_retry,
            "empty give_up must not arm post_plan retry"
        );
        assert!(
            !plan.post_plan_action(),
            "empty give_up must not leave a pending post_plan retry"
        );
        assert_eq!(plan.post_plan_retry_count(), 0);
    }

    #[test]
    fn test_goal_plan_post_plan_retry_fires_once_after_set_goals() {
        let mut plan = GoalPlan::new();
        let reply = plan.apply_user_goal_message("/goal land the restore path");
        assert!(reply.goals_set);
        assert!(
            reply.post_plan_retry,
            "post_plan_action must fire once after set_goals"
        );
        assert_eq!(plan.post_plan_retry_count(), 1);
        assert!(
            !plan.post_plan_action(),
            "post_plan retry is one-shot after set_goals"
        );
        assert_eq!(plan.post_plan_retry_count(), 1);
    }

    /// Operator 2026-10-21: "When I scrolled up and re-ran the goal prompt, goals are
    /// fixed, it's just that your fix also reset the earlier goal."
    #[test]
    fn test_goal_plan_same_prompt_does_not_reset_earlier_goal() {
        let mut plan = GoalPlan::new();
        let prompt = "/goal We need to update to Lean 4.35, in parallel, using our seven team mates and Hunter as a watcher.";
        let first = plan.apply_user_goal_message(prompt);
        assert!(first.goals_set);
        assert!(first.post_plan_retry);
        let progress = plan.goals().to_vec();
        assert!(
            !progress.is_empty(),
            "the earlier goal must be stored before the same prompt is re-run"
        );
        let retry_count = plan.post_plan_retry_count();
        let second = plan.apply_user_goal_message(prompt);
        assert!(
            !second.goals_set,
            "When I scrolled up and re-ran the goal prompt, goals are fixed, it's just that your fix also reset the earlier goal. \
             same prompt must not call set_goals"
        );
        assert!(
            !second.post_plan_retry,
            "same prompt must not re-arm post_plan retry (that restarts planning)"
        );
        assert_eq!(
            plan.goals(),
            progress.as_slice(),
            "same prompt must keep the earlier GoalPlan, not a new empty one"
        );
        assert_eq!(plan.post_plan_retry_count(), retry_count);
        assert!(
            !reply_has_no_current_goals(&second),
            "same-prompt resume must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            second.text
        );
    }

    /// Operator: A just-created goal (`Goal created just now`, 0 tokens) must not
    /// paint old mill Progress checkmarks (`+332 more` from a prior Lean 4.35 walk)
    /// as if they were this planning turn. Stale tracker history must not masquerade
    /// as current Progress.
    #[test]
    fn test_goal_plan_new_goal_does_not_carry_prior_mill_progress() {
        let mut plan = GoalPlan::new();
        let mill = "/goal We need to update to Lean 4.35, in parallel, using our seven team mates and Hunter as a watcher.";
        let first = plan.apply_user_goal_message(mill);
        assert!(first.goals_set);
        let mill_progress: Vec<String> = (0..347)
            .map(|i| format!("Lean 4.35 mill walk {i}"))
            .collect();
        plan.record_progress_items(mill_progress.clone());
        assert_eq!(plan.progress_items().len(), 347);

        let same = plan.apply_user_goal_message(mill);
        assert!(
            !same.goals_set,
            "same-prompt resume must not reset an in-flight matching goal"
        );
        assert_eq!(
            plan.progress_items(),
            mill_progress.as_slice(),
            "same-prompt resume must keep tracker progress"
        );

        let created = plan.apply_user_goal_message("/goal Plan this planning turn");
        assert!(created.goals_set);
        assert!(
            plan.progress_items().is_empty(),
            "A just-created goal (`Goal created just now`, 0 tokens) must not paint old mill Progress checkmarks (`+332 more` from a prior Lean 4.35 walk) as if they were this planning turn. Stale tracker history must not masquerade as current Progress. got {:?}",
            plan.progress_items()
        );
        assert_eq!(plan.primary_goal(), Some("Plan this planning turn"));
    }

    #[test]
    fn test_goal_plan_empty_goals_does_not_reply_no_current_goals() {
        let mut plan = GoalPlan::new();
        for reply in [
            plan.give_up(),
            plan.replace(Vec::new()),
            plan.apply_user_goal_message(""),
            plan.apply_user_goal_message("/goal status"),
        ] {
            assert!(
                !reply_has_no_current_goals(&reply),
                "empty goals must not reply {NO_CURRENT_GOALS:?}, got {:?}",
                reply.text
            );
        }
    }

    #[test]
    fn test_goal_plan_persists_across_session_update() {
        let mut plan = GoalPlan::new();
        plan.apply_user_goal_message("/goal keep this goal");
        plan.apply_session_update(vec![("cursor".into(), "42".into())]);
        assert_eq!(plan.goals(), &["keep this goal".to_string()]);
        assert_eq!(plan.extra(), &[("cursor".to_string(), "42".to_string())]);
    }

    #[test]
    fn test_goal_plan_persists_across_conversation_compaction() {
        let mut plan = GoalPlan::new();
        plan.apply_user_goal_message("/goal survive compact");
        plan.after_conversation_compaction();
        assert_eq!(plan.goals(), &["survive compact".to_string()]);
    }

    #[test]
    fn test_goal_plan_survives_session_restore() {
        let mut plan = GoalPlan::new();
        plan.apply_user_goal_message("/goal restore me");
        plan.apply_session_update(vec![("tab".into(), "1".into())]);
        let snapshot = plan.snapshot();
        let mut restored = GoalPlan::new();
        restored.restore(snapshot);
        assert_eq!(restored.goals(), &["restore me".to_string()]);
        assert_eq!(restored.primary_goal(), Some("restore me"));
    }

    #[test]
    fn test_goal_plan_not_lost_on_restore_when_other_fields_change() {
        let mut plan = GoalPlan::new();
        plan.apply_user_goal_message("/goal keep across restore");
        let snapshot = plan.snapshot();
        let mut restored = GoalPlan::new();
        restored.restore(snapshot);
        restored.apply_session_update(vec![
            ("model".into(), "grok-4.7".into()),
            ("cwd".into(), "/tmp".into()),
        ]);
        assert_eq!(restored.goals(), &["keep across restore".to_string()]);
        assert_eq!(restored.extra().len(), 2);
    }

    /// Operator 2026-10-21: the goal got lost. After Planned, the GoalPlan
    /// names the planned work, not the original complaint, and give_up still
    /// does not reply no current goals.
    #[test]
    fn test_goal_plan_adopt_planned_goal_names_the_work_without_arming_retry() {
        let mut plan = GoalPlan::new();
        let applied = plan.apply_user_goal_message(
            "/goal Goal planning is still entirely broken after the claimed fix.",
        );
        assert!(applied.goals_set);
        assert!(applied.post_plan_retry);
        plan.adopt_planned_goal("Rewrite tests onto production GoalPlan arms");
        assert_eq!(
            plan.primary_goal(),
            Some("Rewrite tests onto production GoalPlan arms"),
            "Operator: the goal got lost. After Planned, GoalPlan must name the current planned work"
        );
        assert!(
            !plan.post_plan_action(),
            "adopting a written plan must not arm another post-plan retry"
        );
        let give_up = plan.give_up();
        assert!(
            !reply_has_no_current_goals(&give_up),
            "give_up after adopt_planned_goal must not reply {NO_CURRENT_GOALS:?}, got {:?}",
            give_up.text
        );
    }
}
