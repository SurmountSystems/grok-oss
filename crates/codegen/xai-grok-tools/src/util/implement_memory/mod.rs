//! Embedded implement-skill memory manager (`memory.py` parity, in-process).
//!
//! Host skill scripts still teach `python3 …/implement/scripts/memory.py …`
//! via `run_terminal_command`. The bash tool **intercepts** those known
//! invocations and runs this module instead of spawning Python.
//!
//! Subcommands: `path` | `read` | `snapshot` | `update` (JSON merge on stdin).
//! User-project `python3 foo.py` is never intercepted.

mod intercept;
mod markdown;
mod merge;
mod ops;
mod workspace;

pub use intercept::{
    MemoryIntercept, MemorySubcommand, UpdateStdinSource, try_parse_memory_intercept,
};
pub use markdown::{MemoryState, parse_memory_file, render_memory_file};
pub use merge::merge_run;
pub use ops::{HandlerResult, run_subcommand};
pub use workspace::{MEMORY_DIR_NAME, canonicalize_remote, memory_paths, workspace_id_for_cwd};

/// Exit codes matching host `memory.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    Io = 1,
    WorkspaceId = 2,
    LockTimeout = 3,
    Spec = 4,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Error with `memory.py`-compatible exit code and stderr-style message.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    WorkspaceId(String),
    #[error("{0}")]
    LockTimeout(String),
    #[error("{0}")]
    Spec(String),
}

impl MemoryError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Io(_) => ExitCode::Io,
            Self::WorkspaceId(_) => ExitCode::WorkspaceId,
            Self::LockTimeout(_) => ExitCode::LockTimeout,
            Self::Spec(_) => ExitCode::Spec,
        }
    }

    /// Format like host `memory.py: …` on stderr.
    pub fn stderr_line(&self) -> String {
        format!("memory.py: {self}")
    }
}

