//! `token_report`: estimate vs actual wall and nested tokens.
//!
//! Agents call this tool instead of hand-writing the six-column markdown
//! table. Formats: human-readable table, or TOON. Cost is input plus
//! output from `model_call_cost_for_sessions` (ledger.rs:656; addition
//! 676-679). Do not copy that addition. Live counters belong to L2-B
//! (`token_economy/live.rs`). This crate cannot import `xai-grok-shell`.

use std::sync::Arc;

use crate::register_resource;
use crate::types::output::{TextOutput, ToolOutput};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_io::ToolInput;
use crate::types::tool_metadata::ToolMetadata;
use crate::util::toon::{self, ToolResultFormat};

/// Stable client-facing tool id (registry suffix after GrokBuild).
pub const TOKEN_REPORT_TOOL_NAME: &str = "token_report";

/// L2-B slot order (`token_economy/live.rs` `TokenSlot`). Do not guess.
pub const LIVE_SLOT_INPUT: usize = 0;
/// Output slot. Completion; includes reasoning.
pub const LIVE_SLOT_OUTPUT: usize = 1;
/// Cached slot. Already inside input. Display only. Never add into cost.
pub const LIVE_SLOT_CACHED: usize = 2;
/// Reasoning slot. Already inside output. Display only. Never add into cost.
pub const LIVE_SLOT_REASONING: usize = 3;

/// Output format the agent asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenReportFormat {
    /// Six-column markdown table the Operator reads in chat.
    Human,
    /// Tabular TOON via `xai_grok_tools::util::toon`.
    Toon,
}

/// One job row, plus the roll-up row, before formatting.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenReportJob {
    /// Operator-facing job name. Ordinary English. Not a nickname.
    pub job: String,
    /// Estimate wall in minutes. Format later: minutes / hours / days.
    pub estimate_wall_minutes: f64,
    /// Estimate nested tokens (raw count, not compact).
    pub estimate_nested_tokens: u64,
    /// Actual wall or elapsed, in minutes. Live rows use elapsed.
    pub actual_wall_minutes: f64,
    /// Actual nested tokens. `None` when the output total was not stored.
    /// Finished rows use `model_call_cost_for_sessions` (input plus output).
    /// Live rows use L2-B counters. Do not invent `0`.
    pub actual_nested_tokens: Option<u64>,
    /// True while the job is still running. Trailing `+` on actual nested
    /// tokens. Last column is `still open`.
    pub running: bool,
}

/// Report the tool prints: per-job rows, then one roll-up.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenReport {
    pub jobs: Vec<TokenReportJob>,
}

/// Typed call site for L2-B live counters.
///
/// Product module EXISTS: `crates/codegen/xai-grok-shell/src/token_economy/live.rs`.
/// `xai-grok-tools` cannot import `xai-grok-shell` (shell already depends on
/// tools). The shell crate injects [`TokenReportLive`] into `Resources` and
/// those methods must call B's functions. Do not fork `AtomicU64`. Do not
/// copy ledger math.
pub trait TokenReportLiveSource: Send + Sync {
    /// `live_slot` at live.rs:181. Slot order: Input 0, Output 1, Cached 2,
    /// Reasoning 3.
    fn live_slot(&self, session_id: &str, slot: usize) -> u64;
    /// `live_totals_for_session` at live.rs:188. Display slots. Not the cost.
    fn live_totals_for_session(&self, session_id: &str) -> (u64, u64, u64, u64);
    /// `live_rollup` at live.rs:201. Display slots. Not the cost.
    fn live_rollup(&self, session_ids: &[&str]) -> (u64, u64, u64, u64);
    /// `live_cost` at live.rs:219. Input plus output. `None` when no complete
    /// call. Never a stand-in 0. Never cached. Never reasoning.
    fn live_cost(&self, session_ids: &[&str]) -> Option<u64>;
    /// `model_call_cost_for_sessions` at ledger.rs:656. Returns
    /// `ModelCallCost.input_plus_output`. `None` when no complete row.
    fn model_call_cost_for_sessions(&self, session_ids: &[&str]) -> Result<Option<i64>, String>;
    /// `finish_job_cost` at live.rs:284. Must call
    /// `model_call_cost_for_sessions` with the job's stored session ids.
    fn finish_job_cost(&self, job_ulid: &str) -> Result<Option<i64>, String>;
}

/// Resource the shell crate inserts so this tool can call L2-B.
pub struct TokenReportLive {
    pub source: Arc<dyn TokenReportLiveSource>,
}

register_resource!("grok_build", "TokenReportLive", TokenReportLive);

