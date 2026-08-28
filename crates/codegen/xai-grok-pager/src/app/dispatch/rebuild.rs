//! `/rebuild` TaskResult handling: report, cancel mid-turn, arm self re-exec.
//!
//! Peer TUIs (other live product windows) receive `SIGUSR1` **after** a
//! successful install and arm re-exec via
//! [`try_arm_peer_rebuild_relaunch_from_request`]. A failed `just install` /
//! verify must not signal peers.
//!
//! **Exit-path contract:** every event-loop exit that can race with peer
//! rebuild (SIGUSR1 quit notify, leader IPC disconnect) must call
//! [`arm_peer_rebuild_before_exit`] so the window re-execs onto the new binary
//! instead of only quitting.

use super::router::dispatch;
use super::turn::do_cancel_turn_for;
use crate::app::actions::{Action, Effect};
use crate::app::agent::AgentId;
use crate::app::app_view::{AppView, RebuildRelaunch};
use crate::scrollback::block::RenderBlock;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// Running-process identity used to decide whether a peer rebuild request is
/// newer than this binary (package version + optional git SHA).
fn running_binary_identity() -> String {
    xai_grok_update::format_build_id(
        env!("CARGO_PKG_VERSION"),
        option_env!("GROK_GIT_SHA").unwrap_or("unknown"),
    )
}

/// Pure decision + path check for peer re-exec (unit-tested).
///
/// Returns the `RebuildRelaunch` to arm when the request is fresh, the installed
/// exe exists, and a session id is available.
///
/// - `signaled == false` (opportunistic): also requires older identity and/or
///   deleted/different running path (anti-thrash after a successful re-exec).
/// - `signaled == true` (`SIGUSR1` received): identity/path gates are skipped.
///   The signal is the operator intent; peers must re-exec, not only quit.
pub(crate) fn peer_rebuild_relaunch_if_applicable(
    self_identity: &str,
    request: &xai_grok_update::RebuildRelaunchRequest,
    now_secs: u64,
    session_id: Option<&str>,
    minimal: bool,
    current_exe: Option<&std::path::Path>,
    signaled: bool,
) -> Option<RebuildRelaunch> {
    if signaled {
        if !xai_grok_update::peer_rebuild_request_is_actionable(request, now_secs) {
            return None;
        }
    } else if !xai_grok_update::should_peer_relaunch_for_request_with_current_exe(
        self_identity,
        request,
        now_secs,
        current_exe,
    ) {
        return None;
    }
    if !request.installed_exe.is_file() {
        return None;
    }
    let session_id = session_id?.to_string();
    if session_id.is_empty() {
        return None;
    }
    Some(RebuildRelaunch {
        session_id,
        installed_exe: request.installed_exe.clone(),
        minimal,
    })
}

