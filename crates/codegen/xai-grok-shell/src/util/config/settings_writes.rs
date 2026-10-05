use super::persist::update_config;
use crate::agent::config::Feature;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::UNIX_EPOCH;
use xai_grok_config::fs_atomic::BoundDest;

// --------------------------------------------------------------------------- Settings helpers: typed disk-write wrappers for each setting
// All route through `update_config`, then `merge_section`, then `save_config` ---------------------------------------------------------------------------

// Process-wide cache for `[ui].follow_up_behavior == "steer"`. The shell agent is a separate process from the pager, so an in-process atomic updated in the pager never reaches the turn loop
// Key the cache on config.toml mtime instead A live settings write invalidates on the next safe-point drain (cheap stat; full parse only when the file changed) 0 = unknown, 1 = queue, 2 = steer.
const FOLLOW_UP_CACHE_UNKNOWN: u8 = 0;
const FOLLOW_UP_CACHE_QUEUE: u8 = 1;
const FOLLOW_UP_CACHE_STEER: u8 = 2;
static FOLLOW_UP_STEER_CACHE: AtomicU8 = AtomicU8::new(FOLLOW_UP_CACHE_UNKNOWN);
static FOLLOW_UP_STEER_MTIME_NS: AtomicU64 = AtomicU64::new(0);

/// Nanoseconds since epoch for the user `config.toml` mtime, or 0 if missing.
fn follow_up_config_mtime_ns() -> u64 {
    let path = crate::util::grok_home::grok_home().join("config.toml");
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Update the hot-path Steer cache (same-process tests / after a local write).
pub fn set_follow_up_steer_cache(steer: bool) {
    FOLLOW_UP_STEER_CACHE.store(
        if steer {
            FOLLOW_UP_CACHE_STEER
        } else {
            FOLLOW_UP_CACHE_QUEUE
        },
        Ordering::Relaxed,
    );
    FOLLOW_UP_STEER_MTIME_NS.store(follow_up_config_mtime_ns(), Ordering::Relaxed);
}

/// Whether Steer is enabled in this process. Hits disk only when the cache is cold or the `config.toml` mtime has changed since the last resolve.
/// That lets the pager toggle Follow-up behavior live without restarting the shell agent. A failed effective-config load does not pin Queue: the previous cache is kept, or a cold failure returns false for this call only.
/// The cold failure writes neither QUEUE nor the mtime.
pub async fn follow_up_steer_enabled() -> bool {
    let mtime = follow_up_config_mtime_ns();
    let cached_mtime = FOLLOW_UP_STEER_MTIME_NS.load(Ordering::Relaxed);
    let cached = FOLLOW_UP_STEER_CACHE.load(Ordering::Relaxed);
    if cached != FOLLOW_UP_CACHE_UNKNOWN && mtime != 0 && mtime == cached_mtime {
        return cached == FOLLOW_UP_CACHE_STEER;
    }
    let root = match crate::config::load_effective_config() {
        Ok(root) => root,
        Err(_) => {
            // Transient load failure: do not cache Queue against this mtime.
            if cached != FOLLOW_UP_CACHE_UNKNOWN {
                return cached == FOLLOW_UP_CACHE_STEER;
            }
            return false;
        }
    };
    let enabled = super::load::load_config_from_toml(&root)
        .ui
        .follow_up_steer_enabled();
    FOLLOW_UP_STEER_CACHE.store(
        if enabled {
            FOLLOW_UP_CACHE_STEER
        } else {
            FOLLOW_UP_CACHE_QUEUE
        },
        Ordering::Relaxed,
    );
    FOLLOW_UP_STEER_MTIME_NS.store(mtime, Ordering::Relaxed);
    enabled
}

/// Persist `[ui].compact_mode` via `update_config`.
pub async fn set_compact_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.compact_mode = value).await
}

/// Persist `[ui].show_timestamps` via `update_config`.
/// `UiConfig::show_timestamps` is `Option<bool>` (pager-side `None` means "use default"), so we wrap.
pub async fn set_show_timestamps(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_timestamps = Some(value)).await
}

/// Persist `[ui].show_timeline` via `update_config`.
/// The `Option<bool>` shape matches `show_timestamps`.
pub async fn set_show_timeline(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_timeline = Some(value)).await
}

