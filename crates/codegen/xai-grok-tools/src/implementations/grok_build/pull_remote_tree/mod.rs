//! `pull_remote_tree` copies a remote (or local stand-in) tree onto a local dest.
//!
//! Direction is source **from** (`HOST:SRC` or a local directory) to local dest
//! only. Copy is a Rust walk plus `std::fs`. OpenSSH may fetch a remote tree;
//! this tool does not shell out to rsync as the copy implementation.

use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::types::tool_metadata::{ToolMetadata, resolve_cwd, shared_resources};

/// Stable client-facing tool id. Not `rsync`.
pub const PULL_REMOTE_TREE_TOOL_NAME: &str = "pull_remote_tree";

/// Directory names skipped at every path component.
pub const EXCLUDED_DIR_NAMES: &[&str] = &[".git", "target", ".lake", "result"];

/// Input: source tree and local destination.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PullRemoteTreeInput {
    /// Remote tree to pull. `HOST:SRC` (OpenSSH) or a local directory that is
    /// not dest. Destination is always local.
    #[schemars(
        description = "Source tree: HOST:SRC (OpenSSH scp-style) or a local directory. Direction is from this source onto local dest only."
    )]
    pub from: String,
    /// Local destination directory. Must not look like `HOST:PATH`.
    #[schemars(
        description = "Local destination directory. Refused when it looks like HOST:PATH. Never a git commit or git push."
    )]
    pub dest: String,
}

/// Model-facing copy result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PullRemoteTreeOutput {
    pub message: String,
    pub files_copied: u64,
}

impl xai_tool_runtime::ToolOutput for PullRemoteTreeOutput {}

impl From<PullRemoteTreeInput> for ToolInput {
    fn from(input: PullRemoteTreeInput) -> Self {
        ToolInput::Dynamic(serde_json::json!({
            "from": input.from,
            "dest": input.dest,
        }))
    }
}

impl From<PullRemoteTreeOutput> for ToolOutput {
    fn from(o: PullRemoteTreeOutput) -> Self {
        ToolOutput::Text(TextOutput::from(o.message))
    }
}

/// Parsed source: local directory or OpenSSH `HOST:SRC`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullSource {
    Local(PathBuf),
    Remote { host: String, path: String },
}

/// Counts from a Rust `std::fs` walk copy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyReport {
    pub files_copied: u64,
    pub dirs_created: u64,
}

/// True when `raw` looks like scp `HOST:PATH` (including `user@host:path`).
pub fn looks_like_ssh_target(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() || s.contains("://") {
        return false;
    }
    if is_windows_drive_path(s) {
        return false;
    }
    let Some((left, right)) = s.split_once(':') else {
        return false;
    };
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if left.contains('/') || left.contains('\\') {
        return false;
    }
    true
}

fn is_windows_drive_path(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b.len() == 2 || b[2] == b'/' || b[2] == b'\\')
}

/// True when argv/path text would run `git commit` or `git push`.
pub fn would_git_commit_or_push(parts: &[&str]) -> bool {
    parts.iter().any(|p| text_has_git_commit_or_push(p))
}

fn text_has_git_commit_or_push(text: &str) -> bool {
    for stmt in text.split(['\n', ';', '|', '&']) {
        let tokens: Vec<&str> = stmt
            .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')))
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.windows(2).any(|w| {
            let git = w[0] == "git" || w[0].ends_with("/git");
            git && (w[1] == "commit" || w[1] == "push")
        }) {
            return true;
        }
    }
    false
}

fn is_excluded_name(name: &OsStr) -> bool {
    EXCLUDED_DIR_NAMES.iter().any(|e| name == *e)
}

fn path_has_excluded_component(path: &Path) -> bool {
    path.components().any(|c| match c {
        std::path::Component::Normal(n) => is_excluded_name(n),
        _ => false,
    })
}

/// Parse `from`. SSH-shaped values are remote; everything else is a local path.
pub fn parse_pull_source(from: &str) -> Result<PullSource, String> {
    let from = from.trim();
    if from.is_empty() {
        return Err("from must not be empty".into());
    }
    if looks_like_ssh_target(from) {
        let (host, path) = from
            .split_once(':')
            .ok_or_else(|| "from looks like HOST:SRC but has no colon".to_string())?;
        validate_ssh_host(host)?;
        validate_remote_path(path)?;
        return Ok(PullSource::Remote {
            host: host.to_owned(),
            path: path.to_owned(),
        });
    }
    Ok(PullSource::Local(PathBuf::from(from)))
}