/// Prompt text for continue-interrupted-turn on rebuild relaunch.
///
/// Prefer the in-flight rewind stash. After first server activity that stash
/// is cleared, so fall back to the last real user prompt in scrollback.
/// Skip bash/cron bubbles so a `!` or scheduled line is not re-queued as
/// the interrupted turn.
fn rebuild_cancel_resume_prompt(agent: &crate::app::agent_view::AgentView) -> Option<String> {
    if let Some(stashed) = agent.session.in_flight_prompt.as_ref() {
        let text = stashed.text.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    last_user_prompt_full_text(&agent.scrollback)
}

fn last_user_prompt_full_text(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> Option<String> {
    let len = scrollback.len();
    for idx in (0..len).rev() {
        let Some(entry) = scrollback.entry(idx) else {
            continue;
        };
        if let RenderBlock::UserPrompt(block) = &entry.block {
            if block.is_bash || block.is_cron {
                continue;
            }
            let text = block.text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

/// Write `canceled_turn_resume.json` for every mid-turn agent before re-exec.
///
/// Session load already applies that marker. Rebuild must persist it: cancel
/// and peer SIGUSR1 quit do not write the file.
fn persist_running_turns_for_rebuild(app: &AppView) {
    use xai_grok_shell::session::canceled_turn_resume::{
        ProcessShutdownResumeArm, arm_and_persist_process_shutdown_cancel_resume,
    };
    for agent in app.agents.values() {
        if !agent.session.state.is_turn_running() {
            continue;
        }
        let Some(session_id) = agent.session.session_id.as_ref().map(|s| s.0.to_string()) else {
            continue;
        };
        let Some(prompt_text) = rebuild_cancel_resume_prompt(agent) else {
            continue;
        };
        arm_and_persist_process_shutdown_cancel_resume(ProcessShutdownResumeArm {
            cwd: agent.session.cwd.to_string_lossy().into_owned(),
            session_id,
            prompt_text,
            prompt_id: agent.session.current_prompt_id.clone(),
        });
    }
}

/// Resolve session id for peer re-exec (active agent first, then any agent).
fn peer_rebuild_session_id(app: &AppView) -> Option<String> {
    app.active_session_id()
        .map(|s| s.to_string())
        .or_else(|| {
            app.agents
                .values()
                .find_map(|a| a.session.session_id.as_ref().map(|s| s.0.to_string()))
        })
        .filter(|s| !s.is_empty())
}

/// After `SIGUSR1` (or opportunistic leader-disconnect recovery), arm re-exec
/// onto the newly installed binary for the active session when the request
/// applies.
///
/// `signaled`: true when this process received cooperative rebuild `SIGUSR1`
/// (force path: fresh request + exe + session is enough). Mid-turn
/// continue-interrupted-turn is persisted here before quit. The quit path
/// does not write `canceled_turn_resume.json`.
/// Returns `true` when `app.rebuild_relaunch` was set.
pub(crate) fn try_arm_peer_rebuild_relaunch_from_request(
    app: &mut AppView,
    signaled: bool,
) -> bool {
    if app.rebuild_relaunch.is_some() {
        return true;
    }
    let Some(request) = xai_grok_update::read_rebuild_relaunch_request() else {
        if signaled {
            tracing::warn!(
                "peer rebuild SIGUSR1 received but no rebuild_relaunch_request.json under grok home"
            );
        }
        return false;
    };
    let self_identity = running_binary_identity();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let session_id = peer_rebuild_session_id(app);
    let current_exe = std::env::current_exe().ok();
    let Some(relaunch) = peer_rebuild_relaunch_if_applicable(
        &self_identity,
        &request,
        now,
        session_id.as_deref(),
        app.screen_mode.is_minimal(),
        current_exe.as_deref(),
        signaled,
    ) else {
        if signaled {
            tracing::warn!(
                self_identity = %self_identity,
                installed = %request.installed_exe.display(),
                has_session = session_id.is_some(),
                "peer rebuild SIGUSR1: request present but re-exec not armed"
            );
        }
        return false;
    };
    if let crate::app::app_view::ActiveView::Agent(agent_id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&agent_id)
    {
        agent.show_toast("Rebuild on another window: relaunching on the new binary…");
    }
    persist_running_turns_for_rebuild(app);
    app.rebuild_relaunch = Some(relaunch);
    true
}

/// Why the event loop is arming peer rebuild on the way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeerRebuildExitReason {
    /// `SIGUSR1` quit-notify path, or final drain of a leftover flag.
    /// Only re-execs when the peer-rebuild flag was set (or already armed).
    SignalOrFlag,
    /// Leader IPC cancelled. Also tries identity/path gates without the flag
    /// so a leader `RelaunchForUpdate` race still picks up the request.
    LeaderDisconnect,
}

/// Call on event-loop exits that can race with peer rebuild.
///
/// Named contract: a peer that received rebuild `SIGUSR1` must arm re-exec,
/// not only exit. Leader IPC cancel is higher priority in the biased select
/// than quit-notify; without arming there, peers quit and never come back.
///
/// Does **not** re-exec on ordinary SIGTERM/`/exit` when no rebuild flag is
/// set (avoids fighting a deliberate kill while a request file is still fresh).
pub(crate) fn arm_peer_rebuild_before_exit(
    app: &mut AppView,
    reason: PeerRebuildExitReason,
) -> bool {
    let signaled = crate::app::signal_handler::take_peer_rebuild_relaunch();
    if try_arm_peer_rebuild_relaunch_from_request(app, signaled) {
        return true;
    }
    // Leader may have drained for RelaunchForUpdate before SIGUSR1 was
    // observed, or cancel won the race with the flag already cleared. Still
    // pick up a fresh request under identity/path gates.
    if matches!(reason, PeerRebuildExitReason::LeaderDisconnect) && !signaled {
        return try_arm_peer_rebuild_relaunch_from_request(app, false);
    }
    false
}

/// Handle [`crate::app::actions::TaskResult::RebuildDone`].
pub(super) fn handle_rebuild_done(
    app: &mut AppView,
    agent_id: AgentId,
    result: Result<Box<xai_grok_update::RebuildReport>, String>,
) -> Vec<Effect> {
    match result {
        Err(error) => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent.rebuild_progress = None;
                agent.show_toast("Rebuild failed");
                agent.scrollback.push_block(RenderBlock::system(format!(
                    "Rebuild failed (no other sessions were asked to quit or re-exec):\n{error}"
                )));
            }
            vec![]
        }
        Ok(report) => {
            let summary = report.summary_lines.join("\n");
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                // Snap bar to 100% for one frame's worth of toast, then clear
                // the dedicated strip so relaunch chrome is clean.
                agent.rebuild_progress = Some(crate::app::agent_view::RebuildUiProgress {
                    fraction: 1.0,
                    detail: format!("Installed {}", report.installed_identity),
                });
                agent.show_toast(&format!("Installed {}", report.installed_identity));
                agent.scrollback.push_block(RenderBlock::system(summary));
                agent.rebuild_progress = None;
            }

            let mut effects = Vec::new();

            // Persist continue-interrupted-turn before cancel. Cancel does not
            // write the marker; first-activity also clears the in-flight stash.
            persist_running_turns_for_rebuild(app);

            // Mid-turn: cancel so the old process does not keep driving the turn.
            if let Some(agent) = app.agents.get(&agent_id)
                && agent.session.state.is_turn_running()
            {
                effects.extend(do_cancel_turn_for(app, agent_id, true, true));
            }

            // Arm self re-exec onto the new binary with the same session when possible.
            let session_id = app
                .agents
                .get(&agent_id)
                .and_then(|a| a.session.session_id.as_ref())
                .map(|s| s.0.to_string());
            if let Some(session_id) = session_id {
                app.rebuild_relaunch = Some(RebuildRelaunch {
                    session_id,
                    installed_exe: report.installed_path.clone(),
                    minimal: app.screen_mode.is_minimal(),
                });
                if let Some(agent) = app.agents.get_mut(&agent_id) {
                    agent.show_toast("Relaunching this session on the new binary…");
                }
                effects.extend(unregister_and_quit(app));
            } else {
                if let Some(agent) = app.agents.get_mut(&agent_id) {
                    agent.scrollback.push_block(RenderBlock::system(format!(
                        "Binary installed at {}. No active session id to re-exec; \
                         restart with: {} --resume <session>",
                        report.installed_path.display(),
                        report.installed_path.display()
                    )));
                }
            }
            effects
        }
    }
}

fn unregister_and_quit(app: &mut AppView) -> Vec<Effect> {
    // Reuse Quit path so active_sessions unregister runs.
    dispatch(Action::QuitConfirmed, app)
}

/// Whether a post-restore re-exec (rebuild or screen-mode) is allowed.
///
/// When `restore_terminal` reports failure, the writer drain may have failed
/// while teardown still ran, or modes may be only partially cleaned. Re-exec
/// onto a new binary in that state is the half-restored TUI glitch: raw mode
/// / alt-screen / mouse latched under a fresh process. Contract: fail loud
/// with a resume hint instead of `exec`.
pub(crate) fn may_exec_relaunch_after_restore(restore_succeeded: bool) -> bool {
    restore_succeeded
}

/// What the quit tail should do after `restore_terminal`.
///
/// Rebuild re-exec (new binary, same session) wins over screen-mode re-exec
/// when both are set. A failed `/rebuild` never arms rebuild, so this stays
/// `None` and the session remains invokable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PostRestoreRelaunch {
    None,
    ExecRebuild,
    ExecScreenMode,
    BlockedRebuild,
    BlockedScreenMode,
}

