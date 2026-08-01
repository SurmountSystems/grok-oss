//! Merge update specs into memory state (host `memory.py` parity).

use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use super::MemoryError;
use super::markdown::{IssueEntry, MemoryState, RecentRun};

pub const MAX_PATTERNS_PER_CATEGORY: usize = 25;
pub const MAX_RECENT_RUNS: usize = 20;

const SEVERITY_ORDER: &[&str] = &["bug", "suggestion", "nit"];

static PUNCT_TAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[.;:,!?\s]+$").expect("punct re"));
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("ws re"));

pub fn sanitize_one_line(text: &str) -> String {
    let re = Regex::new(r"[\r\n\t]+").expect("nl re");
    re.replace_all(text, " ").trim().to_string()
}

pub fn normalize(text: &str) -> String {
    let text = text.to_lowercase();
    let text = text.trim();
    let text = PUNCT_TAIL_RE.replace_all(text, "");
    WHITESPACE_RE.replace_all(&text, " ").to_string()
}

fn require_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, MemoryError> {
    value.as_str().ok_or_else(|| {
        MemoryError::Spec(format!(
            "\"{field}\" must be a string, got {}",
            type_name(value)
        ))
    })
}

fn require_optional_str(value: Option<&Value>, field: &str) -> Result<String, MemoryError> {
    match value {
        None | Some(Value::Null) => Ok(String::new()),
        Some(v) => Ok(require_str(v, field)?.to_string()),
    }
}

fn require_list<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, MemoryError> {
    value.as_array().ok_or_else(|| {
        MemoryError::Spec(format!(
            "\"{field}\" must be a list, got {}",
            type_name(value)
        ))
    })
}

fn require_dict<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, MemoryError> {
    value.as_object().ok_or_else(|| {
        MemoryError::Spec(format!(
            "\"{field}\" must be an object, got {}",
            type_name(value)
        ))
    })
}

