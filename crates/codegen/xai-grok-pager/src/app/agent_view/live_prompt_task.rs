//! Live composer submit: insert `prompt_task` (ULID) and finish with
//! `prompt_exec_metrics` via the existing grok_oss store.
//!
//! Fail-open when grok_oss.db cannot be opened. Does not invent remaining
//! SuperGrok limits. SuperGrok is a paid product.

use super::AgentView;
use xai_grok_shell::grok_oss::{
    GrokOssStore, LivePromptTask, PromptExecRecord, try_open_from_token_economy_config,
};
use xai_grok_shell::token_economy::token_economy_from_disk;
use xai_grok_shell::tools::TodoItem;

/// Todo meta key for the prompt-as-task ULID. Same string as todo chrome.
const PROMPT_TASK_ID_META_KEY: &str = "promptTaskId";

impl AgentView {
    /// Fail-open open of grok_oss.db from Token Economy config.
    ///
    /// Tests skip unless `grok_oss_database_path` is overridden so crate tests
    /// do not write the operator store.
    fn try_open_live_prompt_store() -> Option<GrokOssStore> {
        let cfg = token_economy_from_disk();
        if cfg!(test) && cfg.grok_oss_database_path.is_none() {
            return None;
        }
        try_open_from_token_economy_config(&cfg)
    }

    /// Composer enqueue: insert `prompt_task` and start HonestWorkClock.
    /// Bound to a client `prompt_id` when the turn actually sends.
    pub(crate) fn start_pending_live_prompt_task(&mut self, body: &str) {
        let store = Self::try_open_live_prompt_store();
        if let Some(task) = LivePromptTask::try_start(store.as_ref(), body) {
            self.pending_live_prompt_tasks.push_back(task);
        }
    }

    /// Attach the oldest pending live prompt_task to the minted prompt_id.
    pub(crate) fn bind_pending_live_prompt_task(&mut self, prompt_id: &str) {
        if let Some(task) = self.pending_live_prompt_tasks.pop_front() {
            self.live_prompt_tasks.insert(prompt_id.to_string(), task);
        }
    }

    /// Immediate send: insert + bind in one step (prompt_id is already minted).
    pub(crate) fn start_and_bind_live_prompt_task(&mut self, prompt_id: &str, body: &str) {
        if self.live_prompt_tasks.contains_key(prompt_id) {
            return;
        }
        let store = Self::try_open_live_prompt_store();
        if let Some(task) = LivePromptTask::try_start(store.as_ref(), body) {
            self.live_prompt_tasks.insert(prompt_id.to_string(), task);
        }
    }

    /// Drain path: bind a pending row, or start+bind if enqueue did not mint one.
    pub(crate) fn bind_or_start_live_prompt_task(&mut self, prompt_id: &str, body: &str) {
        if self.live_prompt_tasks.contains_key(prompt_id) {
            return;
        }
        if self.pending_live_prompt_tasks.is_empty() {
            self.start_and_bind_live_prompt_task(prompt_id, body);
        } else {
            self.bind_pending_live_prompt_task(prompt_id);
        }
    }

    /// ULID for the running (or still-pending) prompt_task, if any.
    pub(crate) fn current_live_prompt_task_id(&self) -> Option<&str> {
        if let Some(pid) = self.session.current_prompt_id.as_deref()
            && let Some(task) = self.live_prompt_tasks.get(pid)
        {
            return Some(task.id.as_str());
        }
        self.pending_live_prompt_tasks
            .front()
            .map(|t| t.id.as_str())
    }

    /// Stamp `meta.promptTaskId` on todos created for this prompt when missing.
    pub(crate) fn stamp_todos_with_live_prompt_task(&self, items: &mut [TodoItem]) {
        let Some(id) = self.current_live_prompt_task_id() else {
            return;
        };
        for item in items {
            let meta = item
                .meta
                .get_or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            let Some(map) = meta.as_object_mut() else {
                continue;
            };
            if map.contains_key(PROMPT_TASK_ID_META_KEY) || map.contains_key("prompt_task_id") {
                continue;
            }
            map.insert(
                PROMPT_TASK_ID_META_KEY.to_string(),
                serde_json::Value::String(id.to_string()),
            );
        }
    }

    /// Pause honest work on every in-flight prompt_task for reconnect.
    pub(crate) fn pause_live_prompt_reconnect(&mut self) {
        for task in self.live_prompt_tasks.values_mut() {
            task.pause_reconnect();
        }
        for task in &mut self.pending_live_prompt_tasks {
            task.pause_reconnect();
        }
    }

    /// Resume honest work after reconnect. No-op when clocks were not paused.
    pub(crate) fn resume_live_prompt_after_reconnect(&mut self) {
        for task in self.live_prompt_tasks.values_mut() {
            task.resume_after_reconnect();
        }
        for task in &mut self.pending_live_prompt_tasks {
            task.resume_after_reconnect();
        }
    }

    /// Write `prompt_exec_metrics` and stop the honest clock. Fail-open.
    pub(crate) fn complete_live_prompt_task(
        &mut self,
        prompt_id: Option<&str>,
        usage_meta: Option<&serde_json::Map<String, serde_json::Value>>,
    ) {
        let Some(pid) = prompt_id else {
            return;
        };
        let Some(task) = self.live_prompt_tasks.remove(pid) else {
            return;
        };
        let record = self.prompt_exec_record_from_usage(usage_meta);
        let store = Self::try_open_live_prompt_store();
        let _ = task.try_finish(store.as_ref(), record);
    }

    fn prompt_exec_record_from_usage(
        &self,
        usage_meta: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> PromptExecRecord {
        let usage = usage_meta.and_then(|m| m.get("usage"));
        let tokens_in = json_i64(usage.and_then(|u| u.get("inputTokens"))).unwrap_or(0);
        let tokens_out = json_i64(usage.and_then(|u| u.get("outputTokens"))).unwrap_or(0);
        let cost_usd_ticks = json_i64(usage.and_then(|u| u.get("costUsdTicks")));
        let model = self
            .session
            .models
            .current_model_id_str()
            .map(str::to_string)
            .or_else(|| self.session.models.current_model_name())
            .unwrap_or_default();
        PromptExecRecord {
            tokens_in,
            tokens_out,
            model,
            reasoning_effort: None,
            wall_ms: 0,
            estimated_tokens_in: None,
            estimated_tokens_out: None,
            estimated_wall_ms: None,
            cost_usd_ticks,
            first_reasoning_token_ms: None,
            tool_call_ms: None,
            thinking_ms: None,
            prefix_cost_hint_ticks: None,
        }
    }
}

impl crate::app::app_view::AppView {
    /// Session disconnect toast: pause honest work on every live/pending
    /// prompt_task. Nested pause at reload start keeps this first timestamp.
    pub(crate) fn handle_session_disconnect_toast(&mut self, attempt: u32) {
        self.show_toast(&format!(
            "Disconnected. Reconnecting... (attempt {attempt})"
        ));
        self.pause_live_prompt_reconnect();
    }

    /// Pause honest work clocks on every agent (and nested subagent view).
    /// No-op when there is no live task / grok_oss.db (fail-open).
    pub(crate) fn pause_live_prompt_reconnect(&mut self) {
        for agent in self.agents.values_mut() {
            agent.pause_live_prompt_reconnect();
            for child in agent.subagent_views.values_mut() {
                child.pause_live_prompt_reconnect();
            }
        }
    }
}

fn json_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
}
