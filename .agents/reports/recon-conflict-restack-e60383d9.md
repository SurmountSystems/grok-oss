# Resolve report: e60383d9 mop pick

Sequencer was already cleared; 17 UU remained. Preference: keep 1.0.3 HEAD
APIs; do not replay the old onto compile mop dumps.

Took `ours` (stage 2 / current 1.0.3 restack) for every UU path:

- Cargo.lock (regenerate later if compile needs it)
- sampling-types/lib.rs (keep ApiErrorCode + INVALID_IMAGE_ERROR_CODE)
- request_task.rs (keep error_code / should_retry fields)
- acp_agent.rs, session_lifecycle.rs, persistence.rs, slash_commands.rs
  (incoming was a larger older dump)
- mvp_agent/mod.rs (keep task::types MonitorEventBuffer + UnblockResult.settings)
- auth/flow.rs, manager.rs, sleep_gate.rs, mod.rs (keep 1.0.3 DarkWakeBudget
  and no-mint readiness path)
- mcp.rs (keep xai_grok_session_events spawn ctx)
- recap.rs (keep HEAD tests)
- sampler_turn.rs (keep included SuperGrok period align + 1.0.3 bearer_resolver)
- tool_calls.rs (keep classifier refresh + `_manager_event`)
- storage/search.rs (keep xai-grok-session-search wrapper)

Auto-merged non-UU mop hunks (pager themes, Cargo features, session handlers)
stay staged. No markers left.