fn validate_ssh_host(host: &str) -> Result<(), String> {
    if host.is_empty() || host.len() > 253 {
        return Err("SSH host is empty or too long".into());
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '@' | '[' | ']'))
    {
        return Err("SSH host contains characters this tool will not pass to OpenSSH".into());
    }
    Ok(())
}

fn validate_remote_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("remote source path must not be empty".into());
    }
    if path.contains('\n')
        || path.contains('\r')
        || path.contains('\0')
        || path.contains('$')
        || path.contains('`')
        || path.contains(';')
        || path.contains('|')
        || path.contains('&')
    {
        return Err(
            "remote source path contains characters this tool will not pass to OpenSSH".into(),
        );
    }
    Ok(())
}

fn resolve_against_cwd(cwd: &Path, raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() { p } else { cwd.join(p) }
}

fn dest_is_inside_src(src: &Path, dest: &Path) -> Result<bool, String> {
    let src_c = dunce::canonicalize(src).map_err(|e| format!("source: {e}"))?;
    let dest_c = if dest.exists() {
        dunce::canonicalize(dest).map_err(|e| format!("dest: {e}"))?
    } else if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        let parent_c = if parent.exists() {
            dunce::canonicalize(parent).map_err(|e| format!("dest parent: {e}"))?
        } else {
            parent.to_path_buf()
        };
        parent_c.join(dest.file_name().unwrap_or_else(|| OsStr::new("")))
    } else {
        dest.to_path_buf()
    };
    Ok(dest_c.starts_with(&src_c))
}

/// Walk `src` and copy into `dest` with `std::fs`, skipping excluded names.
pub fn copy_tree_std_fs(src: &Path, dest: &Path) -> Result<CopyReport, String> {
    let src_meta =
        fs::symlink_metadata(src).map_err(|e| format!("source {}: {e}", src.display()))?;
    if src_meta.file_type().is_symlink() {
        return Err("source must be a directory, not a symlink".into());
    }
    if !src_meta.is_dir() {
        return Err(format!("source must be a directory: {}", src.display()));
    }
    if dest_is_inside_src(src, dest)? {
        return Err("destination must not be inside the source tree".into());
    }
    fs::create_dir_all(dest).map_err(|e| format!("create dest {}: {e}", dest.display()))?;

    let mut report = CopyReport::default();
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rel = dir.strip_prefix(src).unwrap_or(Path::new(""));
        if path_has_excluded_component(rel) {
            continue;
        }
        let dest_dir = dest.join(rel);
        if dir != src {
            fs::create_dir_all(&dest_dir)
                .map_err(|e| format!("create {}: {e}", dest_dir.display()))?;
            report.dirs_created += 1;
        }
        let entries = fs::read_dir(&dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
        for ent in entries {
            let ent = ent.map_err(|e| format!("read {}: {e}", dir.display()))?;
            let name = ent.file_name();
            if is_excluded_name(&name) {
                continue;
            }
            let src_path = ent.path();
            let dest_path = dest_dir.join(&name);
            let meta = fs::symlink_metadata(&src_path)
                .map_err(|e| format!("stat {}: {e}", src_path.display()))?;
            if meta.file_type().is_symlink() {
                copy_symlink(&src_path, &dest_path)?;
                report.files_copied += 1;
            } else if meta.is_dir() {
                stack.push(src_path);
            } else if meta.is_file() {
                fs::copy(&src_path, &dest_path).map_err(|e| {
                    format!(
                        "copy {} -> {}: {e}",
                        src_path.display(),
                        dest_path.display()
                    )
                })?;
                copy_permissions(&src_path, &dest_path)?;
                report.files_copied += 1;
            }
        }
    }
    Ok(report)
}

fn copy_symlink(src: &Path, dest: &Path) -> Result<(), String> {
    let target = fs::read_link(src).map_err(|e| format!("readlink {}: {e}", src.display()))?;
    #[cfg(unix)]
    {
        if dest.exists() {
            let _ = fs::remove_file(dest);
        }
        std::os::unix::fs::symlink(&target, dest)
            .map_err(|e| format!("symlink {} -> {}: {e}", dest.display(), target.display()))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (dest, target);
        Err("copying symlinks is not supported on this platform".into())
    }
}

