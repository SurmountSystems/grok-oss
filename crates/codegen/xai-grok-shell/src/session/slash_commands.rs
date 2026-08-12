//! ACP slash command advertising and resolution.

use agent_client_protocol as acp;
use xai_grok_tools::implementations::skills::skill::format_skill_name;
use xai_grok_tools::implementations::skills::types::SkillInfo;

/// A built-in slash command.
pub(crate) struct BuiltinCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub argument_hint: Option<&'static str>,
    pub aliases: &'static [&'static str],
    /// Capability the agent must have for this command to be useful.
    /// Filtered by `CommandAvailability::allows()` at advertising time;
    /// commands that map to `BuiltinGate::AlwaysOn` are never gated.
    pub gate: BuiltinGate,
    resolve: fn(args: &str) -> BuiltinAction,
}

/// Capability gate that decides whether a `BuiltinCommand` is advertised
/// and resolvable in a given session.
///
/// Each variant maps to a feature/tool the agent must actually have:
/// - `Memory`: a memory backend is configured (`SessionMemory::is_enabled`).
/// - `Scheduler`: `scheduler_create` is registered.
/// - `Hooks`: a hook registry is loaded.
/// - `Plugins`: a plugin registry is loaded.
/// - `Feedback`: the feedback manager is enabled.
/// - `MemoryConfigured`: memory backend params exist (may be currently
///   disabled). Used for `/memory` so the user can re-enable via toggle.
/// - `Goal`: `resolve_goal()` feature flag is on AND `update_goal` is in the
///   session toolset (see `goal_slash_and_harness_available` in `acp_session.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinGate {
    AlwaysOn,
    Feedback,
    Memory,
    MemoryConfigured,
    /// Checks `scheduler_create` only. If any future shell-side builtin
    /// needs a separate scheduler-delete gate, add a `SchedulerDelete` variant.
    Scheduler,
    Hooks,
    Plugins,
    Goal,
}

