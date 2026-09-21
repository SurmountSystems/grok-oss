//! Soft process-rule reminders injected into nested spawn prompts.
//!
//! This diverges from SpaceXAI: upstream has no `/settings` process-rule
//! reminder list. Surmount injects configured strings as `<system-reminder>`
//! family text the same way write-path assignment does. Reminders are **soft**:
//! spawn still succeeds, a third implementor L2 still spawns, there is no
//! auto-kill, no deny, and no second permission system. Default list is empty
//! so nothing injects until the operator types reminders. "At most two
//! implementor L2s." / "only two implementor L2s allowed" is example help
//! copy, not a spawn reject and not `MAX_LIVE_L2S=2`.

use std::cell::RefCell;

/// Configured process-rule reminder lines (newline-separated, trimmed empties dropped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRuleReminders {
    pub enabled: bool,
    pub lines: Vec<String>,
}

thread_local! {
    static LIVE: RefCell<Option<ProcessRuleReminders>> = const { RefCell::new(None) };
}

impl ProcessRuleReminders {
    pub fn from_ui(enabled: bool, raw: &str) -> Self {
        let lines = if enabled {
            raw.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };
        Self { enabled, lines }
    }

    /// Immediate override after a settings toggle (disk persist is async).
    pub fn set_live(this: Self) {
        LIVE.with(|c| *c.borrow_mut() = Some(this));
    }

    /// Live cache, else disk `[ui]` keys, else default on + empty list.
    pub fn current() -> Self {
        if let Some(live) = LIVE.with(|c| c.borrow().clone()) {
            return live;
        }
        Self::from_disk()
    }

    fn from_disk() -> Self {
        let root = match xai_grok_config::load_effective_config_disk_only() {
            Ok(r) => r,
            Err(_) => return Self::from_ui(true, ""),
        };
        let ui = root.get("ui");
        let enabled = ui
            .and_then(|u| u.get("process_rule_reminders_enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let raw = ui
            .and_then(|u| u.get("process_rule_reminders"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Self::from_ui(enabled, raw)
    }

    /// Off or empty → no extra reminder text.
    pub fn as_injection(&self) -> Option<String> {
        if !self.enabled || self.lines.is_empty() {
            return None;
        }
        Some(format_process_rule_reminder(&self.lines))
    }

    /// Drop the thread-local override so later tests read disk / default.
    pub fn clear_live() {
        LIVE.with(|c| *c.borrow_mut() = None);
    }
}

/// Prepend configured process-rule reminder text to nested spawn prompt
/// or parent spawn chrome. Off or empty list returns `text` unchanged
/// (upstream-like). Spawn still succeeds; this is not a deny.
pub fn with_process_rule_spawn_reminder(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    match ProcessRuleReminders::current().as_injection() {
        Some(inj) => format!("{inj}\n\n{text}"),
        None => text.to_string(),
    }
}

/// `<system-reminder>` family body for configured process-rule strings.
pub fn format_process_rule_reminder(lines: &[String]) -> String {
    let body = lines.join("\n");
    format!(
        "<system-reminder>\n\
         These are soft process-rule reminders from /settings. They do not deny spawn, \
         do not auto-kill nested work, and are not a second permission system. \
         Keep such reminders soft for now.\n\
         {body}\n\
         </system-reminder>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_rule_reminders_off_injects_no_extra_reminder_text() {
        let cfg = ProcessRuleReminders::from_ui(false, "only two implementor L2s allowed");
        assert_eq!(cfg.as_injection(), None);
    }

    #[test]
    fn empty_list_injects_nothing_when_enabled() {
        let cfg = ProcessRuleReminders::from_ui(true, "  \n\n");
        assert_eq!(cfg.as_injection(), None);
    }

    #[test]
    fn process_rule_reminder_text_appears_when_enabled() {
        let cfg = ProcessRuleReminders::from_ui(true, "At most two implementor L2s.");
        let text = cfg.as_injection().expect("non-empty enabled list injects");
        assert!(text.contains("<system-reminder>"));
        assert!(text.contains("At most two implementor L2s."));
        assert!(text.contains("Keep such reminders soft for now"));
        assert!(
            !text.contains("MAX_LIVE_L2S"),
            "must not bake a launch cap into reminder text"
        );
    }

    #[test]
    fn example_only_two_string_is_not_a_spawn_reject() {
        // Quote: keep such reminders soft for now; "only two implementor L2s allowed"
        // is EXAMPLE STRING not spawn reject.
        let cfg = ProcessRuleReminders::from_ui(true, "only two implementor L2s allowed");
        let text = cfg
            .as_injection()
            .expect("example string still injects as text");
        assert!(text.contains("only two implementor L2s allowed"));
        assert!(text.contains("Keep such reminders soft for now"));
    }

    #[test]
    fn process_rule_reminder_text_in_nested_spawn_prompt() {
        ProcessRuleReminders::clear_live();
        ProcessRuleReminders::set_live(ProcessRuleReminders::from_ui(
            true,
            "At most two implementor L2s.",
        ));
        let out = with_process_rule_spawn_reminder("implement slice I");
        ProcessRuleReminders::clear_live();
        assert!(
            out.contains("<system-reminder>"),
            "nested spawn prompt must carry reminder chrome: {out}"
        );
        assert!(out.contains("At most two implementor L2s."));
        assert!(out.contains("implement slice I"));
        assert!(out.contains("Keep such reminders soft for now"));
    }

    #[test]
    fn process_rule_reminders_off_nested_spawn_has_no_extra_reminder_text() {
        ProcessRuleReminders::clear_live();
        ProcessRuleReminders::set_live(ProcessRuleReminders::from_ui(
            false,
            "only two implementor L2s allowed",
        ));
        let out = with_process_rule_spawn_reminder("implement slice I");
        ProcessRuleReminders::clear_live();
        assert_eq!(out, "implement slice I");
        assert!(
            !out.contains("<system-reminder>"),
            "settings off must not add reminder tags"
        );
        assert!(
            !out.contains("only two implementor L2s allowed"),
            "settings off must not add the configured string"
        );
    }

    #[test]
    fn process_rule_reminder_configured_third_l2_still_spawns() {
        // "only two implementor L2s allowed" is example help copy, not a
        // spawn reject and not MAX_LIVE_L2S=2. The helper returns Ok text.
        ProcessRuleReminders::clear_live();
        ProcessRuleReminders::set_live(ProcessRuleReminders::from_ui(
            true,
            "only two implementor L2s allowed",
        ));
        let first = with_process_rule_spawn_reminder("implementor one");
        let second = with_process_rule_spawn_reminder("implementor two");
        let third = with_process_rule_spawn_reminder("implementor three");
        ProcessRuleReminders::clear_live();
        for (label, prompt) in [
            ("first", first.as_str()),
            ("second", second.as_str()),
            ("third", third.as_str()),
        ] {
            assert!(
                prompt.contains("implementor"),
                "{label} spawn prompt must still exist: {prompt}"
            );
            assert!(
                prompt.contains("only two implementor L2s allowed"),
                "{label} spawn must inject the example string as text, not deny"
            );
            assert!(
                !prompt.contains("MAX_LIVE_L2S"),
                "{label} must not bake a launch cap into reminder text"
            );
        }
    }
}
