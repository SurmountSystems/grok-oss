//! Syntax highlighting support using syntect.
//!
//! This module provides the `Syntect` struct which holds the syntax definitions
//! and theme for code block highlighting.

use std::io::Cursor;
use std::path::Path;

use syntect::{
    easy::HighlightLines,
    highlighting::{Theme as SyntectTheme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxReference, SyntaxSet, SyntaxSetBuilder},
};

/// Syntax highlighting configuration.
///
/// Holds the theme and syntax definitions for code highlighting.
/// Create one instance and pass it to the markdown renderer.
pub struct Syntect {
    /// The color theme for syntax highlighting.
    pub theme: SyntectTheme,
    /// The syntax definitions (bundled `.sublime-syntax` files, yaml-load).
    pub syntax_set: SyntaxSet,
}

impl Syntect {
    /// Create a new Syntect instance from theme bytes.
    ///
    /// The theme bytes should be a TextMate `.tmTheme` file.
    /// Syntaxes come from the crate's bundled `.sublime-syntax` files
    /// (yaml-load, no syntect dump-load / bincode).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let syntect = Syntect::new(include_bytes!("assets/tokyo-night.tmTheme"));
    /// ```
    pub fn new(theme_bytes: &[u8]) -> Self {
        let mut cursor = Cursor::new(theme_bytes);
        let theme = ThemeSet::load_from_reader(&mut cursor).expect("Failed to load theme");
        Self {
            theme,
            syntax_set: bundled_syntax_set(),
        }
    }

    /// Find a syntax definition by file path extension.
    pub fn find_syntax_by_file_path(&self, file_path: &Path) -> Option<&SyntaxReference> {
        let ext = file_path.extension()?.to_str()?;
        self.syntax_set.find_syntax_by_extension(ext)
    }

    /// Find a syntax definition by language token (e.g., "rust", "python").
    pub fn find_syntax_by_token(&self, token: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_token(token)
    }

    /// Create a highlighter for the given file path.
    pub fn highlight_lines_by_file_path(&self, file_path: &Path) -> Option<HighlightLines<'_>> {
        Some(HighlightLines::new(
            self.find_syntax_by_file_path(file_path)?,
            &self.theme,
        ))
    }

    /// Create a highlighter for the given language token.
    pub fn highlight_lines_for_token(&self, token: &str) -> Option<HighlightLines<'_>> {
        Some(HighlightLines::new(
            self.find_syntax_by_token(token)?,
            &self.theme,
        ))
    }

    /// Highlighter for a fenced code block *info* string: a normal language token
    /// (e.g. `rust`, `python`), or a **line-range citation** of the form
    /// `lineStart:lineEnd:path/to/file.ext` where the syntax is resolved the same
    /// way as [`Syntect::highlight_lines_by_file_path`] (see
    /// [`Syntect::find_syntax_by_file_path`]).
    ///
    /// If the string matches the citation form but no syntax is found for the
    /// path, this falls back to [`Syntect::find_syntax_by_token`] with the full
    /// `fence_info` string, so plain ` ```lang` blocks keep working and odd
    /// citations degrade like the pre-citation code path.
    pub fn highlight_lines_for_fence_info(&self, fence_info: &str) -> Option<HighlightLines<'_>> {
        Some(HighlightLines::new(
            self.find_syntax_for_fence_info(fence_info)?,
            &self.theme,
        ))
    }

    /// Resolve the [`SyntaxReference`] for a fenced code block *info* string,
    /// using the SAME rules as [`Syntect::highlight_lines_for_fence_info`]:
    /// a `lineStart:lineEnd:path` citation resolves by file path, otherwise
    /// (or if the path has no known syntax) it falls back to a language token.
    ///
    /// Exposed so the incremental open-code highlighter can build its own
    /// resumable `ParseState`/`HighlightState` against exactly the syntax the
    /// batch `HighlightLines` path would have used — keeping the two
    /// byte-identical.
    pub(crate) fn find_syntax_for_fence_info(&self, fence_info: &str) -> Option<&SyntaxReference> {
        if let Some((_, _, path)) = parse_line_citation_fence_info(fence_info)
            && let Some(s) = self.find_syntax_by_file_path(Path::new(path))
        {
            return Some(s);
        }
        self.find_syntax_by_token(fence_info)
    }
}