/// All built-in slash commands. Order here = display order in autocomplete.
pub(super) const BUILTIN_COMMANDS: &[BuiltinCommand] = &[
    BuiltinCommand {
        name: "compact",
        description: "Compress conversation history to save context window",
        argument_hint: Some("optional context about what to preserve"),
        aliases: &[],
        gate: BuiltinGate::AlwaysOn,
        resolve: |args| BuiltinAction::Compact {
            user_context: if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        },
    },
    BuiltinCommand {
        name: "always-approve",
        description: "Toggle always-approve mode (skip all permission prompts)",
        argument_hint: Some("on|off"),
        aliases: &["yolo"],
        gate: BuiltinGate::AlwaysOn,
        resolve: |args| BuiltinAction::SetYolo {
            enabled: !matches!(
                args.to_lowercase().as_str(),
                "off" | "false" | "0" | "no" | "disable"
            ),
        },
    },
    BuiltinCommand {
        name: "flush",
        description: "Flush conversation memory to disk now",
        argument_hint: None,
        aliases: &[],
        gate: BuiltinGate::Memory,
        resolve: |_args| BuiltinAction::FlushMemory,
    },
    BuiltinCommand {
        name: "dream",
        description: "Run memory consolidation (merge session logs into organized topics)",
        argument_hint: None,
        aliases: &[],
        gate: BuiltinGate::Memory,
        resolve: |_args| BuiltinAction::Dream,
    },
    BuiltinCommand {
        name: "memory",
        description: "Browse, view, and manage your memories",
        argument_hint: Some("on|off"),
        aliases: &["mem"],
        gate: BuiltinGate::MemoryConfigured,
        resolve: |args| {
            let trimmed = args.trim().to_lowercase();
            match trimmed.as_str() {
                "on" | "enable" => BuiltinAction::MemoryToggle { enabled: true },
                "off" | "disable" => BuiltinAction::MemoryToggle { enabled: false },
                _ => BuiltinAction::MemoryBrowse,
            }
        },
    },
    BuiltinCommand {
        name: "context",
        description: "Show context window usage and session stats",
        argument_hint: None,
        aliases: &[],
        gate: BuiltinGate::AlwaysOn,
        resolve: |_args| BuiltinAction::ContextInfo,
    },
    BuiltinCommand {
        name: "economic-mode",
        description: "Cap context at 200K for cheaper Grok 4.5 pricing; clamps auto /implement --effort to 1 (on by default)",
        argument_hint: Some("on|off|status|global on|global off"),
        aliases: &["economic", "econ"],
        gate: BuiltinGate::AlwaysOn,
        resolve: |args| {
            let trimmed = args.trim().to_lowercase();
            match trimmed.as_str() {
                "" => BuiltinAction::EconomicMode {
                    enabled: None,
                    persist_global: false,
                    status_only: false,
                },
                "on" | "enable" | "true" | "1" => BuiltinAction::EconomicMode {
                    enabled: Some(true),
                    persist_global: false,
                    status_only: false,
                },
                "off" | "disable" | "false" | "0" => BuiltinAction::EconomicMode {
                    enabled: Some(false),
                    persist_global: false,
                    status_only: false,
                },
                "status" | "?" => BuiltinAction::EconomicMode {
                    enabled: None,
                    persist_global: false,
                    status_only: true,
                },
                "global on" | "global enable" => BuiltinAction::EconomicMode {
                    enabled: Some(true),
                    persist_global: true,
                    status_only: false,
                },
                "global off" | "global disable" => BuiltinAction::EconomicMode {
                    enabled: Some(false),
                    persist_global: true,
                    status_only: false,
                },
                _ => BuiltinAction::EconomicMode {
                    enabled: None,
                    persist_global: false,
                    status_only: true,
                },
            }
        },
    },
    BuiltinCommand {
        name: "hooks-trust",
        description: "Trust this project for hook execution",
        argument_hint: None,
        aliases: &[],
        gate: BuiltinGate::Hooks,
        resolve: |_args| BuiltinAction::HooksTrust,
    },
    BuiltinCommand {
        name: "hooks-list",
        description: "Show hooks loaded in this session",
        argument_hint: None,
        aliases: &[],
        gate: BuiltinGate::Hooks,
        resolve: |_args| BuiltinAction::HooksList,
    },
    BuiltinCommand {
        name: "hooks-add",
        description: "Add a custom hook file or directory",
        argument_hint: Some("path to hook file or directory"),
        aliases: &[],
        gate: BuiltinGate::Hooks,
        resolve: |args| BuiltinAction::HooksAdd {
            path: args.trim().to_string(),
        },
    },
    BuiltinCommand {
        name: "hooks-remove",
        description: "Remove a custom hook file or directory path",
        argument_hint: Some("path to hook file or directory"),
        aliases: &[],
        gate: BuiltinGate::Hooks,
        resolve: |args| BuiltinAction::HooksRemove {
            path: args.trim().to_string(),
        },
    },
    BuiltinCommand {
        name: "hooks-untrust",
        description: "Remove trust for the current project",
        argument_hint: None,
        aliases: &[],
        gate: BuiltinGate::Hooks,
        resolve: |_args| BuiltinAction::HooksUntrust,
    },
    BuiltinCommand {
        name: "plugins",
        description: "Manage plugins (list, reload, trust, add, remove)",
        argument_hint: Some("list | reload | trust <path> | add <path> | remove <path>"),
        aliases: &["plugin"],
        gate: BuiltinGate::Plugins,
        resolve: |args| {
            let trimmed = args.trim();
            if trimmed.is_empty() || trimmed == "list" {
                BuiltinAction::PluginsList
            } else if trimmed == "reload" {
                BuiltinAction::PluginsReload
            } else if trimmed.starts_with("trust") {
                BuiltinAction::PluginsTrust
            } else if let Some(path) = trimmed.strip_prefix("add ") {
                BuiltinAction::PluginsAdd {
                    path: path.trim().to_string(),
                }
            } else if let Some(path) = trimmed.strip_prefix("remove ") {
                BuiltinAction::PluginsRemove {
                    path: path.trim().to_string(),
                }
            } else if let Some(args) = trimmed.strip_prefix("install ") {
                let args = args.trim();
                let trust = args.ends_with(" --trust") || args == "--trust";
                let source = if trust {
                    args.trim_end_matches(" --trust").trim().to_string()
                } else {
                    args.to_string()
                };
                BuiltinAction::PluginsInstall { source, trust }
            } else if let Some(args) = trimmed.strip_prefix("uninstall ") {
                let args = args.trim();
                let confirm = args.ends_with(" --confirm") || args == "--confirm";
                let name = if confirm {
                    args.trim_end_matches(" --confirm").trim().to_string()
                } else {
                    args.to_string()
                };
                BuiltinAction::PluginsUninstall { name, confirm }
            } else if trimmed == "update" {
                BuiltinAction::PluginsUpdate { name: None }
            } else if let Some(name) = trimmed.strip_prefix("update ") {
                BuiltinAction::PluginsUpdate {
                    name: Some(name.trim().to_string()),
                }
            } else {
                BuiltinAction::PluginsList
            }
        },
    },
    BuiltinCommand {
        name: "reload-plugins",
        description: "Reload plugins from disk (alias for /plugins reload)",
        argument_hint: None,
        aliases: &[],
        gate: BuiltinGate::Plugins,
        resolve: |_args| BuiltinAction::PluginsReload,
    },
    BuiltinCommand {
        name: "session-info",
        description: "Show session details (model, turns, context usage)",
        argument_hint: None,
        aliases: &["status", "info"],
        gate: BuiltinGate::AlwaysOn,
        resolve: |_args| BuiltinAction::SessionInfo,
    },
    BuiltinCommand {
        name: "feedback",
        description: "Send feedback about the current session",
        argument_hint: Some("feedback text"),
        aliases: &[],
        gate: BuiltinGate::Feedback,
        resolve: |args| BuiltinAction::Feedback {
            text: args.trim().to_string(),
        },
    },
    BuiltinCommand {
        name: "goal",
        description: "Set, manage, or check an autonomous goal",
        argument_hint: Some("<objective> [--budget <tokens>] | status | pause | resume | clear"),
        aliases: &[],
        gate: BuiltinGate::Goal,
        resolve: |args| {
            let trimmed = args.trim();
            match trimmed.to_lowercase().as_str() {
                "" | "status" => BuiltinAction::GoalStatus,
                "pause" => BuiltinAction::GoalPause,
                "resume" => BuiltinAction::GoalResume,
                "clear" => BuiltinAction::GoalClear,
                _ => {
                    let (objective, token_budget) = parse_goal_budget(trimmed);
                    BuiltinAction::GoalSet {
                        objective,
                        token_budget,
                    }
                }
            }
        },
    },
];

