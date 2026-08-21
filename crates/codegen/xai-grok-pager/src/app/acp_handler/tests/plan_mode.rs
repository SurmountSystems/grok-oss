#![cfg_attr(rustfmt, rustfmt::skip)]
    use super::*;

    /// Mid-turn `exit_plan_mode` (not restore) still auto-opens the pane.
    #[test]
    fn live_exit_plan_mode_present_still_docks_side_panel() {
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Live present"));

        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(agent.plan_approval_view.is_some());
        assert!(
            agent.line_viewer.is_some(),
            "live mid-turn present must still auto-open the plan side panel"
        );
        assert_eq!(
            agent.plan_loop_status_label(),
            Some("Plan ready. Side panel open"),
        );
    }

    /// Restore / resume re-park must keep the live waiter and must not dock.
    #[test]
    fn resume_restore_parks_waiter_without_docking_side_panel() {
        use crate::app::actions::Action;
        use crate::app::dispatch::dispatch;
        use crate::views::plan_approval_view::{PLAN_IDLE_REVIEW_STATUS, PLAN_READY_STATUS};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "exit-plan-mode-resume-sess-1", "CreatePlan");
            agent.plan_mode_active = true;
        }
        let (ext, rx) = make_exit_plan_ext_with_tool_call_id(
            "exit-plan-mode-resume-sess-1",
            Some("# Restored waiter"),
        );
        assert!(handle_exit_plan_mode(ext, &mut app));
        {
            let agent = app.agents.get(&AgentId(0)).unwrap();
            assert!(
                agent
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|p| !p.is_local_idle_decision && p.response_tx.is_some()),
                "restore must park a live waiter"
            );
            assert!(
                agent.line_viewer.is_none(),
                "restore must not auto-dock the plan side panel"
            );
            assert_ne!(
                agent.plan_loop_status_label(),
                Some(PLAN_READY_STATUS),
                "restore must not paint Plan ready while the pane is shut"
            );
            assert_ne!(
                agent.plan_loop_status_label(),
                Some(PLAN_IDLE_REVIEW_STATUS),
                "restore must not idle as Plan written. Click or /view-plan"
            );
            assert_ne!(
                agent.plan_loop_status_label(),
                Some("Plan ready. Side panel open"),
                "restore must not claim the side panel is open"
            );
        }

        let _ = dispatch(Action::ShowPlan, &mut app);
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            let pav = agent
                .plan_approval_view
                .as_ref()
                .expect("/view-plan must reopen the restored waiter");
            assert!(!pav.is_local_idle_decision);
            assert!(pav.response_tx.is_some());
            assert!(
                agent
                    .line_viewer
                    .as_ref()
                    .is_some_and(|v| v.feedback_active()),
                "/view-plan must bind Approve to the restored waiter"
            );
            agent.approve_plan();
        }
        let response = rx.blocking_recv().expect("Approve must complete the live waiter");
        let raw = response.expect("waiter response Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
        assert_eq!(parsed["outcome"], "approved");
    }

    /// Restore after Approve/Quit must not re-arm Plan ready chrome.
    #[test]
    fn resume_restore_skips_when_plan_decision_resolved() {
        use crate::views::plan_approval_view::PLAN_READY_STATUS;

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "exit-plan-mode-resume-sess-1", "CreatePlan");
            agent.plan_mode_active = true;
            agent.plan_decision_resolved = true;
            agent.prompt.set_text("btcdragonlord.com is not mine either btw");
        }
        let (ext, _rx) = make_exit_plan_ext_with_tool_call_id(
            "exit-plan-mode-resume-sess-1",
            Some("# Leftover after Approve"),
        );
        let _ = handle_exit_plan_mode(ext, &mut app);
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.plan_approval_view.is_none(),
            "restore must not re-park after Approve/Quit"
        );
        assert!(agent.plan_decision_resolved);
        assert!(agent.line_viewer.is_none());
        assert_ne!(agent.plan_loop_status_label(), Some(PLAN_READY_STATUS));
        assert_eq!(
            agent.prompt.text(),
            "btcdragonlord.com is not mine either btw",
            "restore skip must keep the mid-type draft"
        );
    }

    #[test]
    fn exit_plan_mode_auto_opens_inline_cursor_plan_preview() {
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Cursor Plan"));

        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();

        assert!(agent.plan_approval_view.is_some());
        assert_eq!(
            agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.markdown_content_for_test()),
            Some("# Cursor Plan")
        );
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
        // File-backed bodies still open via request plan_content even when
        // plan.md is not on disk under the agent's cwd.
        assert_eq!(
            agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.markdown_content_for_test()),
            Some("# File Plan")
        );
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.feedback_active()),
            "file-backed exit_plan_mode must arm five-CTA approval chrome"
        );
        assert_eq!(
            agent.plan_loop_status_label(),
            Some("Plan ready. Side panel open"),
            "file-backed park is review, not Waiting on plan approval"
        );
    }

    #[test]
    fn plan_approval_soft_park_is_not_fullscreen() {
        let mut app = make_app_with_agent("sess-1");
        app.current_ui.plan_approval_park = Some("soft".into());
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Soft Park"));
        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();
        let viewer = agent
            .line_viewer
            .as_ref()
            .expect("soft park must open the plan preview");
        assert!(
            !viewer.fullscreen,
            "[ui] plan_approval_park = soft must open a side panel, not fullscreen"
        );
        assert!(
            !app.current_ui.plan_approval_force_modal(),
            "pager must consult plan_approval_force_modal (soft is false)"
        );
    }

    #[test]
    fn plan_approval_modal_park_is_fullscreen() {
        let mut app = make_app_with_agent("sess-1");
        app.current_ui.plan_approval_park = Some("modal".into());
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Modal Park"));
        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();
        let viewer = agent
            .line_viewer
            .as_ref()
            .expect("modal park must open the plan preview");
        assert!(
            viewer.fullscreen,
            "[ui] plan_approval_park = modal must force fullscreen"
        );
        assert!(
            app.current_ui.plan_approval_force_modal(),
            "pager must consult plan_approval_force_modal (modal is true)"
        );
    }

    #[test]
    fn exit_plan_mode_empty_opens_placeholder_preview() {
        // Empty plan.md must still surface a decision UI — otherwise the user
        // only sees "Waiting on plan approval" with a dead Tab:plan and thinks
        // the session is stuck.
        let mut app = make_app_with_agent("sess-1");
        let (ext, _rx) = make_exit_plan_ext(None);

        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();

        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("plan_approval_view must be set");
        assert!(!pav.has_plan);
        assert_eq!(
            pav.focus,
            crate::views::plan_approval_view::PlanApprovalFocus::Preview,
            "empty approval must keep Preview focus once the placeholder opens"
        );
        assert_eq!(
            agent
                .line_viewer
                .as_ref()
                .and_then(|v| v.markdown_content_for_test()),
            Some(crate::views::plan_approval_view::EMPTY_PLAN_PLACEHOLDER)
        );
    }

    #[test]
    fn exit_plan_mode_dismisses_open_modal() {
        // Regression: if the user has Ctrl+P command palette open when the
        // agent calls exit_plan_mode, the modal must be dismissed so the
        // plan preview is visible and input routes correctly. Otherwise the
        // modal hides the line viewer in draw order while input gets
        // routed to the invisible line viewer, leaving the user stuck.
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.active_modal = Some(crate::views::modal::ActiveModal::CommandPalette {
                entries: crate::views::modal::default_palette_entries(
                    agent.sharing_enabled,
                    &agent.prompt.slash_controller,
                ),
                state: crate::views::picker::PickerState::input_active(),
                window: crate::views::modal_window::ModalWindowState::new(),
            });
        }

        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Cursor Plan"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.active_modal.is_none(),
            "exit_plan_mode must dismiss the open modal so the plan preview is visible"
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_some());
    }

    #[test]
    fn exit_plan_mode_dismisses_open_block_viewer() {
        // Regression: if the user has an Edit/tool block_viewer open when
        // exit_plan_mode opens, dismiss it so wheel scroll reaches the plan
        // line_viewer. Draw returns on line_viewer (plan visible) but
        // handle_scroll prefers block_viewer while it remains in state.
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

        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            agent.block_viewer.is_none(),
            "exit_plan_mode must dismiss open block_viewer so the plan can scroll"
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(agent.line_viewer.is_some());
    }

    #[test]
    fn new_exit_plan_mode_present_clears_decision_resolved_and_in_flight() {
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            agent.plan_decision_resolved = true;
            agent.plan_feedback_in_flight = Some(
                crate::views::plan_approval_view::PlanFeedbackInFlight::Revising,
            );
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Second plan"));
        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert!(
            !agent.plan_decision_resolved,
            "new exit_plan_mode present must clear plan_decision_resolved"
        );
        assert!(
            agent.plan_feedback_in_flight.is_none(),
            "new exit_plan_mode present must clear plan_feedback_in_flight"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "new present must park review chrome (not auto-approve)"
        );
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
        // Empty approval still opens the placeholder decision surface (not a
        // silent "no plan" toast) so the user always sees a way to proceed.
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

    /// Isolated `plan.md` present must not wipe a mid-compose draft or treat
    /// the next letter as Approve. L1 stays the typer.
    #[test]
    fn exit_plan_mode_keeps_mid_compose_draft_and_a_types() {
        use crate::actions::ActionRegistry;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("oh you interrupted my typing");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Isolated plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(
            agent.prompt.text().contains("oh you interrupted my typing"),
            "present must keep the live composer draft, got {:?}",
            agent.prompt.text()
        );
        assert!(agent.plan_approval_view.is_some());
        assert!(
            agent.line_viewer.is_some(),
            "soft park still auto-opens the plan.md side panel"
        );

        let a = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        let _ = agent.handle_input(&a, &ActionRegistry::defaults());
        assert!(
            agent.plan_approval_view.is_some(),
            "mid-compose `a` must type, not Approve"
        );
        assert!(
            agent.prompt.text().contains("oh you interrupted my typing"),
            "draft must still be in the composer after `a`"
        );
        assert!(
            agent.prompt.text().contains('a'),
            "the typed `a` must land in the composer, got {:?}",
            agent.prompt.text()
        );
    }

    /// Force-fullscreen modal park is paint-only. Mid-compose keys stay text.
    #[test]
    fn exit_plan_mode_modal_park_does_not_steal_mid_compose_keys() {
        use crate::actions::ActionRegistry;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        app.current_ui.plan_approval_park = Some("modal".into());
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("still typing a thought");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Modal plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        let viewer = agent
            .line_viewer
            .as_ref()
            .expect("modal park still opens plan.md");
        assert!(
            viewer.fullscreen,
            "modal park may paint fullscreen; it must not steal keys"
        );
        assert!(
            agent.prompt.text().contains("still typing a thought"),
            "modal present must keep the live draft, got {:?}",
            agent.prompt.text()
        );

        let q = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        let _ = agent.handle_input(&q, &ActionRegistry::defaults());
        assert!(
            agent.plan_approval_view.is_some(),
            "mid-compose `q` must type, not Quit"
        );
        assert!(
            agent.prompt.text().contains('q'),
            "typed `q` must land in the composer, got {:?}",
            agent.prompt.text()
        );
    }

    /// Empty composer after present: printable letters still go to the
    /// composer (non-capturing side panel). Empty-prompt `a` still Approves.
    #[test]
    fn exit_plan_mode_empty_present_printable_goes_to_composer() {
        use crate::actions::ActionRegistry;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Empty present"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(agent.prompt.text().trim().is_empty());
        let h = Event::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        let _ = agent.handle_input(&h, &ActionRegistry::defaults());
        assert!(
            agent.plan_approval_view.is_some(),
            "a non-accelerator letter must not decide the plan"
        );
        assert_eq!(
            agent.prompt.text(),
            "h",
            "printable keys go to the composer after present, got {:?}",
            agent.prompt.text()
        );
    }

    fn assert_exit_plan_approved(
        rx: tokio::sync::oneshot::Receiver<xai_acp_lib::AcpResult<acp::ExtResponse>>,
    ) {
        let response = rx.blocking_recv().expect("Approve must complete the live waiter");
        let raw = response.expect("waiter response Ok");
        let parsed: serde_json::Value = serde_json::from_str(raw.0.get()).expect("json");
        assert_eq!(
            parsed["outcome"], "approved",
            "Approve must complete x.ai/exit_plan_mode as approved; got {parsed:?}"
        );
    }

    /// `/view-plan` after the live present panel is dismissed must reopen
    /// that waiter. Approve must complete the reverse-request. Do not invent
    /// a local idle park whose Approve does nothing to the tool.
    #[test]
    fn view_plan_slash_binds_approve_to_live_exit_plan_mode_waiter() {
        use crate::app::actions::Action;
        use crate::app::dispatch::dispatch;

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "call-view-plan-waiter", "CreatePlan");
            agent.plan_mode_active = true;
        }
        let (ext, rx) =
            make_exit_plan_ext_with_tool_call_id("call-view-plan-waiter", Some("# Live waiter"));
        assert!(handle_exit_plan_mode(ext, &mut app));
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            assert!(
                agent
                    .plan_approval_view
                    .as_ref()
                    .is_some_and(|p| !p.is_local_idle_decision && p.response_tx.is_some()),
                "fixture: live exit_plan_mode waiter is parked"
            );
            agent.cancel_line_viewer();
            assert!(agent.line_viewer.is_none(), "fixture: panel dismissed");
        }

        let _ = dispatch(Action::ShowPlan, &mut app);

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("/view-plan must reopen the live waiter park");
        assert!(
            !pav.is_local_idle_decision,
            "/view-plan must not replace the live waiter with a local idle park"
        );
        assert!(
            pav.response_tx.is_some(),
            "/view-plan must keep the live reverse-request channel"
        );
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.feedback_active()),
            "/view-plan must open the live waiter panel with Approve bound"
        );

        agent.approve_plan();
        assert_exit_plan_approved(rx);
    }

    /// Status click uses the same bind as `/view-plan`: reopen the live
    /// waiter, do not open a second view-only panel.
    #[test]
    fn plan_status_click_binds_approve_to_live_exit_plan_mode_waiter() {
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "call-status-waiter", "CreatePlan");
            agent.plan_mode_active = true;
        }
        let (ext, rx) =
            make_exit_plan_ext_with_tool_call_id("call-status-waiter", Some("# Status waiter"));
        assert!(handle_exit_plan_mode(ext, &mut app));
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            agent.cancel_line_viewer();
            // Status click / plan-chip path (`open_plan_from_view_plan_or_status`).
            agent.open_plan_from_view_plan_or_status();
            let pav = agent
                .plan_approval_view
                .as_ref()
                .expect("status click must reopen the live waiter");
            assert!(!pav.is_local_idle_decision);
            assert!(pav.response_tx.is_some());
            assert!(
                agent
                    .line_viewer
                    .as_ref()
                    .is_some_and(|v| v.feedback_active()),
                "status click must open the live waiter panel"
            );
            agent.approve_plan();
        }
        assert_exit_plan_approved(rx);
    }

    /// `/view-plan` while a subagent is focused must still bind to the live
    /// parent `exit_plan_mode` waiter. Do not park a local idle on the child
    /// whose Approve leaves the tool waiting.
    #[test]
    fn view_plan_from_subagent_binds_to_parent_live_waiter() {
        use crate::app::actions::Action;
        use crate::app::dispatch::dispatch;

        let mut app = make_app_with_parent_and_child("sess-1", "child-sess");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "call-parent-waiter", "CreatePlan");
            agent.plan_mode_active = true;
        }
        let (ext, rx) =
            make_exit_plan_ext_with_tool_call_id("call-parent-waiter", Some("# Parent waiter"));
        assert!(handle_exit_plan_mode(ext, &mut app));
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            agent.cancel_line_viewer();
            agent.active_subagent = Some("child-sess".into());
        }

        let _ = dispatch(Action::ShowPlan, &mut app);

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(
            agent
                .subagent_views
                .get("child-sess")
                .is_some_and(|c| c.plan_approval_view.is_none()
                    || c.plan_approval_view
                        .as_ref()
                        .is_some_and(|p| !p.is_local_idle_decision)),
            "/view-plan must not invent a child local-idle park"
        );
        let pav = agent
            .plan_approval_view
            .as_ref()
            .expect("/view-plan must keep the parent live waiter");
        assert!(!pav.is_local_idle_decision);
        assert!(pav.response_tx.is_some());
        assert!(
            agent
                .line_viewer
                .as_ref()
                .is_some_and(|v| v.feedback_active()),
            "/view-plan must open the parent live waiter panel"
        );
        agent.approve_plan();
        assert_exit_plan_approved(rx);
    }

    /// Resume / rebuild with a live waiter and a mid-type draft must not
    /// paint Plan ready while the pane is shut and Enter is send.
    #[test]
    fn resume_restore_does_not_paint_plan_ready_while_composer_is_send_armed() {
        use crate::app::agent::AgentState;
        use crate::views::plan_approval_view::{PLAN_IDLE_REVIEW_STATUS, PLAN_READY_STATUS};

        const DRAFT: &str = "btcdragonlord.com is not mine either btw";
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "exit-plan-mode-resume-sess-1", "CreatePlan");
            agent.plan_mode_active = true;
            agent.session.state = AgentState::Idle;
            agent.prompt.set_text(DRAFT);
        }
        let (ext, _rx) = make_exit_plan_ext_with_tool_call_id(
            "exit-plan-mode-resume-sess-1",
            Some("# Restored waiter"),
        );
        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert_eq!(agent.prompt.text(), DRAFT, "restore must keep the mid-type draft");
        assert!(
            agent.line_viewer.is_none(),
            "restore must not auto-dock the plan side panel"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_READY_STATUS),
            "shut pane + Enter:send must not paint Plan ready"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_IDLE_REVIEW_STATUS),
            "restore must not idle as Plan written. Click or /view-plan"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some("Plan ready. Side panel open"),
        );
    }

    /// Resume / rebuild re-park must not dock the pane when they were typing.
    #[test]
    fn resume_restore_does_not_open_pane_when_composer_has_draft() {
        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "exit-plan-mode-resume-sess-1", "CreatePlan");
            agent.plan_mode_active = true;
            agent.prompt.set_text("still typing after the reboot");
        }
        let (ext, _rx) = make_exit_plan_ext_with_tool_call_id(
            "exit-plan-mode-resume-sess-1",
            Some("# Restored waiter"),
        );
        assert!(handle_exit_plan_mode(ext, &mut app));
        let agent = app.agents.get(&AgentId(0)).unwrap();
        assert_eq!(
            agent.prompt.text(),
            "still typing after the reboot",
            "resume present must keep the mid-compose draft"
        );
        assert!(
            agent.line_viewer.is_none(),
            "resume must not auto-open the plan pane while they are typing"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(crate::views::plan_approval_view::PLAN_READY_STATUS),
            "resume must not paint Plan ready while the pane is shut"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(crate::views::plan_approval_view::PLAN_IDLE_REVIEW_STATUS),
        );
    }

    /// Idle resume waiter must not steal Enter. A non-empty composer submits
    /// a normal prompt. Empty Enter still never Approves.
    #[test]
    fn resume_restore_mid_compose_enter_sends_normal_prompt() {
        use crate::actions::ActionRegistry;
        use crate::app::actions::Action;
        use crate::app::app_view::InputOutcome;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "exit-plan-mode-resume-sess-1", "CreatePlan");
            agent.plan_mode_active = true;
            agent.prompt.set_text("send this as a normal prompt");
        }
        let (ext, mut rx) = make_exit_plan_ext_with_tool_call_id(
            "exit-plan-mode-resume-sess-1",
            Some("# Restored waiter"),
        );
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        let outcome = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            &ActionRegistry::defaults(),
        );
        match outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(
                    text.contains("send this as a normal prompt"),
                    "Enter must submit the composer as a normal prompt, got {text:?}"
                );
            }
            other => panic!("Enter must SendPrompt, got {other:?}"),
        }
        assert!(
            agent.plan_approval_view.is_some(),
            "submitting a normal prompt must not Approve or Revise the parked plan"
        );
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent."),
            "Enter must not steal the draft as Revise notes"
        );
        assert!(
            rx.try_recv().is_err(),
            "Enter must not complete the exit_plan_mode waiter"
        );
    }

    /// Live mid-turn present must not steal Enter either. Draft stays a
    /// normal prompt, not Revise notes.
    #[test]
    fn live_present_mid_compose_enter_sends_normal_prompt() {
        use crate::actions::ActionRegistry;
        use crate::app::actions::Action;
        use crate::app::app_view::InputOutcome;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("oh you interrupted my typing");
        }
        let (ext, mut rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Isolated plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(
            agent.prompt.text().contains("oh you interrupted my typing"),
            "present must keep the live composer draft"
        );
        let outcome = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            &ActionRegistry::defaults(),
        );
        match outcome {
            InputOutcome::Action(Action::SendPrompt(text)) => {
                assert!(
                    text.contains("oh you interrupted my typing"),
                    "Enter must submit the draft as a normal prompt, got {text:?}"
                );
            }
            other => panic!("Enter must SendPrompt after live present, got {other:?}"),
        }
        assert!(
            agent.plan_approval_view.is_some(),
            "normal submit must leave the plan waiter parked"
        );
        assert_ne!(
            agent.toast.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Plan revision sent.")
        );
        assert!(rx.try_recv().is_err(), "Enter must not Approve or Revise");
    }

    /// Empty Enter on the idle resume waiter never Approves.
    #[test]
    fn resume_restore_empty_enter_never_approves() {
        use crate::actions::ActionRegistry;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "exit-plan-mode-resume-sess-1", "CreatePlan");
            agent.plan_mode_active = true;
            agent.prompt.set_text("");
        }
        let (ext, mut rx) = make_exit_plan_ext_with_tool_call_id(
            "exit-plan-mode-resume-sess-1",
            Some("# Restored waiter"),
        );
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        let _ = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            &ActionRegistry::defaults(),
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "empty Enter must not Approve"
        );
        assert!(
            rx.try_recv().is_err(),
            "empty Enter must not complete the waiter"
        );
    }

    /// Esc still dismisses the open plan pane and keeps the draft.
    #[test]
    fn live_present_esc_dismisses_pane_and_keeps_draft() {
        use crate::actions::ActionRegistry;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("keep this draft");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Isolated plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(agent.line_viewer.is_some(), "live present docks the pane");
        let _ = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &ActionRegistry::defaults(),
        );
        assert!(
            agent.line_viewer.is_none(),
            "Esc must dismiss the plan pane"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "Esc dismisses the pane, it does not Exit the waiter"
        );
        assert_eq!(
            agent.prompt.text(),
            "keep this draft",
            "Esc must not wipe the composer draft"
        );
    }

    fn draw_plan_present_frame(agent: &mut crate::app::agent_view::AgentView) {
        use crate::app::bundle::BundleState;
        use crate::scrollback::render::ScratchBuffer;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        let mut scratch = ScratchBuffer::new();
        agent.last_terminal_size = (120, 40);
        let _ = agent.draw(
            area,
            &mut buf,
            &crate::actions::ActionRegistry::defaults(),
            &mut scratch,
            None,
            false,
            crate::app::agent_view::BannerSlotParams::none(),
            &BundleState::default(),
            false,
            false,
            &mut Vec::new(),
            crate::app::agent_view::AppRenderParams::default(),
        );
    }

    /// Iso 2026-08-19: plan present with a draft and the side panel shut
    /// must still accept printable keys into the composer.
    #[test]
    fn plan_present_closed_panel_nonempty_composer_accepts_printable_keys() {
        use crate::actions::ActionRegistry;
        use crate::app::agent_view::KeyOwner;
        use crate::views::plan_approval_view::PlanApprovalFocus;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text(
                "Need you to spawn a subagent to comply with our new process rules",
            );
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Iso plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        let _ = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &ActionRegistry::defaults(),
        );
        assert!(
            agent.line_viewer.is_none(),
            "fixture: panel is shut after Esc"
        );
        // Click-composer-then-close left Prompt focus on a hidden park.
        // A shut panel must not keep exclusive plan key ownership.
        agent
            .plan_approval_view
            .as_mut()
            .expect("waiter stays")
            .focus = PlanApprovalFocus::Prompt;
        assert_eq!(
            agent.key_owner(),
            KeyOwner::Pane,
            "shut plan panel must not steal the composer keyboard"
        );

        let x = Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        let _ = agent.handle_input(&x, &ActionRegistry::defaults());
        assert!(
            agent.prompt.text().contains(
                "Need you to spawn a subagent to comply with our new process rules"
            ),
            "present must keep the mid-compose draft, got {:?}",
            agent.prompt.text()
        );
        assert!(
            agent.prompt.text().contains('x'),
            "printable keys must land in the composer with the panel shut, got {:?}",
            agent.prompt.text()
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "typing must not Approve or Exit the parked plan"
        );
    }

    /// Iso 2026-08-19: mouse drag on the composer is not eaten by the plan
    /// key owner, including while the present pane is still docked.
    #[test]
    fn plan_present_composer_mouse_drag_is_not_eaten() {
        use crate::actions::ActionRegistry;
        use crossterm::event::{
            Event, MouseButton, MouseEvent, MouseEventKind,
        };

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text(
                "Need you to spawn a subagent to comply with our new process rules",
            );
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Iso plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(agent.line_viewer.is_some(), "fixture: live present docks");
        draw_plan_present_frame(agent);
        let ta = agent.prompt.textarea_area();
        assert!(
            ta.area() > 0,
            "composer textarea must paint so a drag can hit it"
        );
        let prompt = agent.pane_areas.prompt;
        assert!(
            prompt.area() > 0,
            "composer pane must paint so a drag can hit it"
        );

        let mouse = |kind: MouseEventKind, column: u16, row: u16| Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        });
        let row = ta.y;
        let start = ta.x.saturating_add(1);
        let end = ta.x.saturating_add(8).min(ta.x.saturating_add(ta.width.saturating_sub(1)));
        let registry = ActionRegistry::defaults();
        let _ = agent.handle_input(
            &mouse(MouseEventKind::Down(MouseButton::Left), start, row),
            &registry,
        );
        let _ = agent.handle_input(
            &mouse(MouseEventKind::Drag(MouseButton::Left), end, row),
            &registry,
        );
        let _ = agent.handle_input(
            &mouse(MouseEventKind::Up(MouseButton::Left), end, row),
            &registry,
        );
        assert!(
            agent.prompt.textarea.selection_range().is_some(),
            "composer mouse drag must select text; plan present must not eat it"
        );
        assert!(
            agent.prompt.text().contains(
                "Need you to spawn a subagent to comply with our new process rules"
            ),
            "a composer drag must not wipe the draft, got {:?}",
            agent.prompt.text()
        );
    }

    /// Iso 2026-08-19 follow-up: a drag on the transcript while the plan
    /// side panel is still docked must select scrollback text, not get eaten
    /// by the parked line-viewer waiter.
    #[test]
    fn plan_present_transcript_mouse_drag_is_not_eaten() {
        use crate::actions::ActionRegistry;
        use crate::scrollback::text_selection::{
            ResolvedSelectableLine, ResolvedSelectionModel,
        };
        use crossterm::event::{
            Event, MouseButton, MouseEvent, MouseEventKind,
        };

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text(
                "Need you to spawn a subagent to comply with our new process rules",
            );
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Iso plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert!(agent.line_viewer.is_some(), "fixture: live present docks");
        draw_plan_present_frame(agent);

        let sb = agent.pane_areas.scrollback;
        let prompt = agent.pane_areas.prompt;
        let modal = agent
            .line_viewer
            .as_ref()
            .and_then(|v| v.last_modal_area);
        assert!(
            sb.area() > 0,
            "scrollback must paint so a drag can hit it; sb={sb:?}"
        );
        let row = sb.y.saturating_add(sb.height / 2).max(sb.y.saturating_add(1));
        let start = sb.x.saturating_add(1);
        let end = sb
            .x
            .saturating_add(8)
            .min(sb.x.saturating_add(sb.width.saturating_sub(1)));
        let hit = |col: u16| (col, row).into();
        assert!(
            sb.contains(hit(start)) && sb.contains(hit(end)),
            "drag must stay inside the transcript; sb={sb:?} start={start} end={end} row={row}"
        );
        if let Some(m) = modal {
            assert!(
                !m.contains(hit(start)) && !m.contains(hit(end)),
                "drag must miss the plan popup; modal={m:?} start={start} end={end} row={row}"
            );
        }
        assert!(
            !prompt.contains(hit(start)) && !prompt.contains(hit(end)),
            "drag must miss the composer; prompt={prompt:?} start={start} end={end} row={row}"
        );

        let mut model = ResolvedSelectionModel::default();
        model.push_line(ResolvedSelectableLine {
            entry_idx: 0,
            range_id: 0,
            block_line_idx: 0,
            screen_y: row,
            screen_x: sb.x,
            selectable_cols: 0..40,
            text: "selectable scrollback line for plan-present drag".into(),
            joiner_to_previous: None,
        });
        agent.update_scrollback_selection_state(model, Default::default());

        let mouse = |kind: MouseEventKind, column: u16, row: u16| Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        });
        let registry = ActionRegistry::defaults();
        let _ = agent.handle_input(
            &mouse(MouseEventKind::Down(MouseButton::Left), start, row),
            &registry,
        );
        let _ = agent.handle_input(
            &mouse(MouseEventKind::Drag(MouseButton::Left), end, row),
            &registry,
        );
        assert!(
            agent.drag_selection.is_some(),
            "transcript mouse drag must select scrollback text; docked plan present must not eat it"
        );
        let _ = agent.handle_input(
            &mouse(MouseEventKind::Up(MouseButton::Left), end, row),
            &registry,
        );
        assert!(
            agent.prompt.text().contains(
                "Need you to spawn a subagent to comply with our new process rules"
            ),
            "a transcript drag must not wipe the draft, got {:?}",
            agent.prompt.text()
        );
        assert!(
            agent.line_viewer.is_some(),
            "transcript drag must not dismiss the docked plan pane"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "transcript drag must not Approve or Exit the parked plan"
        );
    }

    /// Iso 2026-08-19: idle cue must not be Plan written. Click or /view-plan
    /// while the side panel is shut. Shut panel + Enter:send must not paint
    /// Plan ready either.
    #[test]
    fn plan_present_closed_panel_idle_cue_is_not_plan_written_click() {
        use crate::actions::ActionRegistry;
        use crate::views::plan_approval_view::{PLAN_IDLE_REVIEW_STATUS, PLAN_READY_STATUS};
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

        let mut app = make_app_with_agent("sess-1");
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            seed_pending_tool(agent, "create-plan-call", "CreatePlan");
            agent.prompt.set_text("draft stays");
        }
        let (ext, _rx) =
            make_exit_plan_ext_with_tool_call_id("create-plan-call", Some("# Iso plan.md"));
        assert!(handle_exit_plan_mode(ext, &mut app));

        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        let _ = agent.handle_input(
            &Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &ActionRegistry::defaults(),
        );
        assert!(agent.line_viewer.is_none(), "fixture: panel is shut");
        assert_eq!(
            agent.prompt.text(),
            "draft stays",
            "Esc dismiss must keep the composer draft"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_READY_STATUS),
            "shut panel + send-armed composer must not paint Plan ready"
        );
        assert_ne!(
            agent.plan_loop_status_label(),
            Some(PLAN_IDLE_REVIEW_STATUS),
            "shut panel must not idle as Plan written. Click or /view-plan"
        );
        let label = agent.plan_loop_status_label().unwrap_or("");
        assert!(
            !label.contains("Click or /view-plan"),
            "shut-panel status must not be the exclusive click cue, got {label:?}"
        );
        assert!(
            agent.plan_approval_view.is_some(),
            "Esc dismisses the viewer, not the waiter"
        );
    }

    /// Rebuild / session rebind restores an unsent draft into an empty composer.
    #[test]
    fn session_rebind_restores_unsent_composer_draft() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("unsent_prompt_draft");
        xai_grok_shell::session::unsent_prompt_draft::write_draft_at(
            &path,
            "still typing a plan note",
        )
        .unwrap();
        let mut agent = make_agent(Some("sess-1"));
        agent.prompt.set_text("");
        let draft = xai_grok_shell::session::unsent_prompt_draft::load_draft_at(&path)
            .unwrap()
            .expect("draft file");
        agent.apply_unsent_draft_if_empty(&draft);
        assert_eq!(
            agent.prompt.text(),
            "still typing a plan note",
            "empty composer after rebuild must restore the unsent draft"
        );
    }


