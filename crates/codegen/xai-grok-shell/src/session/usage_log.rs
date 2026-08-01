//! Append-only per-session `usage.jsonl` for model-call billing rows.
//!
//! Written at the end of model turns from
//! [`crate::session::acp_session::SessionActor::record_response_token_usage`].
//! Main-agent rows use `turn_type`/`agent_kind` = `"main"`; subagent/task
//! sessions use `turn_type` = `"agent_turn"` and `agent_kind` = the task's
//! subagent type (e.g. `explore`, `general-purpose`). Fail-open: open /
//! serialize / write failures never break the turn.
//!
//! Schema v1 is stable enough for later SQL ingest.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use serde::Serialize;
use xai_grok_sampling_types::{TokenUsage, reported_cost_ticks};

/// Filename under the session directory.
pub const USAGE_FILE: &str = "usage.jsonl";

/// Schema version stamped on every row.
pub const SCHEMA_VERSION: u32 = 1;

/// `turn_type` for a main-agent tool-loop model call.
pub const TURN_TYPE_MAIN: &str = "main";

/// `turn_type` for a subagent / task-agent model call.
pub const TURN_TYPE_AGENT_TURN: &str = "agent_turn";

/// `agent_kind` for the primary session agent (not a subagent).
pub const AGENT_KIND_MAIN: &str = "main";

/// Fallback `agent_kind` when a subagent session has no type label.
pub const AGENT_KIND_SUBAGENT: &str = "subagent";

/// One JSONL row in `usage.jsonl` (schema v1).
///
/// Field names are snake_case and SQL-friendly. Optional token/cost fields are
/// omitted when unknown so incomplete rows stay compact.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UsageRecord {
    pub schema_version: u32,
    /// Row id (ULID, Crockford base32).
    pub event_ulid: String,
    /// Optional work/join ULID (set when a work id is known for the turn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_ulid: Option<String>,
    /// RFC3339 timestamp (UTC, millis).
    pub timestamp: String,
    /// e.g. `main` (main loop), `agent_turn` (subagent/task).
    pub turn_type: String,
    /// e.g. `main`, `explore`, `general-purpose`, `plan`, …
    pub agent_kind: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Full prompt tokens (includes cache reads; cache writes folded in).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Cache **read** hits only (`TokenUsage.cached_prompt_tokens`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Provider `total_tokens` (live context length on Responses; not always input+output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    /// USD ticks (1e10 per USD). Absent when unreported or zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd_ticks: Option<i64>,
    /// True when cost was missing/zero on this call (even if other rows had cost).
    pub cost_missing: bool,
    /// True when usage is incomplete/untrustworthy for this call.
    pub incomplete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_duration_ms: Option<u64>,
}

/// Identity fields that distinguish main vs subagent/task rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageIdentity {
    pub turn_type: String,
    pub agent_kind: String,
    pub work_ulid: Option<String>,
}

impl UsageIdentity {
    pub fn main() -> Self {
        Self {
            turn_type: TURN_TYPE_MAIN.to_owned(),
            agent_kind: AGENT_KIND_MAIN.to_owned(),
            work_ulid: None,
        }
    }

    /// Subagent / task-agent identity. `agent_kind` should be the task
    /// `subagent_type` (e.g. `explore`); falls back to [`AGENT_KIND_SUBAGENT`].
    pub fn agent_turn(agent_kind: impl Into<String>, work_ulid: Option<String>) -> Self {
        let kind = agent_kind.into();
        Self {
            turn_type: TURN_TYPE_AGENT_TURN.to_owned(),
            agent_kind: if kind.is_empty() {
                AGENT_KIND_SUBAGENT.to_owned()
            } else {
                kind
            },
            work_ulid,
        }
    }
}

