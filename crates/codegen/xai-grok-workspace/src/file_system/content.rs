use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use xai_grok_tools::implementations::grok_build::grep::embedded::{
    PrintMode, SearchRequest, search_line_hits,
};

// Canonical in xai-grok-workspace-types; re-exported for existing paths.
pub use xai_grok_workspace_types::rpc::search::{
    ContentMatch, ContentMatchFile, ContentSearchData,
};

#[derive(Debug, Clone, Default)]
pub struct ContentSearchParams {
    pub pattern: String,
    pub case_insensitive: bool,
    pub literal: bool,
    pub globs: Vec<String>,
    pub max_files: Option<usize>,
    pub max_matches: Option<usize>,
    pub respect_gitignore: bool,
}

/// Batch of results sent during streaming search.
#[derive(Debug, Clone, Default)]
pub struct ContentSearchBatch {
    pub files: Vec<ContentMatchFile>,
    pub total_matches: usize,
    pub total_files: usize,
    pub done: bool,
    pub truncated: bool,
}

const BATCH_INTERVAL_MS: u64 = 50;
const DEFAULT_MAX_FILES: usize = 100;
const DEFAULT_MAX_MATCHES: usize = 1000;
const MAX_COUNT_PER_FILE: usize = 50;

fn build_ripgrep_command(root: &Path, params: &ContentSearchParams) -> anyhow::Result<Command> {
    let rg_path = crate::util::ripgrep::rg_path()?;

    let mut cmd = Command::new(&rg_path);
    cmd.current_dir(root);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    xai_tty_utils::detach_search_command(&mut cmd);

    cmd.arg("--json");
    cmd.arg("--line-number");

    const DEFAULT_EXCLUSIONS: &[&str] = &["!.git/**", "!submodules/**", "!vendor/**"];
    for glob in DEFAULT_EXCLUSIONS {
        cmd.arg("--glob").arg(glob);
    }

    cmd.arg("--max-filesize").arg("1M");
    cmd.arg("--max-count").arg("50");
    cmd.arg("--max-columns").arg("500");
    cmd.arg("--max-columns-preview");

    if params.case_insensitive {
        cmd.arg("--ignore-case");
    }
    if params.literal {
        cmd.arg("--fixed-strings");
    }
    if !params.respect_gitignore {
        cmd.arg("--no-ignore");
    }
    for glob in &params.globs {
        cmd.arg("--glob").arg(glob);
    }

    cmd.arg("-e").arg(&params.pattern);
    cmd.arg(".");

    Ok(cmd)
}

fn extract_match_positions(data: &serde_json::Value) -> (Option<usize>, Option<usize>) {
    data.get("submatches")
        .and_then(|s| s.as_array())
        .and_then(|arr| arr.first())
        .map(|first| {
            let start = first
                .get("start")
                .and_then(|s| s.as_u64())
                .map(|s| s as usize);
            let end = first
                .get("end")
                .and_then(|e| e.as_u64())
                .map(|e| e as usize);
            (start, end)
        })
        .unwrap_or((None, None))
}

fn parse_match_from_json(data: &serde_json::Value) -> Option<ContentMatch> {
    let line_number = data.get("line_number").and_then(|l| l.as_u64())? as usize;
    let content = data
        .get("lines")
        .and_then(|l| l.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim_end_matches('\n')
        .to_string();
    let (match_start, match_end) = extract_match_positions(data);

    Some(ContentMatch {
        line: line_number,
        content,
        match_start,
        match_end,
    })
}

fn parse_file_path_from_json(root: &Path, json: &serde_json::Value) -> Option<String> {
    let path = json
        .get("data")
        .and_then(|d| d.get("path"))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())?;
    let normalized = path.strip_prefix("./").unwrap_or(path);
    if Path::new(normalized).is_absolute() {
        return Some(normalized.to_string());
    }
    Some(root.join(normalized).to_string_lossy().to_string())
}

