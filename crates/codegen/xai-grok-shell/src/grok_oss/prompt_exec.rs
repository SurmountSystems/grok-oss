//! Prompt-task execution metrics in `grok_oss.db` (schema v3).
//!
//! Tokens, honest wall clock, model, estimate vs actual, and Token Economy
//! cost ticks. Not a new billing meter. Not the upstream session sqlite.

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::OptionalExtension;

use super::GrokOssStore;
use crate::token_economy::ticks_to_usd;
use crate::util::dual_clock::DualClock;

/// Insert payload for one prompt-task execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptExecRecord {
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub wall_ms: i64,
    pub estimated_tokens_in: Option<i64>,
    pub estimated_tokens_out: Option<i64>,
    pub estimated_wall_ms: Option<i64>,
    pub cost_usd_ticks: Option<i64>,
    /// Later latency-diagnostics slice; stored so the column round-trips now.
    pub first_reasoning_token_ms: Option<i64>,
    pub tool_call_ms: Option<i64>,
    pub thinking_ms: Option<i64>,
    pub prefix_cost_hint_ticks: Option<i64>,
}

/// Stored exec-metrics row for a `prompt_tasks` ULID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptExecMetrics {
    pub id: String,
    pub prompt_task_id: String,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub wall_ms: i64,
    pub estimated_tokens_in: Option<i64>,
    pub estimated_tokens_out: Option<i64>,
    pub estimated_wall_ms: Option<i64>,
    pub cost_usd_ticks: Option<i64>,
    pub cost_missing: bool,
    pub first_reasoning_token_ms: Option<i64>,
    pub tool_call_ms: Option<i64>,
    pub thinking_ms: Option<i64>,
    pub prefix_cost_hint_ticks: Option<i64>,
    pub created_at: String,
}

/// Tokens per dollar from known `cost_usd_ticks` via Token Economy
/// [`ticks_to_usd`]. None when cost ticks are missing or not positive.
/// Does not invent included SuperGrok period used percent.
pub fn tokens_per_dollar(
    tokens_in: i64,
    tokens_out: i64,
    cost_usd_ticks: Option<i64>,
) -> Option<f64> {
    let ticks = cost_usd_ticks.filter(|&t| t > 0)?;
    let usd = ticks_to_usd(ticks);
    if usd <= 0.0 {
        return None;
    }
    Some((tokens_in.saturating_add(tokens_out)) as f64 / usd)
}

/// Honest work milliseconds: DualClock monotonic (pauses in sleep) minus
/// recorded reconnection. Wall-clock sleep/suspend is not work.
pub(crate) fn honest_work_ms(started: DualClock, now: DualClock, reconnect_ms: u64) -> u64 {
    let (awake, _) = started.elapsed_between(now);
    duration_as_millis(awake).saturating_sub(reconnect_ms)
}

fn duration_as_millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Work timer that pauses across laptop sleep and excludes reconnection.
pub struct HonestWorkClock {
    started: DualClock,
    reconnect_ms: u64,
    reconnect_started: Option<DualClock>,
}

impl HonestWorkClock {
    pub fn start() -> Self {
        Self::start_at(DualClock::now())
    }

    pub(crate) fn start_at(now: DualClock) -> Self {
        Self {
            started: now,
            reconnect_ms: 0,
            reconnect_started: None,
        }
    }

    pub fn pause_reconnect(&mut self) {
        self.pause_reconnect_at(DualClock::now());
    }

    pub(crate) fn pause_reconnect_at(&mut self, now: DualClock) {
        if self.reconnect_started.is_none() {
            self.reconnect_started = Some(now);
        }
    }

    pub fn resume_after_reconnect(&mut self) {
        self.resume_after_reconnect_at(DualClock::now());
    }

    pub(crate) fn resume_after_reconnect_at(&mut self, now: DualClock) {
        let Some(paused) = self.reconnect_started.take() else {
            return;
        };
        let gap = honest_work_ms(paused, now, 0);
        self.reconnect_ms = self.reconnect_ms.saturating_add(gap);
    }

