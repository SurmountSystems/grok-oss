//! In-app how-to documentation data (embedded markdown).
//!
//! Single source of truth: two static arrays (`USER_GUIDE`, `REFERENCE_DOCS`)
//! hold every doc. All lookups are zero-allocation; `DocEntry` exists only for
//! backward compatibility with the TUI doc picker.

/// A compile-time document entry. All fields are `&'static str`.
#[derive(Debug)]
pub struct Doc {
    pub filename: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

/// Owned variant for the TUI doc picker (backward compat).
#[derive(Debug, Clone)]
pub struct DocEntry {
    pub title: String,
    pub description: String,
    /// Embedded markdown content.
    pub content: &'static str,
}

impl From<&Doc> for DocEntry {
    fn from(d: &Doc) -> Self {
        Self {
            title: d.title.into(),
            description: d.description.into(),
            content: d.content,
        }
    }
}

// ── Static doc tables ────────────────────────────────────────────────────────

macro_rules! guide {
    ($file:literal, $title:literal, $desc:literal) => {
        Doc {
            filename: $file,
            title: $title,
            description: $desc,
            content: include_str!(concat!("../docs/user-guide/", $file)),
        }
    };
}

pub static USER_GUIDE: &[Doc] = &[
    guide!(
        "01-getting-started.md",
        "Getting Started",
        "Installation, first launch, and basic interaction"
    ),
    guide!(
        "02-authentication.md",
        "Authentication",
        "Browser login, API keys, OIDC, external auth providers"
    ),
    guide!(
        "03-keyboard-shortcuts.md",
        "Keyboard Shortcuts",
        "Complete reference for all TUI key bindings"
    ),
    guide!(
        "04-slash-commands.md",
        "Slash Commands",
        "All / commands, including goals, research, and workflow management"
    ),
    guide!(
        "05-configuration.md",
        "Configuration",
        "config.toml, pager.toml, environment variables, file locations"
    ),
    guide!(
        "06-theming.md",
        "Theming and Appearance",
        "Themes, color support, pager.toml customization"
    ),
    guide!(
        "07-mcp-servers.md",
        "MCP Servers",
        "Setting up external tool integrations via MCP"
    ),
    guide!(
        "08-skills.md",
        "Skills",
        "Creating and using reusable prompt packages"
    ),
    guide!(
        "09-plugins.md",
        "Plugins and Marketplace",
        "Installing, managing, and creating plugin packages"
    ),
    guide!(
        "10-hooks.md",
        "Hooks",
        "Project lifecycle scripts for pre/post tool-use events"
    ),
    guide!(
        "11-custom-models.md",
        "Custom Models",
        "BYOK, Ollama, OpenAI-compatible endpoints"
    ),
    guide!(
        "12-project-rules.md",
        "Project Rules (AGENTS.md)",
        "Per-directory instructions and precedence rules"
    ),
    guide!(
        "13-memory.md",
        "Memory",
        "Cross-session knowledge persistence and search"
    ),
    guide!(
        "14-headless-mode.md",
        "Headless Mode and Scripting",
        "Non-interactive CLI for automation and CI/CD"
    ),
    guide!(
        "15-agent-mode.md",
        "Agent Mode and IDE Integration",
        "ACP stdio transport, WebSocket relay, SDK integration"
    ),
    guide!(
        "16-subagents.md",
        "Subagents and Personas",
        "Spawning parallel child agents with specialized roles"
    ),
    guide!(
        "17-sessions.md",
        "Session Management",
        "Save, load, resume, rewind, and compact sessions"
    ),
    guide!(
        "18-sandbox.md",
        "Sandbox Mode",
        "OS-level filesystem and network isolation"
    ),
    guide!(
        "19-plan-mode.md",
        "Plan Mode",
        "Structured planning with approval dialogs"
    ),
    guide!(
        "20-background-tasks.md",
        "Background Tasks and Monitoring",
        "Background commands, /loop, monitor, scheduler"
    ),
    guide!(
        "21-terminal-support.md",
        "Terminal Support and Troubleshooting",
        "tmux, Byobu, Zellij, SSH, truecolor, clipboard, and diagnostics"
    ),
    guide!(
        "22-permissions-and-safety.md",
        "Permissions and Safety",
        "Modes, authorization order, allow/ask/deny rules, matching, and hooks"
    ),
    guide!(
        "23-dashboard.md",
        "Agent Dashboard",
        "Live multi-session roster: peek, dispatch, pin, stop, and search"
    ),
    guide!(
        "24-monitoring-usage.md",
        "Monitoring Usage (External OpenTelemetry)",
        "Export usage metrics to a customer OpenTelemetry collector"
    ),
];

/// Non-user-guide reference docs. Separate from USER_GUIDE because they
/// live under `docs/` (not `docs/user-guide/`), are not extracted to disk,
/// and do not follow the NN-*.md managed naming pattern. Bundled via
/// `include_str!` so they are available at runtime without a docs path.
static REFERENCE_DOCS: &[Doc] = &[
    Doc {
        filename: "hooks-and-plugins.md",
        title: "Hooks & Plugins Guide",
        description: "Using hooks, plugins, and marketplace",
        content: include_str!("../docs/hooks-and-plugins.md"),
    },
    Doc {
        filename: "custom-hooks.md",
        title: "Creating Custom Hooks",
        description: "Writing your own hooks and matchers",
        content: include_str!("../docs/custom-hooks.md"),
    },
];

// ── Public API ───────────────────────────────────────────────────────────────

/// Find a doc by title (case-insensitive). Returns the static entry.
pub fn find_doc(title: &str) -> Option<&'static Doc> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .find(|d| d.title.eq_ignore_ascii_case(title))
}

