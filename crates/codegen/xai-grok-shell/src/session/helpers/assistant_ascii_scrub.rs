//! Assistant AI-text ASCII scrub at shell choke points (Wave 0 S1–S3).
//!
//! Applies [`xai_grok_tools::util::ascii_scrub`] to **assistant message text**
//! only (stream chunks, chat_state assistant content, fallback one-shots).
//! Does **not** scrub user messages, tool arguments, or tool results.
//!
//! ## Enablement (default ON)
//!
//! 1. Env `GROK_SCRUB_ASCII_PUNCT` — ops kill-switch (see tools helper).
//! 2. Process config preference ([`set_config_enabled`] / [`config_enabled`])
//!    seeded from `[ui] scrub_ascii_punct` (default true). Either layer off
//!    disables scrub.
//! 3. **Session agent override** ([`session_override_disabled`]) — only set
//!    after user **approval** via [`apply_agent_scrub_disable_request`].
//!    Unapproved / rejected requests never disable scrub.
//!
//! ## Agent override (S3)
//!
//! Agents must not silently turn hygiene off. The only agent path is:
//!
//! 1. Agent calls the `disable_ascii_scrub` tool (or a future equivalent).
//! 2. Shell **always** surfaces `session/request_permission` with
//!    [`scrub_disable_permission_options`] (AllowOnce / AllowAlways / Reject*)
//!    — never YOLO / Read auto-allow for this tool.
//! 3. Outcome is mapped with [`approval_from_permission_option`] and applied via
//!    [`apply_agent_scrub_disable_request`] / product
//!    [`request_agent_scrub_disable`].
//!
//! | Decision | Effect |
//! |----------|--------|
//! | `None` (cancelled / no decision) | Scrub stays on |
//! | [`ScrubDisableApproval::Reject`] | Scrub stays on |
//! | [`ScrubDisableApproval::AllowOnce`] | Scrub off for this process/session |
//! | [`ScrubDisableApproval::AllowAlways`] | Session off + durable `[ui] scrub_ascii_punct = false` via settings write |
//!
//! User-driven durable off remains env / `[ui] scrub_ascii_punct` / settings —
//! that path does **not** need agent approval.

use std::sync::atomic::{AtomicBool, Ordering};

use agent_client_protocol as acp;
use xai_grok_tools::implementations::grok_build::DISABLE_ASCII_SCRUB_TOOL_NAME;
use xai_grok_tools::util::ascii_scrub;

/// Durable `[ui] scrub_ascii_punct` preference (default ON).
static CONFIG_ENABLED: AtomicBool = AtomicBool::new(true);

/// Session-scoped agent override: when true, scrub is forced **off** for this
/// process (set only after approved [`apply_agent_scrub_disable_request`]).
static SESSION_OVERRIDE_DISABLED: AtomicBool = AtomicBool::new(false);

/// User decision from the session permission UX for “disable ASCII scrub?”.
///
/// Mirrors ACP `PermissionOptionKind` AllowOnce / AllowAlways / Reject* without
/// depending on the ACP crate here (keeps unit tests leaf-simple).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrubDisableApproval {
    /// User rejected (or equivalent) — scrub stays on.
    Reject,
    /// AllowOnce — disable scrub for the remainder of this session/process.
    AllowOnce,
    /// AllowAlways — session disable **and** flip durable config preference off.
    AllowAlways,
}

/// Wire-stable option ids for a scrub-disable permission prompt.
///
/// Callers that build `session/request_permission` options should use these
/// ids so [`approval_from_permission_option`] can map the selection.
pub const OPTION_ID_ALLOW_ONCE: &str = "scrub-disable-allow-once";
pub const OPTION_ID_ALLOW_ALWAYS: &str = "scrub-disable-allow-always";
pub const OPTION_ID_REJECT: &str = "scrub-disable-reject";

/// One option offered on the scrub-disable permission prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrubDisablePermissionOption {
    pub option_id: &'static str,
    pub label: &'static str,
    pub approval: ScrubDisableApproval,
}