pub async fn set_page_flip_on_send(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.page_flip_on_send = Some(value)).await
}

pub async fn set_confirm_before_rewind(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.confirm_before_rewind = Some(value)).await
}

/// Persist `[ui].turbo_planning` and update the live cache in this process.
pub async fn set_turbo_planning(value: bool) -> Result<()> {
    super::set_turbo_planning_live(value);
    update_config(|cfg| cfg.ui.turbo_planning = Some(value)).await
}

/// Persist `[ui].process_rule_reminders_enabled`.
pub async fn set_process_rule_reminders_enabled(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.process_rule_reminders_enabled = Some(value)).await
}

/// Persist `[ui].process_rule_reminders`.
pub async fn set_process_rule_reminders(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.process_rule_reminders = Some(value)).await
}

pub async fn set_dashboard_preview(value: bool) -> Result<()> {
    rewrite_user_config_locked("dashboard preview", move |path| {
        write_dashboard_preview(
            path,
            value,
            crate::util::config::persist::atomic_write_follow_bound,
        )
    })
    .await
}

pub(super) fn write_dashboard_preview(
    path: &Path,
    value: bool,
    write: impl FnOnce(&Path, &BoundDest, &str) -> std::io::Result<()>,
) -> Result<()> {
    rewrite_user_config_table(path, "dashboard preview", write, |root| {
        let ui = root
            .entry("ui".to_owned())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .with_context(|| format!("{} [ui] must be a table", path.display()))?;
        ui.insert("dashboard_preview".to_owned(), toml::Value::Boolean(value));
        Ok(())
    })
}

/// Run a raw-table rewrite of the user `config.toml` under the config write guard.
/// For keys `update_config` cannot express (a deletion, or a table `save_config_locked` does not merge).
async fn rewrite_user_config_locked(
    what: &'static str,
    rewrite: impl FnOnce(&Path) -> Result<()> + Send + 'static,
) -> Result<()> {
    let guard = crate::util::config::persist::lock_config_writes()
        .await
        .map_err(|error| anyhow::anyhow!("lock config.toml to save {what}: {error}"))?;
    guard
        .run_blocking(move || rewrite(&crate::util::config::mcp::user_config_path()))
        .await
        .map_err(|error| anyhow::anyhow!("{what} persistence task failed: {error}"))?
}

/// Read `path`, hand its root table to `edit`, and publish the result with `write`; `what` names the setting in every error.
fn rewrite_user_config_table(
    path: &Path,
    what: &str,
    write: impl FnOnce(&Path, &BoundDest, &str) -> std::io::Result<()>,
    edit: impl FnOnce(&mut toml::map::Map<String, toml::Value>) -> Result<()>,
) -> Result<()> {
    let (destination, content) = crate::util::config::persist::read_follow_bound(path)
        .map_err(|error| anyhow::anyhow!("read {} to save {what}: {error}", path.display()))?;
    let mut document =
        crate::util::config::persist::parse_existing_config_toml(&content).map_err(|error| {
            anyhow::anyhow!(
                "parse {} to save {what}: {}",
                path.display(),
                xai_grok_config::toml_error_detail(&content, &error)
            )
        })?;
    let root = document
        .as_table_mut()
        .with_context(|| format!("{} must contain a TOML table", path.display()))?;
    edit(root)?;
    let content = toml::to_string_pretty(&document)
        .map_err(|error| anyhow::anyhow!("serialize {} for {what}: {error}", path.display()))?;
    write(path, &destination, &content)
        .map_err(|error| anyhow::anyhow!("write {} to save {what}: {error}", path.display()))
}

/// Persist `[ui].ulid_session_ids` via `update_config`.
pub async fn set_ulid_session_ids(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.ulid_session_ids = Some(value)).await
}

/// Persist `[ui].combine_queued_prompts` via `update_config`.
pub async fn set_combine_queued_prompts(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.combine_queued_prompts = Some(value)).await
}

/// Persist `[ui].follow_up_behavior` (`"queue"` | `"steer"`).
pub async fn set_follow_up_behavior(value: String) -> Result<()> {
    // Keep the hot-path cache in sync before the disk write returns.
    set_follow_up_steer_cache(value == "steer");
    update_config(|cfg| cfg.ui.follow_up_behavior = Some(value)).await
}

