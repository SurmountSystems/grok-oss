//! Embedded ripgrep: grok-oss grep calls the `grep` crate plus `ignore`.
//! It does not exec a sidecar `rg` binary.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use grep::matcher::Matcher;
use grep::printer::{StandardBuilder, SummaryBuilder, SummaryKind};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use ignore::types::TypesBuilder;

/// How to print matches, matching the former `rg` flags the tools passed.
#[derive(Debug, Clone, Copy)]
pub enum PrintMode {
    /// `--heading --line-number --with-filename`
    Content,
    /// `-l` / `--files-with-matches`
    FilesWithMatches,
    /// `-c`
    Count,
}

/// Request for an in-process search. Same knobs the tools used to pass to `rg`.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub pattern: String,
    pub path: PathBuf,
    pub case_insensitive: bool,
    pub literal: bool,
    pub glob: Option<String>,
    pub extra_globs: Vec<String>,
    pub deny_globs: Vec<String>,
    pub file_type: Option<String>,
    pub hidden: bool,
    pub no_ignore: bool,
    pub multiline: bool,
    pub before_context: usize,
    pub after_context: usize,
    pub max_filesize: Option<u64>,
    pub max_columns: Option<u64>,
    pub print: PrintMode,
    /// Stop after this many output lines (heading + matches). `None` walks all.
    pub max_output_lines: Option<usize>,
}

