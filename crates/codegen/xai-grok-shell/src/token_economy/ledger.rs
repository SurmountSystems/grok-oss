//! Local + remote Token Economy books inside `grok_oss.db`.
//!
//! Local: idempotent ingest from session `usage.jsonl`.
//! Remote: Management / SuperGrok samples as JSON payloads (no secrets).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::grok_oss::GrokOssStore;

/// One row as stored / aggregated for the local book.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalUsageEvent {
    pub event_ulid: String,
    pub session_id: String,
    pub work_ulid: Option<String>,
    pub timestamp_utc: String,
    pub turn_type: String,
    pub agent_kind: String,
    pub model_id: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cost_usd_ticks: Option<i64>,
    pub cost_missing: bool,
    pub incomplete: bool,
    pub sampling_identity: Option<String>,
}

/// Wire-compatible subset of `usage.jsonl` schema v1 for ingest.
#[derive(Debug, Clone, Deserialize)]
struct UsageJsonlRow {
    event_ulid: String,
    #[serde(default)]
    work_ulid: Option<String>,
    timestamp: String,
    turn_type: String,
    agent_kind: String,
    session_id: String,
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cached_tokens: Option<u64>,
    #[serde(default)]
    reasoning_tokens: Option<u64>,
    #[serde(default)]
    total_tokens: Option<u64>,
    #[serde(default)]
    cost_usd_ticks: Option<i64>,
    #[serde(default)]
    cost_missing: bool,
    #[serde(default)]
    incomplete: bool,
}

impl From<UsageJsonlRow> for LocalUsageEvent {
    fn from(r: UsageJsonlRow) -> Self {
        Self {
            event_ulid: r.event_ulid,
            session_id: r.session_id,
            work_ulid: r.work_ulid,
            timestamp_utc: r.timestamp,
            turn_type: r.turn_type,
            agent_kind: r.agent_kind,
            model_id: r.model_id,
            input_tokens: r.input_tokens.map(|v| v as i64),
            output_tokens: r.output_tokens.map(|v| v as i64),
            cached_tokens: r.cached_tokens.map(|v| v as i64),
            reasoning_tokens: r.reasoning_tokens.map(|v| v as i64),
            total_tokens: r.total_tokens.map(|v| v as i64),
            cost_usd_ticks: r.cost_usd_ticks,
            cost_missing: r.cost_missing,
            incomplete: r.incomplete,
            sampling_identity: None,
        }
    }
}

/// How many rows were inserted vs already present on an ingest pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestStats {
    pub inserted: u64,
    pub skipped_duplicate: u64,
    pub parse_errors: u64,
}

/// Aggregate local book for a time window (inclusive/exclusive as filtered).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalBookSummary {
    pub events: u64,
    pub cost_missing_events: u64,
    /// Sum of known `cost_usd_ticks` only (None side never invented).
    pub cost_usd_ticks_sum: Option<i64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
}

/// One remote meter sample row (`remote_meter_sample`), no secrets in payload.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteMeterSample {
    pub sampled_at: String,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub payload: JsonValue,
}

impl LocalBookSummary {
    pub fn has_any_cost(&self) -> bool {
        self.cost_usd_ticks_sum.is_some()
    }
}

/// True when this ULID would record nested L2/L3 spend that is already stored.
///
/// One grok-oss ULID row per nested work. An L3 whose tokens are already inside
/// the L2 total for that work is not a second row. A later ulid that sums those
/// tokens again is not stored. Same `event_ulid` stays `INSERT OR IGNORE`.
/// L3 stored first is replaced by the covering L2 rollup, not skipped here.
fn nested_spend_already_recorded(
    store: &GrokOssStore,
    event: &LocalUsageEvent,
) -> Result<bool, rusqlite::Error> {
    let (Some(total), Some(work)) = (event.total_tokens, event.work_ulid.as_deref()) else {
        return Ok(false);
    };
    let existing: Option<i64> = store.connection().query_row(
        "SELECT MAX(total_tokens) FROM local_usage_event
         WHERE session_id = ?1 AND work_ulid = ?2
           AND lower(agent_kind) IN ('l2', 'nested_l2')",
        rusqlite::params![event.session_id, work],
        |row| row.get(0),
    )?;
    let kind = event.agent_kind.to_ascii_lowercase();
    match kind.as_str() {
        "l3" | "nested_l3" => Ok(existing.is_some_and(|parent| parent >= total)),
        "l2" | "nested_l2" => Ok(existing.is_some()),
        _ => Ok(false),
    }
}

