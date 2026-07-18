use agent_client_protocol as acp;
use anyhow::Result;
use indexmap::IndexMap;
use std::path::PathBuf;
use toml::Value as TomlValue;
use toml::map::Map as TomlMap;
use xai_grok_agent::prompt::skills::SkillsConfig;
use xai_grok_tools::types::compat::{CompatConfig, CompatConfigToml};

<<<<<<< HEAD
use xai_grok_config::ClaudeImport;
pub(crate) use xai_grok_config::mcp_servers::{
    MCP_SCOPE_PROJECT, McpEnabledFilter, get_mcp_server_config, load_mcp_json_file,
    load_mcp_preferences, materialize_mcp_config, parse_mcp_servers_from_toml,
};
use xai_grok_config::mcp_servers::{
    MCP_SCOPE_USER, McpPreferencesLoad, McpServerSources, load_cursor_mcp_servers_as_configs,
    load_mcp_preferences_from, mcp_preferences_path,
};
pub use xai_grok_config_types::{McpConfig, RelaySyncConfig};
pub use xai_grok_config_types::{
    McpJsonOAuthBlock, McpPreferenceSource, McpPreferencesFile, McpServerConfig,
    McpServerConfigProblem, McpServerPreferences, McpServerProblemSeverity,
    McpServerTransportConfig, McpSetupConfig, McpSetupDerivedValue, McpSetupField,
    McpSetupFieldType, McpSetupOption, McpSetupResolution,
};
=======
pub use xai_grok_mcp::oauth_config::{McpOAuthConfig, McpOAuthConfigMap};
// MCP server config value types extracted to `xai-grok-config-types` (config
// dependency inversion); re-exported so `crate::util::config::*` paths keep working.
pub use xai_grok_config_types::{
    McpJsonOAuthBlock, McpPreferenceSource, McpPreferencesFile, McpServerConfig,
    McpServerPreferences, McpServerTransportConfig, McpSetupConfig, McpSetupDerivedValue,
    McpSetupField, McpSetupFieldType, McpSetupOption, McpSetupResolution,
};
// Permission-policy value types likewise extracted; re-exported to keep paths stable.
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub use xai_grok_config_types::{
    PatternMode, PermissionConfig, PermissionRule, RuleAction, ToolFilter,
};
pub use xai_grok_mcp::oauth_config::{McpOAuthConfig, McpOAuthConfigMap};
pub use xai_grok_workspace::project_config::{
    MCP_JSON_FILENAME, find_mcp_json_files, mcp_json_candidate_paths,
};

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub cli: crate::agent::config::CliConfig,
    pub models: crate::agent::config::ModelsConfig,
    pub ui: crate::agent::config::UiConfig,
    pub harness: crate::agent::config::HarnessConfig,
    pub skills: SkillsConfig,
    /// Round-tripped so the pager preserves per-vendor toggles when persisting other settings.
    pub compat: CompatConfigToml,
    /// From `[endpoints]`.
    pub management_api_key: Option<String>,
    pub permission: Option<PermissionConfig>,
    pub diagnostics: crate::agent::config::DiagnosticsConfig,
    /// Round-tripped through `merge_section` so pager setters can persist session fields.
    pub session: crate::agent::config::SessionConfig,
    /// `[toolset.ask_user_question]` sub-table, the only `[toolset]` piece the settings modal writes.
    /// The rest of `[toolset]` never round-trips (it carries runtime-only structs whose defaults must not hit disk).
    pub ask_user_question: crate::tools::config::AskUserQuestionToolConfig,
    pub privacy: PrivacyConfig,
    pub consent: super::consent::ConsentConfig,
    pub telemetry: TelemetryPersistConfig,
    pub features: FeaturesPersistConfig,
}

/// Unmodeled keys under `[telemetry]` are preserved by the deep merge in `save_config_locked`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TelemetryPersistConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_upload: Option<bool>,
}

/// Unmodeled keys under `[features]` are preserved by the deep merge in `save_config_locked`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeaturesPersistConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_trace_card: Option<bool>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PrivacyConfig {
    /// RFC 3339 UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_banner_acked: Option<String>,
}

pub(crate) fn get_mcp_server_config_with_project(
    name: &str,
    cwd: &std::path::Path,
) -> Option<McpServerConfig> {
    xai_grok_config::mcp_servers::get_mcp_server_config_with_project(
        name,
        &crate::config::find_project_configs(cwd),
    )
}

pub(crate) fn mcp_server_scope(name: &str, cwd: &std::path::Path) -> &'static str {
    xai_grok_config::mcp_servers::mcp_server_scope(name, &crate::config::find_project_configs(cwd))
}

