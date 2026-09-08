//! Plan-approval chrome restored by the shell after quit + resume.
//!
//! When `exit_plan_mode` is parked and the user quits, the shell persists
//! `awaiting_plan_approval = true` in `plan_mode.json`. The first session
//! scripts that tool call through mock inference so the bundled shell parks
//! a live waiter (ContentController is not ACP and cannot answer
//! `x.ai/exit_plan_mode`). On `--continue` the shell re-issues the reverse-
//! request — a real live ACP waiter — so the pager re-shows approval chrome
//! through its normal path with no pager-side disk logic. Approving then
//! leaves plan mode and starts the implement turn.
//!
//! This FAILS without the shell re-park (PR2 product change): no reverse-request
//! reaches the resumed pager, so no approval chrome appears.
//!
//! ## Named contract (soft-park approve path)
//!
//! Soft-park is **non-capturing** for letter keys: `a` / `A` / `s` / `q` type
//! into the composer or the plan pane box. Empty `?` still arms Clarify.
//! Empty Enter never Approves. A live mid-turn `exit_plan_mode` still
//! auto-opens the plan side panel. Resume / `--continue` parks the waiter
//! without docking and without painting idle "Plan written. Click or
//! /view-plan" while the pane is shut. Open the pane with `/view-plan` or
//! a status click before Approve. Product approve path:
//!
//! 1. **Mouse** click on the painted footer **Approve** word (primary)
//!
//! Footer paint is word-only: `approve  |  comment  |  revise  |  exit`
//! (narrow docks drop separators to spaces). There is no Notes button, no
//! Quit label, and no `a approve` / `A notes` / `s revise` / `q quit` prefix.
//! Do **not** match bare `"approve"` — transcript card prose can contain
//! that substring and is not a hit target. Do **not** fall back to empty
//! Enter even if a shortcut bar still says `Enter:approve`.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::wait_for_welcome;
use crate::{ContentController, MousePoint, PtyHarness, ScriptedResponse, SseEvent, pager_binary};

const DEFAULT_ROWS: u16 = 50;
const DEFAULT_COLS: u16 = 120;
const WELCOME_TIMEOUT: Duration = Duration::from_secs(20);
/// Direct pager↔shell ACP so resume reverse-requests are not dropped by a
/// leader with no ExtMethod waiter. `--trust` skips the folder-trust gate
/// that can stall `--continue` on the welcome recap. `--yolo` skips a
/// permission card on the live `exit_plan_mode` park (mid-turn still
/// auto-docks).
const PAGER_E2E_ARGS: &[&str] = &["--yolo", "--trust", "--no-leader"];
/// Distinct per-turn sentinels: turn 1 seeds the session before quit; turn 2 is
/// the implement turn the shell injects after the resumed approval is approved.
const SETUP_SENTINEL: &str = "GBT3703SETUP";
const IMPLEMENT_SENTINEL: &str = "GBT3703IMPLEMENTED";

/// Word-only approval footer (separator `"  |  "` from `line_viewer` paint).
/// Unique vs card prose (`to approve,`) and vs a lone `"approve"`.
const LABELED_APPROVE_CTA: &str = "approve  |  comment";
/// Full four-CTA strip when the right pane is wide enough for separators.
const LABELED_FOOTER_STRIP: &str = "approve  |  comment  |  revise  |  exit";
/// Narrow dock drops separators; still word-only, no letter prefixes.
const NARROW_FOOTER_STRIP: &str = "approve comment revise exit";

const PLAN_BODY: &str = "\
# Plan GBT3703Repro

## Steps
1. Seed plan file on disk
2. Quit pager with the approval parked
3. Resume and expect restored approval chrome
";