/// Operator contract sample used by the golden tests.
///
/// Numbers are chosen so wall and nested-token format rules are visible:
/// minutes at or below 60, hours above 60, N.Nk under 1000.0k, N.Nm at
/// 1000.0k or more, a live trailing plus, under, over, and still open.
pub fn contract_fixture() -> TokenReport {
    TokenReport {
        jobs: vec![
            TokenReportJob {
                job: "Token report tool".to_string(),
                estimate_wall_minutes: 24.0,
                estimate_nested_tokens: 112_400,
                actual_wall_minutes: 18.0,
                actual_nested_tokens: Some(98_200),
                running: false,
            },
            TokenReportJob {
                job: "Live token accounting".to_string(),
                estimate_wall_minutes: 36.0,
                estimate_nested_tokens: 80_000,
                actual_wall_minutes: 42.0,
                actual_nested_tokens: Some(91_500),
                running: false,
            },
            TokenReportJob {
                job: "Dock visibility".to_string(),
                estimate_wall_minutes: 90.0,
                estimate_nested_tokens: 20_400_000,
                actual_wall_minutes: 12.0,
                actual_nested_tokens: Some(401_000),
                running: true,
            },
            TokenReportJob {
                job: "roll up".to_string(),
                estimate_wall_minutes: 150.0,
                estimate_nested_tokens: 20_592_400,
                actual_wall_minutes: 72.0,
                actual_nested_tokens: Some(590_700),
                running: true,
            },
        ],
    }
}

const HUMAN_HEADER: &str = "| job | estimate wall | estimate nested tokens | actual wall or elapsed | actual nested tokens | under / over |";
const HUMAN_SEP: &str = "| --- | --- | --- | --- | --- | --- |";
const TOON_FIELDS: &str = "job,estimate_wall,estimate_nested_tokens,actual_wall_or_elapsed,actual_nested_tokens,under_over";

/// Nested tokens: N.Nk if under 1000.0k; N.Nm if 1000.0k or more. Always one decimal.
pub fn format_nested_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}m", count as f64 / 1_000_000.0)
    } else {
        format!("{:.1}k", count as f64 / 1_000.0)
    }
}

/// Wall: minutes if at most 60; N.N hours if over 60 minutes and at most 24 hours;
/// N.N days if over 24 hours.
pub fn format_wall_minutes(minutes: f64) -> String {
    if minutes <= 60.0 {
        format!("{minutes:.1} minutes")
    } else if minutes <= 24.0 * 60.0 {
        format!("{:.1} hours", minutes / 60.0)
    } else {
        format!("{:.1} days", minutes / (24.0 * 60.0))
    }
}

fn under_over(job: &TokenReportJob) -> &'static str {
    if job.running {
        return "still open";
    }
    let wall_over = job.actual_wall_minutes > job.estimate_wall_minutes;
    let tokens_over = job
        .actual_nested_tokens
        .is_some_and(|n| n > job.estimate_nested_tokens);
    if wall_over || tokens_over {
        "over"
    } else if job.actual_nested_tokens.is_some() {
        "under"
    } else {
        "still open"
    }
}

fn actual_nested_cell(job: &TokenReportJob) -> String {
    match job.actual_nested_tokens {
        Some(n) => {
            let mut cell = format_nested_tokens(n);
            if job.running {
                cell.push('+');
            }
            cell
        }
        None => "the output total was not stored".to_string(),
    }
}

struct FormattedRow {
    job: String,
    estimate_wall: String,
    estimate_nested: String,
    actual_wall: String,
    actual_nested: String,
    under_over: &'static str,
}

fn format_row(job: &TokenReportJob) -> FormattedRow {
    FormattedRow {
        job: job.job.clone(),
        estimate_wall: format_wall_minutes(job.estimate_wall_minutes),
        estimate_nested: format_nested_tokens(job.estimate_nested_tokens),
        actual_wall: format_wall_minutes(job.actual_wall_minutes),
        actual_nested: actual_nested_cell(job),
        under_over: under_over(job),
    }
}

fn render_human(report: &TokenReport) -> String {
    let mut out = String::new();
    out.push_str(HUMAN_HEADER);
    out.push('\n');
    out.push_str(HUMAN_SEP);
    out.push('\n');
    for job in &report.jobs {
        let row = format_row(job);
        out.push_str("| ");
        out.push_str(&row.job);
        out.push_str(" | ");
        out.push_str(&row.estimate_wall);
        out.push_str(" | ");
        out.push_str(&row.estimate_nested);
        out.push_str(" | ");
        out.push_str(&row.actual_wall);
        out.push_str(" | ");
        out.push_str(&row.actual_nested);
        out.push_str(" | ");
        out.push_str(row.under_over);
        out.push_str(" |\n");
    }
    out
}