pub(crate) fn load_mcp_servers_with_oauth(
    cwd: &std::path::Path,
    compat: &CompatConfig,
) -> (Vec<acp::McpServer>, McpOAuthConfigMap) {
<<<<<<< HEAD
    xai_grok_config::mcp_servers::load_mcp_servers_with_oauth(&mcp_server_sources(
        cwd,
        compat,
        "load_mcp_servers_with_oauth",
    ))
=======
    let global_config =
        crate::config::load_from_disk().unwrap_or_else(|_| TomlValue::Table(toml::map::Map::new()));

    let mut servers_map: IndexMap<String, McpServerConfig> = IndexMap::new();
    for (name, config) in parse_mcp_servers_from_toml(&global_config) {
        servers_map.insert(name, config);
    }

    let project_configs = crate::config::find_project_configs(cwd);
    for config_path in &project_configs {
        if let Ok(root) = crate::config::load_config_file(config_path) {
            for (name, config) in parse_mcp_servers_from_toml(&root) {
                servers_map.insert(name, config);
            }
        }
    }
    // Also load from ~/.claude.json (lower priority than TOML)
    for (name, config) in load_claude_json_mcp_servers_as_configs(cwd, compat) {
        servers_map.entry(name).or_insert(config);
    }

    // Also load from ~/.cursor/mcp.json (lower priority than TOML and ~/.claude.json)
    for (name, config) in load_cursor_mcp_servers_as_configs(cwd, compat) {
        servers_map.entry(name).or_insert(config);
    }

    // Also load from .mcp.json files (lower priority than TOML, ~/.claude.json, and ~/.cursor)
    for (name, config) in load_mcp_json_servers_as_configs(cwd) {
        servers_map.entry(name).or_insert(config);
    }

    let mut oauth_configs = McpOAuthConfigMap::new();
    let mut acp_servers = Vec::new();

    let preferences = load_mcp_preferences().file();
    let sub = &crate::config::expand_env_vars_in_string;
    for (name, config) in servers_map {
        let mut config = match config.resolve_setup(preferences.servers.get(&name)) {
            McpSetupResolution::Resolved(config) => config,
            McpSetupResolution::Required(_) => continue,
            McpSetupResolution::Invalid(reason) => {
                tracing::warn!(server = %name, error = %reason, "MCP setup config is invalid");
                continue;
            }
        };
        config.expand_strings(sub);
        if let Some(oauth) = config.oauth_config() {
            oauth_configs.insert(name.clone(), oauth);
        }
        if let Some(acp_server) = config.to_acp_mcp_server(name) {
            acp_servers.push(acp_server);
        }
    }

    (acp_servers, oauth_configs)
>>>>>>> e3fdf3ed (Merge 2 (#4))
}

pub fn load_mcp_servers(cwd: &std::path::Path, compat: &CompatConfig) -> Vec<acp::McpServer> {
    xai_grok_config::mcp_servers::load_mcp_servers(&mcp_server_sources(
        cwd,
        compat,
        "load_mcp_servers",
    ))
}

<<<<<<< HEAD
pub(crate) fn mcp_server_sources(
=======
/// Load MCP servers from config.toml only (global + project-scoped), without
/// loading from `~/.claude.json`, `~/.cursor/mcp.json`, or
/// `.mcp.json` sources.
///
/// Used by [`crate::session::managed_mcp::merge_managed_mcp_servers_sourced`]
/// which handles those non-TOML sources separately with proper `ConfigSource`
/// tracking. Using [`load_mcp_servers`] there would cause all entries to be
/// tagged as `ConfigSource::ConfigToml`, hiding the true origin.
pub(crate) fn load_mcp_servers_toml_only(cwd: &std::path::Path) -> Vec<acp::McpServer> {
    let preferences = load_mcp_preferences().file();
    let sub = &crate::config::expand_env_vars_in_string;
    load_all_mcp_configs(cwd)
        .into_iter()
        .filter_map(|(name, config)| {
            let mut config = match config.resolve_setup(preferences.servers.get(&name)) {
                McpSetupResolution::Resolved(config) => config,
                McpSetupResolution::Required(_) => return None,
                McpSetupResolution::Invalid(reason) => {
                    tracing::warn!(server = %name, error = %reason, "MCP setup config is invalid");
                    return None;
                }
            };
            config.expand_strings(sub);
            config.to_acp_mcp_server(name)
        })
        .collect()
}

/// Merge MCP servers from a pre-parsed global config with project-scoped overrides.
///
/// Same merge strategy as [`load_mcp_servers_with_project`] but takes the global
/// config as a pre-parsed `toml::Value` instead of re-reading from disk. Project
/// configs are still read from disk because the watcher signals paths, not content.
pub(crate) fn reload_mcp_servers_merged(
    global_config: &TomlValue,
>>>>>>> e3fdf3ed (Merge 2 (#4))
    cwd: &std::path::Path,
    compat: &CompatConfig,
    caller: &'static str,
) -> McpServerSources {
    McpServerSources {
        cwd: cwd.to_path_buf(),
        project_configs: crate::config::find_project_configs(cwd),
        mcp_json_files: find_mcp_json_files(cwd),
        compat: *compat,
        claude_import: claude_import(caller),
    }
<<<<<<< HEAD
=======

    let project_configs = crate::config::find_project_configs(cwd);
    for config_path in &project_configs {
        if let Ok(root) = crate::config::load_config_file(config_path) {
            let project_servers = parse_mcp_servers_from_toml(&root);
            if !project_servers.is_empty() {
                tracing::info!(
                    count = project_servers.len(),
                    path = %config_path.display(),
                    "Loaded project-scoped MCP servers from .grok/config.toml"
                );
                for (name, config) in project_servers {
                    servers.insert(name, config);
                }
            }
        }
    }
    // Also load from ~/.claude.json (lower priority than TOML)
    let claude_servers = load_claude_json_mcp_servers_as_configs(cwd, compat);
    tracing::info!(
        count = claude_servers.len(),
        "Loaded MCP servers from ~/.claude.json"
    );
    for (name, config) in claude_servers {
        servers.entry(name).or_insert(config);
    }

    // Also load from ~/.cursor/mcp.json (lower priority than TOML and ~/.claude.json)
    let cursor_servers = load_cursor_mcp_servers_as_configs(cwd, compat);
    tracing::info!(
        count = cursor_servers.len(),
        "Loaded Cursor MCP servers from ~/.cursor/mcp.json"
    );
    for (name, config) in cursor_servers {
        servers.entry(name).or_insert(config);
    }

    // Also load from .mcp.json files (lower priority than TOML)
    let mcp_json_servers = load_mcp_json_servers_as_configs(cwd);
    tracing::info!(
        count = mcp_json_servers.len(),
        "Loaded .mcp.json MCP servers"
    );
    for (name, config) in mcp_json_servers {
        servers.entry(name).or_insert(config);
    }

    let preferences = load_mcp_preferences().file();
    let sub = &crate::config::expand_env_vars_in_string;
    servers
        .into_iter()
        .filter_map(|(name, config)| {
            let mut config = match config.resolve_setup(preferences.servers.get(&name)) {
                McpSetupResolution::Resolved(config) => config,
                McpSetupResolution::Required(_) => return None,
                McpSetupResolution::Invalid(reason) => {
                    tracing::warn!(server = %name, error = %reason, "MCP setup config is invalid");
                    return None;
                }
            };
            config.expand_strings(sub);
            config.to_acp_mcp_server(name)
        })
        .collect()
>>>>>>> e3fdf3ed (Merge 2 (#4))
}

pub(crate) fn load_mcp_servers_toml_only(cwd: &std::path::Path) -> Vec<acp::McpServer> {
    xai_grok_config::mcp_servers::load_mcp_servers_toml_only(&crate::config::find_project_configs(
        cwd,
    ))
}

pub(crate) fn load_mcp_json_servers(cwd: &std::path::Path) -> Vec<acp::McpServer> {
    xai_grok_config::mcp_servers::load_mcp_json_servers(
        &find_mcp_json_files(cwd),
        claude_import("load_mcp_json_servers"),
    )
}

pub(crate) async fn save_mcp_preferences(prefs: &McpPreferencesFile) -> Result<()> {
    save_mcp_preferences_to(&mcp_preferences_path(), prefs).await
}

pub(crate) async fn save_mcp_preferences_to(
    path: &std::path::Path,
    prefs: &McpPreferencesFile,
) -> Result<()> {
    if matches!(load_mcp_preferences_from(path), McpPreferencesLoad::Corrupt) {
        anyhow::bail!(
            "refusing to overwrite unreadable MCP preferences at {}",
            path.display()
        );
    }
    let json = serde_json::to_string_pretty(prefs)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    // Setup values may hold secrets: owner-only from creation.
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        xai_grok_config::fs_atomic::write_user_file_atomically(&path, &json, Some(0o600))
    })
    .await?
    .map_err(|e| anyhow::anyhow!("failed to write mcp preferences: {e}"))?;
    Ok(())
}

pub(crate) async fn restore_mcp_preference_server(
    server_name: &str,
    previous: Option<McpServerPreferences>,
) -> Result<()> {
    let load = load_mcp_preferences();
    if !load.is_writable() {
        // Error, not Ok: the caller's "no state left behind" contract failed
        // and it must at least log that the saved values were kept.
        anyhow::bail!("MCP preferences file is unwritable; saved setup values were not restored");
    }
    let mut prefs = load.file();
    match previous {
        Some(entry) => {
            prefs.servers.insert(server_name.to_string(), entry);
        }
        None => {
            prefs.servers.remove(server_name);
        }
    }
    save_mcp_preferences(&prefs).await
}

#[derive(Debug, Clone)]
pub struct McpSetupServerEntry {
    pub name: String,
    pub config: McpServerConfig,
    pub source: McpPreferenceSource,
}

pub(crate) fn plugin_mcp_servers(
    plugin: &xai_grok_agent::plugins::LoadedPlugin,
) -> xai_grok_config::mcp_servers::PluginMcpServers {
    xai_grok_config::mcp_servers::PluginMcpServers {
        plugin_name: plugin.name.clone(),
        root: plugin.root.clone(),
        data_dir: plugin.data_dir(),
        scope: xai_grok_config::mcp_servers::McpServerScope::from(plugin.scope),
        mcp_config_path: plugin.mcp_config_path.clone(),
        inline_mcp_servers: plugin.inline_mcp_servers.clone(),
    }
}

/// User and project TOML include `enabled = false` so Space-disabled setup servers stay visible.
/// Other sources skip `enabled = false` because Space cannot unstick those flags.
pub(crate) fn collect_mcp_setup_configs(
    cwd: &std::path::Path,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: &CompatConfig,
) -> IndexMap<String, McpSetupServerEntry> {
    let mut result = IndexMap::new();
    for (name, (config, scope)) in load_mcp_server_configs_with_project(cwd) {
        if config.setup.is_none() {
            continue;
        }
        result.insert(
            name.clone(),
            McpSetupServerEntry {
                name,
                config,
                source: McpPreferenceSource {
                    kind: "config".to_string(),
                    plugin: None,
                    scope: Some(scope.to_string()),
                },
            },
        );
    }

    let claude_import = claude_import("collect_mcp_setup_configs");
    if claude_import == ClaudeImport::NotImported {
        insert_compat_setup_configs(
            &mut result,
            xai_grok_config::mcp_servers::load_claude_json_mcp_servers_as_configs(
                cwd,
                compat,
                claude_import,
            ),
            MCP_SCOPE_USER,
        );
        insert_compat_setup_configs(
            &mut result,
            load_cursor_mcp_servers_as_configs(cwd, compat),
            MCP_SCOPE_USER,
        );
        insert_compat_setup_configs(
            &mut result,
            xai_grok_config::mcp_servers::load_mcp_json_servers_as_configs(
                &find_mcp_json_files(cwd),
                claude_import,
            ),
            MCP_SCOPE_PROJECT,
        );
    }

    if let Some(registry) = plugin_registry {
        let toml_claimed_names = all_toml_mcp_server_names(cwd);
        for plugin in registry.active_plugins() {
            let plugin_configs = xai_grok_config::mcp_servers::plugin_setup_server_configs(
                &plugin_mcp_servers(plugin),
            );
            for (name, config) in plugin_configs {
                if toml_claimed_names.contains(&name) || !config.enabled || config.setup.is_none() {
                    continue;
                }
                result.entry(name.clone()).or_insert(McpSetupServerEntry {
                    name,
                    config,
                    source: McpPreferenceSource {
                        kind: "plugin".to_string(),
                        plugin: Some(plugin.name.clone()),
                        scope: None,
                    },
                });
            }
        }
    }
    result
}

fn insert_compat_setup_configs(
    result: &mut IndexMap<String, McpSetupServerEntry>,
    configs: IndexMap<String, McpServerConfig>,
    scope: &'static str,
) {
    for (name, config) in configs {
        if !config.enabled || config.setup.is_none() {
            continue;
        }
        result.entry(name.clone()).or_insert(McpSetupServerEntry {
            name,
            config,
            source: McpPreferenceSource {
                kind: "config".to_string(),
                plugin: None,
                scope: Some(scope.to_string()),
            },
        });
    }
}

fn claude_import(caller: &'static str) -> ClaudeImport {
    ClaudeImport::from_marker(crate::claude_import::is_claude_import_marked_with_log(
        caller,
    ))
}

pub fn mcp_preferences_path() -> PathBuf {
    xai_grok_config::grok_home().join("mcp_preferences.json")
}

/// Result of loading prefs. Corrupt files are readable as empty for resolution
/// but must not be overwritten (would clobber other servers).
#[derive(Debug, Clone)]
pub enum McpPreferencesLoad {
    Ok(McpPreferencesFile),
    Missing,
    Corrupt,
}

impl McpPreferencesLoad {
    pub fn file(&self) -> McpPreferencesFile {
        match self {
            Self::Ok(f) => f.clone(),
            Self::Missing | Self::Corrupt => McpPreferencesFile::default(),
        }
    }

    pub fn is_writable(&self) -> bool {
        !matches!(self, Self::Corrupt)
    }
}

pub fn load_mcp_preferences() -> McpPreferencesLoad {
    load_mcp_preferences_from(&mcp_preferences_path())
}

pub fn load_mcp_preferences_from(path: &std::path::Path) -> McpPreferencesLoad {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return McpPreferencesLoad::Missing,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read MCP preferences");
            return McpPreferencesLoad::Corrupt;
        }
    };
    match serde_json::from_str(&content) {
        Ok(file) => McpPreferencesLoad::Ok(file),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to parse MCP preferences");
            McpPreferencesLoad::Corrupt
        }
    }
}

pub async fn save_mcp_preferences(prefs: &McpPreferencesFile) -> Result<()> {
    save_mcp_preferences_to(&mcp_preferences_path(), prefs).await
}

pub async fn save_mcp_preferences_to(
    path: &std::path::Path,
    prefs: &McpPreferencesFile,
) -> Result<()> {
    if matches!(load_mcp_preferences_from(path), McpPreferencesLoad::Corrupt) {
        anyhow::bail!(
            "refusing to overwrite unreadable MCP preferences at {}",
            path.display()
        );
    }
    let json = serde_json::to_string_pretty(prefs)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = path.with_extension(format!(
        "json.tmp.{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    tokio::fs::write(&tmp, &json).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|e| anyhow::anyhow!("failed to set mcp preferences permissions: {e}"))?;
    }
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

/// Restore a single server key after a failed setup (best-effort).
pub async fn restore_mcp_preference_server(
    server_name: &str,
    previous: Option<McpServerPreferences>,
) -> Result<()> {
    let load = load_mcp_preferences();
    if !load.is_writable() {
        return Ok(());
    }
    let mut prefs = load.file();
    match previous {
        Some(entry) => {
            prefs.servers.insert(server_name.to_string(), entry);
        }
        None => {
            prefs.servers.remove(server_name);
        }
    }
    save_mcp_preferences(&prefs).await
}

/// Unresolved setup-bearing MCP config collected for `/mcps` list and auth.
#[derive(Debug, Clone)]
pub struct McpSetupServerEntry {
    pub name: String,
    pub config: McpServerConfig,
    pub source: McpPreferenceSource,
}

/// Collect MCP configs that declare a `setup` schema from config and plugins.
/// Used to surface setup-required rows and drive `x.ai/mcp/setup`.
pub fn collect_mcp_setup_configs(
    cwd: &std::path::Path,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: &CompatConfig,
) -> IndexMap<String, McpSetupServerEntry> {
    let mut result = IndexMap::new();
    for (name, (config, scope)) in load_mcp_server_configs_with_project(cwd) {
        if !config.enabled || config.setup.is_none() {
            continue;
        }
        result.insert(
            name.clone(),
            McpSetupServerEntry {
                name,
                config,
                source: McpPreferenceSource {
                    kind: "config".to_string(),
                    plugin: None,
                    scope: Some(scope.to_string()),
                },
            },
        );
    }
    if !crate::claude_import::is_claude_import_marked_with_log("collect_mcp_setup_configs") {
        for (name, config) in load_claude_json_mcp_servers_as_configs(cwd, compat) {
            if !config.enabled || config.setup.is_none() {
                continue;
            }
            result.entry(name.clone()).or_insert(McpSetupServerEntry {
                name,
                config,
                source: McpPreferenceSource {
                    kind: "config".to_string(),
                    plugin: None,
                    scope: Some(MCP_SCOPE_USER.to_string()),
                },
            });
        }
        for (name, config) in load_cursor_mcp_servers_as_configs(cwd, compat) {
            if !config.enabled || config.setup.is_none() {
                continue;
            }
            result.entry(name.clone()).or_insert(McpSetupServerEntry {
                name,
                config,
                source: McpPreferenceSource {
                    kind: "config".to_string(),
                    plugin: None,
                    scope: Some(MCP_SCOPE_USER.to_string()),
                },
            });
        }
        for (name, config) in load_mcp_json_servers_as_configs(cwd) {
            if !config.enabled || config.setup.is_none() {
                continue;
            }
            result.entry(name.clone()).or_insert(McpSetupServerEntry {
                name,
                config,
                source: McpPreferenceSource {
                    kind: "config".to_string(),
                    plugin: None,
                    scope: Some(MCP_SCOPE_PROJECT.to_string()),
                },
            });
        }
    }
    if let Some(registry) = plugin_registry {
        let toml_claimed_names = all_toml_mcp_server_names(cwd);
        for plugin in registry.active_plugins() {
            // File first, then inline; first-wins matches runtime plugin load.
            let mut plugin_configs = IndexMap::new();
            if let Some(ref mcp_path) = plugin.mcp_config_path
                && let Some(config) = read_mcp_json(mcp_path)
            {
                for (name, server) in config.mcp_servers {
                    plugin_configs.entry(name).or_insert(server);
                }
            }
            if let Some(ref inline_value) = plugin.inline_mcp_servers {
                let normalized =
                    xai_grok_agent::plugins::manifest::normalize_inline_mcp_servers(inline_value);
                if let Ok(config) = serde_json::from_value::<McpConfig>(normalized) {
                    for (name, server) in config.mcp_servers {
                        plugin_configs.entry(name).or_insert(server);
                    }
                }
            }
            for (name, config) in plugin_configs {
                if toml_claimed_names.contains(&name) || !config.enabled || config.setup.is_none() {
                    continue;
                }
                result.entry(name.clone()).or_insert(McpSetupServerEntry {
                    name,
                    config,
                    source: McpPreferenceSource {
                        kind: "plugin".to_string(),
                        plugin: Some(plugin.name.clone()),
                        scope: None,
                    },
                });
            }
        }
    }
    result
}

pub const MANAGED_GATEWAY_DISABLED_CONNECTORS_KEY: &str = "__managed_gateway_connectors";

/// Uses a dedicated top-level section (not `[mcp_servers]`) to avoid creating incomplete server entries that fail to deserialize for managed servers.
pub(crate) async fn save_mcp_disabled_tools(
    server_name: &str,
    disabled_tools: &[String],
) -> Result<()> {
    let name = server_name.to_string();
    let disabled_tools = disabled_tools.to_vec();
    rmw_mcp_toml(
        &config_path(),
        McpMissing::EmptyTable,
        false,
        move |table| {
            let section = table
                .entry("disabled_mcp_tools")
                .or_insert_with(|| TomlValue::Table(TomlMap::new()))
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("disabled_mcp_tools is not a table"))?;

            if disabled_tools.is_empty() {
                section.remove(&name);
                if section.is_empty() {
                    table.remove("disabled_mcp_tools");
                }
            } else {
                let arr = disabled_tools
                    .iter()
                    .map(|s| TomlValue::String(s.clone()))
                    .collect();
                section.insert(name, TomlValue::Array(arr));
            }
            Ok(true)
        },
    )
    .await?;
    Ok(())
}

/// On enable, if the project unstick fails after a successful user-tier write, this rolls back whatever was already written and returns the error.
pub async fn save_mcp_server_enabled_in(
    server_name: &str,
    enabled: bool,
    cwd: &std::path::Path,
) -> Result<Vec<PathBuf>> {
    let mut modified = Vec::new();

    let user_path = config_path();
    let name = server_name.to_string();
    if write_toml_table_if_changed(&user_path, move |table| {
        apply_mcp_server_enabled(table, &name, enabled);
    })
    .await?
    {
        modified.push(user_path);
    }

    // Disable is personal (user `disabled_mcp_servers`) and must not dirty shared files
    if enabled && let Some(path) = nearest_project_mcp_definition(cwd, server_name) {
        match clear_sticky_project_disabled_at(&path, server_name).await {
            Ok(true) => modified.push(path),
            Ok(false) => {}
            Err(e) => {
                if let Err(re) =
                    restore_mcp_server_enabled_after_enable(server_name, &modified).await
                {
                    tracing::warn!(
                        server = server_name,
                        error = %re,
                        "failed to roll back partial enable after project unstick error"
                    );
                }
                return Err(e);
            }
        }
    }

    Ok(modified)
}

pub(crate) async fn save_user_mcp_server_enabled(server_name: &str, enabled: bool) -> Result<()> {
    let name = server_name.to_string();
    write_toml_table_if_changed(&config_path(), move |table| {
        apply_mcp_server_enabled(table, &name, enabled);
    })
    .await
    .map(|_| ())
}

/// Restores an equivalent disabled state, not necessarily the original encoding: a server disabled only
/// by a sticky project field may pick up a personal `disabled_mcp_servers` entry.
pub(crate) async fn restore_mcp_server_enabled_after_enable(
    server_name: &str,
    modified_paths: &[PathBuf],
) -> Result<()> {
    let user_path = config_path();
    for path in modified_paths {
        if path == user_path.as_path() {
            save_user_mcp_server_enabled(server_name, false).await?;
        } else {
            set_sticky_project_disabled_at(path, server_name).await?;
        }
    }
    Ok(())
}

/// `find_project_configs` lists the cwd last, and the nearest definition wins.
fn nearest_project_mcp_definition(cwd: &std::path::Path, server_name: &str) -> Option<PathBuf> {
    crate::config::find_project_configs(cwd)
        .into_iter()
        .rev()
        .find(|path| mcp_server_defined_at(path, server_name))
}

fn is_user_config_path(path: &std::path::Path) -> bool {
    path == config_path().as_path()
}

#[derive(Clone, Copy)]
enum McpMissing {
    EmptyTable,
    Skip,
}

fn parse_mcp_root(path: &std::path::Path, original: &str) -> Result<TomlValue> {
    super::persist::parse_existing_config_toml(original).map_err(|parse_err| {
        anyhow::anyhow!(
            "refusing to overwrite unparseable {}: {}; fix the syntax before retrying",
            path.display(),
            parse_err
        )
    })
}

fn apply_mcp_rmw(
    path: &std::path::Path,
    original: &str,
    f: impl FnOnce(&mut TomlMap<String, TomlValue>) -> Result<bool>,
    skip_if_unchanged: bool,
    publish: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<bool> {
    let mut root = parse_mcp_root(path, original)?;
    let before = toml::to_string_pretty(&root)?;
    let table = root
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("config root is not a table"))?;
    if !f(table)? {
        return Ok(false);
    }
    let toml_str = toml::to_string_pretty(&root)?;
    if skip_if_unchanged && before == toml_str {
        return Ok(false);
    }
    publish(&toml_str).map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))?;
    Ok(true)
}

fn rmw_mcp_toml_user(
    path: &std::path::Path,
    f: impl FnOnce(&mut TomlMap<String, TomlValue>) -> Result<bool>,
    skip_if_unchanged: bool,
) -> Result<bool> {
    let (dest, original) = super::persist::read_follow_bound(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
    apply_mcp_rmw(path, &original, f, skip_if_unchanged, |s| {
        super::persist::atomic_write_follow_bound(path, &dest, s)
    })
}

fn rmw_mcp_toml_project(
    path: &std::path::Path,
    missing: McpMissing,
    f: impl FnOnce(&mut TomlMap<String, TomlValue>) -> Result<bool>,
    skip_if_unchanged: bool,
) -> Result<bool> {
    let original = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match missing {
            McpMissing::Skip => return Ok(false),
            McpMissing::EmptyTable => String::new(),
        },
        Err(e) => {
            return Err(anyhow::anyhow!("failed to read {}: {e}", path.display()));
        }
    };
    apply_mcp_rmw(path, &original, f, skip_if_unchanged, |s| {
        super::persist::atomic_replace_string(path, s)
    })
}

async fn rmw_mcp_toml(
    path: &std::path::Path,
    missing: McpMissing,
    skip_if_unchanged: bool,
    f: impl FnOnce(&mut TomlMap<String, TomlValue>) -> Result<bool> + Send + 'static,
) -> Result<bool> {
    let is_user = is_user_config_path(path);
    let path = path.to_path_buf();
    if is_user {
        let guard = super::persist::lock_config_writes().await?;
        guard
            .run_blocking(move || rmw_mcp_toml_user(&path, f, skip_if_unchanged))
            .await
            .map_err(|e| anyhow::anyhow!("config write task failed: {e}"))?
    } else {
        tokio::task::spawn_blocking(move || {
            rmw_mcp_toml_project(&path, missing, f, skip_if_unchanged)
        })
        .await
        .map_err(|e| anyhow::anyhow!("config write task failed: {e}"))?
    }
}

/// Production MCP saves go through [`rmw_mcp_toml`]; this remains for the
/// follow-symlink regression that pins the one-shot path.
#[cfg(test)]
async fn persist_mcp_toml(path: &std::path::Path, toml_str: &str) -> Result<()> {
    let is_user = is_user_config_path(path);
    let path = path.to_path_buf();
    let toml_str = toml_str.to_string();
    if is_user {
        let guard = super::persist::lock_config_writes().await?;
        guard
            .run_blocking(move || {
                super::persist::atomic_write_string(&path, &toml_str)
                    .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))
            })
            .await
            .map_err(|e| anyhow::anyhow!("config write task failed: {e}"))?
    } else {
        tokio::task::spawn_blocking(move || {
            super::persist::atomic_replace_string(&path, &toml_str)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))
        })
        .await
        .map_err(|e| anyhow::anyhow!("config write task failed: {e}"))?
    }
}

