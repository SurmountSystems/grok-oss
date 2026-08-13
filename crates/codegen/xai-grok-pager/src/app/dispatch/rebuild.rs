//! `/rebuild` TaskResult handling: report, cancel mid-turn, arm self re-exec.
//!
//! Peer TUIs (other live product windows) receive `SIGUSR1` after a rebuild
//! and arm re-exec via [`try_arm_peer_rebuild_relaunch_from_request`].
//!
//! **Exit-path contract:** every event-loop exit that can race with peer
//! rebuild (SIGUSR1 quit notify, leader IPC disconnect) must call
//! [`arm_peer_rebuild_before_exit`] so the window re-execs onto the new binary
//! instead of only quitting.

// Peer-rebuild helpers are unit-tested. Event-loop SIGUSR1 / leader-disconnect
// wire-up is not on this restack tip yet (clippy --lib --bins does not compile
// tests, so those items look unused).
#![allow(dead_code)]

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
/// cancel-resume is still handled by the graceful quit path that follows.
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
                    "Rebuild failed (no leaders were signaled):\n{error}"
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

            // Mid-turn: cancel with canceled_turn_resume so reopen re-queues once.
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
}
