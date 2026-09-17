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

/// Streaming content search with batched status notifications. grok-oss grep
/// is embedded Rust, not a sidecar `rg`. Cancellation is dropping this future
/// (the blocking search may finish the current file).
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

    #[tokio::test]
    async fn embedded_content_search_finds_a_line_without_execing_rg() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "needle here\n").unwrap();
        let params = ContentSearchParams {
            pattern: "needle".to_string(),
            ..Default::default()
        };
        let data = content_search_streaming(tmp.path(), &params, |_| {})
            .await
            .expect("embedded search");
        assert_eq!(data.total_matches, 1);
        assert!(
            data.files.iter().any(|f| f.path.contains("a.txt")),
            "expected a.txt in {:?}",
            data.files
        );
    }
}