async fn write_toml_table_if_changed(
    path: &std::path::Path,
    f: impl FnOnce(&mut TomlMap<String, TomlValue>) + Send + 'static,
) -> Result<bool> {
    let missing = if is_user_config_path(path) {
        McpMissing::EmptyTable
    } else {
        McpMissing::Skip
    };
    rmw_mcp_toml(path, missing, true, move |table| {
        f(table);
        Ok(true)
    })
    .await
}

/// Uses `toml_edit` so the shared project file keeps its comments and layout.
async fn clear_sticky_project_disabled_at(
    path: &std::path::Path,
    server_name: &str,
) -> Result<bool> {
    let original = match tokio::fs::read_to_string(path).await {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(anyhow::anyhow!("failed to read {}: {e}", path.display()));
        }
    };
    let mut doc: toml_edit::DocumentMut = original
        .parse()
        .map_err(|e| anyhow::anyhow!("refusing to rewrite unparseable {}: {e}", path.display()))?;

    let Some(servers) = doc
        .get_mut("mcp_servers")
        .and_then(|item| item.as_table_like_mut())
    else {
        return Ok(false);
    };
    let Some(entry) = servers.get_mut(server_name) else {
        return Ok(false);
    };
    let Some(server_table) = entry.as_table_like_mut() else {
        return Ok(false);
    };
    if server_table.get("enabled").and_then(|v| v.as_bool()) != Some(false) {
        return Ok(false);
    }
    server_table.insert("enabled", toml_edit::value(true));

    let updated = doc.to_string();
    if updated == original {
        return Ok(false);
    }
    super::persist::atomic_replace_string(path, &updated)
        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))?;
    Ok(true)
}

