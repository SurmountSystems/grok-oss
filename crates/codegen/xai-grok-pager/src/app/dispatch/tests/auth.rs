//! Tests for login, logout, account switching, and auth-code dispatchers.

use super::*;

#[test]
fn cta_mcps_loaded_needs_auth_opens_modal_and_seeds() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::extensions_modal::{ExtensionsTab, TabDataState};
    use crate::views::mcps_modal::{McpSectionId, McpServerDisplayStatus, section_key};
    let mut app = test_app_with_agent();
    app.team_id = Some("team-uuid".into());
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().plugin_cta.phase = CtaPhase::AwaitingMcps {
        name: "figma".into(),
    };
    let servers = vec![
        cta_mcp_server("grok_com_managed", None, McpServerDisplayStatus::Ready),
        cta_mcp_server("local-srv", None, McpServerDisplayStatus::Ready),
        cta_mcp_server("other-srv", Some("slack"), McpServerDisplayStatus::Ready),
        cta_mcp_server(
            "figma-srv",
            Some("figma"),
            McpServerDisplayStatus::NeedsAuth,
        ),
    ];
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(servers),
        }),
        &mut app,
    );
    // The CTA is finished; the modal owns the flow from here
    assert_eq!(test_agent(&app, id).plugin_cta.phase, CtaPhase::Hidden);
    // Modal opened to the MCP Servers tab.
    let modal = test_agent(&app, id)
        .extensions_modal
        .as_ref()
        .expect("extensions modal should be open");
    assert_eq!(modal.active_tab, ExtensionsTab::McpServers);
    // Session team id seeded so the Managed subtitle deep link matches Ctrl+O.
    assert_eq!(modal.session_team_id.as_deref(), Some("team-uuid"));
    // MCP tab seeded directly from the read we already have (no flash).
    match &modal.mcps_data {
        TabDataState::Loaded(servers) => assert_eq!(servers.len(), 4),
        other => panic!("expected mcps_data Loaded, got {other:?}"),
    }
    // Managed, Local, and other plugins collapsed; only the target plugin expanded
    let collapsed = &modal.mcps_collapsed_sections;
    assert!(collapsed.contains(&section_key(&McpSectionId::Managed)));
    assert!(collapsed.contains(&section_key(&McpSectionId::Local)));
    assert!(collapsed.contains(&section_key(&McpSectionId::Plugin("slack".into()))));
    assert!(!collapsed.contains(&section_key(&McpSectionId::Plugin("figma".into()))));
    assert!(modal.mcps_section_collapse_initialized);
    // Emits the same full set of tab fetches as a manual open so no tab is stuck Loading, plus the candidate refresh
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchHooksList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchPluginsList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchMarketplaceList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchMcpsList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchSkillsList { .. }))
            .count(),
        1
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaCatalog { .. }))
    );
}

#[test]
fn cta_mcps_loaded_no_needs_auth_terminal_sets_installed() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::mcps_modal::McpServerDisplayStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::AwaitingMcps {
            name: "figma".into(),
        };
        cta.expects_mcp = true;
    }
    // The plugin server is present and Ready (terminal, no auth), so the CTA settles now
    let servers = vec![cta_mcp_server(
        "figma-srv",
        Some("figma"),
        McpServerDisplayStatus::Ready,
    )];
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(servers),
        }),
        &mut app,
    );
    assert_eq!(
        test_agent(&app, id).plugin_cta.phase,
        CtaPhase::Installed {
            name: "figma".into()
        }
    );
    assert!(test_agent(&app, id).extensions_modal.is_none());
    // No modal repopulation; settling emits the auto-dismiss timer and the candidate refresh, and never re-fetches the MCP list
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchMcpsList { .. }))
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RetryPluginCtaMcps { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::DismissCtaInstalled { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaCatalog { .. }))
    );
}

#[test]
fn cta_mcps_loaded_later_needs_auth_opens_handoff() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::mcps_modal::McpServerDisplayStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::AwaitingMcps {
            name: "figma".into(),
        };
        cta.expects_mcp = true;
        // Several polls already elapsed before the server reached NeedsAuth.
        cta.mcp_attempt = 5;
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(vec![cta_mcp_server(
                "figma-srv",
                Some("figma"),
                McpServerDisplayStatus::NeedsAuth,
            )]),
        }),
        &mut app,
    );
    // NeedsAuth is terminal: the modal opens immediately even mid-poll
    assert_eq!(test_agent(&app, id).plugin_cta.phase, CtaPhase::Hidden);
    assert!(test_agent(&app, id).extensions_modal.is_some());
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RetryPluginCtaMcps { .. }))
    );
}

/// A bash command typed while a turn is running goes straight to the server: an Effect and an optimistic echo, no local queue entry.
#[test]
fn bash_while_running_is_server_authoritative() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(Action::SendBashCommand("ls -la".into()), &mut app);
    let Some(effect) = effects.first() else {
        panic!("expected an effect, got {effects:?}");
    };
    let pid = match effect {
        Effect::SendBashCommand {
            command, prompt_id, ..
        } => {
            assert_eq!(command, "ls -la");
            prompt_id.clone()
        }
        other => panic!("expected immediate SendBashCommand, got {other:?}"),
    };
    assert_eq!(test_agent(&app, id).session.queue_len(), 0);
    // Optimistic echo present with kind="bash".
    let q = app
        .shared_prompt_queue("test-session")
        .expect("echo present");
    assert_eq!(q.len(), 1);
    let Some(front) = q.first() else {
        panic!("expected queue echo: {q:?}");
    };
    assert_eq!(front.id, pid);
    assert_eq!(front.kind, "bash");
    assert_eq!(front.text, "ls -la");
}

#[test]
fn auth_complete_triggers_bundle_status_fetch() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
        }),
        &mut app,
    );

    assert!(matches!(app.auth_state, AuthState::Done));
    // The pager only refreshes the on-disk catalog snapshot; the bundle download runs inside the shell after auth
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchBundleStatus))
    );
}

