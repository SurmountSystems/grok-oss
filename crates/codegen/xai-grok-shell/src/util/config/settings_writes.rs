use super::persist::update_config;
use anyhow::Result;

// ---------------------------------------------------------------------------
// Settings helpers — typed disk-write wrappers for each setting.
// All route through `update_config` → `merge_section` → `save_config`.
// ---------------------------------------------------------------------------

/// Persist `[ui].compact_mode` via `update_config`.
pub async fn set_compact_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.compact_mode = value).await
}

/// Persist `[ui].show_timestamps` via `update_config`. `UiConfig::show_timestamps`
/// is `Option<bool>` — pager-side `None` means "use default" — so we wrap.
pub async fn set_show_timestamps(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_timestamps = Some(value)).await
}

/// Persist `[ui].show_timeline` via `update_config`. Same `Option<bool>`
/// shape as `show_timestamps`.
pub async fn set_show_timeline(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_timeline = Some(value)).await
}

pub async fn set_page_flip_on_send(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.page_flip_on_send = Some(value)).await
}

pub async fn set_confirm_before_rewind(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.confirm_before_rewind = Some(value)).await
}

/// Persist `[ui].combine_queued_prompts` via `update_config`.
pub async fn set_combine_queued_prompts(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.combine_queued_prompts = Some(value)).await
}

/// Persist `[ui].simple_mode` via `update_config`. Same `Option<bool>`
/// shape as `show_timestamps`.
pub async fn set_simple_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.simple_mode = Some(value)).await
}

/// Persist `[ui.contextual_hints].undo` via `update_config`. The nested struct
/// stays out of `config.toml` until a tip is toggled (`skip_serializing_if`).
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

/// Persist `[ui.contextual_hints].ssh_wrap` via `update_config`.
pub async fn set_contextual_hint_ssh_wrap(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.ssh_wrap = Some(value)).await
}

/// Persist `[ui].theme` via `update_config`. Caller must pass the
/// canonical theme name (`groknight`, `tokyonight`, `auto`, etc.).
pub async fn set_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.theme = Some(value)).await
}

/// Persist `[ui].auto_dark_theme` via `update_config`. `UiConfig::auto_dark_theme`
/// is `Option<String>` (canonical theme name; `auto` is rejected by the
/// pager's `load_auto_theme_config` filter at read time to prevent
/// circular reference).
pub async fn set_auto_dark_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.auto_dark_theme = Some(value)).await
}

/// Persist `[ui].auto_light_theme` via `update_config`. Same shape as
/// [`set_auto_dark_theme`].
pub async fn set_auto_light_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.auto_light_theme = Some(value)).await
}

/// Maximum length (in bytes) accepted by [`set_default_model`].
/// Defense against callers bypassing catalog validation.
pub const MAX_DEFAULT_MODEL_LEN: usize = 256;

/// Persist `[models].default` and dismiss any active campaign nudging it (an
/// explicit user pick wins over the soft campaign default).
///
/// This is the only sanctioned writer of `models.default`; it routes through
/// [`super::campaigns::persist_models_default`] so a user pick always dismisses
/// an active campaign. Do not persist `models.default` via raw `update_config`,
/// or a campaign would keep overriding the user's choice.
///
/// Caller must validate `value` against the model catalog first.
/// Empty string clears the field (falls back to remote/built-in default).
/// Length over [`MAX_DEFAULT_MODEL_LEN`] returns `Err`.
pub async fn set_default_model(value: String) -> Result<()> {
    super::campaigns::persist_models_default(
        if value.is_empty() { None } else { Some(value) },
        None,
    )
    .await
}

/// Persist `[privacy].privacy_banner_acked` (RFC 3339 UTC dismiss time).
pub async fn set_privacy_banner_acked(acked_at_rfc3339: String) -> Result<()> {
    update_config(|cfg| {
        cfg.privacy.privacy_banner_acked = Some(acked_at_rfc3339);
    })
    .await
}

/// Persist `[ui].fork_secondary_model` via `update_config`.
///
/// Caller must validate against the model catalog. Empty string
/// restores the built-in default. Length > [`MAX_DEFAULT_MODEL_LEN`] → `Err`.
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

/// Bounds for [`set_max_thoughts_width`]. Mirrored from the pager's
/// registry consts; a CI test pins the agreement.
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
/// Nested field only — does not replace the whole `display_refresh` object.
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
/// Clears the legacy `selection_highlight_duration_ms` and the retired
/// `double_click_action` keys it supersedes so the two can never drift (one-shot
/// disk migration away from the legacy key on any Settings write).
pub async fn set_keep_text_selection(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.keep_text_selection = Some(value);
        cfg.ui.selection_highlight_duration_ms = None;
        cfg.ui.double_click_action = None;
    })
    .await
}

/// Persist `[ui].render_mermaid` via `update_config`. Value is one of the
/// canonical strings `auto` | `on` | `off`.
pub async fn set_render_mermaid(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.render_mermaid = Some(value)).await
}

/// Persist `[ui].hunk_tracker_mode` via `update_config`. Value is one of the
/// canonical strings `agent_only` | `all_dirty` | `off`.
/// Restart-required: the mode is read once at connect time.
pub async fn set_hunk_tracker_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.hunk_tracker_mode = Some(value)).await
}

/// Persist `[ui].voice_capture_mode` via `update_config`. Value is one of the
/// canonical strings `toggle` | `hold`.
pub async fn set_voice_capture_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_capture_mode = Some(value)).await
}

/// Persist `[ui].voice_stt_language` via `update_config`. Value is a canonical
/// language code from the settings catalog (`en`, `es`, …) or `auto` (system
/// locale, falling back to English).
pub async fn set_voice_stt_language(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_stt_language = Some(value)).await
}

/// Persist `[ui].voice_keybind_enabled` via `update_config`. When `false` the
/// Ctrl+Space / F8 voice chord is ignored (`/voice` still works).
pub async fn set_voice_keybind_enabled(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_keybind_enabled = Some(value)).await
}

/// Persist `[ui].default_selected_permission` via `update_config`. Value is
/// one of the canonical strings from `DEFAULT_SELECTED_PERMISSION_CHOICES`
/// (`default` | `allow_once` | `allow_always` | `reject`); `default` is the
/// "no preselection" sentinel.
pub async fn set_default_selected_permission(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.default_selected_permission = Some(value)).await
}

/// Persist `[ui].cancel_subagents_on_turn_cancel` via `update_config`.
/// Canonical values: `ask` (clear / prompt each time), `always_stop`,
/// `always_continue`.
pub async fn set_cancel_subagents_on_turn_cancel(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.cancel_subagents_on_turn_cancel = if value == "ask" { None } else { Some(value) };
    })
    .await
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
/// Restart-required: sessions resolve the threshold once at build time.
/// Callers should pass a value in `0..=100` (the settings modal uses discrete
/// choices 85/90/95/98).
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
/// (still below env overrides). Restart-required for open sessions.
/// Grok 4.5 card presets: 200_000 (long-context price cliff) and 475_000
/// (95% of the 500k window).
pub async fn set_auto_compact_threshold_tokens(value: u64) -> Result<()> {
    update_config(|cfg| {
        cfg.session.auto_compact_threshold_tokens = Some(value);
        cfg.session.auto_compact_threshold_percent = None;
    })
    .await
}