/// Pure quit-tail decision used by `app::run` after restore.
pub(crate) fn post_restore_relaunch_action(
    restore_succeeded: bool,
    has_rebuild: bool,
    has_screen_mode: bool,
) -> PostRestoreRelaunch {
    if has_rebuild {
        if may_exec_relaunch_after_restore(restore_succeeded) {
            PostRestoreRelaunch::ExecRebuild
        } else {
            PostRestoreRelaunch::BlockedRebuild
        }
    } else if has_screen_mode {
        if may_exec_relaunch_after_restore(restore_succeeded) {
            PostRestoreRelaunch::ExecScreenMode
        } else {
            PostRestoreRelaunch::BlockedScreenMode
        }
    } else {
        PostRestoreRelaunch::None
    }
}

/// User-visible stderr line after restore, before rebuild `exec`.
///
/// Contract: **none**. Toast + scrollback already told the operator the
/// relaunch is coming. A line between leave-alt-screen and the new process
/// flashes on the primary screen (the classic post-`/rebuild` glitch).
/// Screen-mode relaunch keeps its mode-switch message; rebuild does not.
pub(crate) fn rebuild_relaunch_post_restore_user_message(
    _relaunch: &RebuildRelaunch,
) -> Option<String> {
    None
}

/// Pure rebuild re-exec plan: same argv rebuild + `GROK_SCREEN_MODE` as
/// `/minimal` ↔ `/fullscreen` ([`crate::app::screen_mode_relaunch`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RebuildRelaunchPlan {
    pub exe: PathBuf,
    pub args: Vec<OsString>,
    pub screen_mode_env: &'static str,
}

/// Build argv + env for re-exec onto the installed binary (no process spawn).
pub(crate) fn plan_rebuild_relaunch(
    relaunch: &RebuildRelaunch,
    current_args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> RebuildRelaunchPlan {
    use crate::app::screen_mode_relaunch::{
        build_screen_mode_relaunch_args, screen_mode_env_value,
    };

    RebuildRelaunchPlan {
        exe: relaunch.installed_exe.clone(),
        args: build_screen_mode_relaunch_args(current_args, &relaunch.session_id, relaunch.minimal),
        screen_mode_env: screen_mode_env_value(relaunch.minimal),
    }
}

/// Re-exec the newly installed binary into `session_id` (after terminal restore).
///
/// Caller must only invoke this when [`may_exec_relaunch_after_restore`] is true.
pub(crate) fn exec_rebuild_relaunch(relaunch: &RebuildRelaunch) -> std::io::Result<()> {
    use crate::app::screen_mode_relaunch::GROK_SCREEN_MODE_ENV;
    use std::io::Write;

    let plan = plan_rebuild_relaunch(relaunch, std::env::args_os());
    let mut cmd = std::process::Command::new(&plan.exe);
    cmd.args(&plan.args);
    cmd.env(GROK_SCREEN_MODE_ENV, plan.screen_mode_env);

    // No user-visible stderr after restore. Contract is unit-tested via
    // `rebuild_relaunch_post_restore_user_message` (must stay None).
    debug_assert!(
        rebuild_relaunch_post_restore_user_message(relaunch).is_none(),
        "rebuild must not flash stderr after leave-alt-screen"
    );
    tracing::info!(
        exe = %plan.exe.display(),
        session_id = %relaunch.session_id,
        screen_mode = plan.screen_mode_env,
        "rebuild relaunch: exec onto installed binary"
    );
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(std::io::Error::other(format!(
            "failed to exec rebuild relaunch: {err}"
        )))
    }

    #[cfg(not(unix))]
    {
        cmd.stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
        let _ = cmd.spawn()?;
        std::process::exit(0);
    }
}

/// Fail-loud lines when restore failed and rebuild re-exec is blocked.
pub(crate) fn print_rebuild_restore_blocked_hint(
    relaunch: &RebuildRelaunch,
    cleanup_error: &impl std::fmt::Display,
    w: &mut impl std::io::Write,
) {
    use crate::app::screen_mode_relaunch::screen_mode_relaunch_resume_hint;
    let _ = writeln!(
        w,
        "Terminal cleanup failed after /rebuild ({cleanup_error})."
    );
    let _ = writeln!(
        w,
        "Not relaunching on the new binary with terminal modes possibly latched."
    );
    let _ = writeln!(
        w,
        "Binary is installed at {}.",
        relaunch.installed_exe.display()
    );
    let _ = writeln!(w, "Resume this session with:");
    let _ = writeln!(
        w,
        "  {}",
        screen_mode_relaunch_resume_hint(&relaunch.session_id, relaunch.minimal)
    );
}