/// L3-then-L2: drop L3 rows when this L2 total already includes their tokens.
fn drop_l3_rows_covered_by_arriving_l2(
    store: &GrokOssStore,
    event: &LocalUsageEvent,
) -> Result<(), rusqlite::Error> {
    let kind = event.agent_kind.to_ascii_lowercase();
    if kind != "l2" && kind != "nested_l2" {
        return Ok(());
    }
    let (Some(total), Some(work)) = (event.total_tokens, event.work_ulid.as_deref()) else {
        return Ok(());
    };
    let l3_sum: Option<i64> = store.connection().query_row(
        "SELECT SUM(total_tokens) FROM local_usage_event
         WHERE session_id = ?1 AND work_ulid = ?2
           AND lower(agent_kind) IN ('l3', 'nested_l3')",
        rusqlite::params![event.session_id, work],
        |row| row.get(0),
    )?;
    let Some(covered) = l3_sum else {
        return Ok(());
    };
    if total < covered {
        return Ok(());
    }
    store.connection().execute(
        "DELETE FROM local_usage_event
         WHERE session_id = ?1 AND work_ulid = ?2
           AND lower(agent_kind) IN ('l3', 'nested_l3')",
        rusqlite::params![event.session_id, work],
    )?;
    Ok(())
}

/// True when this `event_ulid` is already a `local_usage_event` primary key.
fn event_ulid_exists(store: &GrokOssStore, event_ulid: &str) -> Result<bool, rusqlite::Error> {
    let n: i64 = store.connection().query_row(
        "SELECT COUNT(*) FROM local_usage_event WHERE event_ulid = ?1",
        [event_ulid],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

/// Same nested L2 ulid: raise `total_tokens` in place. Not a second row.
///
/// A lower sample (compact) does not shrink the stored total. A different
/// ulid is not updated here, so a third summed row stays rejected.
fn raise_nested_l2_total_if_higher(
    store: &GrokOssStore,
    event: &LocalUsageEvent,
) -> Result<(), rusqlite::Error> {
    let kind = event.agent_kind.to_ascii_lowercase();
    if kind != "l2" && kind != "nested_l2" {
        return Ok(());
    }
    let Some(total) = event.total_tokens else {
        return Ok(());
    };
    if total <= 0 {
        return Ok(());
    }
    store.connection().execute(
        "UPDATE local_usage_event
         SET total_tokens = ?1
         WHERE event_ulid = ?2
           AND lower(agent_kind) IN ('l2', 'nested_l2')
           AND COALESCE(total_tokens, -1) < ?1",
        rusqlite::params![total, event.event_ulid],
    )?;
    Ok(())
}

/// Finish write for one nested row. Sets output and reasoning only.
/// Does not insert a row and does not change `total_tokens`.
pub fn set_nested_l2_finish_output(
    store: &GrokOssStore,
    event_ulid: &str,
    output_tokens: i64,
    reasoning_tokens: i64,
) -> Result<usize, rusqlite::Error> {
    store.connection().execute(
        "UPDATE local_usage_event
         SET output_tokens = ?1, reasoning_tokens = ?2
         WHERE event_ulid = ?3
           AND lower(agent_kind) IN ('l2', 'nested_l2')",
        rusqlite::params![output_tokens, reasoning_tokens, event_ulid],
    )
}

/// Insert one local event. Idempotent on `event_ulid`. Fail-open: errors returned.
///
/// One nested row. The same L2 `event_ulid` refreshes its total upward and
/// does not insert again. A zero L2 total is not stored. A different L2 ulid
/// for the same work is not stored. An L3 already covered by the L2 total
/// is not stored.
pub fn insert_local_usage_event(
    store: &GrokOssStore,
    event: &LocalUsageEvent,
) -> Result<bool, rusqlite::Error> {
    let kind = event.agent_kind.to_ascii_lowercase();
    if (kind == "l2" || kind == "nested_l2") && event.total_tokens.unwrap_or(0) <= 0 {
        return Ok(false);
    }
    if event_ulid_exists(store, &event.event_ulid)? {
        raise_nested_l2_total_if_higher(store, event)?;
        return Ok(false);
    }
    if nested_spend_already_recorded(store, event)? {
        return Ok(false);
    }
    drop_l3_rows_covered_by_arriving_l2(store, event)?;
    let ingested_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let changed = store.connection().execute(
        r#"
INSERT OR IGNORE INTO local_usage_event (
  event_ulid, session_id, work_ulid, timestamp_utc, turn_type, agent_kind,
  model_id, input_tokens, output_tokens, cached_tokens, reasoning_tokens,
  total_tokens, cost_usd_ticks, cost_missing, incomplete, sampling_identity, ingested_at
) VALUES (
  ?1, ?2, ?3, ?4, ?5, ?6,
  ?7, ?8, ?9, ?10, ?11,
  ?12, ?13, ?14, ?15, ?16, ?17
)
"#,
        rusqlite::params![
            event.event_ulid,
            event.session_id,
            event.work_ulid,
            event.timestamp_utc,
            event.turn_type,
            event.agent_kind,
            event.model_id,
            event.input_tokens,
            event.output_tokens,
            event.cached_tokens,
            event.reasoning_tokens,
            event.total_tokens,
            event.cost_usd_ticks,
            event.cost_missing as i64,
            event.incomplete as i64,
            event.sampling_identity,
            ingested_at,
        ],
    )?;
    Ok(changed > 0)
}

#[cfg(test)]
mod nested_ulid_spend_tests {
    use super::*;
    use crate::grok_oss::open_at;
    use tempfile::TempDir;

    fn nested_event(ulid: &str, kind: &str, total: i64) -> LocalUsageEvent {
        LocalUsageEvent {
            event_ulid: ulid.to_string(),
            session_id: "sess-nested-once".into(),
            work_ulid: Some("01ARZ3NDEKTSV4RRFFQ69G5FAY".into()),
            timestamp_utc: "2026-09-21T00:00:00.000Z".into(),
            turn_type: "nested".into(),
            agent_kind: kind.into(),
            model_id: None,
            input_tokens: None,
            output_tokens: None,
            cached_tokens: None,
            reasoning_tokens: None,
            total_tokens: Some(total),
            cost_usd_ticks: None,
            cost_missing: true,
            incomplete: false,
            sampling_identity: None,
        }
    }

    /// Operator: ULID session rows. Nested spend is recorded once.
    #[test]
    fn grok_oss_sqlite_ulid_rows_record_nested_l2_and_l3_spend_once() {
        let l2_ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let l3_ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
        let sum_ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
        let late_l3_ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAT";
        let l3_tokens = 85_600;
        let l2_total = 442_200 + l3_tokens;
        assert_eq!(l2_ulid.len(), 26, "ULID session rows");
        assert!(
            !l2_ulid.contains('-')
                && !l3_ulid.contains('-')
                && !sum_ulid.contains('-')
                && !late_l3_ulid.contains('-'),
            "ULID session rows are grok-oss ULIDs, not Grok Build UUIDs"
        );

        let tmp = TempDir::new().expect("temp dir");
        let store = open_at(&tmp.path().join("grok_oss.db")).expect("grok_oss.db");
        assert!(
            insert_local_usage_event(&store, &nested_event(l3_ulid, "l3", l3_tokens)).expect("l3"),
            "ULID session rows"
        );
        assert!(
            insert_local_usage_event(&store, &nested_event(l2_ulid, "l2", l2_total)).expect("l2"),
            "Nested spend is recorded once"
        );
        assert!(
            !insert_local_usage_event(&store, &nested_event(late_l3_ulid, "l3", l3_tokens))
                .expect("l3 after l2"),
            "Nested spend is recorded once"
        );
        assert!(
            !insert_local_usage_event(&store, &nested_event(sum_ulid, "l2", l2_total + l3_tokens))
                .expect("third"),
            "Nested spend is recorded once. must not store a third summed row"
        );
        assert!(
            !insert_local_usage_event(&store, &nested_event(l2_ulid, "l2", l2_total))
                .expect("again"),
            "ULID session rows stay INSERT OR IGNORE on event_ulid"
        );

        let count: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM local_usage_event",
                rusqlite::params![],
                |row| row.get(0),
            )
            .expect("count");
        let sum: i64 = store
            .connection()
            .query_row(
                "SELECT COALESCE(SUM(total_tokens), 0) FROM local_usage_event",
                rusqlite::params![],
                |row| row.get(0),
            )
            .expect("sum");
        assert_eq!(count, 1, "ULID session rows");
        assert_eq!(sum, l2_total, "Nested spend is recorded once");
    }
}

/// Fail-open insert (logs debug on error). Returns whether a new row was written.
pub fn try_insert_local_usage_event(store: &GrokOssStore, event: &LocalUsageEvent) -> bool {
    match insert_local_usage_event(store, event) {
        Ok(b) => b,
        Err(e) => {
            tracing::debug!(error = %e, "local_usage_event insert failed (fail-open)");
            false
        }
    }
}

/// Read `usage.jsonl` lines into events (skips bad lines).
pub fn parse_usage_jsonl(path: &Path) -> (Vec<LocalUsageEvent>, u64) {
    let mut events = Vec::new();
    let mut parse_errors = 0u64;
    let Ok(file) = File::open(path) else {
        return (events, 0);
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            parse_errors += 1;
            continue;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<UsageJsonlRow>(line) {
            Ok(row) => events.push(LocalUsageEvent::from(row)),
            Err(_) => parse_errors += 1,
        }
    }
    (events, parse_errors)
}

/// Ingest one session's `usage.jsonl` into the store. Idempotent.
pub fn ingest_usage_jsonl(store: &GrokOssStore, path: &Path) -> IngestStats {
    let (events, parse_errors) = parse_usage_jsonl(path);
    let mut stats = IngestStats {
        parse_errors,
        ..Default::default()
    };
    for ev in &events {
        match insert_local_usage_event(store, ev) {
            Ok(true) => stats.inserted += 1,
            Ok(false) => stats.skipped_duplicate += 1,
            Err(e) => {
                tracing::debug!(error = %e, "ingest insert failed (fail-open)");
            }
        }
    }
    stats
}

/// Walk `$GROK_HOME/sessions` for `usage.jsonl` files and ingest.
///
/// Fail-open: missing home / sessions dir → empty stats.
pub fn ingest_all_sessions_usage(store: &GrokOssStore, grok_home: &Path) -> IngestStats {
    let sessions = grok_home.join("sessions");
    let mut total = IngestStats::default();
    let Ok(rd) = std::fs::read_dir(&sessions) else {
        return total;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        // sessions/<encoded>/usage.jsonl (one level) or nested
        collect_usage_files(&path, &mut |p| {
            let s = ingest_usage_jsonl(store, p);
            total.inserted += s.inserted;
            total.skipped_duplicate += s.skipped_duplicate;
            total.parse_errors += s.parse_errors;
        });
    }
    total
}

fn collect_usage_files(dir: &Path, f: &mut dyn FnMut(&Path)) {
    if dir.is_file() {
        if dir.file_name().and_then(|n| n.to_str()) == Some("usage.jsonl") {
            f(dir);
        }
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_usage_files(&p, f);
        } else if p.file_name().and_then(|n| n.to_str()) == Some("usage.jsonl") {
            f(&p);
        }
    }
}

