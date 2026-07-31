//! Plan-approval chrome restored by the shell after quit + resume.
//!
//! When `exit_plan_mode` is parked and the user quits, the shell persists
//! `awaiting_plan_approval = true` in `plan_mode.json`. On `--continue` the
//! shell re-issues the `x.ai/exit_plan_mode` reverse-request — a real live ACP
//! waiter — so the pager re-shows approval chrome through its normal path with
//! no pager-side disk logic. Approving then leaves plan mode and starts the
//! implement turn.
//!
//! This FAILS without the shell re-park (PR2 product change): no reverse-request
//! reaches the resumed pager, so no approval chrome appears.
//!
//! ## Named contract (soft-park approve path)
//!
//! Soft-park is **non-capturing** for keyboard CTAs (L1 modal-free, 2026-07-29):
//! empty-prompt `a` / `A` / `?` / `s` / `q` / Enter type into the composer or
//! no-op; they do **not** exclusive-approve. Product approve paths after resume:
//!
//! 1. **Mouse** footer soft-park CTAs (primary) — works with draft text too
//! 2. **`/view-plan`** (or status chip) side panel, then empty-prompt `a`
//!
//! This scenario uses path (1): click the painted footer **approve** label.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::wait_for_welcome;
use crate::{ContentController, MousePoint, PtyHarness, pager_binary};

const DEFAULT_ROWS: u16 = 50;
const DEFAULT_COLS: u16 = 120;
const WELCOME_TIMEOUT: Duration = Duration::from_secs(20);
/// Distinct per-turn sentinels: turn 1 seeds the session before quit; turn 2 is
/// the implement turn the shell injects after the resumed approval is approved.
const SETUP_SENTINEL: &str = "GBT3703SETUP";
const IMPLEMENT_SENTINEL: &str = "GBT3703IMPLEMENTED";

const PLAN_BODY: &str = "\
# Plan GBT3703Repro

## Steps
1. Seed plan file on disk
2. Quit pager with the approval parked
3. Resume and expect restored approval chrome
";

/// Regression: the shell re-parks `exit_plan_mode` on resume; approving via the
/// soft-park footer mouse CTA leaves plan mode and starts the implement turn.
pub async fn assert_plan_approval_restored_after_resume() -> Result<()> {
    let content = ContentController::start()
        .await
        .context("start ContentController")?;
    let mut setup_turn = content.expect_agent_turn(
        "initial plan-drafting turn",
        format!("{SETUP_SENTINEL}: drafted a plan for the user to review."),
    );
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
        &[],
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

    // Quit and reap BEFORE seeding so the still-live shell cannot re-persist
    // and clobber the seeded state.
    first.inject_keys(b"\x11").context("ctrl-q once")?;
    first.update(Duration::from_millis(200));
    first.inject_keys(b"\x11").context("ctrl-q confirm")?;
    first.quit().context("reap first pager")?;

    let seeded = seed_parked_approval(content.home()).context("seed parked approval")?;
    assert!(seeded > 0, "no session dir seeded");

    let mut resumed = PtyHarness::spawn_with_content_in_dir(
        &binary,
        DEFAULT_ROWS,
        DEFAULT_COLS,
        &content,
        &["--continue"],
        Some(project.path()),
    )
    .context("spawn resumed pager")?;

    // The shell re-parks `exit_plan_mode` on resume, so soft-park approval chrome
    // can open immediately and cover chat history. Prefer the chrome markers
    // (product signal) over SETUP_SENTINEL, which may not be visible under the
    // plan viewer. Without the shell re-park this times out.
    //
    // Markers match soft-park CTA legend / footer (`PLAN_CARD_CTAS` + Preview
    // hints): "s revise" (not the old "request changes" label) and the card
    // header. Full-screen viewer uses "quit plan"; soft-park uses "q quit".
    resumed
        .wait_for_text("s revise", WELCOME_TIMEOUT)
        .context("restored approval 's revise' after --continue")?;
    resumed
        .wait_for_text("Plan ready for review", Duration::from_secs(5))
        .context("restored plan-ready card after resume")?;
    let screen = resumed.screen_contents();
    if !screen.contains("approve") {
        bail!("expected approval primary action after resume\n{screen}");
    }
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

    // Soft-park is non-capturing: bare `a` types into the composer (see screen
    // dump with `❯ a` under the prompt). Approve via the painted footer CTA —
    // mouse primary product path. First "approve" is the primary button; the
    // second is "approve w/ comment".
    click_screen_text(&mut resumed, "approve", 0).context("click soft-park footer Approve CTA")?;
    resumed
        .wait_for_text(IMPLEMENT_SENTINEL, Duration::from_secs(30))
        .context("footer Approve must leave plan mode and start the implement turn")?;
    tokio::time::timeout(Duration::from_secs(10), implement_turn.wait_satisfied())
        .await
        .context("implement turn expectation timeout")?;

    resumed.quit().context("quit resumed pager")?;
    Ok(())
}

/// Click the `occurrence`-th on-screen match of `text` (0-indexed), SGR mouse.
///
/// Coordinates match scripted runner convention: 0-indexed row/col from the
/// visible screen text snapshot, converted to 1-indexed SGR in the wire bytes.
fn click_screen_text(harness: &mut PtyHarness, text: &str, occurrence: usize) -> Result<()> {
    let point = locate_screen_text(harness, text, occurrence)?;
    // Click slightly into the label (not the leading key glyph) so the hit
    // lands inside the soft-park button rect painted around "approve".
    let col = point.col.saturating_add(1);
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

/// Mark the persisted session as having a parked plan approval: write `plan.md`
/// and flip `awaiting_plan_approval` to `true` in `plan_mode.json` for every
/// session dir under the sandbox home.
fn seed_parked_approval(home: &Path) -> Result<usize> {
    let sessions_root = home.join(".grok").join("sessions");
    if !sessions_root.is_dir() {
        bail!(
            "expected sessions under {} after first turn",
            sessions_root.display()
        );
    }
    let mut seeded = 0usize;
    for cwd_ent in std::fs::read_dir(&sessions_root).context("read sessions root")? {
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
    std::fs::write(path, serde_json::to_vec_pretty(&value)?).context("write plan_mode.json")?;
    Ok(())
}
