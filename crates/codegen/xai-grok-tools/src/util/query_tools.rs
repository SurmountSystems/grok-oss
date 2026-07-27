//! `$PATH`-aware helper for steering messages that suggest shell tools.
//!
//! Hints that recommend concrete binaries (`jq`, `sed`, `cut`, …) must only
//! name tools that actually exist on the tool server, with no "if available"
//! hedge. Consumers call [`QueryTools::detect`] once and build an example
//! clause via [`examples_clause`]; when nothing relevant is installed the
//! clause is empty so the surrounding hint reads cleanly.
//!
//! **No `python3`.** Recovery/dump/edit steers must not train the model to
//! shell Python for maintainer workflows — prefer native tools
//! (`read_file` / `grep` / `search_replace`) and only name lightweight
//! shell utilities when line-oriented tools cannot help (long single-line
//! JSON/text). User-project Python is unrelated and stays allowed in shell
//! policy.
//!
//! Shared by the `use_tool` MCP-dump steer, web_fetch overflow, and related
//! recovery hints.

/// Query tools present on the tool server's `$PATH`, each `Some(name)` when
/// detected; see [`xai_grok_config::shell::is_command_available`].
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct QueryTools {
    /// `jq`, if present.
    pub(crate) jq: Option<&'static str>,
    /// `sed`, if present.
    pub(crate) sed: Option<&'static str>,
    /// `cut`, if present.
    pub(crate) cut: Option<&'static str>,
}

impl QueryTools {
    /// Probe `$PATH` for the tools the steer may suggest; resolved once.
    pub(crate) fn detect() -> Self {
        use std::sync::OnceLock;
        use xai_grok_config::shell::is_command_available;
        static DETECTED: OnceLock<QueryTools> = OnceLock::new();
        *DETECTED.get_or_init(|| {
            let present = |name: &'static str| is_command_available(name).then_some(name);
            Self {
                jq: present("jq"),
                sed: present("sed"),
                cut: present("cut"),
            }
        })
    }

    /// Backtick-wrapped tools for querying structured JSON, preference order.
    pub(crate) fn json_tools(self) -> Vec<String> {
        Self::wrap([self.jq])
    }

    /// Backtick-wrapped tools for slicing/searching a long-line text file.
    pub(crate) fn text_tools(self) -> Vec<String> {
        Self::wrap([self.sed, self.cut])
    }

    /// Backtick-wrap the tools that are present, dropping absent ones.
    fn wrap(tools: impl IntoIterator<Item = Option<&'static str>>) -> Vec<String> {
        tools
            .into_iter()
            .flatten()
            .map(|t| format!("`{t}`"))
            .collect()
    }
}

/// `" (e.g. `jq` or `sed`)"` for the present tools, or `""` when none were
/// detected — so a steer never names a tool that isn't installed.
pub(crate) fn examples_clause(tools: &[String]) -> String {
    match tools {
        [] => String::new(),
        [a] => format!(" (e.g. {a})"),
        [a, b] => format!(" (e.g. {a} or {b})"),
        [rest @ .., last] => format!(" (e.g. {}, or {last})", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> QueryTools {
        QueryTools {
            jq: Some("jq"),
            sed: Some("sed"),
            cut: Some("cut"),
        }
    }

    #[test]
    fn examples_clause_formats_lists() {
        assert_eq!(examples_clause(&[]), "");
        assert_eq!(examples_clause(&["`jq`".into()]), " (e.g. `jq`)");
        assert_eq!(
            examples_clause(&["`jq`".into(), "`sed`".into()]),
            " (e.g. `jq` or `sed`)"
        );
        assert_eq!(
            examples_clause(&["`sed`".into(), "`cut`".into(), "`awk`".into()]),
            " (e.g. `sed`, `cut`, or `awk`)"
        );
    }

    /// Membership and preference order per tool set; absent tools are dropped
    /// (these are the invariants every consumer steer relies on).
    #[test]
    fn tool_sets_membership_and_order() {
        assert_eq!(all().json_tools(), vec!["`jq`"]);
        assert_eq!(all().text_tools(), vec!["`sed`", "`cut`"]);

        let partial = QueryTools {
            jq: None,
            sed: Some("sed"),
            cut: Some("cut"),
        };
        assert_eq!(partial.json_tools(), Vec::<String>::new());
        assert_eq!(partial.text_tools(), vec!["`sed`", "`cut`"]);

        let none = QueryTools::default();
        assert!(none.json_tools().is_empty());
        assert!(none.text_tools().is_empty());
    }

    /// Python must never appear in recovery/query example clauses (A1 steer
    /// demotion). Shell Python is still fine for user-project work; steers
    /// must not recommend it for dump/edit recovery.
    #[test]
    fn steers_never_name_python() {
        // detect() must not probe python binaries (field list is jq/sed/cut).
        let detect_src = include_str!("query_tools.rs");
        let detect_fn = detect_src
            .split("fn detect()")
            .nth(1)
            .and_then(|s| s.split("fn json_tools").next())
            .expect("detect body");
        assert!(
            !detect_fn.contains("python"),
            "detect() must not probe python: {detect_fn}"
        );
        for tools in [all(), QueryTools::default()] {
            let joined = format!(
                "{}{}",
                tools.json_tools().join(" "),
                tools.text_tools().join(" ")
            );
            assert!(
                !joined.contains("python"),
                "tool sets must not include python: {joined}"
            );
        }
    }
}
