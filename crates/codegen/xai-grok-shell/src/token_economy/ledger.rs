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
}
