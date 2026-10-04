use std::path::PathBuf;
use std::sync::OnceLock;
use xai_tool_runtime::{ToolError, ToolErrorKind};

#[cfg(bundle_rg)]
const RG_BYTES: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle-rg/rg-",
    env!("GROK_TOOLS_RG_VER"),
    "-",
    env!("GROK_TOOLS_RG_TARGET"),
    ".bin.zst"
));

#[cfg(bundle_rg)]
fn resolve_bundled_rg() -> Result<Option<PathBuf>, crate::util::vendor::InstallError> {
    crate::util::vendor::resolve(
        concat!(
            "rg-",
            env!("GROK_TOOLS_RG_VER"),
            "-",
            env!("GROK_TOOLS_RG_TARGET")
        ),
        RG_BYTES,
        env!("GROK_TOOLS_RG_SHA256"),
    )
}

pub fn rg_path() -> Result<PathBuf, ToolError> {
    static RG_EXEC: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    RG_EXEC.get_or_init(|| {
        #[cfg(bundle_rg)]
        {
            resolve_bundled_rg()
                .map(|found| found.unwrap_or_else(|| PathBuf::from("rg")))
                .map_err(|e| e.to_string())
        }
        #[cfg(not(bundle_rg))]
        {
            Ok(rg_from_path_or_runfiles())
        }
    });

    let truncated = writer.truncated;
    let bytes = writer.inner;
    let exit_code = if !stderr.is_empty() && bytes.is_empty() && !any_match {
        2
    } else if !any_match && bytes.is_empty() {
        1
    } else {
        0
    };
    Ok(SearchStdout {
        bytes,
        stderr,
        exit_code,
        truncated,
    })
}

fn search_one_file(
    req: &SearchRequest,
    matcher: &grep::regex::RegexMatcher,
    searcher: &mut grep::searcher::Searcher,
    path: &Path,
    writer: &mut BudgetWriter,
    stop: &AtomicBool,
) -> Result<bool, SearchError> {
    if writer.truncated {
        stop.store(true, Ordering::Relaxed);
        return Ok(false);
    }
    let before = writer.inner.len();
    match req.print {
        PrintMode::Content => {
            let mut printer = StandardBuilder::new()
                .heading(true)
                .max_columns(req.max_columns)
                .max_columns_preview(req.max_columns.is_some())
                .build_no_color(&mut *writer);
            searcher
                .search_path(matcher, path, printer.sink_with_path(matcher, path))
                .map_err(|e| SearchError {
                    message: e.to_string(),
                })?;
        }
        PrintMode::FilesWithMatches => {
            let mut printer = SummaryBuilder::new()
                .kind(SummaryKind::PathWithMatch)
                .build_no_color(&mut *writer);
            searcher
                .search_path(matcher, path, printer.sink_with_path(matcher, path))
                .map_err(|e| SearchError {
                    message: e.to_string(),
                })?;
        }
        PrintMode::Count => {
            let mut printer = SummaryBuilder::new()
                .kind(SummaryKind::Count)
                .exclude_zero(true)
                .build_no_color(&mut *writer);
            searcher
                .search_path(matcher, path, printer.sink_with_path(matcher, path))
                .map_err(|e| SearchError {
                    message: e.to_string(),
                })?;
        }
    }
    Ok(writer.inner.len() > before)
}

/// Collect matching lines without going through an `rg` printer.
pub fn search_line_hits(req: &SearchRequest) -> Result<Vec<LineHit>, SearchError> {
    let matcher = matcher_for(req)?;
    let mut searcher = searcher_for(req);
    let mut hits = Vec::new();
    let files: Vec<PathBuf> = if req.path.is_file() {
        vec![req.path.clone()]
    } else {
        let walker = walk_builder_for(req)?;
        walker
            .build()
            .filter_map(|d| d.ok())
            .filter(|d| d.file_type().is_some_and(|t| t.is_file()))
            .map(|d| d.into_path())
            .collect()
    };
    for path in files {
        let path_str = path.display().to_string();
        let collected = std::cell::RefCell::new(Vec::new());
        let sink = grep::searcher::sinks::UTF8(|line_number, line| {
            let text = line.trim_end_matches(['\n', '\r']).to_string();
            let (match_start, match_end) = matcher
                .find(text.as_bytes())
                .ok()
                .flatten()
                .map(|m| (Some(m.start()), Some(m.end())))
                .unwrap_or((None, None));
            collected.borrow_mut().push(LineHit {
                path: path_str.clone(),
                line_number: line_number as usize,
                line_text: text,
                match_start,
                match_end,
            });
            Ok(true)
        });
        searcher
            .search_path(&matcher, &path, sink)
            .map_err(|e| SearchError {
                message: e.to_string(),
            })?;
        hits.append(&mut collected.into_inner());
    }
    Ok(hits)
}