/// Regression: the shell re-parks `exit_plan_mode` on resume; approving via the
/// side-panel footer mouse CTA leaves plan mode and starts the implement turn.
pub async fn assert_plan_approval_restored_after_resume() -> Result<()> {
    let content = ContentController::start()
        .await
        .context("start ContentController")?;
    let mut setup_turn = content.expect_agent_turn(
        "initial plan-drafting turn",
        format!("{SETUP_SENTINEL}: drafted a plan for the user to review."),
    );
    // ContentController is mock inference, not ACP. A text-only first turn
    // never intercepts `exit_plan_mode`, so resume has no reverse-request
    // waiter even if `plan_mode.json` is seeded. Script the tool call so
    // the bundled shell parks a live waiter before quit.
    let _park_turn = expect_exit_plan_mode_turn(&content, "call_gbt3703_park");
    let mut implement_turn = content.expect_agent_turn(
        "implementation after approval",
        format!("{IMPLEMENT_SENTINEL}: implementing the approved plan."),
    );

    let project = tempfile::tempdir().context("project dir")?;
    std::fs::create_dir_all(project.path().join(".git")).context("create .git")?;

    let binary = pager_binary().context("resolve pager binary")?;
    let mut first = PtyHarness::spawn_with_content_in_dir(
        &binary,
        DEFAULT_ROWS,
        DEFAULT_COLS,
        &content,
        PAGER_E2E_ARGS,
        Some(project.path()),
    )
    .context("spawn first pager")?;

    wait_for_welcome(&mut first).await?;

    first.inject_keys(b"go\r").context("submit setup turn")?;
    first
        .wait_for_text(SETUP_SENTINEL, Duration::from_secs(30))
        .context("setup turn rendered")?;
    tokio::time::timeout(Duration::from_secs(10), setup_turn.wait_satisfied())
        .await
        .context("setup turn expectation timeout")?;

    let sessions_root = content.sandbox().grok_home().join("sessions");
    write_plan_md_in_sessions(&sessions_root).context("write plan.md before park")?;

    first
        .inject_keys(b"present the plan\r")
        .context("submit exit_plan_mode park turn")?;
    first
        .wait_for_text("Plan ready. Side panel open", Duration::from_secs(30))
        .context("live exit_plan_mode must park (auto-dock) before quit")?;

    // Quit and reap BEFORE seeding so the still-live shell cannot re-persist
    // and clobber the seeded state.
    first.inject_keys(b"\x11").context("ctrl-q once")?;
    first.update(Duration::from_millis(200));
    first.inject_keys(b"\x11").context("ctrl-q confirm")?;
    first.quit().context("reap first pager")?;

    let seeded = seed_parked_approval(&content.sandbox().grok_home().join("sessions"))
        .context("seed parked approval")?;
    assert!(seeded > 0, "no session dir seeded");

    let mut continue_args = PAGER_E2E_ARGS.to_vec();
    continue_args.insert(0, "--continue");
    let mut resumed = PtyHarness::spawn_with_content_in_dir(
        &binary,
        DEFAULT_ROWS,
        DEFAULT_COLS,
        &content,
        &continue_args,
        Some(project.path()),
    )
    .context("spawn resumed pager")?;

    // The shell re-parks `exit_plan_mode` on resume as a live waiter.
    // Restore must not auto-dock the side panel and must not paint the
    // shut-panel idle click cue. Session restore is the ready signal;
    // `/view-plan` binds Approve. Live mid-turn present still auto-opens.
    wait_for_restored_session(&mut resumed)
        .context("restored session after resume (not idle Plan written chrome)")?;
    if resumed.contains_text("Plan ready. Side panel open") {
        bail!(
            "resume must not auto-dock the plan side panel\n{}",
            resumed.screen_contents()
        );
    }
    if resumed.contains_text("Plan written. Click or /view-plan") {
        bail!(
            "resume must not idle as Plan written. Click or /view-plan while the pane is shut\n{}",
            resumed.screen_contents()
        );
    }

    // Restore can land after the first slash. Keep sending `/view-plan`
    // until the parked waiter binds Approve (Plan ready. Side panel open).
    // The four-word footer alone is not enough: view-only plan.md paints
    // the same strip and Approve is a no-op. Do not auto-dock. Do not wait
    // on idle Plan written chrome.
    wait_for_restored_approve_footer(&mut resumed)
        .context("/view-plan must bind Approve to the restored waiter")?;
    let screen = resumed.screen_contents();
    // History was seeded before quit; plan body from disk is a stronger signal
    // that the session was restored when chrome already covers the transcript.
    if !screen.contains("GBT3703Repro")
        && !screen.contains(SETUP_SENTINEL)
        && !screen.contains("Seed plan file on disk")
    {
        bail!("expected resumed session content (plan body or setup sentinel)\n{screen}");
    }
    if resumed.contains_text("panicked") {
        bail!("pager panicked\n{screen}");
    }

    // Letters type; empty Enter never Approves. After /view-plan, click
    // the painted Approve word (not card prose).
    click_plan_approve_cta(&mut resumed).context("click side-panel Approve CTA")?;
    resumed
        .wait_for_text(IMPLEMENT_SENTINEL, Duration::from_secs(30))
        .context("panel Approve must leave plan mode and start the implement turn")?;
    tokio::time::timeout(Duration::from_secs(10), implement_turn.wait_satisfied())
        .await
        .context("implement turn expectation timeout")?;

    resumed.quit().context("quit resumed pager")?;
    Ok(())
}