/// Split a trailing `--budget <tokens>` flag off a `/goal` objective.
///
/// Only a TRAILING, standalone flag is consumed: the flag must be its own
/// whitespace-separated token and the value a final all-digit positive
/// token. Anything else stays part of the objective so a goal text that
/// merely mentions the flag is never silently mangled.
fn parse_goal_budget(trimmed: &str) -> (String, Option<i64>) {
    if let Some((head, tail)) = trimmed.rsplit_once("--budget") {
        let value = tail.trim();
        let flag_is_own_token = head.ends_with(char::is_whitespace)
            && tail.starts_with(char::is_whitespace)
            && !value.contains(char::is_whitespace);
        let head = head.trim_end();
        if flag_is_own_token
            && !head.is_empty()
            && !value.is_empty()
            && value.bytes().all(|b| b.is_ascii_digit())
            && let Ok(budget) = value.parse::<i64>()
            && budget > 0
        {
            return (head.to_string(), Some(budget));
        }
    }
    (trimmed.to_string(), None)
}

const PROMPT_COMMANDS: &[BuiltinCommand] = &[BuiltinCommand {
    name: "loop",
    description: "Run a prompt on a recurring interval",
    argument_hint: Some("[interval] <prompt>"),
    aliases: &[],
    gate: BuiltinGate::Scheduler,
    // INVARIANT: resolve() short-circuits any prompt-only command via a
    // PROMPT_COMMANDS lookup before reaching this closure. If a future
    // refactor changes that ordering this `unreachable!` will surface
    // the bug loudly instead of silently dispatching to ContextInfo
    // (which is what the previous sentinel did).
    resolve: |_| unreachable!("/loop is dispatched via the PROMPT_COMMANDS path in resolve()"),
}];

/// A slash command — either built-in or from a SKILL.md file.
pub(super) enum SlashCommand<'a> {
    BuiltIn(&'a BuiltinCommand),
    Skill(&'a SkillInfo),
}

impl<'a> SlashCommand<'a> {
    pub fn name(&self) -> &str {
        match self {
            SlashCommand::BuiltIn(b) => b.name,
            SlashCommand::Skill(s) => &s.name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            SlashCommand::BuiltIn(b) => b.description,
            SlashCommand::Skill(s) => s.short_description.as_deref().unwrap_or(&s.description),
        }
    }

    pub fn argument_hint(&self) -> Option<&str> {
        match self {
            SlashCommand::BuiltIn(b) => b.argument_hint,
            SlashCommand::Skill(s) => s.argument_hint.as_deref(),
        }
    }
}

/// Builtins first (win on name collisions), then user-invocable skills.
///
/// `availability` filters tool/extension-gated builtins so commands like
/// `/flush` and `/loop` only show up when the agent
/// actually has the backing capability. Always-on builtins are
/// unaffected.
pub(super) fn all_commands<'a>(
    skills: &'a [SkillInfo],
    availability: CommandAvailability,
) -> Vec<SlashCommand<'a>> {
    let mut commands: Vec<SlashCommand<'_>> = BUILTIN_COMMANDS
        .iter()
        .filter(|b| availability.allows(b.gate))
        .map(SlashCommand::BuiltIn)
        .collect();
    commands.extend(
        PROMPT_COMMANDS
            .iter()
            .filter(|b| availability.allows(b.gate))
            .map(SlashCommand::BuiltIn),
    );
    commands.extend(
        skills
            .iter()
            .filter(|s| s.user_invocable && s.enabled)
            .map(SlashCommand::Skill),
    );
    commands
}

/// Per-session capability snapshot used to gate which built-in slash
/// commands the shell advertises and resolves.
///
/// Each field corresponds to a `BuiltinGate` variant. Construct via
/// `CommandAvailability::all_enabled()` for tests, or build it from a
/// live `SessionActor` (see the call site in `acp_session.rs`).
///
/// `Default` returns every gate disabled (fail-closed) so a forgotten
/// initialization advertises only `BuiltinGate::AlwaysOn` commands.
/// In test code, prefer `all_enabled()` when the gating itself isn't
/// under test -- otherwise the test will silently lose coverage of any
/// gated builtin.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CommandAvailability {
    pub feedback: bool,
    /// Memory backend is enabled AND the active toolset includes the
    /// memory read tools. `/flush` and `/dream` only make sense when the
    /// model can later read back what they wrote, so the read-side tool
    /// presence is the right signal -- harnesses that don't register
    /// `memory_search`/`memory_get` get the commands hidden without the
    /// gating layer needing to know about agent_type.
    pub memory: bool,
    /// Memory backend is configured (has `backend_params`) but not
    /// necessarily currently enabled. Gates `/memory` (browse + toggle)
    /// so the user can re-enable memory after toggling it off.
    pub memory_configured: bool,
    pub scheduler: bool,
    pub hooks: bool,
    pub plugins: bool,
    pub goal: bool,
}

