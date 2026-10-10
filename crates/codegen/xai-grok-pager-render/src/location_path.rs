use std::borrow::Cow;

use unicode_width::UnicodeWidthStr;

/// Display-only middle-component shortener for already-abbreviated location paths.
///
/// After a `~` / `$GROK_HOME` prefix (or a leading `/` / drive letter / UNC
/// `\\server\share` / `//host/share` / `\\?\UNC\server\share`), the last two
/// components stay full and earlier ones become one letter. Leading dots are
/// kept plus the first non-dot character (`.grok` → `.g`, `..cache` → `..c`).
/// Literal `.` / `..` stay as-is. Drive-relative `C:foo\bar` does not gain a
/// root separator; rooted `\foo\bar` keeps one. Paths with 0–2 components
/// after the prefix are unchanged. Callers that have not measured a column
/// budget still shorten every earlier component.
pub(crate) fn shorten_location_path(path: &str) -> Cow<'_, str> {
    let Some(parsed) = parse_location_for_shortening(path) else {
        return Cow::Borrowed(path);
    };
    Cow::Owned(render_shortened(&parsed, |index| {
        index >= parsed.keep_unc_root && index + KEEP_FULL < parsed.parts.len()
    }))
}

/// Keep `path` when its display width fits `budget` columns. Otherwise
/// shorten the earliest eligible component, then the next, until the
/// result fits. The last two components stay full. A result that is still
/// wider than `budget` is the fully shortened path. The caller may ellipsis.
pub fn fit_location_path_to_budget(path: &str, budget: usize) -> Cow<'_, str> {
    if path.width() <= budget {
        return Cow::Borrowed(path);
    }
    let Some(parsed) = parse_location_for_shortening(path) else {
        return Cow::Borrowed(path);
    };
    let eligible: Vec<usize> = parsed
        .parts
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            (index >= parsed.keep_unc_root && index + KEEP_FULL < parsed.parts.len())
                .then_some(index)
        })
        .collect();
    for count in 1..=eligible.len() {
        let out = render_shortened(&parsed, |index| {
            eligible.iter().take(count).any(|item| *item == index)
        });
        if out.width() <= budget {
            return Cow::Owned(out);
        }
    }
    Cow::Owned(render_shortened(&parsed, |index| eligible.contains(&index)))
}

const KEEP_FULL: usize = 2;

struct ParsedLocation<'a> {
    prefix: Cow<'a, str>,
    parts: Vec<&'a str>,
    sep: char,
    restore_root: bool,
    unc_style: bool,
    drive_relative: bool,
    keep_unc_root: usize,
}

