//! Workspace-scoped path resolution (host `memory.py` parity).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

use super::MemoryError;

pub const MEMORY_DIR_NAME: &str = "implement-memory";

const GIT_TIMEOUT_SECS: u64 = 5;

static SSH_REMOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\w.-]+@([\w.-]+):(.+)$").expect("ssh remote re"));
static SSH_BARE_REMOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\w-]+(?:\.[\w-]+)+):([^/].*)$").expect("bare ssh remote re"));
static URL_REMOTE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[a-z][a-z0-9+.-]*://(?:[^@]+@)?([\w.-]+)(?::\d+)?(/.+)$")
        .expect("url remote re")
});

const CASE_INSENSITIVE_HOSTS: &[&str] = &["github.com", "gitlab.com", "bitbucket.org"];

/// Normalise a git remote URL to a stable key (host `canonicalize_remote`).
pub fn canonicalize_remote(url: &str) -> String {
    let mut url = url.trim().trim_end_matches('/').to_string();
    while url.ends_with(".git") {
        url.truncate(url.len() - 4);
    }
    if let Some(caps) = SSH_REMOTE_RE.captures(&url) {
        let host = caps[1].to_ascii_lowercase();
        let mut path = caps[2].to_string();
        if CASE_INSENSITIVE_HOSTS.iter().any(|h| *h == host) {
            path = path.to_ascii_lowercase();
        }
        return format!("{host}/{path}");
    }
    if let Some(caps) = SSH_BARE_REMOTE_RE.captures(&url) {
        let host = caps[1].to_ascii_lowercase();
        let mut path = caps[2].to_string();
        if CASE_INSENSITIVE_HOSTS.iter().any(|h| *h == host) {
            path = path.to_ascii_lowercase();
        }
        return format!("{host}/{path}");
    }
    if let Some(caps) = URL_REMOTE_RE.captures(&url) {
        let host = caps[1].to_ascii_lowercase();
        let mut path = caps[2].to_string();
        if CASE_INSENSITIVE_HOSTS.iter().any(|h| *h == host) {
            path = path.to_ascii_lowercase();
        }
        return format!("{host}{path}");
    }
    url
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    use std::sync::Arc;

    use crate::util::{ProcessGroup, global_process_scope};
    use xai_tty_utils::detach_std_command;

    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    // Own process group so a hung probe can be killed as a tree, and so the
    // global session scope can reap it if this thread wedges.
    detach_std_command(&mut cmd);
    #[allow(clippy::disallowed_methods)] // enrolled into ProcessScope below
    let mut child = cmd.spawn().ok()?;
    let group = ProcessGroup::new()
        .and_then(|mut group| {
            group.attach_std(&child)?;
            Ok(Arc::new(group))
        })
        .ok()?;
    let _enrolled = global_process_scope().register(&group);

    // Soft timeout: if git hangs, kill the enrolled tree and treat as missing.
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut out = String::new();
                use std::io::Read;
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                let out = out.trim().to_string();
                return if out.is_empty() { None } else { Some(out) };
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(GIT_TIMEOUT_SECS) {
                    let _ = group.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

/// Return `"<readable-name>-<hash12>"` for this workspace (cwd's git context).
pub fn workspace_id_for_cwd(cwd: &Path) -> Result<String, MemoryError> {
    let mut id_source = String::new();

    if let Some(raw_remote) = git(cwd, &["config", "--get", "remote.origin.url"]) {
        id_source = canonicalize_remote(&raw_remote);
    }

    if id_source.is_empty() {
        if let Some(common_dir) = git(cwd, &["rev-parse", "--git-common-dir"]) {
            let p = Path::new(&common_dir);
            id_source = if p.is_absolute() {
                dunce::canonicalize(p)
                    .unwrap_or_else(|_| p.to_path_buf())
                    .to_string_lossy()
                    .into_owned()
            } else {
                dunce::canonicalize(cwd.join(p))
                    .unwrap_or_else(|_| cwd.join(p))
                    .to_string_lossy()
                    .into_owned()
            };
        }
    }

    if id_source.is_empty() {
        id_source = dunce::canonicalize(cwd)
            .unwrap_or_else(|_| cwd.to_path_buf())
            .to_string_lossy()
            .into_owned();
        if id_source.is_empty() {
            return Err(MemoryError::WorkspaceId(
                "could not determine workspace id (no git repo, no remote, no usable cwd)".into(),
            ));
        }
    }

    let digest = Sha256::digest(id_source.as_bytes());
    let hex = format!("{digest:x}");
    let short = &hex[..12];

    let mut raw_name = id_source
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("workspace")
        .to_string();
    if raw_name.ends_with(".git") {
        raw_name.truncate(raw_name.len() - 4);
    }
    let safe: String = raw_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim_matches(|c| c == '_' || c == '.' || c == '-');
    let mut safe: String = safe.chars().take(40).collect();
    while safe.ends_with('_') || safe.ends_with('.') || safe.ends_with('-') {
        safe.pop();
    }
    if safe.is_empty() {
        safe = "workspace".into();
    }

    Ok(format!("{safe}-{short}"))
}

/// Paths for this workspace under `$HOME/.grok/implement-memory/`.
pub struct MemoryPaths {
    pub dir: PathBuf,
    pub file: PathBuf,
    pub lock: PathBuf,
}

pub fn memory_paths(
    cwd: &Path,
    home: Option<&Path>,
    create_dir: bool,
) -> Result<MemoryPaths, MemoryError> {
    let home = match home {
        Some(h) => h.to_path_buf(),
        None => dirs::home_dir().ok_or_else(|| {
            MemoryError::WorkspaceId(
                "could not determine the user's home directory ($HOME unset and pwd lookup failed)"
                    .into(),
            )
        })?,
    };
    let base = home.join(".grok").join(MEMORY_DIR_NAME);
    if create_dir {
        std::fs::create_dir_all(&base).map_err(|e| {
            MemoryError::WorkspaceId(format!(
                "could not create memory directory {}: {e}",
                base.display()
            ))
        })?;
    }
    let workspace_id = workspace_id_for_cwd(cwd)?;
    Ok(MemoryPaths {
        dir: base.clone(),
        file: base.join(format!("{workspace_id}.md")),
        lock: base.join(format!("{workspace_id}.lock")),
    })
}