impl CommandAvailability {
    /// `true` if commands gated on `gate` should be advertised this session.
    pub fn allows(&self, gate: BuiltinGate) -> bool {
        match gate {
            BuiltinGate::AlwaysOn => true,
            BuiltinGate::Feedback => self.feedback,
            BuiltinGate::Memory => self.memory,
            BuiltinGate::MemoryConfigured => self.memory_configured,
            BuiltinGate::Scheduler => self.scheduler,
            BuiltinGate::Hooks => self.hooks,
            BuiltinGate::Plugins => self.plugins,
            BuiltinGate::Goal => self.goal,
        }
    }

    /// Test helper: every gate satisfied (matches the legacy "feedback only"
    /// fixture but enables every newly-gated command too).
    #[cfg(test)]
    pub fn all_enabled() -> Self {
        Self {
            feedback: true,
            memory: true,
            memory_configured: true,
            scheduler: true,
            hooks: true,
            plugins: true,
            goal: true,
        }
    }
}

/// Build the JSON value for `AvailableCommandsUpdate.meta` containing the
/// agent's currently-registered tool names.
///
/// Wire format: `{"tools": ["read_file", "scheduler_create", ...]}`.
/// Pager clients drain this and call `CommandRegistry::set_available_tools`
/// to gate tool-dependent commands like `/loop`.
///
/// Takes `&[String]` rather than `&[&str]` because serde_json copies
/// each entry into the `Value` regardless, so an intermediate
/// `Vec<&str>` adapter would just waste an allocation.
pub(crate) fn build_tools_meta(tool_names: &[String]) -> acp::Meta {
    let mut meta = acp::Meta::new();
    meta.insert("tools".to_owned(), serde_json::json!(tool_names));
    meta
}

/// Build the ACP `AvailableCommand` list for the client autocomplete menu.
///
/// Skills include `scope` and `path` in `_meta` so the client can show
/// where the command comes from (e.g. "project" vs "global") and link
/// to the SKILL.md source.
pub(super) fn available_commands(
    skills: &[SkillInfo],
    availability: CommandAvailability,
) -> Vec<acp::AvailableCommand> {
    // Detect duplicate bare names among user-invocable skills so we can
    // advertise qualified names (e.g. "local:commit") when ambiguous.
    let mut name_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for s in skills.iter().filter(|s| s.user_invocable) {
        *name_counts.entry(&s.name).or_default() += 1;
    }

    // Collect builtin command names so skills that collide are also qualified.
    let builtin_names: std::collections::HashSet<&str> =
        BUILTIN_COMMANDS.iter().map(|b| b.name).collect();

    all_commands(skills, availability)
        .iter()
        .flat_map(|cmd| {
            let entries: Vec<acp::AvailableCommand> = match cmd {
                SlashCommand::BuiltIn(b) => {
                    vec![
                        acp::AvailableCommand::new(
                            b.name.to_string(),
                            cmd.description().to_string(),
                        )
                        .input(cmd.argument_hint().map(|hint| {
                            acp::AvailableCommandInput::Unstructured(
                                acp::UnstructuredCommandInput::new(hint.to_string()),
                            )
                        })),
                    ]
                }
                SlashCommand::Skill(s) => {
                    let meta = serde_json::json!({
                        "scope": s.scope,
                        "path": s.path,
                    })
                    .as_object()
                    .cloned();
                    let qualified = format_skill_name(s);
                    let bare_collides = name_counts.get(s.name.as_str()).copied().unwrap_or(0) > 1
                        || builtin_names.contains(s.name.as_str());
                    let make_entry = |name: String| {
                        acp::AvailableCommand::new(name, cmd.description().to_string())
                            .input(cmd.argument_hint().map(|hint| {
                                acp::AvailableCommandInput::Unstructured(
                                    acp::UnstructuredCommandInput::new(hint.to_string()),
                                )
                            }))
                            .meta(meta.clone())
                    };
                    let mut entries = Vec::new();
                    if bare_collides || s.plugin_name.is_some() {
                        entries.push(make_entry(qualified));
                    }
                    if !bare_collides {
                        entries.push(make_entry(s.name.clone()));
                    }
                    entries
                }
            };
            entries
        })
        .collect()
}

