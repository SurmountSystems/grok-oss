//! Session transcript replay: line peeks, rewind-aware prepare, and bounded
//! streaming load.
//!
//! Invariants:
//! - InProgress `tool_call_update` peeks are typed serde (unknown fields
//!   ignored) so `content` / `rawOutput` are not allocated.
//! - Production child/fork replay streams one typed ACP update at a time
//!   ([`stream_replay_updates_at`]); [`load_updates_for_replay_at`] stays a
//!   typed materialize-all reference for tests.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use agent_client_protocol as acp;

use super::relocation::RelocationView;
use super::{
    RawLinePeek, RawParamsPeek, RawUpdatePeek, SessionUpdate, SessionUpdateEnvelope,
    filter_rewind_lines, replay_updates_path_in_dir, strip_context_wrappers,
};
use crate::extensions::notification::SessionNotification;
use crate::extensions::notification::SessionUpdate as XaiUpdate;
use crate::sampling::ConversationItem;
use crate::session::wire_tags::{
    AVAILABLE_COMMANDS_UPDATE, TOOL_CALL_STATUS_IN_PROGRESS, TOOL_CALL_UPDATE,
};

// `_meta` protocol field names (not enum discriminants).
/// `_meta` key holding the running token count. The serde `rename` below must
/// match it by hand (serde attrs can't reference a const).
const TOTAL_TOKENS_KEY: &str = "totalTokens";
/// `_meta` key holding the per-event id used for cursor-based reconnect.
const EVENT_ID_KEY: &str = "eventId";

/// What to do when cwd hints miss `updates.jsonl`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReplayLookupFallback {
    #[default]
    Relocation,
    HintedOnly,
}

/// Optional location hints so child `updates.jsonl` lookup can skip a full
/// `~/.grok/sessions` RelocationView scan.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReplayPathHint<'a> {
    /// Parent session working directory; tried as
    /// `<sessions>/<encoded_cwd>/<child_id>/updates.jsonl`.
    pub parent_cwd: Option<&'a Path>,
    /// Child working directory when it differs from the parent (worktree /
    /// custom cwd). Tried before [`Self::parent_cwd`].
    pub child_cwd: Option<&'a Path>,
    /// When cwd hints miss: scan via [`RelocationView`], or return None.
    pub fallback: ReplayLookupFallback,
}

#[doc(hidden)]
pub struct PreparedReplay<'a> {
    /// Rewind-filtered replay lines, each borrowed from the input transcript.
    pub lines: Vec<&'a str>,
    pub(crate) mark_replay: bool,
    pub(crate) last_tokens: u64,
    /// Highest `eventId` counter across all live (rewind-filtered) lines, used
    /// to re-seed the process-global event counter on resume so post-load live
    /// events keep monotonically increasing ids (see
    /// [`crate::util::event_id::ensure_event_counter_at_least`]). `None` when no
    /// line carried a parseable `eventId` (older shell).
    pub(crate) max_event_seq: Option<u64>,
    pub(crate) total_live: usize,
    /// Replayed spawns with no matching finish (a rewind can drop the finish):
    /// `(subagent_id, child_session_id)`, reconciled on load.
    pub(crate) unfinished_subagents: Vec<(String, String)>,
}

/// One live replay line located by byte offset so `session/load` does not
/// hold the whole `updates.jsonl` as one `String` (iso mill resume was 1.3GiB).
#[derive(Debug, Clone, Copy)]
pub struct ReplayLineLoc {
    pub offset: u64,
    pub len: u32,
}

/// Offset plan for streaming `session/load` replay. Pass 1 classifies and
/// rewind-filters; pass 2 seeks one survivor line at a time.
#[derive(Debug, Clone)]
pub struct ReplayFilePlan {
    pub lines: Vec<ReplayLineLoc>,
    pub mark_replay: bool,
    pub last_tokens: u64,
    pub max_event_seq: Option<u64>,
    pub total_live: usize,
    pub unfinished_subagents: Vec<(String, String)>,
    pub end_offset: u64,
    /// True when a surviving line is a user or agent message chunk.
    pub has_user_or_agent_chunk: bool,
}

