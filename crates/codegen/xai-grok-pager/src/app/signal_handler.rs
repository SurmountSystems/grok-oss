//! TUI-side signal handlers that restore the terminal before exiting.
//!
//! Without these, signal-triggered termination (SIGINT, SIGTERM, SIGHUP from
//! WSL/SSH disconnect) leaves the user's terminal in raw mode with the
//! alternate screen still active, mouse capture still on, and Kitty keyboard
//! protocol flags still pushed. The next time the terminal becomes visible the
//! user sees a stale snapshot of the TUI that looks alive but is backed by a
//! dead process.
//!
//! SIGINT / SIGTERM / SIGHUP are handled in a tokio task. The handler runs
//! in normal Rust context (not actual signal-handler context), so it can use
//! the full [`super::emit_terminal_teardown_sequences`] path
//! (`with_locked_stderr`, conditional cursor-style reset, multiplexer flush) plus
//! `disable_raw_mode`, then flush Sentry/OpenTelemetry, then exit.
//!
//! SIGPIPE is intentionally left alone. The current disposition is `SIG_IGN`
//! (Rust's stdlib default), which means writes to a closed pipe return
//! `Err(BrokenPipe)` instead of terminating the process. The writer thread
//! already swallows those errors. Installing a custom handler would change
//! the disposition inherited by fork+exec children (MCP servers, hooks) from
//! `SIG_IGN` to `SIG_DFL`, re-introducing a prior SIGPIPE regression.

use std::sync::atomic::{AtomicBool, Ordering};

use agent_client_protocol as acp;

use super::ScreenMode;

/// Whether the active screen mode is fullscreen. Set by [`install`].
static SCREEN_MODE_FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// Most recently active session ID for signal-handler cleanup.
/// `parking_lot::Mutex` avoids poisoning in the `-> !` exit path.
static CURRENT_SESSION_ID: parking_lot::Mutex<Option<acp::SessionId>> =
    parking_lot::Mutex::new(None);

pub(crate) fn set_current_session_id(id: Option<acp::SessionId>) {
    *CURRENT_SESSION_ID.lock() = id;
}

/// Lets the signal handler route SIGINT/SIGTERM/SIGHUP into the same graceful
/// quit as `/exit` (running teardown + history/telemetry flushes) instead of a
/// hard exit. Registered by the event loop before it starts.
static QUIT_NOTIFY: parking_lot::Mutex<Option<std::sync::Arc<tokio::sync::Notify>>> =
    parking_lot::Mutex::new(None);

pub(crate) fn set_quit_notify(notify: std::sync::Arc<tokio::sync::Notify>) {
    *QUIT_NOTIFY.lock() = Some(notify);
}

/// Deregister the quit notify once the event loop has exited; a later first
/// signal force-exits instead of notifying nobody.
pub(crate) fn clear_quit_notify() {
    *QUIT_NOTIFY.lock() = None;
}

/// Whether the TUI currently owns the terminal. Set by [`install`], cleared
/// by [`mark_restored`] or by [`shutdown_with_terminal_restore`] after
/// teardown completes. The SIGPIPE path (SIG_IGN, no handler) does not
/// interact with this flag.
static TERMINAL_OWNED: AtomicBool = AtomicBool::new(false);

/// Set when SIGUSR1 asks this TUI to re-exec onto a newly installed binary.
static PEER_REBUILD_RELAUNCH: AtomicBool = AtomicBool::new(false);

/// Consume the peer-rebuild flag (true once after SIGUSR1 until taken).
pub(crate) fn take_peer_rebuild_relaunch() -> bool {
    PEER_REBUILD_RELAUNCH.swap(false, Ordering::AcqRel)
}

/// Read the peer-rebuild flag without consuming it.
///
/// Arm must peek first and take only after re-exec is armed. Consuming on a
/// failed arm (no request file yet) made peers print a resume hint and die.
pub(crate) fn peek_peer_rebuild_relaunch() -> bool {
    PEER_REBUILD_RELAUNCH.load(Ordering::Acquire)
}

/// Arm the peer-rebuild flag (Unix `SIGUSR1` listener and tests).
pub(crate) fn mark_peer_rebuild_relaunch_from_sigusr1() {
    PEER_REBUILD_RELAUNCH.store(true, Ordering::Release);
}

/// Unix lifecycle signals the TUI maps to quit or rebuild re-exec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnixLifecycleSignal {
    /// Ctrl-C / SIGINT: quit. Must not arm peer rebuild re-exec.
    Interrupt,
    Terminate,
    Hangup,
    /// Cooperative `/rebuild` re-exec.
    User1,
}