/// ```text
/// lineStart:lineEnd:path/to/file.ext
/// ```
///
/// The path is the segment after the **second** colon; it is then parsed with
/// [`Path::new`]. Paths with extra colons in the first two segments (e.g. some
/// Windows `C:...` forms) are not supported; use a repo-relative or
/// forward-slash form.
/// Sublime syntax sources shipped in this crate. Loaded with syntect
/// yaml-load so highlighting does not pull bincode dump-load.
const BUNDLED_SYNTAXES: &[(&str, &str)] = &[
    (
        "Rust",
        include_str!("../assets/syntaxes/Rust.sublime-syntax"),
    ),
    (
        "JSON",
        include_str!("../assets/syntaxes/JSON.sublime-syntax"),
    ),
    (
        "Python",
        include_str!("../assets/syntaxes/Python.sublime-syntax"),
    ),
    (
        "JavaScript",
        include_str!("../assets/syntaxes/JavaScript.sublime-syntax"),
    ),
    (
        "TypeScript",
        include_str!("../assets/syntaxes/TypeScript.sublime-syntax"),
    ),
    (
        "Bash",
        include_str!("../assets/syntaxes/Bash.sublime-syntax"),
    ),
    (
        "TOML",
        include_str!("../assets/syntaxes/TOML.sublime-syntax"),
    ),
    (
        "YAML",
        include_str!("../assets/syntaxes/YAML.sublime-syntax"),
    ),
    ("Go", include_str!("../assets/syntaxes/Go.sublime-syntax")),
    (
        "Markdown",
        include_str!("../assets/syntaxes/Markdown.sublime-syntax"),
    ),
    (
        "HTML",
        include_str!("../assets/syntaxes/HTML.sublime-syntax"),
    ),
    ("CSS", include_str!("../assets/syntaxes/CSS.sublime-syntax")),
    (
        "Diff",
        include_str!("../assets/syntaxes/Diff.sublime-syntax"),
    ),
    ("SQL", include_str!("../assets/syntaxes/SQL.sublime-syntax")),
    ("XML", include_str!("../assets/syntaxes/XML.sublime-syntax")),
];

fn bundled_syntax_set() -> SyntaxSet {
    use std::sync::OnceLock;
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(|| {
        let mut builder = SyntaxSetBuilder::new();
        for (name, src) in BUNDLED_SYNTAXES {
            let def = SyntaxDefinition::load_from_str(src, true, Some(name)).unwrap_or_else(|e| {
                panic!("bundled syntax {name} failed to load: {e}");
            });
            builder.add(def);
        }
        builder.add_plain_text_syntax();
        builder.build()
    })
    .clone()
}

fn parse_line_citation_fence_info(info: &str) -> Option<(&str, &str, &str)> {
    let mut it = info.splitn(3, ':');
    let start = it.next()?;
    let end = it.next()?;
    let path = it.next()?;
    if start.is_empty() || !start.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if end.is_empty() || !end.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if path.is_empty() {
        return None;
    }
    Some((start, end, path))
}