/// Whether a replay stream forwarded any update. Gates the caller's
/// post-replay memory purge: `Empty` means nothing was reclaimable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum ReplayEmission {
    Emitted,
    Empty,
}

/// Collapses ToolCall + ToolCallUpdates into one ToolCall during replay.
/// Parent forward never flushes leftovers (cursor/`_meta.eventId` contract).
/// Child stream EOF calls [`Self::take_pending`] so start-only tools still hydrate.
pub(crate) struct ReplayToolCollapser {
    pending: HashMap<acp::ToolCallId, acp::ToolCall>,
}

impl ReplayToolCollapser {
    pub(crate) fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Ingest one ACP update. `None` means hold or drop (do not forward yet).
    pub(crate) fn push(&mut self, update: acp::SessionUpdate) -> Option<acp::SessionUpdate> {
        match update {
            acp::SessionUpdate::AvailableCommandsUpdate(_) => None,
            acp::SessionUpdate::ToolCall(tc) => {
                if matches!(
                    tc.status,
                    acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed
                ) {
                    return Some(acp::SessionUpdate::ToolCall(tc));
                }
                self.pending.insert(tc.tool_call_id.clone(), tc);
                None
            }
            acp::SessionUpdate::ToolCallUpdate(mut u) => match u.fields.status {
                Some(acp::ToolCallStatus::Completed) | Some(acp::ToolCallStatus::Failed) => {
                    if let Some(mut base) = self.pending.remove(&u.tool_call_id) {
                        base.update(std::mem::take(&mut u.fields));
                        return Some(acp::SessionUpdate::ToolCall(base));
                    }
                    Some(acp::SessionUpdate::ToolCallUpdate(u))
                }
                None => {
                    if let Some(base) = self.pending.get_mut(&u.tool_call_id) {
                        base.update(std::mem::take(&mut u.fields));
                    }
                    None
                }
                Some(_) => None,
            },
            other => Some(other),
        }
    }

    /// Child-stream EOF: emit ToolCalls that never saw Completed/Failed.
    pub(crate) fn take_pending(&mut self) -> impl Iterator<Item = acp::SessionUpdate> {
        std::mem::take(&mut self.pending)
            .into_values()
            .map(acp::SessionUpdate::ToolCall)
    }

