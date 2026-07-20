// `McpOAuthConfig` / `McpOAuthConfigMap` are re-exported via `mcp` (see `mcp.rs`)

mod announcements;
mod campaigns;

mod economic_mode;
mod hints;
mod load;
mod mcp;
mod mcp_reenable;
mod permissions;
mod persist;
mod resolve;
mod settings_writes;
mod tips;
mod worktree;

pub use announcements::*;
pub use campaigns::{
    CampaignModelsDefault, campaign_driven_models_default, persist_models_default,
    sync_campaign_fields,
};

pub use economic_mode::{
    ECONOMIC_CONTEXT_CAP, ECONOMIC_MODE_DEFAULT, apply_economic_context_cap,
    economic_mode_from_disk, economic_mode_from_toml, resolve_economic_mode,
};
pub use hints::*;
pub use load::*;
pub use mcp::*;
pub(crate) use mcp_reenable::{McpDefinitionIndex, needs_definition_scan};
pub use permissions::*;
pub use persist::*;
pub use resolve::*;
pub use settings_writes::*;
pub use tips::*;
pub use worktree::*;
pub use xai_grok_config::effective_config::{
    EffectiveConfigLayers, load_effective_config, load_effective_config_with_layers,
    remote_campaigns_from_settings, set_remote_campaigns_from_settings,
};
pub use xai_grok_config::load_effective_config_disk_only;
// These types live in `xai-grok-config`; the re-export keeps `crate::util::config::{RemoteSettings, GoalRoleModel}` working
pub use xai_grok_config_types::{
    CampaignOverride, ConsentGate, ContextualHintsRemote, DisplayRefreshSettings,
    DoomLoopRecoverySettings, GoalRoleModel, LongReasoningReminderSettings, RemoteSettings,
    WorktreeAutoGcSettings, WorktreeKindMaxAge, deserialize_tolerant,
};