async fn set_sticky_project_disabled_at(path: &std::path::Path, server_name: &str) -> Result<bool> {
    let original = match tokio::fs::read_to_string(path).await {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(anyhow::anyhow!("failed to read {}: {e}", path.display()));
        }
    };
    let mut doc: toml_edit::DocumentMut = original
        .parse()
        .map_err(|e| anyhow::anyhow!("refusing to rewrite unparseable {}: {e}", path.display()))?;

    let Some(servers) = doc
        .get_mut("mcp_servers")
        .and_then(|item| item.as_table_like_mut())
    else {
        return Ok(false);
    };
    let Some(entry) = servers.get_mut(server_name) else {
        return Ok(false);
    };
    let Some(server_table) = entry.as_table_like_mut() else {
        return Ok(false);
    };
    if server_table.get("enabled").and_then(|v| v.as_bool()) != Some(true) {
        return Ok(false);
    }
    server_table.insert("enabled", toml_edit::value(false));

    let updated = doc.to_string();
    if updated == original {
        return Ok(false);
    }
    super::persist::atomic_replace_string(path, &updated)
        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))?;
    Ok(true)
}

fn apply_mcp_server_enabled(
    table: &mut TomlMap<String, TomlValue>,
    server_name: &str,
    enabled: bool,
) {
    let mut disabled_list: Vec<String> = table
        .get("disabled_mcp_servers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if enabled {
        disabled_list.retain(|n| n != server_name);
    } else if !disabled_list.contains(&server_name.to_string()) {
        disabled_list.push(server_name.to_string());
    }

    if disabled_list.is_empty() {
        table.remove("disabled_mcp_servers");
    } else {
        let arr = disabled_list
            .iter()
            .map(|s| TomlValue::String(s.clone()))
            .collect();
        table.insert("disabled_mcp_servers".to_string(), TomlValue::Array(arr));
    }

    set_mcp_server_enabled_field(table, server_name, enabled);
}

fn set_mcp_server_enabled_field(
    table: &mut TomlMap<String, TomlValue>,
    server_name: &str,
    enabled: bool,
) {
    if let Some(servers) = table.get_mut("mcp_servers").and_then(|v| v.as_table_mut())
        && let Some(entry) = servers.get_mut(server_name)
        && let Some(server_table) = entry.as_table_mut()
    {
        server_table.insert("enabled".to_string(), TomlValue::Boolean(enabled));
    }
}

/// Also removes the server from `disabled_mcp_servers`, so a newly defined server starts enabled.
pub(crate) async fn save_mcp_server_config(
    server_name: &str,
    config: &McpServerConfig,
) -> Result<()> {
    save_mcp_server_config_at(&config_path(), server_name, config).await
}

pub async fn save_mcp_server_config_at(
    path: &std::path::Path,
    server_name: &str,
    config: &McpServerConfig,
) -> Result<()> {
    let name = server_name.to_string();
    let serialized = toml::Value::try_from(config)
        .map_err(|e| anyhow::anyhow!("failed to serialize MCP server config: {e}"))?;
    rmw_mcp_toml(path, McpMissing::EmptyTable, false, move |table| {
        let servers = table
            .entry("mcp_servers")
            .or_insert_with(|| TomlValue::Table(TomlMap::new()))
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("mcp_servers is not a table"))?;
        servers.insert(name.clone(), serialized);

        if let Some(arr) = table
            .get_mut("disabled_mcp_servers")
            .and_then(|v| v.as_array_mut())
        {
            arr.retain(|v| v.as_str() != Some(name.as_str()));
            if arr.is_empty() {
                table.remove("disabled_mcp_servers");
            }
        }
        Ok(true)
    })
    .await?;
    Ok(())
}

pub(crate) async fn delete_mcp_server_config(server_name: &str) -> Result<bool> {
    delete_mcp_server_config_at(&config_path(), server_name).await
}

/// OAuth credential cleanup is keyed by server name against the global credential store.
/// It therefore also drops credentials a same-named server in another config file uses.
pub async fn delete_mcp_server_config_at(
    path: &std::path::Path,
    server_name: &str,
) -> Result<bool> {
    let name = server_name.to_string();
    let wrote = rmw_mcp_toml(path, McpMissing::Skip, false, move |table| {
        let existed = table
            .get_mut("mcp_servers")
            .and_then(|v| v.as_table_mut())
            .and_then(|servers| servers.remove(&name))
            .is_some();

        if !existed {
            return Ok(false);
        }

        if table
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .is_some_and(|t| t.is_empty())
        {
            table.remove("mcp_servers");
        }

        if let Some(arr) = table
            .get_mut("disabled_mcp_servers")
            .and_then(|v| v.as_array_mut())
        {
            arr.retain(|v| v.as_str() != Some(name.as_str()));
            if arr.is_empty() {
                table.remove("disabled_mcp_servers");
            }
        }

        if let Some(section) = table
            .get_mut("disabled_mcp_tools")
            .and_then(|v| v.as_table_mut())
        {
            section.remove(&name);
            if section.is_empty() {
                table.remove("disabled_mcp_tools");
            }
        }
        Ok(true)
    })
    .await?;

    if wrote
        && let Ok(mut cred_store) = xai_grok_mcp::credentials::McpCredentialStore::load_default()
    {
        let removed = cred_store.remove_by_server_name(server_name);
        if removed > 0 {
            let _ = cred_store.save_default();
        }
    }

    Ok(wrote)
}

pub(crate) fn get_all_mcp_disabled_tools(
    _cwd: &std::path::Path,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    xai_grok_config::mcp_servers::get_all_mcp_disabled_tools()
}

<<<<<<< HEAD
=======
/// Load all configured MCP servers as `(name, config)` pairs.
///
/// Reads from `load_effective_config()`, which merges the system-managed,
/// managed, and user config layers only. Use
/// [`load_mcp_server_configs_with_project`] for a view that also includes
/// project-scoped `.grok/config.toml` files.
pub fn load_mcp_server_configs() -> IndexMap<String, McpServerConfig> {
    let root =
        crate::config::load_effective_config().unwrap_or_else(|_| TomlValue::Table(TomlMap::new()));
    parse_mcp_servers_from_toml(&root)
}

fn parse_mcp_servers_from_toml(root: &TomlValue) -> IndexMap<String, McpServerConfig> {
    let TomlValue::Table(table) = root else {
        return IndexMap::new();
    };
    let Some(TomlValue::Table(mcp_servers)) = table.get("mcp_servers") else {
        return IndexMap::new();
    };

    let mut result = IndexMap::new();
    for (name, value) in mcp_servers {
        if let Ok(config) = toml::Value::try_into::<McpServerConfig>(value.clone()) {
            result.insert(name.clone(), config);
        }
    }
    result
}

// ── .mcp.json support ────────────────────────────────────────────────

// `.mcp.json` discovery moved to `xai-grok-workspace` (client-side, shared with
// the folder-trust gate); re-exported so `crate::util::config::*` paths keep working.
pub use xai_grok_workspace::project_config::{
    MCP_JSON_FILENAME, find_mcp_json_files, mcp_json_candidate_paths,
};

