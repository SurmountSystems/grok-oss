//! Lock-free live token slots and per-job estimate versus actual.
//!
//! Slot order is [`TokenSlot`]: Input=0, Output=1, Cached=2, Reasoning=3.
//! Live cost is input plus output. Cached is already inside input. Reasoning
//! is already inside output. Durable job cost calls
//! [`super::ledger::model_call_cost_for_sessions`] (ledger.rs:656; addition
//! 676-679). Overlay context (`sum_shown_counts_once` at
//! `l2_token_tracking.rs`:421, `fetch_max` at :290) is not this vec.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chrono::Utc;

use super::ledger::{
    LocalUsageEvent, ModelCallCost, insert_local_usage_event, local_usage_events_for_session,
    model_call_cost_for_sessions,
};
use crate::grok_oss::GrokOssStore;

/// Four live slots. L2-C must not guess these indices.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSlot {
    Input = 0,
    Output = 1,
    Cached = 2,
    Reasoning = 3,
}

/// Length of the per-session AtomicU64 vec. Slot order is [`TokenSlot`].
pub const TOKEN_SLOT_COUNT: usize = 4;

struct SessionLive {
    slots: [AtomicU64; TOKEN_SLOT_COUNT],
    complete_calls: AtomicU64,
}

impl SessionLive {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            slots: std::array::from_fn(|_| AtomicU64::new(0)),
            complete_calls: AtomicU64::new(0),
        })
    }
}

/// Match on [`TokenSlot`] so slot access cannot panic. Order stays Input=0,
/// Output=1, Cached=2, Reasoning=3.
fn slot_atomic(slots: &[AtomicU64; TOKEN_SLOT_COUNT], slot: TokenSlot) -> &AtomicU64 {
    let [input, output, cached, reasoning] = slots;
    match slot {
        TokenSlot::Input => input,
        TokenSlot::Output => output,
        TokenSlot::Cached => cached,
        TokenSlot::Reasoning => reasoning,
    }
}

/// Per-session lock-free four-slot vec. Map/ULID once-gates use a mutex;
/// slot updates are `fetch_add`.
pub struct LiveTokenLedger {
    sessions: Mutex<HashMap<String, Arc<SessionLive>>>,
    seen_event_ulids: Mutex<HashSet<String>>,
}

impl Default for LiveTokenLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveTokenLedger {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            seen_event_ulids: Mutex::new(HashSet::new()),
        }
    }

    /// Process-wide ledger. `record_model_call` writes here so L1/L2/L3,
    /// retries, and tool-loop turns share one vec.
    pub fn process() -> &'static Self {
        static LEDGER: OnceLock<LiveTokenLedger> = OnceLock::new();
        LEDGER.get_or_init(Self::new)
    }

    fn session_live(&self, session_id: &str) -> Arc<SessionLive> {
        let mut map = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.entry(session_id.to_string())
            .or_insert_with(SessionLive::new)
            .clone()
    }

    fn get_session_live(&self, session_id: &str) -> Option<Arc<SessionLive>> {
        let map = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.get(session_id).cloned()
    }
}

/// One completed sampler model call. `turn_type` `main` and `agent_turn` only
/// for cost. A `nested` completion-notice is not cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedModelCall {
    pub event_ulid: String,
    pub session_id: String,
    pub work_ulid: Option<String>,
    pub turn_type: String,
    pub agent_kind: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub reasoning_tokens: u64,
}

/// Per-session or rollup slot totals. `total_tokens` is not a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LiveSlotTotals {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
    pub reasoning: u64,
}

impl LiveSlotTotals {
    /// Same addition as ledger.rs:676-679. Never cached. Never reasoning.
    pub fn input_plus_output(self) -> u64 {
        self.input.saturating_add(self.output)
    }
}

/// Estimates at persist. Actuals are filled by [`finish_job_cost`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobCostRecord {
    pub job_ulid: String,
    pub job_kind: String,
    pub session_ids: Vec<String>,
    pub estimate_input: Option<i64>,
    pub estimate_output: Option<i64>,
    pub estimate_wall_ms: Option<i64>,
    pub estimate_nested_tokens: Option<i64>,
}

/// Estimate versus actual grouped by `job_kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobKindActual {
    pub job_kind: String,
    pub jobs: u64,
    pub estimate_input_sum: Option<i64>,
    pub estimate_output_sum: Option<i64>,
    pub actual_input_plus_output: Option<i64>,
    pub missing_event_ulids: Vec<String>,
}

fn is_cost_turn(turn_type: &str) -> bool {
    turn_type == "main" || turn_type == "agent_turn"
}