#[test]
fn auth_complete_with_deferred_load_also_fetches_status() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    app.deferred_startup.session =
        Some(crate::app::session_startup::DeferredSessionStartup::Load {
            session_id: "test-session".into(),
            session_cwd: None,
            chat_kind: false,
        });

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
        }),
        &mut app,
    );

    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchBundleStatus))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadSession { .. }))
    );
    assert!(app.deferred_startup.session.is_none());
}

/// `/login` from the welcome screen (startup, logged out) must not stash a return view; the normal login-then-load flow is preserved.
#[test]
fn login_from_welcome_does_not_stash_return_view() {
    let mut app = test_app();
    assert_eq!(app.active_view, ActiveView::Welcome);

    dispatch(Action::Login, &mut app);

    assert_eq!(app.active_view, ActiveView::Welcome);
    assert_eq!(app.auth_return_view, None);
}

/// A build without account logins refuses a login, a logout or an account switch, in the session.
#[test]
fn without_account_logins_login_and_logout_are_refused_in_the_session() {
    let refusal =
        xai_grok_config::Distribution::withholding(&[xai_grok_config::Capability::AccountLogin])
            .refusal(xai_grok_config::Capability::AccountLogin)
            .expect("a build without account logins refuses one");
    for action in [Action::Login, Action::Logout, Action::SwitchAccount] {
        let mut app = test_app_with_agent();
        app.distribution = xai_grok_config::Distribution::withholding(&[
            xai_grok_config::Capability::AccountLogin,
        ]);
        let view = app.active_view;

        let effects = dispatch(action, &mut app);

        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(view, app.active_view, "no login screen");
        assert_eq!(refusal, last_system_text(&app, AgentId(0)));
    }
}

#[test]
fn without_account_logins_the_welcome_login_is_refused() {
    let mut app = test_app();
    app.distribution =
        xai_grok_config::Distribution::withholding(&[xai_grok_config::Capability::AccountLogin]);
    app.auth_state = AuthState::Pending { error: None };

    assert!(dispatch(Action::Login, &mut app).is_empty());
    assert!(matches!(app.auth_state, AuthState::Pending { .. }));
    let refusal =
        xai_grok_config::Distribution::withholding(&[xai_grok_config::Capability::AccountLogin])
            .refusal(xai_grok_config::Capability::AccountLogin);
    assert_eq!(
        refusal,
        app.welcome_toast
            .as_ref()
            .map(|(message, _)| message.as_str())
    );
}

/// Compact-auth recovery: the prompt is held across an auto-compact 401, stashed on PromptResponse, and resubmitted on a mid-session AuthComplete.
#[test]
fn e2e_compact_auth_failure_holds_prompt_and_resubmits_after_login() {
    use crate::app::acp_handler::apply_session_event_for_test;
    use crate::app::agent::{AgentState, InFlightPrompt};
    use crate::scrollback::EntryId;
    use crate::scrollback::block::RenderBlock;
    use xai_grok_shell::extensions::notification::{RetryState, SessionUpdate as XaiSessionUpdate};

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-compact-auth-e2e"));
        agent.session.current_prompt_id = Some("prompt-1".into());
        agent.session.in_flight_prompt = Some(InFlightPrompt {
            text: "please continue after login".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(1),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });

        apply_session_event_for_test(
            &XaiSessionUpdate::AutoCompactStarted {
                tokens_used: 180_000,
                context_window: 200_000,
                percentage: 90,
                threshold_percent: None,
                threshold_tokens: None,
                reason: "threshold".into(),
            },
            &mut agent.session,
            &mut agent.scrollback,
        );
        assert!(
            agent.session.in_flight_prompt.is_none(),
            "cancel rewind must still be blocked mid-compact"
        );
        assert_eq!(
            agent
                .session
                .compact_held_prompt
                .as_ref()
                .map(|p| p.text.as_str()),
            Some("please continue after login"),
            "must hold the prompt text for reauth auto-resubmit"
        );

        apply_session_event_for_test(
            &XaiSessionUpdate::AutoCompactFailed {
                error: "authentication problem — re-authenticate using /login and retry.".into(),
            },
            &mut agent.session,
            &mut agent.scrollback,
        );
        assert!(agent.session.compact_held_prompt.is_some());

        apply_session_event_for_test(
            &XaiSessionUpdate::RetryState(RetryState::Failed {
                error_type: "auth".into(),
                message: "Unauthorized (401): compaction failed".into(),
            }),
            &mut agent.session,
            &mut agent.scrollback,
        );
        let has_reauth = (0..agent.scrollback.len()).any(|i| {
            matches!(
                agent.scrollback.entry(i).map(|e| &e.block),
                Some(RenderBlock::SessionEvent(ev))
                    if matches!(ev.event, SessionEvent::ReAuthRequired)
            )
        });
        assert!(has_reauth, "RetryState auth must show ReAuthRequired");
    }

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: Some("prompt-1".into()),
        }),
        &mut app,
    );
    assert_eq!(
        test_agent(&app, id)
            .reauth_stashed_prompt
            .as_ref()
            .map(|p| p.text.as_str()),
        Some("please continue after login"),
        "PromptResponse must stash the compact-held prompt for AuthComplete"
    );

    dispatch(Action::Login, &mut app);
    let seq = authenticating_seq(&app);
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: seq,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        test_agent(&app, id).reauth_stashed_prompt.is_none(),
        "stash consumed on AuthComplete"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "AuthComplete must resubmit the prompt so compact runs again with valid auth, got: {effects:?}"
    );
}

