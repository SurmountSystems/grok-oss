#![cfg_attr(rustfmt, rustfmt::skip)]
    use super::*;

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