/// Bytes that look like `rg` stdout, plus the exit code `rg` would have used.
#[derive(Debug, Clone)]
pub struct SearchStdout {
    pub bytes: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct SearchError {
    pub message: String,
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// One content hit (path, line, text). Used when callers format themselves.
#[derive(Debug, Clone)]
pub struct LineHit {
    pub path: String,
    pub line_number: usize,
    pub line_text: String,
    pub match_start: Option<usize>,
    pub match_end: Option<usize>,
}

struct BudgetWriter {
    inner: Vec<u8>,
    max_lines: Option<usize>,
    lines: usize,
    truncated: bool,
}

impl BudgetWriter {
    fn new(max_lines: Option<usize>) -> Self {
        Self {
            inner: Vec::new(),
            max_lines,
            lines: 0,
            truncated: false,
        }
    }
}

impl Write for BudgetWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.truncated {
            return Ok(buf.len());
        }
        if let Some(max) = self.max_lines
            && self.lines >= max
        {
            self.truncated = true;
            return Ok(buf.len());
        }
        self.inner.extend_from_slice(buf);
        self.lines += bytecount_newlines(buf);
        if let Some(max) = self.max_lines
            && self.lines >= max
        {
            self.truncated = true;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bytecount_newlines(buf: &[u8]) -> usize {
    buf.iter().filter(|&&b| b == b'\n').count()
}

fn matcher_for(req: &SearchRequest) -> Result<grep::regex::RegexMatcher, SearchError> {
    let mut builder = RegexMatcherBuilder::new();
    builder.case_insensitive(req.case_insensitive);
    builder.fixed_strings(req.literal);
    if req.multiline {
        builder.multi_line(true).dot_matches_new_line(true);
    }
    let pattern = if req.pattern.is_empty() {
        "^"
    } else {
        req.pattern.as_str()
    };
    builder.build(pattern).map_err(|e| SearchError {
        message: e.to_string(),
    })
}

fn searcher_for(req: &SearchRequest) -> grep::searcher::Searcher {
    SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\0'))
        .line_number(true)
        .before_context(req.before_context)
        .after_context(req.after_context)
        .multi_line(req.multiline)
        .build()
}

fn walk_builder_for(req: &SearchRequest) -> Result<WalkBuilder, SearchError> {
    let mut walker = WalkBuilder::new(&req.path);
    walker.hidden(!req.hidden);
    walker.git_ignore(!req.no_ignore);
    walker.git_exclude(!req.no_ignore);
    walker.git_global(!req.no_ignore);
    walker.follow_links(false);
    if let Some(max) = req.max_filesize {
        walker.max_filesize(Some(max));
    }

    let mut overrides = OverrideBuilder::new(&req.path);
    if let Some(glob) = req.glob.as_deref().filter(|g| !g.is_empty()) {
        overrides.add(glob).map_err(|e| SearchError {
            message: format!("invalid glob: {e}"),
        })?;
    }
    for g in &req.extra_globs {
        overrides.add(g).map_err(|e| SearchError {
            message: format!("invalid glob: {e}"),
        })?;
    }
    for deny in &req.deny_globs {
        let pat = if deny.starts_with('!') {
            deny.clone()
        } else {
            format!("!{deny}")
        };
        overrides.add(&pat).map_err(|e| SearchError {
            message: format!("invalid glob: {e}"),
        })?;
    }
    let built = overrides.build().map_err(|e| SearchError {
        message: format!("invalid glob: {e}"),
    })?;
    walker.overrides(built);

    if let Some(t) = req.file_type.as_deref().filter(|s| !s.is_empty()) {
        let mut types = TypesBuilder::new();
        types.add_defaults();
        let known = types.definitions().iter().any(|d| d.name() == t);
        if !known && t != "all" {
            return Err(SearchError {
                message: format!("unrecognized file type: {t}"),
            });
        }
        types.select(t);
        let types = types.build().map_err(|e| SearchError {
            message: e.to_string(),
        })?;
        walker.types(types);
    }
    Ok(walker)
}

/// Run an in-process search and format stdout the way `rg` did for grok-oss grep.
pub fn search_to_rg_stdout(req: &SearchRequest) -> Result<SearchStdout, SearchError> {
    let matcher = matcher_for(req)?;
    let mut searcher = searcher_for(req);
    let stop = AtomicBool::new(false);
    let mut writer = BudgetWriter::new(req.max_output_lines);
    let mut any_match = false;
    let mut stderr = Vec::new();

    if req.path.is_file() {
        any_match |= search_one_file(req, &matcher, &mut searcher, &req.path, &mut writer, &stop)?;
    } else {
        let walker = walk_builder_for(req)?;
        for dent in walker.build() {
            if stop.load(Ordering::Relaxed) || writer.truncated {
                break;
            }
            let dent = match dent {
                Ok(d) => d,
                Err(e) => {
                    let _ = writeln!(stderr, "{e}");
                    continue;
                }
            };
            if !dent.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            match search_one_file(
                req,
                &matcher,
                &mut searcher,
                dent.path(),
                &mut writer,
                &stop,
            ) {
                Ok(hit) => any_match |= hit,
                Err(e) => {
                    let _ = writeln!(stderr, "{}: {e}", dent.path().display());
                }
            }
        }
    }

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
        let just = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../justfile"));
        let install = just
            .split("\ninstall:\n")
            .nth(1)
            .and_then(|rest| rest.split("\nbuild-dist:").next())
            .unwrap_or(just);
        assert!(
            !install.contains("cargo install") || !install.contains("ripgrep"),
            "just install does not cargo-install ripgrep; grok-oss grep is embedded Rust, not a sidecar rg:\n{install}"
        );
        assert!(
            !install.contains("--version 15.0.0") || !install.contains("ripgrep"),
            "just install does not cargo-install ripgrep"
        );
        assert!(
            install.contains("cargo build --release -p xai-grok-pager-bin"),
            "just install is cargo build --release of the pager"
        );

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
            "/src/implementations/grok_build/grep/embedded.rs"
        ));
        assert!(
            this_src.contains("grep::regex") && this_src.contains("WalkBuilder"),
            "grok-oss grep is embedded Rust, not a sidecar rg"
        );
        assert!(
            !this_src.contains("Command::new") && !this_src.contains("std::process::Command"),
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
        .unwrap();
        let text = String::from_utf8_lossy(&out.bytes);
        assert!(text.contains("hello world"), "embedded hit missing: {text}");
        assert_eq!(out.exit_code, 0);
    }
}