/// Login that never stashed a return view (welcome, dashboard, another process) must still retry the 401'd prompt.
#[test]
fn auth_complete_retries_stashed_prompt_without_a_return_view() {
    use crate::app::agent::InFlightPrompt;
    use crate::scrollback::EntryId;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-auth-complete-no-return"));
        agent.reauth_stashed_prompt = Some(InFlightPrompt {
            text: "continue after login".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(1),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 7,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 7,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        test_agent(&app, id).reauth_stashed_prompt.is_none(),
        "stash consumed on AuthComplete without a return view"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "AuthComplete must resubmit the stashed prompt, got: {effects:?}"
    );
}

/// Operator 2026-10-21: "Hello??? I just logged in and it just says Waiting for the model... it's been a minute."
/// AuthComplete after login must POST the interrupted turn immediately with recoveredBearer / live parent JWT.
/// Parent already TurnRunning (Waiting for the model) must not block that POST.
#[test]
fn auth_complete_after_login_posts_immediately_while_waiting_for_the_model() {
    use crate::app::agent::InFlightPrompt;
    use crate::scrollback::EntryId;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-wait-after-login"));
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
        agent.reauth_stashed_prompt = Some(InFlightPrompt {
            text: "interrupted parent turn".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(1),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 41,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 41,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                recovered_bearer: true,
                recovered_bearer_jwt: Some(jwt),
                ..
            } if session_id.0.as_ref() == "sess-wait-after-login"
                && text == "interrupted parent turn"
                && jwt == PARENT_SAMPLER_CURRENT_API_KEY
        ) || matches!(
            e,
            Effect::SendPrompt { session_id, text, .. }
                if session_id.0.as_ref() == "sess-wait-after-login"
                    && text == "interrupted parent turn"
        )),
        "AuthComplete after login must POST immediately with recoveredBearer / live parent JWT while chrome is Waiting for the model, got: {effects:?}"
    );
}

/// Operator 2026-10-21: login while the parent turn is still in flight (no reauth stash).
/// AuthComplete must POST that interrupted in_flight prompt immediately. Do not sit on Waiting for the model with no POST.
#[test]
fn auth_complete_after_login_posts_in_flight_prompt_while_waiting_for_the_model() {
    use crate::app::agent::InFlightPrompt;
    use crate::scrollback::EntryId;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-wait-in-flight"));
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
        agent.reauth_stashed_prompt = None;
        agent.session.in_flight_prompt = Some(InFlightPrompt {
            text: "still waiting parent turn".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(2),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 42,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 42,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                recovered_bearer: true,
                recovered_bearer_jwt: Some(jwt),
                ..
            } if session_id.0.as_ref() == "sess-wait-in-flight"
                && text == "still waiting parent turn"
                && jwt == PARENT_SAMPLER_CURRENT_API_KEY
        )),
        "AuthComplete after login must POST the in-flight parent turn immediately with recoveredBearer / live parent JWT, got: {effects:?}"
    );
}

/// Live parent sampler JWT (`parent_sampler_handle.current_api_key()`). Nested recoveredBearer
/// puts this on `_meta.recoveredBearerJwt`.
const PARENT_SAMPLER_CURRENT_API_KEY: &str = "parent-sampler-current-api-key";

/// Operator contract: after re-auth, nested L2s that 401'd resume the same
/// way a dropped network does. AuthComplete must not leave those rows failed.
#[test]
fn auth_complete_resumes_nested_l2s_that_401d() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 9,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 9,
            meta: None,
        }),
        &mut app,
    );
    let nested = test_agent(&app, id)
        .subagent_sessions
        .get("cs-nested-401")
        .expect("nested row stays");
    assert!(
        !nested.finished,
        "AuthComplete must un-finish the 401'd nested L2 so occupancy restore can resume it"
    );
    assert!(
        nested.attempt.status.is_none(),
        "AuthComplete must clear the failed nested status so the row is live again"
    );
    assert_eq!(
        test_agent(&app, id)
            .session
            .session_id
            .as_ref()
            .map(|sid| sid.0.as_ref()),
        Some("sess-parent-reauth"),
        "AuthComplete must keep the parent session bound"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::LoadSession { .. })),
        "LoadSession of the nested id would replace the parent chat, got: {effects:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                ..
            } if session_id.0.as_ref() == "cs-nested-401" && text == "implement the plan chrome"
        )),
        "AuthComplete must resend the nested L2 prompt after re-auth, got: {effects:?}"
    );
}

const NESTED_BEARER_NO_AUTH_CONTEXT_401: &str = "Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)";

fn nested_l2_that_401d(parent_sid: &str) -> crate::app::subagent::SubagentInfo {
    let now = std::time::Instant::now();
    crate::app::subagent::SubagentInfo {
        subagent_id: "sa-nested-401".into(),
        child_session_id: "cs-nested-401".into(),
        description: "Plan chrome, limits, freeze, jumping-the-gun".into(),
        subagent_type: "general-purpose".into(),
        finished: true,
        status: Some("failed".into()),
        attempt: crate::app::subagent::SubagentAttemptInfo {
            lifecycle: crate::app::subagent::SubagentLifecycleState::running_legacy_for_test(),
            persona: None,
            role: Some("implementer".into()),
            model: None,
            context_source: None,
            resumed_from: None,
            capability_mode: None,
            workflow_run_id: None,
            context_normalized: false,
            parent_prompt_id: None,
            parent_session_id: Some(parent_sid.into()),
            depth: Some(1),
            started_at: now,
            last_progress_at: now,
            status: Some("failed".into()),
            error: Some(NESTED_BEARER_NO_AUTH_CONTEXT_401.into()),
            duration_ms: None,
            tool_calls: None,
            turns: None,
            turn_count: None,
            tool_call_count: None,
            tokens_used: None,
            tokens_past: 0,
            context_window_tokens: None,
            context_usage_pct: None,
            tools_used: Vec::new(),
            error_count: None,
            activity_label: None,
            is_background: false,
            pending_kill: false,
            kill_requested_at: None,
            scrollback_entry_id: None,
            terminal_entry_id: None,
        },
        completed_attempt_tokens: 0,
        sealed_attempt_tokens: Default::default(),
        prompt: Some("implement the plan chrome".into()),
        child_cwd: None,
        worktree_path: None,
        transcript: Default::default(),
    }
}