/// Persist `[ui].simple_mode` via `update_config`.
/// The `Option<bool>` shape matches `show_timestamps`.
pub async fn set_simple_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.simple_mode = Some(value)).await
}

/// Persist `[ui.contextual_hints].undo` via `update_config`.
/// The nested struct stays out of `config.toml` until a tip is toggled (`skip_serializing_if`).
pub async fn set_contextual_hint_undo(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.undo = Some(value)).await
}

/// Persist `[ui.contextual_hints].plan_mode` via `update_config`.
pub async fn set_contextual_hint_plan_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.plan_mode = Some(value)).await
}

/// Persist `[ui.contextual_hints].image_input` via `update_config`.
pub async fn set_contextual_hint_image_input(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.image_input = Some(value)).await
}

/// Persist `[ui.contextual_hints].send_now` via `update_config`.
pub async fn set_contextual_hint_send_now(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.send_now = Some(value)).await
}

/// Persist `[ui.contextual_hints].small_screen` via `update_config`.
pub async fn set_contextual_hint_small_screen(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.small_screen = Some(value)).await
}

/// Persist `[ui.contextual_hints].word_select` via `update_config`.
pub async fn set_contextual_hint_word_select(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.word_select = Some(value)).await
}

/// Persist `[ui.contextual_hints].export_copy` via `update_config`.
pub async fn set_contextual_hint_export_copy(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.export_copy = Some(value)).await
}

/// Persist `[ui.contextual_hints].ssh_wrap` via `update_config`.
pub async fn set_contextual_hint_ssh_wrap(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.ssh_wrap = Some(value)).await
}

/// Persist `[ui].theme` via `update_config`.
/// Caller must pass the canonical theme name (`groknight`, `tokyonight`, `auto`, etc.).
pub async fn set_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.theme = Some(value)).await
}

/// Persist `[ui].auto_dark_theme` via `update_config`.
/// `UiConfig::auto_dark_theme` is `Option<String>` holding a canonical theme name.
/// The pager's `load_auto_theme_config` filter rejects `auto` at read time to prevent a circular reference.
pub async fn set_auto_dark_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.auto_dark_theme = Some(value)).await
}

/// Persist `[ui].auto_light_theme` via `update_config`.
/// The shape matches [`set_auto_dark_theme`].
pub async fn set_auto_light_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.auto_light_theme = Some(value)).await
}

/// Maximum length (in bytes) accepted by [`set_default_model`].
/// It defends against callers bypassing catalog validation.
pub const MAX_DEFAULT_MODEL_LEN: usize = 256;

/// Persist `[models].default`. This is the only sanctioned writer of `models.default`. It routes through [`super::campaigns::persist_models_default`] so a user pick always dismisses an active campaign.
/// Do not persist `models.default` via raw `update_config`, or a campaign would keep overriding the user's choice. Caller must validate `value` against the model catalog first.
/// Empty string clears the field (falls back to remote/built-in default). Length over [`MAX_DEFAULT_MODEL_LEN`] returns `Err`.
pub async fn set_default_model(value: String) -> Result<()> {
    super::campaigns::persist_models_default(
        if value.is_empty() { None } else { Some(value) },
        None,
    )
    .await
}

/// Persist `[models].default_reasoning_effort` via `update_config`.
///
/// Settings row choices are `low` / `medium` / `high` (Grok 4.6 baked default
/// is medium). Other `ReasoningEffort` values are rejected here so the modal
/// cannot write a catalog-only effort as the environment default.
pub async fn set_default_reasoning_effort(value: String) -> Result<()> {
    use xai_grok_sampling_types::ReasoningEffort;
    let effort: ReasoningEffort = value
        .parse()
        .map_err(|e| anyhow::anyhow!("default_reasoning_effort: {e}"))?;
    match effort {
        ReasoningEffort::Low | ReasoningEffort::Medium | ReasoningEffort::High => {}
        other => anyhow::bail!(
            "default_reasoning_effort {other} is not a Settings row choice (low, medium, high)"
        ),
    }
    update_config(|cfg| cfg.models.default_reasoning_effort = Some(effort)).await
}