/// `fetch_add` Input, Output, Cached, Reasoning once for this `event_ulid`.
/// Ignores a duplicate ULID. Does not add cached onto input. Does not add
/// reasoning onto output. `nested` notices are not cost.
pub fn record_completed_model_call(ledger: &LiveTokenLedger, call: &CompletedModelCall) {
    if call.event_ulid.is_empty() || call.session_id.is_empty() {
        return;
    }
    {
        let mut seen = ledger
            .seen_event_ulids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !seen.insert(call.event_ulid.clone()) {
            return;
        }
    }
    if !is_cost_turn(&call.turn_type) {
        return;
    }
    let live = ledger.session_live(&call.session_id);
    slot_atomic(&live.slots, TokenSlot::Input).fetch_add(call.input_tokens, Ordering::Relaxed);
    slot_atomic(&live.slots, TokenSlot::Output).fetch_add(call.output_tokens, Ordering::Relaxed);
    slot_atomic(&live.slots, TokenSlot::Cached).fetch_add(call.cached_tokens, Ordering::Relaxed);
    slot_atomic(&live.slots, TokenSlot::Reasoning)
        .fetch_add(call.reasoning_tokens, Ordering::Relaxed);
    live.complete_calls.fetch_add(1, Ordering::Relaxed);
}

pub fn live_slot(ledger: &LiveTokenLedger, session_id: &str, slot: TokenSlot) -> u64 {
    ledger
        .get_session_live(session_id)
        .map(|live| slot_atomic(&live.slots, slot).load(Ordering::Relaxed))
        .unwrap_or(0)
}

pub fn live_totals_for_session(ledger: &LiveTokenLedger, session_id: &str) -> LiveSlotTotals {
    match ledger.get_session_live(session_id) {
        Some(live) => LiveSlotTotals {
            input: slot_atomic(&live.slots, TokenSlot::Input).load(Ordering::Relaxed),
            output: slot_atomic(&live.slots, TokenSlot::Output).load(Ordering::Relaxed),
            cached: slot_atomic(&live.slots, TokenSlot::Cached).load(Ordering::Relaxed),
            reasoning: slot_atomic(&live.slots, TokenSlot::Reasoning).load(Ordering::Relaxed),
        },
        None => LiveSlotTotals::default(),
    }
}

/// Sum of named leaves. Duplicate session ids are ignored.
pub fn live_rollup(ledger: &LiveTokenLedger, session_ids: &[&str]) -> LiveSlotTotals {
    let mut seen = HashSet::new();
    let mut totals = LiveSlotTotals::default();
    for session_id in session_ids {
        if !seen.insert(*session_id) {
            continue;
        }
        let one = live_totals_for_session(ledger, session_id);
        totals.input = totals.input.saturating_add(one.input);
        totals.output = totals.output.saturating_add(one.output);
        totals.cached = totals.cached.saturating_add(one.cached);
        totals.reasoning = totals.reasoning.saturating_add(one.reasoning);
    }
    totals
}

/// Rollup input plus output, or `None` when no recorded complete call.
/// Never a stand-in 0. Never cached. Never reasoning.
pub fn live_cost(ledger: &LiveTokenLedger, session_ids: &[&str]) -> Option<u64> {
    let mut seen = HashSet::new();
    let mut complete = 0u64;
    for session_id in session_ids {
        if !seen.insert(*session_id) {
            continue;
        }
        if let Some(live) = ledger.get_session_live(session_id) {
            complete = complete.saturating_add(live.complete_calls.load(Ordering::Relaxed));
        }
    }
    if complete == 0 {
        return None;
    }
    Some(live_rollup(ledger, session_ids).input_plus_output())
}

/// Must call [`insert_local_usage_event`] (ledger.rs:249). Table `local_usage_event`.
pub fn persist_model_call(
    store: &GrokOssStore,
    event: &LocalUsageEvent,
) -> Result<bool, rusqlite::Error> {
    insert_local_usage_event(store, event)
}

fn json_err(err: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(err))
}

