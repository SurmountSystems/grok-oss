//! Session-todo chrome for prompt-task exec metrics.
//!
//! Tokens spent vs estimated, honest wall time, Token Economy cost ticks plus
//! tokens per dollar, time to first reasoning token, tool time vs thinking,
//! and prefix cost as context grows. Fail-open: missing store rows or missing
//! columns stay missing. Does not invent included SuperGrok period used
//! percent.

use std::time::Duration;

use xai_grok_shell::grok_oss::{GrokOssStore, PromptExecMetrics, tokens_per_dollar};
use xai_grok_shell::token_economy::ticks_to_usd;
use xai_grok_shell::tools::TodoItem;
use xai_tty_utils::format_human_duration;

/// Todo meta key for the prompt-as-task ULID. Distinct from `taskId` (live
/// subagent bind).
pub const PROMPT_TASK_ID_META_KEY: &str = "promptTaskId";

/// Prompt-task ULID from todo meta, when present.
pub fn todo_prompt_task_id(item: &TodoItem) -> Option<&str> {
    item.meta
        .as_ref()
        .and_then(|m| {
            m.get(PROMPT_TASK_ID_META_KEY)
                .or_else(|| m.get("prompt_task_id"))
        })
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Latest stored exec metrics for a prompt_task ULID. None when the row is
/// missing or the query fails (fail-open).
pub fn load_todo_exec_metrics_fail_open(
    store: &GrokOssStore,
    prompt_task_id: &str,
) -> Option<PromptExecMetrics> {
    store
        .load_latest_prompt_exec_metrics_for_prompt_task(prompt_task_id)
        .ok()
        .flatten()
}

/// Compact session-todo suffix: tokens spent vs estimated, honest wall, Token
/// Economy cost and tokens per dollar, first reasoning, tool vs thinking, and
/// prefix cost as context grows.
pub fn format_todo_exec_metrics_chrome(metrics: &PromptExecMetrics) -> String {
    let tokens = format_tokens_spent_vs_estimated(metrics);
    let wall = format_honest_wall_ms(metrics.wall_ms);
    let cost = format_token_economy_cost(metrics);
    let first = format_first_reasoning(metrics.first_reasoning_token_ms);
    let tool_vs_thinking = format_tool_vs_thinking(metrics.tool_call_ms, metrics.thinking_ms);
    let prefix = format_prefix_cost_as_context_grows(metrics.prefix_cost_hint_ticks);
    format!("{tokens} · {wall} · {cost} · {first} · {tool_vs_thinking} · {prefix}")
}

fn format_tokens_spent_vs_estimated(metrics: &PromptExecMetrics) -> String {
    let spent = metrics.tokens_in.saturating_add(metrics.tokens_out);
    match (metrics.estimated_tokens_in, metrics.estimated_tokens_out) {
        (Some(ein), Some(eout)) => {
            let estimated = ein.saturating_add(eout);
            format!("{spent} spent / {estimated} estimated tokens")
        }
        _ => format!("{spent} spent / estimate missing"),
    }
}

fn format_honest_wall_ms(wall_ms: i64) -> String {
    let ms = u64::try_from(wall_ms.max(0)).unwrap_or(0);
    format_human_duration(Duration::from_millis(ms))
}

/// Compact wait: milliseconds under 1s, seconds under 60s, then 15m43s / 1h2m.
/// None when the stored column is missing or negative.
fn format_latency_ms(ms: Option<i64>) -> Option<String> {
    let ms = ms.filter(|&v| v >= 0)?;
    let ms = u64::try_from(ms).unwrap_or(0);
    if ms < 1_000 {
        return Some(format!("{ms}ms"));
    }
    Some(format_human_duration(Duration::from_millis(ms)))
}

fn format_first_reasoning(first_reasoning_token_ms: Option<i64>) -> String {
    match format_latency_ms(first_reasoning_token_ms) {
        Some(wait) => format!("first reasoning {wait}"),
        None => "first reasoning missing".to_string(),
    }
}

fn format_tool_vs_thinking(tool_call_ms: Option<i64>, thinking_ms: Option<i64>) -> String {
    let tool = match format_latency_ms(tool_call_ms) {
        Some(wait) => format!("tool {wait}"),
        None => "tool missing".to_string(),
    };
    let thinking = match format_latency_ms(thinking_ms) {
        Some(wait) => format!("thinking {wait}"),
        None => "thinking missing".to_string(),
    };
    format!("{tool} / {thinking}")
}

fn format_prefix_cost_as_context_grows(prefix_cost_hint_ticks: Option<i64>) -> String {
    let Some(ticks) = prefix_cost_hint_ticks.filter(|&t| t > 0) else {
        return "prefix cost missing".to_string();
    };
    let usd = ticks_to_usd(ticks);
    if usd >= 0.0001 {
        format!("${usd:.4} prefix cost as context grows")
    } else {
        format!("{ticks} ticks prefix cost as context grows")
    }
}

fn format_token_economy_cost(metrics: &PromptExecMetrics) -> String {
    if metrics.cost_missing {
        return "cost missing".to_string();
    }
    let Some(ticks) = metrics.cost_usd_ticks.filter(|&t| t > 0) else {
        return "cost missing".to_string();
    };
    let usd = ticks_to_usd(ticks);
    match tokens_per_dollar(metrics.tokens_in, metrics.tokens_out, Some(ticks)) {
        Some(tpd) => format!("${usd:.4} Token Economy · {tpd:.0} tokens per dollar"),
        None => format!("${usd:.4} Token Economy"),
    }
}

/// Styled todo-row content: item text plus optional exec-metrics chrome.
pub fn todo_row_content_with_metrics(content: &str, metrics_chrome: Option<&str>) -> String {
    match metrics_chrome.filter(|s| !s.is_empty()) {
        Some(chrome) => format!("{content}  {chrome}"),
        None => content.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_shell::grok_oss::{PromptExecRecord, open_at};
    use xai_grok_shell::tools::{TodoPriority, TodoStatus};

    fn metrics_fixture(
        estimated_tokens_in: Option<i64>,
        estimated_tokens_out: Option<i64>,
        wall_ms: i64,
        cost_usd_ticks: Option<i64>,
    ) -> PromptExecMetrics {
        PromptExecMetrics {
            id: "01EXECMETRICSTEST00000001".into(),
            prompt_task_id: "01PROMPTTASKTEST000000001".into(),
            tokens_in: 1_200,
            tokens_out: 400,
            model: "grok-4.6".into(),
            reasoning_effort: Some("medium".into()),
            wall_ms,
            estimated_tokens_in,
            estimated_tokens_out,
            estimated_wall_ms: None,
            cost_usd_ticks,
            cost_missing: cost_usd_ticks.is_none(),
            first_reasoning_token_ms: None,
            tool_call_ms: None,
            thinking_ms: None,
            prefix_cost_hint_ticks: None,
            created_at: "2026-08-19T00:00:00Z".into(),
        }
    }

    fn todo_with_prompt_task(id: &str) -> TodoItem {
        TodoItem {
            content: "ship the board chrome".into(),
            priority: TodoPriority::default(),
            status: TodoStatus::InProgress,
            meta: Some(serde_json::json!({ PROMPT_TASK_ID_META_KEY: id })),
            size: None,
        }
    }

    /// Named contract: a session todo row shows tokens spent vs estimated.
    #[test]
    fn session_todo_row_shows_tokens_spent_vs_estimated() {
        let metrics = metrics_fixture(Some(1_000), Some(350), 12_500, Some(5_000_000_000));
        let chrome = format_todo_exec_metrics_chrome(&metrics);
        let row = todo_row_content_with_metrics("ship the board chrome", Some(&chrome));
        assert!(
            chrome.contains("1600 spent / 1350 estimated tokens"),
            "tokens spent vs estimated, got {chrome:?}"
        );
        assert!(
            row.contains("ship the board chrome"),
            "row keeps todo content, got {row:?}"
        );
        assert!(
            row.contains("1600 spent / 1350 estimated tokens"),
            "row shows tokens spent vs estimated, got {row:?}"
        );
    }

    /// Named contract: missing estimate stays missing (not a fake number).
    #[test]
    fn session_todo_row_shows_honest_missing_token_estimate() {
        let metrics = metrics_fixture(None, None, 12_500, Some(5_000_000_000));
        let chrome = format_todo_exec_metrics_chrome(&metrics);
        assert!(
            chrome.contains("1600 spent / estimate missing"),
            "honest missing estimate, got {chrome:?}"
        );
        assert!(
            !chrome.contains("estimated tokens"),
            "must not invent an estimate, got {chrome:?}"
        );
    }

    /// Named contract: honest wall is compact minutes when >= 60 seconds,
    /// never a raw 943s count.
    #[test]
    fn session_todo_row_shows_honest_wall_time_not_raw_seconds() {
        let long = metrics_fixture(Some(1_000), Some(350), 943_000, Some(5_000_000_000));
        let chrome = format_todo_exec_metrics_chrome(&long);
        assert!(
            chrome.contains("15m43s"),
            "943_000 ms is 15m43s compact chrome, got {chrome:?}"
        );
        assert!(
            !chrome.contains("943s"),
            "must not print raw 943s, got {chrome:?}"
        );
        let hour = metrics_fixture(Some(1_000), Some(350), 3_725_000, Some(5_000_000_000));
        let hour_chrome = format_todo_exec_metrics_chrome(&hour);
        assert!(
            hour_chrome.contains("1h2m"),
            "3_725_000 ms is 1h2m, got {hour_chrome:?}"
        );
        let short = metrics_fixture(Some(1_000), Some(350), 5_200, Some(5_000_000_000));
        let short_chrome = format_todo_exec_metrics_chrome(&short);
        assert!(
            short_chrome.contains("5.2s"),
            "under 60s may stay seconds, got {short_chrome:?}"
        );
    }

    /// Named contract: cost comes from Token Economy ticks and tokens per
    /// dollar. Does not invent included SuperGrok period used percent.
    #[test]
    fn session_todo_row_shows_token_economy_cost_and_tokens_per_dollar() {
        let metrics = metrics_fixture(Some(1_000), Some(350), 12_500, Some(5_000_000_000));
        let chrome = format_todo_exec_metrics_chrome(&metrics);
        assert!(
            chrome.contains("$0.5000 Token Economy"),
            "Token Economy ticks as dollars, got {chrome:?}"
        );
        assert!(
            chrome.contains("3200 tokens per dollar"),
            "tokens per dollar from TPD helper, got {chrome:?}"
        );
        assert!(
            !chrome.to_lowercase().contains("included"),
            "must not invent included SuperGrok period used percent, got {chrome:?}"
        );
        let missing = metrics_fixture(Some(1_000), Some(350), 12_500, None);
        let missing_chrome = format_todo_exec_metrics_chrome(&missing);
        assert!(
            missing_chrome.contains("cost missing"),
            "honest missing cost, got {missing_chrome:?}"
        );
        assert!(
            !missing_chrome.contains("$"),
            "must not invent a dollar cost, got {missing_chrome:?}"
        );
    }

    /// Named contract: a todo with a prompt_task ULID fail-open loads stored
    /// metrics; missing rows stay missing.
    #[test]
    fn session_todo_loads_stored_metrics_for_prompt_task_ulid_fail_open() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let task = store
            .insert_prompt_task("run the board chrome slice", "queued", None, None)
            .unwrap();
        let item = todo_with_prompt_task(&task.id);
        assert_eq!(todo_prompt_task_id(&item), Some(task.id.as_str()));
        assert!(
            load_todo_exec_metrics_fail_open(&store, &task.id).is_none(),
            "fail-open: no metrics row yet"
        );

        let record = PromptExecRecord {
            tokens_in: 1_200,
            tokens_out: 400,
            model: "grok-4.6".into(),
            reasoning_effort: Some("medium".into()),
            wall_ms: 943_000,
            estimated_tokens_in: Some(1_000),
            estimated_tokens_out: Some(350),
            estimated_wall_ms: Some(900_000),
            cost_usd_ticks: Some(5_000_000_000),
            first_reasoning_token_ms: None,
            tool_call_ms: None,
            thinking_ms: None,
            prefix_cost_hint_ticks: None,
        };
        store.insert_prompt_exec_metrics(&task.id, &record).unwrap();
        let loaded = load_todo_exec_metrics_fail_open(&store, &task.id)
            .expect("stored metrics for prompt_task ULID");
        let chrome = format_todo_exec_metrics_chrome(&loaded);
        let row = todo_row_content_with_metrics(&item.content, Some(&chrome));
        assert!(row.contains("1600 spent / 1350 estimated tokens"));
        assert!(row.contains("15m43s"));
        assert!(row.contains("$0.5000 Token Economy"));
        assert!(row.contains("3200 tokens per dollar"));
        assert!(load_todo_exec_metrics_fail_open(&store, "01NOTAPROMPTTASKULID00001").is_none());
    }

    /// Named contract: session-todo chrome paints time to first reasoning
    /// token, tool time vs thinking, and prefix cost as context grows from
    /// stored columns. Does not invent numbers.
    #[test]
    fn session_todo_row_shows_first_reasoning_tool_vs_thinking_and_prefix_cost() {
        let mut metrics = metrics_fixture(Some(1_000), Some(350), 12_500, Some(5_000_000_000));
        metrics.first_reasoning_token_ms = Some(180);
        metrics.tool_call_ms = Some(3_000);
        metrics.thinking_ms = Some(8_000);
        metrics.prefix_cost_hint_ticks = Some(1_000_000_000);
        let chrome = format_todo_exec_metrics_chrome(&metrics);
        assert!(
            chrome.contains("first reasoning 180ms"),
            "time to first reasoning token, got {chrome:?}"
        );
        assert!(
            chrome.contains("tool 3.0s / thinking 8.0s"),
            "tool time vs thinking, got {chrome:?}"
        );
        assert!(
            chrome.contains("$0.1000 prefix cost as context grows"),
            "prefix cost as context grows from Token Economy ticks, got {chrome:?}"
        );
        assert!(
            !chrome.to_lowercase().contains("included"),
            "must not invent included SuperGrok period used percent, got {chrome:?}"
        );
        assert!(
            !chrome.to_lowercase().contains("extras"),
            "must not teach extras as a nickname, got {chrome:?}"
        );
    }

    /// Named contract: missing latency columns stay missing labels, not
    /// invented zeros.
    #[test]
    fn session_todo_row_shows_honest_missing_latency_diagnostics() {
        let metrics = metrics_fixture(Some(1_000), Some(350), 12_500, Some(5_000_000_000));
        let chrome = format_todo_exec_metrics_chrome(&metrics);
        assert!(
            chrome.contains("first reasoning missing"),
            "honest missing first reasoning, got {chrome:?}"
        );
        assert!(
            chrome.contains("tool missing / thinking missing"),
            "honest missing tool vs thinking, got {chrome:?}"
        );
        assert!(
            chrome.contains("prefix cost missing"),
            "honest missing prefix cost, got {chrome:?}"
        );
        assert!(
            !chrome.contains("first reasoning 0"),
            "must not invent a first-reasoning number, got {chrome:?}"
        );
        assert!(
            !chrome.contains("$0.0000 prefix"),
            "must not invent a prefix-cost dollar amount, got {chrome:?}"
        );
    }

    /// Named contract: latency waits of at least 60 seconds use compact
    /// minutes (15m43s / 1h2m), never a raw 943s count.
    #[test]
    fn session_todo_row_shows_latency_waits_in_minutes_when_at_least_sixty_seconds() {
        let mut long = metrics_fixture(Some(1_000), Some(350), 12_500, Some(5_000_000_000));
        long.first_reasoning_token_ms = Some(943_000);
        long.tool_call_ms = Some(943_000);
        long.thinking_ms = Some(3_725_000);
        long.prefix_cost_hint_ticks = Some(1_000_000_000);
        let chrome = format_todo_exec_metrics_chrome(&long);
        assert!(
            chrome.contains("first reasoning 15m43s"),
            "943_000 ms first reasoning is 15m43s, got {chrome:?}"
        );
        assert!(
            chrome.contains("tool 15m43s / thinking 1h2m"),
            "tool 943_000 ms and thinking 3_725_000 ms, got {chrome:?}"
        );
        assert!(
            !chrome.contains("943s"),
            "must not print raw 943s, got {chrome:?}"
        );
        let mut mixed = metrics_fixture(Some(1_000), Some(350), 12_500, Some(5_000_000_000));
        mixed.first_reasoning_token_ms = Some(5_200);
        mixed.tool_call_ms = Some(3_000);
        mixed.thinking_ms = None;
        mixed.prefix_cost_hint_ticks = None;
        let mixed_chrome = format_todo_exec_metrics_chrome(&mixed);
        assert!(
            mixed_chrome.contains("first reasoning 5.2s"),
            "under 60s may stay seconds, got {mixed_chrome:?}"
        );
        assert!(
            mixed_chrome.contains("tool 3.0s / thinking missing"),
            "present tool with missing thinking stays honest, got {mixed_chrome:?}"
        );
        assert!(
            mixed_chrome.contains("prefix cost missing"),
            "honest missing prefix cost, got {mixed_chrome:?}"
        );
    }

    /// Named contract: chrome paints stored latency columns from grok_oss.db
    /// without inventing values when the insert left them unset.
    #[test]
    fn session_todo_paints_stored_latency_columns_fail_open() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = open_at(&tmp.path().join("grok_oss.db")).unwrap();
        let task = store
            .insert_prompt_task("run the latency chrome slice", "queued", None, None)
            .unwrap();
        let record = PromptExecRecord {
            tokens_in: 1_200,
            tokens_out: 400,
            model: "grok-4.6".into(),
            reasoning_effort: Some("medium".into()),
            wall_ms: 12_500,
            estimated_tokens_in: Some(1_000),
            estimated_tokens_out: Some(350),
            estimated_wall_ms: Some(10_000),
            cost_usd_ticks: Some(5_000_000_000),
            first_reasoning_token_ms: Some(180),
            tool_call_ms: Some(3_000),
            thinking_ms: Some(8_000),
            prefix_cost_hint_ticks: Some(1_000_000_000),
        };
        store.insert_prompt_exec_metrics(&task.id, &record).unwrap();
        let loaded = load_todo_exec_metrics_fail_open(&store, &task.id)
            .expect("stored metrics for prompt_task ULID");
        let chrome = format_todo_exec_metrics_chrome(&loaded);
        assert!(chrome.contains("first reasoning 180ms"), "got {chrome:?}");
        assert!(
            chrome.contains("tool 3.0s / thinking 8.0s"),
            "got {chrome:?}"
        );
        assert!(
            chrome.contains("$0.1000 prefix cost as context grows"),
            "got {chrome:?}"
        );
    }
}