/// Persist `[privacy].privacy_banner_acked` (RFC 3339 UTC dismiss time).
pub async fn set_privacy_banner_acked(acked_at_rfc3339: String) -> Result<()> {
    update_config(|cfg| {
        cfg.privacy.privacy_banner_acked = Some(acked_at_rfc3339);
    })
    .await
}

/// Persist `[telemetry].trace_upload`.
pub async fn set_trace_upload(value: bool) -> Result<()> {
    update_config(|cfg| {
        cfg.telemetry.trace_upload = Some(value);
    })
    .await
}

/// Persist `[features].feedback_trace_card`.
pub async fn set_feedback_trace_card(value: bool) -> Result<()> {
    update_config(|cfg| {
        cfg.features.feedback_trace_card = Some(value);
    })
    .await
}

/// Persist or remove a registry `[features]` key in the user `config.toml`.
/// `None` deletes the key so the remote and default tiers apply again.
pub async fn set_feature_override(feature: Feature, value: Option<bool>) -> Result<()> {
    rewrite_user_config_locked(feature.path(), move |path| {
        write_feature_override(
            path,
            feature,
            value,
            crate::util::config::persist::atomic_write_follow_bound,
        )
    })
    .await
}

pub(super) fn write_feature_override(
    path: &Path,
    feature: Feature,
    value: Option<bool>,
    write: impl FnOnce(&Path, &BoundDest, &str) -> std::io::Result<()>,
) -> Result<()> {
    rewrite_user_config_table(path, feature.path(), write, |root| {
        match value {
            Some(flag) => {
                let features = root
                    .entry("features".to_owned())
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
                    .as_table_mut()
                    .with_context(|| format!("{} [features] must be a table", path.display()))?;
                features.insert(feature.key().to_owned(), toml::Value::Boolean(flag));
            }
            None => {
                let table = root.get_mut("features").and_then(toml::Value::as_table_mut);
                if let Some(features) = table {
                    features.remove(feature.key());
                    if features.is_empty() {
                        root.remove("features");
                    }
                }
            }
        }
        Ok(())
    })
}

/// Persist `[ui].fork_secondary_model` via `update_config`. Caller must validate against the model catalog. Empty string restores the built-in default. A length over [`MAX_DEFAULT_MODEL_LEN`] returns `Err`.
pub async fn set_fork_secondary_model(value: String) -> Result<()> {
    if value.len() > MAX_DEFAULT_MODEL_LEN {
        anyhow::bail!(
            "fork_secondary_model name too long ({} > {} bytes)",
            value.len(),
            MAX_DEFAULT_MODEL_LEN
        );
    }
    update_config(|cfg| {
        cfg.ui.fork_secondary_model = if value.is_empty() {
            crate::models::default_model().to_string()
        } else {
            value
        };
    })
    .await
}

/// Bounds for [`set_max_thoughts_width`].
/// They mirror the pager's registry consts; a CI test pins the agreement.
const MAX_THOUGHTS_WIDTH_SHELL_MIN: i64 = 40;
const MAX_THOUGHTS_WIDTH_SHELL_MAX: i64 = 500;

/// Persist `[ui].max_thoughts_width` via `update_config`.
/// Defensively clamps to `[40, 500]` at the shell boundary.
pub async fn set_max_thoughts_width(value: i64) -> Result<()> {
    let clamped = value.clamp(MAX_THOUGHTS_WIDTH_SHELL_MIN, MAX_THOUGHTS_WIDTH_SHELL_MAX) as u16;
    update_config(|cfg| cfg.ui.max_thoughts_width = clamped).await
}

/// Persist `[ui].scroll_speed` via `update_config`.
/// Defensively clamps to `[1, 100]` at the shell boundary.
pub async fn set_scroll_speed(value: i64) -> Result<()> {
    let clamped = value.clamp(1, 100) as u8;
    update_config(|cfg| cfg.ui.scroll_speed = Some(clamped)).await
}

/// Persist `[ui].scroll_mode` (`auto` | `wheel` | `trackpad`) via `update_config`.
pub async fn set_scroll_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.scroll_mode = Some(value)).await
}

