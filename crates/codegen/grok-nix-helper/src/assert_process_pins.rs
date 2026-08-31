//! Assert Surmount process-pin paths are present in the worktree (or a git tree).
//!
//! Fails with a missing list. Does not modify git.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Required files: silent absence after recon is a process bug.
pub const REQUIRED_FILES: &[&str] = &[
    "AGENTS.md",
    "FORK.md",
    "RESIDUAL.md",
    "README.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "justfile",
    "flake.nix",
    "flake.lock",
    "docs/upstream-history.md",
    "docs/upstream-import-log.md",
    "docs/upstream-onto-log.md",
    "docs/git-workflow.md",
    "doc/dev/upstream-regression-filters.md",
    "crates/codegen/grok-nix-helper/src/put_history.rs",
    "crates/codegen/grok-nix-helper/src/import_upstream.rs",
    "crates/codegen/grok-nix-helper/src/join_main.rs",
    "crates/codegen/grok-nix-helper/src/recon_status.rs",
    "crates/codegen/grok-nix-helper/src/detect_upstream.rs",
    "crates/codegen/grok-nix-helper/src/fork_paths.rs",
    "crates/codegen/grok-nix-helper/src/git_cmd.rs",
    "crates/codegen/grok-nix-helper/src/extract_debug.rs",
    "crates/codegen/grok-nix-helper/src/generate_announcements.rs",
    "crates/codegen/grok-nix-helper/src/sync_upstream.rs",
    "flake/grok-nix-helper.nix",
    "crates/codegen/grok-nix-helper/Cargo.toml",
    "crates/codegen/grok-nix-helper/Cargo.lock",
    "crates/codegen/grok-nix-helper/src/main.rs",
    ".github/workflows/upstream-export.yml",
    ".github/workflows/ci.yml",
];

/// Required directories (at least one tracked blob under path, or dir in worktree).
pub const REQUIRED_DIRS: &[&str] = &[
    "packaging",
    "crates/codegen/grok-rate-limit",
    "crates/codegen/grok-nix-helper",
    "flake",
    "doc/dev",
    "docs/dev",
    ".grok/workflows",
    ".agents/skills",
];

pub const LAND_CLASS_MARKERS: &[&str] = &[
    "### 1. CLI identity",
    "### 2. Config is a surface",
    "### 3. grok-oss SQL extras",
    "### 4. DOGE / Surmount chrome",
    "### 5. Dual-auth hop after included SuperGrok period limits are full",
    "### 6. Last-session on start",
    "### 7. Product skills are not a Python runtime",
];

#[derive(Debug, Default)]
pub struct PinReport {
    pub missing: Vec<String>,
    pub warn: Vec<String>,
}