/// Summarize local events with optional timestamp window (RFC 3339 string compare
/// works for ISO timestamps with consistent Z format).
pub fn summarize_local_book(
    store: &GrokOssStore,
    window_start: Option<&str>,
    window_end: Option<&str>,
) -> Result<LocalBookSummary, rusqlite::Error> {
    let mut sql = String::from(
        "SELECT COUNT(*),
                SUM(CASE WHEN cost_missing != 0 THEN 1 ELSE 0 END),
                SUM(cost_usd_ticks),
                SUM(COALESCE(input_tokens, 0)),
                SUM(COALESCE(output_tokens, 0)),
                SUM(COALESCE(total_tokens, 0)),
                SUM(CASE WHEN cost_usd_ticks IS NOT NULL THEN 1 ELSE 0 END)
         FROM local_usage_event WHERE 1=1",
    );
    let mut params: Vec<String> = Vec::new();
    if let Some(s) = window_start {
        sql.push_str(" AND timestamp_utc >= ?");
        params.push(s.to_string());
    }
    if let Some(e) = window_end {
        sql.push_str(" AND timestamp_utc < ?");
        params.push(e.to_string());
    }

    let mut stmt = store.connection().prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();
    stmt.query_row(param_refs.as_slice(), |row| {
        let events: i64 = row.get(0)?;
        let missing: i64 = row.get::<_, Option<i64>>(1)?.unwrap_or(0);
        let cost_sum: Option<i64> = row.get(2)?;
        let with_cost: i64 = row.get::<_, Option<i64>>(6)?.unwrap_or(0);
        Ok(LocalBookSummary {
            events: events as u64,
            cost_missing_events: missing as u64,
            cost_usd_ticks_sum: if with_cost > 0 { cost_sum } else { None },
            input_tokens: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            output_tokens: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            total_tokens: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
        })
    })
}