fn copy_permissions(src: &Path, dest: &Path) -> Result<(), String> {
    let perms = fs::metadata(src)
        .map_err(|e| format!("metadata {}: {e}", src.display()))?
        .permissions();
    fs::set_permissions(dest, perms).map_err(|e| format!("chmod {}: {e}", dest.display()))
}

/// Fetch `HOST:SRC` through OpenSSH `ssh` + remote `tar`, unpack into `staging`.
///
/// The later dest copy is still [`copy_tree_std_fs`]. This is transport only.
pub fn fetch_remote_tree_via_openssh(
    host: &str,
    remote_path: &str,
    staging: &Path,
) -> Result<(), String> {
    let ssh = which::which("ssh")
        .map_err(|_| "OpenSSH ssh is not on PATH; cannot fetch a remote tree".to_string())?;
    fs::create_dir_all(staging).map_err(|e| format!("staging {}: {e}", staging.display()))?;
    let mut cmd = Command::new(ssh);
    cmd.arg("-o")
        .arg("BatchMode=yes")
        .arg("--")
        .arg(host)
        .arg("tar")
        .arg("-C")
        .arg(remote_path)
        .arg("--exclude=.git")
        .arg("--exclude=target")
        .arg("--exclude=.lake")
        .arg("--exclude=result")
        .arg("-cf")
        .arg("-")
        .arg(".")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::util::detach_std_command(&mut cmd);
    #[allow(clippy::disallowed_methods)] // enrolled into ProcessScope below
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("OpenSSH ssh failed to start: {e}"))?;
    let _group = match crate::util::global_process_scope().enroll_std(&child) {
        Ok(group) => group,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("failed to enroll OpenSSH ssh: {e}"));
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "OpenSSH ssh stdout missing".to_string())?;
    let stderr_handle = child.stderr.take().map(|mut stderr| {
        std::thread::spawn(move || {
            let mut err = String::new();
            let _ = stderr.read_to_string(&mut err);
            err
        })
    });
    let mut archive = tar::Archive::new(stdout);
    for entry in archive
        .entries()
        .map_err(|e| format!("read remote tar: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("remote tar entry: {e}"))?;
        let Ok(path) = entry.path() else {
            continue;
        };
        if path_has_excluded_component(path.as_ref()) {
            continue;
        }
        if path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            continue;
        }
        entry
            .unpack_in(staging)
            .map_err(|e| format!("unpack remote tree: {e}"))?;
    }
    let status = child.wait().map_err(|e| format!("OpenSSH ssh wait: {e}"))?;
    if !status.success() {
        let extra = stderr_handle
            .and_then(|h| h.join().ok())
            .unwrap_or_default();
        let extra = extra.trim();
        return Err(if extra.is_empty() {
            format!("OpenSSH ssh exited {status}")
        } else {
            format!("OpenSSH ssh exited {status}: {extra}")
        });
    }
    Ok(())
}

/// Validate inputs and copy `from` onto local `dest`.
pub fn pull_remote_tree(cwd: &Path, from: &str, dest: &str) -> Result<CopyReport, String> {
    if would_git_commit_or_push(&[from, dest]) {
        return Err(
            "Refused: from or dest would git commit or git push. This tool never commits or pushes."
                .into(),
        );
    }
    if looks_like_ssh_target(dest) {
        return Err(format!(
            "Refused: dest {dest:?} looks like HOST:PATH. pull_remote_tree copies onto a local directory only."
        ));
    }
    let dest_path = resolve_against_cwd(cwd, dest.trim());
    let source = parse_pull_source(from)?;
    match source {
        PullSource::Local(raw) => {
            let src = resolve_against_cwd(cwd, raw.to_str().unwrap_or(from));
            copy_tree_std_fs(&src, &dest_path)
        }
        PullSource::Remote { host, path } => {
            let staging = tempfile::tempdir().map_err(|e| format!("staging temp dir: {e}"))?;
            fetch_remote_tree_via_openssh(&host, &path, staging.path())?;
            copy_tree_std_fs(staging.path(), &dest_path)
        }
    }
}

/// Agent tool: pull a remote tree onto a local dest.
#[derive(Debug, Default)]
pub struct PullRemoteTreeTool;