fn path_in_tree(tree_ish: &str, p: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-e", &format!("{tree_ish}:{p}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn dir_in_tree(tree_ish: &str, p: &str) -> bool {
    let out = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", tree_ish, "--", p])
        .output();
    match out {
        Ok(o) if o.status.success() => !String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        _ => false,
    }
}

fn git_show(tree_ish: &str, p: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["show", &format!("{tree_ish}:{p}")])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

fn tree_is_valid(tree_ish: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", &format!("{tree_ish}^{{tree}}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn catalog_markers(body: &str, missing: &mut Vec<String>) {
    for marker in LAND_CLASS_MARKERS {
        if !body.contains(marker) {
            missing.push(format!(
                "doc/dev/upstream-regression-filters.md (missing land class title: {marker})"
            ));
        }
    }
}

fn allowed_product_skill_py(rest: &str) -> bool {
    matches!(
        rest,
        "implement/scripts/memory.py"
            | "execute-plan/scripts/validate-plan.py"
            | "shared/resume-session/session_reader.py"
    ) || ((rest.starts_with("docx/")
        || rest.starts_with("pptx/")
        || rest.starts_with("xlsx/")
        || rest.starts_with("pdf/"))
        && rest.ends_with(".py"))
}

fn walk_py(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            walk_py(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("py") {
            out.push(p);
        }
    }
}

fn dir_has_file(dir: &Path) -> bool {
    fn walk(dir: &Path) -> bool {
        let Ok(rd) = fs::read_dir(dir) else {
            return false;
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_file() {
                return true;
            }
            if p.is_dir() && walk(&p) {
                return true;
            }
        }
        false
    }
    walk(dir)
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

/// Worktree check used by tests (does not spawn git).
pub fn check_worktree(root: &Path, strict: bool) -> PinReport {
    let mut report = PinReport::default();
    for f in REQUIRED_FILES {
        if !root.join(f).is_file() {
            report.missing.push((*f).to_string());
        }
    }
    for d in REQUIRED_DIRS {
        let p = root.join(d);
        if !p.is_dir() {
            report.missing.push(format!("{d}/ (absent)"));
        } else if strict && !dir_has_file(&p) {
            report.missing.push(format!("{d}/ (empty, STRICT=1)"));
        }
    }

    let catalog = root.join("doc/dev/upstream-regression-filters.md");
    if catalog.is_file()
        && let Ok(body) = fs::read_to_string(&catalog)
    {
        catalog_markers(&body, &mut report.missing);
    }

    let agents = root.join("AGENTS.md");
    if agents.is_file()
        && let Ok(body) = fs::read_to_string(&agents)
    {
        if !body.contains("parent is coordinator") {
            report
                .warn
                .push("AGENTS.md present but missing expected 'parent is coordinator' pin".into());
        }
        if !body.contains("stay-supergrok") {
            report.missing.push(
                "AGENTS.md (must name stay-supergrok fail-open / named /limits commands)".into(),
            );
        }
    }

    let fork = root.join("FORK.md");
    if fork.is_file()
        && let Ok(body) = fs::read_to_string(&fork)
    {
        if !contains_ci(&body, "upstream")
            && !contains_ci(&body, "import")
            && !contains_ci(&body, "onto")
        {
            report.warn.push(
                "FORK.md present but no upstream/import/onto mention (odd for this fork)".into(),
            );
        }
        if !body.contains("non-excepted Python") {
            report.missing.push(
                "FORK.md land class 7 (must say a restack that installs non-excepted Python is a failed land)".into(),
            );
        }
        if !body.contains("stay-supergrok") {
            report
                .missing
                .push("FORK.md (must name stay-supergrok)".into());
        }
        if !body.contains("limits_pins.json") {
            report
                .missing
                .push("FORK.md (must name limits_pins.json sidecar)".into());
        }
    }

    let readme = root.join("README.md");
    if readme.is_file()
        && let Ok(body) = fs::read_to_string(&readme)
        && !contains_ci(&body, "Grok OSS")
        && !contains_ci(&body, "grok-oss")
    {
        report
            .warn
            .push("README.md present but missing Grok OSS branding (possible xAI clobber)".into());
    }

    let guide = root.join("crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md");
    if guide.is_file()
        && let Ok(body) = fs::read_to_string(&guide)
        && !body.contains("not a Python runtime")
    {
        report.missing.push(format!(
            "{} (must say product skills are not a Python runtime)",
            guide.strip_prefix(root).unwrap_or(&guide).display()
        ));
    }

    for root_name in [".agents/skills", ".grok/skills"] {
        let dir = root.join(root_name);
        if !dir.is_dir() {
            continue;
        }
        let mut pys = Vec::new();
        walk_py(&dir, &mut pys);
        for py in pys {
            let rel = py.strip_prefix(root).unwrap_or(&py);
            let rel_s = rel.to_string_lossy().replace('\\', "/");
            let rest = rel_s
                .strip_prefix(".agents/skills/")
                .or_else(|| rel_s.strip_prefix(".grok/skills/"))
                .unwrap_or(&rel_s);
            if !allowed_product_skill_py(rest) {
                report.missing.push(format!(
                    "{rel_s} (non-excepted Python under a product skill root)"
                ));
            }
        }
    }

    report
}

fn check_tree(tree_ish: &str) -> Result<PinReport, String> {
    if !tree_is_valid(tree_ish) {
        return Err(format!("error: not a valid tree-ish: {tree_ish}"));
    }
    let mut report = PinReport::default();
    for f in REQUIRED_FILES {
        if !path_in_tree(tree_ish, f) {
            report.missing.push((*f).to_string());
        }
    }
    for d in REQUIRED_DIRS {
        if !dir_in_tree(tree_ish, d) {
            report.missing.push(format!("{d}/ (empty or absent)"));
        }
    }
    if path_in_tree(tree_ish, "doc/dev/upstream-regression-filters.md")
        && let Some(body) = git_show(tree_ish, "doc/dev/upstream-regression-filters.md")
    {
        catalog_markers(&body, &mut report.missing);
    }
    Ok(report)
}

fn find_repo_root() -> PathBuf {
    let mut p = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if p.join("flake.nix").is_file() && p.join("AGENTS.md").is_file() {
            return p;
        }
        if !p.pop() {
            break;
        }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn take_flag_value(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    if i + 1 >= args.len() {
        args.remove(i);
        return None;
    }
    let v = args[i + 1].clone();
    args.drain(i..=i + 1);
    Some(v)
}

pub fn run(raw: &[String]) -> ExitCode {
    let mut args: Vec<String> = raw.to_vec();
    let root = take_flag_value(&mut args, "--root")
        .map(PathBuf::from)
        .unwrap_or_else(find_repo_root);
    let strict = args.iter().any(|a| a == "--strict") || env::var("STRICT").as_deref() == Ok("1");
    args.retain(|a| a != "--strict");

    let tree_ish = env::var("TREE_ISH")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| args.first().cloned().filter(|s| !s.is_empty()));

    if let Some(tree) = tree_ish {
        match check_tree(&tree) {
            Err(e) => {
                let _ = writeln!(io::stderr(), "{e}");
                return ExitCode::from(2);
            }
            Ok(report) => {
                let _ = writeln!(io::stdout(), "assert-process-pins: checking tree {tree}");
                return finish(report);
            }
        }
    }

    let _ = writeln!(
        io::stdout(),
        "assert-process-pins: checking worktree at {}",
        root.display()
    );
    finish(check_worktree(&root, strict))
}

fn finish(report: PinReport) -> ExitCode {
    if !report.warn.is_empty() {
        let _ = writeln!(io::stderr(), "WARN:");
        for w in &report.warn {
            let _ = writeln!(io::stderr(), "  - {w}");
        }
    }
    if !report.missing.is_empty() {
        let _ = writeln!(
            io::stderr(),
            "FAIL: process-pin paths missing ({}):",
            report.missing.len()
        );
        for m in &report.missing {
            let _ = writeln!(io::stderr(), "  - {m}");
        }
        let _ = writeln!(io::stderr());
        let _ = writeln!(
            io::stderr(),
            "After import: ensure paths are in FORK_PATHS (grok-nix-helper import-upstream-export)."
        );
        let _ = writeln!(
            io::stderr(),
            "After onto: re-apply from origin/main or cherry-pick the product commits that added them."
        );
        let _ = writeln!(
            io::stderr(),
            "Research: doc/dev/research/fork-paths-hardening-2026-07-24.md"
        );
        return ExitCode::from(1);
    }
    let _ = writeln!(
        io::stdout(),
        "OK: all required process-pin paths present ({} files + {} dirs).",
        REQUIRED_FILES.len(),
        REQUIRED_DIRS.len()
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_files_do_not_list_replaced_shell_helpers() {
        for f in REQUIRED_FILES {
            assert!(
                !f.ends_with("with-ci-hermetic-path.sh"),
                "replaced hermetic PATH script still listed: {f}"
            );
            assert!(
                !f.ends_with("assert-process-pins.sh"),
                "replaced assert script still listed: {f}"
            );
            assert!(
                !f.ends_with("ensure-working-nix-path.sh"),
                "replaced ensure-nix script still listed: {f}"
            );
            assert!(
                !f.ends_with("nix-current-system.sh"),
                "replaced current-system script still listed: {f}"
            );
        }
        assert!(REQUIRED_FILES.contains(&"crates/codegen/grok-nix-helper/Cargo.toml"));
        assert!(REQUIRED_FILES.contains(&"flake/grok-nix-helper.nix"));
        assert!(REQUIRED_FILES.contains(&"crates/codegen/grok-nix-helper/src/join_main.rs"));
        assert!(REQUIRED_FILES.contains(&"crates/codegen/grok-nix-helper/src/put_history.rs"));
        assert!(REQUIRED_FILES.contains(&"crates/codegen/grok-nix-helper/src/import_upstream.rs"));
        assert!(REQUIRED_FILES.contains(&"crates/codegen/grok-nix-helper/src/extract_debug.rs"));
        assert!(
            REQUIRED_FILES
                .contains(&"crates/codegen/grok-nix-helper/src/generate_announcements.rs")
        );
        assert!(REQUIRED_FILES.contains(&"crates/codegen/grok-nix-helper/src/sync_upstream.rs"));
        assert!(REQUIRED_FILES.contains(&"crates/codegen/grok-nix-helper/src/git_cmd.rs"));
        assert!(REQUIRED_DIRS.contains(&"crates/codegen/grok-nix-helper"));
        assert!(REQUIRED_DIRS.contains(&"flake"));
        for f in REQUIRED_FILES {
            assert!(
                !f.starts_with("scripts/") || !f.ends_with(".sh"),
                "recon shell must not remain a required pin: {f}"
            );
        }
    }

    #[test]
    fn missing_fork_md_fails_loud() {
        let tmp = env::temp_dir().join(format!("grok-nix-helper-assert-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let report = check_worktree(&tmp, false);
        assert!(
            report.missing.iter().any(|m| m == "FORK.md"),
            "missing list must name FORK.md, got {:?}",
            report.missing
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn allowed_python_exceptions() {
        assert!(allowed_product_skill_py("implement/scripts/memory.py"));
        assert!(allowed_product_skill_py("docx/foo.py"));
        assert!(!allowed_product_skill_py("implement/oops.py"));
    }

    #[test]
    fn land_class_markers_are_the_seven_titles() {
        assert_eq!(LAND_CLASS_MARKERS.len(), 7);
        assert!(LAND_CLASS_MARKERS[0].contains("CLI identity"));
        assert!(LAND_CLASS_MARKERS[6].contains("not a Python runtime"));
    }
}