fn scrollback_has_resuming_nested_work(agent: &crate::app::agent_view::AgentView) -> bool {
    (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(crate::scrollback::block::RenderBlock::System(sys))
                if crate::app::dispatch::is_resuming_nested_work_copy(&sys.text)
        )
    })
}

fn scrollback_has_reauth_required(agent: &crate::app::agent_view::AgentView) -> bool {
    (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(crate::scrollback::block::RenderBlock::SessionEvent(ev))
                if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    })
}

fn scrollback_has_request_failed(agent: &crate::app::agent_view::AgentView) -> bool {
    (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(crate::scrollback::block::RenderBlock::SessionEvent(ev))
                if matches!(ev.event, SessionEvent::RequestFailed { .. })
        )
    })
}

/// Operator: "This is a fucking lie btw: Re-authenticated. Resuming nested work…"
/// The product does not paint that sentence when the nested retry still 401s with
/// `auth_kind=bearer` plus `reason=no auth context`. Nested resume did not start
/// as a success. Keep the nested 401 visible. Do not paint `/login`.
#[test]
fn product_does_not_paint_reauthenticated_resuming_nested_work_when_the_nested_retry_still_401s_with_bearer_no_auth_context()
 {
    use crate::app::acp_handler::apply_session_event_for_test;
    use crate::scrollback::block::RenderBlock;
    use xai_grok_shell::extensions::notification::{RetryState, SessionUpdate as XaiSessionUpdate};

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 11,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 11,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                ..
            } if session_id.0.as_ref() == "cs-nested-401" && text == "implement the plan chrome"
        )),
        "AuthComplete may still un-finish nested L2s and resend, got: {effects:?}"
    );
    assert!(
        !scrollback_has_resuming_nested_work(test_agent(&app, id)),
        "AuthComplete must not paint Re-authenticated. Resuming nested work before the nested POST is in flight or succeeded"
    );

    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.scrollback.push_block(RenderBlock::system(
            "Re-authenticated. Resuming nested work\u{2026}".to_string(),
        ));
        agent.scrollback.push_block(RenderBlock::system(
            "Re-authenticated. Resuming nested work...".to_string(),
        ));
        apply_session_event_for_test(
            &XaiSessionUpdate::RetryState(RetryState::Failed {
                error_type: "auth".into(),
                message: NESTED_BEARER_NO_AUTH_CONTEXT_401.into(),
            }),
            &mut agent.session,
            &mut agent.scrollback,
        );
    }
    let agent = test_agent(&app, id);
    assert!(
        !scrollback_has_resuming_nested_work(agent),
        "owed outcome: the product does not paint Re-authenticated. Resuming nested work when the nested retry still 401s with auth_kind=bearer plus reason=no auth context"
    );
    assert!(
        !scrollback_has_reauth_required(agent),
        "nested 401 with no auth context must not tell the Operator to /login while the parent still talks"
    );
    assert!(
        scrollback_has_request_failed(agent),
        "keep the nested 401 visible, got scrollback without RequestFailed"
    );
}

/// The copy may paint once a nested UnstickResendPrompt actually dispatched and
/// the send succeeded. Parent AuthComplete alone is not that.
#[test]
fn nested_resume_copy_paints_only_after_unstick_resend_prompt_response_ok() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
    }
    app.auth_state = AuthState::Authenticating {
        request_seq: 12,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 12,
            meta: None,
        }),
        &mut app,
    );
    let prompt_id = effects.iter().find_map(|e| match e {
        Effect::UnstickResendPrompt { prompt_id, .. } => Some(prompt_id.clone()),
        _ => None,
    });
    let prompt_id = prompt_id.expect("AuthComplete must emit UnstickResendPrompt");
    assert!(
        !scrollback_has_resuming_nested_work(test_agent(&app, id)),
        "copy waits until the nested resend is actually dispatched"
    );

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Ok(acp::PromptResponse::new(acp::StopReason::EndTurn).meta(
                serde_json::json!({ "promptId": prompt_id.clone() })
                    .as_object()
                    .cloned(),
            )),
            http_status: None,
            prompt_id: Some(prompt_id),
        }),
        &mut app,
    );
    assert!(
        scrollback_has_resuming_nested_work(test_agent(&app, id)),
        "nested resume copy may paint after the UnstickResendPrompt send succeeded"
    );
}

/// Nested UnstickResendPrompt that 401s with the live paste must not paint the
/// success sentence. AuthComplete already queued the resend.
#[test]
fn nested_unstick_resend_prompt_response_401_does_not_paint_resuming_nested_work() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
    }
    app.auth_state = AuthState::Authenticating {
        request_seq: 13,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 13,
            meta: None,
        }),
        &mut app,
    );
    let prompt_id = effects.iter().find_map(|e| match e {
        Effect::UnstickResendPrompt { prompt_id, .. } => Some(prompt_id.clone()),
        _ => None,
    });
    let prompt_id = prompt_id.expect("AuthComplete must emit UnstickResendPrompt");
    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err(NESTED_BEARER_NO_AUTH_CONTEXT_401.to_string()),
            http_status: Some(401),
            prompt_id: Some(prompt_id),
        }),
        &mut app,
    );
    let agent = test_agent(&app, id);
    assert!(
        !scrollback_has_resuming_nested_work(agent),
        "owed outcome: the product does not paint Re-authenticated. Resuming nested work when the nested retry still 401s with auth_kind=bearer plus reason=no auth context"
    );
    assert!(
        !scrollback_has_reauth_required(agent),
        "nested 401 with no auth context must not tell the Operator to /login while the parent still talks"
    );
}

