//! Embedded plan-validator (`validate-plan.py` parity, in-process).
//!
//! Host execute-plan skill still teaches
//! `python3 …/execute-plan/scripts/validate-plan.py <design-doc>`.
//! The bash tool **intercepts** those known invocations and runs this
//! module instead of spawning Python.
//!
//! Exit codes: 0 = valid, 1 = validation errors, 2 = usage / I/O errors.
//! Output (stdout): a JSON report (host parity).

mod dag;
mod intercept;
mod parse;

pub use dag::{compute_levels, linearize, validate_dag};
pub use intercept::{PlanValidateIntercept, try_parse_plan_validate_intercept};
pub use parse::{PrEntry, parse_pr_plan};

/// Exit codes matching host `validate-plan.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    Validation = 1,
    UsageIo = 2,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Result of an in-process handler (stdout/stderr/exit like a process).
#[derive(Debug, Clone)]
pub struct HandlerResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl HandlerResult {
    fn json_out(value: serde_json::Value, exit: ExitCode) -> Self {
        let stdout = format!(
            "{}\n",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| {
                r#"{"valid":false,"errors":["Internal error: failed to serialize report"]}"#
                    .to_owned()
            })
        );
        Self {
            stdout,
            stderr: String::new(),
            exit_code: exit.as_i32(),
        }
    }
}

/// Resolve `doc_path` against the bash tool working directory (shell parity).
///
/// Relative design-doc paths must open under session/project `cwd`, not the
/// product process cwd — same reason `implement_memory::execute_intercept`
/// takes `&cwd`.
pub fn resolve_doc_path(doc_path: &std::path::Path, cwd: &std::path::Path) -> std::path::PathBuf {
    if doc_path.is_absolute() {
        doc_path.to_path_buf()
    } else {
        cwd.join(doc_path)
    }
}

/// Run an intercepted invocation in-process.
///
/// `cwd` is the bash tool working directory (`TerminalRunRequest.working_directory`).
pub fn execute_intercept(
    intercept: &PlanValidateIntercept,
    cwd: &std::path::Path,
) -> HandlerResult {
    let path = resolve_doc_path(&intercept.doc_path, cwd);
    validate_path(&path)
}

/// Validate a design document path (host CLI parity).
pub fn validate_path(path: &std::path::Path) -> HandlerResult {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return HandlerResult::json_out(
                serde_json::json!({
                    "valid": false,
                    "errors": [format!("File not found: {}", path.display())],
                }),
                ExitCode::UsageIo,
            );
        }
        Err(e) => {
            return HandlerResult::json_out(
                serde_json::json!({
                    "valid": false,
                    "errors": [format!("Cannot read file {}: {}", path.display(), e)],
                }),
                ExitCode::UsageIo,
            );
        }
    };
    validate_content(&content)
}