    #[cfg(test)]
    pub(crate) fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// Load replay-ready typed ACP updates for a session, or `None` when the
/// session or its `updates.jsonl` is missing.
pub fn load_updates_for_replay(
    session_id: &str,
) -> std::io::Result<Option<Vec<acp::SessionUpdate>>> {
    let Some(session_dir) =
        crate::session::persistence::find_persisted_session_dir_by_id_result(session_id)?
    else {
        return Ok(None);
    };
    let Some(updates_path) = replay_updates_path_in_dir(&session_dir) else {
        return Ok(None);
    };
    Ok(Some(collect_replay_updates(&updates_path)?))
}

/// Like [`load_updates_for_replay`], but resolves the session under a specific
/// grok home. Typed, materialize-all replay reader: collects every update into
/// owned `Vec`s. Production forwards replay through [`stream_replay_updates_at`]
/// to bound peak memory, so this has no production caller and is compiled only
/// for tests: the `testkit_synth_roundtrip` and `session_load_perf` parity
/// references and the in-crate relocation tests.
#[cfg(any(test, feature = "test-support"))]
pub fn load_updates_for_replay_at(
    session_id: &str,
    grok_home: &std::path::Path,
) -> std::io::Result<Option<Vec<acp::SessionUpdate>>> {
    let Some(updates_path) =
        resolve_replay_updates_path(session_id, grok_home, ReplayPathHint::default())?
    else {
        return Ok(None);
    };
    Ok(Some(collect_replay_updates(&updates_path)?))
}

/// Collect every replay-ready ACP update from `updates_path` into a `Vec`, the
/// materializing counterpart of the streaming [`for_each_replay_update_in_file`].
fn collect_replay_updates(
    updates_path: &std::path::Path,
) -> std::io::Result<Vec<acp::SessionUpdate>> {
    let mut acp_updates: Vec<acp::SessionUpdate> = Vec::new();
    for_each_replay_update_in_file(updates_path, |u| acp_updates.push(u))?;
    Ok(acp_updates)
}

fn is_safe_session_id_component(session_id: &str) -> bool {
    let mut parts = Path::new(session_id).components();
    matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
}

fn try_fast_replay_updates_path(
    session_id: &str,
    grok_home: &Path,
    hint: ReplayPathHint<'_>,
) -> Option<PathBuf> {
    if !is_safe_session_id_component(session_id) {
        return None;
    }
    for cwd in [hint.child_cwd, hint.parent_cwd].into_iter().flatten() {
        let encoded = xai_grok_config::encode_cwd_dirname(&cwd.to_string_lossy());
        let candidate = grok_home.join("sessions").join(encoded).join(session_id);
        if let Some(path) = replay_updates_path_in_dir(&candidate) {
            return Some(path);
        }
    }
    None
}

/// Resolve `updates.jsonl` for `session_id` under `grok_home`, or `None` when
/// the session directory or the file is missing. Shared by the typed
/// `load_updates_for_replay_at` and the streaming [`stream_replay_updates_at`].
pub(crate) fn resolve_replay_updates_path(
    session_id: &str,
    grok_home: &std::path::Path,
    hint: ReplayPathHint<'_>,
) -> std::io::Result<Option<std::path::PathBuf>> {
    match try_fast_replay_updates_path(session_id, grok_home, hint) {
        Some(path) if !super::relocation::has_relocation_journal(grok_home, session_id) => {
            return Ok(Some(path));
        }
        Some(_journaled) => {
            // Journal is authority; hinted source may be stale.
        }
        None if hint.fallback == ReplayLookupFallback::HintedOnly => {
            // Hinted miss: skip RelocationView (UI-thread scan).
            return Ok(None);
        }
        None => {}
    }
    let sessions_root = grok_home.join("sessions");
    let view = RelocationView::load_for_sessions_root(&sessions_root).map_err(io::Error::other)?;
    let Some(session_dir) = view
        .find_persisted_session_dir(session_id)
        .map_err(io::Error::other)?
    else {
        return Ok(None);
    };
    Ok(replay_updates_path_in_dir(&session_dir))
}

/// Invoke `f` once per client-replay ACP update for a session under `grok_home`,
/// never building the full typed `Vec`. Skips ACU / InProgress lines before
/// full notification serde and collapses ToolCall+updates. The typed
/// [`load_updates_for_replay_at`] reference does not skip those.
///
/// `Empty` folds missing-session, missing-file, and no-ACP-updates.
/// I/O errors from reading the file still propagate.
pub fn stream_replay_updates_at<F: FnMut(acp::SessionUpdate)>(
    session_id: &str,
    grok_home: &std::path::Path,
    f: F,
) -> std::io::Result<ReplayEmission> {
    stream_replay_updates_at_hinted(session_id, grok_home, ReplayPathHint::default(), f)
}

/// Plan replay of `updates.jsonl` without slurping the file into one String.
///
/// Named contract: last-session resume must paint Operator/Agent lines. A
/// gigabyte `updates.jsonl` must not block chrome-only for minutes because
/// `read_to_string` copied the whole file first.
pub fn plan_replay_file(updates_path: &Path, cursor: Option<&str>) -> io::Result<ReplayFilePlan> {
    plan_replay_file_inner(updates_path, cursor, true)
}

fn plan_replay_file_inner(
    updates_path: &Path,
    cursor: Option<&str>,
    drop_redundant: bool,
) -> io::Result<ReplayFilePlan> {
    let file = File::open(updates_path)?;
    let end_offset = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut buf = String::new();
    let mut offset: u64 = 0;
    let mut locs: Vec<ReplayLineLoc> = Vec::new();
    let mut steps: Vec<super::RewindStep> = Vec::new();
    let mut event_ids: Vec<Option<String>> = Vec::new();
    let mut tokens: Vec<Option<u64>> = Vec::new();
    let mut dropped: Vec<bool> = Vec::new();
    let mut is_user_or_agent: Vec<bool> = Vec::new();
    let mut unfinished: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let mut max_event_seq: Option<u64> = None;
    let mut has_rewind = false;

    loop {
        buf.clear();
        let n = reader.read_line(&mut buf)?;
        if n == 0 {
            break;
        }
        let line_len = n as u32;
        let line = buf.trim();
        if line.is_empty() {
            offset += u64::from(line_len);
            continue;
        }
        let step = super::rewind_step_for_line(line);
        if matches!(step, super::RewindStep::Rewind { .. }) {
            has_rewind = true;
        }
        if line.contains("subagent_spawned") || line.contains("subagent_finished") {
            update_unfinished_subagents(line, &mut unfinished);
        }
        if line.contains(EVENT_ID_KEY)
            && let Some(seq) = line_event_seq(line)
        {
            max_event_seq = Some(max_event_seq.map_or(seq, |m| m.max(seq)));
        }
        locs.push(ReplayLineLoc {
            offset,
            len: line_len,
        });
        steps.push(step);
        event_ids.push(line_event_id(line).map(|s| s.into_owned()));
        tokens.push(line_total_tokens(line));
        dropped.push(drop_redundant && line_is_dropped_on_replay(line));
        is_user_or_agent
            .push(line.contains("user_message_chunk") || line.contains("agent_message_chunk"));
        offset += u64::from(line_len);
    }

    let live_idx: Vec<usize> = if has_rewind {
        super::filter_rewind_by((0..locs.len()).collect(), |&i| steps[i])
    } else {
        (0..locs.len()).collect()
    };

    let last_tokens = live_idx.iter().rev().find_map(|&i| tokens[i]).unwrap_or(0);

    let cursor_pos = cursor.and_then(|id| {
        live_idx
            .iter()
            .rposition(|&i| event_ids[i].as_deref() == Some(id))
            .filter(|&pos| {
                live_idx[pos + 1..]
                    .iter()
                    .all(|&i| dropped[i] || event_ids[i].is_some())
            })
    });
    let mark_replay = cursor_pos.is_none();
    let start = cursor_pos.map_or(0, |pos| pos + 1);

    let mut lines = Vec::new();
    let mut total_live = 0usize;
    let mut has_user_or_agent_chunk = false;
    for (pos, &i) in live_idx.iter().enumerate() {
        if dropped[i] {
            continue;
        }
        total_live += 1;
        if pos >= start {
            if is_user_or_agent[i] {
                has_user_or_agent_chunk = true;
            }
            lines.push(locs[i]);
        }
    }

    Ok(ReplayFilePlan {
        lines,
        mark_replay,
        last_tokens,
        max_event_seq,
        total_live,
        unfinished_subagents: unfinished.into_iter().collect(),
        end_offset,
        has_user_or_agent_chunk,
    })
}

fn line_event_seq(line: &str) -> Option<u64> {
    line_event_id(line)?.rsplit('-').next()?.parse().ok()
}

fn update_unfinished_subagents(
    line: &str,
    pending: &mut std::collections::BTreeMap<String, String>,
) {
    let raw = serde_json::from_str::<RawLinePeek<'_>>(line)
        .ok()
        .and_then(|e| e.params.map(|p| p.get()))
        .unwrap_or(line);
    let Ok(notification) = serde_json::from_str::<SessionNotification>(raw) else {
        return;
    };
    match notification.update {
        XaiUpdate::SubagentSpawned {
            subagent_id,
            child_session_id,
            ..
        } => {
            pending.insert(subagent_id, child_session_id);
        }
        XaiUpdate::SubagentFinished { subagent_id, .. } => {
            pending.remove(&subagent_id);
        }
        _ => {}
    }
}

/// Operator/Agent UI lines from `chat_history.jsonl` when `updates.jsonl`
/// has no `user_message_chunk` / `agent_message_chunk` (fork parent after
/// occupancy drop, or a failed huge-file replay).
///
/// Screenshot contract: 119K/500K with an empty scrollback is a fail.
pub fn chat_history_replay_lines(session_id: &str, items: &[ConversationItem]) -> Vec<String> {
    let mut lines = Vec::new();
    for item in items {
        let (tag, text) = match item {
            ConversationItem::User(u) => {
                if u.synthetic_reason.is_some() {
                    continue;
                }
                ("user_message_chunk", item.text_content())
            }
            ConversationItem::Assistant(_) => ("agent_message_chunk", item.text_content()),
            _ => continue,
        };
        if text.trim().is_empty() {
            continue;
        }
        let text_json = serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".to_string());
        lines.push(format!(
            r#"{{"timestamp":0,"method":"session/update","params":{{"sessionId":{sid},"update":{{"sessionUpdate":"{tag}","content":{{"type":"text","text":{text}}}}}}}}}"#,
            sid = serde_json::to_string(session_id).unwrap_or_else(|_| "\"\"".to_string()),
            tag = tag,
            text = text_json,
        ));
    }
    lines
}

/// Read one planned replay line. The buffer is reused by the caller.
pub fn read_replay_line_at(
    file: &mut File,
    loc: ReplayLineLoc,
    buf: &mut String,
) -> io::Result<()> {
    buf.clear();
    file.seek(SeekFrom::Start(loc.offset))?;
    let mut bytes = vec![0u8; loc.len as usize];
    file.read_exact(&mut bytes)?;
    buf.push_str(
        std::str::from_utf8(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
    );
    Ok(())
}

/// [`stream_replay_updates_at`] with parent/child cwd hints so child hydrate
/// can skip a full sessions-root scan on the common encoded-cwd path.
pub fn stream_replay_updates_at_hinted<F: FnMut(acp::SessionUpdate)>(
    session_id: &str,
    grok_home: &std::path::Path,
    hint: ReplayPathHint<'_>,
    mut f: F,
) -> std::io::Result<ReplayEmission> {
    let Some(updates_path) = resolve_replay_updates_path(session_id, grok_home, hint)? else {
        return Ok(ReplayEmission::Empty);
    };
    let plan = plan_replay_file(&updates_path, None)?;
    let mut file = File::open(&updates_path)?;
    let mut buf = String::new();
    let mut collapser = ReplayToolCollapser::new();
    let mut forwarded = false;
    for loc in plan.lines {
        read_replay_line_at(&mut file, loc, &mut buf)?;
        let line = buf.trim();
        if line.is_empty() || line_is_dropped_on_replay(line) {
            continue;
        }
        match SessionUpdateEnvelope::from_str(line) {
            Ok(SessionUpdate::Acp(notif)) => {
                let update = strip_context_wrappers(notif.update);
                if let Some(update) = collapser.push(update) {
                    forwarded = true;
                    f(update);
                }
            }
            Ok(SessionUpdate::Xai(_)) => {}
            Err(e) => tracing::debug!(error = %e, "skipping unparseable replay line"),
        }
    }
    for update in collapser.take_pending() {
        forwarded = true;
        f(update);
    }
    Ok(if forwarded {
        ReplayEmission::Emitted
    } else {
        ReplayEmission::Empty
    })
}

/// Typed-load core: rewind-filter and forward every ACP update (including ACU).
/// Not used by [`stream_replay_updates_at`] (that path peeks + collapses).
pub(crate) fn for_each_replay_update_in_file<F: FnMut(acp::SessionUpdate)>(
    updates_path: &std::path::Path,
    mut f: F,
) -> std::io::Result<bool> {
    let plan = plan_replay_file_inner(updates_path, None, false)?;
    let mut file = File::open(updates_path)?;
    let mut buf = String::new();
    let mut forwarded = false;
    for loc in plan.lines {
        read_replay_line_at(&mut file, loc, &mut buf)?;
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }
        match SessionUpdateEnvelope::from_str(line) {
            Ok(SessionUpdate::Acp(notif)) => {
                forwarded = true;
                f(strip_context_wrappers(notif.update));
            }
            Ok(SessionUpdate::Xai(_)) => {}
            Err(e) => tracing::debug!(error = %e, "skipping unparseable replay line"),
        }
    }
    Ok(forwarded)
}

/// Unpaired spawns across the rewind-filtered timeline. Substring pre-filter
/// keeps non-subagent lines off the JSON path.
pub(crate) fn collect_unfinished_subagents(filtered: &[&str]) -> Vec<(String, String)> {
    let mut pending: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for line in filtered {
        if !line.contains("subagent_spawned") && !line.contains("subagent_finished") {
            continue;
        }
        let raw = serde_json::from_str::<RawLinePeek<'_>>(line)
            .ok()
            .and_then(|e| e.params.map(|p| p.get()))
            .unwrap_or(line);
        let Ok(notification) = serde_json::from_str::<SessionNotification>(raw) else {
            continue;
        };
        match notification.update {
            XaiUpdate::SubagentSpawned {
                subagent_id,
                child_session_id,
                ..
            } => {
                pending.insert(subagent_id, child_session_id);
            }
            XaiUpdate::SubagentFinished { subagent_id, .. } => {
                pending.remove(&subagent_id);
            }
            _ => {}
        }
    }
    pending.into_iter().collect()
}

/// The raw `_meta` object of a persisted line, if any, without allocating a
/// `serde_json::Value`. Handles both the enveloped (`{method,params}`) and legacy
/// (params-at-top-level) on-disk formats.
fn line_meta(line: &str) -> Option<&serde_json::value::RawValue> {
    let env = serde_json::from_str::<RawLinePeek<'_>>(line).ok()?;
    let raw = env.params.map(|p| p.get()).unwrap_or(line);
    serde_json::from_str::<RawParamsPeek<'_>>(raw).ok()?.meta
}

/// Catalog lines stay on disk but are re-advertised after every `session/load`,
/// so replay skips them. Typed peek ignores the huge `availableCommands` array.
pub(crate) fn line_is_available_commands_update(line: &str) -> bool {
    line.contains(&*AVAILABLE_COMMANDS_UPDATE)
        && peek_line_update(line).is_some_and(|u| u.session_update == *AVAILABLE_COMMANDS_UPDATE)
}

/// Fat 100ms bash `tool_call_update`s: typed peek of `sessionUpdate` + `status`
/// only (unknown fields ignored). Completed/Failed and `status: None` stay.
pub(crate) fn line_is_in_progress_tool_call_update(line: &str) -> bool {
    if !line.contains(&*TOOL_CALL_UPDATE) || !line.contains(&*TOOL_CALL_STATUS_IN_PROGRESS) {
        return false;
    }
    peek_line_update(line).is_some_and(|u| {
        u.session_update == *TOOL_CALL_UPDATE
            && u.status == Some(TOOL_CALL_STATUS_IN_PROGRESS.as_str())
    })
}

fn peek_line_update(line: &str) -> Option<RawUpdatePeek<'_>> {
    let env = serde_json::from_str::<RawLinePeek<'_>>(line).ok()?;
    let raw = env.params.map(|p| p.get()).unwrap_or(line);
    serde_json::from_str::<RawParamsPeek<'_>>(raw).ok()?.update
}