/// Persist `[ui].invert_scroll` via `update_config`.
pub async fn set_invert_scroll(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.invert_scroll = Some(value)).await
}

/// Persist `[ui.display_refresh].auto_cadence_enabled` via `update_config`.
/// It writes only the nested field and does not replace the whole `display_refresh` object.
pub async fn set_display_refresh_auto_cadence(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.display_refresh.auto_cadence_enabled = Some(value)).await
}

/// Persist `[ui].scroll_lines` via `update_config`.
/// Defensively clamps to `[1, 10]` at the shell boundary.
pub async fn set_scroll_lines(value: i64) -> Result<()> {
    let clamped = value.clamp(1, 10) as u8;
    update_config(|cfg| cfg.ui.scroll_lines = Some(clamped)).await
}

/// Persist `[ui].vim_mode` via `update_config`.
pub async fn set_vim_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.vim_mode = Some(value)).await
}

/// Persist `[ui].remember_tool_approvals` via `update_config`.
pub async fn set_remember_tool_approvals(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.remember_tool_approvals = Some(value)).await
}

/// Persist `[ui].show_thinking_blocks` via `update_config`.
pub async fn set_show_thinking_blocks(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_thinking_blocks = Some(value)).await
}

/// Persist `[ui].always_expand_thinking` via `update_config`.
pub async fn set_always_expand_thinking(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.always_expand_thinking = Some(value)).await
}

/// Persist `[ui].hide_header` via `update_config`.
pub async fn set_hide_header(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.hide_header = value).await
}

/// Persist `[ui].composer_multiline` via `update_config`.
pub async fn set_composer_multiline(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.composer_multiline = Some(value)).await
}

/// Persist `[ui].allow_session_multiline` via `update_config`.
pub async fn set_allow_session_multiline(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.allow_session_multiline = Some(value)).await
}

/// Persist `[ui].scrub_ascii_punct` via `update_config`.
pub async fn set_scrub_ascii_punct(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.scrub_ascii_punct = Some(value)).await
}

/// Persist `[ui].plan_approval_park` (`soft` | `modal`) via `update_config`.
pub async fn set_plan_approval_park(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.plan_approval_park = Some(value)).await
}

/// Persist `[subagents].allow_worktree` without merging the rest of the table.
pub async fn set_allow_worktree(value: bool) -> Result<()> {
    super::persist::update_subagents_allow_worktree(value).await
}

/// Persist `[ui].prompt_suggestions` via `update_config`.
pub async fn set_prompt_suggestions(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.prompt_suggestions = Some(value)).await
}

/// Persist `[ui].auto_run_implement` via `update_config`.
pub async fn set_auto_run_implement(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.auto_run_implement = Some(value)).await
}

/// Persist `[ui].economic_mode` via `update_config`.
///
/// Soft-caps effective context at [`super::ECONOMIC_CONTEXT_CAP`] for Grok 4.5
/// pricing. Default ON when unset. Active sessions use `/economic-mode` for an
/// immediate override; this write seeds new sessions and the global default.
pub async fn set_economic_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.economic_mode = Some(value)).await
}

/// Persist `[ui].resume_canceled_turn_on_restart` via `update_config`.
///
/// Default ON when unset: re-queue an explicitly canceled turn once when the
/// same session is opened again.
pub async fn set_resume_canceled_turn_on_restart(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.resume_canceled_turn_on_restart = Some(value)).await
}

/// Persist one boolean under `[token_economy]` without splatting unrelated keys.
pub async fn set_token_economy_bool(field: &str, value: bool) -> Result<()> {
    use toml::Value as TomlValue;
    update_token_economy_key(field, TomlValue::Boolean(value)).await
}

/// Persist one integer under `[token_economy]` (effort knobs 0–5; 0 lock = unlocked).
pub async fn set_token_economy_int(field: &str, value: i64) -> Result<()> {
    use toml::Value as TomlValue;
    update_token_economy_key(field, TomlValue::Integer(value)).await
}