/// Options for the existing permission UX when the agent requests scrub off.
pub fn scrub_disable_permission_options() -> &'static [ScrubDisablePermissionOption] {
    &[
        ScrubDisablePermissionOption {
            option_id: OPTION_ID_ALLOW_ONCE,
            label: "Yes, disable ASCII scrub for this session",
            approval: ScrubDisableApproval::AllowOnce,
        },
        ScrubDisablePermissionOption {
            option_id: OPTION_ID_ALLOW_ALWAYS,
            label: "Yes, and remember (turn off in settings)",
            approval: ScrubDisableApproval::AllowAlways,
        },
        ScrubDisablePermissionOption {
            option_id: OPTION_ID_REJECT,
            label: "No, keep scrubbing fancy punctuation",
            approval: ScrubDisableApproval::Reject,
        },
    ]
}

/// Map a permission option id (and optional kind string) to an approval.
///
/// Accepts our stable ids and common ACP-style kind tokens (`allow_once`,
/// `allow_always`, `reject_once`, …). Unknown / empty → [`ScrubDisableApproval::Reject`]
/// (fail-closed: never disable without a clear allow).
pub fn approval_from_permission_option(
    option_id: &str,
    kind: Option<&str>,
) -> ScrubDisableApproval {
    let id = option_id.trim();
    for opt in scrub_disable_permission_options() {
        if opt.option_id == id {
            return opt.approval;
        }
    }
    // Generic ACP option ids used by other prompts — still map kinds.
    match id {
        "allow-once" | "allow_once" => return ScrubDisableApproval::AllowOnce,
        "always-allow" | "allow-always" | "allow_always" => {
            return ScrubDisableApproval::AllowAlways;
        }
        "reject-once" | "reject_once" | "reject-always" | "reject_always" | "reject" => {
            return ScrubDisableApproval::Reject;
        }
        _ => {}
    }
    if let Some(k) = kind {
        let k = k.trim().to_ascii_lowercase();
        return match k.as_str() {
            "allowonce" | "allow_once" | "allow-once" => ScrubDisableApproval::AllowOnce,
            "allowalways" | "allow_always" | "allow-always" => ScrubDisableApproval::AllowAlways,
            "rejectonce" | "reject_once" | "reject-once" | "rejectalways" | "reject_always"
            | "reject-always" | "reject" | "cancelled" | "cancel" => ScrubDisableApproval::Reject,
            _ => ScrubDisableApproval::Reject,
        };
    }
    ScrubDisableApproval::Reject
}

/// Update the config-layer preference (settings apply / session setup).
///
/// This is the **user / settings** path — not an agent silent override.
pub fn set_config_enabled(enabled: bool) {
    CONFIG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Current config-layer preference (tests / diagnostics).
pub fn config_enabled() -> bool {
    CONFIG_ENABLED.load(Ordering::Relaxed)
}

/// Whether a session agent override has forced scrub off.
pub fn session_override_disabled() -> bool {
    SESSION_OVERRIDE_DISABLED.load(Ordering::Relaxed)
}

/// Clear the session agent override (tests / new session seed).
pub fn clear_session_override() {
    SESSION_OVERRIDE_DISABLED.store(false, Ordering::Relaxed);
}

/// Whether scrub should run right now (env AND config AND no session override).
pub fn scrub_active() -> bool {
    if session_override_disabled() {
        return false;
    }
    ascii_scrub::scrub_enabled() && config_enabled()
}

/// Apply an agent request to disable scrub, gated on permission UX approval.
///
/// - `None` — cancelled / no user decision → scrub stays on; returns `false`
/// - `Some(Reject)` — scrub stays on; returns `false`
/// - `Some(AllowOnce)` — session override on; returns `true`
/// - `Some(AllowAlways)` — session override + process config pref off; returns `true`
///   (disk write is **not** done here — use
///   [`apply_agent_scrub_disable_request_with_persist`] / product path)
///
/// Returns whether scrub is **disabled** after this call due to an allow.
/// Never disables without an explicit allow decision.
pub fn apply_agent_scrub_disable_request(approval: Option<ScrubDisableApproval>) -> bool {
    match approval {
        None | Some(ScrubDisableApproval::Reject) => false,
        Some(ScrubDisableApproval::AllowOnce) => {
            SESSION_OVERRIDE_DISABLED.store(true, Ordering::Relaxed);
            true
        }
        Some(ScrubDisableApproval::AllowAlways) => {
            SESSION_OVERRIDE_DISABLED.store(true, Ordering::Relaxed);
            set_config_enabled(false);
            true
        }
    }
}

/// Apply approval; on [`ScrubDisableApproval::AllowAlways`] also run
/// `on_allow_always_persist` (product: settings write for
/// `[ui].scrub_ascii_punct = false`).
///
/// Fail-closed: no allow → no persist callback.
pub async fn apply_agent_scrub_disable_request_with_persist<F, Fut>(
    approval: Option<ScrubDisableApproval>,
    on_allow_always_persist: F,
) -> bool
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let always = matches!(approval, Some(ScrubDisableApproval::AllowAlways));
    let disabled = apply_agent_scrub_disable_request(approval);
    if disabled && always {
        on_allow_always_persist().await;
    }
    disabled
}