/// Store a remote meter sample. `source` e.g. `management_usage_series`,
/// `management_prepaid`, `supergrok_included`. Payload must not contain secrets.
pub fn insert_remote_meter_sample(
    store: &GrokOssStore,
    source: &str,
    window_start: Option<&str>,
    window_end: Option<&str>,
    payload: &JsonValue,
) -> Result<i64, rusqlite::Error> {
    let sampled_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let payload_json = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    store.connection().execute(
        r#"
INSERT INTO remote_meter_sample (source, sampled_at, window_start, window_end, payload_json)
VALUES (?1, ?2, ?3, ?4, ?5)
"#,
        rusqlite::params![source, sampled_at, window_start, window_end, payload_json],
    )?;
    Ok(store.connection().last_insert_rowid())
}

/// Fail-open remote sample insert.
pub fn try_insert_remote_meter_sample(
    store: &GrokOssStore,
    source: &str,
    window_start: Option<&str>,
    window_end: Option<&str>,
    payload: &JsonValue,
) -> Option<i64> {
    match insert_remote_meter_sample(store, source, window_start, window_end, payload) {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::debug!(error = %e, "remote_meter_sample insert failed (fail-open)");
            None
        }
    }
}

/// Latest remote sample for a source (by id desc).
pub fn latest_remote_sample(
    store: &GrokOssStore,
    source: &str,
) -> Result<Option<RemoteMeterSample>, rusqlite::Error> {
    let mut stmt = store.connection().prepare(
        r#"
SELECT sampled_at, window_start, window_end, payload_json
FROM remote_meter_sample
WHERE source = ?1
ORDER BY id DESC
LIMIT 1
"#,
    )?;
    let mut rows = stmt.query(rusqlite::params![source])?;
    if let Some(row) = rows.next()? {
        let sampled_at: String = row.get(0)?;
        let window_start: Option<String> = row.get(1)?;
        let window_end: Option<String> = row.get(2)?;
        let payload_s: String = row.get(3)?;
        let payload: JsonValue = serde_json::from_str(&payload_s).unwrap_or(JsonValue::Null);
        Ok(Some(RemoteMeterSample {
            sampled_at,
            window_start,
            window_end,
            payload,
        }))
    } else {
        Ok(None)
    }
}