/// SIGUSR1 arms peer rebuild. SIGINT / SIGTERM / SIGHUP do not.
pub(crate) fn unix_lifecycle_signal_arms_peer_rebuild(signal: UnixLifecycleSignal) -> bool {
    matches!(signal, UnixLifecycleSignal::User1)
}

/// First-signal exit code for the graceful quit path.
pub(crate) fn unix_lifecycle_signal_exit_code(signal: UnixLifecycleSignal) -> i32 {
    match signal {
        UnixLifecycleSignal::Interrupt => 130,
        UnixLifecycleSignal::Terminate => 143,
        UnixLifecycleSignal::Hangup => 129,
        UnixLifecycleSignal::User1 => 0,
    }
}

fn apply_unix_lifecycle_signal(signal: UnixLifecycleSignal) {
    if unix_lifecycle_signal_arms_peer_rebuild(signal) {
        mark_peer_rebuild_relaunch_from_sigusr1();
    }
    request_graceful_or_exit(unix_lifecycle_signal_exit_code(signal));
}

/// Install signal handlers for the TUI lifecycle. Call after `init_terminal`.
pub(crate) fn install(mode: ScreenMode) {
    SCREEN_MODE_FULLSCREEN.store(mode.is_fullscreen(), Ordering::Release);
    TERMINAL_OWNED.store(true, Ordering::Release);

    // Ignore SIGTTIN/SIGTTOU so the pager can't be suspended if a
    // child process (or its grandchild) briefly steals the terminal's
    // foreground process group. Every TUI that reads from stdin should
    // do this — without it, a single tcsetpgrp() from a rogue child
    // stops the entire pager, leaving the terminal in raw mode.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTTIN, libc::SIG_IGN);
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
    }

    spawn_async_signal_task();
}

/// Mark the terminal as no longer owned by the TUI. Called by the clean
/// shutdown path and the panic hook so a signal arriving after the shell
/// has reclaimed the terminal does not re-run teardown.
pub(crate) fn mark_restored() {
    TERMINAL_OWNED.store(false, Ordering::Release);
}

fn spawn_async_signal_task() {
    tokio::spawn(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sigterm = signal(SignalKind::terminate()).ok();
            let mut sighup = signal(SignalKind::hangup()).ok();
            let mut sigusr1 = signal(SignalKind::user_defined1()).ok();
            // SIGUSR1 is cooperative rebuild re-exec (not default terminate).
            // SIGTERM/HUP/INT: first requests graceful quit; a second forces exit.
            tokio::select! {
                code = next_signal_code(&mut sigterm, &mut sighup) => {
                    let kind = match code {
                        130 => UnixLifecycleSignal::Interrupt,
                        143 => UnixLifecycleSignal::Terminate,
                        129 => UnixLifecycleSignal::Hangup,
                        _ => UnixLifecycleSignal::Terminate,
                    };
                    apply_unix_lifecycle_signal(kind);
                    let code2 = next_signal_code(&mut sigterm, &mut sighup).await;
                    shutdown_with_terminal_restore(code2);
                }
                _ = recv_optional_unix_signal(&mut sigusr1) => {
                    apply_unix_lifecycle_signal(UnixLifecycleSignal::User1);
                    let code2 = next_signal_code(&mut sigterm, &mut sighup).await;
                    shutdown_with_terminal_restore(code2);
                }
            }
        }
        #[cfg(windows)]
        {
            // ctrl_c gets the first-graceful / second-force treatment. Console
            // close, logoff, and shutdown stay immediate: the OS grants only a
            // short window, so graceful teardown may not finish.
            use tokio::signal::windows;
            let mut ctrl_close = windows::ctrl_close().ok();
            let mut ctrl_logoff = windows::ctrl_logoff().ok();
            let mut ctrl_shutdown = windows::ctrl_shutdown().ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    handle_windows_ctrl_c_double().await;
                }
                _ = recv_optional_ctrl_close(&mut ctrl_close) => {
                    shutdown_with_terminal_restore(1);
                }
                _ = recv_optional_ctrl_logoff(&mut ctrl_logoff) => {
                    shutdown_with_terminal_restore(0);
                }
                _ = recv_optional_ctrl_shutdown(&mut ctrl_shutdown) => {
                    shutdown_with_terminal_restore(0);
                }
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            if tokio::signal::ctrl_c().await.is_ok() {
                request_graceful_or_exit(130);
                let _ = tokio::signal::ctrl_c().await;
                shutdown_with_terminal_restore(130);
            }
        }
    });
}