impl UsageRecord {
    /// Build a complete model-call row from provider usage + identity.
    pub fn model_call(
        identity: UsageIdentity,
        session_id: impl Into<String>,
        prompt_id: Option<String>,
        model_id: Option<String>,
        usage: &TokenUsage,
        api_duration_ms: Option<u64>,
        cost_usd_ticks: Option<i64>,
    ) -> Self {
        let cost = reported_cost_ticks(cost_usd_ticks);
        Self {
            schema_version: SCHEMA_VERSION,
            event_ulid: xai_grok_tools::util::ulid::mint(),
            work_ulid: identity.work_ulid,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            turn_type: identity.turn_type,
            agent_kind: identity.agent_kind,
            session_id: session_id.into(),
            prompt_id,
            model_id,
            input_tokens: Some(u64::from(usage.prompt_tokens)),
            output_tokens: Some(u64::from(usage.completion_tokens)),
            cached_tokens: Some(u64::from(usage.cached_prompt_tokens)),
            reasoning_tokens: Some(u64::from(usage.reasoning_tokens)),
            total_tokens: Some(u64::from(usage.total_tokens)),
            cost_usd_ticks: cost,
            cost_missing: cost.is_none(),
            incomplete: false,
            api_duration_ms,
        }
    }

    /// Build an incomplete row when the provider omitted usage.
    pub fn incomplete(
        identity: UsageIdentity,
        session_id: impl Into<String>,
        prompt_id: Option<String>,
        model_id: Option<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            event_ulid: xai_grok_tools::util::ulid::mint(),
            work_ulid: identity.work_ulid,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            turn_type: identity.turn_type,
            agent_kind: identity.agent_kind,
            session_id: session_id.into(),
            prompt_id,
            model_id,
            input_tokens: None,
            output_tokens: None,
            cached_tokens: None,
            reasoning_tokens: None,
            total_tokens: None,
            cost_usd_ticks: None,
            cost_missing: true,
            incomplete: true,
            api_duration_ms: None,
        }
    }

    /// Build a complete main-loop model-call row from provider usage.
    pub fn main_model_call(
        session_id: impl Into<String>,
        prompt_id: Option<String>,
        model_id: Option<String>,
        usage: &TokenUsage,
        api_duration_ms: Option<u64>,
        cost_usd_ticks: Option<i64>,
    ) -> Self {
        Self::model_call(
            UsageIdentity::main(),
            session_id,
            prompt_id,
            model_id,
            usage,
            api_duration_ms,
            cost_usd_ticks,
        )
    }

    /// Build an incomplete main-loop row when the provider omitted usage.
    pub fn main_incomplete(
        session_id: impl Into<String>,
        prompt_id: Option<String>,
        model_id: Option<String>,
    ) -> Self {
        Self::incomplete(UsageIdentity::main(), session_id, prompt_id, model_id)
    }

    /// Build a complete subagent/task model-call row.
    pub fn agent_model_call(
        agent_kind: impl Into<String>,
        work_ulid: Option<String>,
        session_id: impl Into<String>,
        prompt_id: Option<String>,
        model_id: Option<String>,
        usage: &TokenUsage,
        api_duration_ms: Option<u64>,
        cost_usd_ticks: Option<i64>,
    ) -> Self {
        Self::model_call(
            UsageIdentity::agent_turn(agent_kind, work_ulid),
            session_id,
            prompt_id,
            model_id,
            usage,
            api_duration_ms,
            cost_usd_ticks,
        )
    }

    /// Build an incomplete subagent/task row when the provider omitted usage.
    pub fn agent_incomplete(
        agent_kind: impl Into<String>,
        work_ulid: Option<String>,
        session_id: impl Into<String>,
        prompt_id: Option<String>,
        model_id: Option<String>,
    ) -> Self {
        Self::incomplete(
            UsageIdentity::agent_turn(agent_kind, work_ulid),
            session_id,
            prompt_id,
            model_id,
        )
    }
}

