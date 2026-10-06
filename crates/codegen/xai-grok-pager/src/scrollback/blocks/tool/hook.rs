//! Tool-call hook rows and the compact counts painted on a verb-group header.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::theme::Theme;

/// When a hook batch ran relative to the tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookPhase {
    Pre,
    Post,
}

/// One hook invocation attached to a tool row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRunEntry {
    pub name: String,
    pub status: HookRunStatus,
    pub output: Option<String>,
}

/// Outcome of one hook. Skipped runs do not count in group labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookRunStatus {
    Success {
        elapsed: std::time::Duration,
    },
    Skipped,
    Blocked {
        detail: String,
        elapsed: std::time::Duration,
    },
    Failed {
        error: String,
        elapsed: std::time::Duration,
    },
}

/// Hooks recorded on one tool-call scrollback row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolCallHookData {
    pub pre_hooks: Vec<HookRunEntry>,
    pub post_hooks: Vec<HookRunEntry>,
}

/// Non-skipped hook outcomes across a verb-group run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HookRunCounts {
    ok: usize,
    blocked: usize,
    failed: usize,
}

impl HookRunCounts {
    pub fn add_data(&mut self, data: &ToolCallHookData) {
        for run in data.pre_hooks.iter().chain(data.post_hooks.iter()) {
            match &run.status {
                HookRunStatus::Success { .. } => self.ok += 1,
                HookRunStatus::Blocked { .. } => self.blocked += 1,
                HookRunStatus::Failed { .. } => self.failed += 1,
                HookRunStatus::Skipped => {}
            }
        }
    }

    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }

    pub fn is_empty(&self) -> bool {
        self.ok == 0 && self.blocked == 0 && self.failed == 0
    }
}

/// Compact suffix for one tool row: `  [hooks: blocked/failed]`.
pub fn render_hooks_inline_suffix(data: &ToolCallHookData) -> Option<Vec<Span<'static>>> {
    let mut counts = HookRunCounts::default();
    counts.add_data(data);
    if counts.blocked == 0 && counts.failed == 0 {
        return None;
    }
    Some(vec![Span::raw(format!(
        "  [hooks: {}/{}]",
        counts.blocked, counts.failed
    ))])
}

/// Stop-hook group line: `{name}  [hooks: blocked/failed]`.
pub fn render_stop_hooks_summary(
    groups: &[(String, Vec<HookRunEntry>)],
) -> Option<Vec<Span<'static>>> {
    if groups.is_empty() {
        return None;
    }
    let mut spans = Vec::new();
    for (index, (name, runs)) in groups.iter().enumerate() {
        let data = ToolCallHookData {
            post_hooks: runs.clone(),
            ..ToolCallHookData::default()
        };
        let mut counts = HookRunCounts::default();
        counts.add_data(&data);
        if index > 0 {
            spans.push(Span::raw(", "));
        }
        spans.push(Span::raw(format!(
            "{name}  [hooks: {}/{}]",
            counts.blocked, counts.failed
        )));
    }
    Some(spans)
}

/// Verb-group header suffix. Skipped hooks are omitted.
///
/// Shape: `  [hooks: 1 ok, 1 blocked, 1 failed]`.
pub fn render_group_hook_counts_inline_suffix(
    counts: &HookRunCounts,
    theme: &Theme,
) -> Vec<Span<'static>> {
    if counts.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<Span<'static>> = Vec::new();
    parts.push(Span::raw("  [hooks: "));
    let mut wrote = false;
    let push_sep = |parts: &mut Vec<Span<'static>>, wrote: &mut bool| {
        if *wrote {
            parts.push(Span::raw(", "));
        }
        *wrote = true;
    };
    if counts.ok > 0 {
        push_sep(&mut parts, &mut wrote);
        parts.push(Span::styled(
            format!("{} ok", counts.ok),
            Style::default()
                .fg(theme.accent_success)
                .add_modifier(Modifier::DIM),
        ));
    }
    if counts.blocked > 0 {
        push_sep(&mut parts, &mut wrote);
        parts.push(Span::styled(
            format!("{} blocked", counts.blocked),
            Style::default().fg(theme.accent_running),
        ));
    }
    if counts.failed > 0 {
        push_sep(&mut parts, &mut wrote);
        parts.push(Span::styled(
            format!("{} failed", counts.failed),
            Style::default().fg(theme.accent_error),
        ));
    }
    parts.push(Span::raw("]"));
    parts
}