impl ToolMetadata for PullRemoteTreeTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        r#"Pull a remote tree onto a local destination directory.

Direction is from `from` (HOST:SRC via OpenSSH, or a local directory that is not dest) onto local `dest` only. Dest that looks like HOST:PATH is refused. Copy is a Rust walk plus std::fs. OpenSSH may fetch a remote tree. This tool does not shell out to rsync as the copy implementation.

Always excludes directories named .git, target, .lake, and result.

Never git commit. Never git push. Refuse when from or dest would be those commands.

Prefer this named tool when the user wants a remote project tree on this machine. Do not invent a shell rsync."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for PullRemoteTreeTool {
    type Args = PullRemoteTreeInput;
    type Output = PullRemoteTreeOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(PULL_REMOTE_TREE_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            PULL_REMOTE_TREE_TOOL_NAME,
            ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.pull_remote_tree", skip_all)]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: PullRemoteTreeInput,
    ) -> Result<PullRemoteTreeOutput, xai_tool_runtime::ToolError> {
        let resources = shared_resources(&ctx)?;
        let cwd = resolve_cwd(&ctx, &resources).await?;
        let report = pull_remote_tree(&cwd, &input.from, &input.dest)
            .map_err(xai_tool_runtime::ToolError::invalid_arguments)?;
        Ok(PullRemoteTreeOutput {
            message: format!(
                "Copied {} file(s) from {} onto {} (excluded .git, target, .lake, result). Never git commit.",
                report.files_copied, input.from, input.dest
            ),
            files_copied: report.files_copied,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::resources::{Cwd, Resources};
    use crate::types::tool_metadata::test_ctx;
    use xai_tool_runtime::Tool;
    use xai_tool_runtime::error::ToolErrorKind;

    fn write_tree(root: &Path) {
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("keep.txt"), "keep").unwrap();
        fs::write(root.join("nested/a.rs"), "fn a() {}").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/x"), "elf").unwrap();
        fs::create_dir_all(root.join(".lake")).unwrap();
        fs::write(root.join(".lake/foo"), "lake").unwrap();
        fs::create_dir_all(root.join("result")).unwrap();
        fs::write(root.join("result/link"), "nix").unwrap();
        fs::write(root.join("keep2.txt"), "keep2").unwrap();
    }

    #[test]
    fn registry_knows_pull_remote_tree() {
        let builder = crate::registry::types::ToolRegistryBuilder::new();
        assert!(
            builder.has_tool_id("GrokBuild:pull_remote_tree"),
            "register_all must wire pull_remote_tree"
        );
    }

    #[test]
    fn tool_id_is_pull_remote_tree_not_rsync() {
        assert_eq!(PullRemoteTreeTool.id().as_str(), PULL_REMOTE_TREE_TOOL_NAME);
        assert_eq!(PULL_REMOTE_TREE_TOOL_NAME, "pull_remote_tree");
        assert_ne!(PULL_REMOTE_TREE_TOOL_NAME, "rsync");
        let desc = ToolMetadata::description_template(&PullRemoteTreeTool);
        assert!(
            desc.contains("pull_remote_tree") || desc.contains("HOST:SRC"),
            "description should name this tool, got {desc}"
        );
        assert!(
            !desc.contains("tool id rsync"),
            "must not present rsync as the tool id"
        );
    }

    #[test]
    fn looks_like_ssh_target_detects_host_colon_path() {
        assert!(looks_like_ssh_target("host:/var/src"));
        assert!(looks_like_ssh_target("user@box:~/proj"));
        assert!(looks_like_ssh_target("box:relative"));
        assert!(!looks_like_ssh_target("/tmp/local"));
        assert!(!looks_like_ssh_target("/tmp/foo:bar"));
        assert!(!looks_like_ssh_target(r"C:\Users\src"));
        assert!(!looks_like_ssh_target("C:/Users/src"));
        assert!(!looks_like_ssh_target("https://example.com/tree"));
    }

    #[test]
    fn would_git_commit_or_push_detects_argv_and_paths() {
        assert!(would_git_commit_or_push(&["git commit -m x", "/tmp/out"]));
        assert!(would_git_commit_or_push(&[
            "/tmp/src",
            "git push origin main"
        ]));
        assert!(would_git_commit_or_push(&["/usr/bin/git commit -am wip"]));
        assert!(!would_git_commit_or_push(&["/tmp/src", "/tmp/dest"]));
        assert!(!would_git_commit_or_push(&[
            "/tmp/my-git-commit-notes",
            "/tmp/out"
        ]));
        assert!(!would_git_commit_or_push(&["git commit-graph write"]));
    }

    #[test]
    fn copy_tree_excludes_git_target_lake_result() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dest = tmp.path().join("dest");
        write_tree(&src);
        let report = copy_tree_std_fs(&src, &dest).unwrap();
        assert!(dest.join("keep.txt").is_file());
        assert!(dest.join("nested/a.rs").is_file());
        assert!(dest.join("keep2.txt").is_file());
        assert!(!dest.join(".git").exists(), ".git must be excluded");
        assert!(!dest.join("target").exists(), "target must be excluded");
        assert!(!dest.join(".lake").exists(), ".lake must be excluded");
        assert!(!dest.join("result").exists(), "result must be excluded");
        assert!(
            report.files_copied >= 3,
            "expected kept files, got {}",
            report.files_copied
        );
    }

    #[test]
    fn pull_remote_tree_local_source_is_not_dest() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("remote_standin");
        let dest = tmp.path().join("local_dest");
        write_tree(&src);
        let report =
            pull_remote_tree(tmp.path(), src.to_str().unwrap(), dest.to_str().unwrap()).unwrap();
        assert!(dest.join("keep.txt").is_file());
        assert!(!dest.join(".git").exists());
        assert!(report.files_copied >= 3);
    }

    #[test]
    fn pull_remote_tree_refuses_ssh_shaped_dest() {
        let err = pull_remote_tree(Path::new("/tmp"), "/tmp/src", "host:/tmp/out").unwrap_err();
        assert!(
            err.contains("HOST:PATH") || err.contains("local"),
            "expected refuse remote dest, got {err}"
        );
    }

    #[test]
    fn pull_remote_tree_refuses_user_at_host_dest() {
        let err = pull_remote_tree(Path::new("/tmp"), "/tmp/src", "user@box:~/out").unwrap_err();
        assert!(
            err.contains("HOST:PATH") || err.contains("local"),
            "expected refuse SSH dest, got {err}"
        );
    }

    #[test]
    fn pull_remote_tree_refuses_git_commit_argv() {
        let err = pull_remote_tree(Path::new("/tmp"), "git commit -m x", "/tmp/out").unwrap_err();
        assert!(
            err.contains("git commit") || err.contains("git push"),
            "expected git mutate refuse, got {err}"
        );
    }

    #[test]
    fn pull_remote_tree_refuses_git_push_dest() {
        let err = pull_remote_tree(Path::new("/tmp"), "/tmp/src", "git push origin").unwrap_err();
        assert!(
            err.contains("git commit") || err.contains("git push"),
            "expected git mutate refuse, got {err}"
        );
    }

    #[tokio::test]
    async fn run_refuses_ssh_shaped_dest() {
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        let tool = PullRemoteTreeTool;
        let err = tool
            .run(
                test_ctx(resources.into_shared()),
                PullRemoteTreeInput {
                    from: "/tmp/src".into(),
                    dest: "box:/tmp/out".into(),
                },
            )
            .await
            .expect_err("SSH-shaped dest must fail at Tool::run");
        assert_eq!(err.kind, ToolErrorKind::InvalidArguments);
        let msg = err.to_string();
        assert!(
            msg.contains("HOST:PATH") || msg.contains("local"),
            "runtime error should refuse SSH dest; got {msg}"
        );
    }

    #[tokio::test]
    async fn run_copies_local_standin_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("from");
        let dest = tmp.path().join("onto");
        write_tree(&src);
        let mut resources = Resources::new();
        resources.insert(Cwd(tmp.path().to_path_buf()));
        let tool = PullRemoteTreeTool;
        let out = tool
            .run(
                test_ctx(resources.into_shared()),
                PullRemoteTreeInput {
                    from: src.to_string_lossy().into_owned(),
                    dest: dest.to_string_lossy().into_owned(),
                },
            )
            .await
            .expect("local stand-in copy should succeed");
        assert!(dest.join("keep.txt").is_file());
        assert!(!dest.join("target").exists());
        assert!(out.files_copied >= 3);
        assert!(out.message.contains("Never git commit"));
    }
}
