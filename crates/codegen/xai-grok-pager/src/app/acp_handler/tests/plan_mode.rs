#![cfg_attr(rustfmt, rustfmt::skip)]
    use super::*;

    #[test]
    fn exit_plan_mode_soft_parks_with_toast_not_modal() {
        // Option A (feat:plan-modal-softer-park): exit_plan_mode parks durable
        // approval + toast without hard-opening the line-viewer modal.
        // Option C: also commits an inline transcript plan card.
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("still drafting");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Cursor Plan"));

        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();

        assert!(agent.plan_approval_view.is_some(), "approval must park durably");
        assert!(
            agent.line_viewer.is_none(),
            "soft park must not auto-open the plan modal"
        );
        assert_eq!(
            agent.toast.as_ref().map(|(m, _)| m.as_str()),
            Some(crate::views::plan_approval_view::PLAN_PARKED_TOAST),
            "soft park must show the non-modal review toast"
        );
        assert_eq!(
            agent.prompt.text(),
            "still drafting",
            "soft park must not clear the live prompt"
        );
        assert_eq!(
            crate::views::plan_approval_view::plan_approval_status_label(true),
            "Plan parked — click or /view-plan to review"
        );
        // Option C: transcript card present with plan preview + CTA legend.
        assert!(
            agent.scrollback.len() >= 1,
            "soft park must commit an inline plan card"
        );
        assert_eq!(
            agent.plan_card_committed_id.as_deref(),
            Some("create-plan-call")
        );
        let card = match &agent.scrollback.entry(agent.scrollback.len() - 1).unwrap().block {
            crate::scrollback::block::RenderBlock::AgentMessage(b) => b.text(),
            other => panic!("expected plan card agent message, got {other:?}"),
        };
        assert!(
            card.contains("# Cursor Plan")
                && card.contains(crate::views::plan_approval_view::PLAN_CARD_CTAS),
            "card must embed plan body and CTAs; got {card:?}"
        );
    }

    #[test]
    fn exit_plan_mode_force_modal_when_plan_approval_park_modal() {
        // Option D: [ui] plan_approval_park = "modal" opens the line-viewer
        // immediately as fullscreen (no soft toast-only park / no side panel).
        let mut app = make_app_with_agent("sess-1");
        app.current_ui.plan_approval_park = Some("modal".into());
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("draft must be stashed");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Modal Plan"));

        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(agent.plan_approval_view.is_some());
        assert!(
            agent.line_viewer.is_some(),
            "force-modal must open the plan line-viewer"
        );
        assert_eq!(
            agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.markdown_content_for_test()),
            Some("# Modal Plan")
        );
        let viewer = agent.line_viewer.as_ref().unwrap();
        assert!(
            viewer.fullscreen && !viewer.side_panel,
            "force-modal must open fullscreen, not side panel"
        );
        // Not soft-park: no PLAN_PARKED_TOAST and no transcript card commit.
        assert_ne!(
            agent.toast.as_ref().map(|(m, _)| m.as_str()),
            Some(crate::views::plan_approval_view::PLAN_PARKED_TOAST),
            "force-modal must not show soft-park toast"
        );
        assert!(
            agent.plan_card_committed_id.is_none(),
            "force-modal must not commit the soft-park transcript card"
        );
        assert_eq!(
            agent.prompt.text(),
            "",
            "force-modal reopen must clear live prompt"
        );
        assert_eq!(
            agent
                .plan_approval_view
                .as_ref()
                .map(|p| p.stashed_prompt.text.as_str()),
            Some("draft must be stashed"),
            "force-modal must stash the live draft"
        );
    }

    #[test]
    fn exit_plan_mode_soft_park_reopen_opens_side_panel() {
        // Option B: on-demand reopen docks as a right-hand side panel (not
        // fullscreen takeover) with approval CTAs and plan body.
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Cursor Plan"));

        assert!(handle_exit_plan_mode(ext, &mut app));
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            assert!(agent.line_viewer.is_none());
            agent.reopen_plan_approval();
            let viewer = agent.line_viewer.as_ref().expect("side panel");
            assert!(viewer.side_panel, "reopen must dock as side panel");
            assert!(!viewer.fullscreen, "reopen must not force fullscreen");
            assert!(
                viewer.plan_ref().is_some_and(|p| p.feedback_active),
                "side panel footer CTAs must be active"
            );
            assert_eq!(viewer.markdown_content_for_test(), Some("# Cursor Plan"));
        }
    }

    #[test]
    fn exit_plan_keeps_inline_plan_preview_available() {
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# First Plan"));

        assert!(handle_exit_plan_mode(ext, &mut app));
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            assert_eq!(
                agent.plan_approval_view.as_ref().map(|s| s.source),
                Some(crate::views::plan_approval_view::PlanReviewSource::Inline)
            );
            agent.line_viewer = None;
            agent.show_plan_preview();
            assert_eq!(
                agent
                    .line_viewer
                    .as_ref()
                    .and_then(|v| v.markdown_content_for_test()),
                Some("# First Plan")
            );
        }
    }

    #[test]
    fn exit_plan_without_inline_content_uses_file_backed_source() {
        let mut app = make_app_with_agent("sess-1");
        let (ext, _rx) = make_exit_plan_ext(Some("# File Plan"));

        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(agent.latest_inline_plan_content.is_none());

        assert_eq!(
            agent.plan_approval_view.as_ref().map(|s| s.source),
            Some(crate::views::plan_approval_view::PlanReviewSource::FileBacked)
        );
        // Soft park: no auto-open. On demand, file-backed body still opens via
        // request plan_content even when plan.md is not on disk under the cwd.
        assert!(agent.line_viewer.is_none());
        agent.reopen_plan_approval();
        assert_eq!(
            agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.markdown_content_for_test()),
            Some("# File Plan")
        );
    }

    #[test]
    fn exit_plan_mode_empty_soft_parks_placeholder_on_demand() {
        // Empty plan.md soft-parks with toast + status; placeholder opens when
        // the user engages (reopen / /view-plan / status click).
        let mut app = make_app_with_agent("sess-1");
        let (ext, _rx) = make_exit_plan_ext(None);

        assert!(handle_exit_plan_mode(ext, &mut app));
        {
            let agent = app.agents.get(&AgentId(0)).unwrap();
            let pav = agent
                .plan_approval_view
                .as_ref()
                .expect("plan_approval_view must be set");
            assert!(!pav.has_plan);
            assert_eq!(
                pav.focus,
                crate::views::plan_approval_view::PlanApprovalFocus::Preview,
                "soft park keeps Preview focus until the user opens the modal"
            );
            assert!(agent.line_viewer.is_none());
            assert_eq!(
                agent.toast.as_ref().map(|(m, _)| m.as_str()),
                Some(crate::views::plan_approval_view::PLAN_PARKED_TOAST)
            );
        }
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            agent.reopen_plan_approval();
            assert_eq!(
                agent
                    .line_viewer
                    .as_ref()
                    .and_then(|v| v.markdown_content_for_test()),
                Some(crate::views::plan_approval_view::EMPTY_PLAN_PLACEHOLDER)
            );
        }
    }

    #[test]
    fn exit_plan_mode_soft_park_preserves_open_modal() {
        // Soft park must not yank the command palette out from under the user.
        // Engaging the plan surface (reopen) still dismisses it so the plan paints.
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.active_modal = Some(crate::views::modal::ActiveModal::CommandPalette {
                entries: crate::views::modal::default_palette_entries(
                    agent.sharing_enabled,
                    agent.prompt.slash_controller.screen_mode(),
                ),
                state: crate::views::picker::PickerState::input_active(),
                window: crate::views::modal_window::ModalWindowState::new(),
            });
        }

        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Cursor Plan"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        {
            let agent = app.agents.get(&AgentId(0)).unwrap();
            assert!(
                agent.active_modal.is_some(),
                "soft park must leave the open modal alone"
            );
            assert!(agent.plan_approval_view.is_some());
            assert!(agent.line_viewer.is_none());
        }
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            agent.reopen_plan_approval();
            assert!(
                agent.active_modal.is_none(),
                "reopen must dismiss the modal so the plan preview is visible"
            );
            assert!(agent.line_viewer.is_some());
        }
    }

    #[test]
    fn exit_plan_mode_soft_park_preserves_block_viewer_until_reopen() {
        // Soft park leaves Edit/tool block_viewer alone; reopen dismisses it so
        // wheel scroll reaches the plan line_viewer.
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.block_viewer = Some(crate::views::block_viewer::BlockViewerPane::for_plain_text(
                "edit",
                "diff content",
            ));
        }

        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Cursor Plan"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        {
            let agent = app.agents.get(&AgentId(0)).unwrap();
            assert!(
                agent.block_viewer.is_some(),
                "soft park must leave block_viewer alone"
            );
            assert!(agent.plan_approval_view.is_some());
            assert!(agent.line_viewer.is_none());
        }
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            agent.reopen_plan_approval();
            assert!(
                agent.block_viewer.is_none(),
                "reopen must dismiss block_viewer so the plan can scroll"
            );
            assert!(agent.line_viewer.is_some());
        }
    }

    #[test]
    fn later_empty_exit_plan_request_clears_stale_inline_plan() {
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (first, _first_rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# First Plan"));
        let (second, _second_rx) = make_exit_plan_ext(None);

        assert!(handle_exit_plan_mode(first, &mut app));
        {
            let agent = app.agents.get(&AgentId(0)).unwrap();
            assert_eq!(
                agent.latest_inline_plan_content.as_deref(),
                Some("# First Plan")
            );
        }
        assert!(handle_exit_plan_mode(second, &mut app));
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(agent.latest_inline_plan_content.is_none());
        // Soft park: empty approval parks without auto-opening; toast still fires.
        assert!(agent.line_viewer.is_none());
        assert_eq!(
            agent.toast.as_ref().map(|(m, _)| m.as_str()),
            Some(crate::views::plan_approval_view::PLAN_PARKED_TOAST)
        );
        agent.reopen_plan_approval();
        assert_eq!(
            agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.markdown_content_for_test()),
            Some(crate::views::plan_approval_view::EMPTY_PLAN_PLACEHOLDER)
        );
    }

    #[test]
    fn exit_plan_mode_shows_overlay() {
        let mut app = make_app_with_agent("sess-A");
        assert!(!app.agents.get(&AgentId(0)).unwrap().session.is_yolo());

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let ext_req = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "sess-A".into(),
            tool_call_id: "tc-normal".into(),
            plan_content: Some("# Plan\nDo stuff".into()),
        };
        let raw = serde_json::value::to_raw_value(&ext_req).unwrap();
        let msg = AcpClientMessage::ExtMethod(xai_acp_lib::AcpArgs {
            request: acp::ExtRequest::new("x.ai/exit_plan_mode", raw.into()),
            response_tx: tx,
        });

        let affected = handle(msg, &mut app);

        assert!(affected, "opening the overlay should need a redraw");
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some(),
            "plan_approval_view must be set for interactive approval"
        );
        assert!(
            rx.try_recv().is_err(),
            "response must NOT have been sent yet (waiting for user)"
        );
    }

    #[test]
    fn exit_plan_mode_shows_overlay_even_in_yolo() {
        let mut app = make_app_with_agent("sess-A");
        app.agents.get_mut(&AgentId(0)).unwrap().session.yolo_mode = true;

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let ext_req = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "sess-A".into(),
            tool_call_id: "tc-yolo".into(),
            plan_content: Some("# Plan\nDo stuff".into()),
        };
        let raw = serde_json::value::to_raw_value(&ext_req).unwrap();
        let msg = AcpClientMessage::ExtMethod(xai_acp_lib::AcpArgs {
            request: acp::ExtRequest::new("x.ai/exit_plan_mode", raw.into()),
            response_tx: tx,
        });

        let affected = handle(msg, &mut app);

        assert!(affected, "overlay should open even in yolo mode");
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_some(),
            "plan_approval_view must be set even in always-approve mode"
        );
        assert!(
            rx.try_recv().is_err(),
            "response must NOT have been sent yet (waiting for user)"
        );
    }

    #[test]
    fn exit_plan_mode_routes_to_background_session_not_active_view() {
        let mut app = make_app_with_agent("sess-A");
        insert_agent(&mut app, AgentId(1), Some("sess-B"));

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let ext_req = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "sess-B".into(),
            tool_call_id: "tc-bg-plan".into(),
            plan_content: Some("# Plan".into()),
        };
        let raw = serde_json::value::to_raw_value(&ext_req).unwrap();
        let msg = AcpClientMessage::ExtMethod(xai_acp_lib::AcpArgs {
            request: acp::ExtRequest::new("x.ai/exit_plan_mode", raw.into()),
            response_tx: tx,
        });

        let affected = handle(msg, &mut app);

        assert!(
            !affected,
            "a background-session plan approval must not redraw the active view"
        );
        assert!(
            app.agents
                .get(&AgentId(1))
                .unwrap()
                .plan_approval_view
                .is_some(),
            "plan approval must be parked on the session that asked (background agent B)"
        );
        assert!(
            app.agents
                .get(&AgentId(0))
                .unwrap()
                .plan_approval_view
                .is_none(),
            "plan approval must NOT land on the unrelated active agent A"
        );
        assert!(rx.try_recv().is_err(), "response must NOT be sent yet");
    }

    /// Regression: tool-call titles containing `"enter_plan_mode"` must not
    /// flip plan mode (the substring matcher used to brick sessions on any
    /// tool mentioning the phrase, e.g. a Grep with that pattern).
    #[test]
    fn tool_call_with_enter_plan_mode_substring_does_not_activate_plan_mode() {
        let mut agent = make_agent(Some("s1"));
        assert!(!agent.plan_mode_active);

        let updates = [
            make_tool_call("enter_plan_mode"),
            make_tool_call_update("enter_plan_mode"),
            make_tool_call("Execute `rg enter_plan_mode`"),
            make_tool_call_update("Execute `rg enter_plan_mode`"),
            make_tool_call_update("Plan mode entered"),
            make_tool_call("mcp__foo__enter_plan_mode"),
        ];
        for update in &updates {
            let refresh_needed = detect_plan_mode_change(update, &mut agent);
            assert!(
                !refresh_needed,
                "tool-call title (not a CurrentModeUpdate) must not request refresh"
            );
            assert!(
                !agent.plan_mode_active,
                "tool-call title must not flip plan mode"
            );
        }
    }

    /// Symmetric: tool-call titles containing `"exit_plan_mode"` must not
    /// deactivate plan mode either. Exit is signaled by `CurrentModeUpdate`.
    #[test]
    fn tool_call_with_exit_plan_mode_substring_does_not_deactivate_plan_mode() {
        let mut agent = make_agent(Some("s1"));
        agent.plan_mode_active = true;

        let updates = [
            make_tool_call("exit_plan_mode"),
            make_tool_call_update("exit_plan_mode"),
            make_tool_call_update("Plan mode exited"),
            make_tool_call("Execute `rg exit_plan_mode`"),
        ];
        for update in &updates {
            let refresh_needed = detect_plan_mode_change(update, &mut agent);
            assert!(!refresh_needed);
            assert!(
                agent.plan_mode_active,
                "tool-call title must not flip plan mode"
            );
        }
    }

    #[test]
    fn current_mode_update_plan_activates_plan_mode() {
        let mut agent = make_agent(Some("s1"));
        assert!(!agent.plan_mode_active);

        let refresh_needed = detect_plan_mode_change(&make_current_mode_update("plan"), &mut agent);
        assert!(refresh_needed);
        assert!(agent.plan_mode_active);
        assert!(agent.plan_mode_pending.is_none());
    }

    #[test]
    fn current_mode_update_default_deactivates_plan_mode() {
        let mut agent = make_agent(Some("s1"));
        agent.plan_mode_active = true;
        agent.plan_mode_pending = Some(true);

        let refresh_needed =
            detect_plan_mode_change(&make_current_mode_update("default"), &mut agent);
        assert!(refresh_needed);
        assert!(!agent.plan_mode_active);
        assert!(agent.plan_mode_pending.is_none());
    }

    /// Unknown mode ids (e.g. a custom agent definition name like
    /// `"browser_use"`) parse to `SessionMode::Default` and deactivate
    /// plan mode.
    #[test]
    fn current_mode_update_unknown_id_treated_as_default() {
        let mut agent = make_agent(Some("s1"));
        agent.plan_mode_active = true;

        let refresh_needed =
            detect_plan_mode_change(&make_current_mode_update("browser_use"), &mut agent);
        assert!(refresh_needed);
        assert!(!agent.plan_mode_active);
    }

    /// Idempotent CurrentModeUpdate still signals refresh because
    /// `plan_mode_pending` was cleared (affects effective state).
    #[test]
    fn current_mode_update_signals_refresh_even_on_no_op_active_change() {
        let mut agent = make_agent(Some("s1"));
        agent.plan_mode_active = true;
        agent.plan_mode_pending = Some(true);

        let refresh_needed = detect_plan_mode_change(&make_current_mode_update("plan"), &mut agent);
        assert!(
            refresh_needed,
            "CurrentModeUpdate must always signal refresh — pending was cleared"
        );
        assert!(agent.plan_mode_active);
        assert!(agent.plan_mode_pending.is_none());
    }