/// Validate document body (no I/O).
pub fn validate_content(content: &str) -> HandlerResult {
    match parse_pr_plan(content) {
        Err(parse_errors) => HandlerResult::json_out(
            serde_json::json!({
                "valid": false,
                "errors": parse_errors,
            }),
            ExitCode::Validation,
        ),
        Ok(entries) => {
            let errors = validate_dag(&entries);
            if !errors.is_empty() {
                return HandlerResult::json_out(
                    serde_json::json!({
                        "valid": false,
                        "errors": errors,
                    }),
                    ExitCode::Validation,
                );
            }
            let levels = compute_levels(&entries);
            let order = linearize(&entries, &levels);
            let num_levels = levels.values().copied().max().map(|m| m + 1).unwrap_or(0);
            let mut counts: std::collections::HashMap<u32, usize> =
                std::collections::HashMap::new();
            for lv in levels.values() {
                *counts.entry(*lv).or_default() += 1;
            }
            let max_parallelism = counts.values().copied().max().unwrap_or(0);
            let level_assignments: serde_json::Map<String, serde_json::Value> = order
                .iter()
                .map(|pid| {
                    (
                        pid.clone(),
                        serde_json::json!(levels.get(pid).copied().unwrap_or(0)),
                    )
                })
                .collect();
            HandlerResult::json_out(
                serde_json::json!({
                    "valid": true,
                    "pr_count": entries.len(),
                    "levels": num_levels,
                    "max_parallelism": max_parallelism,
                    "linearized_order": order,
                    "level_assignments": level_assignments,
                    "errors": [],
                }),
                ExitCode::Success,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn intercept_known_validate_plan_py() {
        let cmd =
            "python3 /home/u/.agents/skills/execute-plan/scripts/validate-plan.py /tmp/design.md";
        let hit = try_parse_plan_validate_intercept(cmd).expect("should intercept");
        assert_eq!(hit.doc_path, std::path::PathBuf::from("/tmp/design.md"));
        assert!(
            hit.script_path
                .ends_with("execute-plan/scripts/validate-plan.py")
        );
    }

    #[test]
    fn intercept_bundled_mirror() {
        let cmd =
            "python3 /home/u/.grok/bundled/skills/execute-plan/scripts/validate-plan.py ./plan.md";
        assert!(try_parse_plan_validate_intercept(cmd).is_some());
    }

    #[test]
    fn unknown_python_does_not_intercept() {
        assert!(try_parse_plan_validate_intercept("python3 foo.py /tmp/x.md").is_none());
        assert!(try_parse_plan_validate_intercept("python3 -c 'print(1)'").is_none());
        assert!(
            try_parse_plan_validate_intercept("python3 /proj/validate-plan.py /tmp/x.md").is_none()
        );
        assert!(try_parse_plan_validate_intercept("ls -la").is_none());
    }

    #[test]
    fn missing_file_usage_exit() {
        let r = validate_path(std::path::Path::new("/no/such/design-doc-xyz.md"));
        assert_eq!(r.exit_code, ExitCode::UsageIo.as_i32());
        assert!(r.stdout.contains("File not found"));
    }

    #[test]
    fn valid_plan_json_report() {
        let doc = r#"
# Design

## PR Plan

### PR 0: Foundation
- **Files/components affected:** a.rs
- **Dependencies:** None
- **Description:** base

### PR 1: Feature
- **Files/components affected:** b.rs
- **Dependencies:** PR 0
- **Description:** builds on foundation
"#;
        let r = validate_content(doc);
        assert_eq!(r.exit_code, 0, "stdout={}", r.stdout);
        let v: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(v["pr_count"], 2);
        assert_eq!(v["levels"], 2);
        assert_eq!(v["max_parallelism"], 1);
        assert_eq!(v["linearized_order"], serde_json::json!(["pr-0", "pr-1"]));
    }

    #[test]
    fn cycle_is_validation_error() {
        let doc = r#"
## PR Plan

### PR 0: A
- **Dependencies:** PR 1
- **Description:** a

### PR 1: B
- **Dependencies:** PR 0
- **Description:** b
"#;
        let r = validate_content(doc);
        assert_eq!(r.exit_code, ExitCode::Validation.as_i32());
        assert!(
            r.stdout.contains("Cycle")
                || r.stdout.contains("cycle")
                || r.stdout.contains("valid\": false")
        );
        let v: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
        assert_eq!(v["valid"], false);
        assert!(!v["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn missing_pr_plan_section() {
        let r = validate_content("# No plan here\n");
        assert_eq!(r.exit_code, ExitCode::Validation.as_i32());
        assert!(r.stdout.contains("PR Plan"));
    }

    #[test]
    fn execute_intercept_reads_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmp,
            r#"
## PR Plan

### PR 1: Solo
- **Dependencies:** None
- **Description:** only
"#
        )
        .unwrap();
        let hit = PlanValidateIntercept {
            script_path: "execute-plan/scripts/validate-plan.py".into(),
            doc_path: tmp.path().to_path_buf(),
        };
        let r = execute_intercept(&hit, std::path::Path::new("/tmp"));
        assert_eq!(r.exit_code, 0, "{}", r.stdout);
        let v: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
        assert_eq!(v["pr_count"], 1);
    }

    #[test]
    fn relative_doc_path_resolves_against_bash_cwd_not_process_cwd() {
        // Put the design doc in a temp dir; set process cwd elsewhere so a
        // bare relative open would fail without bash-cwd join.
        let dir = tempfile::tempdir().unwrap();
        let doc_path = dir.path().join("design.md");
        std::fs::write(
            &doc_path,
            r#"
## PR Plan

### PR 1: Solo
- **Dependencies:** None
- **Description:** only
"#,
        )
        .unwrap();

        let other = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(other.path()).unwrap();

        let hit = PlanValidateIntercept {
            script_path: "execute-plan/scripts/validate-plan.py".into(),
            doc_path: std::path::PathBuf::from("design.md"),
        };
        // Without bash cwd join this would miss the file under process cwd.
        let wrong = validate_path(std::path::Path::new("design.md"));
        assert_eq!(
            wrong.exit_code,
            ExitCode::UsageIo.as_i32(),
            "process-cwd open must not find the doc"
        );

        let r = execute_intercept(&hit, dir.path());
        std::env::set_current_dir(prev).unwrap();

        assert_eq!(r.exit_code, 0, "stdout={}", r.stdout);
        let v: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(v["pr_count"], 1);

        assert_eq!(
            resolve_doc_path(std::path::Path::new("design.md"), dir.path()),
            dir.path().join("design.md")
        );
        assert_eq!(
            resolve_doc_path(std::path::Path::new("/abs/x.md"), dir.path()),
            std::path::PathBuf::from("/abs/x.md")
        );
    }
}
