//! Parse `## PR Plan` sections from design docs (host `validate-plan.py` parity).

use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrEntry {
    pub id: String,
    pub number: String,
    pub title: String,
    pub files: Vec<String>,
    pub dependencies: Vec<String>,
    pub description: String,
}

static FENCED_BACKTICK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)^\s*```[^\n]*\n.*?^\s*```\s*$").expect("fence regex"));
static FENCED_TILDE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)^\s*~~~[^\n]*\n.*?^\s*~~~\s*$").expect("tilde fence regex"));
static PR_PLAN_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^## PR Plan\s*$").expect("pr plan heading"));
static ANY_H2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^## ").expect("any h2"));
static PR_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^###\s+PR\s+(\S+?):\s*(.+)$").expect("pr heading"));

fn strip_fenced_code_blocks(content: &str) -> String {
    let s = FENCED_BACKTICK.replace_all(content, "");
    FENCED_TILDE.replace_all(&s, "").into_owned()
}

/// End of the PR Plan section: next `## ` heading that is not another `## PR Plan`.
fn next_section_start(rest: &str) -> Option<usize> {
    for m in ANY_H2.find_iter(rest) {
        let line_end = rest[m.start()..]
            .find('\n')
            .map(|i| m.start() + i)
            .unwrap_or(rest.len());
        let line = rest[m.start()..line_end].trim_end();
        if line == "## PR Plan" {
            continue;
        }
        return Some(m.start());
    }
    None
}

/// Parse the `## PR Plan` section. `Ok(entries)` or `Err(error messages)`.
pub fn parse_pr_plan(content: &str) -> Result<Vec<PrEntry>, Vec<String>> {
    let stripped = strip_fenced_code_blocks(content);
    let heading = match PR_PLAN_HEADING.find(&stripped) {
        Some(m) => m,
        None => return Err(vec!["No '## PR Plan' section found in the document".into()]),
    };
    let start = heading.end();
    let section = if let Some(end) = next_section_start(&stripped[start..]) {
        &stripped[start..start + end]
    } else {
        &stripped[start..]
    };

    let matches: Vec<_> = PR_HEADING.captures_iter(section).collect();
    if matches.is_empty() {
        return Err(vec!["No PR entries found in the PR Plan section".into()]);
    }

    let mut entries = Vec::new();
    let mut parse_errors = Vec::new();

    for (i, m) in matches.iter().enumerate() {
        let pr_num = m.get(1).map(|x| x.as_str()).unwrap_or("").to_owned();
        let title = m
            .get(2)
            .map(|x| x.as_str().trim().to_owned())
            .unwrap_or_default();
        let body_start = m.get(0).map(|x| x.end()).unwrap_or(0);
        let body_end = matches
            .get(i + 1)
            .and_then(|n| n.get(0))
            .map(|x| x.start())
            .unwrap_or(section.len());
        let body = &section[body_start..body_end];

        let files_raw = extract_field(body, "Files/components affected");
        let deps_raw = extract_field(body, "Dependencies");
        let description = extract_field(body, "Description").unwrap_or_default();

        let files = files_raw
            .as_deref()
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter(|f| !f.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();

        let (dependencies, dep_errors) =
            parse_dependencies(deps_raw.as_deref(), &format!("PR {pr_num}"));
        parse_errors.extend(dep_errors);

        entries.push(PrEntry {
            id: format!("pr-{}", pr_num.to_ascii_lowercase()),
            number: pr_num,
            title,
            files,
            dependencies,
            description,
        });
    }

    if !parse_errors.is_empty() {
        return Err(parse_errors);
    }
    Ok(entries)
}

fn extract_field(body: &str, field_name: &str) -> Option<String> {
    let escaped = regex::escape(field_name);
    // **Field:** val / **Field**: val / plain Field: val + indented continuations
    let pattern = format!(r"(?mi)^\s*[-*]\s+\**{escaped}:?\**:?\s*(.+(?:\n[ \t]+\S.*)*)");
    let re = Regex::new(&pattern).ok()?;
    let m = re.captures(body)?;
    let raw = m.get(1)?.as_str();
    let collapsed = Regex::new(r"\s*\n[ \t]+").ok()?.replace_all(raw, " ");
    Some(collapsed.trim().to_owned())
}

fn parse_dependencies(deps_raw: Option<&str>, pr_label: &str) -> (Vec<String>, Vec<String>) {
    let Some(deps_raw) = deps_raw else {
        return (vec![], vec![]);
    };
    let stripped = deps_raw.trim();
    if stripped.is_empty() || matches!(stripped.to_ascii_lowercase().as_str(), "none" | "n/a" | "-")
    {
        return (vec![], vec![]);
    }

    let mut deps = Vec::new();
    let mut errors = Vec::new();
    let dep_re = Regex::new(r"(?i)^PR\s+(\S+)").expect("dep re");
    for part in stripped.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(m) = dep_re.captures(part) {
            let id = m
                .get(1)
                .map(|x| x.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            deps.push(format!("pr-{id}"));
        } else {
            errors.push(format!(
                "Unrecognized dependency format '{part}' in {pr_label} (expected 'PR <id>')"
            ));
        }
    }
    (deps, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fenced_example_plans() {
        let doc = r#"
# Doc

```
## PR Plan
### PR 9: Fake
- **Dependencies:** None
```

## PR Plan

### PR 1: Real
- **Dependencies:** None
- **Description:** ok
"#;
        let entries = parse_pr_plan(doc).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "pr-1");
    }
}
