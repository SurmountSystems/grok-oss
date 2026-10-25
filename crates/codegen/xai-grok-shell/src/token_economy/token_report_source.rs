//! Adapter so `token_report` can call L2-B live counters.
//!
//! `xai-grok-tools` cannot import this crate. This module implements
//! [`TokenReportLiveSource`] by calling [`super::live`] and
//! [`super::ledger::model_call_cost_for_sessions`]. It does not copy the
//! ledger addition. It does not own AtomicU64 slots or the `job_cost` schema.

use std::sync::Arc;

use xai_grok_tools::implementations::grok_build::{TokenReportLive, TokenReportLiveSource};

use super::live::{
    LiveTokenLedger, TokenSlot, finish_job_cost, live_cost, live_rollup, live_slot,
    live_totals_for_session,
};
use super::model_call_cost_for_sessions;
use super::token_economy_from_disk;
use crate::grok_oss::try_open_from_token_economy_config;

struct ShellTokenReportLive;

fn slot_from_index(slot: usize) -> Option<TokenSlot> {
    match slot {
        s if s == TokenSlot::Input as usize => Some(TokenSlot::Input),
        s if s == TokenSlot::Output as usize => Some(TokenSlot::Output),
        s if s == TokenSlot::Cached as usize => Some(TokenSlot::Cached),
        s if s == TokenSlot::Reasoning as usize => Some(TokenSlot::Reasoning),
        _ => None,
    }
}

fn open_store() -> Result<crate::grok_oss::GrokOssStore, String> {
    let cfg = token_economy_from_disk();
    try_open_from_token_economy_config(&cfg)
        .ok_or_else(|| "grok_oss.db open failed (fail-open)".to_string())
}

impl TokenReportLiveSource for ShellTokenReportLive {
    fn live_slot(&self, session_id: &str, slot: usize) -> u64 {
        let Some(slot) = slot_from_index(slot) else {
            return 0;
        };
        // live.rs:181
        live_slot(LiveTokenLedger::process(), session_id, slot)
    }

    fn live_totals_for_session(&self, session_id: &str) -> (u64, u64, u64, u64) {
        // live.rs:188
        let totals = live_totals_for_session(LiveTokenLedger::process(), session_id);
        (totals.input, totals.output, totals.cached, totals.reasoning)
    }

    fn live_rollup(&self, session_ids: &[&str]) -> (u64, u64, u64, u64) {
        // live.rs:201
        let totals = live_rollup(LiveTokenLedger::process(), session_ids);
        (totals.input, totals.output, totals.cached, totals.reasoning)
    }

    fn live_cost(&self, session_ids: &[&str]) -> Option<u64> {
        // live.rs:219. Input plus output. Never cached. Never reasoning.
        live_cost(LiveTokenLedger::process(), session_ids)
    }

    fn model_call_cost_for_sessions(&self, session_ids: &[&str]) -> Result<Option<i64>, String> {
        let store = open_store()?;
        // ledger.rs:656. Do not copy the addition at 676-679.
        model_call_cost_for_sessions(&store, session_ids)
            .map(|cost| cost.input_plus_output)
            .map_err(|e| e.to_string())
    }

    fn finish_job_cost(&self, job_ulid: &str) -> Result<Option<i64>, String> {
        let store = open_store()?;
        // live.rs:284. Calls model_call_cost_for_sessions. Do not compute a parallel sum.
        finish_job_cost(&store, job_ulid)
            .map(|cost| cost.input_plus_output)
            .map_err(|e| e.to_string())
    }
}

/// Resource inserted into tool `Resources` so `token_report` can call L2-B.
pub fn token_report_live_resource() -> TokenReportLive {
    TokenReportLive {
        source: Arc::new(ShellTokenReportLive),
    }
}

#[cfg(test)]
mod tests {
    use super::super::live::{CompletedModelCall, record_completed_model_call};
    use super::*;

    fn call(
        event_ulid: &str,
        session_id: &str,
        input: u64,
        output: u64,
        cached: u64,
        reasoning: u64,
    ) -> CompletedModelCall {
        CompletedModelCall {
            event_ulid: event_ulid.to_string(),
            session_id: session_id.to_string(),
            work_ulid: None,
            turn_type: "agent_turn".to_string(),
            agent_kind: "l3".to_string(),
            input_tokens: input,
            output_tokens: output,
            cached_tokens: cached,
            reasoning_tokens: reasoning,
        }
    }

    #[test]
    fn adapter_live_cost_calls_b_and_does_not_add_cached_or_reasoning() {
        let session = "token-report-adapter-sess-001";
        record_completed_model_call(
            LiveTokenLedger::process(),
            &call("01TOKENREPORTADAPTER00000001", session, 100, 50, 20, 10),
        );
        let src = ShellTokenReportLive;
        assert_eq!(src.live_cost(&[session]), Some(150));
        assert_eq!(src.live_slot(session, TokenSlot::Input as usize), 100);
        assert_eq!(src.live_slot(session, TokenSlot::Output as usize), 50);
        assert_eq!(src.live_slot(session, TokenSlot::Cached as usize), 20);
        assert_eq!(src.live_slot(session, TokenSlot::Reasoning as usize), 10);
        let (input, output, cached, reasoning) = src.live_totals_for_session(session);
        assert_eq!((input, output, cached, reasoning), (100, 50, 20, 10));
        let rollup = src.live_rollup(&[session, session]);
        assert_eq!(rollup, (100, 50, 20, 10));
    }
}