/// Operator: "And also you don't resume after login. You just sit there as if I've done nothing, which is not useful."
/// After AuthComplete, resend the interrupted nested prompt with recovered Bearer.
#[test]
fn auth_complete_resends_last_nested_prompt_with_recovered_bearer() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 17,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 17,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                recovered_bearer: true,
                ..
            } if session_id.0.as_ref() == "cs-nested-401" && text == "implement the plan chrome"
        )),
        "AuthComplete must resend the last nested prompt with recovered Bearer, got: {effects:?}"
    );
    assert!(
        test_agent(&app, id).session.state.is_turn_running(),
        "after login the parent must not sit idle; nested POST with recovered Bearer is in flight"
    );
    assert!(
        !scrollback_has_resuming_nested_work(test_agent(&app, id)),
        "do not paint Re-authenticated. Resuming nested work until that nested POST is in flight or succeeded"
    );
}

/// Operator: "Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)"
/// AuthComplete after that 401 starts a nested POST with recoveredBearer.
#[tokio::test]
async fn auth_complete_after_nested_bearer_no_auth_context_401_starts_nested_post_with_recovered_bearer()
 {
    use crate::app::effects::{SessionFlags, execute};
    use std::path::Path;
    use tokio::task::JoinSet;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 31,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let mut effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 31,
            meta: None,
        }),
        &mut app,
    );
    let unstick = effects
        .drain(..)
        .find(|e| {
            matches!(
                e,
                Effect::UnstickResendPrompt {
                    session_id,
                    recovered_bearer: true,
                    recovered_bearer_jwt: Some(jwt),
                    ..
                } if session_id.0.as_ref() == "cs-nested-401"
                    && jwt == PARENT_SAMPLER_CURRENT_API_KEY
            )
        })
        .expect("AuthComplete after that 401 must emit UnstickResendPrompt recoveredBearer");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let Some(xai_acp_lib::AcpAgentMessage::Prompt(args)) = rx.recv().await else {
            panic!("AuthComplete after that 401 must start a nested POST");
        };
        let meta = args.request.meta.as_ref().expect("_meta");
        assert_eq!(
            meta.get("recoveredBearer"),
            Some(&serde_json::Value::Bool(true)),
            "nested POST must carry recoveredBearer. Operator: Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)"
        );
        assert_eq!(
            meta.get("recoveredBearerJwt"),
            Some(&serde_json::Value::String(
                PARENT_SAMPLER_CURRENT_API_KEY.to_string()
            )),
            "recoveredBearerJwt must be the live parent sampler JWT"
        );
        let _ = args
            .response_tx
            .send(Ok(acp::PromptResponse::new(acp::StopReason::EndTurn)));
    });
    let mut tasks = JoinSet::new();
    let (progress_tx, _progress_rx) = tokio::sync::mpsc::unbounded_channel();
    execute(
        unstick,
        &mut tasks,
        &tx,
        Path::new("."),
        &SessionFlags::default(),
        &progress_tx,
    );
    let result = tasks
        .join_next()
        .await
        .expect("nested POST task")
        .expect("nested POST must not panic");
    match result {
        TaskResult::PromptResponse { result, .. } => {
            assert!(result.is_ok(), "nested POST with recoveredBearer must send");
        }
        other => panic!("expected PromptResponse from nested POST, got {other:?}"),
    }
}

/// Operator: "And also you don't resume after login. You just sit there as if I've done nothing, which is not useful."
/// A still-running nested 401 (`finished` still false) must resume on AuthComplete.
/// Prior fixtures all set `finished: true`, so that hole sat untested.
#[test]
fn auth_complete_resumes_still_running_nested_l2_that_401d() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
        let mut info = nested_l2_that_401d("sess-parent-reauth");
        info.finished = false;
        info.status = None;
        info.attempt.status = None;
        info.attempt.error = Some(NESTED_BEARER_NO_AUTH_CONTEXT_401.into());
        agent.subagent_sessions.insert("cs-nested-401".into(), info);
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 19,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 19,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                recovered_bearer: true,
                ..
            } if session_id.0.as_ref() == "cs-nested-401" && text == "implement the plan chrome"
        )),
        "AuthComplete must resume a still-running nested L2 that 401'd even when finished is still false. Operator: Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context). And also you don't resume after login. You just sit there as if I've done nothing, which is not useful. Got: {effects:?}"
    );
    assert!(
        test_agent(&app, id).session.state.is_turn_running(),
        "after login the parent must not sit idle; nested POST with recovered Bearer is in flight"
    );
    assert!(
        !scrollback_has_resuming_nested_work(test_agent(&app, id)),
        "do not paint Re-authenticated. Resuming nested work until that nested POST is in flight or succeeded"
    );
}

/// Operator: "Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)"
/// "And also you don't resume after login. You just sit there as if I've done nothing, which is not useful."
/// Nested bearer + xai-grok-cli + no auth context 401 must not leave the parent sitting idle.
#[test]
fn nested_bearer_no_auth_context_401_does_not_leave_parent_sitting_idle() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
    }
    app.auth_state = AuthState::Authenticating {
        request_seq: 18,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 18,
            meta: None,
        }),
        &mut app,
    );
    let prompt_id = effects.iter().find_map(|e| match e {
        Effect::UnstickResendPrompt {
            prompt_id,
            recovered_bearer: true,
            ..
        } => Some(prompt_id.clone()),
        _ => None,
    });
    let prompt_id = prompt_id.expect("AuthComplete must resend nested work with recovered Bearer");
    assert!(
        test_agent(&app, id).session.state.is_turn_running(),
        "AuthComplete must not leave the parent idle after login"
    );

    let retry = dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err(NESTED_BEARER_NO_AUTH_CONTEXT_401.to_string()),
            http_status: Some(401),
            prompt_id: Some(prompt_id),
        }),
        &mut app,
    );
    let agent = test_agent(&app, id);
    assert!(
        agent.session.state.is_turn_running(),
        "nested bearer+xai-grok-cli+no auth context 401 must not leave the parent sitting idle as if login did nothing"
    );
    assert!(
        retry.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                recovered_bearer: true,
                ..
            } if session_id.0.as_ref() == "cs-nested-401" && text == "implement the plan chrome"
        )),
        "nested 401 after login must resend the interrupted nested prompt with recovered Bearer, got: {retry:?}"
    );
    assert!(
        !scrollback_has_resuming_nested_work(agent),
        "do not paint Re-authenticated. Resuming nested work unless a nested POST using that Bearer is in flight or succeeded"
    );
    assert!(
        !scrollback_has_reauth_required(agent),
        "nested 401 with no auth context must not tell the Operator to /login while the parent still talks"
    );
}