/// Product AllowAlways path: session apply + disk write via
/// [`crate::util::config::set_scrub_ascii_punct`](false).
pub async fn apply_agent_scrub_disable_request_product(
    approval: Option<ScrubDisableApproval>,
) -> bool {
    apply_agent_scrub_disable_request_with_persist(approval, || async {
        if let Err(e) = crate::util::config::set_scrub_ascii_punct(false).await {
            tracing::warn!(
                error = %e,
                "AllowAlways scrub disable: failed to persist [ui].scrub_ascii_punct=false"
            );
        }
    })
    .await
}

/// Whether `tool_name` is the agent scrub-disable tool (client-facing name).
pub fn is_disable_ascii_scrub_tool(tool_name: &str) -> bool {
    tool_name == DISABLE_ASCII_SCRUB_TOOL_NAME
        || tool_name.eq_ignore_ascii_case("DisableAsciiScrub")
        || tool_name == format!("GrokBuild:{DISABLE_ASCII_SCRUB_TOOL_NAME}")
}

/// Build ACP `PermissionOption`s for the scrub-disable prompt.
pub fn scrub_disable_acp_permission_options() -> Vec<acp::PermissionOption> {
    scrub_disable_permission_options()
        .iter()
        .map(|o| {
            let kind = match o.approval {
                ScrubDisableApproval::AllowOnce => acp::PermissionOptionKind::AllowOnce,
                ScrubDisableApproval::AllowAlways => acp::PermissionOptionKind::AllowAlways,
                ScrubDisableApproval::Reject => acp::PermissionOptionKind::RejectOnce,
            };
            acp::PermissionOption::new(o.option_id, o.label.to_owned(), kind)
        })
        .collect()
}

/// Map an ACP `session/request_permission` response to approval (fail-closed).
///
/// - `Cancelled` → `None` (scrub stays on)
/// - `Selected` → mapped via [`approval_from_permission_option`]
/// - unknown outcome → `Some(Reject)`
pub fn approval_from_permission_response(
    resp: &acp::RequestPermissionResponse,
) -> Option<ScrubDisableApproval> {
    match &resp.outcome {
        acp::RequestPermissionOutcome::Cancelled => None,
        acp::RequestPermissionOutcome::Selected(selected) => {
            let id = selected.option_id.0.as_ref();
            Some(approval_from_permission_option(id, None))
        }
        _ => Some(ScrubDisableApproval::Reject),
    }
}

/// Result of the product scrub-disable permission flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrubDisableFlowResult {
    /// Scrub disabled after an allow (session; durable when `always`).
    Disabled { always: bool },
    /// Cancelled / rejected / error — scrub remains on.
    KeptOn,
}