    pub fn work_ms(&self) -> u64 {
        self.work_ms_at(DualClock::now())
    }

    pub(crate) fn work_ms_at(&self, now: DualClock) -> u64 {
        let mut reconnect = self.reconnect_ms;
        if let Some(paused) = self.reconnect_started {
            reconnect = reconnect.saturating_add(honest_work_ms(paused, now, 0));
        }
        honest_work_ms(self.started, now, reconnect)
    }
}

/// In-flight prompt-as-task row plus the honest work clock started at submit.
pub struct LivePromptTask {
    pub id: String,
    clock: HonestWorkClock,
}

impl LivePromptTask {
    /// Insert a `prompt_tasks` ULID row and start [`HonestWorkClock`].
    pub fn start(store: &GrokOssStore, body: &str) -> Result<Self> {
        Self::start_at(store, body, DualClock::now())
    }

    pub(crate) fn start_at(store: &GrokOssStore, body: &str, now: DualClock) -> Result<Self> {
        let task = store.insert_prompt_task(body, "running", None, None)?;
        Ok(Self {
            id: task.id,
            clock: HonestWorkClock::start_at(now),
        })
    }

    /// Pause honest work for a reconnect window. Nested pause is a no-op.
    pub fn pause_reconnect(&mut self) {
        self.clock.pause_reconnect();
    }

    pub(crate) fn pause_reconnect_at(&mut self, now: DualClock) {
        self.clock.pause_reconnect_at(now);
    }

    /// Resume honest work after reconnect. No-op if not paused.
    pub fn resume_after_reconnect(&mut self) {
        self.clock.resume_after_reconnect();
    }

    pub(crate) fn resume_after_reconnect_at(&mut self, now: DualClock) {
        self.clock.resume_after_reconnect_at(now);
    }

    pub fn is_reconnect_paused(&self) -> bool {
        self.clock.reconnect_started.is_some()
    }

    /// Fail-open start: missing store or insert error returns None.
    pub fn try_start(store: Option<&GrokOssStore>, body: &str) -> Option<Self> {
        let store = store?;
        match Self::start(store, body) {
            Ok(task) => Some(task),
            Err(e) => {
                tracing::debug!(error = %e, "live prompt_task insert failed (fail-open)");
                None
            }
        }
    }

    pub fn work_ms(&self) -> u64 {
        self.clock.work_ms()
    }

    /// Write `prompt_exec_metrics` with `wall_ms` from the honest clock.
    pub fn finish(
        self,
        store: &GrokOssStore,
        record: PromptExecRecord,
    ) -> Result<PromptExecMetrics> {
        self.finish_at(store, record, DualClock::now())
    }

    pub(crate) fn finish_at(
        self,
        store: &GrokOssStore,
        mut record: PromptExecRecord,
        now: DualClock,
    ) -> Result<PromptExecMetrics> {
        let work = self.clock.work_ms_at(now);
        record.wall_ms = i64::try_from(work).unwrap_or(i64::MAX);
        store.insert_prompt_exec_metrics(&self.id, &record)
    }

    /// Fail-open finish: missing store or insert error returns None.
    pub fn try_finish(
        self,
        store: Option<&GrokOssStore>,
        record: PromptExecRecord,
    ) -> Option<PromptExecMetrics> {
        let store = store?;
        match self.finish(store, record) {
            Ok(metrics) => Some(metrics),
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    "live prompt_exec_metrics insert failed (fail-open)"
                );
                None
            }
        }
    }
}

