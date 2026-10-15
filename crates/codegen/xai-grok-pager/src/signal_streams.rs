/// Stand-in code for a claimed SIGUSR1. This is a peer `/rebuild` re-exec, not a quit.
/// 128 + 10 matches the conventional signal exit, but the process must not die with that status.
pub const PEER_REBUILD_SIGNAL_CODE: i32 = 138;

/// Signals claimed at `install` so the default disposition cannot kill the process.
pub fn claimed_lifecycle_signal_names() -> &'static [&'static str] {
    &["SIGINT", "SIGTERM", "SIGHUP", "SIGUSR1"]
}

/// `true` when this stream code is the peer rebuild re-exec, not a quit.
pub fn peer_rebuild_code_arms_reexec(code: i32) -> bool {
    code == PEER_REBUILD_SIGNAL_CODE
}

/// SIGINT, SIGTERM, SIGHUP, and SIGUSR1 are claimed from construction for as long as this
/// lives: without a live stream a signal takes its default action and kills the process.
/// SIGUSR1 maps to [`PEER_REBUILD_SIGNAL_CODE`] so the default action cannot kill a peer.
#[cfg(unix)]
pub struct SignalStreams {
    interrupt: Option<tokio::signal::unix::Signal>,
    terminate: Option<tokio::signal::unix::Signal>,
    hangup: Option<tokio::signal::unix::Signal>,
    user1: Option<tokio::signal::unix::Signal>,
}

#[cfg(unix)]
impl SignalStreams {
    pub fn install() -> SignalStreams {
        use tokio::signal::unix::{SignalKind, signal};

        SignalStreams {
            interrupt: signal(SignalKind::interrupt())
                .inspect_err(|error| tracing::warn!(%error, "no SIGINT handler"))
                .ok(),
            terminate: signal(SignalKind::terminate())
                .inspect_err(|error| tracing::warn!(%error, "no SIGTERM handler"))
                .ok(),
            hangup: signal(SignalKind::hangup())
                .inspect_err(|error| tracing::warn!(%error, "no SIGHUP handler"))
                .ok(),
            user1: signal(SignalKind::user_defined1())
                .inspect_err(|error| tracing::warn!(%error, "no SIGUSR1 handler"))
                .ok(),
        }
    }

    pub async fn next_code(&mut self) -> i32 {
        next_signal_code(
            &mut self.interrupt,
            &mut self.terminate,
            &mut self.hangup,
            &mut self.user1,
        )
        .await
    }
}

#[cfg(windows)]
pub struct SignalStreams {
    ctrl_c: Option<tokio::signal::windows::CtrlC>,
}

#[cfg(windows)]
impl SignalStreams {
    pub fn install() -> SignalStreams {
        SignalStreams {
            ctrl_c: tokio::signal::windows::ctrl_c()
                .inspect_err(|error| tracing::warn!(%error, "no Ctrl-C handler"))
                .ok(),
        }
    }

    pub async fn next_code(&mut self) -> i32 {
        match self.ctrl_c.as_mut() {
            Some(ctrl_c) => {
                ctrl_c.recv().await;
            }
            None => std::future::pending::<()>().await,
        }
        130
    }
}

/// Wait for the next SIGINT, SIGTERM, SIGHUP, or SIGUSR1 and map it to its code.
/// SIGUSR1 is [`PEER_REBUILD_SIGNAL_CODE`], a peer re-exec, not a quit.
#[cfg(unix)]
pub(crate) async fn next_signal_code(
    sigint: &mut Option<tokio::signal::unix::Signal>,
    sigterm: &mut Option<tokio::signal::unix::Signal>,
    sighup: &mut Option<tokio::signal::unix::Signal>,
    sigusr1: &mut Option<tokio::signal::unix::Signal>,
) -> i32 {
    tokio::select! {
        _ = recv_optional_unix_signal(sigint) => 130,
        _ = recv_optional_unix_signal(sigterm) => 143,
        _ = recv_optional_unix_signal(sighup) => 129,
        _ = recv_optional_unix_signal(sigusr1) => PEER_REBUILD_SIGNAL_CODE,
    }
}

/// Await one recv on an optional unix signal stream, or pend forever if absent.
#[cfg(unix)]
async fn recv_optional_unix_signal(sig: &mut Option<tokio::signal::unix::Signal>) {
    if let Some(s) = sig.as_mut() {
        let _ = s.recv().await;
    } else {
        std::future::pending::<()>().await;
    }
}