/// Fail-open append one JSON line to `{session_dir}/usage.jsonl`.
///
/// Never panics; never returns an error to the caller. First failure logs a
/// single warning (same pattern as `events.jsonl`).
pub fn append_usage_record(session_dir: &Path, record: &UsageRecord) {
    static ERROR_LOGGED: AtomicBool = AtomicBool::new(false);

    let path = session_dir.join(USAGE_FILE);
    let Ok(mut line) = serde_json::to_vec(record) else {
        return;
    };
    line.push(b'\n');

    let write_result = (|| -> std::io::Result<()> {
        // Session dir is normally already present; create if missing so tests
        // and early-spawn edge cases still work.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(&line)?;
        Ok(())
    })();

    if let Err(e) = write_result
        && !ERROR_LOGGED.swap(true, Ordering::Relaxed)
    {
        tracing::warn!(
            path = %path.display(),
            error = %e,
            "usage.jsonl write failed (fail-open; further failures suppressed)"
        );
    }
}

/// Convenience: build + append a model-call row with identity. Fail-open.
pub fn record_model_call(
    session_dir: &Path,
    identity: UsageIdentity,
    session_id: &str,
    prompt_id: Option<String>,
    model_id: Option<String>,
    usage: &TokenUsage,
    api_duration_ms: Option<u64>,
    cost_usd_ticks: Option<i64>,
) {
    let record = UsageRecord::model_call(
        identity,
        session_id,
        prompt_id,
        model_id,
        usage,
        api_duration_ms,
        cost_usd_ticks,
    );
    append_usage_record(session_dir, &record);
}

/// Convenience: build + append an incomplete row with identity. Fail-open.
pub fn record_incomplete(
    session_dir: &Path,
    identity: UsageIdentity,
    session_id: &str,
    prompt_id: Option<String>,
    model_id: Option<String>,
) {
    let record = UsageRecord::incomplete(identity, session_id, prompt_id, model_id);
    append_usage_record(session_dir, &record);
}

/// Convenience: build + append a main model-call row. Fail-open.
pub fn record_main_model_call(
    session_dir: &Path,
    session_id: &str,
    prompt_id: Option<String>,
    model_id: Option<String>,
    usage: &TokenUsage,
    api_duration_ms: Option<u64>,
    cost_usd_ticks: Option<i64>,
) {
    record_model_call(
        session_dir,
        UsageIdentity::main(),
        session_id,
        prompt_id,
        model_id,
        usage,
        api_duration_ms,
        cost_usd_ticks,
    );
}

/// Convenience: build + append an incomplete main row. Fail-open.
pub fn record_main_incomplete(
    session_dir: &Path,
    session_id: &str,
    prompt_id: Option<String>,
    model_id: Option<String>,
) {
    record_incomplete(
        session_dir,
        UsageIdentity::main(),
        session_id,
        prompt_id,
        model_id,
    );
}

/// Convenience: build + append a subagent/task model-call row. Fail-open.
pub fn record_agent_model_call(
    session_dir: &Path,
    agent_kind: &str,
    work_ulid: Option<String>,
    session_id: &str,
    prompt_id: Option<String>,
    model_id: Option<String>,
    usage: &TokenUsage,
    api_duration_ms: Option<u64>,
    cost_usd_ticks: Option<i64>,
) {
    record_model_call(
        session_dir,
        UsageIdentity::agent_turn(agent_kind, work_ulid),
        session_id,
        prompt_id,
        model_id,
        usage,
        api_duration_ms,
        cost_usd_ticks,
    );
}