/// Pre-session builtin commands for `InitializeResponse._meta`.
///
/// Advertises every always-on command plus any gated command whose gate
/// is satisfied by `availability`. Pre-session, only config-derived gates
/// (e.g. `goal`, which is driven by the `resolve_goal()` feature flag and
/// not by a live toolset) can be evaluated; runtime/tool-dependent gates
/// stay closed because there's no session context yet. See
/// `MvpAgent::command_availability` for how the pre-session snapshot is
/// built. With `CommandAvailability::default()` (all gates closed) this
/// is equivalent to advertising only `BuiltinGate::AlwaysOn` commands.
pub(crate) fn builtin_commands(availability: CommandAvailability) -> Vec<acp::AvailableCommand> {
    BUILTIN_COMMANDS
        .iter()
        .filter(|cmd| availability.allows(cmd.gate))
        .map(|cmd| {
            acp::AvailableCommand::new(cmd.name.to_string(), cmd.description.to_string()).input(
                cmd.argument_hint.map(|hint| {
                    acp::AvailableCommandInput::Unstructured(acp::UnstructuredCommandInput::new(
                        hint.to_string(),
                    ))
                }),
            )
        })
        .collect()
}

// ── x.ai/commands/list ext method ────────────────────────────────

#[derive(serde::Deserialize)]
pub(crate) struct ListCommandsRequest {
    pub cwd: Option<String>,
}

#[derive(serde::Serialize)]
pub(crate) struct ListCommandsResponse {
    pub commands: Vec<acp::AvailableCommand>,
}

/// Build the available commands list, optionally scoped to a working directory.
/// - `Some(cwd)`: full skill discovery (Local + Repo + User) + builtins.
/// - `None`: builtins + global (User-scoped) skills only.
pub(crate) async fn list_commands(
    cwd: Option<&str>,
    skills_config: &xai_grok_agent::prompt::skills::SkillsConfig,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    availability: CommandAvailability,
    compat: xai_grok_tools::types::compat::CompatConfig,
) -> ListCommandsResponse {
    let skills = xai_grok_agent::prompt::skills::list_skills_with_plugins(
        cwd,
        skills_config,
        plugin_registry,
        compat,
    )
    .await;
    ListCommandsResponse {
        commands: available_commands(&skills, availability),
    }
}

// ── Slash command resolution ────────────────────────────────────

/// A parsed skill reference from user input.
///
/// Produced by `parse_skill_references()` when scanning user text for known
/// `/{skill_name}` tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedSkillRef {
    /// The skill name (bare or qualified, as typed by the user).
    pub name: String,
    /// Arguments following this skill reference, up to the next skill or end-of-input.
    pub args: String,
    /// The resolved `SkillInfo` path, for loading SKILL.md.
    pub skill_path: String,
    /// Scope-qualified name (e.g. "user:commit"), used for telemetry.
    pub qualified_name: String,
    /// Plugin name if this is a plugin skill.
    pub plugin_name: Option<String>,
}

pub(super) enum SlashCommandOutcome {
    /// Execute directly, no model round-trip.
    Builtin(BuiltinAction),
    /// One or more skills detected in user input.
    ///
    /// The original prompt `blocks` are preserved verbatim — they are NOT
    /// rewritten. The shell's prompt assembly layer will read each skill's
    /// SKILL.md, apply substitutions, and build the `<skill_information>`
    /// envelope alongside the `<user_query>` block.
    InvokeSkill {
        /// The original, unmodified prompt blocks.
        blocks: Vec<acp::ContentBlock>,
        /// Parsed skill references (one per detected `/{skill}` token).
        skills: Vec<ParsedSkillRef>,
    },
}

pub(super) enum BuiltinAction {
    Compact {
        user_context: Option<String>,
    },
    SetYolo {
        enabled: bool,
    },
    FlushMemory,
    Dream,
    ContextInfo,
    /// Cap effective context at 200K for pricing. `enabled: None` with
    /// `status_only: false` means toggle; `persist_global` writes `[ui].economic_mode`.
    EconomicMode {
        enabled: Option<bool>,
        persist_global: bool,
        status_only: bool,
    },
    HooksTrust,
    HooksList,
    HooksAdd {
        path: String,
    },
    HooksRemove {
        path: String,
    },
    HooksUntrust,
    PluginsList,
    PluginsReload,
    PluginsTrust,
    SessionInfo,
    PluginsAdd {
        path: String,
    },
    PluginsRemove {
        path: String,
    },
    PluginsInstall {
        source: String,
        trust: bool,
    },
    PluginsUninstall {
        name: String,
        confirm: bool,
    },
    PluginsUpdate {
        name: Option<String>,
    },
    Feedback {
        text: String,
    },
    MemoryBrowse,
    MemoryToggle {
        enabled: bool,
    },
    GoalSet {
        objective: String,
        token_budget: Option<i64>,
    },
    GoalStatus,
    GoalPause,
    GoalResume,
    GoalClear,
}