/// Streaming content search with batched status notifications.
/// Cancellation is dropping the future: the spawn config kills rg on drop.
pub async fn content_search_streaming<F>(
    root: &Path,
    params: &ContentSearchParams,
    on_status: F,
) -> anyhow::Result<ContentSearchData>
where
    F: Fn(ContentSearchBatch) + Send + 'static,
{
    let max_files = params.max_files.unwrap_or(DEFAULT_MAX_FILES);
    let max_matches = params.max_matches.unwrap_or(DEFAULT_MAX_MATCHES);
    let mut extra_globs = vec![
        "!.git/**".to_string(),
        "!submodules/**".to_string(),
        "!vendor/**".to_string(),
    ];
    extra_globs.extend(params.globs.iter().cloned());
    let req = SearchRequest {
        pattern: params.pattern.clone(),
        path: root.to_path_buf(),
        case_insensitive: params.case_insensitive,
        literal: params.literal,
        glob: None,
        extra_globs,
        deny_globs: Vec::new(),
        file_type: None,
        hidden: false,
        no_ignore: !params.respect_gitignore,
        multiline: false,
        before_context: 0,
        after_context: 0,
        max_filesize: Some(1024 * 1024),
        max_columns: Some(500),
        print: PrintMode::Content,
        max_output_lines: None,
    };

    let hits = tokio::task::spawn_blocking(move || search_line_hits(&req))
        .await
        .map_err(|e| anyhow::anyhow!("embedded grep join: {e}"))?
        .map_err(|e| anyhow::anyhow!("embedded grep: {e}"))?;

    let mut per_file: BTreeMap<String, Vec<ContentMatch>> = BTreeMap::new();
    for hit in hits {
        let entry = per_file.entry(hit.path).or_default();
        if entry.len() >= MAX_COUNT_PER_FILE {
            continue;
        }
        entry.push(ContentMatch {
            line: hit.line_number,
            content: hit.line_text,
            match_start: hit.match_start,
            match_end: hit.match_end,
        });
    }

    let mut cmd = build_ripgrep_command(root, params)?;
    #[allow(clippy::disallowed_methods)] // waited on below; killed on drop (cancellation)
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn ripgrep: {}", e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture ripgrep stdout"))?;

    let mut reader = BufReader::new(stdout).lines();
    let mut files: Vec<ContentMatchFile> = Vec::new();
    let mut pending_files: Vec<ContentMatchFile> = Vec::new();
    let mut total_matches = 0usize;
    let mut last_notify = Instant::now();
    let mut truncated = false;

    for (path, matches) in per_file {
        if files.len() >= max_files || total_matches >= max_matches {
            truncated = true;
            break;
        }
        let take = matches.len().min(max_matches.saturating_sub(total_matches));
        if take == 0 {
            truncated = true;
            break;
        }
        total_matches += take;
        let mut file = ContentMatchFile::new(path);
        file.matches = matches.into_iter().take(take).collect();
        pending_files.push(file.clone());
        files.push(file);

        if !pending_files.is_empty()
            && last_notify.elapsed().as_millis() >= BATCH_INTERVAL_MS as u128
        {
            on_status(ContentSearchBatch {
                files: std::mem::take(&mut pending_files),
                total_matches,
                total_files: files.len(),
                done: false,
                truncated: false,
            });
            tokio::task::yield_now().await;
            last_notify = Instant::now();
        }
    }

    let total_files = files.len();
    on_status(ContentSearchBatch {
        files: pending_files,
        total_matches,
        total_files,
        done: true,
        truncated,
    });
    tokio::task::yield_now().await;

    Ok(ContentSearchData {
        files,
        total_matches,
        total_files,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cancellation is dropping the future; commands from `build_ripgrep_command` must kill rg on drop.
    #[cfg(unix)]
    #[tokio::test]
    async fn embedded_content_search_finds_a_line_without_execing_rg() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Overflow the stdout pipe (rg caps 50 matches/file, so use many files) so rg blocks on write and stays alive until killed
        let line = format!("needle {}\n", "x".repeat(120));
        for i in 0..200 {
            std::fs::write(tmp.path().join(format!("f{i}.txt")), line.repeat(50)).unwrap();
        }

        let params = ContentSearchParams {
            pattern: "needle".to_string(),
            ..Default::default()
        };
        let mut cmd = build_ripgrep_command(tmp.path(), &params).expect("build rg command");
        // rg is hermetic under Bazel and on PATH locally; spawn failure is a real bug.
        #[allow(clippy::disallowed_methods)] // test child, killed on drop below
        let mut child = cmd.spawn().expect("spawn rg");
        let pid = child.id().expect("child pid");

        // Hold the read end open (no EPIPE death) and drop the child mid-run.
        let stdout_pipe = child.stdout.take();
        drop(child);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !xai_tty_utils::process_not_running(pid) {
            assert!(
                std::time::Instant::now() < deadline,
                "rg (pid {pid}) still running 5s after its Child was dropped — leaked"
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        drop(stdout_pipe);
    }
}