/// Convenience: build + append an incomplete subagent/task row. Fail-open.
pub fn record_agent_incomplete(
    session_dir: &Path,
    agent_kind: &str,
    work_ulid: Option<String>,
    session_id: &str,
    prompt_id: Option<String>,
    model_id: Option<String>,
) {
    record_incomplete(
        session_dir,
        UsageIdentity::agent_turn(agent_kind, work_ulid),
        session_id,
        prompt_id,
        model_id,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::TokenUsage;

    fn sample_usage() -> TokenUsage {
        TokenUsage {
            prompt_tokens: 1_000,
            completion_tokens: 50,
            total_tokens: 12_000,
            reasoning_tokens: 10,
            cached_prompt_tokens: 200,
        }
    }

    #[test]
    fn main_model_call_serializes_sql_ready_fields() {
        let rec = UsageRecord::main_model_call(
            "sess-1",
            Some("prompt-abc".into()),
            Some("grok-4".into()),
            &sample_usage(),
            Some(42),
            Some(1_000_000),
        );
        assert_eq!(rec.schema_version, SCHEMA_VERSION);
        assert_eq!(rec.turn_type, TURN_TYPE_MAIN);
        assert_eq!(rec.agent_kind, AGENT_KIND_MAIN);
        assert_eq!(rec.input_tokens, Some(1_000));
        assert_eq!(rec.output_tokens, Some(50));
        assert_eq!(rec.cached_tokens, Some(200));
        assert_eq!(rec.reasoning_tokens, Some(10));
        assert_eq!(rec.total_tokens, Some(12_000));
        assert_eq!(rec.cost_usd_ticks, Some(1_000_000));
        assert!(!rec.cost_missing);
        assert!(!rec.incomplete);
        assert!(xai_grok_tools::util::ulid::is_valid(&rec.event_ulid));
        assert_eq!(rec.event_ulid.len(), 26);
        assert!(rec.work_ulid.is_none());

        let v: serde_json::Value = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["session_id"], "sess-1");
        assert_eq!(v["prompt_id"], "prompt-abc");
        assert_eq!(v["model_id"], "grok-4");
        assert_eq!(v["input_tokens"], 1000);
        assert_eq!(v["output_tokens"], 50);
        assert_eq!(v["cached_tokens"], 200);
        assert_eq!(v["cost_usd_ticks"], 1_000_000);
        assert_eq!(v["cost_missing"], false);
        assert_eq!(v["incomplete"], false);
        assert_eq!(v["api_duration_ms"], 42);
        // work_ulid omitted when None
        assert!(v.get("work_ulid").is_none());
        assert!(v["timestamp"].as_str().is_some());
        assert!(v["event_ulid"].as_str().unwrap().len() == 26);
    }

    #[test]
    fn agent_model_call_uses_agent_turn_and_subagent_kind() {
        let work = xai_grok_tools::util::ulid::mint();
        let rec = UsageRecord::agent_model_call(
            "explore",
            Some(work.clone()),
            "child-sess",
            Some("pid".into()),
            Some("grok-4".into()),
            &sample_usage(),
            Some(11),
            Some(500),
        );
        assert_eq!(rec.turn_type, TURN_TYPE_AGENT_TURN);
        assert_eq!(rec.agent_kind, "explore");
        assert_eq!(rec.work_ulid.as_deref(), Some(work.as_str()));
        assert_eq!(rec.session_id, "child-sess");
        assert_eq!(rec.input_tokens, Some(1_000));
        assert!(!rec.incomplete);
        assert_ne!(rec.agent_kind, AGENT_KIND_MAIN);
        assert_ne!(rec.turn_type, TURN_TYPE_MAIN);

        let v: serde_json::Value = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["turn_type"], "agent_turn");
        assert_eq!(v["agent_kind"], "explore");
        assert_eq!(v["work_ulid"], work);
        assert_eq!(v["session_id"], "child-sess");
        assert_eq!(v["api_duration_ms"], 11);
        assert_eq!(v["cost_usd_ticks"], 500);
    }

    #[test]
    fn agent_model_call_empty_kind_falls_back_to_subagent() {
        let rec =
            UsageRecord::agent_model_call("", None, "s", None, None, &sample_usage(), None, None);
        assert_eq!(rec.agent_kind, AGENT_KIND_SUBAGENT);
        assert_eq!(rec.turn_type, TURN_TYPE_AGENT_TURN);
        assert!(rec.work_ulid.is_none());
        let v: serde_json::Value = serde_json::to_value(&rec).unwrap();
        assert!(v.get("work_ulid").is_none());
    }

    #[test]
    fn agent_incomplete_omits_tokens_and_keeps_kind() {
        let rec = UsageRecord::agent_incomplete(
            "general-purpose",
            None,
            "s",
            Some("p".into()),
            Some("m".into()),
        );
        assert!(rec.incomplete);
        assert!(rec.cost_missing);
        assert_eq!(rec.agent_kind, "general-purpose");
        assert_eq!(rec.turn_type, TURN_TYPE_AGENT_TURN);
        assert!(rec.input_tokens.is_none());
        let v: serde_json::Value = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["incomplete"], true);
        assert_eq!(v["agent_kind"], "general-purpose");
        assert!(v.get("input_tokens").is_none());
    }

    #[test]
    fn zero_cost_ticks_normalize_to_missing() {
        let rec = UsageRecord::main_model_call(
            "s",
            None,
            None,
            &sample_usage(),
            None,
            Some(0), // reported_cost_ticks filters zeros
        );
        assert!(rec.cost_usd_ticks.is_none());
        assert!(rec.cost_missing);
        let v: serde_json::Value = serde_json::to_value(&rec).unwrap();
        assert!(v.get("cost_usd_ticks").is_none());
        assert_eq!(v["cost_missing"], true);
    }

    #[test]
    fn incomplete_row_omits_token_fields() {
        let rec = UsageRecord::main_incomplete("s", Some("p".into()), None);
        assert!(rec.incomplete);
        assert!(rec.cost_missing);
        assert!(rec.input_tokens.is_none());
        let v: serde_json::Value = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["incomplete"], true);
        assert!(v.get("input_tokens").is_none());
        assert!(v.get("output_tokens").is_none());
        assert!(v.get("cached_tokens").is_none());
    }

    #[test]
    fn append_writes_jsonl_line() {
        let dir = tempfile::tempdir().unwrap();
        let rec = UsageRecord::main_model_call(
            "sess-append",
            None,
            Some("m".into()),
            &sample_usage(),
            Some(1),
            None,
        );
        append_usage_record(dir.path(), &rec);
        append_usage_record(dir.path(), &rec);

        let text = std::fs::read_to_string(dir.path().join(USAGE_FILE)).unwrap();
        let lines: Vec<&str> = text.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["session_id"], "sess-append");
        assert_eq!(first["turn_type"], "main");
        assert_eq!(first["agent_kind"], "main");
        assert_eq!(first["input_tokens"], 1000);
    }

    #[test]
    fn record_agent_model_call_helper_writes_subagent_fields() {
        let dir = tempfile::tempdir().unwrap();
        let work = xai_grok_tools::util::ulid::mint();
        record_agent_model_call(
            dir.path(),
            "plan",
            Some(work.clone()),
            "child-1",
            Some("p1".into()),
            Some("mid".into()),
            &sample_usage(),
            Some(3),
            Some(99),
        );
        let text = std::fs::read_to_string(dir.path().join(USAGE_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(v["turn_type"], "agent_turn");
        assert_eq!(v["agent_kind"], "plan");
        assert_eq!(v["work_ulid"], work);
        assert_eq!(v["session_id"], "child-1");
        assert_eq!(v["prompt_id"], "p1");
        assert_eq!(v["model_id"], "mid");
        assert_eq!(v["cost_usd_ticks"], 99);
        assert_eq!(v["api_duration_ms"], 3);
        assert_ne!(v["agent_kind"], "main");
    }

    #[test]
    fn append_fail_open_on_unwritable_path() {
        // Point session_dir at a regular file so create/open of usage.jsonl fails.
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").unwrap();
        let rec = UsageRecord::main_model_call("s", None, None, &sample_usage(), None, None);
        // Must not panic.
        append_usage_record(&blocker, &rec);
        append_usage_record(&blocker, &rec);
    }

    #[test]
    fn record_main_model_call_helper_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        record_main_model_call(
            dir.path(),
            "sid",
            Some("pid".into()),
            Some("mid".into()),
            &sample_usage(),
            Some(9),
            Some(77),
        );
        let text = std::fs::read_to_string(dir.path().join(USAGE_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(v["session_id"], "sid");
        assert_eq!(v["prompt_id"], "pid");
        assert_eq!(v["model_id"], "mid");
        assert_eq!(v["cost_usd_ticks"], 77);
        assert_eq!(v["api_duration_ms"], 9);
    }
}