fn render_toon(report: &TokenReport) -> String {
    let n = report.jobs.len();
    let mut out = format!("jobs[{n}]{{{TOON_FIELDS}}}:\n");
    for job in &report.jobs {
        let row = format_row(job);
        out.push_str("  ");
        out.push_str(&row.job);
        out.push(',');
        out.push_str(&row.estimate_wall);
        out.push(',');
        out.push_str(&row.estimate_nested);
        out.push(',');
        out.push_str(&row.actual_wall);
        out.push(',');
        out.push_str(&row.actual_nested);
        out.push(',');
        out.push_str(row.under_over);
        out.push('\n');
    }
    out
}

/// Render the report as a human table or as TOON.
///
/// Golden tests assert exact output against `fixtures/golden_human.md` and
/// `fixtures/golden_toon.toon`. Agents call this tool; they do not hand-write
/// those tables in prompts.
pub fn render_token_report(report: &TokenReport, format: TokenReportFormat) -> String {
    match format {
        TokenReportFormat::Human => render_human(report),
        TokenReportFormat::Toon => render_toon(report),
    }
}

/// Append a roll-up row when the caller did not already include one.
pub fn ensure_rollup(mut report: TokenReport) -> TokenReport {
    if report.jobs.iter().any(|job| job.job == "roll up") {
        return report;
    }
    let running = report.jobs.iter().any(|job| job.running);
    let estimate_wall_minutes: f64 = report
        .jobs
        .iter()
        .map(|job| job.estimate_wall_minutes)
        .sum();
    let estimate_nested_tokens: u64 = report
        .jobs
        .iter()
        .map(|job| job.estimate_nested_tokens)
        .sum();
    let actual_wall_minutes: f64 = report.jobs.iter().map(|job| job.actual_wall_minutes).sum();
    let actual_nested_tokens = {
        let mut sum = 0u64;
        let mut any = false;
        for job in &report.jobs {
            if let Some(n) = job.actual_nested_tokens {
                sum = sum.saturating_add(n);
                any = true;
            }
        }
        any.then_some(sum)
    };
    report.jobs.push(TokenReportJob {
        job: "roll up".to_string(),
        estimate_wall_minutes,
        estimate_nested_tokens,
        actual_wall_minutes,
        actual_nested_tokens,
        running,
    });
    report
}

fn i64_cost_to_u64(cost: Option<i64>) -> Option<u64> {
    cost.and_then(|n| u64::try_from(n).ok())
}

fn actual_nested_from_live(
    live: Option<&dyn TokenReportLiveSource>,
    session_ids: Option<&[String]>,
    job_ulid: Option<&str>,
    running: bool,
    supplied: Option<u64>,
) -> Option<u64> {
    let Some(src) = live else {
        return supplied;
    };
    let ids: Option<Vec<&str>> = session_ids.map(|list| list.iter().map(String::as_str).collect());
    if running {
        return ids
            .as_deref()
            .and_then(|ids| src.live_cost(ids))
            .or(supplied);
    }
    if let Some(ulid) = job_ulid {
        return i64_cost_to_u64(src.finish_job_cost(ulid).ok().flatten()).or(supplied);
    }
    if let Some(ids) = ids.as_deref() {
        return i64_cost_to_u64(src.model_call_cost_for_sessions(ids).ok().flatten()).or(supplied);
    }
    supplied
}

/// Resolve format from the tool argument, then `GROK_TOOL_RESULT_FORMAT`.
///
/// L2-D owns making TOON the harness default. This tool only selects TOON
/// when the argument is `toon` or the env is exactly `toon`.
pub fn resolve_token_report_format(requested: Option<&str>) -> TokenReportFormat {
    match requested.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("toon") => TokenReportFormat::Toon,
        Some("human") | Some("markdown") | Some("table") => TokenReportFormat::Human,
        Some(_) => TokenReportFormat::Human,
        None => match toon::tool_result_format_from_env() {
            ToolResultFormat::Toon => TokenReportFormat::Toon,
            ToolResultFormat::Json | ToolResultFormat::Auto => TokenReportFormat::Human,
        },
    }
}