pub(crate) fn line_is_dropped_on_replay(line: &str) -> bool {
    line_is_available_commands_update(line) || line_is_in_progress_tool_call_update(line)
}

/// Extract `_meta.totalTokens` from a persisted update line without allocating a
/// `serde_json::Value`. Returns `None` when the line carries no token count.
fn line_total_tokens(line: &str) -> Option<u64> {
    if !line.contains(TOTAL_TOKENS_KEY) {
        return None;
    }
    #[derive(serde::Deserialize)]
    struct TokensPeek {
        #[serde(rename = "totalTokens")]
        total_tokens: Option<u64>,
    }
    serde_json::from_str::<TokensPeek>(line_meta(line)?.get())
        .ok()
        .and_then(|t| t.total_tokens)
}

/// This line's `_meta.eventId`, if any. Cheap peek (no `Value`).
fn line_event_id(line: &str) -> Option<std::borrow::Cow<'_, str>> {
    if !line.contains(EVENT_ID_KEY) {
        return None;
    }
    #[derive(serde::Deserialize)]
    struct EventIdPeek<'a> {
        #[serde(rename = "eventId", borrow)]
        event_id: Option<std::borrow::Cow<'a, str>>,
    }
    serde_json::from_str::<EventIdPeek<'_>>(line_meta(line)?.get())
        .ok()
        .and_then(|e| e.event_id)
}