pub fn load_mcp_json_file(path: &std::path::Path) -> Vec<acp::McpServer> {
    if !path.is_file() {
        return vec![];
    }
    let Some(value) = read_mcp_json(path) else {
        return vec![];
    };
    let label = path.display().to_string();
    parse_mcp_config(&value, &label, &crate::config::expand_env_vars_in_string)
}
/// Load .mcp.json servers as McpServerConfig map (for merging into load_mcp_servers).
pub(crate) fn load_mcp_json_servers_as_configs(
    cwd: &std::path::Path,
) -> IndexMap<String, McpServerConfig> {
    // Phase 2 cutoff: if the user has imported, skip reading .mcp.json.
    if crate::claude_import::is_claude_import_marked_with_log("load_mcp_json_servers_as_configs") {
        return IndexMap::new();
    }
    load_mcp_json_servers_as_configs_unfiltered(cwd)
}

/// Like [`load_mcp_json_servers_as_configs`] but bypasses the import-marker
/// gate. Used by the `/import-claude` scanner so users can re-import items
/// they previously skipped, even after the runtime cutoff is active.
pub fn load_mcp_json_servers_as_configs_unfiltered(
    cwd: &std::path::Path,
) -> IndexMap<String, McpServerConfig> {
    let mcp_json_files = find_mcp_json_files(cwd);
    if mcp_json_files.is_empty() {
        return IndexMap::new();
    }

    let mut result = IndexMap::new();

    // Reverse so cwd entries win on name conflict.
    for mcp_path in mcp_json_files.iter().rev() {
        if let Some(config) = read_mcp_json(mcp_path) {
            for (name, cfg) in config.mcp_servers {
                result.entry(name).or_insert(cfg);
            }
        }
    }

    result
}

pub(crate) fn parse_mcp_config(
    config: &McpConfig,
    source_label: &str,
    sub: &dyn Fn(&str) -> String,
) -> Vec<acp::McpServer> {
    parse_mcp_config_with_oauth(config, source_label, sub).0
}

pub(crate) fn parse_mcp_config_with_oauth(
    config: &McpConfig,
    source_label: &str,
    sub: &dyn Fn(&str) -> String,
) -> (Vec<acp::McpServer>, McpOAuthConfigMap) {
    let preferences = load_mcp_preferences().file();
    let mut servers = Vec::new();
    let mut oauth_configs = McpOAuthConfigMap::new();
    for (name, server_config) in &config.mcp_servers {
        let mut server_config = match server_config.resolve_setup(preferences.servers.get(name)) {
            McpSetupResolution::Resolved(config) => config,
            McpSetupResolution::Required(_) => continue,
            McpSetupResolution::Invalid(reason) => {
                tracing::warn!(
                    source = source_label,
                    server = %name,
                    error = %reason,
                    "MCP setup config is invalid"
                );
                continue;
            }
        };
        server_config.expand_strings(sub);
        if let Some(oauth) = server_config.oauth_config() {
            oauth_configs.insert(name.clone(), oauth);
        }
        if let Some(server) = server_config.to_acp_mcp_server(name.clone()) {
            servers.push(server);
        } else {
            tracing::warn!(
                source = source_label,
                server = name,
                "MCP server has no 'command' (stdio) or 'url' (http/sse); skipping"
            );
        }
    }

    if !servers.is_empty() {
        tracing::info!(
            source = source_label,
            count = servers.len(),
            "loaded MCP servers"
        );
    }

    (servers, oauth_configs)
}

/// Load MCP servers from `~/.claude.json`.
///
/// User-level MCP servers live at the top-level `mcpServers` key,
/// and per-project (local-scope) MCP servers under `projects.<cwd>.mcpServers`.
///
/// Returns servers from both locations (project-specific first, then user-level).
pub fn load_claude_json_mcp_servers(
    cwd: &std::path::Path,
    compat: &CompatConfig,
) -> Vec<acp::McpServer> {
    // Compat gate: skip ~/.claude.json MCP loading when disabled.
    if !compat.claude.mcps {
        return vec![];
    }
    // Phase 2 cutoff: if the user has imported, skip reading ~/.claude.json.
    if crate::claude_import::is_claude_import_marked_with_log("load_claude_json_mcp_servers") {
        return vec![];
    }

    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    let claude_json_path = home.join(".claude.json");
    load_claude_json_mcp_servers_from(&claude_json_path, cwd)
}
/// Load ~/.claude.json MCP servers as McpServerConfig map (for merging into load_mcp_servers).
pub(crate) fn load_claude_json_mcp_servers_as_configs(
    cwd: &std::path::Path,
    compat: &CompatConfig,
) -> IndexMap<String, McpServerConfig> {
    // Compat gate: skip ~/.claude.json MCP loading when disabled.
    if !compat.claude.mcps {
        return IndexMap::new();
    }
    // Phase 2 cutoff: if the user has imported, skip reading ~/.claude.json.
    if crate::claude_import::is_claude_import_marked_with_log(
        "load_claude_json_mcp_servers_as_configs",
    ) {
        return IndexMap::new();
    }
    load_claude_json_mcp_servers_as_configs_unfiltered(cwd)
}

/// Like [`load_claude_json_mcp_servers_as_configs`] but bypasses the
/// import-marker gate. Used by the `/import-claude` scanner so users can
/// re-import items they previously skipped, even after the runtime cutoff
/// is active.
pub fn load_claude_json_mcp_servers_as_configs_unfiltered(
    cwd: &std::path::Path,
) -> IndexMap<String, McpServerConfig> {
    let Some(home) = dirs::home_dir() else {
        return IndexMap::new();
    };
    let claude_json_path = home.join(".claude.json");
    load_claude_json_mcp_servers_from_as_configs(&claude_json_path, cwd)
}

fn load_claude_json_mcp_servers_from_as_configs(
    claude_json_path: &std::path::Path,
    cwd: &std::path::Path,
) -> IndexMap<String, McpServerConfig> {
    let content = match std::fs::read_to_string(claude_json_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(
                path = %claude_json_path.display(),
                error = %e,
                "failed to read ~/.claude.json"
            );
            return IndexMap::new();
        }
    };
    let config: ClaudeJsonConfig = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(
                path = %claude_json_path.display(),
                error = %e,
                "failed to parse ~/.claude.json"
            );
            return IndexMap::new();
        }
    };

    let mut result = IndexMap::new();

    // Per-project MCP servers (local scope, higher priority)
    let cwd_key = cwd.to_string_lossy();
    if let Some(project) = config.projects.get(cwd_key.as_ref()) {
        for (name, cfg) in &project.mcp_servers {
            result.insert(name.clone(), cfg.clone());
        }
    }

    // User-level MCP servers (lower priority)
    for (name, cfg) in &config.user_mcp.mcp_servers {
        result.entry(name.clone()).or_insert(cfg.clone());
    }
    tracing::info!(
        project_count = config
            .projects
            .get(cwd_key.as_ref())
            .map(|p| p.mcp_servers.len())
            .unwrap_or(0),
        user_level_count = config.user_mcp.mcp_servers.len(),
        total_count = result.len(),
        "MCP servers loaded from ~/.claude.json"
    );

    result
}

/// Load MCP servers from editor MCP config files.
///
/// Scans project-level `<cwd>/.cursor/mcp.json` first (higher priority),
/// then global `~/.cursor/mcp.json`. Both use the `{"mcpServers": {...}}`
/// format identical to `.mcp.json`. Gated by `compat.cursor.mcps`.
pub fn load_cursor_mcp_servers(
    cwd: &std::path::Path,
    compat: &CompatConfig,
) -> Vec<acp::McpServer> {
    // Compat gate: skip Cursor MCP loading when disabled.
    if !compat.cursor.mcps {
        return vec![];
    }
    let mut result = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    // Project-level (higher priority)
    let project_path = cwd.join(".cursor").join("mcp.json");
    for server in load_mcp_json_file(&project_path) {
        let name = match &server {
            acp::McpServer::Http(acp::McpServerHttp { name, .. })
            | acp::McpServer::Sse(acp::McpServerSse { name, .. })
            | acp::McpServer::Stdio(acp::McpServerStdio { name, .. }) => name.clone(),
            // TODO(acp-0.10): `McpServer` is #[non_exhaustive].
            _ => continue,
        };
        if seen_names.insert(name) {
            result.push(server);
        }
    }

    // Global (lower priority)
    if let Some(home) = dirs::home_dir() {
        let global_path = home.join(".cursor").join("mcp.json");
        for server in load_mcp_json_file(&global_path) {
            let name = match &server {
                acp::McpServer::Http(acp::McpServerHttp { name, .. })
                | acp::McpServer::Sse(acp::McpServerSse { name, .. })
                | acp::McpServer::Stdio(acp::McpServerStdio { name, .. }) => name.clone(),
                // TODO(acp-0.10): `McpServer` is #[non_exhaustive].
                _ => continue,
            };
            if seen_names.insert(name) {
                result.push(server);
            }
        }
    }

    result
}

/// Load Cursor MCP servers as McpServerConfig map (for merging into load_mcp_servers).
///
/// Scans project-level `<cwd>/.cursor/mcp.json` first, then global.
pub(crate) fn load_cursor_mcp_servers_as_configs(
    cwd: &std::path::Path,
    compat: &CompatConfig,
) -> IndexMap<String, McpServerConfig> {
    // Compat gate: skip Cursor MCP loading when disabled.
    if !compat.cursor.mcps {
        return IndexMap::new();
    }
    let mut result = IndexMap::new();

    // Project-level (higher priority)
    let project_path = cwd.join(".cursor").join("mcp.json");
    if project_path.is_file()
        && let Some(config) = read_mcp_json(&project_path)
    {
        for (name, cfg) in config.mcp_servers {
            result.insert(name, cfg);
        }
    }

    // Global (lower priority — or_insert so project wins)
    if let Some(home) = dirs::home_dir() {
        let global_path = home.join(".cursor").join("mcp.json");
        if global_path.is_file()
            && let Some(config) = read_mcp_json(&global_path)
        {
            for (name, cfg) in config.mcp_servers {
                result.entry(name).or_insert(cfg);
            }
        }
    }

    result
}