/// One job the agent passes to the tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TokenReportJobInput {
    /// Operator-facing job name. Ordinary English.
    #[schemars(description = "Operator-facing job name.")]
    pub job: String,
    /// Estimate wall in minutes (raw). The tool formats minutes / hours / days.
    #[schemars(
        description = "Estimate wall in minutes. The tool formats minutes, hours, or days."
    )]
    pub estimate_wall_minutes: f64,
    /// Estimate nested tokens (raw count).
    #[schemars(description = "Estimate nested tokens as a raw count.")]
    pub estimate_nested_tokens: u64,
    /// Actual wall or elapsed, in minutes. Required. Do not copy the estimate.
    #[schemars(
        description = "Actual wall or elapsed minutes. Required. Live rows use elapsed. Do not copy the estimate."
    )]
    pub actual_wall_minutes: Option<f64>,
    /// Optional supplied actual nested tokens when live counters are not wired.
    /// Finished live cost still comes from `model_call_cost_for_sessions`.
    #[serde(default)]
    #[schemars(
        description = "Optional actual nested tokens when live counters are not injected. Omit rather than invent 0."
    )]
    pub actual_nested_tokens: Option<u64>,
    /// Session ids for this job (L2 plus every nested L3). Passed to B's live functions.
    #[serde(default)]
    #[schemars(description = "Session ids for this job: the L2 and every nested L3, once each.")]
    pub session_ids: Option<Vec<String>>,
    /// Job ulid for `finish_job_cost` on a finished job.
    #[serde(default)]
    #[schemars(description = "Job ulid for finish_job_cost on a finished job.")]
    pub job_ulid: Option<String>,
    /// True while the job is still running.
    #[serde(default)]
    #[schemars(
        description = "True while the job is still running. Trailing plus on actual nested tokens."
    )]
    pub running: Option<bool>,
}

/// Input: format plus per-job rows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TokenReportInput {
    /// `human` (markdown table) or `toon`. Default human unless env is `toon`.
    #[serde(default)]
    #[schemars(
        description = "human (markdown table) or toon. Default human unless GROK_TOOL_RESULT_FORMAT=toon."
    )]
    pub format: Option<String>,
    /// Per-job rows. A roll-up row is appended when none is named roll up.
    #[schemars(
        description = "Per-job rows. A roll-up row is appended when none is named roll up."
    )]
    pub jobs: Vec<TokenReportJobInput>,
}

/// Model-facing rendered table or TOON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenReportOutput {
    /// Rendered human table or TOON.
    pub report: String,
    /// Present when live counters were requested but not injected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<String>,
}

impl xai_tool_runtime::ToolOutput for TokenReportOutput {}

impl From<TokenReportInput> for ToolInput {
    fn from(input: TokenReportInput) -> Self {
        ToolInput::Dynamic(serde_json::to_value(input).unwrap_or_else(|_| serde_json::json!({})))
    }
}

impl From<TokenReportOutput> for ToolOutput {
    fn from(o: TokenReportOutput) -> Self {
        let text = match o.waiting {
            Some(waiting) => format!("{}{waiting}\n", o.report),
            None => o.report,
        };
        ToolOutput::Text(TextOutput::from(text))
    }
}

const WAITING_LIVE: &str = "WAITING: L2-B live counters exist at crates/codegen/xai-grok-shell/src/token_economy/live.rs:181 (live_slot), live.rs:201 (live_rollup), live.rs:219 (live_cost), live.rs:284 (finish_job_cost). Cost is model_call_cost_for_sessions at ledger.rs:656. xai-grok-tools cannot import xai-grok-shell. Inject TokenReportLive into Resources from the shell crate.";

fn jobs_from_input(
    inputs: Vec<TokenReportJobInput>,
    live: Option<&dyn TokenReportLiveSource>,
) -> Result<(Vec<TokenReportJob>, bool), xai_tool_runtime::ToolError> {
    let mut wanted_live = false;
    let mut jobs = Vec::new();
    for input in inputs {
        if input.session_ids.is_some() || input.job_ulid.is_some() {
            wanted_live = true;
        }
        let Some(actual_wall_minutes) = input.actual_wall_minutes else {
            return Err(xai_tool_runtime::ToolError::invalid_arguments(format!(
                "actual_wall_minutes is required for job {:?}; elapsed while running, spawn to finish report when done. Do not copy the estimate. Do not invent 0.",
                input.job
            )));
        };
        let running = input.running.unwrap_or(false);
        let actual_nested_tokens = actual_nested_from_live(
            live,
            input.session_ids.as_deref(),
            input.job_ulid.as_deref(),
            running,
            input.actual_nested_tokens,
        );
        jobs.push(TokenReportJob {
            job: input.job,
            estimate_wall_minutes: input.estimate_wall_minutes,
            estimate_nested_tokens: input.estimate_nested_tokens,
            actual_wall_minutes,
            actual_nested_tokens,
            running,
        });
    }
    Ok((jobs, wanted_live))
}