/// Record a reconciliation run for history.
pub fn insert_reconciliation_run(
    store: &GrokOssStore,
    window_start: &str,
    window_end: &str,
    local: &LocalBookSummary,
    remote_api_class_usd_cents: Option<i64>,
    remote_oauth_class_usd_cents: Option<i64>,
    notes: &str,
) -> Result<i64, rusqlite::Error> {
    let ran_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    store.connection().execute(
        r#"
INSERT INTO reconciliation_run (
  ran_at, window_start, window_end, local_cost_usd_ticks, local_events,
  local_cost_missing_events, remote_api_class_usd_cents, remote_oauth_class_usd_cents, notes
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
"#,
        rusqlite::params![
            ran_at,
            window_start,
            window_end,
            local.cost_usd_ticks_sum,
            local.events as i64,
            local.cost_missing_events as i64,
            remote_api_class_usd_cents,
            remote_oauth_class_usd_cents,
            notes,
        ],
    )?;
    Ok(store.connection().last_insert_rowid())
}

/// Sum of `input_tokens` and `output_tokens` on complete model-call rows.
///
/// `None` when no complete model-call row exists. Never a stand-in 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCallCost {
    /// Sum of input_tokens + output_tokens on complete model-call rows.
    /// None when no complete model-call row exists. Never a stand-in 0.
    pub input_plus_output: Option<i64>,
    /// event_ulid values skipped because input_tokens or output_tokens is null.
    pub missing_event_ulids: Vec<String>,
}

/// Job cost for these sessions. Session ids are deduped. Each `event_ulid` counts once.
///
/// Includes `turn_type` `main` and `agent_turn` only. A completion-notice row
/// (`turn_type` `nested`) is left out.
pub fn model_call_cost_for_sessions(
    store: &GrokOssStore,
    session_ids: &[&str],
) -> Result<ModelCallCost, rusqlite::Error> {
    let mut seen_sessions = std::collections::HashSet::new();
    let mut seen_ulids = std::collections::HashSet::new();
    let mut sum: i64 = 0;
    let mut complete: u64 = 0;
    let mut missing_event_ulids = Vec::new();
    for session_id in session_ids {
        if !seen_sessions.insert((*session_id).to_string()) {
            continue;
        }
        for event in local_usage_events_for_session(store, session_id)? {
            if !seen_ulids.insert(event.event_ulid.clone()) {
                continue;
            }
            if event.turn_type != "main" && event.turn_type != "agent_turn" {
                continue;
            }
            match (event.input_tokens, event.output_tokens) {
                (Some(input), Some(output)) => {
                    // Each row is one billed call; this call's input does not contain this call's output.
                    sum = sum.saturating_add(input).saturating_add(output);
                    complete += 1;
                }
                _ => missing_event_ulids.push(event.event_ulid),
            }
        }
    }
    Ok(ModelCallCost {
        input_plus_output: if complete == 0 { None } else { Some(sum) },
        missing_event_ulids,
    })
}

/// Host chrome `0K/0K` after tool work is a broken context reading.
///
/// The cell says the context figure was not stored. It is not `0`, and it
/// does not invent a token count from that chrome. A real input plus output
/// cost is a separate figure. Subagents overlay paint uses
/// [`last_input_tokens_from_usage_jsonl`] instead of this sentence.
pub fn broken_host_context_cell(host_context: &str, did_tool_work: bool) -> Option<&'static str> {
    if !did_tool_work {
        return None;
    }
    if host_chrome_is_zero_context(host_context) {
        Some("the context figure was not stored")
    } else {
        None
    }
}