/// Run an intercepted invocation in-process. Returns stdout body + exit code.
///
/// Errors are formatted as a single stderr-style line (host parity); stdout
/// is empty on failure.
pub fn execute_intercept(
    intercept: &MemoryIntercept,
    cwd: &std::path::Path,
    home: Option<&std::path::Path>,
) -> HandlerResult {
    let stdin = match &intercept.subcommand {
        MemorySubcommand::Update { stdin } => match stdin {
            UpdateStdinSource::Empty => String::new(),
            UpdateStdinSource::Literal(s) => s.clone(),
            UpdateStdinSource::FromFile(path) => match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    return HandlerResult {
                        stdout: String::new(),
                        stderr: format!("memory.py: I/O error: {e}"),
                        exit_code: ExitCode::Io.as_i32(),
                    };
                }
            },
        },
        _ => String::new(),
    };
    run_subcommand(&intercept.subcommand, cwd, home, &stdin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn intercept_known_memory_py_snapshot() {
        let cmd = "python3 /home/u/.agents/skills/implement/scripts/memory.py snapshot";
        let hit = try_parse_memory_intercept(cmd).expect("should intercept");
        assert!(matches!(hit.subcommand, MemorySubcommand::Snapshot));
        assert!(hit.script_path.ends_with("implement/scripts/memory.py"));
    }

    #[test]
    fn intercept_known_memory_py_path_and_read() {
        let base = "/home/u/.grok/bundled/skills/implement/scripts/memory.py";
        for sub in ["path", "read", "snapshot"] {
            let cmd = format!("python3 {base} {sub}");
            let hit = try_parse_memory_intercept(&cmd).expect(sub);
            match sub {
                "path" => assert!(matches!(hit.subcommand, MemorySubcommand::Path)),
                "read" => assert!(matches!(hit.subcommand, MemorySubcommand::Read)),
                "snapshot" => assert!(matches!(hit.subcommand, MemorySubcommand::Snapshot)),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn intercept_python_without_3() {
        let cmd = "python /opt/skills/implement/scripts/memory.py path";
        assert!(try_parse_memory_intercept(cmd).is_some());
    }

    #[test]
    fn intercept_update_with_file_redirect() {
        let cmd = "python3 /x/implement/scripts/memory.py update < /tmp/grok-mem.json";
        let hit = try_parse_memory_intercept(cmd).expect("update redirect");
        match hit.subcommand {
            MemorySubcommand::Update {
                stdin: UpdateStdinSource::FromFile(p),
            } => assert_eq!(p, PathBuf::from("/tmp/grok-mem.json")),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn intercept_update_with_echo_pipe() {
        let cmd = r#"echo '{}' | python3 /x/implement/scripts/memory.py update"#;
        let hit = try_parse_memory_intercept(cmd).expect("echo pipe");
        match hit.subcommand {
            MemorySubcommand::Update {
                stdin: UpdateStdinSource::Literal(s),
            } => assert_eq!(s, "{}"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn unknown_python_does_not_intercept() {
        assert!(try_parse_memory_intercept("python3 foo.py snapshot").is_none());
        assert!(try_parse_memory_intercept("python3 ./scripts/memory.py snapshot").is_none());
        assert!(try_parse_memory_intercept("python3 /proj/memory.py path").is_none());
        assert!(try_parse_memory_intercept("ls -la").is_none());
        assert!(try_parse_memory_intercept("python3 -c 'print(1)'").is_none());
    }

    #[test]
    fn user_project_memory_py_not_intercepted() {
        // basename memory.py alone is not enough — must be implement skill path
        assert!(
            try_parse_memory_intercept("python3 /home/u/myproject/memory.py snapshot").is_none()
        );
    }

    #[test]
    fn canonicalize_remote_collapses_ssh_https() {
        let a = canonicalize_remote("git@github.com:Owner/Repo.git");
        let b = canonicalize_remote("https://github.com/owner/repo");
        let c = canonicalize_remote("ssh://git@github.com/Owner/Repo.git");
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, "github.com/owner/repo");
    }

    #[test]
    fn parse_render_round_trip() {
        let md = "\
# Implementation Review Patterns\n\
\n\
> note\n\
\n\
## Common Issues\n\
\n\
### Testing\n\
- Missing edge-case tests (seen 2 times)\n\
\n\
## Recent Runs\n\
\n\
### 2026-07-25 — \"Add intercept\"\n\
- **Rounds**: 1\n\
";
        let state = parse_memory_file(md);
        assert_eq!(state.common_issues.get("Testing").map(|e| e.len()), Some(1));
        assert_eq!(state.common_issues["Testing"][0].count, 2);
        assert_eq!(state.recent_runs.len(), 1);
        let out = render_memory_file(&state);
        let again = parse_memory_file(&out);
        assert_eq!(
            again.common_issues["Testing"][0].description,
            "Missing edge-case tests"
        );
        assert_eq!(again.recent_runs[0].date, "2026-07-25");
    }

    #[test]
    fn snapshot_missing_file_exists_false() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        // Force a cwd-based workspace under a non-git empty dir.
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&cwd).unwrap();
        let hit = MemoryIntercept {
            script_path: "/x/implement/scripts/memory.py".into(),
            subcommand: MemorySubcommand::Snapshot,
        };
        let res = execute_intercept(&hit, &cwd, Some(home));
        assert_eq!(res.exit_code, 0, "stderr={}", res.stderr);
        let v: serde_json::Value = serde_json::from_str(&res.stdout).unwrap();
        assert_eq!(v["exists"], false);
        assert!(v["common_issues"].as_array().unwrap().is_empty());
    }

    #[test]
    fn path_and_read_ops() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&cwd).unwrap();

        let path_hit = MemoryIntercept {
            script_path: "/x/implement/scripts/memory.py".into(),
            subcommand: MemorySubcommand::Path,
        };
        let path_res = execute_intercept(&path_hit, &cwd, Some(home));
        assert_eq!(path_res.exit_code, 0);
        let mem_path = path_res.stdout.trim().to_string();
        assert!(mem_path.contains("implement-memory"));
        assert!(mem_path.ends_with(".md"));

        // create file
        std::fs::create_dir_all(std::path::Path::new(&mem_path).parent().unwrap()).unwrap();
        std::fs::write(&mem_path, "# hi\n").unwrap();

        let read_hit = MemoryIntercept {
            script_path: "/x/implement/scripts/memory.py".into(),
            subcommand: MemorySubcommand::Read,
        };
        let read_res = execute_intercept(&read_hit, &cwd, Some(home));
        assert_eq!(read_res.exit_code, 0);
        assert_eq!(read_res.stdout, "# hi\n");
    }

    #[test]
    fn update_empty_spec_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&cwd).unwrap();

        let hit = MemoryIntercept {
            script_path: "/x/implement/scripts/memory.py".into(),
            subcommand: MemorySubcommand::Update {
                stdin: UpdateStdinSource::Literal("{}".into()),
            },
        };
        let res = execute_intercept(&hit, &cwd, Some(home));
        assert_eq!(res.exit_code, 0, "stderr={}", res.stderr);
        let v: serde_json::Value = serde_json::from_str(&res.stdout).unwrap();
        assert_eq!(v["existed_before"], false);
        let file = v["file"].as_str().unwrap();
        assert!(std::path::Path::new(file).exists());
    }
}