/// Inserts `job_cost` with estimates. Does not invent actuals.
pub fn persist_job_cost(store: &GrokOssStore, job: &JobCostRecord) -> Result<(), rusqlite::Error> {
    let session_ids_json = serde_json::to_string(&job.session_ids).map_err(json_err)?;
    let started_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    store.connection().execute(
        r#"
INSERT INTO job_cost (
  job_ulid, job_kind, session_ids_json,
  estimate_input, estimate_output, estimate_wall_ms, estimate_nested_tokens,
  started_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT(job_ulid) DO UPDATE SET
  job_kind = excluded.job_kind,
  session_ids_json = excluded.session_ids_json,
  estimate_input = excluded.estimate_input,
  estimate_output = excluded.estimate_output,
  estimate_wall_ms = excluded.estimate_wall_ms,
  estimate_nested_tokens = excluded.estimate_nested_tokens
"#,
        rusqlite::params![
            job.job_ulid,
            job.job_kind,
            session_ids_json,
            job.estimate_input,
            job.estimate_output,
            job.estimate_wall_ms,
            job.estimate_nested_tokens,
            started_at,
        ],
    )?;
    Ok(())
}

/// Must call [`model_call_cost_for_sessions`] with the job's stored session ids.
/// Stores `actual_input_plus_output` from that result. Does not compute a
/// parallel cost sum.
pub fn finish_job_cost(
    store: &GrokOssStore,
    job_ulid: &str,
) -> Result<ModelCallCost, rusqlite::Error> {
    let session_ids_json: String = store.connection().query_row(
        "SELECT session_ids_json FROM job_cost WHERE job_ulid = ?1",
        [job_ulid],
        |row| row.get(0),
    )?;
    let session_ids: Vec<String> = serde_json::from_str(&session_ids_json).map_err(json_err)?;
    let session_refs: Vec<&str> = session_ids.iter().map(String::as_str).collect();
    // Cost is this function, not a second addition (ledger.rs:656, 676-679).
    let cost = model_call_cost_for_sessions(store, &session_refs)?;
    let (actual_input, actual_output, actual_cached, actual_reasoning) =
        display_slot_sums_for_sessions(store, &session_refs)?;
    let missing_json = serde_json::to_string(&cost.missing_event_ulids).map_err(json_err)?;
    let finished_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    store.connection().execute(
        r#"
UPDATE job_cost SET
  actual_input_plus_output = ?1,
  actual_input = ?2,
  actual_output = ?3,
  actual_cached = ?4,
  actual_reasoning = ?5,
  missing_event_ulids_json = ?6,
  finished_at = ?7
WHERE job_ulid = ?8
"#,
        rusqlite::params![
            cost.input_plus_output,
            actual_input,
            actual_output,
            actual_cached,
            actual_reasoning,
            missing_json,
            finished_at,
            job_ulid,
        ],
    )?;
    Ok(cost)
}

/// Display-only input, output, cached, reasoning. Not the cost.
type DisplaySlotSums = (Option<i64>, Option<i64>, Option<i64>, Option<i64>);
type DisplaySlotSumsResult = Result<DisplaySlotSums, rusqlite::Error>;

/// Display slot sums for complete `main` / `agent_turn` rows. Not the cost.
fn display_slot_sums_for_sessions(
    store: &GrokOssStore,
    session_ids: &[&str],
) -> DisplaySlotSumsResult {
    let mut seen_sessions = HashSet::new();
    let mut seen_ulids = HashSet::new();
    let mut input: i64 = 0;
    let mut output: i64 = 0;
    let mut cached: i64 = 0;
    let mut reasoning: i64 = 0;
    let mut complete = 0u64;
    for session_id in session_ids {
        if !seen_sessions.insert(*session_id) {
            continue;
        }
        for event in local_usage_events_for_session(store, session_id)? {
            if !seen_ulids.insert(event.event_ulid.clone()) {
                continue;
            }
            if !is_cost_turn(&event.turn_type) {
                continue;
            }
            if let (Some(i), Some(o)) = (event.input_tokens, event.output_tokens) {
                input = input.saturating_add(i);
                output = output.saturating_add(o);
                if let Some(c) = event.cached_tokens {
                    cached = cached.saturating_add(c);
                }
                if let Some(r) = event.reasoning_tokens {
                    reasoning = reasoning.saturating_add(r);
                }
                complete += 1;
            }
        }
    }
    if complete == 0 {
        Ok((None, None, None, None))
    } else {
        Ok((Some(input), Some(output), Some(cached), Some(reasoning)))
    }
}

/// Reads `job_cost` grouped by `job_kind`.
pub fn estimate_vs_actual_by_job_kind(
    store: &GrokOssStore,
    job_kind: &str,
) -> Result<JobKindActual, rusqlite::Error> {
    let mut stmt = store.connection().prepare(
        "SELECT estimate_input, estimate_output, actual_input_plus_output, missing_event_ulids_json
         FROM job_cost WHERE job_kind = ?1",
    )?;
    let rows = stmt.query_map([job_kind], |row| {
        Ok((
            row.get::<_, Option<i64>>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut jobs = 0u64;
    let mut estimate_input_sum: Option<i64> = None;
    let mut estimate_output_sum: Option<i64> = None;
    let mut actual_input_plus_output: Option<i64> = None;
    let mut missing_event_ulids = Vec::new();
    let mut seen_missing = HashSet::new();
    for row in rows {
        let (est_in, est_out, actual, missing_json) = row?;
        jobs += 1;
        if let Some(v) = est_in {
            estimate_input_sum = Some(estimate_input_sum.unwrap_or(0).saturating_add(v));
        }
        if let Some(v) = est_out {
            estimate_output_sum = Some(estimate_output_sum.unwrap_or(0).saturating_add(v));
        }
        if let Some(v) = actual {
            actual_input_plus_output =
                Some(actual_input_plus_output.unwrap_or(0).saturating_add(v));
        }
        if let Some(json) = missing_json {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&json) {
                for ulid in list {
                    if seen_missing.insert(ulid.clone()) {
                        missing_event_ulids.push(ulid);
                    }
                }
            }
        }
    }
    Ok(JobKindActual {
        job_kind: job_kind.to_string(),
        jobs,
        estimate_input_sum,
        estimate_output_sum,
        actual_input_plus_output,
        missing_event_ulids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grok_oss::open_at;
    use tempfile::TempDir;

    fn call(
        event_ulid: &str,
        session_id: &str,
        turn_type: &str,
        agent_kind: &str,
        input: u64,
        output: u64,
        cached: u64,
        reasoning: u64,
    ) -> CompletedModelCall {
        CompletedModelCall {
            event_ulid: event_ulid.to_string(),
            session_id: session_id.to_string(),
            work_ulid: None,
            turn_type: turn_type.to_string(),
            agent_kind: agent_kind.to_string(),
            input_tokens: input,
            output_tokens: output,
            cached_tokens: cached,
            reasoning_tokens: reasoning,
        }
    }

    fn usage_row(
        event_ulid: &str,
        session_id: &str,
        turn_type: &str,
        agent_kind: &str,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        cached_tokens: Option<i64>,
        reasoning_tokens: Option<i64>,
        total_tokens: Option<i64>,
        incomplete: bool,
    ) -> LocalUsageEvent {
        LocalUsageEvent {
            event_ulid: event_ulid.to_string(),
            session_id: session_id.to_string(),
            work_ulid: None,
            timestamp_utc: "2026-10-24T00:00:00.000Z".into(),
            turn_type: turn_type.to_string(),
            agent_kind: agent_kind.to_string(),
            model_id: None,
            input_tokens,
            output_tokens,
            cached_tokens,
            reasoning_tokens,
            total_tokens,
            cost_usd_ticks: None,
            cost_missing: input_tokens.is_none() || output_tokens.is_none(),
            incomplete,
            sampling_identity: None,
        }
    }

    /// Grok OSS: four AtomicU64 slots, this order. This diverges from upstream
    /// xAI because Grok OSS live cost is a lock-free vec L2-C reads by TokenSlot.
    #[test]
    fn lock_free_atomic_u64_slots_are_input_output_cached_reasoning() {
        assert_eq!(TokenSlot::Input as usize, 0);
        assert_eq!(TokenSlot::Output as usize, 1);
        assert_eq!(TokenSlot::Cached as usize, 2);
        assert_eq!(TokenSlot::Reasoning as usize, 3);
        assert_eq!(TOKEN_SLOT_COUNT, 4);
        let ledger = LiveTokenLedger::new();
        record_completed_model_call(
            &ledger,
            &call(
                "01LIVE SLOTORDER00000000001",
                "sess-slots",
                "main",
                "main",
                100,
                40,
                25,
                10,
            ),
        );
        assert_eq!(live_slot(&ledger, "sess-slots", TokenSlot::Input), 100);
        assert_eq!(live_slot(&ledger, "sess-slots", TokenSlot::Output), 40);
        assert_eq!(live_slot(&ledger, "sess-slots", TokenSlot::Cached), 25);
        assert_eq!(live_slot(&ledger, "sess-slots", TokenSlot::Reasoning), 10);
        let totals = live_totals_for_session(&ledger, "sess-slots");
        assert_eq!(totals.input, 100);
        assert_eq!(totals.output, 40);
        assert_eq!(totals.cached, 25);
        assert_eq!(totals.reasoning, 10);
    }

    #[test]
    fn token_slot_indices_are_stable() {
        assert_eq!(TokenSlot::Input as usize, 0);
        assert_eq!(TokenSlot::Output as usize, 1);
        assert_eq!(TokenSlot::Cached as usize, 2);
        assert_eq!(TokenSlot::Reasoning as usize, 3);
        assert_ne!(TokenSlot::Input as usize, TokenSlot::Cached as usize);
        assert_ne!(TokenSlot::Output as usize, TokenSlot::Reasoning as usize);
    }

    /// Each completed call is one event_ulid. L1/L2/L3, a retry, and a tool-loop
    /// turn each count once. Nested notice is not cost.
    #[test]
    fn each_token_counted_once_across_l1_l2_l3_retries_and_tool_calls() {
        let ledger = LiveTokenLedger::new();
        record_completed_model_call(
            &ledger,
            &call(
                "01L1CALL000000000000000001",
                "l1",
                "main",
                "main",
                100,
                10,
                20,
                4,
            ),
        );
        record_completed_model_call(
            &ledger,
            &call(
                "01L1TOOLLOOP00000000000001",
                "l1",
                "main",
                "main",
                150,
                12,
                30,
                5,
            ),
        );
        record_completed_model_call(
            &ledger,
            &call(
                "01L2CALL000000000000000001",
                "l2",
                "agent_turn",
                "l2",
                1_000,
                100,
                200,
                40,
            ),
        );
        record_completed_model_call(
            &ledger,
            &call(
                "01L2RETRY00000000000000001",
                "l2",
                "agent_turn",
                "l2",
                1_000,
                80,
                200,
                30,
            ),
        );
        record_completed_model_call(
            &ledger,
            &call(
                "01L3CALL000000000000000001",
                "l3",
                "agent_turn",
                "l3",
                500,
                50,
                10,
                20,
            ),
        );
        record_completed_model_call(
            &ledger,
            &call(
                "01L3NESTEDNOTICE0000000001",
                "l2",
                "nested",
                "l3",
                0,
                50,
                0,
                20,
            ),
        );
        let cost = live_cost(&ledger, &["l1", "l2", "l3"]).expect("complete calls");
        assert_eq!(cost, 110 + 162 + 1_100 + 1_080 + 550);
        assert_eq!(cost, 3_002);
        assert_ne!(cost, 3_002 + 20 + 30 + 200 + 200 + 10);
        assert_ne!(cost, 3_002 + 4 + 5 + 40 + 30 + 20);
        assert_ne!(cost, 3_002 + 50);
    }

    #[test]
    fn reasoning_already_inside_output_is_not_readded() {
        let ledger = LiveTokenLedger::new();
        record_completed_model_call(
            &ledger,
            &call(
                "01REASONINSIDE000000000001",
                "sess",
                "agent_turn",
                "l2",
                1_000,
                100,
                200,
                40,
            ),
        );
        let cost = live_cost(&ledger, &["sess"]).expect("complete");
        assert_eq!(cost, 1_100);
        assert_ne!(cost, 1_100 + 40);
        assert_ne!(
            live_slot(&ledger, "sess", TokenSlot::Output).saturating_add(live_slot(
                &ledger,
                "sess",
                TokenSlot::Reasoning
            )),
            cost
        );
    }

    #[test]
    fn cached_already_inside_input_is_not_readded() {
        let ledger = LiveTokenLedger::new();
        record_completed_model_call(
            &ledger,
            &call(
                "01CACHEINSIDE0000000000001",
                "sess",
                "agent_turn",
                "l2",
                1_000,
                100,
                200,
                40,
            ),
        );
        let cost = live_cost(&ledger, &["sess"]).expect("complete");
        assert_eq!(cost, 1_100);
        assert_ne!(cost, 1_100 + 200);
        assert_ne!(
            live_slot(&ledger, "sess", TokenSlot::Input).saturating_add(live_slot(
                &ledger,
                "sess",
                TokenSlot::Cached
            )),
            cost
        );
    }

    #[test]
    fn total_tokens_is_not_the_cost() {
        let ledger = LiveTokenLedger::new();
        record_completed_model_call(
            &ledger,
            &call(
                "01TOTALNOTCOST000000000001",
                "sess",
                "agent_turn",
                "general-purpose",
                180_000,
                4_000,
                12_000,
                0,
            ),
        );
        let cost = live_cost(&ledger, &["sess"]).expect("complete");
        assert_eq!(cost, 184_000);
        assert_ne!(cost, 180_000);
        assert_ne!(cost, 4_000);
        assert_ne!(cost, 12_000);
        let totals = live_totals_for_session(&ledger, "sess");
        assert_ne!(
            Some(
                totals
                    .input
                    .saturating_add(totals.output)
                    .saturating_add(totals.cached)
            ),
            Some(cost)
        );
    }

    #[test]
    fn live_cost_is_input_plus_output_only() {
        let ledger = LiveTokenLedger::new();
        record_completed_model_call(
            &ledger,
            &call(
                "01LIVECOSTONLY000000000001",
                "a",
                "main",
                "main",
                10,
                5,
                3,
                2,
            ),
        );
        record_completed_model_call(
            &ledger,
            &call(
                "01LIVECOSTONLY000000000002",
                "b",
                "agent_turn",
                "l3",
                20,
                7,
                4,
                1,
            ),
        );
        let cost = live_cost(&ledger, &["a", "b"]).expect("complete");
        assert_eq!(cost, 10 + 5 + 20 + 7);
        assert_eq!(cost, 42);
        assert_ne!(cost, 42 + 3 + 4);
        assert_ne!(cost, 42 + 2 + 1);
        assert_eq!(live_rollup(&ledger, &["a", "b"]).input_plus_output(), 42);
    }

    #[test]
    fn live_cost_none_when_no_complete_call() {
        let ledger = LiveTokenLedger::new();
        assert_eq!(live_cost(&ledger, &["missing"]), None);
        assert_ne!(live_cost(&ledger, &["missing"]), Some(0));
        record_completed_model_call(
            &ledger,
            &call(
                "01NESTEDONLY00000000000001",
                "nested-sess",
                "nested",
                "l2",
                9,
                9,
                0,
                0,
            ),
        );
        assert_eq!(live_cost(&ledger, &["nested-sess"]), None);
        assert_ne!(live_cost(&ledger, &["nested-sess"]), Some(0));
        assert_ne!(live_cost(&ledger, &["nested-sess"]), Some(18));
    }

    #[test]
    fn nested_turn_type_is_not_live_cost() {
        let ledger = LiveTokenLedger::new();
        record_completed_model_call(
            &ledger,
            &call(
                "01AGENTTURN000000000000001",
                "l2",
                "agent_turn",
                "l2",
                1_000,
                100,
                0,
                0,
            ),
        );
        record_completed_model_call(
            &ledger,
            &call(
                "01NESTEDROW000000000000001",
                "l2",
                "nested",
                "l3",
                0,
                600,
                0,
                20,
            ),
        );
        assert_eq!(live_cost(&ledger, &["l2"]), Some(1_100));
        assert_ne!(live_cost(&ledger, &["l2"]), Some(1_100 + 600));
        assert_eq!(live_slot(&ledger, "l2", TokenSlot::Output), 100);
    }

    type RollupLeaf = (&'static str, u64, u64, u64, u64);
    type RollupCases = &'static [&'static [RollupLeaf]];

    /// Property: rollup of named leaves equals the saturating sum of each
    /// leaf's four slots. Duplicate session ids are ignored.
    #[test]
    fn live_rollup_equals_sum_of_leaves() {
        let cases: RollupCases = &[
            &[],
            &[("a", 1, 2, 0, 0)],
            &[("l1", 10, 5, 3, 2), ("l2", 20, 7, 1, 4), ("l3", 8, 1, 0, 1)],
            &[("s1", 0, 1, 0, 0), ("s2", 1_000_000, 2, 50, 3)],
            &[("only", 7, 0, 7, 0)],
        ];
        for (i, leaves) in cases.iter().enumerate() {
            let ledger = LiveTokenLedger::new();
            let mut ids = Vec::new();
            let mut expect = LiveSlotTotals::default();
            for (n, (id, input, output, cached, reasoning)) in leaves.iter().enumerate() {
                record_completed_model_call(
                    &ledger,
                    &call(
                        &format!("01ROLLUP{i:02}LEAF{n:02}0000000001"),
                        id,
                        "agent_turn",
                        "l3",
                        *input,
                        *output,
                        *cached,
                        *reasoning,
                    ),
                );
                ids.push(*id);
                expect.input = expect.input.saturating_add(*input);
                expect.output = expect.output.saturating_add(*output);
                expect.cached = expect.cached.saturating_add(*cached);
                expect.reasoning = expect.reasoning.saturating_add(*reasoning);
            }
            let refs: Vec<&str> = ids.clone();
            assert_eq!(live_rollup(&ledger, &refs), expect, "case {i}");
            if !ids.is_empty() {
                let mut dup = refs.clone();
                dup.push(ids[0]);
                assert_eq!(live_rollup(&ledger, &dup), expect, "case {i} duplicate id");
                let cost = live_cost(&ledger, &refs).expect("complete");
                assert_eq!(cost, expect.input_plus_output(), "case {i} cost");
            } else {
                assert_eq!(live_cost(&ledger, &[]), None);
            }
        }
    }

    #[test]
    fn duplicate_event_ulid_does_not_fetch_add_twice() {
        let ledger = LiveTokenLedger::new();
        let once = call(
            "01DUPULIDONCE0000000000001",
            "sess",
            "main",
            "main",
            50,
            7,
            4,
            2,
        );
        record_completed_model_call(&ledger, &once);
        record_completed_model_call(&ledger, &once);
        record_completed_model_call(&ledger, &once);
        assert_eq!(live_slot(&ledger, "sess", TokenSlot::Input), 50);
        assert_eq!(live_slot(&ledger, "sess", TokenSlot::Output), 7);
        assert_eq!(live_slot(&ledger, "sess", TokenSlot::Cached), 4);
        assert_eq!(live_slot(&ledger, "sess", TokenSlot::Reasoning), 2);
        assert_eq!(live_cost(&ledger, &["sess"]), Some(57));
        assert_ne!(live_cost(&ledger, &["sess"]), Some(171));
    }

    /// Property: concurrent record of the same event_ulid fetch_adds once.
    #[test]
    fn duplicate_event_ulid_does_not_fetch_add_twice_under_concurrency() {
        let ledger = Arc::new(LiveTokenLedger::new());
        let once = call(
            "01CONCURULID00000000000001",
            "concur-sess",
            "agent_turn",
            "l2",
            100,
            20,
            8,
            5,
        );
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let ledger = Arc::clone(&ledger);
                let once = once.clone();
                scope.spawn(move || {
                    for _ in 0..50 {
                        record_completed_model_call(&ledger, &once);
                    }
                });
            }
        });
        assert_eq!(live_slot(&ledger, "concur-sess", TokenSlot::Input), 100);
        assert_eq!(live_slot(&ledger, "concur-sess", TokenSlot::Output), 20);
        assert_eq!(live_slot(&ledger, "concur-sess", TokenSlot::Cached), 8);
        assert_eq!(live_slot(&ledger, "concur-sess", TokenSlot::Reasoning), 5);
        assert_eq!(live_cost(&ledger, &["concur-sess"]), Some(120));
        assert_ne!(live_cost(&ledger, &["concur-sess"]), Some(120 * 8 * 50));
    }

    #[test]
    fn persist_model_call_is_insert_local_usage_event() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let event = usage_row(
            "01PERSISTCALL0000000000001",
            "sess-persist",
            "main",
            "main",
            Some(11),
            Some(3),
            Some(2),
            Some(1),
            Some(14),
            false,
        );
        assert!(persist_model_call(&store, &event).unwrap());
        assert!(!persist_model_call(&store, &event).unwrap());
        let rows = local_usage_events_for_session(&store, "sess-persist").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_ulid, "01PERSISTCALL0000000000001");
        assert_eq!(rows[0].input_tokens, Some(11));
        assert_eq!(rows[0].output_tokens, Some(3));
    }

    #[test]
    fn sqlite_migration_stores_per_call_and_per_job_costs() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        assert_eq!(
            store.schema_version().unwrap(),
            crate::grok_oss::SCHEMA_VERSION
        );
        let n: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='job_cost'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "schema v8 must create job_cost");
        let idx: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_job_cost_kind'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1, "idx_job_cost_kind");
        let event = usage_row(
            "01JOBCALL00000000000000001",
            "job-sess",
            "agent_turn",
            "l2",
            Some(1_000),
            Some(100),
            Some(200),
            Some(40),
            Some(1_200),
            false,
        );
        assert!(persist_model_call(&store, &event).unwrap());
        persist_job_cost(
            &store,
            &JobCostRecord {
                job_ulid: "01JOBULID00000000000000001".into(),
                job_kind: "implement".into(),
                session_ids: vec!["job-sess".into()],
                estimate_input: Some(900),
                estimate_output: Some(80),
                estimate_wall_ms: Some(1_200),
                estimate_nested_tokens: Some(1_000),
            },
        )
        .unwrap();
        let actual_before: Option<i64> = store
            .connection()
            .query_row(
                "SELECT actual_input_plus_output FROM job_cost WHERE job_ulid = ?1",
                ["01JOBULID00000000000000001"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual_before, None, "persist must not invent actuals");
        let finished = finish_job_cost(&store, "01JOBULID00000000000000001").unwrap();
        assert_eq!(finished.input_plus_output, Some(1_100));
        let stored: Option<i64> = store
            .connection()
            .query_row(
                "SELECT actual_input_plus_output FROM job_cost WHERE job_ulid = ?1",
                ["01JOBULID00000000000000001"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, Some(1_100));
        assert_ne!(stored, Some(1_200));
        assert_ne!(stored, Some(1_100 + 200));
        assert_ne!(stored, Some(1_100 + 40));
    }

    /// Prove finish_job_cost calls model_call_cost_for_sessions by matching
    /// that function on the stored session ids. Same L2+L3 numbers as
    /// l3_model_call_tokens_contribute_to_l2_cost (do not rewrite that test).
    #[test]
    fn finish_job_cost_calls_model_call_cost_for_sessions() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let l2 = usage_row(
            "01L2CALLCOST00000000000002",
            "l2-sess",
            "agent_turn",
            "l2",
            Some(1_000),
            Some(100),
            Some(200),
            Some(40),
            Some(1_200),
            false,
        );
        let l3 = usage_row(
            "01L3CALLCOST00000000000002",
            "l3-sess",
            "agent_turn",
            "l3",
            Some(500),
            Some(50),
            Some(10),
            Some(20),
            Some(600),
            false,
        );
        let nested_notice = usage_row(
            "01L3NESTEDNOTICE0000000002",
            "l2-sess",
            "nested",
            "l3",
            None,
            Some(50),
            None,
            Some(20),
            Some(600),
            false,
        );
        assert!(persist_model_call(&store, &l2).unwrap());
        assert!(persist_model_call(&store, &l3).unwrap());
        assert!(persist_model_call(&store, &nested_notice).unwrap());
        persist_job_cost(
            &store,
            &JobCostRecord {
                job_ulid: "01FINISHJOB000000000000001".into(),
                job_kind: "review".into(),
                session_ids: vec!["l2-sess".into(), "l3-sess".into(), "l2-sess".into()],
                estimate_input: Some(1_400),
                estimate_output: Some(140),
                estimate_wall_ms: Some(600_000),
                estimate_nested_tokens: Some(1_650),
            },
        )
        .unwrap();
        let finished = finish_job_cost(&store, "01FINISHJOB000000000000001").unwrap();
        let expected =
            model_call_cost_for_sessions(&store, &["l2-sess", "l3-sess", "l2-sess"]).unwrap();
        assert_eq!(
            finished, expected,
            "finish_job_cost must return model_call_cost_for_sessions on stored session ids"
        );
        assert_eq!(finished.input_plus_output, Some(1_650));
        assert_ne!(finished.input_plus_output, Some(1_100));
        assert_ne!(finished.input_plus_output, Some(1_100 + 600));
        assert_ne!(finished.input_plus_output, Some(1_650 + 200 + 10));
        assert_ne!(finished.input_plus_output, Some(1_650 + 40 + 20));
        assert_ne!(finished.input_plus_output, Some(0));
    }

    #[test]
    fn estimate_vs_actual_by_job_kind_reads_back() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let row = usage_row(
            "01ESTACTUAL000000000000001",
            "kind-sess",
            "main",
            "main",
            Some(80),
            Some(20),
            Some(10),
            Some(5),
            Some(100),
            false,
        );
        assert!(persist_model_call(&store, &row).unwrap());
        persist_job_cost(
            &store,
            &JobCostRecord {
                job_ulid: "01KINDJOBA000000000000001".into(),
                job_kind: "explore".into(),
                session_ids: vec!["kind-sess".into()],
                estimate_input: Some(70),
                estimate_output: Some(15),
                estimate_wall_ms: Some(100),
                estimate_nested_tokens: Some(90),
            },
        )
        .unwrap();
        persist_job_cost(
            &store,
            &JobCostRecord {
                job_ulid: "01KINDJOBB000000000000001".into(),
                job_kind: "explore".into(),
                session_ids: vec!["kind-sess".into()],
                estimate_input: Some(90),
                estimate_output: Some(25),
                estimate_wall_ms: Some(200),
                estimate_nested_tokens: Some(110),
            },
        )
        .unwrap();
        finish_job_cost(&store, "01KINDJOBA000000000000001").unwrap();
        let got = estimate_vs_actual_by_job_kind(&store, "explore").unwrap();
        assert_eq!(got.job_kind, "explore");
        assert_eq!(got.jobs, 2);
        assert_eq!(got.estimate_input_sum, Some(160));
        assert_eq!(got.estimate_output_sum, Some(40));
        assert_eq!(got.actual_input_plus_output, Some(100));
        assert_ne!(got.actual_input_plus_output, Some(100 + 10));
        assert_ne!(got.actual_input_plus_output, Some(100 + 5));
        let empty = estimate_vs_actual_by_job_kind(&store, "missing-kind").unwrap();
        assert_eq!(empty.jobs, 0);
        assert_eq!(empty.actual_input_plus_output, None);
        assert_ne!(empty.actual_input_plus_output, Some(0));
    }

    #[test]
    fn job_cost_table_has_no_secret_columns() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let mut stmt = store
            .connection()
            .prepare("PRAGMA table_info(job_cost)")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(!cols.is_empty());
        for col in &cols {
            let lower = col.to_ascii_lowercase();
            for banned in [
                "api_key",
                "jwt",
                "password",
                "bearer",
                "management_key",
                "secret",
            ] {
                assert!(
                    !lower.contains(banned),
                    "job_cost column {col} must not store secrets ({banned})"
                );
            }
        }
        assert!(cols.iter().any(|c| c == "session_ids_json"));
        assert!(cols.iter().any(|c| c == "actual_input_plus_output"));
        assert!(cols.iter().any(|c| c == "job_kind"));
    }
}