/// Does this line's `_meta.eventId` equal `cursor_id`?
fn line_has_event_id(line: &str, cursor_id: &str) -> bool {
    line_event_id(line).as_deref() == Some(cursor_id)
}

/// Rewind-filter, resolve the reconnect cursor, drop redundant command
/// catalogs and InProgress tool_call_updates, and scan `totalTokens`. Pure
/// data processing, no I/O.
///
/// The cursor is resolved before dropping ACUs / InProgress lines, because an
/// idle client often reconnects with one of those `eventId`s as its cursor;
/// resolving against the inclusive set keeps reconnect incremental instead of
/// a full replay.
///
/// `#[doc(hidden)] pub` (not stable API): production replay uses it, and the
/// session-load memory test drives it to check the peek stays zero-copy.
#[doc(hidden)]
pub fn prepare_replay_lines<'a>(contents: &'a str, cursor: Option<&str>) -> PreparedReplay<'a> {
    let filtered = filter_rewind_lines(contents.lines().filter(|l| !l.trim().is_empty()).collect());

    let mut max_event_seq: Option<u64> = None;
    for line in &filtered {
        if line.contains("eventId")
            && let Ok(env) = serde_json::from_str::<RawLinePeek<'_>>(line)
            && let Some(raw) = env.params.map(|p| p.get())
            && let Ok(pp) = serde_json::from_str::<RawParamsPeek<'_>>(raw)
            && let Some(meta_raw) = pp.meta
            && let Ok(meta) = serde_json::from_str::<serde_json::Value>(meta_raw.get())
            && let Some(seq) = meta
                .get("eventId")
                .and_then(|v| v.as_str())
                .and_then(|s| s.rsplit('-').next())
                .and_then(|c| c.parse::<u64>().ok())
        {
            max_event_seq = Some(max_event_seq.map_or(seq, |m| m.max(seq)));
        }
    }

    let last_tokens = filtered
        .iter()
        .rev()
        .find_map(|l| line_total_tokens(l))
        .unwrap_or(0);

    let cursor_pos = cursor
        .and_then(|id| filtered.iter().rposition(|l| line_has_event_id(l, id)))
        .filter(|&pos| {
            let bounded = filtered[pos + 1..]
                .iter()
                .all(|l| line_is_dropped_on_replay(l) || line_event_id(l).is_some());
            if !bounded {
                tracing::warn!(
                    "replay: post-cursor tail contains eventId-less lines; full replay instead"
                );
            }
            bounded
        });
    let mark_replay = cursor_pos.is_none();
    let start = cursor_pos.map_or(0, |pos| pos + 1);

    let mut lines: Vec<&str> = Vec::with_capacity(filtered.len().saturating_sub(start));
    let mut total_live = 0usize;
    for (i, &line) in filtered.iter().enumerate() {
        if line_is_dropped_on_replay(line) {
            continue;
        }
        total_live += 1;
        if i >= start {
            lines.push(line);
        }
    }

    PreparedReplay {
        lines,
        mark_replay,
        last_tokens,
        max_event_seq,
        total_live,
        unfinished_subagents: collect_unfinished_subagents(&filtered),
    }
}

/// Blank-strip, drop redundant command catalogs and InProgress tool updates,
/// and rewind-filter a raw `updates.jsonl` segment. Shared by the delta-replay
/// path (which has no reconnect cursor); the initial replay path is
/// [`prepare_replay_lines`], which additionally resolves a cursor (and so must
/// see ACUs / InProgress lines) before dropping them.
pub(crate) fn filter_delta_replay_lines(contents: &str) -> Vec<&str> {
    let live: Vec<&str> = contents
        .lines()
        .filter(|l| !l.trim().is_empty() && !line_is_dropped_on_replay(l))
        .collect();
    filter_rewind_lines(live)
}