/// If pulldown left container indent on continuation lines of a fenced
/// block (first line already stripped, later lines still prefixed), drop
/// that common prefix. Inner relative indent stays.
fn strip_leaked_fence_indent(text: &str) -> String {
    let mut iter = text.split_inclusive('\n');
    let Some(first) = iter.next() else {
        return text.to_string();
    };
    let first_body = first.trim_end_matches(['\n', '\r']);
    if first_body.starts_with(' ') || first_body.starts_with('\t') {
        return text.to_string();
    }
    let rest: Vec<&str> = iter.collect();
    if rest.is_empty() {
        return text.to_string();
    }
    let min_ws = rest
        .iter()
        .map(|line| {
            let body = line.trim_end_matches(['\n', '\r']);
            if body.is_empty() {
                usize::MAX
            } else {
                body.chars().take_while(|c| *c == ' ' || *c == '\t').count()
            }
        })
        .min()
        .unwrap_or(0);
    if min_ws == 0 || min_ws == usize::MAX {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    out.push_str(first);
    for line in rest {
        let body = line.trim_end_matches(['\n', '\r']);
        if body.is_empty() {
            out.push_str(line);
            continue;
        }
        let stripped: String = body.chars().skip(min_ws).collect();
        out.push_str(&stripped);
        if line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Syntax highlight code, returning raw styled segments per line.
///
/// `fence_info` is the fenced code block *info* string (language tag or
/// `lineStart:lineEnd:path` citation form); see
/// [`Syntect::highlight_lines_for_fence_info`]. Lives here (not in `parse`)
/// so both the parser and the streaming highlighter caches depend one-way on
/// `syntax`.
pub(crate) fn syntax_highlight_raw(
    syntect: Option<&Syntect>,
    fence_info: &str,
    text: &str,
) -> Option<Vec<Vec<(syntect::highlighting::Style, String)>>> {
    use syntect::util::LinesWithEndings;

    let syn = syntect?;
    let mut hl = match syn.highlight_lines_for_fence_info(fence_info) {
        Some(hl) => hl,
        None => HighlightLines::new(syn.syntax_set.find_syntax_plain_text(), &syn.theme),
    };
    let text = strip_leaked_fence_indent(text);
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(&text) {
        let highlighted = hl.highlight_line(line, &syn.syntax_set).ok()?;
        lines.push(
            highlighted
                .into_iter()
                .map(|(s, t)| (s, t.to_string()))
                .collect(),
        );
    }
    Some(lines)
}

/// Get a shared Syntect instance for tests.
///
/// This loads the tokyo-night theme bundled with the crate.
/// Uses a static OnceLock for efficiency in test runs.
#[cfg(any(test, fuzzing))]
#[allow(dead_code)]
pub fn test_syntect() -> &'static Syntect {
    use std::sync::OnceLock;
    static TEST_SYNTECT: OnceLock<Syntect> = OnceLock::new();
    TEST_SYNTECT.get_or_init(|| Syntect::new(include_bytes!("../assets/tokyo-night.tmTheme")))
}

#[cfg(test)]
mod tests {
    use super::parse_line_citation_fence_info;

    #[test]
    fn line_citation_fence_parses_start_end_path() {
        assert_eq!(
            parse_line_citation_fence_info("37:65:crates/example/src/tools/read.rs"),
            Some(("37", "65", "crates/example/src/tools/read.rs"))
        );
    }

    #[test]
    fn line_citation_rejects_non_numeric_line() {
        assert_eq!(parse_line_citation_fence_info("37:ab:file.rs"), None);
    }

    #[test]
    fn line_citation_rejects_plain_lang_token() {
        assert_eq!(parse_line_citation_fence_info("rust"), None);
        assert_eq!(parse_line_citation_fence_info(""), None);
    }

    #[test]
    fn highlight_lines_for_fence_info_resolves_citation_path_to_rust() {
        let s = super::test_syntect();
        assert!(
            s.highlight_lines_for_fence_info("37:65:crates/codegen/xai-grok-markdown/src/parse.rs")
                .is_some()
        );
    }

    #[test]
    fn highlight_lines_for_fence_info_still_accepts_rust_token() {
        let s = super::test_syntect();
        assert!(s.highlight_lines_for_fence_info("rust").is_some());
    }

    #[test]
    fn highlight_lines_for_token_json_from_bundled_syntax() {
        let s = super::test_syntect();
        assert!(s.highlight_lines_for_token("json").is_some());
    }
}