/// Fail-loud lines when rebuild `exec` itself fails (restore had succeeded).
pub(crate) fn print_rebuild_exec_failure_hint(
    relaunch: &RebuildRelaunch,
    error: &impl std::fmt::Display,
    w: &mut impl std::io::Write,
) {
    use crate::app::screen_mode_relaunch::screen_mode_relaunch_resume_hint;
    let _ = writeln!(w, "Failed to relaunch on new binary: {error}");
    let _ = writeln!(
        w,
        "Binary is installed at {}.",
        relaunch.installed_exe.display()
    );
    let _ = writeln!(w, "Resume this session with:");
    let _ = writeln!(
        w,
        "  {}",
        screen_mode_relaunch_resume_hint(&relaunch.session_id, relaunch.minimal)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    fn sample_relaunch(minimal: bool) -> RebuildRelaunch {
        RebuildRelaunch {
            session_id: "sess-1".into(),
            installed_exe: PathBuf::from("/tmp/grok-oss-new"),
            minimal,
        }
    }

    #[test]
    fn rebuild_relaunch_struct_holds_paths() {
        let r = sample_relaunch(false);
        assert_eq!(r.session_id, "sess-1");
        assert!(!r.minimal);
        assert_eq!(r.installed_exe, PathBuf::from("/tmp/grok-oss-new"));
    }

    /// Contract: peer of a rebuild arms re-exec when identity is older and the
    /// installed binary path exists (all live product windows, not only invoker).
    #[test]
    fn peer_rebuild_relaunch_if_applicable_arms_when_older_and_exe_exists() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("grok-oss");
        std::fs::File::create(&exe)
            .unwrap()
            .write_all(b"stub")
            .unwrap();
        let req =
            xai_grok_update::make_rebuild_relaunch_request(exe.clone(), "0.2.120 (newsha)", 1_000);
        let armed = peer_rebuild_relaunch_if_applicable(
            "0.2.120 (oldsha)",
            &req,
            1_000,
            Some("sess-peer"),
            false,
            Some(exe.as_path()),
            false,
        )
        .expect("must arm peer re-exec");
        assert_eq!(armed.session_id, "sess-peer");
        assert_eq!(armed.installed_exe, exe);
        assert!(!armed.minimal);
    }

    #[test]
    fn peer_rebuild_relaunch_if_applicable_skips_equal_identity_same_path() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("grok-oss");
        std::fs::write(&exe, b"stub").unwrap();
        let req =
            xai_grok_update::make_rebuild_relaunch_request(exe.clone(), "0.2.120 (samesha)", 1_000);
        assert!(
            peer_rebuild_relaunch_if_applicable(
                "0.2.120 (samesha)",
                &req,
                1_000,
                Some("sess"),
                false,
                Some(exe.as_path()),
                false,
            )
            .is_none()
        );
    }

    /// Contract (operator failure): peer received SIGUSR1 and must re-exec even
    /// when compile-time identity matches the install (same-commit rebuild) and
    /// the path looks equal. Without this, peers quit and never come back.
    #[test]
    fn peer_rebuild_signaled_arms_even_when_identity_and_path_equal() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("grok-oss");
        std::fs::write(&exe, b"stub").unwrap();
        let req =
            xai_grok_update::make_rebuild_relaunch_request(exe.clone(), "0.2.120 (samesha)", 1_000);
        let armed = peer_rebuild_relaunch_if_applicable(
            "0.2.120 (samesha)",
            &req,
            1_000,
            Some("sess-peer"),
            false,
            Some(exe.as_path()),
            true, // SIGUSR1 received
        )
        .expect("signaled peer must re-exec, not only quit");
        assert_eq!(armed.session_id, "sess-peer");
        assert_eq!(armed.installed_exe, exe);
    }

    #[test]
    fn peer_rebuild_signaled_skips_stale_request() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("grok-oss");
        std::fs::write(&exe, b"stub").unwrap();
        let req =
            xai_grok_update::make_rebuild_relaunch_request(exe.clone(), "0.2.120 (samesha)", 1_000);
        let now = 1_000 + 15 * 60 + 1;
        assert!(
            peer_rebuild_relaunch_if_applicable(
                "0.2.120 (samesha)",
                &req,
                now,
                Some("sess"),
                false,
                Some(exe.as_path()),
                true,
            )
            .is_none()
        );
    }

    #[test]
    fn peer_rebuild_relaunch_if_applicable_arms_deleted_inode() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("grok-oss");
        std::fs::write(&exe, b"stub").unwrap();
        let req =
            xai_grok_update::make_rebuild_relaunch_request(exe.clone(), "0.2.120 (samesha)", 1_000);
        let deleted = PathBuf::from(format!("{} (deleted)", exe.display()));
        let armed = peer_rebuild_relaunch_if_applicable(
            "0.2.120 (samesha)",
            &req,
            1_000,
            Some("sess-peer"),
            true,
            Some(deleted.as_path()),
            false,
        )
        .expect("deleted inode must arm re-exec");
        assert!(armed.minimal);
        assert_eq!(armed.session_id, "sess-peer");
    }

    #[test]
    fn peer_rebuild_relaunch_if_applicable_skips_missing_exe() {
        let req = xai_grok_update::make_rebuild_relaunch_request(
            PathBuf::from("/no/such/grok-oss-binary-for-test"),
            "0.2.120 (newsha)",
            1_000,
        );
        assert!(
            peer_rebuild_relaunch_if_applicable(
                "0.2.120 (oldsha)",
                &req,
                1_000,
                Some("sess"),
                false,
                Some(Path::new("/no/such/old")),
                false,
            )
            .is_none()
        );
        // Signaled path also requires the installed exe to exist.
        assert!(
            peer_rebuild_relaunch_if_applicable(
                "0.2.120 (oldsha)",
                &req,
                1_000,
                Some("sess"),
                false,
                Some(Path::new("/no/such/old")),
                true,
            )
            .is_none()
        );
    }

    /// Contract: failed restore must not re-exec (half-restored TUI glitch).
    #[test]
    fn may_exec_relaunch_blocked_when_restore_failed() {
        assert!(
            !may_exec_relaunch_after_restore(false),
            "restore failure must block rebuild/screen-mode re-exec"
        );
    }

    #[test]
    fn may_exec_relaunch_allowed_when_restore_ok() {
        assert!(may_exec_relaunch_after_restore(true));
    }

    /// Contract: no post-restore stderr chatter for rebuild (parity gap vs
    /// intentional screen-mode mode-switch message).
    #[test]
    fn rebuild_relaunch_has_no_post_restore_user_stderr() {
        let r = sample_relaunch(false);
        assert_eq!(
            rebuild_relaunch_post_restore_user_message(&r),
            None,
            "rebuild must not flash a line on the primary screen after leave-alt-screen"
        );
        let r_min = sample_relaunch(true);
        assert_eq!(rebuild_relaunch_post_restore_user_message(&r_min), None);
    }

    /// Argv + env parity with screen-mode relaunch for the same session/mode.
    #[test]
    fn plan_rebuild_relaunch_matches_screen_mode_args_and_env() {
        use crate::app::screen_mode_relaunch::{
            build_screen_mode_relaunch_args, screen_mode_env_value,
        };

        let current = ["grok-oss", "--no-leader", "--model", "grok-4", "do stuff"];
        let relaunch = sample_relaunch(false);
        let plan = plan_rebuild_relaunch(&relaunch, current.iter().copied());

        let expected_args =
            build_screen_mode_relaunch_args(current.iter().copied(), "sess-1", false);
        assert_eq!(plan.exe, PathBuf::from("/tmp/grok-oss-new"));
        assert_eq!(plan.args, expected_args);
        assert_eq!(plan.screen_mode_env, screen_mode_env_value(false));
        assert_eq!(plan.screen_mode_env, "fullscreen");

        // Args must resume the session and force fullscreen (not re-fire prompt).
        let as_str: Vec<String> = plan
            .args
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(
            as_str
                .windows(2)
                .any(|w| w[0] == "--resume" && w[1] == "sess-1")
        );
        assert!(as_str.iter().any(|s| s == "--fullscreen"));
        assert!(as_str.iter().any(|s| s == "--no-leader"));
        assert!(as_str.iter().any(|s| s == "--model"));
        assert!(!as_str.iter().any(|s| s.contains("do stuff")));
    }

    #[test]
    fn plan_rebuild_relaunch_minimal_sets_minimal_env_and_flag() {
        use crate::app::screen_mode_relaunch::screen_mode_env_value;

        let relaunch = sample_relaunch(true);
        let plan = plan_rebuild_relaunch(&relaunch, ["grok-oss"].iter().copied());
        assert_eq!(plan.screen_mode_env, "minimal");
        assert_eq!(plan.screen_mode_env, screen_mode_env_value(true));
        let as_str: Vec<String> = plan
            .args
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(as_str.iter().any(|s| s == "--minimal"));
        assert!(!as_str.iter().any(|s| s == "--fullscreen"));
    }

    #[test]
    fn restore_blocked_hint_mentions_cleanup_and_resume() {
        let r = sample_relaunch(true);
        let mut buf = Vec::new();
        print_rebuild_restore_blocked_hint(&r, &"drain failed", &mut buf);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Terminal cleanup failed"), "{out}");
        assert!(out.contains("drain failed"), "{out}");
        assert!(out.contains("Not relaunching"), "{out}");
        assert!(out.contains("/tmp/grok-oss-new"), "{out}");
        assert!(out.contains("GROK_SCREEN_MODE=minimal"), "{out}");
        assert!(out.contains("--resume sess-1"), "{out}");
        assert!(out.contains("--minimal"), "{out}");
    }

    #[test]
    fn exec_failure_hint_uses_screen_mode_resume_hint() {
        let r = sample_relaunch(false);
        let mut buf = Vec::new();
        print_rebuild_exec_failure_hint(&r, &"exec denied", &mut buf);
        let out = String::from_utf8(buf).unwrap();
        assert!(
            out.contains("Failed to relaunch on new binary: exec denied"),
            "{out}"
        );
        assert!(out.contains("GROK_SCREEN_MODE=fullscreen"), "{out}");
        assert!(out.contains("--fullscreen"), "{out}");
        assert!(out.contains("--resume sess-1"), "{out}");
        // Must not recommend bare upstream `grok` without product mode env.
        assert!(
            !out.contains("Resume with: grok-oss --resume"),
            "must use full screen-mode resume hint:\n{out}"
        );
    }

    /// Contract: failed `just install` / verify reports in this session and
    /// does not arm self re-exec (peers were not asked to quit).
    #[test]
    fn handle_rebuild_done_failure_reports_and_does_not_relaunch() {
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            agent.rebuild_progress = Some(crate::app::agent_view::RebuildUiProgress {
                fraction: 0.97,
                detail: "Verifying installed binary".into(),
            });
        }
        let effects = handle_rebuild_done(
            &mut app,
            agent_id,
            Err("`just install` failed with status exit 1\nError: os error 6".into()),
        );
        assert!(
            effects.is_empty(),
            "failed rebuild must not quit: {effects:?}"
        );
        assert!(
            app.rebuild_relaunch.is_none(),
            "failed install must not arm self re-exec"
        );
        let agent = app.agents.get(&agent_id).expect("agent");
        assert!(
            agent.rebuild_progress.is_none(),
            "failed rebuild must clear the in-progress bar so /rebuild is invokable"
        );
        let scroll: Vec<&str> = agent
            .scrollback
            .iter_entries()
            .filter_map(|(_, e)| match &e.block {
                crate::scrollback::block::RenderBlock::System(s) => Some(s.text.as_str()),
                _ => None,
            })
            .collect();
        let joined = scroll.join("\n");
        assert!(
            joined.contains("Rebuild failed"),
            "failure must land in scrollback: {joined}"
        );
        assert!(
            joined.contains("asked to quit"),
            "failure copy must say the fleet was not signaled: {joined}"
        );
    }

    /// Contract: after a failed rebuild, `/rebuild` still starts a new run.
    #[test]
    fn rebuild_still_invokable_after_failed_rebuild_done() {
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        let _ = handle_rebuild_done(&mut app, agent_id, Err("just install failed".into()));
        let effects = super::super::dispatch(Action::RebuildAndRelaunch, &mut app);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::RunRebuild { .. })),
            "after a failed rebuild, /rebuild must still spawn RunRebuild: {effects:?}"
        );
    }

    fn sample_success_report(installed: &std::path::Path) -> xai_grok_update::RebuildReport {
        xai_grok_update::RebuildReport {
            source_root: PathBuf::from("/src"),
            installed_path: installed.to_path_buf(),
            installed_identity: "0.2.120 (newsha)".into(),
            install_backend: xai_grok_update::InstallBackend::JustInstall,
            leader_outcomes: vec![],
            peer_outcomes: vec![],
            live_sessions: vec![],
            summary_lines: vec!["Rebuild complete.".into()],
        }
    }

    /// Named contract: after a successful `/rebuild` while a turn is running
    /// (in-flight stash already cleared, last user prompt still in scrollback),
    /// the invoker must write continue-interrupted-turn
    /// (`canceled_turn_resume.json`), arm self re-exec for the same session,
    /// and the re-exec SessionLoaded path must auto-continue that prompt.
    /// Silent idle or a lost session is a miss.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn handle_rebuild_done_mid_turn_writes_cancel_resume_and_session_load_continues_the_turn() {
        use crate::app::actions::{Action, Effect, TaskResult};
        use crate::app::agent::{AgentId, AgentState};
        use agent_client_protocol as acp;

        let grok_home = tempfile::tempdir().unwrap();
        let _home = xai_grok_test_support::EnvGuard::set("GROK_HOME", grok_home.path());
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "rebuild-resume-mid-turn";
        let prompt = "finish the rebuild resume contract after first activity";
        let installed = proj.path().join("grok-oss-installed");
        std::fs::write(&installed, b"stub").unwrap();

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        app.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd.clone();
            agent.session.state = AgentState::TurnRunning;
            agent.session.current_prompt_id = Some("pid-rebuild-resume".into());
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::user_prompt(prompt));
            // First server activity cleared the rewind stash. Live mid-turn
            // after tools start looks like this.
            agent.session.in_flight_prompt = None;
        }

        let effects = handle_rebuild_done(
            &mut app,
            agent_id,
            Ok(Box::new(sample_success_report(&installed))),
        );
        let relaunch = app
            .rebuild_relaunch
            .as_ref()
            .expect("successful rebuild must arm self re-exec so the session is not lost");
        assert_eq!(relaunch.session_id, sid);
        assert_eq!(relaunch.installed_exe, installed);
        assert!(
            effects.iter().any(|e| matches!(e, Effect::Quit)),
            "successful rebuild must quit into re-exec, got {effects:?}"
        );

        let marker =
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .expect("load marker")
                .expect(
                    "successful /rebuild while mid-turn must write canceled_turn_resume.json \
             so reopen continues the turn",
                );
        assert_eq!(marker.prompt_text, prompt);
        assert!(
            xai_grok_shell::session::canceled_turn_resume::should_auto_resume_on_restart(
                true,
                Some(&marker)
            )
        );

        // Re-exec equivalent: cold SessionLoaded of the same session.
        let mut reopened = crate::app::app_view::tests::test_app_with_agent();
        reopened.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = reopened.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.state = AgentState::Idle;
            agent.session.loading_replay = true;
            agent.session.pending_prompts.clear();
        }
        let load_effects = super::super::dispatch(
            Action::TaskComplete(TaskResult::SessionLoaded {
                agent_id,
                session_id: acp::SessionId::new(sid),
                models: None,
                code_restored: false,
                restore_summary: None,
                restore_degree: None,
                running_prompt_id: None,
                scheduler_background_loops: None,
            }),
            &mut reopened,
        );
        let agent = reopened.agents.get(&agent_id).unwrap();
        let toast = agent
            .toast
            .as_ref()
            .map(|(msg, _)| msg.as_str())
            .unwrap_or("");
        assert!(
            toast.contains("Continuing interrupted turn"),
            "re-exec session load must toast continue-interrupted-turn; got {toast:?}"
        );
        let continued = load_effects.iter().any(|e| {
            matches!(
                e,
                Effect::SendPrompt { text, .. } if text == prompt
            ) || matches!(
                e,
                Effect::SendPromptBlocks { .. } | Effect::SetModeThenPrompt { .. }
            )
        });
        assert!(
            continued,
            "re-exec session load must auto-continue the interrupted prompt, got {load_effects:?}"
        );
        assert!(
            agent.session.state.is_turn_running(),
            "continued turn must be running; state={:?}",
            agent.session.state
        );

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
    }

    /// Mid-turn cancel-resume plus leftover plan.md must continue the turn
    /// and must not auto-open the plan side panel.
    #[test]
    fn rebuild_or_resume_does_not_auto_open_plan_side_panel_when_turn_is_owed() {
        use crate::app::actions::{Action, TaskResult};
        use crate::app::agent::{AgentId, AgentState};
        use crate::views::plan_approval_view::PLAN_IDLE_REVIEW_STATUS;
        use agent_client_protocol as acp;

        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "rebuild-resume-no-plan-dock";
        let prompt = "continue this turn without docking plan review";
        let installed = proj.path().join("grok-oss-installed");
        std::fs::write(&installed, b"stub").unwrap();

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        app.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd.clone();
            agent.session.state = AgentState::TurnRunning;
            agent.session.current_prompt_id = Some("pid-rebuild-no-dock".into());
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::user_prompt(prompt));
            agent.session.in_flight_prompt = None;
        }

        let _ = handle_rebuild_done(
            &mut app,
            agent_id,
            Ok(Box::new(sample_success_report(&installed))),
        );

        let mut reopened = crate::app::app_view::tests::test_app_with_agent();
        reopened.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = reopened.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.state = AgentState::Idle;
            agent.session.loading_replay = true;
            agent.session.pending_prompts.clear();
            agent.plan_mode_active = true;
            agent.plan_mode_pending = None;
            agent.plan_decision_resolved = false;
            agent.latest_inline_plan_content =
                Some("# Leftover plan\n\nDo not auto-open this pane\n".into());
        }
        let _ = super::super::dispatch(
            Action::TaskComplete(TaskResult::SessionLoaded {
                agent_id,
                session_id: acp::SessionId::new(sid),
                models: None,
                code_restored: false,
                restore_summary: None,
                restore_degree: None,
                running_prompt_id: None,
                scheduler_background_loops: None,
            }),
            &mut reopened,
        );
        {
            let agent = reopened.agents.get_mut(&agent_id).unwrap();
            let toast = agent
                .toast
                .as_ref()
                .map(|(msg, _)| msg.as_str())
                .unwrap_or("");
            assert!(
                toast.contains("Continuing interrupted turn"),
                "re-exec session load must toast continue-interrupted-turn; got {toast:?}"
            );
            agent.surface_idle_plan_review_if_needed();
            assert!(
                agent.line_viewer.is_none(),
                "rebuild/resume must not auto-open the plan side panel when a turn is owed"
            );
            assert_ne!(
                agent.plan_loop_status_label(),
                Some("Plan ready. Side panel open"),
                "must not paint Plan ready. Side panel open while the turn continues"
            );
            let _ = PLAN_IDLE_REVIEW_STATUS;
        }

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
    }

    /// Named contract: `/rebuild` while the pane is idle after a completed
    /// user turn must not write `canceled_turn_resume.json` and must not
    /// auto re-fire that last prompt on re-exec session load. Mid-turn
    /// continue is a different contract.
    #[test]
    fn handle_rebuild_done_idle_completed_turn_does_not_write_cancel_resume_or_refire_last_prompt()
    {
        use crate::app::actions::{Action, Effect, TaskResult};
        use crate::app::agent::{AgentId, AgentState};
        use crate::scrollback::blocks::SessionEvent;
        use agent_client_protocol as acp;

        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "rebuild-idle-no-refire";
        let prompt = "already finished turn must not be sent again after rebuild";
        let installed = proj.path().join("grok-oss-installed");
        std::fs::write(&installed, b"stub").unwrap();

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        app.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd.clone();
            agent.session.state = AgentState::Idle;
            agent.session.current_prompt_id = None;
            agent.session.in_flight_prompt = None;
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::user_prompt(prompt));
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::agent_message(
                    "done; nothing left to continue",
                ));
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::session_event(
                    SessionEvent::TurnCompleted { elapsed: None },
                ));
        }

        let effects = handle_rebuild_done(
            &mut app,
            agent_id,
            Ok(Box::new(sample_success_report(&installed))),
        );
        let relaunch = app
            .rebuild_relaunch
            .as_ref()
            .expect("idle /rebuild still arms self re-exec onto the new binary");
        assert_eq!(relaunch.session_id, sid);
        assert!(
            effects.iter().any(|e| matches!(e, Effect::Quit)),
            "successful idle rebuild must quit into re-exec, got {effects:?}"
        );
        assert!(
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .expect("load marker")
                .is_none(),
            "idle completed turn must not write canceled_turn_resume.json"
        );

        let mut reopened = crate::app::app_view::tests::test_app_with_agent();
        reopened.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = reopened.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.state = AgentState::Idle;
            agent.session.loading_replay = true;
            agent.session.pending_prompts.clear();
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::user_prompt(prompt));
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::agent_message(
                    "done; nothing left to continue",
                ));
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::session_event(
                    SessionEvent::TurnCompleted { elapsed: None },
                ));
        }
        let load_effects = super::super::dispatch(
            Action::TaskComplete(TaskResult::SessionLoaded {
                agent_id,
                session_id: acp::SessionId::new(sid),
                models: None,
                code_restored: false,
                restore_summary: None,
                restore_degree: None,
                running_prompt_id: None,
                scheduler_background_loops: None,
            }),
            &mut reopened,
        );
        let agent = reopened.agents.get(&agent_id).unwrap();
        let toast = agent
            .toast
            .as_ref()
            .map(|(msg, _)| msg.as_str())
            .unwrap_or("");
        assert!(
            !toast.contains("Continuing interrupted turn"),
            "idle completed reopen must not toast continue-interrupted-turn; got {toast:?}"
        );
        let refired = load_effects.iter().any(|e| {
            matches!(
                e,
                Effect::SendPrompt { text, .. } if text == prompt
            ) || matches!(
                e,
                Effect::SendPromptBlocks { .. } | Effect::SetModeThenPrompt { .. }
            )
        });
        assert!(
            !refired,
            "idle completed reopen must not auto re-fire the last prompt, got {load_effects:?}"
        );
        assert!(
            agent.session.state.is_idle(),
            "idle completed reopen must stay idle; state={:?}",
            agent.session.state
        );

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
    }

    /// Named contract: a leftover `canceled_turn_resume.json` from an earlier
    /// interrupt must be dropped on session load when the primary user turn
    /// already finished successfully. Do not re-fire a completed prompt.
    #[test]
    #[serial_test::serial(GROK_HOME)]
    fn session_load_drops_stale_cancel_resume_marker_when_primary_turn_finished_successfully() {
        use crate::app::actions::{Action, Effect, TaskResult};
        use crate::app::agent::{AgentId, AgentState};
        use crate::scrollback::blocks::SessionEvent;
        use agent_client_protocol as acp;

        let _grok_home = crate::test_util::GrokHomeFixture::new();
        let proj = tempfile::tempdir().unwrap();
        let cwd = proj.path().to_path_buf();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let sid = "rebuild-stale-marker-drop";
        let prompt = "stale leftover marker must not re-send this finished prompt";

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();

        let marker = xai_grok_shell::session::canceled_turn_resume::build_user_cancel_marker(
            prompt,
            Some("pid-stale-drop"),
            "2026-08-17T00:00:00Z",
        )
        .expect("marker");
        xai_grok_shell::session::canceled_turn_resume::write_canceled_turn_resume(
            &cwd_str, sid, &marker,
        )
        .expect("write leftover marker");

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        app.current_ui.resume_canceled_turn_on_restart = Some(true);
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.session_id = Some(sid.into());
            agent.session.cwd = cwd;
            agent.session.state = AgentState::Idle;
            agent.session.loading_replay = true;
            agent.session.pending_prompts.clear();
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::user_prompt(prompt));
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::agent_message(
                    "turn already finished after the earlier interrupt",
                ));
            agent
                .scrollback
                .push_block(crate::scrollback::block::RenderBlock::session_event(
                    SessionEvent::TurnCompleted { elapsed: None },
                ));
        }

        let load_effects = super::super::dispatch(
            Action::TaskComplete(TaskResult::SessionLoaded {
                agent_id,
                session_id: acp::SessionId::new(sid),
                models: None,
                code_restored: false,
                restore_summary: None,
                restore_degree: None,
                running_prompt_id: None,
                scheduler_background_loops: None,
            }),
            &mut app,
        );
        let agent = app.agents.get(&agent_id).unwrap();
        let toast = agent
            .toast
            .as_ref()
            .map(|(msg, _)| msg.as_str())
            .unwrap_or("");
        assert!(
            !toast.contains("Continuing interrupted turn"),
            "stale leftover marker must not toast continue; got {toast:?}"
        );
        let refired = load_effects.iter().any(|e| {
            matches!(
                e,
                Effect::SendPrompt { text, .. } if text == prompt
            ) || matches!(
                e,
                Effect::SendPromptBlocks { .. } | Effect::SetModeThenPrompt { .. }
            )
        });
        assert!(
            !refired,
            "stale leftover marker must not auto re-fire a finished turn, got {load_effects:?}"
        );
        assert!(
            agent.session.state.is_idle(),
            "stale leftover must stay idle; state={:?}",
            agent.session.state
        );
        assert!(
            xai_grok_shell::session::canceled_turn_resume::load_canceled_turn_resume(&cwd_str, sid)
                .expect("load marker")
                .is_none(),
            "stale leftover canceled_turn_resume.json must be dropped on load"
        );

        let _ = xai_grok_shell::session::canceled_turn_resume::clear_canceled_turn_resume(
            &cwd_str, sid,
        );
        xai_grok_shell::session::canceled_turn_resume::clear_process_shutdown_cancel_resume();
    }

    /// Contract: restore + rebuild_relaunch execs the new binary; a failed
    /// install never sets rebuild_relaunch so this stays None.
    #[test]
    fn post_restore_prefers_rebuild_relaunch_only_when_armed() {
        assert_eq!(
            post_restore_relaunch_action(true, false, false),
            PostRestoreRelaunch::None
        );
        assert_eq!(
            post_restore_relaunch_action(true, true, true),
            PostRestoreRelaunch::ExecRebuild
        );
        assert_eq!(
            post_restore_relaunch_action(false, true, false),
            PostRestoreRelaunch::BlockedRebuild
        );
        assert_eq!(
            post_restore_relaunch_action(true, false, true),
            PostRestoreRelaunch::ExecScreenMode
        );
        assert_eq!(
            post_restore_relaunch_action(false, false, true),
            PostRestoreRelaunch::BlockedScreenMode
        );
    }
}