/// Agent tool: estimate versus actual wall and nested tokens.
#[derive(Debug, Default)]
pub struct TokenReportTool;

impl ToolMetadata for TokenReportTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn description_template(&self) -> &str {
        r#"Print estimate versus actual wall time and nested tokens, per job and rolled up.

Call this tool instead of hand-writing the six-column markdown table. Pass `jobs` with job name, estimate_wall_minutes, estimate_nested_tokens, actual_wall_minutes (elapsed while running), optional session_ids (L2 plus every nested L3), optional job_ulid, and running. Pass `format` as `human` (markdown table) or `toon`.

Columns: job, estimate wall, estimate nested tokens, actual wall or elapsed, actual nested tokens, under / over. Spell every number. Never write `same`. Never print `0` for a null. Cost is input plus output from model_call_cost_for_sessions. Cached tokens are already inside input. Reasoning tokens are already inside output. Live nested counts come from L2-B counters, not a second formula."#
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for TokenReportTool {
    type Args = TokenReportInput;
    type Output = TokenReportOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(TOKEN_REPORT_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            TOKEN_REPORT_TOOL_NAME,
            ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.token_report", skip_all)]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: TokenReportInput,
    ) -> Result<TokenReportOutput, xai_tool_runtime::ToolError> {
        if input.jobs.is_empty() {
            return Err(xai_tool_runtime::ToolError::invalid_arguments(
                "jobs must not be empty",
            ));
        }
        let format = resolve_token_report_format(input.format.as_deref());
        let live = match crate::types::tool_metadata::shared_resources(&ctx) {
            Ok(shared) => {
                let guard = shared.lock().await;
                guard
                    .get::<TokenReportLive>()
                    .map(|live| live.source.clone())
            }
            Err(_) => None,
        };
        let live_ref = live.as_deref();
        let (jobs, wanted_live) = jobs_from_input(input.jobs, live_ref)?;
        let report = ensure_rollup(TokenReport { jobs });
        let waiting = if wanted_live && live.is_none() {
            Some(WAITING_LIVE.to_string())
        } else {
            None
        };
        Ok(TokenReportOutput {
            report: render_token_report(&report, format),
            waiting,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::resources::{Cwd, Resources};
    use crate::types::tool_metadata::test_ctx;
    use xai_tool_runtime::Tool;

    const GOLDEN_HUMAN: &str = include_str!("fixtures/golden_human.md");
    const GOLDEN_TOON: &str = include_str!("fixtures/golden_toon.toon");

    fn fixture_input(format: &str) -> TokenReportInput {
        TokenReportInput {
            format: Some(format.to_string()),
            jobs: contract_fixture()
                .jobs
                .into_iter()
                .map(|job| TokenReportJobInput {
                    job: job.job,
                    estimate_wall_minutes: job.estimate_wall_minutes,
                    estimate_nested_tokens: job.estimate_nested_tokens,
                    actual_wall_minutes: Some(job.actual_wall_minutes),
                    actual_nested_tokens: job.actual_nested_tokens,
                    session_ids: None,
                    job_ulid: None,
                    running: Some(job.running),
                })
                .collect(),
        }
    }

    #[test]
    fn golden_human_table_has_operator_contract_columns() {
        assert!(
            GOLDEN_HUMAN.contains("| job | estimate wall | estimate nested tokens | actual wall or elapsed | actual nested tokens | under / over |"),
            "human golden must have the six Operator contract columns; got {GOLDEN_HUMAN}"
        );
        assert!(
            GOLDEN_HUMAN.contains("| Token report tool |"),
            "human golden must include the per-job row; got {GOLDEN_HUMAN}"
        );
        assert!(
            GOLDEN_HUMAN.contains("| roll up |"),
            "human golden must include the roll-up row; got {GOLDEN_HUMAN}"
        );
        assert!(
            GOLDEN_HUMAN.contains("| still open |"),
            "human golden must include still open; got {GOLDEN_HUMAN}"
        );
        assert!(
            !GOLDEN_HUMAN.contains("same"),
            "never write same for a copied figure; got {GOLDEN_HUMAN}"
        );
    }

    #[test]
    fn golden_toon_is_tabular_and_round_trips() {
        let trimmed = GOLDEN_TOON.trim_end();
        assert!(
            trimmed.starts_with(
                "jobs[4]{job,estimate_wall,estimate_nested_tokens,actual_wall_or_elapsed,actual_nested_tokens,under_over}:"
            ),
            "TOON golden must be a four-row tabular jobs array; got {GOLDEN_TOON}"
        );
        let value = crate::util::toon::decode(trimmed)
            .unwrap_or_else(|e| panic!("TOON golden must decode: {e}; body {GOLDEN_TOON}"));
        assert!(
            crate::util::toon::is_tabular_eligible(&value),
            "TOON golden must stay tabular-eligible; got {value:?}"
        );
        let jobs = value
            .get("jobs")
            .and_then(|v| v.as_array())
            .expect("TOON golden jobs array");
        assert_eq!(jobs.len(), 4, "per job plus roll up");
    }

    #[test]
    fn golden_human_table_matches_contract_fixture() {
        let got = render_token_report(&contract_fixture(), TokenReportFormat::Human);
        assert_eq!(
            got, GOLDEN_HUMAN,
            "human table must match fixtures/golden_human.md exactly"
        );
    }

    #[test]
    fn golden_toon_matches_contract_fixture() {
        let got = render_token_report(&contract_fixture(), TokenReportFormat::Toon);
        assert_eq!(
            got, GOLDEN_TOON,
            "TOON must match fixtures/golden_toon.toon exactly"
        );
    }

    #[test]
    fn tool_id_is_stable() {
        assert_eq!(TOKEN_REPORT_TOOL_NAME, "token_report");
        assert_eq!(TokenReportTool.id().as_str(), TOKEN_REPORT_TOOL_NAME);
        assert_eq!(LIVE_SLOT_INPUT, 0);
        assert_eq!(LIVE_SLOT_OUTPUT, 1);
        assert_eq!(LIVE_SLOT_CACHED, 2);
        assert_eq!(LIVE_SLOT_REASONING, 3);
    }

    #[test]
    fn nested_token_and_wall_cells_match_golden_numbers() {
        assert_eq!(format_nested_tokens(112_400), "112.4k");
        assert_eq!(format_nested_tokens(98_200), "98.2k");
        assert_eq!(format_nested_tokens(80_000), "80.0k");
        assert_eq!(format_nested_tokens(91_500), "91.5k");
        assert_eq!(format_nested_tokens(20_400_000), "20.4m");
        assert_eq!(format_nested_tokens(401_000), "401.0k");
        assert_eq!(format_nested_tokens(20_592_400), "20.6m");
        assert_eq!(format_nested_tokens(590_700), "590.7k");
        assert_eq!(format_wall_minutes(24.0), "24.0 minutes");
        assert_eq!(format_wall_minutes(18.0), "18.0 minutes");
        assert_eq!(format_wall_minutes(36.0), "36.0 minutes");
        assert_eq!(format_wall_minutes(42.0), "42.0 minutes");
        assert_eq!(format_wall_minutes(90.0), "1.5 hours");
        assert_eq!(format_wall_minutes(12.0), "12.0 minutes");
        assert_eq!(format_wall_minutes(150.0), "2.5 hours");
        assert_eq!(format_wall_minutes(72.0), "1.2 hours");
    }

    #[test]
    fn rendered_toon_decodes_through_util_toon() {
        let got = render_token_report(&contract_fixture(), TokenReportFormat::Toon);
        let value = crate::util::toon::decode(got.trim_end())
            .unwrap_or_else(|e| panic!("rendered TOON must decode: {e}; body {got}"));
        assert!(crate::util::toon::is_tabular_eligible(&value));
    }

    #[test]
    fn registry_knows_token_report() {
        let builder = crate::registry::types::ToolRegistryBuilder::new();
        assert!(
            builder.has_tool_id("GrokBuild:token_report"),
            "register_all / ToolRegistryBuilder must wire token_report"
        );
    }

    #[tokio::test]
    async fn tool_renders_contract_fixture_human() {
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        let out = TokenReportTool
            .run(test_ctx(resources.into_shared()), fixture_input("human"))
            .await
            .expect("tool ok");
        assert_eq!(out.report, GOLDEN_HUMAN);
        assert!(out.waiting.is_none());
    }

    #[tokio::test]
    async fn tool_renders_contract_fixture_toon() {
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        let out = TokenReportTool
            .run(test_ctx(resources.into_shared()), fixture_input("toon"))
            .await
            .expect("tool ok");
        assert_eq!(out.report, GOLDEN_TOON);
        assert!(out.waiting.is_none());
    }

    struct MockLive {
        cost: Option<u64>,
        finish: Option<i64>,
        model: Option<i64>,
        totals: (u64, u64, u64, u64),
        live_cost_calls: std::sync::Mutex<u32>,
        finish_calls: std::sync::Mutex<u32>,
        model_calls: std::sync::Mutex<u32>,
        live_slot_calls: std::sync::Mutex<u32>,
        live_totals_calls: std::sync::Mutex<u32>,
        live_rollup_calls: std::sync::Mutex<u32>,
    }

    impl MockLive {
        fn input_plus_output() -> Self {
            Self {
                cost: Some(2_000),
                finish: Some(2_000),
                model: Some(2_000),
                totals: (1_500, 500, 200, 100),
                live_cost_calls: std::sync::Mutex::new(0),
                finish_calls: std::sync::Mutex::new(0),
                model_calls: std::sync::Mutex::new(0),
                live_slot_calls: std::sync::Mutex::new(0),
                live_totals_calls: std::sync::Mutex::new(0),
                live_rollup_calls: std::sync::Mutex::new(0),
            }
        }
    }

    impl TokenReportLiveSource for MockLive {
        fn live_slot(&self, _session_id: &str, slot: usize) -> u64 {
            *self.live_slot_calls.lock().expect("live_slot_calls") += 1;
            match slot {
                LIVE_SLOT_INPUT => self.totals.0,
                LIVE_SLOT_OUTPUT => self.totals.1,
                LIVE_SLOT_CACHED => self.totals.2,
                LIVE_SLOT_REASONING => self.totals.3,
                _ => 0,
            }
        }

        fn live_totals_for_session(&self, _session_id: &str) -> (u64, u64, u64, u64) {
            *self.live_totals_calls.lock().expect("live_totals_calls") += 1;
            self.totals
        }

        fn live_rollup(&self, _session_ids: &[&str]) -> (u64, u64, u64, u64) {
            *self.live_rollup_calls.lock().expect("live_rollup_calls") += 1;
            self.totals
        }

        fn live_cost(&self, _session_ids: &[&str]) -> Option<u64> {
            *self.live_cost_calls.lock().expect("live_cost_calls") += 1;
            self.cost
        }

        fn model_call_cost_for_sessions(
            &self,
            _session_ids: &[&str],
        ) -> Result<Option<i64>, String> {
            *self.model_calls.lock().expect("model_calls") += 1;
            Ok(self.model)
        }

        fn finish_job_cost(&self, _job_ulid: &str) -> Result<Option<i64>, String> {
            *self.finish_calls.lock().expect("finish_calls") += 1;
            Ok(self.finish)
        }
    }

    fn one_job_input(
        running: bool,
        session_ids: Option<Vec<String>>,
        job_ulid: Option<String>,
        supplied: Option<u64>,
    ) -> TokenReportInput {
        TokenReportInput {
            format: Some("human".to_string()),
            jobs: vec![TokenReportJobInput {
                job: "Live path job".to_string(),
                estimate_wall_minutes: 24.0,
                estimate_nested_tokens: 3_000,
                actual_wall_minutes: Some(18.0),
                actual_nested_tokens: supplied,
                session_ids,
                job_ulid,
                running: Some(running),
            }],
        }
    }

    #[test]
    fn render_token_report_is_not_empty() {
        let human = render_token_report(&contract_fixture(), TokenReportFormat::Human);
        let toon = render_token_report(&contract_fixture(), TokenReportFormat::Toon);
        assert!(
            !human.is_empty(),
            "human renderer must not stay a TDD-red empty stub"
        );
        assert!(
            !toon.is_empty(),
            "TOON renderer must not stay a TDD-red empty stub"
        );
        assert_eq!(human, GOLDEN_HUMAN);
        assert_eq!(toon, GOLDEN_TOON);
    }

    #[tokio::test]
    async fn tool_waiting_when_session_ids_without_live() {
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        let out = TokenReportTool
            .run(
                test_ctx(resources.into_shared()),
                one_job_input(
                    true,
                    Some(vec!["sess-l2".to_string(), "sess-l3".to_string()]),
                    None,
                    Some(9_999),
                ),
            )
            .await
            .expect("tool ok");
        assert!(
            out.waiting.is_some(),
            "session_ids without TokenReportLive must set WAITING"
        );
        assert!(
            out.waiting
                .as_deref()
                .is_some_and(|w| w.contains("live.rs:181") && w.contains("ledger.rs:656")),
            "WAITING must cite B live.rs and ledger.rs; got {:?}",
            out.waiting
        );
        assert!(
            out.report.contains("Live path job"),
            "WAITING path still prints the table; got {}",
            out.report
        );
    }

    #[tokio::test]
    async fn tool_running_job_uses_live_cost_not_cached_or_reasoning() {
        let mock = Arc::new(MockLive::input_plus_output());
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        resources.insert(TokenReportLive {
            source: mock.clone(),
        });
        let out = TokenReportTool
            .run(
                test_ctx(resources.into_shared()),
                one_job_input(
                    true,
                    Some(vec!["sess-l2".to_string(), "sess-l3".to_string()]),
                    None,
                    Some(9_999),
                ),
            )
            .await
            .expect("tool ok");
        assert!(out.waiting.is_none(), "injected live must not WAITING");
        assert_eq!(*mock.live_cost_calls.lock().expect("live_cost_calls"), 1);
        assert_eq!(*mock.finish_calls.lock().expect("finish_calls"), 0);
        assert_eq!(*mock.model_calls.lock().expect("model_calls"), 0);
        assert!(
            out.report.contains("| 2.0k+ |"),
            "running actual nested tokens must be live_cost 1500+500=2000 (2.0k+), not supplied 9999, not 1500+500+200+100; got {}",
            out.report
        );
        assert!(
            !out.report.contains("| 10.0k+") && !out.report.contains("2.8k"),
            "must not print supplied 9999 as 10.0k+ or cached+reasoning 2.8k; got {}",
            out.report
        );
        assert!(
            out.report.contains("| still open |"),
            "running job last column is still open; got {}",
            out.report
        );
    }

    #[tokio::test]
    async fn tool_finished_job_ulid_uses_finish_job_cost() {
        let mock = Arc::new(MockLive::input_plus_output());
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        resources.insert(TokenReportLive {
            source: mock.clone(),
        });
        let out = TokenReportTool
            .run(
                test_ctx(resources.into_shared()),
                one_job_input(
                    false,
                    None,
                    Some("01JOBCOSTFINISH00000000001".to_string()),
                    Some(9_999),
                ),
            )
            .await
            .expect("tool ok");
        assert!(out.waiting.is_none());
        assert_eq!(*mock.finish_calls.lock().expect("finish_calls"), 1);
        assert_eq!(*mock.live_cost_calls.lock().expect("live_cost_calls"), 0);
        assert!(
            out.report.contains("| 2.0k |"),
            "finished job_ulid actual nested tokens must be finish_job_cost 2000; got {}",
            out.report
        );
        assert!(
            out.report.contains("| under |"),
            "2000 actual vs 3000 estimate is under; got {}",
            out.report
        );
    }

    #[tokio::test]
    async fn tool_finished_session_ids_use_model_call_cost_for_sessions() {
        let mock = Arc::new(MockLive::input_plus_output());
        let mut resources = Resources::new();
        resources.insert(Cwd(std::path::PathBuf::from("/tmp")));
        resources.insert(TokenReportLive {
            source: mock.clone(),
        });
        let out = TokenReportTool
            .run(
                test_ctx(resources.into_shared()),
                one_job_input(
                    false,
                    Some(vec!["sess-l2".to_string(), "sess-l3".to_string()]),
                    None,
                    Some(9_999),
                ),
            )
            .await
            .expect("tool ok");
        assert!(out.waiting.is_none());
        assert_eq!(*mock.model_calls.lock().expect("model_calls"), 1);
        assert_eq!(*mock.live_cost_calls.lock().expect("live_cost_calls"), 0);
        assert_eq!(*mock.finish_calls.lock().expect("finish_calls"), 0);
        assert!(
            out.report.contains("| 2.0k |"),
            "finished session_ids actual nested tokens must be model_call_cost_for_sessions 2000; got {}",
            out.report
        );
    }

    #[test]
    fn mock_live_slot_order_matches_b_token_slot() {
        let mock = MockLive::input_plus_output();
        assert_eq!(mock.live_slot("s", LIVE_SLOT_INPUT), 1_500);
        assert_eq!(mock.live_slot("s", LIVE_SLOT_OUTPUT), 500);
        assert_eq!(mock.live_slot("s", LIVE_SLOT_CACHED), 200);
        assert_eq!(mock.live_slot("s", LIVE_SLOT_REASONING), 100);
        assert_eq!(mock.live_cost(&["s"]), Some(2_000));
        let (input, output, cached, reasoning) = mock.live_totals_for_session("s");
        assert_eq!((input, output, cached, reasoning), (1_500, 500, 200, 100));
        assert_eq!(mock.live_rollup(&["s", "s"]), (1_500, 500, 200, 100));
        assert_ne!(
            input
                .saturating_add(output)
                .saturating_add(cached)
                .saturating_add(reasoning),
            2_000,
            "adding cached or reasoning would be a second cost formula"
        );
    }
}