/// Wait for the next SIGINT/SIGTERM/SIGHUP and map it to its exit code.
///
/// Shared with the agent binary (`xai-grok-pager-bin`) so the 130/143/129 map
/// cannot drift between TUI and agent signal handlers.
#[cfg(unix)]
pub async fn next_signal_code(
    sigterm: &mut Option<tokio::signal::unix::Signal>,
    sighup: &mut Option<tokio::signal::unix::Signal>,
) -> i32 {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => 130,
        _ = recv_optional_unix_signal(sigterm) => 143,
        _ = recv_optional_unix_signal(sighup) => 129,
    }
}

/// Await one recv on an optional unix signal stream, or pend forever if absent.
#[cfg(unix)]
pub async fn recv_optional_unix_signal(sig: &mut Option<tokio::signal::unix::Signal>) {
    if let Some(s) = sig.as_mut() {
        let _ = s.recv().await;
    } else {
        std::future::pending::<()>().await;
    }
}

#[cfg(windows)]
async fn handle_windows_ctrl_c_double() {
    request_graceful_or_exit(130);
    let _ = tokio::signal::ctrl_c().await;
    shutdown_with_terminal_restore(130);
}

// Tokio exposes distinct CtrlClose / CtrlLogoff / CtrlShutdown types with no
// shared trait — generate the three identical recv helpers from one body.
#[cfg(windows)]
macro_rules! define_recv_optional_windows_signal {
    ($name:ident, $ty:ty) => {
        async fn $name(sig: &mut Option<$ty>) {
            if let Some(s) = sig.as_mut() {
                let _ = s.recv().await;
            } else {
                std::future::pending::<()>().await;
            }
        }
    };
}

#[cfg(windows)]
define_recv_optional_windows_signal!(recv_optional_ctrl_close, tokio::signal::windows::CtrlClose);
#[cfg(windows)]
define_recv_optional_windows_signal!(
    recv_optional_ctrl_logoff,
    tokio::signal::windows::CtrlLogoff
);
#[cfg(windows)]
define_recv_optional_windows_signal!(
    recv_optional_ctrl_shutdown,
    tokio::signal::windows::CtrlShutdown
);

/// Request the event loop's graceful quit when it is registered and the TUI
/// still owns the terminal; otherwise hard-exit (agent mode, or a signal after
/// teardown already started).
fn request_graceful_or_exit(code: i32) {
    let notify = QUIT_NOTIFY.lock().clone();
    if TERMINAL_OWNED.load(Ordering::Acquire)
        && let Some(n) = notify
    {
        // An orphan never gets the second, forcing signal; bound the quit.
        super::exit_timeout::arm(code);
        n.notify_one();
    } else {
        shutdown_with_terminal_restore(code);
    }
}

/// The second-signal teardown, exposed for the exit-timeout path.
pub(crate) fn force_exit(exit_code: i32) -> ! {
    shutdown_with_terminal_restore(exit_code)
}

/// Restore the terminal first, then flush observability, then exit.
///
/// Restore must precede the (up to 2-second) Sentry flush; otherwise the
/// user stares at a raw-mode + alt-screen + mouse-SGR terminal for that
/// whole window. Best-effort: a frame queued on the writer thread microseconds
/// before the signal can still land after our teardown writes -- the writer
/// thread is not reachable from here without a deadlock risk.
fn shutdown_with_terminal_restore(exit_code: i32) -> ! {
    // The graceful quit (or a prior teardown) already restored the terminal;
    // skip teardown and just flush telemetry before exiting.
    if !TERMINAL_OWNED.load(Ordering::Acquire) {
        flush_telemetry_and_exit(exit_code);
    }
    let mode = if SCREEN_MODE_FULLSCREEN.load(Ordering::Acquire) {
        ScreenMode::Fullscreen
    } else {
        ScreenMode::Inline
    };
    // Signal-path shutdown has no terminal handle, so fall back to the screen
    // bottom for the final cursor position.
    super::emit_terminal_teardown_sequences(mode, None);
    let _ = crossterm::terminal::disable_raw_mode();
    // Mark after teardown so concurrent paths see TERMINAL_OWNED == true
    // until all escape sequences and tcsetattr have been written.
    TERMINAL_OWNED.store(false, Ordering::Release);
    // Best-effort unregister (non-blocking flock to avoid hanging).
    if CURRENT_SESSION_ID.lock().is_some() {
        let _ = xai_grok_active_sessions::try_unregister_pid(std::process::id());
    }
    flush_telemetry_and_exit(exit_code);
}