impl GrokOssStore {
    /// Insert one exec-metrics row for a prompt-task ULID. Mints a ULID.
    pub fn insert_prompt_exec_metrics(
        &self,
        prompt_task_id: &str,
        record: &PromptExecRecord,
    ) -> Result<PromptExecMetrics> {
        let id = xai_grok_tools::util::ulid::mint();
        let now = Utc::now().to_rfc3339();
        let cost_missing = record.cost_usd_ticks.is_none();
        self.connection()
            .execute(
                "INSERT INTO prompt_exec_metrics (
                   id, prompt_task_id, tokens_in, tokens_out, model, reasoning_effort,
                   wall_ms, estimated_tokens_in, estimated_tokens_out, estimated_wall_ms,
                   cost_usd_ticks, cost_missing, first_reasoning_token_ms, tool_call_ms,
                   thinking_ms, prefix_cost_hint_ticks, created_at
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6,
                   ?7, ?8, ?9, ?10,
                   ?11, ?12, ?13, ?14,
                   ?15, ?16, ?17
                 )",
                rusqlite::params![
                    id,
                    prompt_task_id,
                    record.tokens_in,
                    record.tokens_out,
                    record.model,
                    record.reasoning_effort,
                    record.wall_ms,
                    record.estimated_tokens_in,
                    record.estimated_tokens_out,
                    record.estimated_wall_ms,
                    record.cost_usd_ticks,
                    i64::from(cost_missing),
                    record.first_reasoning_token_ms,
                    record.tool_call_ms,
                    record.thinking_ms,
                    record.prefix_cost_hint_ticks,
                    now,
                ],
            )
            .context("insert prompt_exec_metrics")?;
        Ok(PromptExecMetrics {
            id,
            prompt_task_id: prompt_task_id.to_owned(),
            tokens_in: record.tokens_in,
            tokens_out: record.tokens_out,
            model: record.model.clone(),
            reasoning_effort: record.reasoning_effort.clone(),
            wall_ms: record.wall_ms,
            estimated_tokens_in: record.estimated_tokens_in,
            estimated_tokens_out: record.estimated_tokens_out,
            estimated_wall_ms: record.estimated_wall_ms,
            cost_usd_ticks: record.cost_usd_ticks,
            cost_missing,
            first_reasoning_token_ms: record.first_reasoning_token_ms,
            tool_call_ms: record.tool_call_ms,
            thinking_ms: record.thinking_ms,
            prefix_cost_hint_ticks: record.prefix_cost_hint_ticks,
            created_at: now,
        })
    }

    /// Load an exec-metrics row by ULID.
    pub fn load_prompt_exec_metrics(&self, id: &str) -> Result<Option<PromptExecMetrics>> {
        self.connection()
            .query_row(
                "SELECT id, prompt_task_id, tokens_in, tokens_out, model, reasoning_effort,
                        wall_ms, estimated_tokens_in, estimated_tokens_out, estimated_wall_ms,
                        cost_usd_ticks, cost_missing, first_reasoning_token_ms, tool_call_ms,
                        thinking_ms, prefix_cost_hint_ticks, created_at
                 FROM prompt_exec_metrics WHERE id = ?1",
                [id],
                |row| {
                    let cost_missing_i: i64 = row.get(11)?;
                    Ok(PromptExecMetrics {
                        id: row.get(0)?,
                        prompt_task_id: row.get(1)?,
                        tokens_in: row.get(2)?,
                        tokens_out: row.get(3)?,
                        model: row.get(4)?,
                        reasoning_effort: row.get(5)?,
                        wall_ms: row.get(6)?,
                        estimated_tokens_in: row.get(7)?,
                        estimated_tokens_out: row.get(8)?,
                        estimated_wall_ms: row.get(9)?,
                        cost_usd_ticks: row.get(10)?,
                        cost_missing: cost_missing_i != 0,
                        first_reasoning_token_ms: row.get(12)?,
                        tool_call_ms: row.get(13)?,
                        thinking_ms: row.get(14)?,
                        prefix_cost_hint_ticks: row.get(15)?,
                        created_at: row.get(16)?,
                    })
                },
            )
            .optional()
            .context("load prompt_exec_metrics")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grok_oss::open_at;
    use crate::token_economy::ticks_to_usd;
    use crate::util::dual_clock::DualClock;
    use std::time::Duration;
    use tempfile::TempDir;

    /// Insert an exec-metrics row for a prompt_task ULID: tokens in/out, Grok
    /// 4.6 medium, honest wall_ms, estimate vs actual, cost ticks / TPD.
    #[test]
    fn insert_exec_metrics_for_prompt_task_ulid_round_trip() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let task = store
            .insert_prompt_task("run the metrics slice", "queued", None, None)
            .unwrap();
        assert!(xai_grok_tools::util::ulid::is_valid(&task.id));

        let record = PromptExecRecord {
            tokens_in: 1_200,
            tokens_out: 400,
            model: "grok-4.6".to_string(),
            reasoning_effort: Some("medium".to_string()),
            wall_ms: 12_500,
            estimated_tokens_in: Some(1_000),
            estimated_tokens_out: Some(350),
            estimated_wall_ms: Some(10_000),
            cost_usd_ticks: Some(5_000_000_000),
            first_reasoning_token_ms: Some(180),
            tool_call_ms: Some(3_000),
            thinking_ms: Some(8_000),
            prefix_cost_hint_ticks: Some(100),
        };
        let inserted = store.insert_prompt_exec_metrics(&task.id, &record).unwrap();
        assert!(xai_grok_tools::util::ulid::is_valid(&inserted.id));
        assert_eq!(inserted.id.len(), 26);
        assert_eq!(inserted.prompt_task_id, task.id);
        assert_eq!(inserted.tokens_in, 1_200);
        assert_eq!(inserted.tokens_out, 400);
        assert_eq!(inserted.model, "grok-4.6");
        assert_eq!(inserted.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(inserted.wall_ms, 12_500);
        assert_eq!(inserted.estimated_tokens_in, Some(1_000));
        assert_eq!(inserted.estimated_tokens_out, Some(350));
        assert_eq!(inserted.estimated_wall_ms, Some(10_000));
        assert_eq!(
            inserted.tokens_in - inserted.estimated_tokens_in.unwrap(),
            200
        );
        assert_eq!(
            inserted.tokens_out - inserted.estimated_tokens_out.unwrap(),
            50
        );
        assert_eq!(
            inserted.wall_ms - inserted.estimated_wall_ms.unwrap(),
            2_500
        );
        assert_eq!(inserted.cost_usd_ticks, Some(5_000_000_000));
        assert!(!inserted.cost_missing);
        assert_eq!(inserted.first_reasoning_token_ms, Some(180));
        assert_eq!(inserted.tool_call_ms, Some(3_000));
        assert_eq!(inserted.thinking_ms, Some(8_000));
        assert_eq!(inserted.prefix_cost_hint_ticks, Some(100));

        let loaded = store
            .load_prompt_exec_metrics(&inserted.id)
            .unwrap()
            .expect("exec metrics row");
        assert_eq!(loaded, inserted);

        let tpd = tokens_per_dollar(
            inserted.tokens_in,
            inserted.tokens_out,
            inserted.cost_usd_ticks,
        )
        .expect("known cost ticks yield tokens per dollar");
        let usd = ticks_to_usd(5_000_000_000);
        assert!((usd - 0.5).abs() < 1e-12);
        assert!((tpd - (1_600.0 / usd)).abs() < 1e-6);
    }

    /// Honest wall clock: elapsed does not include a sleep/suspend gap.
    /// Helper: [`honest_work_ms`] (monotonic/awake DualClock elapsed).
    #[test]
    fn honest_wall_clock_excludes_sleep_suspend_gap() {
        let start = DualClock::now();
        let after_sleep = DualClock {
            mono: start.mono + Duration::from_millis(2_000),
            wall: start.wall + Duration::from_secs(120),
        };
        assert_eq!(
            honest_work_ms(start, after_sleep, 0),
            2_000,
            "sleep/suspend (wall minus mono) must not count as work"
        );
        let (awake, total) = start.elapsed_between(after_sleep);
        assert_eq!(awake, Duration::from_millis(2_000));
        assert_eq!(total, Duration::from_secs(120));
    }

    /// Reconnection time is not counted as work.
    #[test]
    fn reconnection_time_is_not_counted_as_work() {
        let start = DualClock::now();
        let reconnect_start = DualClock {
            mono: start.mono + Duration::from_millis(4_000),
            wall: start.wall + Duration::from_millis(4_000),
        };
        let reconnect_end = DualClock {
            mono: start.mono + Duration::from_millis(7_000),
            wall: start.wall + Duration::from_millis(7_000),
        };
        let done = DualClock {
            mono: start.mono + Duration::from_millis(10_000),
            wall: start.wall + Duration::from_millis(10_000),
        };

        let mut clock = HonestWorkClock::start_at(start);
        clock.pause_reconnect_at(reconnect_start);
        clock.resume_after_reconnect_at(reconnect_end);
        assert_eq!(
            clock.work_ms_at(done),
            7_000,
            "3s reconnect while awake must not count as work"
        );
        assert_eq!(honest_work_ms(start, done, 3_000), 7_000);
    }

    /// Start inserts a prompt_task ULID and finish writes exec metrics. Missing
    /// store fail-opens.
    #[test]
    fn live_prompt_task_start_and_finish_round_trip_and_fail_open() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let live = LivePromptTask::start(&store, "live write").unwrap();
        assert!(xai_grok_tools::util::ulid::is_valid(&live.id));
        assert_eq!(live.id.len(), 26);
        let task = store.load_prompt_task(&live.id).unwrap().expect("task");
        assert_eq!(task.body, "live write");
        assert_eq!(task.status, "running");

        let metrics = live
            .finish(
                &store,
                PromptExecRecord {
                    tokens_in: 10,
                    tokens_out: 4,
                    model: "grok-4.6".into(),
                    reasoning_effort: None,
                    wall_ms: 0,
                    estimated_tokens_in: None,
                    estimated_tokens_out: None,
                    estimated_wall_ms: None,
                    cost_usd_ticks: Some(1_000_000_000),
                    first_reasoning_token_ms: None,
                    tool_call_ms: None,
                    thinking_ms: None,
                    prefix_cost_hint_ticks: None,
                },
            )
            .unwrap();
        assert_eq!(metrics.tokens_in, 10);
        assert_eq!(metrics.tokens_out, 4);
        assert_eq!(metrics.model, "grok-4.6");
        assert!(metrics.wall_ms >= 0);
        assert_eq!(metrics.cost_usd_ticks, Some(1_000_000_000));
        assert!(LivePromptTask::try_start(None, "no store").is_none());
    }

    /// Live finish `wall_ms` is honest work: reconnect time is not included.
    #[test]
    fn live_prompt_task_finish_wall_ms_excludes_reconnect_interval() {
        let tmp = TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let start = DualClock::now();
        let reconnect_start = DualClock {
            mono: start.mono + Duration::from_millis(4_000),
            wall: start.wall + Duration::from_millis(4_000),
        };
        let reconnect_end = DualClock {
            mono: start.mono + Duration::from_millis(7_000),
            wall: start.wall + Duration::from_millis(7_000),
        };
        let done = DualClock {
            mono: start.mono + Duration::from_millis(10_000),
            wall: start.wall + Duration::from_millis(10_000),
        };

        let mut live = LivePromptTask::start_at(&store, "reconnect pause", start).unwrap();
        live.pause_reconnect_at(reconnect_start);
        live.resume_after_reconnect_at(reconnect_end);
        let metrics = live
            .finish_at(
                &store,
                PromptExecRecord {
                    tokens_in: 10,
                    tokens_out: 4,
                    model: "grok-4.6".into(),
                    reasoning_effort: None,
                    wall_ms: 99_999,
                    estimated_tokens_in: None,
                    estimated_tokens_out: None,
                    estimated_wall_ms: None,
                    cost_usd_ticks: Some(1_000_000_000),
                    first_reasoning_token_ms: None,
                    tool_call_ms: None,
                    thinking_ms: None,
                    prefix_cost_hint_ticks: None,
                },
                done,
            )
            .unwrap();
        assert_eq!(
            metrics.wall_ms, 7_000,
            "3s reconnect while awake must not count in wall_ms"
        );
        assert_eq!(honest_work_ms(start, done, 3_000), 7_000);
    }
}