/// All doc titles, zero allocation.
pub fn all_titles() -> impl Iterator<Item = &'static str> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .map(|d| d.title)
}

/// Returns the content of a how-to document by exact title match (case-insensitive).
pub fn get_howto_doc(title: &str) -> Option<&'static str> {
    find_doc(title).map(|d| d.content)
}

/// Returns a list of available how-to titles for the model to choose from.
pub fn list_howto_titles() -> Vec<String> {
    all_titles().map(String::from).collect()
}

/// Returns all docs as owned `DocEntry` values for the TUI doc picker.
pub fn default_howto_entries() -> Vec<DocEntry> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .map(DocEntry::from)
        .collect()
}

/// Extract user-guide docs to `<grok_home>/docs/user-guide/`.
///
/// Called from the pager binary startup so the model can read them from disk.
pub fn extract_user_guide_docs(grok_home: &std::path::Path) {
    let docs_dir = grok_home.join("docs").join("user-guide");
    if let Err(e) = std::fs::create_dir_all(&docs_dir) {
        tracing::warn!(error = %e, "Failed to create user-guide docs directory");
        return;
    }
    for doc in USER_GUIDE {
        if let Err(e) = std::fs::write(docs_dir.join(doc.filename), doc.content) {
            tracing::debug!(error = %e, filename = doc.filename, "Failed to extract user-guide doc");
        }
    }
    // Clean up stale managed docs (files removed from USER_GUIDE since last run).
    // Only remove files matching the managed naming pattern (NN-*.md).
    if let Ok(entries) = std::fs::read_dir(&docs_dir) {
        let valid: std::collections::HashSet<&str> =
            USER_GUIDE.iter().map(|d| d.filename).collect();
        for dir_entry in entries.flatten() {
            if let Some(name) = dir_entry.file_name().to_str() {
                let is_managed = name.len() > 3
                    && name.as_bytes()[0].is_ascii_digit()
                    && name.as_bytes()[1].is_ascii_digit()
                    && name.as_bytes()[2] == b'-'
                    && name.ends_with(".md");
                if is_managed
                    && !valid.contains(name)
                    && let Err(e) = std::fs::remove_file(dir_entry.path())
                {
                    tracing::debug!(error = %e, filename = name, "Failed to remove stale user-guide doc");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_guide_entries_are_valid() {
        for doc in USER_GUIDE {
            assert!(!doc.content.is_empty(), "Doc {} is empty", doc.filename);
            assert!(
                !doc.title.is_empty(),
                "Doc {} has empty title",
                doc.filename
            );
            assert!(
                !doc.description.is_empty(),
                "Doc {} has empty description",
                doc.filename
            );
            assert!(
                doc.content.starts_with('#'),
                "Doc {} should start with a markdown header",
                doc.filename
            );
        }
    }

    #[test]
    fn user_guide_entries_have_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for doc in USER_GUIDE {
            assert!(
                seen.insert(doc.filename),
                "Duplicate doc in list: {}",
                doc.filename
            );
        }
    }

    #[test]
    fn default_howto_entries_includes_all_user_guide_docs() {
        let entries = default_howto_entries();
        assert_eq!(entries.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
        for (i, doc) in USER_GUIDE.iter().enumerate() {
            assert_eq!(entries[i].title, doc.title, "Entry {} title mismatch", i);
        }
    }

    #[test]
    fn find_doc_is_case_insensitive() {
        let doc = find_doc("getting started").expect("should find Getting Started");
        assert_eq!(doc.title, "Getting Started");
        assert!(find_doc("nonexistent guide").is_none());
    }

    #[test]
    fn all_titles_covers_both_tables() {
        let titles: Vec<_> = all_titles().collect();
        assert_eq!(titles.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
    }

    #[test]
    fn get_howto_doc_delegates_to_find_doc() {
        assert!(get_howto_doc("Getting Started").is_some());
        assert!(get_howto_doc("Hooks & Plugins Guide").is_some());
        assert!(get_howto_doc("no such doc").is_none());
    }

    #[test]
    fn list_howto_titles_returns_all() {
        let titles = list_howto_titles();
        assert_eq!(titles.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
    }

    #[test]
    fn extract_writes_docs_and_cleans_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let docs_dir = tmp.path().join("docs").join("user-guide");

        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("99-removed.md"), "stale").unwrap();
        std::fs::write(docs_dir.join("notes.md"), "user notes").unwrap();

        extract_user_guide_docs(tmp.path());

        for doc in USER_GUIDE {
            let path = docs_dir.join(doc.filename);
            assert!(path.exists(), "Expected doc {} to exist", doc.filename);
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                doc.content,
                "Content mismatch for {}",
                doc.filename
            );
        }
        assert!(
            !docs_dir.join("99-removed.md").exists(),
            "Stale doc should be cleaned up"
        );
        assert!(
            docs_dir.join("notes.md").exists(),
            "User file should not be deleted"
        );
    }

    /// Named contract: after included SuperGrok period limits are full, dual-auth
    /// hop is shipped (SuperGrok dollar credits, then console failover). The
    /// embedded user-guide must not say that hop is unshipped.
    #[test]
    fn user_guide_does_not_claim_automatic_host_hop_is_unshipped() {
        for doc in USER_GUIDE {
            let claims_unshipped = doc.content.contains("not a shipped automatic hop")
                || doc.content.contains("**not** a shipped automatic hop")
                || doc.content.contains("is **not** shipped on this restack")
                || (doc.content.contains("Automatic hop")
                    && doc.content.contains("**not** shipped"))
                || (doc.content.contains("Automatic host hop")
                    && doc.content.contains("**not** shipped"));
            assert!(
                !claims_unshipped,
                "{} still claims automatic host hop after included SuperGrok period limits are full is unshipped",
                doc.filename
            );
        }

        let auth = USER_GUIDE
            .iter()
            .find(|d| d.filename == "02-authentication.md")
            .expect("02-authentication.md is embedded");
        let slash = USER_GUIDE
            .iter()
            .find(|d| d.filename == "04-slash-commands.md")
            .expect("04-slash-commands.md is embedded");
        for (name, content) in [
            ("02-authentication.md", auth.content),
            ("04-slash-commands.md", slash.content),
        ] {
            assert!(
                content.contains("while included SuperGrok period limits still have room")
                    || content.contains("While included SuperGrok period limits still have room"),
                "{name} must say stay on SuperGrok while included SuperGrok period limits have room"
            );
            assert!(
                content.contains("After those included SuperGrok period limits are full"),
                "{name} must describe hop after included SuperGrok period limits are full"
            );
            assert!(
                content.contains("SuperGrok dollar credits") && content.contains("console"),
                "{name} must name SuperGrok dollar credits then console failover"
            );
            assert!(
                !content.contains("free SuperGrok"),
                "{name} must not call SuperGrok free"
            );
        }
    }

    /// Named contract: user-guide spend-order sentences match source
    /// (personal SuperGrok paying JWT first, Team JWT omitted while personal
    /// exists, combined remaining, one fetcher).
    #[test]
    fn user_guide_names_token_economy_spend_order() {
        let auth = USER_GUIDE
            .iter()
            .find(|d| d.filename == "02-authentication.md")
            .expect("02-authentication.md is embedded");
        let slash = USER_GUIDE
            .iter()
            .find(|d| d.filename == "04-slash-commands.md")
            .expect("04-slash-commands.md is embedded");
        for (name, content) in [
            ("02-authentication.md", auth.content),
            ("04-slash-commands.md", slash.content),
        ] {
            assert!(
                content.contains(
                    "spend included SuperGrok period limits on a stored personal SuperGrok login first"
                ),
                "{name} must spend personal included SuperGrok period limits first"
            );
            assert!(
                content.contains("A Team / Business SuperGrok JWT is not the paying source"),
                "{name} must say a Team / Business SuperGrok JWT is not the paying source while a personal login exists"
            );
            assert!(
                content.contains("SuperGrok dollar credits that never expire"),
                "{name} must name SuperGrok dollar credits that never expire"
            );
            assert!(
                content.contains("console team prepaid / console API credits"),
                "{name} must put console team prepaid / console API credits last"
            );
            assert!(
                content.contains(
                    "Remaining included SuperGrok period limits across distinct stored plans are added together"
                ),
                "{name} must say remaining included SuperGrok period limits are added together"
            );
            assert!(
                content.contains("That sum is the real remaining included quota"),
                "{name} must say the sum is the real remaining included quota"
            );
            assert!(
                content.contains("unified pool") && content.contains("counts once"),
                "{name} must say a unified pool counts once"
            );
            assert!(
                content.contains("Only one `grok-oss` process fetches"),
                "{name} must say only one grok-oss process fetches billing and limits"
            );
            assert!(
                content.contains("snapshot under `$GROK_HOME`"),
                "{name} must say other live TUIs read a snapshot under $GROK_HOME"
            );
            assert!(
                content.contains("There is no extra daemon"),
                "{name} must say there is no extra daemon"
            );
            assert!(
                content.contains("Rebuild SIGUSR1 is not this"),
                "{name} must say rebuild SIGUSR1 is not the limits snapshot"
            );
            assert!(
                content.contains("second `grok-oss login` that stores the Team principal"),
                "{name} must say a second SuperGrok plan needs a second grok-oss login"
            );
            assert!(
                content.contains("grok.com's account switcher is a different product"),
                "{name} must say grok.com's account switcher is a different product"
            );
            assert!(
                !content.contains("free SuperGrok"),
                "{name} must not call SuperGrok free"
            );
        }
    }

    /// Named contract: `/limits` user-guide keeps fail-open plus named
    /// commands. grok-oss limits is a client printout, not xAI billing truth.
    #[test]
    fn user_guide_limits_names_fail_open_and_named_commands() {
        let auth = USER_GUIDE
            .iter()
            .find(|d| d.filename == "02-authentication.md")
            .expect("02-authentication.md is embedded");
        let slash = USER_GUIDE
            .iter()
            .find(|d| d.filename == "04-slash-commands.md")
            .expect("04-slash-commands.md is embedded");
        for (name, content) in [
            ("02-authentication.md", auth.content),
            ("04-slash-commands.md", slash.content),
        ] {
            assert!(
                content.contains("stay-supergrok"),
                "{name} must name stay-supergrok"
            );
            assert!(
                content.contains("use-console"),
                "{name} must name use-console"
            );
            assert!(
                content.contains("limits_pins.json"),
                "{name} must name the limits_pins.json sidecar"
            );
            assert!(
                content.contains("not xAI billing truth"),
                "{name} must say grok-oss limits is not xAI billing truth"
            );
            assert!(
                content.contains("must not mark SuperGrok used up"),
                "{name} must say a client 100% / remaining 0 / $0 printout must not mark SuperGrok used up"
            );
            assert!(
                !content.contains("free SuperGrok"),
                "{name} must not call SuperGrok free"
            );
        }
        assert!(
            slash.content.contains("meter included")
                && slash.content.contains("dollar-credits")
                && slash.content.contains("refresh"),
            "04-slash-commands.md must name meter included|dollar-credits|console|combined and refresh"
        );
        assert!(
            slash.content.contains("preferred_method") && slash.content.contains("api_key"),
            "04-slash-commands.md must say stock preferred_method = api_key still pins console"
        );
        assert!(
            slash.content.contains("does not require console credits"),
            "04-slash-commands.md must say hop-back does not require console credits"
        );
        let auth_lower = auth.content.to_ascii_lowercase();
        assert!(
            auth_lower.contains("fetches the console.x.ai billing credits card")
                && auth_lower.contains("prepaidcredits")
                && auth_lower.contains("prepaidcreditsused")
                && auth_lower.contains("postpaid/invoice/preview"),
            "02-authentication.md must say grok-oss fetches GetAmountToPay remaining"
        );
        assert!(
            auth_lower.contains("total.val")
                && auth_lower.contains("prepaidbalance.val")
                && auth_lower.contains("does not hop sampling from this card"),
            "02-authentication.md must keep total.val / prepaidBalance.val distinct and not hop from the card"
        );
        assert!(
            !auth_lower.contains("does not fetch the console.x.ai billing credits card"),
            "02-authentication.md must not claim grok-oss never fetches the Billing Credits card"
        );
        assert!(
            !auth_lower.contains("billing credits card is console team prepaid")
                && !auth_lower.contains("billing credits card is supergrok dollar credits"),
            "02-authentication.md must not classify the Billing Credits card as another meter"
        );
    }

    /// Named contract: product skills are not a Python runtime. Restack must
    /// not drop this from user-guide `08-skills.md`.
    #[test]
    fn user_guide_skills_are_not_a_python_runtime() {
        let skills = USER_GUIDE
            .iter()
            .find(|d| d.filename == "08-skills.md")
            .expect("08-skills.md is embedded");
        assert!(
            skills.content.contains("not a Python runtime"),
            "08-skills.md must say product skills are not a Python runtime"
        );
        assert!(
            skills.content.contains("must not add `.py` helpers")
                || skills.content.contains("must not add .py helpers"),
            "08-skills.md must tell agents not to add .py helpers"
        );
        assert!(
            skills.content.contains("implement/scripts/memory.py")
                && skills.content.contains("validate-plan.py")
                && skills.content.contains("session_reader.py"),
            "08-skills.md must name the allowlisted intercept CLI forms"
        );
        assert!(
            skills.content.contains("docx")
                && skills.content.contains("pptx")
                && skills.content.contains("xlsx")
                && skills.content.contains("pdf"),
            "08-skills.md must name the office/PDF exception"
        );
        assert!(
            skills.content.contains("default Grok OSS skill")
                || skills.content.contains("default product skills"),
            "08-skills.md must say polish/subagent are default Grok OSS skills"
        );
        assert!(
            skills
                .content
                .contains("crates/codegen/xai-grok-bundle/skills/")
                && skills.content.contains("/polish")
                && skills.content.contains("/subagent"),
            "08-skills.md must name the in-tree default skill source and /polish /subagent"
        );
        assert!(
            skills
                .content
                .contains("not project packs at `.agents/skills/polish/`")
                || skills
                    .content
                    .contains("Do not ship them as project `.agents/skills/polish`"),
            "08-skills.md must not treat polish/subagent as project-only packs"
        );
        let slash = USER_GUIDE
            .iter()
            .find(|d| d.filename == "04-slash-commands.md")
            .expect("04-slash-commands.md is embedded");
        assert!(
            slash.content.contains("default Grok OSS skill")
                && slash
                    .content
                    .contains("crates/codegen/xai-grok-bundle/skills/polish/")
                && slash
                    .content
                    .contains("crates/codegen/xai-grok-bundle/skills/subagent/"),
            "04-slash-commands.md must describe /polish and /subagent as default Grok OSS skills"
        );
        assert!(
            !slash
                .content
                .contains("version-controlled **repo skill** at `.agents/skills/polish"),
            "04-slash-commands.md must not call /polish a project repo skill"
        );
    }

    /// Named contract: operator-facing resume / `--version` examples use
    /// `grok-oss`, never upstream `grok`.
    #[test]
    fn user_guide_resume_and_version_examples_use_grok_oss() {
        let getting_started = USER_GUIDE
            .iter()
            .find(|d| d.filename == "01-getting-started.md")
            .expect("01-getting-started.md is embedded");
        assert!(
            getting_started.content.contains("grok-oss --resume"),
            "01-getting-started must show grok-oss --resume"
        );
        assert!(
            getting_started.content.contains("grok-oss --version"),
            "01-getting-started must show grok-oss --version"
        );
        assert!(
            getting_started.content.contains("grok-oss --yolo"),
            "01-getting-started must not tell operators to run grok --yolo"
        );
        for doc in USER_GUIDE {
            assert!(
                !doc.content.contains("grok --resume"),
                "{} must not tell operators to run grok --resume",
                doc.filename
            );
            assert!(
                !doc.content.contains("grok --version"),
                "{} must not tell operators to run grok --version",
                doc.filename
            );
            assert!(
                !doc.content.contains("grok --yolo"),
                "{} must not tell operators to run grok --yolo",
                doc.filename
            );
            assert!(
                !doc.content.contains("grok --continue"),
                "{} must not tell operators to run grok --continue",
                doc.filename
            );
        }
    }

    /// Named contract (G1): user-guide 19 idle CTAs are Approve / Comment /
    /// Revise / Exit. Clarify is the comment-flow action. Letter A types.
    /// Notes (`A`) is gone. Empty `a` does not Approve.
    #[test]
    fn user_guide_plan_mode_ctas_are_approve_clarify_revise_exit() {
        let plan = USER_GUIDE
            .iter()
            .find(|d| d.filename == "19-plan-mode.md")
            .expect("19-plan-mode.md is embedded");
        let content = plan.content;
        assert!(
            content.contains("**Approve**")
                && content.contains("**Comment**")
                && content.contains("**Clarify**")
                && content.contains("**Revise**")
                && content.contains("**Exit**"),
            "19-plan-mode.md must name Approve, Comment, Clarify, Revise, and Exit"
        );
        assert!(
            content.contains("comment composer") || content.contains("Comment**"),
            "19-plan-mode.md must teach Comment as the idle notes entry"
        );
        assert!(
            !content.contains("Approve with notes") && !content.contains("Notes (`A`)"),
            "19-plan-mode.md must not keep Notes (`A`) as a CTA"
        );
        assert!(
            !content.contains("empty-prompt `a`") && !content.contains("empty-prompt a"),
            "19-plan-mode.md must not say empty-prompt a Approves"
        );
        assert!(
            content.contains("also") || content.contains("type"),
            "19-plan-mode.md must say letters type into the prompt while review is open"
        );
        assert!(
            content.contains("--legacy"),
            "19-plan-mode.md must keep the questionnaire on --legacy only"
        );
        assert!(
            content.contains("Empty `Enter`") || content.contains("Empty Enter"),
            "19-plan-mode.md must still say empty Enter never Approves"
        );
    }

    /// Named contract: implement-loop effort in user-guide `05-configuration`
    /// is thoroughness. It is not reviewer fan-out and not how many Review
    /// rows to launch.
    #[test]
    fn user_guide_implement_effort_is_thoroughness_not_reviewer_fan_out() {
        let config = USER_GUIDE
            .iter()
            .find(|d| d.filename == "05-configuration.md")
            .expect("05-configuration.md is embedded");
        assert!(
            !config.content.contains("reviewer fan-out"),
            "05-configuration.md must not say reviewer fan-out"
        );
        assert!(
            !config.content.contains("always-a-reviewer"),
            "05-configuration.md must not say always-a-reviewer"
        );
        assert!(
            config
                .content
                .contains("not how many Review rows to launch"),
            "05-configuration.md must say implement effort is not how many Review rows to launch"
        );
    }

    /// Named contract: leftover operator-facing CLI examples for this tree
    /// use `grok-oss`, not bare `grok sessions` / `grok login` / `grok mcp add`
    /// and similar operator commands. Official xAI `grok` product mentions and
    /// `~/.grok` paths are not this contract.
    #[test]
    fn user_guide_operator_cli_examples_use_grok_oss() {
        const FORBIDDEN: &[&str] = &[
            "grok sessions",
            "grok login",
            "grok logout",
            "grok mcp ",
            "grok mcp`",
            "grok inspect",
            "grok doctor",
            "grok plugin ",
            "grok plugin`",
            "grok memory",
            "grok dashboard",
            "grok wrap",
            "grok agent",
            "grok models",
            "grok workspace",
            "grok worktree",
            "grok setup",
            "grok du",
            "grok disk-usage",
            "grok -p",
            "grok -w",
            "grok --",
        ];
        for doc in USER_GUIDE {
            for stem in FORBIDDEN {
                assert!(
                    !doc.content.contains(stem),
                    "{} must not tell operators to run `{stem}`; use grok-oss for this tree",
                    doc.filename
                );
            }
        }

        let sessions = USER_GUIDE
            .iter()
            .find(|d| d.filename == "17-sessions.md")
            .expect("17-sessions.md is embedded");
        assert!(
            sessions.content.contains("grok-oss sessions"),
            "17-sessions must show grok-oss sessions"
        );

        let auth = USER_GUIDE
            .iter()
            .find(|d| d.filename == "02-authentication.md")
            .expect("02-authentication.md is embedded");
        assert!(
            auth.content.contains("grok-oss login"),
            "02-authentication must show grok-oss login"
        );

        let mcp = USER_GUIDE
            .iter()
            .find(|d| d.filename == "07-mcp-servers.md")
            .expect("07-mcp-servers.md is embedded");
        assert!(
            mcp.content.contains("grok-oss mcp add"),
            "07-mcp-servers must show grok-oss mcp add"
        );
    }
}