impl BuiltinAction {
    pub(crate) fn command_name(&self) -> &'static str {
        match self {
            BuiltinAction::Compact { .. } => "compact",
            BuiltinAction::SetYolo { .. } => "yolo",
            BuiltinAction::FlushMemory => "flush",
            BuiltinAction::Dream => "dream",
            BuiltinAction::ContextInfo => "context",
            BuiltinAction::EconomicMode { .. } => "economic-mode",
            BuiltinAction::HooksTrust => "hooks-trust",
            BuiltinAction::HooksList => "hooks-list",
            BuiltinAction::HooksAdd { .. } => "hooks-add",
            BuiltinAction::HooksRemove { .. } => "hooks-remove",
            BuiltinAction::HooksUntrust => "hooks-untrust",
            BuiltinAction::PluginsList => "plugins-list",
            BuiltinAction::PluginsReload => "plugins-reload",
            BuiltinAction::PluginsTrust => "plugins-trust",
            BuiltinAction::SessionInfo => "session",
            BuiltinAction::PluginsAdd { .. } => "plugins-add",
            BuiltinAction::PluginsRemove { .. } => "plugins-remove",
            BuiltinAction::PluginsInstall { .. } => "plugins-install",
            BuiltinAction::PluginsUninstall { .. } => "plugins-uninstall",
            BuiltinAction::PluginsUpdate { .. } => "plugins-update",
            BuiltinAction::Feedback { .. } => "feedback",
            BuiltinAction::MemoryBrowse => "memory",
            BuiltinAction::MemoryToggle { .. } => "memory",
            BuiltinAction::GoalSet { .. }
            | BuiltinAction::GoalStatus
            | BuiltinAction::GoalPause
            | BuiltinAction::GoalResume
            | BuiltinAction::GoalClear => "goal",
        }
    }

    pub(crate) fn args_provided(&self) -> bool {
        match self {
            BuiltinAction::Compact { user_context } => user_context.is_some(),
            BuiltinAction::SetYolo { .. } => true,
            BuiltinAction::FlushMemory => false,
            BuiltinAction::Dream => false,
            BuiltinAction::ContextInfo => false,
            BuiltinAction::EconomicMode {
                enabled,
                persist_global,
                status_only,
            } => enabled.is_some() || *persist_global || *status_only,
            BuiltinAction::HooksTrust => false,
            BuiltinAction::HooksList => false,
            BuiltinAction::HooksAdd { .. } => true,
            BuiltinAction::HooksRemove { .. } => true,
            BuiltinAction::HooksUntrust => false,
            BuiltinAction::PluginsList => false,
            BuiltinAction::PluginsReload => false,
            BuiltinAction::PluginsTrust => false,
            BuiltinAction::SessionInfo => false,
            BuiltinAction::PluginsAdd { .. } => true,
            BuiltinAction::PluginsRemove { .. } => true,
            BuiltinAction::PluginsInstall { .. } => true,
            BuiltinAction::PluginsUninstall { .. } => true,
            BuiltinAction::PluginsUpdate { name } => name.is_some(),
            BuiltinAction::Feedback { text } => !text.is_empty(),
            BuiltinAction::MemoryBrowse => false,
            BuiltinAction::MemoryToggle { .. } => true,
            BuiltinAction::GoalSet { .. } => true,
            BuiltinAction::GoalStatus
            | BuiltinAction::GoalPause
            | BuiltinAction::GoalResume
            | BuiltinAction::GoalClear => false,
        }
    }
}

/// How to rewrite the user's prompt when a slash command resolves to a skill.
///
/// - `RewriteToRun` (default): replace `/foo args` with `"run /foo args"`,
///   matching today's Grok Build flow that calls our dedicated `skill` tool.
/// - `Passthrough`: leave the prompt verbatim. Some templates use this —
///   the model is trained to spot a leading `/<name>`, look it up in the
///   `<agent_skills>` listing, and call the Read tool on `fullPath`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SkillSlashRewrite {
    #[default]
    RewriteToRun,
    Passthrough,
}