/// `None` when the path has 0–2 components after the prefix. Those stay
/// the original string, including a trailing separator.
fn parse_location_for_shortening(path: &str) -> Option<ParsedLocation<'_>> {
    const GROK_HOME_PREFIX: &str = "$GROK_HOME";
    const VERBATIM_UNC_PREFIX: &str = r"\\?\UNC\";

    let sep = if path.contains('\\') && !path.contains('/') {
        '\\'
    } else {
        '/'
    };
    let mut drive_chars = path.chars();
    let windows_drive = matches!(drive_chars.next(), Some(c) if c.is_ascii_alphabetic())
        && drive_chars.next() == Some(':');
    // `///foo` is a POSIX path with extra slashes, not `//host/share`.
    let unc_style = (path.starts_with(r"\\") || path.starts_with("//"))
        && path
            .as_bytes()
            .get(2)
            .is_some_and(|b| *b != b'/' && *b != b'\\');
    // Unix filenames may contain `\`. Windows hosts and Windows-style paths still split on it.
    let backslash_is_sep = cfg!(windows)
        || windows_drive
        || path.starts_with(r"\\")
        || (path.contains('\\') && !path.contains('/'));
    let after_drive = path.get(2..).unwrap_or("");
    // `C:foo` is the current directory on that drive; `C:\foo` is rooted on it.
    let drive_relative = windows_drive && !after_drive.starts_with(['/', '\\']);

    let (prefix, remainder, restore_root) = if path == GROK_HOME_PREFIX
        || path
            .strip_prefix(GROK_HOME_PREFIX)
            .is_some_and(|rest| rest.starts_with(['/', '\\']))
    {
        (
            Cow::Borrowed(GROK_HOME_PREFIX),
            path.get(GROK_HOME_PREFIX.len()..)
                .unwrap_or("")
                .trim_start_matches(['/', '\\']),
            false,
        )
    } else if path == "~"
        || path
            .strip_prefix('~')
            .is_some_and(|rest| rest.starts_with(['/', '\\']))
    {
        (
            Cow::Borrowed("~"),
            path.get(1..).unwrap_or("").trim_start_matches(['/', '\\']),
            false,
        )
    } else if windows_drive {
        (
            Cow::Borrowed(path.get(..2).unwrap_or("")),
            after_drive.trim_start_matches(['/', '\\']),
            false,
        )
    } else if unc_style {
        // `\\?\UNC\` is the verbatim marker; server\share start after it, not at `?`/`UNC`.
        // `\\` / `//` / `\\?\UNC\` already end in the separator; the first component attaches.
        let verbatim_unc = path
            .get(..VERBATIM_UNC_PREFIX.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(VERBATIM_UNC_PREFIX));
        let prefix = if verbatim_unc {
            path.get(..VERBATIM_UNC_PREFIX.len())
                .unwrap_or(VERBATIM_UNC_PREFIX)
        } else if path.starts_with(r"\\") {
            r"\\"
        } else {
            "//"
        };
        (
            Cow::Borrowed(prefix),
            path.get(prefix.len()..)
                .unwrap_or("")
                .trim_start_matches(['/', '\\']),
            false,
        )
    } else if path.starts_with('/') || (backslash_is_sep && path.starts_with('\\')) {
        (
            Cow::Borrowed(""),
            path.trim_start_matches(['/', '\\']),
            true,
        )
    } else {
        (Cow::Borrowed(""), path, false)
    };

    let parts: Vec<&str> = remainder
        .split(|c| c == '/' || (backslash_is_sep && c == '\\'))
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() <= KEEP_FULL {
        return None;
    }

    // UNC: first two components are `server\share` and stay full in the same loop.
    let keep_unc_root = if unc_style { 2 } else { 0 };
    Some(ParsedLocation {
        prefix,
        parts,
        sep,
        restore_root,
        unc_style,
        drive_relative,
        keep_unc_root,
    })
}

fn render_shortened(parsed: &ParsedLocation<'_>, shorten: impl Fn(usize) -> bool) -> String {
    let mut out = String::with_capacity(
        parsed.prefix.len()
            + parsed
                .parts
                .iter()
                .map(|part| part.len() + 1)
                .sum::<usize>(),
    );
    if !parsed.prefix.is_empty() {
        out.push_str(parsed.prefix.as_ref());
    }
    for (index, part) in parsed.parts.iter().enumerate() {
        // Drive-relative `C:foo` and UNC `\\server` attach the first name to the prefix.
        let attach_without_sep = (parsed.unc_style || parsed.drive_relative) && index == 0;
        if parsed.restore_root || (!out.is_empty() && !attach_without_sep) {
            out.push(parsed.sep);
        }
        if shorten(index) {
            out.push_str(shorten_location_component(part));
        } else {
            out.push_str(part);
        }
    }
    out
}

fn shorten_location_component(component: &str) -> &str {
    // Leading dots plus the first non-dot so `..cache` is `..c`, not traversal `..`.
    let mut end = 0;
    for (i, c) in component.char_indices() {
        end = i + c.len_utf8();
        if c != '.' {
            break;
        }
    }
    component.get(..end).unwrap_or("")
}

#[cfg(test)]
#[path = "location_path_tests.rs"]
mod tests;