async fn update_token_economy_key(field: &str, value: toml::Value) -> Result<()> {
    use super::mcp::user_config_path;
    use super::persist::lock_config_writes;
    use toml::Value as TomlValue;
    use toml::map::Map as TomlMap;
    let _guard = lock_config_writes().await;
    let path = user_config_path();
    let mut root: TomlValue = match tokio::fs::read_to_string(&path).await {
        Ok(s) => match toml::from_str::<TomlValue>(&s) {
            Ok(v) => v,
            Err(parse_err) => {
                return Err(anyhow::anyhow!(
                    "refusing to overwrite unparseable {}: {}; save a backup \
                         and fix the syntax error before retrying",
                    path.display(),
                    parse_err,
                ));
            }
        },
        Err(_) => TomlValue::Table(TomlMap::new()),
    };
    if !matches!(root, TomlValue::Table(_)) {
        root = TomlValue::Table(TomlMap::new());
    }
    let table = root.as_table_mut().expect("root must be a table");
    let te = table
        .entry("token_economy".to_string())
        .or_insert_with(|| TomlValue::Table(TomlMap::new()));
    if let TomlValue::Table(te_tbl) = te {
        te_tbl.insert(field.to_string(), value);
    } else {
        let mut te_tbl = TomlMap::new();
        te_tbl.insert(field.to_string(), value);
        *te = TomlValue::Table(te_tbl);
    }
    // Validate the resulting table so Settings cannot write an invalid policy.
    let validated =
        crate::token_economy::token_economy_from_toml(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    let toml_str = toml::to_string_pretty(&root)?;
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    #[cfg(unix)]
    let prior_mode: Option<u32> = match tokio::fs::metadata(&path).await {
        Ok(m) => {
            use std::os::unix::fs::PermissionsExt;
            Some(m.permissions().mode())
        }
        Err(_) => None,
    };
    #[cfg(not(unix))]
    let prior_mode: Option<u32> = None;
    let suffix = {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("toml.tmp.{}.{}", std::process::id(), nanos)
    };
    let tmp = path.with_extension(suffix);
    tokio::fs::write(&tmp, toml_str).await?;
    #[cfg(unix)]
    if let Some(mode) = prior_mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode)).await;
    }
    let _ = prior_mode;
    tokio::fs::rename(&tmp, &path).await?;
    // Keep process live cache aligned with what we wrote (CLI / effect path).
    crate::token_economy::set_token_economy_live(validated);
    Ok(())
}

/// Persist `[toolset.ask_user_question].timeout_enabled` via `update_config`
/// (the user tier of the shell's tiered resolver; the effective value is
/// re-resolved at agent build).
pub async fn set_ask_user_question_timeout_enabled(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ask_user_question.timeout_enabled = Some(value)).await
}

/// Persist `[ui].group_tool_verbs` via `update_config`.
pub async fn set_group_tool_verbs(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.group_tool_verbs = Some(value)).await
}

/// Persist `[ui].collapsed_edit_blocks` via `update_config`.
pub async fn set_collapsed_edit_blocks(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.collapsed_edit_blocks = Some(value)).await
}

/// Persist `[ui].keep_text_selection` (`flash` | `hold` | `word_select`).
/// Clears the legacy `selection_highlight_duration_ms` and the retired `double_click_action` keys it supersedes so the two can never drift.
/// This makes any Settings write a one-shot disk migration away from the legacy keys.
pub async fn set_keep_text_selection(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.keep_text_selection = Some(value);
        cfg.ui.selection_highlight_duration_ms = None;
        cfg.ui.double_click_action = None;
    })
    .await
}

/// Persist `[ui].render_mermaid` via `update_config`.
/// Value is one of the canonical strings `auto` | `on` | `off`.
pub async fn set_render_mermaid(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.render_mermaid = Some(value)).await
}

/// Persist `[ui].hunk_tracker_mode` via `update_config`.
/// Value is one of the canonical strings `agent_only` | `all_dirty` | `off`.
/// Restart-required: the mode is read once at connect time.
pub async fn set_hunk_tracker_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.hunk_tracker_mode = Some(value)).await
}

/// Persist `[ui].voice_capture_mode` via `update_config`.
/// Value is one of the canonical strings `toggle` | `hold`.
pub async fn set_voice_capture_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_capture_mode = Some(value)).await
}