/// Scan user input left-to-right for `/{word}` tokens where `word` matches
/// a **known registered skill name** (bare or qualified).
///
/// Unknown `/words` (like `/api/v2/users`, `/tmp/file`) are NOT treated as
/// skill references — only tokens that resolve to a known skill count.
///
/// Returns `None` when no known skill references are found. Otherwise returns
/// the list of `ParsedSkillRef` entries with each skill's args (the text
/// between one skill token and the next, or end-of-input).
pub(crate) fn parse_skill_references(
    text: &str,
    skills: &[SkillInfo],
    availability: CommandAvailability,
) -> Option<Vec<ParsedSkillRef>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let commands = all_commands(skills, availability);

    // Build a map from bare name → SkillInfo reference, tracking ambiguous
    // names (multiple skills sharing the same bare name). Ambiguous bare
    // names are excluded from the map so `/commit` passes through when two
    // skills are both called "commit" in different scopes — the user must
    // use the qualified form `/local:commit` instead.
    let mut skill_map: std::collections::HashMap<&str, Option<&SkillInfo>> =
        std::collections::HashMap::new();
    for cmd in &commands {
        if let SlashCommand::Skill(s) = cmd {
            skill_map
                .entry(&s.name)
                .and_modify(|v| *v = None) // duplicate → mark ambiguous
                .or_insert(Some(s));
        }
    }
    // Remove ambiguous entries so they're never matched by bare name.
    skill_map.retain(|_, v| v.is_some());

    // Collect positions of all /{word} tokens, checking if each matches a known skill.
    struct SkillHit<'a> {
        /// Byte offset of the '/' in the source text.
        offset: usize,
        /// The text as typed by the user (e.g. "commit", "user:commit").
        typed_name: String,
        /// Resolved skill info.
        skill: &'a SkillInfo,
    }

    let mut hits: Vec<SkillHit<'_>> = Vec::new();
    let bytes = trimmed.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'/' {
            i += 1;
            continue;
        }
        // Must be at start of text or preceded by whitespace.
        if i > 0 && !bytes[i - 1].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i + 1; // skip '/'
        if start >= bytes.len() {
            break;
        }
        // Grab the word: everything until whitespace, '/' or end.
        let end = trimmed[start..]
            .find(|c: char| c.is_whitespace())
            .map(|rel| start + rel)
            .unwrap_or(trimmed.len());
        let word = &trimmed[start..end];
        if word.is_empty() {
            i = start;
            continue;
        }

        // Check if it's a builtin — skip those (builtins are not multi-skill candidates).
        let is_builtin = BUILTIN_COMMANDS
            .iter()
            .chain(PROMPT_COMMANDS.iter())
            .any(|b| b.name == word || b.aliases.contains(&word));
        if is_builtin {
            i = end;
            continue;
        }

        // Check bare name in map (only unambiguous entries remain).
        if let Some(Some(skill)) = skill_map.get(word) {
            hits.push(SkillHit {
                offset: i,
                typed_name: word.to_string(),
                skill,
            });
            i = end;
            continue;
        }

        // Check qualified name (e.g. "user:commit").
        let mut found = false;
        for cmd in &commands {
            if let SlashCommand::Skill(s) = cmd
                && format_skill_name(s) == word
            {
                hits.push(SkillHit {
                    offset: i,
                    typed_name: word.to_string(),
                    skill: s,
                });
                found = true;
                break;
            }
        }
        if found {
            i = end;
            continue;
        }

        // Unknown /word — skip.
        i = end;
    }

    if hits.is_empty() {
        return None;
    }

    // Compute args for each hit: text from end-of-skill-token to start of next hit (or end).
    let mut refs = Vec::with_capacity(hits.len());
    for (idx, hit) in hits.iter().enumerate() {
        let word_end = hit.offset + 1 + hit.typed_name.len(); // past the /word
        let args_end = if idx + 1 < hits.len() {
            hits[idx + 1].offset
        } else {
            trimmed.len()
        };
        let args = trimmed[word_end..args_end].trim().to_string();
        refs.push(ParsedSkillRef {
            name: hit.typed_name.clone(),
            args,
            skill_path: hit.skill.path.clone(),
            qualified_name: format_skill_name(hit.skill),
            plugin_name: hit.skill.plugin_name.clone(),
        });
    }

    Some(refs)
}

/// Load each parsed skill's SKILL.md, apply substitutions, and build the
/// `<skill_information>` envelope.
///
/// Shared by turn start (prompt assembly in `process_conversation_turn`) and
/// the mid-turn interjection drain, so a skill delivers identically whether
/// it starts a turn or is force-sent into a running one. Returns `None` when
/// no skill content loads (missing files are logged and skipped; the
/// `<skills_referenced>` index still lists every parsed ref).
pub(super) async fn build_skill_information_for_refs(
    parsed_skills: &[ParsedSkillRef],
    slash_skills: &[SkillInfo],
    session_id: &str,
) -> Option<String> {
    use xai_grok_tools::implementations::skills::skill::{
        SkillRef, SubstitutionContext, apply_substitutions, build_skill_block_with_run_id,
        build_skill_information, load_skill_content, mint_skill_run_id,
    };

    let mut skill_blocks: Vec<String> = Vec::new();
    for sk in parsed_skills {
        // Find the SkillInfo by path (more reliable than by name for
        // qualified skills).
        let Some(info) = slash_skills.iter().find(|s| s.path == sk.skill_path) else {
            continue;
        };
        match load_skill_content(info).await {
            Ok(mut content) => {
                let skill_dir = std::path::Path::new(&info.path)
                    .parent()
                    .and_then(|p| p.to_str());
                let args = if sk.args.is_empty() {
                    None
                } else {
                    Some(sk.args.as_str())
                };
                // Host-mint once per skill expansion so ${RUN_ID} and the
                // envelope run_id attribute stay consistent (no model/shell).
                let run_id = mint_skill_run_id();
                apply_substitutions(
                    &mut content,
                    args,
                    &SubstitutionContext {
                        skill_dir,
                        session_id: Some(session_id),
                        run_id: Some(run_id.as_str()),
                        plugin_root: info.plugin_root.as_deref(),
                        plugin_data: info.plugin_data.as_deref(),
                    },
                );
                skill_blocks.push(build_skill_block_with_run_id(
                    &sk.name, &sk.args, &content, &run_id,
                ));
            }
            Err(e) => {
                tracing::warn!(skill = %sk.name, error = %e, "failed to load skill for expansion");
            }
        }
    }

    if skill_blocks.is_empty() {
        return None;
    }
    let refs: Vec<SkillRef<'_>> = parsed_skills
        .iter()
        .map(|sk| SkillRef {
            name: &sk.name,
            path: &sk.skill_path,
        })
        .collect();
    Some(build_skill_information(&skill_blocks, &refs))
}