/// Product path: `session/request_permission` with scrub options → apply
/// (AllowAlways also persists via settings write).
///
/// Never disables without a clear allow. Does **not** honor YOLO auto-approve
/// (caller must use this instead of the normal permission manager for the
/// `disable_ascii_scrub` tool).
pub async fn request_agent_scrub_disable(
    gateway: &xai_acp_lib::AcpAgentGatewaySender,
    session_id: acp::SessionId,
    tool_call_id: impl Into<String>,
) -> ScrubDisableFlowResult {
    use agent_client_protocol::Client as _;

    let tool_call_id = tool_call_id.into();
    let options = scrub_disable_acp_permission_options();
    let tool_call_update = acp::ToolCallUpdate::new(
        acp::ToolCallId::new(std::sync::Arc::from(tool_call_id.as_str())),
        acp::ToolCallUpdateFields::new()
            .title("Disable ASCII-safe assistant punctuation?".to_owned())
            .kind(acp::ToolKind::Other),
    );
    let req = acp::RequestPermissionRequest::new(session_id, tool_call_update, options);

    let approval = match gateway.request_permission(req).await {
        Ok(resp) => approval_from_permission_response(&resp),
        Err(e) => {
            tracing::error!(?e, "scrub disable session/request_permission failed");
            None
        }
    };

    let always = matches!(approval, Some(ScrubDisableApproval::AllowAlways));
    if apply_agent_scrub_disable_request_product(approval).await {
        ScrubDisableFlowResult::Disabled { always }
    } else {
        ScrubDisableFlowResult::KeptOn
    }
}

/// Test / pure path: apply a pre-resolved permission option id (as if the
/// user selected it on the prompt). Used by unit tests and any caller that
/// already owns the `request_permission` round-trip.
pub async fn apply_scrub_disable_from_option_id(
    option_id: Option<&str>,
    kind: Option<&str>,
) -> ScrubDisableFlowResult {
    let approval = option_id.map(|id| approval_from_permission_option(id, kind));
    let always = matches!(approval, Some(ScrubDisableApproval::AllowAlways));
    // Unit-test path: no disk; process pref only. Product gateway path uses
    // `request_agent_scrub_disable` → `apply_agent_scrub_disable_request_product`.
    if apply_agent_scrub_disable_request(approval) {
        ScrubDisableFlowResult::Disabled { always }
    } else {
        ScrubDisableFlowResult::KeptOn
    }
}

/// Scrub assistant prose when enabled; otherwise return `text` unchanged.
pub fn scrub_assistant_text(text: String) -> String {
    if !scrub_active() {
        return text;
    }
    ascii_scrub::maybe_scrub_ascii_punct_owned(text, Some(true))
}

/// Scrub `AssistantItem.content` only; leave tool calls and other item kinds alone.
pub fn scrub_assistant_conversation_item(
    item: xai_grok_sampling_types::ConversationItem,
) -> xai_grok_sampling_types::ConversationItem {
    use xai_grok_sampling_types::ConversationItem;
    match item {
        ConversationItem::Assistant(mut a) if scrub_active() => {
            if ascii_scrub::needs_ascii_scrub(a.content.as_ref()) {
                let scrubbed = ascii_scrub::scrub_ascii_punct(a.content.as_ref());
                a.content = std::sync::Arc::<str>::from(scrubbed);
            }
            ConversationItem::Assistant(a)
        }
        other => other,
    }
}