/// Operator: "Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)"
/// UnstickResendPrompt recovered_bearer true results in recovered_bearer_jwt equal to parent sampler current_api_key.
#[test]
fn unstick_resend_prompt_recovered_bearer_true_results_in_recovered_bearer_jwt_equal_to_parent_sampler_current_api_key()
 {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.parent_sampler_jwt = Some(PARENT_SAMPLER_CURRENT_API_KEY.to_string());
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 21,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 21,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                recovered_bearer: true,
                recovered_bearer_jwt: Some(jwt),
                ..
            } if session_id.0.as_ref() == "cs-nested-401"
                && jwt == PARENT_SAMPLER_CURRENT_API_KEY
        )),
        "UnstickResendPrompt recovered_bearer true must carry recovered_bearer_jwt equal to parent sampler current_api_key, got: {effects:?}"
    );
}

/// Operator: "Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context)"
/// "And also you don't resume after login. You just sit there as if I've done nothing, which is not useful."
/// AuthComplete after login with no hand-seeded parent_sampler_jwt still emits UnstickResendPrompt.
/// Nested session/prompt is sent with recoveredBearer true; nested resolves the resident parent JWT.
#[test]
fn auth_complete_after_login_with_no_hand_seeded_parent_sampler_jwt_still_emits_unstick_resend_prompt()
 {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.parent_sampler_jwt = None;
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 22,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 22,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                text,
                recovered_bearer: true,
                recovered_bearer_jwt: None,
                ..
            } if session_id.0.as_ref() == "cs-nested-401" && text == "implement the plan chrome"
        )),
        "AuthComplete after login must emit UnstickResendPrompt even when parent_sampler_jwt is none. Operator: Request failed: Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=PermissionDenied, reason=no auth context). And also you don't resume after login. You just sit there as if I've done nothing, which is not useful. Got: {effects:?}"
    );
    assert!(
        test_agent(&app, id).session.state.is_turn_running(),
        "after login the parent must not sit idle; nested POST with recoveredBearer is in flight"
    );
    assert!(
        !scrollback_has_resuming_nested_work(test_agent(&app, id)),
        "do not paint Re-authenticated. Resuming nested work unless a nested POST using that Bearer is in flight or succeeded"
    );
}

/// Shell AuthenticateResponse `_meta` as `auth_response_with_meta` actually builds it
/// (`recoveredBearerJwt` / `parentSamplerJwt` / `parentSamplerJwts`).
/// AuthComplete fills parent_sampler_jwt from that meta, not a hand-seeded pager field.
#[test]
fn auth_complete_fills_parent_sampler_jwt_from_authenticate_response_meta() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.session.session_id = Some(acp::SessionId::new("sess-parent-reauth"));
        agent.parent_sampler_jwt = None;
        agent.subagent_sessions.insert(
            "cs-nested-401".into(),
            nested_l2_that_401d("sess-parent-reauth"),
        );
    }
    app.auth_return_view = None;
    app.auth_state = AuthState::Authenticating {
        request_seq: 23,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 23,
            meta: Some(serde_json::json!({
                "recoveredBearerJwt": PARENT_SAMPLER_CURRENT_API_KEY,
                "parentSamplerJwt": PARENT_SAMPLER_CURRENT_API_KEY,
                "parentSamplerJwts": {
                    "sess-parent-reauth": PARENT_SAMPLER_CURRENT_API_KEY
                }
            })),
        }),
        &mut app,
    );
    assert_eq!(
        test_agent(&app, id).parent_sampler_jwt.as_deref(),
        Some(PARENT_SAMPLER_CURRENT_API_KEY),
        "AuthComplete must copy recoveredBearerJwt / parentSamplerJwt from AuthenticateResponse _meta"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::UnstickResendPrompt {
                session_id,
                recovered_bearer: true,
                recovered_bearer_jwt: Some(jwt),
                ..
            } if session_id.0.as_ref() == "cs-nested-401"
                && jwt == PARENT_SAMPLER_CURRENT_API_KEY
        )),
        "UnstickResendPrompt recovered_bearer_jwt must equal parent sampler current_api_key from login meta, got: {effects:?}"
    );
}

/// Without `compact_held_prompt`, clearing `in_flight_prompt` when compact starts leaves nothing for reauth to stash.
#[test]
fn pre_fix_compact_start_without_hold_cannot_stash_for_reauth() {
    use crate::app::agent::AgentState;
    use crate::scrollback::block::RenderBlock;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-pre-fix"));
        agent.session.current_prompt_id = Some("p1".into());
        agent.session.in_flight_prompt = None;
        agent.session.compact_held_prompt = None;
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }
    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: Some("p1".into()),
        }),
        &mut app,
    );
    assert!(
        test_agent(&app, id).reauth_stashed_prompt.is_none(),
        "without compact_held / in_flight, reauth cannot stash — the pre-fix bug"
    );
}

