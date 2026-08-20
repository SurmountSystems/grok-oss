//! Lookup exec metrics by prompt-task ULID (session-todo chrome).

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;

use super::{GrokOssStore, PromptExecMetrics};

impl GrokOssStore {
    /// Latest exec-metrics row for a prompt_task ULID. None when missing.
    pub fn load_latest_prompt_exec_metrics_for_prompt_task(
        &self,
        prompt_task_id: &str,
    ) -> Result<Option<PromptExecMetrics>> {
        let id: Option<String> = self
            .connection()
            .query_row(
                "SELECT id FROM prompt_exec_metrics WHERE prompt_task_id = ?1
                 ORDER BY created_at DESC LIMIT 1",
                [prompt_task_id],
                |row| row.get(0),
            )
            .optional()
            .context("lookup prompt_exec_metrics by prompt_task_id")?;
        match id {
            Some(id) => self.load_prompt_exec_metrics(&id),
            None => Ok(None),
        }
    }
}