/// Seed [`CONFIG_ENABLED`] from effective config (fail-open → ON).
/// Clears any leftover session agent override so a new session starts clean.
pub fn seed_from_effective_config() {
    clear_session_override();
    let enabled = match crate::config::load_effective_config() {
        Ok(root) => root
            .get("ui")
            .and_then(|u| u.get("scrub_ascii_punct"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        Err(_) => true,
    };
    set_config_enabled(enabled);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use xai_grok_tools::util::ascii_scrub::ENV_SCRUB_ASCII_PUNCT;

    /// Serialize env + config + session-override mutations for this module.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_and_reset() -> std::sync::MutexGuard<'static, ()> {
        let g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure clean defaults for each test.
        unsafe {
            std::env::remove_var(ENV_SCRUB_ASCII_PUNCT);
        }
        set_config_enabled(true);
        clear_session_override();
        g
    }

    #[test]
    fn default_on_scrubs_curly_quotes() {
        let _g = lock_and_reset();
        let out = scrub_assistant_text("say \u{201C}hi\u{201D}".into());
        assert_eq!(out, "say \"hi\"");
    }

    #[test]
    fn env_off_preserves_unicode() {
        let _g = lock_and_reset();
        unsafe {
            std::env::set_var(ENV_SCRUB_ASCII_PUNCT, "0");
        }
        let raw = "em\u{2014}dash and \u{2018}quotes\u{2019}";
        assert_eq!(scrub_assistant_text(raw.into()), raw);
        unsafe {
            std::env::remove_var(ENV_SCRUB_ASCII_PUNCT);
        }
    }

    #[test]
    fn config_off_preserves_unicode() {
        let _g = lock_and_reset();
        set_config_enabled(false);
        let raw = "em\u{2014}dash";
        assert_eq!(scrub_assistant_text(raw.into()), raw);
        set_config_enabled(true);
    }

    #[test]
    fn scrub_assistant_item_content_only() {
        let _g = lock_and_reset();
        let item = xai_grok_sampling_types::ConversationItem::assistant("it\u{2019}s fine");
        let scrubbed = scrub_assistant_conversation_item(item);
        match scrubbed {
            xai_grok_sampling_types::ConversationItem::Assistant(a) => {
                assert_eq!(a.content.as_ref(), "it's fine");
            }
            other => panic!("expected assistant, got {other:?}"),
        }
    }

    #[test]
    fn scrub_assistant_item_passthrough_when_off() {
        let _g = lock_and_reset();
        set_config_enabled(false);
        let item = xai_grok_sampling_types::ConversationItem::assistant("it\u{2019}s fine");
        let out = scrub_assistant_conversation_item(item);
        match out {
            xai_grok_sampling_types::ConversationItem::Assistant(a) => {
                assert_eq!(a.content.as_ref(), "it\u{2019}s fine");
            }
            other => panic!("expected assistant, got {other:?}"),
        }
        set_config_enabled(true);
    }

    // ── S3: agent override only with approval ─────────────────────────────

    #[test]
    fn unapproved_agent_request_does_not_disable_scrub() {
        let _g = lock_and_reset();
        assert!(scrub_active(), "precondition: scrub on");

        // No decision yet / cancelled.
        assert!(!apply_agent_scrub_disable_request(None));
        assert!(!session_override_disabled());
        assert!(scrub_active());
        assert_eq!(
            scrub_assistant_text("say \u{201C}hi\u{201D}".into()),
            "say \"hi\"",
            "unapproved request must still scrub"
        );
    }

    #[test]
    fn reject_keeps_scrub_on() {
        let _g = lock_and_reset();
        assert!(!apply_agent_scrub_disable_request(Some(
            ScrubDisableApproval::Reject
        )));
        assert!(!session_override_disabled());
        assert!(scrub_active());
        assert!(config_enabled());
        assert_eq!(scrub_assistant_text("em\u{2014}dash".into()), "em--dash");
    }

    #[test]
    fn allow_once_disables_scrub_for_session() {
        let _g = lock_and_reset();
        let raw = "em\u{2014}dash and \u{2018}quotes\u{2019}";

        assert!(apply_agent_scrub_disable_request(Some(
            ScrubDisableApproval::AllowOnce
        )));
        assert!(session_override_disabled());
        assert!(
            config_enabled(),
            "AllowOnce must not flip durable config preference"
        );
        assert!(!scrub_active());
        assert_eq!(
            scrub_assistant_text(raw.into()),
            raw,
            "AllowOnce must preserve fancy punctuation"
        );
    }

    #[test]
    fn allow_always_disables_session_and_config() {
        let _g = lock_and_reset();
        let raw = "it\u{2019}s";

        assert!(apply_agent_scrub_disable_request(Some(
            ScrubDisableApproval::AllowAlways
        )));
        assert!(session_override_disabled());
        assert!(
            !config_enabled(),
            "AllowAlways must flip durable config preference off"
        );
        assert!(!scrub_active());
        assert_eq!(scrub_assistant_text(raw.into()), raw);

        // Clearing only the session override still leaves config off.
        clear_session_override();
        assert!(!session_override_disabled());
        assert!(!config_enabled());
        assert!(!scrub_active());
    }

    #[test]
    fn clear_session_override_restores_scrub_when_config_on() {
        let _g = lock_and_reset();
        assert!(apply_agent_scrub_disable_request(Some(
            ScrubDisableApproval::AllowOnce
        )));
        assert!(!scrub_active());
        clear_session_override();
        assert!(scrub_active());
        assert_eq!(
            scrub_assistant_text("say \u{201C}hi\u{201D}".into()),
            "say \"hi\""
        );
    }

    #[test]
    fn approval_from_permission_option_maps_stable_ids() {
        assert_eq!(
            approval_from_permission_option(OPTION_ID_ALLOW_ONCE, None),
            ScrubDisableApproval::AllowOnce
        );
        assert_eq!(
            approval_from_permission_option(OPTION_ID_ALLOW_ALWAYS, None),
            ScrubDisableApproval::AllowAlways
        );
        assert_eq!(
            approval_from_permission_option(OPTION_ID_REJECT, None),
            ScrubDisableApproval::Reject
        );
        assert_eq!(
            approval_from_permission_option("unknown-id", None),
            ScrubDisableApproval::Reject
        );
        assert_eq!(
            approval_from_permission_option("allow-once", Some("AllowOnce")),
            ScrubDisableApproval::AllowOnce
        );
        assert_eq!(
            approval_from_permission_option("x", Some("reject_once")),
            ScrubDisableApproval::Reject
        );
    }

    #[test]
    fn permission_options_catalog_has_allow_and_reject() {
        let opts = scrub_disable_permission_options();
        assert!(
            opts.iter()
                .any(|o| o.approval == ScrubDisableApproval::AllowOnce)
        );
        assert!(
            opts.iter()
                .any(|o| o.approval == ScrubDisableApproval::AllowAlways)
        );
        assert!(
            opts.iter()
                .any(|o| o.approval == ScrubDisableApproval::Reject)
        );
    }

    #[test]
    fn end_to_end_unapproved_then_approved_via_option_id() {
        let _g = lock_and_reset();
        let raw = "\u{201C}quote\u{201D}";

        // Unapproved (unknown option / reject path).
        let rejected = approval_from_permission_option("nope", None);
        assert_eq!(rejected, ScrubDisableApproval::Reject);
        assert!(!apply_agent_scrub_disable_request(Some(rejected)));
        assert_eq!(scrub_assistant_text(raw.into()), "\"quote\"");

        // Approved AllowOnce via stable id.
        let allowed = approval_from_permission_option(OPTION_ID_ALLOW_ONCE, Some("AllowOnce"));
        assert!(apply_agent_scrub_disable_request(Some(allowed)));
        assert_eq!(
            scrub_assistant_text("\u{201C}quote\u{201D}".into()),
            "\u{201C}quote\u{201D}"
        );
    }

    #[test]
    fn is_disable_ascii_scrub_tool_matches_stable_names() {
        assert!(is_disable_ascii_scrub_tool(DISABLE_ASCII_SCRUB_TOOL_NAME));
        assert!(is_disable_ascii_scrub_tool("GrokBuild:disable_ascii_scrub"));
        assert!(is_disable_ascii_scrub_tool("DisableAsciiScrub"));
        assert!(!is_disable_ascii_scrub_tool("read_file"));
        assert!(!is_disable_ascii_scrub_tool("set_config"));
    }

    #[test]
    fn scrub_disable_acp_options_have_three_kinds() {
        let opts = scrub_disable_acp_permission_options();
        assert_eq!(opts.len(), 3);
        assert!(
            opts.iter()
                .any(|o| o.kind == acp::PermissionOptionKind::AllowOnce)
        );
        assert!(
            opts.iter()
                .any(|o| o.kind == acp::PermissionOptionKind::AllowAlways)
        );
        assert!(
            opts.iter()
                .any(|o| o.kind == acp::PermissionOptionKind::RejectOnce)
        );
    }

    #[test]
    fn approval_from_permission_response_cancelled_is_none() {
        let resp = acp::RequestPermissionResponse::new(acp::RequestPermissionOutcome::Cancelled);
        assert_eq!(approval_from_permission_response(&resp), None);
    }

    #[test]
    fn approval_from_permission_response_maps_selected_ids() {
        let allow = acp::RequestPermissionResponse::new(acp::RequestPermissionOutcome::Selected(
            acp::SelectedPermissionOutcome::new(acp::PermissionOptionId::new(OPTION_ID_ALLOW_ONCE)),
        ));
        assert_eq!(
            approval_from_permission_response(&allow),
            Some(ScrubDisableApproval::AllowOnce)
        );
        let reject = acp::RequestPermissionResponse::new(acp::RequestPermissionOutcome::Selected(
            acp::SelectedPermissionOutcome::new(acp::PermissionOptionId::new(OPTION_ID_REJECT)),
        ));
        assert_eq!(
            approval_from_permission_response(&reject),
            Some(ScrubDisableApproval::Reject)
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // ENV_LOCK serializes scrub state across awaits
    async fn request_without_approval_no_op_via_option_path() {
        let _g = lock_and_reset();
        assert!(scrub_active());
        let result = apply_scrub_disable_from_option_id(None, None).await;
        assert_eq!(result, ScrubDisableFlowResult::KeptOn);
        assert!(scrub_active());
        assert!(!session_override_disabled());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // ENV_LOCK serializes scrub state across awaits
    async fn reject_option_keeps_scrub_on_via_product_option_path() {
        let _g = lock_and_reset();
        let result =
            apply_scrub_disable_from_option_id(Some(OPTION_ID_REJECT), Some("RejectOnce")).await;
        assert_eq!(result, ScrubDisableFlowResult::KeptOn);
        assert!(scrub_active());
        assert!(config_enabled());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // ENV_LOCK serializes scrub state across awaits
    async fn allow_once_option_disables_session_only() {
        let _g = lock_and_reset();
        let raw = "em\u{2014}dash";
        let result =
            apply_scrub_disable_from_option_id(Some(OPTION_ID_ALLOW_ONCE), Some("AllowOnce")).await;
        assert_eq!(result, ScrubDisableFlowResult::Disabled { always: false });
        assert!(session_override_disabled());
        assert!(config_enabled(), "AllowOnce must not flip durable pref");
        assert!(!scrub_active());
        assert_eq!(scrub_assistant_text(raw.into()), raw);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // ENV_LOCK serializes scrub state across awaits
    async fn allow_always_with_persist_callback_runs_disk_hook() {
        let _g = lock_and_reset();
        let persisted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = persisted.clone();
        let disabled = apply_agent_scrub_disable_request_with_persist(
            Some(ScrubDisableApproval::AllowAlways),
            || {
                let flag = flag.clone();
                async move {
                    flag.store(true, Ordering::Relaxed);
                }
            },
        )
        .await;
        assert!(disabled);
        assert!(
            persisted.load(Ordering::Relaxed),
            "AllowAlways must invoke durable persist callback"
        );
        assert!(session_override_disabled());
        assert!(!config_enabled());
        assert!(!scrub_active());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // ENV_LOCK serializes scrub state across awaits
    async fn reject_does_not_invoke_persist_callback() {
        let _g = lock_and_reset();
        let persisted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = persisted.clone();
        let disabled = apply_agent_scrub_disable_request_with_persist(
            Some(ScrubDisableApproval::Reject),
            || {
                let flag = flag.clone();
                async move {
                    flag.store(true, Ordering::Relaxed);
                }
            },
        )
        .await;
        assert!(!disabled);
        assert!(
            !persisted.load(Ordering::Relaxed),
            "Reject must not write durable settings"
        );
        assert!(scrub_active());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // ENV_LOCK serializes scrub state across awaits
    async fn allow_once_does_not_invoke_persist_callback() {
        let _g = lock_and_reset();
        let persisted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = persisted.clone();
        let disabled = apply_agent_scrub_disable_request_with_persist(
            Some(ScrubDisableApproval::AllowOnce),
            || {
                let flag = flag.clone();
                async move {
                    flag.store(true, Ordering::Relaxed);
                }
            },
        )
        .await;
        assert!(disabled);
        assert!(
            !persisted.load(Ordering::Relaxed),
            "AllowOnce must not write durable settings"
        );
        assert!(config_enabled());
    }
}