/// A second auth-failed turn with no rewindable prompt (`in_flight_prompt == None`) must not clobber the stash from an earlier 401.
#[test]
fn second_auth_failure_does_not_clobber_reauth_stash() {
    use crate::scrollback::block::RenderBlock;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::InFlightPrompt {
            text: "first prompt".into(),
            images: Vec::new(),
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.in_flight_prompt = None;
    }

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: None,
        }),
        &mut app,
    );

    assert_eq!(
        test_agent(&app, id)
            .reauth_stashed_prompt
            .as_ref()
            .map(|prompt| prompt.text.as_str()),
        Some("first prompt"),
        "a None in_flight_prompt must not wipe an earlier stash"
    );
}

/// Cancelling a mid-session re-auth drops the stashed prompt so it is not silently resubmitted on a later, unrelated login.
#[test]
fn cancel_login_drops_reauth_stashed_prompt() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().reauth_stashed_prompt =
        Some(crate::app::agent::InFlightPrompt {
            text: "stale".into(),
            images: Vec::new(),
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });

    dispatch(Action::Login, &mut app);
    dispatch(Action::CancelLogin, &mut app);

    assert!(
        test_agent(&app, id).reauth_stashed_prompt.is_none(),
        "cancelling re-auth must drop the stashed prompt"
    );
}

/// Cancelling a mid-session re-auth strips the stale `ReAuthRequired` prompt from scrollback.
/// A later `PromptResponse` can then no longer re-detect it and re-stash the prompt for silent resubmission.
#[test]
fn cancel_login_strips_reauth_prompt_from_scrollback() {
    use crate::scrollback::block::RenderBlock;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::InFlightPrompt {
            text: "stale".into(),
            images: Vec::new(),
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }

    dispatch(Action::Login, &mut app);
    dispatch(Action::CancelLogin, &mut app);

    let sb = &test_agent(&app, id).scrollback;
    let has_reauth = (0..sb.len()).any(|i| {
        matches!(
            sb.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev)) if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    });
    assert!(
        !has_reauth,
        "cancelling re-auth must strip the stale re-auth prompt from scrollback"
    );
}

/// Cancelling `/login` must strip a re-auth banner that a later nested-run summary pushed off the trailing run.
#[test]
fn cancel_login_strips_reauth_prompt_that_is_not_trailing() {
    use crate::scrollback::block::RenderBlock;
    use crate::scrollback::blocks::SubagentBlock;
    use std::time::Duration;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
        agent
            .scrollback
            .push_block(RenderBlock::Subagent(SubagentBlock::failed(
                "nested run",
                "child-1",
                Duration::from_secs(1),
                None,
            )));
    }

    dispatch(Action::Login, &mut app);
    dispatch(Action::CancelLogin, &mut app);

    let sb = &test_agent(&app, id).scrollback;
    let has_reauth = (0..sb.len()).any(|i| {
        matches!(
            sb.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev)) if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    });
    assert!(
        !has_reauth,
        "cancelling re-auth must strip a non-trailing re-auth prompt"
    );
}

/// Empty `auth_methods` (the preferred_method pin is unavailable) must not invent `grok.com` or start an OIDC flow the agent did not advertise.
#[test]
fn login_with_empty_auth_methods_fails_closed() {
    let mut app = test_app_with_agent();
    app.auth_methods.clear();
    app.login_method_id = None;

    let effects = dispatch(Action::Login, &mut app);

    assert!(
        effects.is_empty(),
        "must not start Authenticate without an advertised method"
    );
    assert_eq!(
        app.active_view,
        ActiveView::Agent(AgentId(0)),
        "must stay on the session view"
    );
    assert!(
        matches!(
            &app.auth_state,
            AuthState::Pending { error: Some(msg) }
                if msg.contains("preferred_method=api_key")
        ),
        "must surface pin-unavailable error, got {:?}",
        app.auth_state
    );
    assert!(app.login_method_id.is_none());
}

/// Puts the app in `Authenticating` with a live task's abort handle installed, as the event loop would.
/// Returns the task's JoinHandle and the seq.
/// Callers assert the task actually gets aborted (`unwrap_err().is_cancelled()`), not merely that the handle slot was cleared.
fn install_live_auth_task(
    app: &mut AppView,
    rt: &tokio::runtime::Runtime,
) -> (tokio::task::JoinHandle<()>, u64) {
    dispatch(Action::Login, app);
    let task = rt.spawn(std::future::pending::<()>());
    match &mut app.auth_state {
        AuthState::Authenticating {
            handle,
            request_seq,
            ..
        } => {
            *handle = Some(task.abort_handle());
            (task, *request_seq)
        }
        other => panic!("expected Authenticating after Login, got {other:?}"),
    }
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// A second `/login` while already authenticating must abort the prior auth task and bump the seq.
/// Single-flight: never two device-code requests running at once.
#[test]
fn login_while_authenticating_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    let (prior_task, first_seq) = install_live_auth_task(&mut app, &rt);

    let effects = dispatch(Action::Login, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "prior auth task must be aborted"
        );
    });
    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(
                *request_seq > first_seq,
                "re-login must bump request_seq for single-flight"
            );
        }
        other => panic!("expected Authenticating after re-Login, got {other:?}"),
    }
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Authenticate { .. })),
        "re-login must emit a new Authenticate"
    );
}

/// A stale `AuthComplete` (its abort lost the race because the task had already finished) must not complete the new attempt.
/// The request-seq guard is the only protection here.
#[test]
fn stale_auth_complete_after_relogin_is_ignored() {
    let mut app = test_app_with_agent();
    dispatch(Action::Login, &mut app);
    let first_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        other => panic!("expected Authenticating after Login, got {other:?}"),
    };
    dispatch(Action::Login, &mut app); // re-login bumps to seq2

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: first_seq,
            meta: None,
        }),
        &mut app,
    );

    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(
                *request_seq > first_seq,
                "stale AuthComplete must leave the new attempt authenticating"
            );
        }
        other => panic!("stale AuthComplete must be ignored, got {other:?}"),
    }
}

