//! Markdown parse/render for implement-memory files.

use indexmap::IndexMap;
use regex::Regex;
use std::sync::LazyLock;

static SECTION_COMMON_ISSUES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^##\s+Common Issues\s*$").expect("section re"));
static SECTION_RECENT_RUNS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^##\s+Recent Runs\s*$").expect("section re"));
static CATEGORY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^###\s+(.+?)\s*$").expect("category re"));
static ISSUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-\s+(.+?)\s+\(seen\s+(\d+)\s+times?\)\s*$").expect("issue re"));
static RUN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^###\s+(\d{4}-\d{2}-\d{2})\s*[\u{2014}\u{2013}-]\s*(.+?)\s*$").expect("run re")
});

/// Default header lines (mirrored in implement SKILL.md).
pub const DEFAULT_HEADER: &[&str] = &[
    "# Implementation Review Patterns",
    "",
    "> This file is maintained by the /implement skill.",
    "> It records common issues found during implementation reviews to help avoid them in future runs.",
    "> Shared across all working directories that resolve to the same workspace id.",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueEntry {
    pub description: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentRun {
    pub date: String,
    pub description: String,
    pub body_lines: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryState {
    pub header: Vec<String>,
    /// Category → entries (insertion order preserved).
    pub common_issues: IndexMap<String, Vec<IssueEntry>>,
    pub recent_runs: Vec<RecentRun>,
}

fn drop_trailing_blank(lines: &[String]) -> Vec<String> {
    let mut end = lines.len();
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    lines[..end].to_vec()
}

pub fn parse_memory_file(content: &str) -> MemoryState {
    let mut state = MemoryState::default();
    if content.trim().is_empty() {
        return state;
    }

    enum Section {
        Header,
        CommonIssues,
        RecentRuns,
    }
    let mut section = Section::Header;
    let mut current_category: Option<String> = None;
    let mut current_run: Option<RecentRun> = None;

    for line in content.lines() {
        if SECTION_COMMON_ISSUES.is_match(line) {
            if let Some(run) = current_run.take() {
                state.recent_runs.push(run);
            }
            section = Section::CommonIssues;
            current_category = None;
            continue;
        }
        if SECTION_RECENT_RUNS.is_match(line) {
            if let Some(run) = current_run.take() {
                state.recent_runs.push(run);
            }
            section = Section::RecentRuns;
            current_category = None;
            continue;
        }

        match section {
            Section::Header => {
                state.header.push(line.to_string());
            }
            Section::CommonIssues => {
                if let Some(caps) = CATEGORY_RE.captures(line) {
                    let cat = caps[1].trim().to_string();
                    state.common_issues.entry(cat.clone()).or_default();
                    current_category = Some(cat);
                    continue;
                }
                if let Some(caps) = ISSUE_RE.captures(line) {
                    if let Some(cat) = current_category.as_ref() {
                        let count: u64 = caps[2].parse().unwrap_or(0);
                        state
                            .common_issues
                            .entry(cat.clone())
                            .or_default()
                            .push(IssueEntry {
                                description: caps[1].trim().to_string(),
                                count,
                            });
                    }
                }
            }
            Section::RecentRuns => {
                if let Some(caps) = RUN_RE.captures(line) {
                    if let Some(run) = current_run.take() {
                        state.recent_runs.push(run);
                    }
                    current_run = Some(RecentRun {
                        date: caps[1].to_string(),
                        description: caps[2].trim().to_string(),
                        body_lines: Vec::new(),
                    });
                    continue;
                }
                if let Some(run) = current_run.as_mut() {
                    run.body_lines.push(line.to_string());
                }
            }
        }
    }
    if let Some(run) = current_run.take() {
        state.recent_runs.push(run);
    }

    state.header = drop_trailing_blank(&state.header);
    for run in &mut state.recent_runs {
        run.body_lines = drop_trailing_blank(&run.body_lines);
    }
    state
}

pub fn render_memory_file(state: &MemoryState) -> String {
    let mut out: Vec<String> = Vec::new();

    if state.header.is_empty() {
        for line in DEFAULT_HEADER {
            out.push((*line).to_string());
        }
    } else {
        out.extend(drop_trailing_blank(&state.header));
    }
    out.push(String::new());

    out.push("## Common Issues".into());
    out.push(String::new());
    let has_any = state.common_issues.values().any(|e| !e.is_empty());
    if !has_any {
        out.push("_No patterns recorded yet._".into());
        out.push(String::new());
    } else {
        for (category, entries) in &state.common_issues {
            if entries.is_empty() {
                continue;
            }
            out.push(format!("### {category}"));
            for e in entries {
                let times = if e.count == 1 { "time" } else { "times" };
                out.push(format!("- {} (seen {} {times})", e.description, e.count));
            }
            out.push(String::new());
        }
    }

    out.push("## Recent Runs".into());
    out.push(String::new());
    for run in &state.recent_runs {
        out.push(format!("### {} — {}", run.date, run.description));
        out.extend(drop_trailing_blank(&run.body_lines));
        out.push(String::new());
    }

    let mut s = out.join("\n");
    while s.ends_with('\n') {
        s.pop();
    }
    s.push('\n');
    s
}