/// List files matching a glob (`rg --files --glob`), in-process via `ignore`.
pub fn list_files(
    search_dir: &Path,
    pattern: &str,
    extra_exclude: &[&str],
) -> Result<Vec<PathBuf>, SearchError> {
    let mut overrides = OverrideBuilder::new(search_dir);
    overrides.add(pattern).map_err(|e| SearchError {
        message: format!("invalid glob: {e}"),
    })?;
    for ex in extra_exclude {
        overrides.add(ex).map_err(|e| SearchError {
            message: format!("invalid glob: {e}"),
        })?;
    }
    let overrides = overrides.build().map_err(|e| SearchError {
        message: format!("invalid glob: {e}"),
    })?;
    let mut walker = WalkBuilder::new(search_dir);
    walker.hidden(false);
    walker.git_ignore(true);
    walker.follow_links(false);
    walker.overrides(overrides);
    let mut out = Vec::new();
    for dent in walker.build() {
        let Ok(dent) = dent else { continue };
        if dent.file_type().is_some_and(|t| t.is_file()) {
            out.push(dent.into_path());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn just_install_does_not_cargo_install_ripgrep_and_grok_oss_grep_is_embedded_rust_not_a_sidecar_rg()
     {
        let tools_build = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
        assert!(
            !tools_build.contains("cargo_install_ripgrep")
                && !(tools_build.contains("\"ripgrep\"") && tools_build.contains("--bin")),
            "grok-oss grep is embedded Rust, not a sidecar rg: xai-grok-tools build.rs must not cargo-install ripgrep"
        );
        assert!(
            !tools_build.contains("github.com/BurntSushi/ripgrep/releases"),
            "xai-grok-tools build.rs must not download a GitHub musl ripgrep tarball"
        );

        let this_src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/implementations/grok_build/grep/ripgrep.rs"
        ));
        let prod = this_src.split("#[cfg(test)]").next().unwrap_or(this_src);
        assert!(
            prod.contains("grep::regex") && prod.contains("WalkBuilder"),
            "grok-oss grep is embedded Rust, not a sidecar rg"
        );
        assert!(
            !prod.contains("std::process::Command"),
            "grok-oss grep is embedded Rust, not a sidecar rg: embedded search must not exec rg"
        );
    }

    #[test]
    fn embedded_search_finds_a_line_without_execing_rg() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello world\n").unwrap();
        let out = search_to_rg_stdout(&SearchRequest {
            pattern: "hello".into(),
            path: tmp.path().to_path_buf(),
            case_insensitive: false,
            literal: false,
            glob: None,
            extra_globs: Vec::new(),
            deny_globs: Vec::new(),
            file_type: None,
            hidden: true,
            no_ignore: false,
            multiline: false,
            before_context: 0,
            after_context: 0,
            max_filesize: Some(5 * 1024 * 1024),
            max_columns: Some(1000),
            print: PrintMode::Content,
            max_output_lines: None,
        })
        .clone()
        .map_err(|msg| ToolError::new(ToolErrorKind::Execution, msg));
    }
}

#[cfg(not(bundle_rg))]
fn rg_from_path_or_runfiles() -> PathBuf {
    if let Ok(p) = std::env::var("RG_BIN_PATH") {
        return PathBuf::from(p);
    }
    if let Ok(rf) = std::env::var("RUNFILES_DIR")
        && let Ok(entries) = std::fs::read_dir(PathBuf::from(rf))
    {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .contains("ripgrep_hermetic")
            {
                for sub in ["amd64/rg", "arm64/rg", "rg"] {
                    let candidate = entry.path().join(sub);
                    if candidate.exists() {
                        return candidate;
                    }
                }
            }
        }
    }
    PathBuf::from("rg")
}