/// Shared `-> !` exit tail of `shutdown_with_terminal_restore`'s early-return
/// and full-teardown paths.
fn flush_telemetry_and_exit(exit_code: i32) -> ! {
    // Reap detached (setsid) background children before the hard exit. This tail
    // runs on the force/second-signal and agent-mode paths that skip the
    // graceful quit; the graceful path reaps them in `app::run`'s teardown.
    xai_tty_utils::global_process_scope().kill_all();
    // Restore fd 2 so Sentry/OTEL flushes reach the terminal.
    xai_tty_utils::restore_native_stderr();
    xai_grok_telemetry::sentry::flush_on_shutdown();
    xai_grok_telemetry::otel_layer::shutdown_otel();
    // Flush the --debug firehose on TUI signal exit (this path bypasses main's flush).
    xai_grok_telemetry::debug_log::flush();
    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Restores `TERMINAL_OWNED` to its prior value on drop, including on panic.
    struct TerminalOwnedGuard(bool);

    impl TerminalOwnedGuard {
        fn set(value: bool) -> Self {
            Self(TERMINAL_OWNED.swap(value, Ordering::AcqRel))
        }
    }

    impl Drop for TerminalOwnedGuard {
        fn drop(&mut self) {
            TERMINAL_OWNED.store(self.0, Ordering::Release);
        }
    }

    #[tokio::test]
    async fn request_graceful_or_exit_notifies_registered_quit() {
        let notify = std::sync::Arc::new(tokio::sync::Notify::new());
        set_quit_notify(notify.clone());
        let stored = QUIT_NOTIFY.lock().clone().expect("quit notify registered");
        assert!(std::sync::Arc::ptr_eq(&stored, &notify));

        // request_graceful_or_exit only notifies (vs process::exit) while the
        // terminal reads as owned; the guard restores the flag even on panic.
        let _owned_guard = TerminalOwnedGuard::set(true);
        request_graceful_or_exit(130);

        tokio::time::timeout(std::time::Duration::from_secs(1), notify.notified())
            .await
            .expect("graceful branch notified the registered quit handle");
    }

    /// Contract: SIGUSR1 must set the peer-rebuild flag (not default terminate).
    /// A failed `/rebuild` never sends the signal; this only arms the receiver.
    #[test]
    fn sigusr1_sets_peer_rebuild_flag_once() {
        let _ = take_peer_rebuild_relaunch();
        mark_peer_rebuild_relaunch_from_sigusr1();
        assert!(
            peek_peer_rebuild_relaunch(),
            "SIGUSR1 must set the cooperative re-exec flag (peek must not consume)"
        );
        assert!(
            peek_peer_rebuild_relaunch(),
            "peek must leave the flag so a later arm retry still sees SIGUSR1"
        );
        assert!(
            take_peer_rebuild_relaunch(),
            "SIGUSR1 must set the cooperative re-exec flag"
        );
        assert!(
            !take_peer_rebuild_relaunch(),
            "flag is one-shot so a later /exit does not re-exec"
        );
    }

    /// Contract: Ctrl-C / SIGINT quits and does not arm peer re-exec.
    /// SIGUSR1 still rebuilds grok-oss.
    #[test]
    fn sigint_does_not_arm_peer_rebuild_sigusr1_does() {
        let _ = take_peer_rebuild_relaunch();
        assert!(
            !unix_lifecycle_signal_arms_peer_rebuild(UnixLifecycleSignal::Interrupt),
            "SIGINT / Ctrl-C must quit without arming peer re-exec"
        );
        assert!(!unix_lifecycle_signal_arms_peer_rebuild(
            UnixLifecycleSignal::Terminate
        ));
        assert!(!unix_lifecycle_signal_arms_peer_rebuild(
            UnixLifecycleSignal::Hangup
        ));
        assert!(unix_lifecycle_signal_arms_peer_rebuild(
            UnixLifecycleSignal::User1
        ));
        assert_eq!(
            unix_lifecycle_signal_exit_code(UnixLifecycleSignal::Interrupt),
            130
        );
        assert_eq!(
            unix_lifecycle_signal_exit_code(UnixLifecycleSignal::User1),
            0
        );
        let _ = take_peer_rebuild_relaunch();
        if unix_lifecycle_signal_arms_peer_rebuild(UnixLifecycleSignal::Interrupt) {
            mark_peer_rebuild_relaunch_from_sigusr1();
        }
        assert!(
            !take_peer_rebuild_relaunch(),
            "SIGINT must not set the peer-rebuild flag"
        );
        if unix_lifecycle_signal_arms_peer_rebuild(UnixLifecycleSignal::User1) {
            mark_peer_rebuild_relaunch_from_sigusr1();
        }
        assert!(
            take_peer_rebuild_relaunch(),
            "SIGUSR1 must set the peer-rebuild flag"
        );
    }
}