/// Click the painted plan-approval Approve word.
///
/// Prefer the separated strip (`approve  |  clarify`). Fall back to the
/// narrow four-word strip. Empty Enter is not an Approve path.
fn click_plan_approve_cta(harness: &mut PtyHarness) -> Result<()> {
    let screen = harness.screen_contents();
    if screen.contains("a approve")
        || screen.contains("A notes")
        || screen.contains("s revise")
        || screen.contains("q quit")
    {
        bail!("old letter-prefixed plan CTAs must not paint\n{screen}");
    }
    if screen.contains(LABELED_APPROVE_CTA) {
        // Inset one cell into "approve" so the hit lands in the button rect.
        return click_screen_text(harness, LABELED_APPROVE_CTA, 0, 1)
            .context("click labeled Approve word");
    }
    if screen.contains(NARROW_FOOTER_STRIP) {
        return click_screen_text(harness, NARROW_FOOTER_STRIP, 0, 1)
            .context("click narrow-dock Approve word");
    }
    bail!(
        "no plan Approve control found (expected '{LABELED_FOOTER_STRIP}' \
         or '{NARROW_FOOTER_STRIP}')\n{screen}"
    )
}

/// Open the restored waiter with `/view-plan` until Approve is bound.
///
/// Success is live-park status after an explicit open: "Plan ready. Side
/// panel open". The word-only footer can paint for view-only plan.md
/// without `plan_approval_view`, and clicking Approve then does nothing.
/// One Enter can race the shell re-park. Retry the slash. Do not treat
/// idle "Plan written. Click or /view-plan" as success.
fn wait_for_restored_approve_footer(harness: &mut PtyHarness) -> Result<()> {
    const SLASH_RETRY: Duration = Duration::from_millis(400);
    const BOUND_APPROVE_STATUS: &str = "Plan ready. Side panel open";
    let deadline = Instant::now() + WELCOME_TIMEOUT;
    let mut last_slash = Instant::now() - SLASH_RETRY;
    loop {
        harness.update(Duration::from_millis(50));
        let screen = harness.screen_contents();
        let footer_painted = screen.contains(LABELED_FOOTER_STRIP)
            || screen.contains(NARROW_FOOTER_STRIP)
            || screen.contains(LABELED_APPROVE_CTA);
        // Do not treat leftover first-session "Plan ready. Side panel open"
        // in scrollback as a bound waiter after `--continue`.
        let recent_status = screen
            .lines()
            .rev()
            .take(16)
            .any(|line| line.contains(BOUND_APPROVE_STATUS));
        if recent_status && footer_painted {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out after {:?} waiting for {BOUND_APPROVE_STATUS:?} with Approve footer\n{screen}",
                WELCOME_TIMEOUT,
            );
        }
        if last_slash.elapsed() >= SLASH_RETRY {
            let composer_holds_slash = screen
                .lines()
                .rev()
                .take(8)
                .any(|line| matches!(line.trim(), "/view-plan" | "/show-plan" | "/plan-view"));
            if composer_holds_slash {
                harness
                    .inject_keys(b"\r")
                    .context("submit in-composer /view-plan")?;
            } else {
                harness
                    .inject_keys(b"/view-plan\r")
                    .context("open restored waiter via /view-plan")?;
            }
            last_slash = Instant::now();
        }
    }
}

/// `--continue` must be inside the restored session, not the welcome recap.
/// Welcome lists "Resume session" and can show the last-turn snippet, which
/// is not a bound waiter.
fn wait_for_restored_session(harness: &mut PtyHarness) -> Result<()> {
    let deadline = Instant::now() + WELCOME_TIMEOUT;
    loop {
        harness.update(Duration::from_millis(50));
        let screen = harness.screen_contents();
        // Welcome recap can show the last-turn snippet (setup sentinel)
        // without the exact "Resume session" label. "New worktree" is
        // welcome-menu only. `/view-plan` on welcome never binds Approve.
        if screen.contains(SETUP_SENTINEL)
            && !screen.contains("Resume session")
            && !screen.contains("New worktree")
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out after {:?} waiting for restored session (setup sentinel, not welcome Resume session / New worktree)\n{screen}",
                WELCOME_TIMEOUT
            );
        }
    }
}