/// Subset of `~/.claude.json` we care about for MCP server discovery.
///
/// Reuses `McpConfig` for both the top-level user MCP servers and per-project
/// entries — the JSON shape (`{ "mcpServers": { ... } }`) is identical at both levels.
#[derive(Default, Deserialize)]
struct ClaudeJsonConfig {
    /// User-level MCP servers (top-level `mcpServers` key).
    #[serde(flatten)]
    user_mcp: McpConfig,
    /// Per-project entries, keyed by absolute project path.
    #[serde(default)]
    projects: HashMap<String, McpConfig>,
}

/// Inner implementation that accepts the file path, making it testable.
fn load_claude_json_mcp_servers_from(
    claude_json_path: &std::path::Path,
    cwd: &std::path::Path,
) -> Vec<acp::McpServer> {
    let content = match std::fs::read_to_string(claude_json_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let config: ClaudeJsonConfig = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(
                path = %claude_json_path.display(),
                error = %e,
                "failed to parse claude.json"
            );
            return vec![];
        }
    };

    let sub = &crate::config::expand_env_vars_in_string;
    let mut servers = Vec::new();

    // Per-project MCP servers (local scope, higher priority)
    let cwd_key = cwd.to_string_lossy();
    if let Some(project) = config.projects.get(cwd_key.as_ref()) {
        let label = format!("~/.claude.json projects[{}]", cwd_key);
        servers.extend(parse_mcp_config(project, &label, sub));
    }

    // User-level MCP servers (lower priority)
    if !config.user_mcp.mcp_servers.is_empty() {
        servers.extend(parse_mcp_config(&config.user_mcp, "~/.claude.json", sub));
    }

    servers
}

/// Read and parse a JSON file. Returns `None` on I/O or parse errors (logged).
pub(crate) fn read_mcp_json(path: &std::path::Path) -> Option<McpConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| {
            tracing::warn!(error = %e, "failed to read MCP JSON");
        })
        .ok()?;
    serde_json::from_str(&content)
        .map_err(|e| {
            tracing::warn!(error = %e, "failed to parse MCP JSON");
        })
        .ok()
}

/// Like `load_mcp_servers_with_project` but returns raw configs without filtering by `enabled`.
fn load_all_mcp_configs(cwd: &std::path::Path) -> IndexMap<String, McpServerConfig> {
    load_mcp_server_configs_with_project(cwd)
        .into_iter()
        .map(|(name, (config, _))| (name, config))
        .collect()
}

/// Load all configured MCP servers with the scope each definition came from
/// (`"user"` or `"project"`).
///
/// Overlays project-scoped `.grok/config.toml` files from `cwd` up to the
/// repo root onto the user-tier config, nearest definition winning — the same
/// override semantics as [`get_mcp_server_config_with_project`].
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub fn load_mcp_server_configs_with_project(
    cwd: &std::path::Path,
) -> IndexMap<String, (McpServerConfig, &'static str)> {
    xai_grok_config::mcp_servers::load_mcp_server_configs_with_project(
        &crate::config::find_project_configs(cwd),
    )
}

pub(crate) fn load_mcp_server_problems_with_project(
    cwd: &std::path::Path,
) -> Vec<McpServerConfigProblem> {
    xai_grok_config::mcp_servers::load_mcp_server_problems_with_project(
        &crate::config::find_project_configs(cwd),
    )
}

/// Excludes gateway connectors (`managed_gateway:…`), which the `/mcps` Space disables through
/// `disabled_mcp_tools.__managed_gateway_connectors`. A `grok_com_*` name is known only when a
/// definition declares it, not by its prefix.
pub fn cli_known_mcp_server_names(cwd: &std::path::Path) -> std::collections::HashSet<String> {
    let mut names = disabled_mcp_server_names(cwd);
    // The merge drops setup-required and invalid entries, which `mcp list` still shows
    names.extend(all_toml_mcp_server_names(cwd));

    let registry = load_cli_plugin_registry(cwd);
    let compat = CompatConfig::default();
    for (server, _) in crate::session::managed_mcp::merge_managed_mcp_servers_sourced(
        cwd,
        Some(&registry),
        &compat,
    ) {
        let name = crate::session::managed_mcp::mcp_server_name(&server);
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    names
}

pub fn disabled_mcp_server_names(cwd: &std::path::Path) -> std::collections::HashSet<String> {
    xai_grok_config::mcp_servers::disabled_mcp_server_names(&crate::config::find_project_configs(
        cwd,
    ))
}

pub(crate) fn all_toml_mcp_server_names(
    cwd: &std::path::Path,
) -> std::collections::HashSet<String> {
    xai_grok_config::mcp_servers::all_toml_mcp_server_names(&crate::config::find_project_configs(
        cwd,
    ))
}

/// Resolves the same cwd-effective `[plugins]` table as session startup, so trusted project plugins are included.
pub fn load_cli_plugin_registry(cwd: &std::path::Path) -> xai_grok_agent::plugins::PluginRegistry {
    let trust_store = xai_grok_agent::plugins::TrustStore::load();
    // Resolve/record the folder-trust verdict first: the effective-plugins resolve below gates project [plugins].paths on it
    let project_trusted = crate::agent::folder_trust::resolve_and_record(cwd, None, false);
    let mut plugin_config = xai_grok_workspace::plugins::resolve_effective_plugins_config(
        xai_grok_workspace::plugins::PluginConfigInputs {
            effective_config: crate::config::load_effective_config().ok().as_ref(),
            home: xai_dirs::home_dir().as_deref(),
            grok_home: xai_grok_config::user_grok_home().as_deref(),
            cwd,
            trust: xai_grok_hooks::trust::Trust::from_verdict(project_trusted),
            claude_import: crate::claude_import::import_marker(),
        },
    );
    let discovered = xai_grok_agent::plugins::discover_plugins(
        Some(cwd),
        &plugin_config,
        &trust_store,
        project_trusted,
    );
    plugin_config.populate_plugin_lists(&discovered);
    xai_grok_agent::plugins::PluginRegistry::from_discovered(
        discovered,
        &plugin_config.disabled,
        &plugin_config.enabled,
    )
}

fn config_path() -> PathBuf {
    // Live `$GROK_HOME` first: `grok_home()` is OnceLock and misses EnvGuard/tests.
    xai_dirs::resolve_grok_home()
        .unwrap_or_else(crate::util::grok_home::grok_home)
        .join("config.toml")
}

pub fn user_config_path() -> PathBuf {
    config_path()
}

pub fn project_config_path(dir: &std::path::Path) -> PathBuf {
    dir.join(".grok").join("config.toml")
}

/// Checks raw key presence rather than deserializing, so malformed entries (the ones users most need `mcp remove` for) are still reported.
pub fn mcp_server_defined_at(path: &std::path::Path, server_name: &str) -> bool {
    let Ok(root) = crate::config::load_config_file(path) else {
        return false;
    };
    root.get("mcp_servers")
        .and_then(|v| v.as_table())
        .is_some_and(|servers| servers.contains_key(server_name))
}

pub fn load_npm_registry_sync() -> Option<String> {
    let root: TomlValue = crate::config::load_effective_config().ok()?;
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(cli)) = table.get("cli")
    {
        cli.get("npm_registry")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}

pub(crate) fn load_gcs_service_account_key_sync() -> Option<String> {
    let root: TomlValue = crate::config::load_effective_config().ok()?;
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(endpoints)) = table.get("endpoints")
    {
        endpoints
            .get("gcs_service_account_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}
/// `None` lets callers fall through to a remote flag when the user hasn't expressed a local preference.
pub fn use_leader_from_toml_opt(root: &TomlValue) -> Option<bool> {
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(cli)) = table.get("cli")
    {
        cli.get("use_leader").and_then(|v| v.as_bool())
    } else {
        None
    }
}

pub fn use_leader_from_toml(root: &TomlValue) -> bool {
    use_leader_from_toml_opt(root).unwrap_or(false)
}

pub(crate) fn session_registry_from_toml_opt(root: &TomlValue) -> Option<bool> {
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(cli)) = table.get("cli")
    {
        cli.get("session_registry").and_then(|v| v.as_bool())
    } else {
        None
    }
}

/// Overrides `[cli] session_registry`; usable before `~/.grok/config.toml` exists.
pub const SESSION_REGISTRY_ENV_VAR: &str = "GROK_SESSION_REGISTRY";

pub(crate) fn session_registry_from_env_opt() -> Option<bool> {
    xai_grok_config::env_bool(SESSION_REGISTRY_ENV_VAR)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrySource {
    Env,
    ConfigToml,
}

impl RegistrySource {
    pub const fn label(self) -> &'static str {
        match self {
            RegistrySource::Env => SESSION_REGISTRY_ENV_VAR,
            RegistrySource::ConfigToml => "[cli] session_registry",
        }
    }
}

/// Env var, then `[cli] session_registry`; `None` defers to remote settings.
pub fn session_registry_local_override_sourced(
    root: Option<&TomlValue>,
) -> Option<(bool, RegistrySource)> {
    if let Some(v) = session_registry_from_env_opt() {
        return Some((v, RegistrySource::Env));
    }
    root.and_then(session_registry_from_toml_opt)
        .map(|v| (v, RegistrySource::ConfigToml))
}

pub(crate) fn session_registry_local_override(root: Option<&TomlValue>) -> Option<bool> {
    session_registry_local_override_sourced(root).map(|(v, _)| v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use toml::Value as TomlValue;

    #[test]
    #[serial_test::serial]
    fn session_registry_local_override_precedence() {
        let toml_true: TomlValue = toml::from_str("[cli]\nsession_registry = true").unwrap();
        {
            let _g = xai_grok_test_support::EnvGuard::set(SESSION_REGISTRY_ENV_VAR, "false");
            assert_eq!(
                session_registry_local_override_sourced(Some(&toml_true)),
                Some((false, RegistrySource::Env)),
                "env wins and reports itself as the source"
            );
        }
        {
            let _g = xai_grok_test_support::EnvGuard::set(SESSION_REGISTRY_ENV_VAR, "bogus");
            assert_eq!(
                session_registry_local_override_sourced(Some(&toml_true)),
                Some((true, RegistrySource::ConfigToml)),
                "unrecognized env values defer to config.toml"
            );
        }
        {
            let _g = xai_grok_test_support::EnvGuard::unset(SESSION_REGISTRY_ENV_VAR);
            assert_eq!(session_registry_local_override_sourced(None), None);
        }
    }

    #[test]
    #[serial_test::serial]
    fn load_cli_plugin_registry_includes_project_config_path_plugins() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());

        let repo = tempfile::tempdir().unwrap();
        git2::Repository::init(repo.path()).unwrap();

        let plugin_dir = repo.path().join("proj-plugin");
        let agents_dir = plugin_dir.join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "proj-plugin", "agents": "./agents"}"#,
        )
        .unwrap();
        std::fs::write(
            agents_dir.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Project reviewer\n---\nBody.\n",
        )
        .unwrap();

        let grok = repo.path().join(".grok");
        std::fs::create_dir_all(&grok).unwrap();
        std::fs::write(
            grok.join("config.toml"),
            format!("[plugins]\npaths = [\"{}\"]\n", plugin_dir.display()),
        )
        .unwrap();

        let registry = load_cli_plugin_registry(repo.path());
        assert!(
            registry.get("proj-plugin").is_some(),
            "project [plugins].paths plugin must be discovered"
        );
        let agents = xai_grok_agent::discovery::plugin_agents(&registry);
        assert!(
            agents
                .iter()
                .any(|a| a.qualified_name == "proj-plugin:reviewer"),
            "project config-path plugin agent must be enumerable, got: {:?}",
            agents.iter().map(|a| &a.qualified_name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn mcp_server_defined_at_checks_raw_key_presence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[mcp_servers.broken]\nurll = \"https://x.example\"\n",
        )
        .unwrap();

        assert!(mcp_server_defined_at(&path, "broken"));
        assert!(!mcp_server_defined_at(&path, "other"));
        assert!(!mcp_server_defined_at(
            &dir.path().join("missing.toml"),
            "broken"
        ));
    }

    #[test]
    fn test_use_leader_parsing_true() {
        let toml_str = r#"
[cli]
use_leader = true
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert!(use_leader_from_toml(&root));
    }

    #[test]
    fn test_use_leader_parsing_false() {
        let toml_str = r#"
[cli]
use_leader = false
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert!(!use_leader_from_toml(&root));
    }

    #[test]
    fn test_use_leader_default_false() {
        let toml_str = r#"
[cli]
auto_update = true
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert!(!use_leader_from_toml(&root));
    }

    #[test]
    fn test_use_leader_no_cli_section() {
        let toml_str = r#"
[models]
default = "grok-code-fast-1"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        if let TomlValue::Table(ref table) = root {
            let has_cli = table.get("cli").is_some();
            assert!(!has_cli);
        }
        assert!(!use_leader_from_toml(&root));
    }

    #[test]
    fn test_use_leader_opt_returns_some_true() {
        let toml_str = r#"
[cli]
use_leader = true
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(use_leader_from_toml_opt(&root), Some(true));
    }

    #[test]
    fn test_use_leader_opt_returns_some_false() {
        let toml_str = r#"
[cli]
use_leader = false
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(use_leader_from_toml_opt(&root), Some(false));
    }

    #[test]
    fn test_use_leader_opt_returns_none_when_absent() {
        let toml_str = r#"
[cli]
auto_update = true
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(use_leader_from_toml_opt(&root), None);
    }

    #[test]
    fn test_use_leader_opt_returns_none_when_no_cli_section() {
        let toml_str = r#"
[models]
default = "grok-code-fast-1"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(use_leader_from_toml_opt(&root), None);
    }

    #[test]
    fn skills_config_default_is_empty() {
        let cfg = SkillsConfig::default();
        assert!(cfg.paths.is_empty());
        assert!(cfg.ignore.is_empty());
    }

    #[test]
    fn skills_config_parses_paths_and_ignore() {
        let root = toml::from_str::<TomlValue>(
            r#"
[skills]
paths = ["~/.grok/skills", "~/.grok/skills/special/SKILL.md"]
ignore = ["~/.grok/skills/noisy/SKILL.md"]
"#,
        )
        .unwrap();
        let TomlValue::Table(ref table) = root else {
            panic!()
        };
        let cfg = table
            .get("skills")
            .and_then(|v| v.clone().try_into::<SkillsConfig>().ok())
            .unwrap_or_default();
        assert_eq!(
            cfg.paths,
            vec!["~/.grok/skills", "~/.grok/skills/special/SKILL.md"]
        );
        assert_eq!(cfg.ignore, vec!["~/.grok/skills/noisy/SKILL.md"]);
    }

    #[test]
    fn mcp_json_candidate_paths_include_missing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        git2::Repository::init(tmp.path()).unwrap();

        let paths = mcp_json_candidate_paths(&nested);
        assert_eq!(
            paths,
            vec![
                tmp.path().join(".mcp.json"),
                tmp.path().join("a").join(".mcp.json"),
                nested.join(".mcp.json"),
            ]
        );
    }

    #[tokio::test]