/// Resolve prompt blocks as a slash command.
/// `Ok(blocks)` = not a command, pass through. `Err(outcome)` = matched.
pub(super) fn resolve(
    prompt_blocks: Vec<acp::ContentBlock>,
    skills: &[SkillInfo],
    availability: CommandAvailability,
    _skill_rewrite: SkillSlashRewrite,
) -> Result<Vec<acp::ContentBlock>, SlashCommandOutcome> {
    let Some((command_name, args)) = parse_slash_prefix(&prompt_blocks) else {
        return Ok(prompt_blocks);
    };

    // Prompt-only commands (e.g. /loop) need a full agent round-trip, not
    // a direct BuiltinAction. They're filtered against the same gate the
    // PROMPT_COMMANDS entry declares -- looking it up here means the gate
    // value lives in exactly one place (the PROMPT_COMMANDS entry) and a
    // future addition just needs the entry, not a parallel branch.
    if let Some(prompt_cmd) = PROMPT_COMMANDS.iter().find(|c| c.name == command_name)
        && availability.allows(prompt_cmd.gate)
    {
        // Dispatch by name so a future PROMPT_COMMANDS entry without a
        // matching arm fails loudly at the call site instead of silently
        // reusing /loop's prompt builder.
        let mut blocks = match prompt_cmd.name {
            "loop" => build_loop_prompt_blocks(args),
            other => {
                unreachable!("prompt-only command /{other} has no resolver wired in resolve()")
            }
        };
        // Annotate with the compact invocation as `displayText` so every client
        // and session replay renders "/loop <args>" instead of the expanded
        // instruction. The pager does this client-side; bare-text clients rely
        // on this server-side annotation.
        let display_text = if args.is_empty() {
            format!("/{command_name}")
        } else {
            format!("/{command_name} {args}")
        };
        if let Some(acp::ContentBlock::Text(tb)) = blocks.first_mut() {
            let map = tb.meta.get_or_insert_with(acp::Meta::new);
            map.insert(
                "displayText".to_string(),
                serde_json::Value::String(display_text),
            );
        }
        // /loop is a prompt-only command — use InvokeSkill with empty skills
        // so the caller forwards the rewritten blocks directly to the model.
        return Err(SlashCommandOutcome::InvokeSkill {
            blocks,
            skills: vec![],
        });
    }

    // Check if the leading /command is a builtin.
    let commands = all_commands(skills, availability);
    let builtin_match = commands
        .iter()
        .find(|c| matches!(c, SlashCommand::BuiltIn(b) if c.name() == command_name || b.aliases.contains(&command_name)));

    if let Some(SlashCommand::BuiltIn(builtin)) = builtin_match {
        let action = (builtin.resolve)(args);
        return Err(SlashCommandOutcome::Builtin(action));
    }

    // Not a builtin — use the multi-skill parser to detect ALL /{skill}
    // references in the full input text, splitting args at skill boundaries.
    let full_text = prompt_blocks
        .iter()
        .find_map(|b| {
            if let acp::ContentBlock::Text(t) = b {
                Some(t.text.as_str())
            } else {
                None
            }
        })
        .unwrap_or("");

    if let Some(parsed_skills) = parse_skill_references(full_text, skills, availability) {
        return Err(SlashCommandOutcome::InvokeSkill {
            blocks: prompt_blocks,
            skills: parsed_skills,
        });
    }

    // No known skill matched — pass through as regular user input.
    Ok(prompt_blocks)
}

/// Extract `(name, args)` if the first text block starts with `/`.
///
/// - `"/compact keep auth"` → `Some(("compact", "keep auth"))`
/// - `"please run /commit"` → `None` (not at start)
fn parse_slash_prefix(prompt_blocks: &[acp::ContentBlock]) -> Option<(&str, &str)> {
    let text = prompt_blocks.iter().find_map(|b| {
        if let acp::ContentBlock::Text(t) = b {
            Some(t.text.as_str())
        } else {
            None
        }
    })?;

    let trimmed = text.trim();
    let without_slash = trimmed.strip_prefix('/')?;

    let (name, args) = match without_slash.find(char::is_whitespace) {
        Some(idx) => (&without_slash[..idx], without_slash[idx..].trim()),
        None => (without_slash, ""),
    };

    if name.is_empty() {
        return None;
    }

    Some((name, args))
}

/// Build the `/loop` prompt blocks for the shell client.
///
/// The wording (usage hint + scheduling instruction) is sourced from
/// `xai-grok-tools` so it stays identical to the pager's `LoopCommand` and the
/// two front-ends can't drift. Like the pager, there is no host-side interval
/// default: the model derives the cadence from the request and asks when none
/// is given.
fn build_loop_prompt_blocks(args: &str) -> Vec<acp::ContentBlock> {
    use xai_grok_tools::implementations::grok_build::{
        loop_schedule_instruction, loop_usage_message,
    };

    let text = if args.trim().is_empty() {
        loop_usage_message().to_string()
    } else {
        loop_schedule_instruction(args)
    };

    vec![acp::ContentBlock::Text(acp::TextContent::new(text))]
}

#[cfg(test)]
#[path = "slash_commands_tests.rs"]
mod tests;