/// Click the `occurrence`-th on-screen match of `text` (0-indexed), SGR mouse.
///
/// Coordinates match scripted runner convention: 0-indexed row/col from the
/// visible screen text snapshot, converted to 1-indexed SGR in the wire bytes.
/// `col_offset` shifts right from the match start (1 = into the Approve word).
fn click_screen_text(
    harness: &mut PtyHarness,
    text: &str,
    occurrence: usize,
    col_offset: u16,
) -> Result<()> {
    let point = locate_screen_text(harness, text, occurrence)?;
    let col = point.col.saturating_add(col_offset);
    let click = format!(
        "{}{}",
        sgr_mouse(0, point.row, col, 'M'),
        sgr_mouse(0, point.row, col, 'm'),
    );
    harness
        .inject_keys(click.as_bytes())
        .with_context(|| format!("inject click at row={} col={col}", point.row))?;
    harness.update(Duration::from_millis(150));
    Ok(())
}

fn locate_screen_text(harness: &PtyHarness, text: &str, occurrence: usize) -> Result<MousePoint> {
    if text.is_empty() {
        bail!("cannot locate empty text");
    }
    let output = harness.screen_output();
    let mut seen = 0usize;
    for (row, line) in output.lines.iter().enumerate() {
        let mut start_byte = 0usize;
        while let Some(rel_byte) = line[start_byte..].find(text) {
            let byte = start_byte + rel_byte;
            if seen == occurrence {
                let col = line[..byte].chars().count();
                return Ok(MousePoint {
                    row: row as u16,
                    col: col as u16,
                });
            }
            seen += 1;
            start_byte = byte + text.len();
        }
    }
    bail!(
        "could not locate occurrence {occurrence} of {text:?} on screen\n{}",
        harness.screen_contents()
    )
}

fn sgr_mouse(button: u16, row: u16, col: u16, suffix: char) -> String {
    format!("\x1b[<{button};{};{}{suffix}", col + 1, row + 1)
}

/// Scripted model turn that invokes `exit_plan_mode` (both pager backends).
fn expect_exit_plan_mode_turn(
    content: &ContentController,
    call_id: &str,
) -> crate::AgentTurnExpectation {
    content.expect_agent_turn_with_responses(
        format!("exit_plan_mode park {call_id}"),
        ScriptedResponse::sse(responses_api_tool_call_events(
            call_id,
            "exit_plan_mode",
            "{}",
        )),
        ScriptedResponse::sse(chat_completions_tool_call_events(
            call_id,
            "exit_plan_mode",
            "{}",
        )),
    )
}

fn responses_api_tool_call_events(call_id: &str, name: &str, arguments: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut seq = 0u64;
    events.push(SseEvent::data(
        serde_json::json!({
            "type": "response.created",
            "sequence_number": seq,
            "response": {
                "id": "resp_plan_park",
                "object": "response",
                "created_at": 1234567890,
                "model": "test-model",
                "status": "in_progress",
                "output": []
            }
        })
        .to_string(),
    ));
    seq += 1;
    events.push(SseEvent::data(
        serde_json::json!({
            "type": "response.function_call_arguments.delta",
            "sequence_number": seq,
            "item_id": call_id,
            "output_index": 0,
            "delta": arguments
        })
        .to_string(),
    ));
    seq += 1;
    events.push(SseEvent::data(
        serde_json::json!({
            "type": "response.completed",
            "sequence_number": seq,
            "response": {
                "id": "resp_plan_park",
                "object": "response",
                "created_at": 1234567890,
                "model": "test-model",
                "status": "completed",
                "output": [{
                    "type": "function_call",
                    "call_id": call_id,
                    "name": name,
                    "arguments": arguments
                }],
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 20,
                    "total_tokens": 30,
                    "input_tokens_details": { "cached_tokens": 0 },
                    "output_tokens_details": { "reasoning_tokens": 0 }
                }
            }
        })
        .to_string(),
    ));
    events.push(SseEvent::data("[DONE]".to_string()));
    events
}