<<<<<<< HEAD
    async fn mcp_preferences_save_refuses_corrupt_file_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp_preferences.json");
        std::fs::write(&path, "not json").unwrap();
=======
    async fn mcp_preferences_missing_malformed_and_save_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp_preferences.json");
        assert!(matches!(
            load_mcp_preferences_from(&path),
            McpPreferencesLoad::Missing
        ));
        assert!(load_mcp_preferences_from(&path).file().servers.is_empty());

        std::fs::write(&path, "not json").unwrap();
        assert!(matches!(
            load_mcp_preferences_from(&path),
            McpPreferencesLoad::Corrupt
        ));
>>>>>>> e3fdf3ed (Merge 2 (#4))
        let prefs = McpPreferencesFile {
            version: 1,
            servers: HashMap::from([(
                "acme".to_string(),
                McpServerPreferences {
                    values: HashMap::from([("site".to_string(), "us5".to_string())]),
                    source: Some(McpPreferenceSource {
                        kind: "plugin".to_string(),
                        plugin: Some("acme".to_string()),
                        scope: None,
                    }),
                    updated_at: Some("2026-06-19T00:00:00Z".to_string()),
                },
            )]),
        };
<<<<<<< HEAD

=======
>>>>>>> e3fdf3ed (Merge 2 (#4))
        assert!(save_mcp_preferences_to(&path, &prefs).await.is_err());

        std::fs::remove_file(&path).unwrap();
        save_mcp_preferences_to(&path, &prefs).await.unwrap();
        let loaded = load_mcp_preferences_from(&path).file();
<<<<<<< HEAD
        let Some(acme) = loaded.servers.get("acme") else {
            panic!("expected acme server: {:?}", loaded.servers);
        };
        assert_eq!(acme.values.get("site").map(String::as_str), Some("us5"));
        assert_eq!(
            acme.source.as_ref().and_then(|s| s.plugin.as_deref()),
=======
        assert_eq!(loaded.servers["acme"].values["site"], "us5");
        assert_eq!(
            loaded.servers["acme"]
                .source
                .as_ref()
                .unwrap()
                .plugin
                .as_deref(),
>>>>>>> e3fdf3ed (Merge 2 (#4))
            Some("acme")
        );
    }

<<<<<<< HEAD
    #[tokio::test]
    async fn save_mcp_server_config_at_refuses_an_unparseable_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[models\ndefault = \"grok-4.6\"\n").unwrap();
        let config = McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "/bin/echo".to_string(),
                args: vec![],
                env: None,
                cwd: None,
            },
            enabled: true,
            oauth: None,
            setup: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            tool_timeouts: None,
            expose_image_base64: None,
        };

        let err = save_mcp_server_config_at(&path, "qa-echo", &config)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("unparseable"), "{err}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[models\ndefault = \"grok-4.6\"\n",
            "the file must be untouched"
        );
        let err = delete_mcp_server_config_at(&path, "qa-echo")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unparseable"), "{err}");
    }

    #[test]
    fn apply_mcp_server_enabled_updates_array_and_per_server_field() {
        let mut root: TomlValue = toml::from_str(
            r#"
[mcp_servers.local]
command = "npx"
enabled = true
"#,
        )
        .unwrap();
        let table = root.as_table_mut().unwrap();

        apply_mcp_server_enabled(table, "local", false);
        let disabled = table
            .get("disabled_mcp_servers")
            .and_then(|v| v.as_array())
            .expect("disabled_mcp_servers array");
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled.first().and_then(|v| v.as_str()), Some("local"));
        assert_eq!(
            table
                .get("mcp_servers")
                .and_then(|v| v.as_table())
                .and_then(|s| s.get("local"))
                .and_then(|v| v.as_table())
                .and_then(|s| s.get("enabled"))
                .and_then(|v| v.as_bool()),
            Some(false)
        );

        apply_mcp_server_enabled(table, "local", true);
        assert!(table.get("disabled_mcp_servers").is_none());
        assert_eq!(
            table
                .get("mcp_servers")
                .and_then(|v| v.as_table())
                .and_then(|s| s.get("local"))
                .and_then(|v| v.as_table())
                .and_then(|s| s.get("enabled"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn apply_mcp_server_enabled_managed_name_only_updates_array() {
        let mut root: TomlValue = toml::from_str("disabled_mcp_servers = []\n").unwrap();
        let table = root.as_table_mut().unwrap();

        apply_mcp_server_enabled(table, "grok_com_slack", false);
        let disabled = table
            .get("disabled_mcp_servers")
            .and_then(|v| v.as_array())
            .expect("disabled_mcp_servers array");
        assert_eq!(disabled.len(), 1);
        assert_eq!(
            disabled.first().and_then(|v| v.as_str()),
            Some("grok_com_slack")
        );
        assert!(table.get("mcp_servers").is_none());

        apply_mcp_server_enabled(table, "grok_com_slack", true);
        assert!(table.get("disabled_mcp_servers").is_none());
        assert!(table.get("mcp_servers").is_none());
    }

    #[tokio::test]
    async fn clear_sticky_project_disabled_at_only_flips_false() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[mcp_servers.proj]
command = "npx"
enabled = false
"#,
        )
        .unwrap();
        assert!(
            clear_sticky_project_disabled_at(&path, "proj")
                .await
                .unwrap()
        );
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("enabled = true"), "{body}");
        assert!(
            !clear_sticky_project_disabled_at(&path, "proj")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn enable_unstick_only_touches_nearest_project_definition() {
        let tmp = tempfile::tempdir().unwrap();
        git2::Repository::init(tmp.path()).unwrap();
        let nested = tmp.path().join("pkg");
        std::fs::create_dir_all(nested.join(".grok")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".grok")).unwrap();

        let sticky = r#"
# keep me
[mcp_servers.svc]
command = "npx"
enabled = false
"#;
        let ancestor = tmp.path().join(".grok").join("config.toml");
        let nearer = nested.join(".grok").join("config.toml");
        std::fs::write(&ancestor, sticky).unwrap();
        std::fs::write(&nearer, sticky).unwrap();

        assert_eq!(
            nearest_project_mcp_definition(&nested, "svc").as_ref(),
            Some(&nearer)
        );

        let path = nearest_project_mcp_definition(&nested, "svc").unwrap();
        clear_sticky_project_disabled_at(&path, "svc")
            .await
            .unwrap();

        let nearer_body = std::fs::read_to_string(&nearer).unwrap();
        assert!(
            nearer_body.contains("enabled = true"),
            "nearest should be unstuck: {nearer_body}"
        );
        assert!(
            nearer_body.contains("# keep me"),
            "toml_edit must preserve comments: {nearer_body}"
        );
        let ancestor_body = std::fs::read_to_string(&ancestor).unwrap();
        assert!(
            ancestor_body.contains("enabled = false"),
            "shadowed ancestor must stay sticky: {ancestor_body}"
        );

        set_sticky_project_disabled_at(&nearer, "svc")
            .await
            .unwrap();
        let re_stuck = std::fs::read_to_string(&nearer).unwrap();
        assert!(
            re_stuck.contains("enabled = false"),
            "re-stick must set enabled=false: {re_stuck}"
        );
        assert!(
            re_stuck.contains("# keep me"),
            "re-stick must preserve comments: {re_stuck}"
        );
    }

    #[tokio::test]
    async fn restore_mcp_server_enabled_after_enable_scopes_tiers() {
        let project = tempfile::tempdir().unwrap();
        let project_cfg = project.path().join("config.toml");
        std::fs::write(
            &project_cfg,
            r#"
# keep me
[mcp_servers.svc]
command = "true"
enabled = false
"#,
        )
        .unwrap();

        clear_sticky_project_disabled_at(&project_cfg, "svc")
            .await
            .unwrap();
        let unstuck = std::fs::read_to_string(&project_cfg).unwrap();
        assert!(unstuck.contains("enabled = true"), "{unstuck}");
        assert!(unstuck.contains("# keep me"), "{unstuck}");

        restore_mcp_server_enabled_after_enable("svc", std::slice::from_ref(&project_cfg))
            .await
            .unwrap();

        let project_body = std::fs::read_to_string(&project_cfg).unwrap();
        assert!(project_body.contains("enabled = false"), "{project_body}");
        assert!(project_body.contains("# keep me"), "{project_body}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn project_enable_replaces_config_symlink_not_referent() {
        let tmp = tempfile::tempdir().unwrap();
        git2::Repository::init(tmp.path()).unwrap();
        let grok = tmp.path().join(".grok");
        std::fs::create_dir_all(&grok).unwrap();
        let outside = tmp.path().join("outside.toml");
        std::fs::write(
            &outside,
            r#"
[mcp_servers.svc]
command = "true"
enabled = false
"#,
        )
        .unwrap();
        let project_cfg = grok.join("config.toml");
        std::os::unix::fs::symlink(&outside, &project_cfg).unwrap();

        assert!(
            clear_sticky_project_disabled_at(&project_cfg, "svc")
                .await
                .unwrap()
        );
        assert!(
            !std::fs::symlink_metadata(&project_cfg)
                .unwrap()
                .file_type()
                .is_symlink(),
            "project slot must become a regular file"
        );
        let body = std::fs::read_to_string(&project_cfg).unwrap();
        assert!(body.contains("enabled = true"), "{body}");
        assert!(
            std::fs::read_to_string(&outside)
                .unwrap()
                .contains("enabled = false"),
            "external referent must stay sticky-disabled"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    #[serial_test::serial]
    async fn persist_mcp_toml_follows_user_config_symlink() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let outside = home.path().join("dotfiles").join("config.toml");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        std::fs::write(&outside, "[mcp_servers.keep]\ncommand = \"true\"\n").unwrap();
        let slot = home.path().join("config.toml");
        std::os::unix::fs::symlink(&outside, &slot).unwrap();

        persist_mcp_toml(
            &config_path(),
            "[mcp_servers.keep]\ncommand = \"true\"\nenabled = false\n",
        )
        .await
        .unwrap();
        assert!(
            std::fs::symlink_metadata(&slot)
                .unwrap()
                .file_type()
                .is_symlink(),
            "user slot must stay a symlink"
        );
        assert_eq!(outside, std::fs::read_link(&slot).unwrap());
        assert!(
            std::fs::read_to_string(&outside)
                .unwrap()
                .contains("enabled = false")
        );
    }

    fn test_stdio_server() -> McpServerConfig {
        toml::from_str("command = \"true\"\n").expect("stdio fixture")
    }

    #[cfg(unix)]
    #[tokio::test]
    #[serial_test::serial]
    async fn save_mcp_server_config_at_refuses_unparseable() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let slot = home.path().join("config.toml");
        std::fs::write(&slot, "not = [valid\n").unwrap();
        let err = save_mcp_server_config_at(&config_path(), "svc", &test_stdio_server())
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unparseable"),
            "expected refuse, got {err}"
        );
        assert_eq!(
            std::fs::read_to_string(&slot).unwrap(),
            "not = [valid\n",
            "file must be left intact"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    #[serial_test::serial]
    async fn save_mcp_server_config_at_follows_user_symlink() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let outside = home.path().join("dotfiles").join("config.toml");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        std::fs::write(&outside, "[mcp_servers.keep]\ncommand = \"true\"\n").unwrap();
        let slot = home.path().join("config.toml");
        std::os::unix::fs::symlink(&outside, &slot).unwrap();

        save_mcp_server_config_at(&config_path(), "svc", &test_stdio_server())
            .await
            .unwrap();
        assert!(
            std::fs::symlink_metadata(&slot)
                .unwrap()
                .file_type()
                .is_symlink(),
            "user slot must stay a symlink"
        );
        let body = std::fs::read_to_string(&outside).unwrap();
        assert!(body.contains("[mcp_servers.svc]"), "{body}");
        assert!(body.contains("[mcp_servers.keep]"), "{body}");
    }

    #[cfg(unix)]
    #[tokio::test]
    #[serial_test::serial]
    async fn save_mcp_disabled_tools_follows_user_symlink() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let outside = home.path().join("dotfiles").join("config.toml");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        std::fs::write(&outside, "[mcp_servers.keep]\ncommand = \"true\"\n").unwrap();
        let slot = home.path().join("config.toml");
        std::os::unix::fs::symlink(&outside, &slot).unwrap();

        save_mcp_disabled_tools("keep", &["tool_a".to_string()])
            .await
            .unwrap();
        assert!(
            std::fs::symlink_metadata(&slot)
                .unwrap()
                .file_type()
                .is_symlink(),
            "user slot must stay a symlink"
        );
        let body = std::fs::read_to_string(&outside).unwrap();
        assert!(body.contains("[mcp_servers.keep]"), "{body}");
        assert!(body.contains("disabled_mcp_tools"), "{body}");
        assert!(body.contains("tool_a"), "{body}");
    }

    #[cfg(unix)]
    #[tokio::test]
    #[serial_test::serial]
    async fn save_mcp_disabled_tools_refuses_unparseable() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let slot = home.path().join("config.toml");
        std::fs::write(&slot, "not = [valid\n").unwrap();
        let err = save_mcp_disabled_tools("svc", &["tool_a".to_string()])
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unparseable"),
            "expected refuse, got {err}"
        );
        assert_eq!(std::fs::read_to_string(&slot).unwrap(), "not = [valid\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    #[serial_test::serial]
    async fn delete_mcp_server_config_at_follows_user_symlink() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let outside = home.path().join("dotfiles").join("config.toml");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        std::fs::write(
            &outside,
            "[mcp_servers.keep]\ncommand = \"true\"\n[mcp_servers.svc]\ncommand = \"true\"\n",
        )
        .unwrap();
        let slot = home.path().join("config.toml");
        std::os::unix::fs::symlink(&outside, &slot).unwrap();

        assert!(
            delete_mcp_server_config_at(&config_path(), "svc")
                .await
                .unwrap()
        );
        assert!(
            std::fs::symlink_metadata(&slot)
                .unwrap()
                .file_type()
                .is_symlink(),
            "user slot must stay a symlink"
        );
        let body = std::fs::read_to_string(&outside).unwrap();
        assert!(!body.contains("[mcp_servers.svc]"), "{body}");
        assert!(body.contains("[mcp_servers.keep]"), "{body}");
    }

    #[cfg(unix)]
    #[tokio::test]
    #[serial_test::serial]
    async fn delete_mcp_server_config_at_refuses_unparseable() {
        let home = tempfile::tempdir().unwrap();
        let _env = xai_grok_test_support::EnvGuard::set("GROK_HOME", home.path());
        let slot = home.path().join("config.toml");
        std::fs::write(&slot, "not = [valid\n").unwrap();
        let err = delete_mcp_server_config_at(&config_path(), "svc")
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unparseable"),
            "expected refuse, got {err}"
        );
        assert_eq!(std::fs::read_to_string(&slot).unwrap(), "not = [valid\n");
    }

    #[tokio::test]
    async fn write_toml_table_if_changed_refuses_unparseable() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("broken.toml");
        std::fs::write(&path, "not = [valid\n").unwrap();
        let err = write_toml_table_if_changed(&path, |_t| {})
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unparseable"),
            "expected refuse, got {err}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "not = [valid\n",
            "file must be left intact"
        );
    }
=======
    // === merge_section tests ===
>>>>>>> e3fdf3ed (Merge 2 (#4))
}