/// Switch-account while authenticating goes through the same single-flight abort as `/login` (sibling entry point).
#[test]
fn switch_account_while_authenticating_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    let (prior_task, first_seq) = install_live_auth_task(&mut app, &rt);

    dispatch(Action::SwitchAccount, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "prior auth task must be aborted on switch-account"
        );
    });
    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(*request_seq > first_seq, "switch must bump request_seq");
        }
        other => panic!("expected Authenticating after SwitchAccount, got {other:?}"),
    }
}

/// Cancelling a mid-session login aborts the in-flight auth task (not just restores the view) so a retry cannot race a still-polling prior attempt.
#[test]
fn cancel_login_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    // Login from a session view stashes `auth_return_view`, making CancelLogin live.
    let (prior_task, _) = install_live_auth_task(&mut app, &rt);

    dispatch(Action::CancelLogin, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "cancel must abort the in-flight auth task"
        );
    });
}

/// Cancelling a mid-session login returns to the session rather than quitting the app, and clears the stashed view and auth state.
#[test]
fn cancel_login_restores_view() {
    let mut app = test_app_with_agent();
    dispatch(Action::Login, &mut app);
    assert_eq!(app.active_view, ActiveView::Welcome);
    let prior_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        other => panic!("expected Authenticating after Login, got {other:?}"),
    };

    let effects = dispatch(Action::CancelLogin, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelAuth { request_seq }] if *request_seq == prior_seq
        ),
        "cancel must tell the shell to stop the in-flight auth poll for this attempt"
    );
    assert_eq!(app.active_view, ActiveView::Agent(AgentId(0)));
    assert_eq!(app.auth_return_view, None);
    assert!(matches!(app.auth_state, AuthState::Done));
}

/// Named contract: a new grok-oss must leave Connecting... and show a real
/// auth error without hanging forever on last-session restore.
#[test]
fn connecting_splash_timeout_leaves_connecting_without_last_session_restore() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    assert!(
        app.auth_state.is_connecting_splash(),
        "precondition: Connecting... splash"
    );
    let load = dispatch(
        Action::LoadSession("huge-last-session".into(), None, false),
        &mut app,
    );
    assert!(
        load.is_empty(),
        "must not start last-session restore while Connecting..., got {load:?}"
    );
    assert!(
        matches!(
            app.deferred_startup.session,
            Some(crate::app::session_startup::DeferredSessionStartup::Load { ref session_id, .. })
                if session_id == "huge-last-session"
        ),
        "last-session stays deferred, got {:?}",
        app.deferred_startup.session
    );

    let effects = dispatch(Action::ConnectingSplashTimeout, &mut app);
    assert!(
        !app.auth_state.is_connecting_splash(),
        "must leave Connecting..., got {:?}",
        app.auth_state
    );
    match &app.auth_state {
        AuthState::Pending { error: Some(e) } => {
            assert_eq!(e, crate::app::app_view::CONNECTING_SPLASH_TIMEOUT_ERROR)
        }
        other => panic!("expected a real auth error, got {other:?}"),
    }
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::CancelAuth { request_seq: 1 })),
        "must cancel the hung authenticate, got {effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::LoadSession { .. })),
        "must not hang on last-session restore to leave Connecting..., got {effects:?}"
    );
    assert!(
        app.deferred_startup.session.is_some(),
        "last-session stays deferred until a real AuthComplete"
    );
}

#[test]
fn auth_url_ready_without_url_leaves_connecting_splash() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthUrlReady {
            request_seq: 1,
            auth_url: None,
            external: false,
            mode: None,
        }),
        &mut app,
    );
    assert!(
        !app.auth_state.is_connecting_splash(),
        "a finished URL poll with no URL must leave Connecting..., got {:?}",
        app.auth_state
    );
    match &app.auth_state {
        AuthState::Pending { error: Some(e) } => {
            assert_eq!(e, crate::app::app_view::CONNECTING_SPLASH_TIMEOUT_ERROR)
        }
        other => panic!("expected a real auth error, got {other:?}"),
    }
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::CancelAuth { request_seq: 1 })),
        "got {effects:?}"
    );
}

#[test]
fn auth_url_ready_with_url_keeps_authenticating() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthUrlReady {
            request_seq: 1,
            auth_url: Some("https://accounts.x.ai/oauth2/device?user_code=ABCD".into()),
            external: false,
            mode: Some("device".into()),
        }),
        &mut app,
    );
    assert!(effects.is_empty());
    match &app.auth_state {
        AuthState::Authenticating {
            auth_url: Some(url),
            mode: AuthMode::Device,
            ..
        } => {
            assert!(url.contains("accounts.x.ai"));
        }
        other => panic!("device URL must leave Connecting... for Device, got {other:?}"),
    }
}

/// `CancelLogin` outside a mid-session login is a no-op (must not move off the welcome screen or panic).
#[test]
fn cancel_login_noop_without_stashed_view() {
    let mut app = test_app();
    let effects = dispatch(Action::CancelLogin, &mut app);
    assert!(effects.is_empty());
    assert_eq!(app.active_view, ActiveView::Welcome);
    assert_eq!(app.auth_return_view, None);
}

#[test]
fn auth_complete_extracts_show_resolved_model_from_meta() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    assert!(app.show_resolved_model);

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: Some(serde_json::json!({ "show_resolved_model": false })),
        }),
        &mut app,
    );

    assert!(!app.show_resolved_model);
}

#[test]
fn auth_complete_preserves_show_resolved_model_when_absent() {
    let mut app = test_app();
    app.show_resolved_model = false;
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: Some(serde_json::to_value(xai_grok_login::AuthMeta::default()).unwrap()),
        }),
        &mut app,
    );

    assert!(!app.show_resolved_model);
}