/// True when host chrome is a zero context window (`0K/0K` or `0/0`).
///
/// Trailing `context+`, `context`, `tokens`, and a percent clause are ignored
/// so `0K/0K tokens (0% context)` and `0K/0K context+` count as zero.
pub fn host_chrome_is_zero_context(host_context: &str) -> bool {
    let mut compact: String = host_context.split_whitespace().collect();
    if let Some(idx) = compact.find('(') {
        compact.truncate(idx);
    }
    let compact = compact
        .trim_end_matches('+')
        .trim_end_matches("context")
        .trim_end_matches("tokens")
        .trim_end_matches('+')
        .trim_end_matches('%');
    compact.eq_ignore_ascii_case("0K/0K") || compact.eq_ignore_ascii_case("0/0")
}

/// Last `usage.jsonl` row `input_tokens` that is greater than zero.
///
/// That integer is the context window (input). Cached tokens are already
/// inside input. Reasoning tokens are already inside output. Do not add
/// either again. `total_tokens` is provider context length, not this figure.
pub fn last_input_tokens_from_usage_jsonl(path: &Path) -> Option<u64> {
    let (events, _) = parse_usage_jsonl(path);
    events.into_iter().rev().find_map(|event| {
        event
            .input_tokens
            .and_then(|n| u64::try_from(n).ok())
            .filter(|n| *n > 0)
    })
}

/// Rows in `local_usage_event` for one `session_id`, ordered by `event_ulid`.
pub fn local_usage_events_for_session(
    store: &GrokOssStore,
    session_id: &str,
) -> Result<Vec<LocalUsageEvent>, rusqlite::Error> {
    let mut stmt = store.connection().prepare(
        "SELECT event_ulid, session_id, work_ulid, timestamp_utc, turn_type, agent_kind,
                model_id, input_tokens, output_tokens, cached_tokens, reasoning_tokens,
                total_tokens, cost_usd_ticks, cost_missing, incomplete, sampling_identity
         FROM local_usage_event
         WHERE session_id = ?1
         ORDER BY event_ulid",
    )?;
    let rows = stmt.query_map([session_id], |row| {
        Ok(LocalUsageEvent {
            event_ulid: row.get(0)?,
            session_id: row.get(1)?,
            work_ulid: row.get(2)?,
            timestamp_utc: row.get(3)?,
            turn_type: row.get(4)?,
            agent_kind: row.get(5)?,
            model_id: row.get(6)?,
            input_tokens: row.get(7)?,
            output_tokens: row.get(8)?,
            cached_tokens: row.get(9)?,
            reasoning_tokens: row.get(10)?,
            total_tokens: row.get(11)?,
            cost_usd_ticks: row.get(12)?,
            cost_missing: row.get::<_, i64>(13)? != 0,
            incomplete: row.get::<_, i64>(14)? != 0,
            sampling_identity: row.get(15)?,
        })
    })?;
    rows.collect()
}