fn require_int(value: &Value, field: &str) -> Result<i64, MemoryError> {
    // JSON bool is not an integer for our purposes
    if value.is_boolean() {
        return Err(MemoryError::Spec(format!(
            "\"{field}\" must be an integer, got bool"
        )));
    }
    value.as_i64().ok_or_else(|| {
        MemoryError::Spec(format!(
            "\"{field}\" must be an integer, got {}",
            type_name(value)
        ))
    })
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

fn format_severity_summary(ibs: &HashMap<String, i64>) -> (String, i64) {
    let mut parts = Vec::new();
    let mut rendered_total = 0i64;
    let mut seen = HashSet::new();
    for sev in SEVERITY_ORDER {
        if let Some(&count) = ibs.get(*sev) {
            seen.insert((*sev).to_string());
            if count > 0 {
                let label = if count == 1 {
                    (*sev).to_string()
                } else {
                    format!("{sev}s")
                };
                parts.push(format!("{count} {label}"));
                rendered_total += count;
            }
        }
    }
    let mut extras: Vec<_> = ibs.keys().filter(|k| !seen.contains(*k)).cloned().collect();
    extras.sort();
    for sev in extras {
        let count = ibs[&sev];
        if count > 0 {
            let label = if count == 1 {
                sev.clone()
            } else {
                format!("{sev}s")
            };
            parts.push(format!("{count} {label}"));
            rendered_total += count;
        }
    }
    (parts.join(", "), rendered_total)
}

fn bump_or_append(
    entries: &mut Vec<IssueEntry>,
    lookup: &mut HashMap<String, usize>,
    description: String,
) -> bool {
    let norm = normalize(&description);
    if let Some(&idx) = lookup.get(&norm) {
        entries[idx].count += 1;
        return true;
    }
    let idx = entries.len();
    entries.push(IssueEntry {
        description,
        count: 1,
    });
    lookup.insert(norm, idx);
    false
}

/// Merge a run spec into `state` in place. Return summary stats as JSON value.
pub fn merge_run(
    state: &mut MemoryState,
    spec: &serde_json::Map<String, Value>,
) -> Result<serde_json::Value, MemoryError> {
    let mut new_patterns = 0u64;
    let mut merged_patterns = 0u64;
    let mut categories_capped: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut recent_runs_dropped = 0u64;
    let mut touched: HashSet<String> = HashSet::new();
    let mut category_lookups: HashMap<String, HashMap<String, usize>> = HashMap::new();

    if let Some(patterns_v) = spec.get("patterns") {
        if !patterns_v.is_null() {
            let patterns = require_list(patterns_v, "patterns")?;
            for (idx, raw) in patterns.iter().enumerate() {
                let field = format!("patterns[{idx}]");
                let raw = require_dict(raw, &field)?;

                let category = match raw.get("category") {
                    None | Some(Value::Null) => "Other".to_string(),
                    Some(Value::String(s)) if s.is_empty() => "Other".to_string(),
                    Some(v) => {
                        let c = sanitize_one_line(require_str(v, &format!("{field}.category"))?);
                        if c.is_empty() { "Other".to_string() } else { c }
                    }
                };

                let description = sanitize_one_line(&require_optional_str(
                    raw.get("description"),
                    &format!("{field}.description"),
                )?);
                if description.is_empty() {
                    continue;
                }

                let entries = state.common_issues.entry(category.clone()).or_default();
                let lookup = category_lookups.entry(category.clone()).or_insert_with(|| {
                    entries
                        .iter()
                        .enumerate()
                        .map(|(i, e)| (normalize(&e.description), i))
                        .collect()
                });
                // lookup indices may be stale after append — rebuild if lengths diverge
                if lookup.len() != entries.len() {
                    *lookup = entries
                        .iter()
                        .enumerate()
                        .map(|(i, e)| (normalize(&e.description), i))
                        .collect();
                }
                if bump_or_append(entries, lookup, description) {
                    merged_patterns += 1;
                } else {
                    new_patterns += 1;
                }
                touched.insert(category);
            }
        }
    }

    for (category, entries) in state.common_issues.iter_mut() {
        entries.sort_by(|a, b| {
            b.count.cmp(&a.count).then_with(|| {
                a.description
                    .to_lowercase()
                    .cmp(&b.description.to_lowercase())
            })
        });
        if entries.len() > MAX_PATTERNS_PER_CATEGORY {
            let dropped = entries.len() - MAX_PATTERNS_PER_CATEGORY;
            entries.truncate(MAX_PATTERNS_PER_CATEGORY);
            categories_capped.insert(category.clone(), Value::from(dropped as u64));
        }
    }

    if let Some(run_raw) = spec.get("run") {
        if !run_raw.is_null() {
            let run = require_dict(run_raw, "run")?;
            if !run.is_empty() {
                recent_runs_dropped = merge_recent_run(state, run)?;
            }
        }
    }

    let mut categories_touched: Vec<String> = touched.into_iter().collect();
    categories_touched.sort();

    Ok(serde_json::json!({
        "new_patterns": new_patterns,
        "merged_patterns": merged_patterns,
        "categories_touched": categories_touched,
        "categories_capped": categories_capped,
        "recent_runs_dropped": recent_runs_dropped,
    }))
}

fn merge_recent_run(
    state: &mut MemoryState,
    run: &serde_json::Map<String, Value>,
) -> Result<u64, MemoryError> {
    let date = match run.get("date") {
        None | Some(Value::Null) => chrono::Utc::now().format("%Y-%m-%d").to_string(),
        Some(Value::String(s)) if s.trim().is_empty() => {
            chrono::Utc::now().format("%Y-%m-%d").to_string()
        }
        Some(v) => {
            let date_str = require_str(v, "run.date")?.trim().to_string();
            if chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").is_err() {
                return Err(MemoryError::Spec(format!(
                    "\"run.date\" must be a calendar-valid YYYY-MM-DD date, got {date_str:?}"
                )));
            }
            date_str
        }
    };

    let mut description = sanitize_one_line(&require_optional_str(
        run.get("description"),
        "run.description",
    )?);
    if description.is_empty() {
        description = "(no description)".into();
    }
    description = description.replace('"', "");
    let description = description.trim();
    let description = if description.is_empty() {
        "(no description)".to_string()
    } else {
        description.to_string()
    };
    let description = format!("\"{description}\"");

    let mut body_lines: Vec<String> = Vec::new();

    if let Some(rounds_raw) = run.get("rounds") {
        if !rounds_raw.is_null() {
            let rounds = require_int(rounds_raw, "run.rounds")?;
            body_lines.push(format!("- **Rounds**: {rounds}"));
        }
    }

    if let Some(ibs_raw) = run.get("issues_by_severity") {
        if !ibs_raw.is_null() {
            let ibs = require_dict(ibs_raw, "run.issues_by_severity")?;
            let mut normalized: HashMap<String, i64> = HashMap::new();
            for (sev, count) in ibs {
                normalized.insert(
                    sev.clone(),
                    require_int(count, &format!("run.issues_by_severity[\"{sev}\"]"))?,
                );
            }
            if !normalized.is_empty() {
                let (summary, rendered_total) = format_severity_summary(&normalized);
                if !summary.is_empty() {
                    body_lines.push(format!("- **Issues**: {rendered_total} total ({summary})"));
                }
            }
        }
    }

    if let Some(kp_raw) = run.get("key_patterns") {
        if !kp_raw.is_null() {
            let key_patterns = require_list(kp_raw, "run.key_patterns")?;
            let mut cleaned = Vec::new();
            for (i, p) in key_patterns.iter().enumerate() {
                let s = sanitize_one_line(require_str(p, &format!("run.key_patterns[{i}]"))?);
                if !s.is_empty() {
                    cleaned.push(s);
                }
            }
            if !cleaned.is_empty() {
                body_lines.push(format!("- **Key patterns**: {}", cleaned.join(", ")));
            }
        }
    }

    if let Some(specs_raw) = run.get("specializations") {
        if !specs_raw.is_null() {
            let specs = require_list(specs_raw, "run.specializations")?;
            let mut cleaned = Vec::new();
            for (i, s) in specs.iter().enumerate() {
                let s = sanitize_one_line(require_str(s, &format!("run.specializations[{i}]"))?);
                if !s.is_empty() {
                    cleaned.push(s);
                }
            }
            if !cleaned.is_empty() {
                body_lines.push(format!(
                    "- **Specializations used**: {}",
                    cleaned.join(", ")
                ));
            }
        }
    }

    state.recent_runs.insert(
        0,
        RecentRun {
            date,
            description,
            body_lines,
        },
    );

    let mut dropped = 0u64;
    if state.recent_runs.len() > MAX_RECENT_RUNS {
        dropped = (state.recent_runs.len() - MAX_RECENT_RUNS) as u64;
        state.recent_runs.truncate(MAX_RECENT_RUNS);
    }
    Ok(dropped)
}
