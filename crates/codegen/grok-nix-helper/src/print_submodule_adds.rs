//! Print `git submodule add` argv lines for the Surmount superproject.
//!
//! Prints argv only. Does not run `git submodule add`. Does not run
//! `git commit`. First children: grok-build, specs, majestic, surmount-server.
//! Humans sign: git commit -S.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// First four children of the `~/Projects/surmount/surmount` superproject.
pub const FIRST_CHILDREN: &[&str] = &["grok-build", "specs", "majestic", "surmount-server"];

pub fn parse_parent(args: &[String]) -> Result<PathBuf, String> {
    let mut parent = PathBuf::from(".");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--parent" => {
                let p = args
                    .get(i + 1)
                    .ok_or_else(|| "print-submodule-adds: --parent needs a path".to_string())?;
                parent = PathBuf::from(p);
                i += 2;
            }
            other => {
                return Err(format!("print-submodule-adds: unknown argument {other}"));
            }
        }
    }
    Ok(parent)
}

/// One argv vector per child: `git submodule add ../NAME NAME`.
pub fn submodule_add_argv(name: &str) -> Vec<String> {
    vec![
        "git".to_string(),
        "submodule".to_string(),
        "add".to_string(),
        format!("../{name}"),
        name.to_string(),
    ]
}

/// Children to print: the first four, then any extra git dirs *inside*
/// `--parent` (not every sibling under `~/Projects/surmount`).
pub fn children_to_print(parent: &Path) -> Vec<String> {
    let mut names: Vec<String> = FIRST_CHILDREN.iter().map(|s| (*s).to_string()).collect();
    if let Ok(rd) = fs::read_dir(parent) {
        let mut extras = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            if names.iter().any(|n| n == name) {
                continue;
            }
            if p.join(".git").exists() {
                extras.push(name.to_string());
            }
        }
        extras.sort();
        names.extend(extras);
    }
    names
}

pub fn submodule_add_lines(parent: &Path) -> Vec<String> {
    children_to_print(parent)
        .iter()
        .map(|name| submodule_add_argv(name).join(" "))
        .collect()
}

pub fn run(args: &[String]) -> ExitCode {
    let parent = match parse_parent(args) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            let _ = writeln!(
                io::stderr(),
                "Usage: grok-nix-helper print-submodule-adds [--parent PATH]"
            );
            return ExitCode::from(2);
        }
    };
    for line in submodule_add_lines(&parent) {
        let _ = writeln!(io::stdout(), "{line}");
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_submodule_adds_mentions_grok_build_specs_surmount_server() {
        let parent = Path::new("/tmp/surmount-superproject");
        let blob = submodule_add_lines(parent).join("\n");
        assert!(blob.contains("grok-build"), "{blob}");
        assert!(blob.contains("specs"), "{blob}");
        assert!(blob.contains("surmount-server"), "{blob}");
        for name in FIRST_CHILDREN {
            assert!(
                blob.contains(&format!("git submodule add ../{name} {name}")),
                "missing add line for {name}: {blob}"
            );
        }
    }

    #[test]
    fn print_submodule_adds_never_contains_commit() {
        let lines = submodule_add_lines(Path::new("."));
        for line in &lines {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(tokens.first().copied(), Some("git"));
            assert_eq!(tokens.get(1).copied(), Some("submodule"));
            assert_eq!(tokens.get(2).copied(), Some("add"));
            assert!(
                !tokens.iter().any(|t| *t == "commit" || *t == "push"),
                "subcommand must not contain commit: {line}"
            );
        }
        for name in FIRST_CHILDREN {
            let argv = submodule_add_argv(name);
            assert!(!argv.iter().any(|t| t == "commit"));
        }
    }

    #[test]
    fn parse_parent_flag() {
        let p = parse_parent(&["--parent".into(), "/tmp/surmount".into()]).unwrap();
        assert_eq!(p, PathBuf::from("/tmp/surmount"));
    }

    #[test]
    fn print_submodule_adds_includes_git_dir_inside_parent() {
        let dir = std::env::temp_dir().join(format!(
            "surmount-super-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let extra = dir.join("extra-child");
        fs::create_dir_all(extra.join(".git")).expect("temp parent git dir");
        let blob = submodule_add_lines(&dir).join("\n");
        let _ = fs::remove_dir_all(&dir);
        assert!(blob.contains("grok-build"), "{blob}");
        assert!(blob.contains("extra-child"), "{blob}");
        assert!(
            blob.contains("git submodule add ../extra-child extra-child"),
            "{blob}"
        );
    }
}