/// Whether `local_usage_event` contains this ulid (spend ingest checks).
pub fn local_usage_event_exists(
    store: &GrokOssStore,
    event_ulid: &str,
) -> Result<bool, rusqlite::Error> {
    let n: i64 = store.connection().query_row(
        "SELECT COUNT(*) FROM local_usage_event WHERE event_ulid = ?1",
        [event_ulid],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

/// How many `reconciliation_run` rows exist.
pub fn count_reconciliation_runs(store: &GrokOssStore) -> Result<i64, rusqlite::Error> {
    store
        .connection()
        .query_row("SELECT COUNT(*) FROM reconciliation_run", [], |row| {
            row.get(0)
        })
}

/// Sessions root under a grok home.
pub fn sessions_dir(grok_home: &Path) -> PathBuf {
    grok_home.join("sessions")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grok_oss::open_at;
    use tempfile::TempDir;

    fn sample_line(ulid: &str, cost: Option<i64>, missing: bool) -> String {
        let cost_field = match cost {
            Some(c) => format!(r#""cost_usd_ticks":{c},"#),
            None => String::new(),
        };
        format!(
            r#"{{"schema_version":1,"event_ulid":"{ulid}","timestamp":"2026-08-03T12:00:00.000Z","turn_type":"main","agent_kind":"main","session_id":"sess-a","input_tokens":10,"output_tokens":2,"total_tokens":12,{cost_field}"cost_missing":{missing},"incomplete":false}}"#
        )
    }

    #[test]
    fn ingest_idempotent_on_event_ulid() {
        let tmp = TempDir::new().unwrap();
        let db = tmp.path().join("grok_oss.db");
        let store = open_at(&db).unwrap();
        let usage = tmp.path().join("usage.jsonl");
        let ulid = "01HZZTESTIDEMPOTENT0000001";
        std::fs::write(
            &usage,
            format!("{}\n", sample_line(ulid, Some(1000), false)),
        )
        .unwrap();
        let s1 = ingest_usage_jsonl(&store, &usage);
        assert_eq!(s1.inserted, 1);
        assert_eq!(s1.skipped_duplicate, 0);
        let s2 = ingest_usage_jsonl(&store, &usage);
        assert_eq!(s2.inserted, 0);
        assert_eq!(s2.skipped_duplicate, 1);
        let summary = summarize_local_book(&store, None, None).unwrap();
        assert_eq!(summary.events, 1);
        assert_eq!(summary.cost_usd_ticks_sum, Some(1000));
    }

    #[test]
    fn cost_missing_preserved() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("g.db")).unwrap();
        let usage = tmp.path().join("usage.jsonl");
        let a = "01HZZCOSTMISSING000000001";
        let b = "01HZZCOSTMISSING000000002";
        std::fs::write(
            &usage,
            format!(
                "{}\n{}\n",
                sample_line(a, None, true),
                sample_line(b, Some(500), false)
            ),
        )
        .unwrap();
        ingest_usage_jsonl(&store, &usage);
        let summary = summarize_local_book(&store, None, None).unwrap();
        assert_eq!(summary.events, 2);
        assert_eq!(summary.cost_missing_events, 1);
        assert_eq!(summary.cost_usd_ticks_sum, Some(500));
    }

    #[test]
    fn remote_sample_round_trip() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("g.db")).unwrap();
        let payload = serde_json::json!({
            "api_class_usd": 1.25,
            "oauth_class_usd": 0.5,
            // must not store keys — test payload is public meter only
        });
        insert_remote_meter_sample(
            &store,
            "management_usage_series",
            Some("2026-07-27T00:00:00Z"),
            Some("2026-08-03T00:00:00Z"),
            &payload,
        )
        .unwrap();
        let got = latest_remote_sample(&store, "management_usage_series")
            .unwrap()
            .expect("sample");
        assert_eq!(got.payload["api_class_usd"], 1.25);
    }

    fn model_call_row(
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
            timestamp_utc: "2026-10-12T00:00:00.000Z".into(),
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

    #[test]
    fn job_cost_is_input_plus_output_not_the_small_output_alone() {
        let parent = model_call_row(
            "01PARENTCALL00000000000001",
            "parent-sess",
            "agent_turn",
            "general-purpose",
            Some(183_190),
            Some(292),
            Some(179_968),
            Some(289),
            Some(183_486),
            false,
        );
        let child = model_call_row(
            "01CHILDCALL000000000000001",
            "child-sess",
            "agent_turn",
            "general-purpose",
            Some(1_000),
            Some(40),
            Some(10),
            Some(30),
            Some(50_000),
            false,
        );
        let nested = model_call_row(
            "01NESTEDL2ROW0000000000001",
            "parent-sess",
            "nested",
            "l2",
            None,
            Some(10_498),
            None,
            Some(4_000),
            Some(181_000),
            false,
        );
        let missing = model_call_row(
            "01MISSINGCALL0000000000001",
            "child-sess",
            "agent_turn",
            "general-purpose",
            None,
            None,
            None,
            None,
            None,
            true,
        );

        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        assert!(insert_local_usage_event(&store, &parent).unwrap());
        assert!(insert_local_usage_event(&store, &child).unwrap());
        assert!(insert_local_usage_event(&store, &nested).unwrap());
        assert!(insert_local_usage_event(&store, &missing).unwrap());

        let cost =
            model_call_cost_for_sessions(&store, &["parent-sess", "child-sess", "parent-sess"])
                .unwrap();
        let got = cost.input_plus_output;
        assert!(
            got == Some(184_522),
            "only-output is not the cost: got {got:?}, expected Some(184522); 332 or 292 is not 184522"
        );
        for forbidden in [
            Some(292_i64),
            Some(332),
            Some(10_498),
            Some(10_498 + 40),
            Some(184_522 + 289 + 30),
            Some(184_522 + 179_968 + 10),
            Some(183_486),
            Some(181_000),
            Some(50_000),
            Some(0),
        ] {
            assert_ne!(got, forbidden, "cost must not be {forbidden:?}");
        }
        assert_eq!(
            cost.missing_event_ulids,
            vec!["01MISSINGCALL0000000000001".to_string()]
        );

        let nested_only = TempDir::new().unwrap();
        let nested_store = open_at(&nested_only.path().join("grok_oss.db")).unwrap();
        assert!(insert_local_usage_event(&nested_store, &nested).unwrap());
        let excluded = model_call_cost_for_sessions(&nested_store, &["parent-sess"]).unwrap();
        assert_eq!(excluded.input_plus_output, None);
        assert_ne!(excluded.input_plus_output, Some(10_498));
        assert_ne!(excluded.input_plus_output, Some(0));
    }

    /// Input and output are disjoint. Reasoning is absent on this row, so it
    /// is not a third addend. Cache reads stay inside input. Provider
    /// `total_tokens` is context length, not the cost. Output alone is not
    /// the cost. Broken host chrome `0K/0K` after tool work is not cost 0.
    #[test]
    fn large_input_180000_and_output_4000_cost_is_184000_not_4000() {
        let row = model_call_row(
            "01LARGEINPUTSMALL000000001",
            "job-sess",
            "agent_turn",
            "general-purpose",
            Some(180_000),
            Some(4_000),
            Some(12_000),
            None,
            Some(180_000),
            false,
        );
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        assert!(insert_local_usage_event(&store, &row).unwrap());
        let cost = model_call_cost_for_sessions(&store, &["job-sess"]).unwrap();
        let got = cost.input_plus_output;
        assert_ne!(
            got,
            Some(4_000),
            "output alone is not the cost of input 180000 and output 4000, got {got:?}"
        );
        assert_eq!(
            got,
            Some(184_000),
            "input 180000 + output 4000 = 184000 when those fields are disjoint, got {got:?}"
        );
        assert_ne!(got, Some(180_000), "context length is not the cost");
        assert_ne!(
            got,
            Some(184_000 + 12_000),
            "cached_tokens is already inside input_tokens"
        );
        assert_ne!(got, Some(0));

        let bare = TempDir::new().unwrap();
        let bare_store = open_at(&bare.path().join("grok_oss.db")).unwrap();
        let unread = model_call_cost_for_sessions(&bare_store, &["did-tool-work"]).unwrap();
        assert_ne!(unread.input_plus_output, Some(0));
        assert!(unread.input_plus_output.is_none());

        let cell = broken_host_context_cell("0K/0K", true).expect("sentence");
        assert_eq!(cell, "the context figure was not stored");
        assert_ne!(cell, "0");
        assert!(!cell.contains("not stored yet"), "{cell}");
        assert!(!cell.contains("not accepted"), "{cell}");
        assert!(!cell.contains("not_fetched"), "{cell}");
        assert!(broken_host_context_cell("168K/200K", true).is_none());
        assert!(broken_host_context_cell("0K/0K", false).is_none());
    }

    /// Last usage.jsonl input_tokens is the context integer. A later zero
    /// row does not hide an earlier real input. Cached tokens stay inside
    /// input. total_tokens is not this figure.
    #[test]
    fn last_input_tokens_from_usage_jsonl_skips_zero_and_uses_input() {
        let tmp = TempDir::new().unwrap();
        let usage = tmp.path().join("usage.jsonl");
        std::fs::write(
            &usage,
            concat!(
                r#"{"schema_version":1,"event_ulid":"01FIRSTINPUT0000000000001","timestamp":"2026-10-13T00:00:00.000Z","turn_type":"agent_turn","agent_kind":"general-purpose","session_id":"child-sess","input_tokens":80000,"output_tokens":10,"cached_tokens":12000,"reasoning_tokens":5,"total_tokens":80000,"cost_missing":false,"incomplete":false}"#,
                "\n",
                r#"{"schema_version":1,"event_ulid":"01SECONDINPUT000000000001","timestamp":"2026-10-13T00:01:00.000Z","turn_type":"agent_turn","agent_kind":"general-purpose","session_id":"child-sess","input_tokens":152000,"output_tokens":400,"cached_tokens":20000,"reasoning_tokens":80,"total_tokens":152000,"cost_missing":false,"incomplete":false}"#,
                "\n",
                r#"{"schema_version":1,"event_ulid":"01ZEROINPUT00000000000001","timestamp":"2026-10-13T00:02:00.000Z","turn_type":"agent_turn","agent_kind":"general-purpose","session_id":"child-sess","input_tokens":0,"output_tokens":0,"total_tokens":0,"cost_missing":false,"incomplete":false}"#,
                "\n",
            ),
        )
        .unwrap();
        let got = last_input_tokens_from_usage_jsonl(&usage);
        assert_eq!(got, Some(152_000));
        assert_ne!(got, Some(0));
        assert_ne!(
            got,
            Some(152_000 + 20_000),
            "cached_tokens is already inside input"
        );
        assert_ne!(
            got,
            Some(80_000),
            "an earlier row is not the last real input"
        );
        assert!(host_chrome_is_zero_context("0K/0K"));
        assert!(host_chrome_is_zero_context("0K/0K context+"));
        assert!(host_chrome_is_zero_context("0K/0K tokens (0% context)"));
        assert!(!host_chrome_is_zero_context("152K/200K context+"));
    }
}