/// Persist `[ui].voice_stt_language` via `update_config`.
/// Value is a canonical language code from the settings catalog (`en`, `es`, …) or `auto` (system locale, falling back to English).
pub async fn set_voice_stt_language(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_stt_language = Some(value)).await
}

/// Persist `[ui].voice_keybind_enabled` via `update_config`.
/// When `false` the Ctrl+Space / F8 voice chord is ignored (`/voice` still works).
pub async fn set_voice_keybind_enabled(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_keybind_enabled = Some(value)).await
}

/// Persist `[ui].default_selected_permission` via `update_config`.
/// Value is one of the canonical strings from `DEFAULT_SELECTED_PERMISSION_CHOICES` (`default` | `allow_once` | `allow_always` | `reject`).
/// `default` is the "no preselection" sentinel.
pub async fn set_default_selected_permission(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.default_selected_permission = Some(value)).await
}

/// Persist `[ui].cancel_subagents_on_turn_cancel` via `update_config`.
/// Canonical values: `ask` (clear / prompt each time), `always_stop`, `always_continue`.
pub async fn set_cancel_subagents_on_turn_cancel(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.cancel_subagents_on_turn_cancel = if value == "ask" { None } else { Some(value) };
    })
    .await
}

/// Persist `[ui.notifications].session_recap` (auto return-from-away recap).
/// Does not gate manual `/recap` (that is `[features] session_recap`).
pub async fn set_notifications_session_recap(value: bool) -> Result<()> {
    super::persist::update_ui_notifications_session_recap(value).await
}

/// Persist `[ui.notifications].session_recap_threshold_secs` (debounce).
/// Clamped to a sane range at the shell boundary.
pub async fn set_notifications_session_recap_threshold_secs(value: i64) -> Result<()> {
    let clamped = value.clamp(5, 3600) as u64;
    super::persist::update_ui_notifications_session_recap_threshold_secs(clamped).await
}

/// Persist `[features].session_recap` (master kill for `/recap` + auto).
///
/// Only this key is written under `[features]` so other feature flags are
/// not splatted from `Features` defaults. Restart / new session so the shell
/// re-advertises `sessionRecap` on ACP initialize.
pub async fn set_features_session_recap(value: bool) -> Result<()> {
    super::persist::update_features_session_recap(value).await
}

/// Persist `[ui].screen_mode` (`fullscreen` | `minimal`). Empty clears the key.
pub async fn set_screen_mode(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.screen_mode = if value.is_empty() { None } else { Some(value) };
    })
    .await
}

/// Persist `[cli].show_tips` via `update_config`.
/// Restart-required: `resolve_tips` reads this once at startup.
pub async fn set_show_tips(value: bool) -> Result<()> {
    update_config(|cfg| cfg.cli.show_tips = Some(value)).await
}

/// Persist `[cli].auto_update` via `update_config`.
/// Restart-required: auto-update check fires once on startup.
pub async fn set_auto_update(value: bool) -> Result<()> {
    update_config(|cfg| cfg.cli.auto_update = Some(value)).await
}

/// Persist `[session].auto_compact_threshold_percent` via `update_config`.
///
/// Clears any absolute-token preference so percent mode is active.
/// After a successful disk write, the pager live-applies open sessions via ACP
/// `x.ai/auto_compact_threshold_changed` (no restart). Callers should pass a
/// value in `0..=100` (settings modal: 85/90/95/98).
pub async fn set_auto_compact_threshold_percent(value: u8) -> Result<()> {
    update_config(|cfg| {
        cfg.session.auto_compact_threshold_percent = Some(value);
        cfg.session.auto_compact_threshold_tokens = None;
    })
    .await
}

/// Persist `[session].auto_compact_threshold_tokens` via `update_config`.
///
/// Clears the session percent field so absolute-token mode wins the resolver
/// (still below env overrides on full resolve). Open sessions pick up the
/// committed tokens value live after successful persist (same ACP path as
/// percent mode). Grok 4.5 card presets: 200_000 (long-context price cliff)
/// and 475_000 (95% of the 500k window).
pub async fn set_auto_compact_threshold_tokens(value: u64) -> Result<()> {
    update_config(|cfg| {
        cfg.session.auto_compact_threshold_tokens = Some(value);
        cfg.session.auto_compact_threshold_percent = None;
    })
    .await
}