fn chat_completions_tool_call_events(call_id: &str, name: &str, arguments: &str) -> Vec<SseEvent> {
    let tool_calls = vec![serde_json::json!({
        "index": 0,
        "id": call_id,
        "type": "function",
        "function": { "name": name, "arguments": arguments }
    })];
    vec![
        SseEvent::data(
            serde_json::json!({
                "id": "chatcmpl-plan-park",
                "object": "chat.completion.chunk",
                "created": 1234567890,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": tool_calls
                    },
                    "finish_reason": null
                }]
            })
            .to_string(),
        ),
        SseEvent::data(
            serde_json::json!({
                "id": "chatcmpl-plan-park",
                "object": "chat.completion.chunk",
                "created": 1234567890,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": "tool_calls"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 20,
                    "total_tokens": 30
                }
            })
            .to_string(),
        ),
        SseEvent::data("[DONE]".to_string()),
    ]
}

fn write_plan_md_in_sessions(sessions_root: &Path) -> Result<usize> {
    if !sessions_root.is_dir() {
        bail!(
            "expected sessions under {} after first turn",
            sessions_root.display()
        );
    }
    let mut written = 0usize;
    for cwd_ent in std::fs::read_dir(sessions_root).context("read sessions root")? {
        let cwd_ent = cwd_ent.context("cwd entry")?;
        if !cwd_ent.file_type().context("ft")?.is_dir() {
            continue;
        }
        for sess_ent in std::fs::read_dir(cwd_ent.path()).context("read cwd sessions")? {
            let sess_ent = sess_ent.context("session entry")?;
            if !sess_ent.file_type().context("ft")?.is_dir() {
                continue;
            }
            std::fs::write(sess_ent.path().join("plan.md"), PLAN_BODY).context("write plan.md")?;
            written += 1;
        }
    }
    if written == 0 {
        bail!(
            "expected at least one session dir under {}",
            sessions_root.display()
        );
    }
    Ok(written)
}

/// Mark the persisted session as having a parked plan approval: write `plan.md`
/// and flip `awaiting_plan_approval` to `true` in `plan_mode.json` for every
/// session dir under the sandbox `$GROK_HOME/sessions`.
fn seed_parked_approval(sessions_root: &Path) -> Result<usize> {
    if !sessions_root.is_dir() {
        bail!(
            "expected sessions under {} after first turn",
            sessions_root.display()
        );
    }
    let mut seeded = 0usize;
    for cwd_ent in std::fs::read_dir(sessions_root).context("read sessions root")? {
        let cwd_ent = cwd_ent.context("cwd entry")?;
        if !cwd_ent.file_type().context("ft")?.is_dir() {
            continue;
        }
        for sess_ent in std::fs::read_dir(cwd_ent.path()).context("read cwd sessions")? {
            let sess_ent = sess_ent.context("session entry")?;
            if !sess_ent.file_type().context("ft")?.is_dir() {
                continue;
            }
            let dir = sess_ent.path();
            std::fs::write(dir.join("plan.md"), PLAN_BODY).context("write plan.md")?;
            write_awaiting_plan_mode(&dir.join("plan_mode.json"))?;
            seeded += 1;
        }
    }
    if seeded == 0 {
        bail!(
            "expected at least one session dir under {}",
            sessions_root.display()
        );
    }
    Ok(seeded)
}

/// Round-trip the shell-written `plan_mode.json` and flip `awaiting_plan_approval`
/// to `true`, preserving every other field. Falls back to a fresh Active
/// snapshot if the shell wrote nothing. The shape mirrors
/// `xai_grok_shell::session::plan_mode::PlanModeSnapshot`; we only touch the one
/// field (robust to schema growth) rather than depend on the heavy shell crate
/// from this test-only harness.
fn write_awaiting_plan_mode(path: &Path) -> Result<()> {
    let mut value: serde_json::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| {
            serde_json::json!({
                "state": "Active",
                "was_previously_active": true,
                "reminder_count": 0,
                "pending_exit_reminder": false,
            })
        });
    let obj = value
        .as_object_mut()
        .context("plan_mode.json must be a JSON object")?;
    // Must be Active for the re-park; awaiting flag is the trigger.
    obj.insert("state".into(), serde_json::Value::String("Active".into()));
    obj.insert(
        "awaiting_plan_approval".into(),
        serde_json::Value::Bool(true),
    );
    // A leftover resolved bit would make the shell skip re-park. This seed
    // is an outstanding decision, not Approve/Quit.
    obj.insert(
        "plan_decision_resolved".into(),
        serde_json::Value::Bool(false),
    );
    std::fs::write(path, serde_json::to_vec_pretty(&value)?).context("write plan_mode.json")?;
    Ok(())
}
