//! Layer 1: PTY management (spawn, inject keys, resize, drain output).

use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use anyhow::{Context, Result};
use portable_pty::{ExitStatus, PtySize, native_pty_system};
use xai_grok_test_support::{TestProcessTree, TestSandbox, process_has_exited_without_reap};
use xai_tty_utils::ProcessGroup;

const PTY_DROP_REAP_TIMEOUT: Duration = Duration::from_millis(250);
/// Grace after group SIGTERM before SIGKILL so a responsive child can run TERM cleanup. Wedged children fall through to SIGKILL.
/// Detached-background reap is the pager quit path, not this grace.
const PTY_DROP_TERM_GRACE: Duration = Duration::from_millis(500);
const PTY_REAP_POLL: Duration = Duration::from_millis(10);
const PENDING_STATUS_ERROR: &str = "exit observed but status unavailable";

/// Raw key byte constants for terminal input injection.
pub mod keys {
    pub const J: &[u8] = b"j";
    pub const K: &[u8] = b"k";
    pub const Q: &[u8] = b"q";
    pub const DOWN: &[u8] = b"\x1b[B";
    pub const UP: &[u8] = b"\x1b[A";
    pub const RIGHT: &[u8] = b"\x1b[C";
    pub const PGDN: &[u8] = b"\x1b[6~";
    pub const PGUP: &[u8] = b"\x1b[5~";
    pub const ENTER: &[u8] = b"\r";
    /// Ctrl+N — leave the welcome home screen into an explicit new session.
    pub const CTRL_N: &[u8] = b"\x0e";
    pub const CTRL_C: &[u8] = b"\x03";
    /// Ctrl+R (0x12): prompt history search or the scrollback mouse-reporting toggle.
    pub const CTRL_R: &[u8] = b"\x12";
    pub const ESC: &[u8] = b"\x1b";
    /// F2 (SS3 `ESC O Q`, the xterm encoding crossterm parses): opens the settings modal.
    pub const F2: &[u8] = b"\x1bOQ";
}

/// One explicit environment mutation applied after the TestSandbox baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvOp<'a> {
    Set(&'a OsStr, &'a OsStr),
    Remove(&'a OsStr),
}

impl<'a> EnvOp<'a> {
    pub fn set(key: &'a str, value: &'a str) -> Self {
        Self::Set(OsStr::new(key), OsStr::new(value))
    }

    pub const fn set_os(key: &'a OsStr, value: &'a OsStr) -> Self {
        Self::Set(key, value)
    }

    pub fn remove(key: &'a str) -> Self {
        Self::Remove(OsStr::new(key))
    }

    pub const fn remove_os(key: &'a OsStr) -> Self {
        Self::Remove(key)
    }
}

#[derive(Debug)]
pub(crate) enum PtyRead {
    Chunk(Vec<u8>),
    Timeout,
    Closed,
}

/// Low-level PTY controller: spawns a child process inside a PTY and provides methods to inject input, resize, and drain output.
pub struct PtyController {
    child: Box<dyn portable_pty::Child + Send>,
    process_tree: Option<TestProcessTree>,
    /// Strong handle for ProcessScope enrollment (Weak stays live while owned).
    process_group: Option<Arc<ProcessGroup>>,
    exit_status: Option<ExitStatus>,
    exit_observed: bool,
    spawn_pid: Option<u32>,
    // portable-pty's Unix kill may reap and cache status through Child::try_wait.
    #[cfg(unix)]
    portable_kill_may_have_reaped: bool,
    #[cfg(test)]
    status_cache_count: usize,
    #[cfg(test)]
    tree_release_count: usize,
    writer: Box<dyn Write + Send>,
    reader_rx: mpsc::Receiver<Vec<u8>>,
    #[allow(dead_code)] // Kept alive to hold the PTY open; used by resize().
    master: Box<dyn portable_pty::MasterPty + Send>,
}

impl PtyController {
    /// Inherit the parent environment for terminal-brand probes, grok-wrap tests, and other fixtures that test inherited host env.
    /// Content-backed pager launches must use [`Self::spawn_in_sandbox`].
    pub fn spawn_inherited_env(
        binary: &Path,
        size: PtySize,
        args: &[&str],
        env: &[(&str, &str)],
        cwd: Option<&Path>,
    ) -> Result<Self> {
        let operations = set_operations(env);
        Self::spawn_inner(binary, size, args, &operations, cwd, None)
    }

    /// Spawn from a [`TestSandbox`] baseline plus typed per-process Set/Remove operations.
    /// The sandbox remains owned by the caller.
    pub fn spawn_in_sandbox(
        binary: &Path,
        size: PtySize,
        args: &[&str],
        sandbox: &TestSandbox,
        env: &[EnvOp<'_>],
        cwd: Option<&Path>,
    ) -> Result<Self> {
        Self::spawn_inner(binary, size, args, env, cwd, Some(sandbox))
    }

    fn spawn_inner(
        binary: &Path,
        size: PtySize,
        args: &[&str],
        env: &[EnvOp<'_>],
        cwd: Option<&Path>,
        sandbox: Option<&TestSandbox>,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(size)?;

        // Own fork/exec: portable-pty has no pre_exec, and the child must arm PDEATHSIG so a SIGKILLed runner (no Drop) cannot leak it.
        #[cfg(unix)]
        let child: Box<dyn portable_pty::Child + Send> = {
            let env_map = crate::pty_spawn::compute_child_env(sandbox, env);
            let dir = crate::pty_spawn::resolve_child_cwd(cwd, env_map.get(OsStr::new("HOME")))?;
            let mut cmd = std::process::Command::new(binary);
            cmd.args(args).env_clear().envs(&env_map).current_dir(dir);
            Box::new(crate::pty_spawn::spawn_pty_session_child(
                cmd,
                pair.master.as_ref(),
            )?)
        };
        // Windows keeps portable-pty's spawn; Job enrollment is a best-effort
        // post-spawn attachment, so a very short-lived descendant may escape
        // before enrollment; diagnostics preserve that downgrade.
        #[cfg(windows)]
        let child = {
            let mut cmd = portable_pty::CommandBuilder::new(binary);
            for arg in args {
                cmd.arg(*arg);
            }
            if let Some(dir) = cwd {
                cmd.cwd(dir);
            }
            if let Some(sandbox) = sandbox {
                cmd.env_clear();
                sandbox.apply_to_command_builder(&mut cmd);
            }
            crate::pty_spawn::apply_child_env(&mut cmd, env);
            #[allow(clippy::disallowed_methods)]
            pair.slave.spawn_command(cmd)?
        };
        #[cfg(unix)]
        let process_pid = child
            .process_id()
            .or_else(|| pair.master.process_group_leader().map(|pid| pid as u32));
        #[cfg(windows)]
        let process_pid = child.process_id();
        let process_tree = process_pid.map(|pid| TestProcessTree::attach(pid, "grok PTY child"));
        // Strong Arc so the scope's Weak stays live while this controller owns the child. Release drops it first.
        let process_group = process_pid.and_then(|pid| {
            let mut group = ProcessGroup::new().ok()?;
            group.attach_pid(pid).ok()?;
            let group = Arc::new(group);
            xai_tty_utils::global_process_scope().register(&group);
            Some(group)
        });
        // Attachment failures remain recorded by TestProcessTree and show up in process_tree_diagnostics() on every harness timeout
        // Drop the slave so we get EOF when the child exits.
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let reader_rx = spawn_reader(reader);

        Ok(Self {
            child,
            process_tree,
            process_group,
            exit_status: None,
            exit_observed: false,
            spawn_pid: process_pid,
            #[cfg(unix)]
            portable_kill_may_have_reaped: false,
            #[cfg(test)]
            status_cache_count: 0,
            #[cfg(test)]
            tree_release_count: 0,
            writer,
            reader_rx,
            master: pair.master,
        })
    }

    /// Write raw key bytes into the PTY stdin.
    pub fn inject_keys(&mut self, keys: &[u8]) -> Result<()> {
        self.writer
            .write_all(keys)
            .context("failed to write to PTY stdin")
    }

    /// Resize the PTY (sends SIGWINCH to the child). Arguments are `(rows, cols)`.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to resize PTY")
    }

    /// Drain the reader channel, collecting all available chunks within `timeout`.
    pub fn drain_output(&self, timeout: Duration) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        let deadline = std::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match self.reader_rx.recv_timeout(remaining) {
                Ok(chunk) => chunks.push(chunk),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        chunks
    }

    /// Send 'q' to trigger the pager's quit handler and wait for exit.
    ///
    /// Key injection is best-effort: the child may have already exited (e.g. no ACP server), so write failures are silently ignored.
    pub fn quit(&mut self) -> Result<()> {
        let _ = self.inject_keys(keys::Q);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if is_quit_complete(self.poll_exit_code())? {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                self.cleanup_descendants();
                self.kill_portable_child()?;
                self.wait_child_bounded(Duration::from_secs(1))
                    .context("failed to wait for pager child after kill")?
                    .context("pager child did not exit within 1s after kill")?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Receive one chunk, distinguishing timeout from reader EOF.
    ///
    /// Processing each chunk inline preserves inter-chunk timing.
    pub(crate) fn recv_chunk(&self, timeout: Duration) -> PtyRead {
        match self.reader_rx.recv_timeout(timeout) {
            Ok(chunk) => PtyRead::Chunk(chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => PtyRead::Timeout,
            Err(mpsc::RecvTimeoutError::Disconnected) => PtyRead::Closed,
        }
    }

    /// Return true only while the child is live; pending status is non-running.
    pub fn is_running(&mut self) -> Result<bool> {
        self.poll_exit_code()
            .map(|state| state == PtyExitPoll::Running)
    }

    /// Poll once without collapsing pending status, liveness, or query errors.
    /// Repeated calls return cached exit status without querying a reaped child.
    pub fn poll_exit_code(&mut self) -> Result<PtyExitPoll<u32>> {
        let poll = self
            .poll_exit_status()
            .map(|status| status.map(|status| status.exit_code()));
        classify_exit_poll(poll, self.exit_observed)
    }

    /// Poll until exit or `timeout` without collapsing lifecycle states.
    /// Returns [`PtyExitPoll::PendingStatus`] immediately because the child is already non-running.
    /// [`PtyExitPoll::Running`] is returned only when the deadline expires while the child remains live.
    pub fn wait_exit_code(&mut self, timeout: Duration) -> Result<PtyExitPoll<u32>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(state) =
                resolve_wait_poll(self.poll_exit_code(), std::time::Instant::now() >= deadline)?
            {
                return Ok(state);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Child PID while the direct child is live.
    /// Once reaped, returns `None` so callers cannot signal a recycled PID.
    pub fn child_pid(&self) -> Option<u32> {
        (!self.exit_observed && self.exit_status.is_none())
            .then_some(self.spawn_pid)
            .flatten()
    }

    /// Deliver a signal directly to the child (unix), bypassing the PTY line discipline.
    /// Exercises the real SIGINT/SIGTERM/SIGHUP paths (distinct from injected Ctrl+C key bytes, which are key events under raw mode).
    /// Call before the child is reaped: a reaped pid can be reused.
    #[cfg(unix)]
    pub fn send_signal(&self, signal: i32) -> Result<()> {
        let pid = self.child_pid().context("no child pid to signal")?;
        // SAFETY: libc::kill has no memory-safety preconditions, and child_pid()
        // only yields a positive live-child pid (never the kill(0)/kill(-1) broadcast).
        let rc = unsafe { libc::kill(pid as libc::pid_t, signal) };
        if rc != 0 {
            return Err(anyhow::Error::from(std::io::Error::last_os_error())).context("libc::kill");
        }
        Ok(())
    }

    fn poll_exit_status(&mut self) -> Result<Option<ExitStatus>> {
        if let Some(status) = self.exit_status.clone() {
            return Ok(Some(status));
        }
        #[cfg(unix)]
        {
            if let Some(pid) = self.spawn_pid {
                match observe_exit_before_reap(
                    process_has_exited_without_reap(pid, "PTY child"),
                    self.exit_observed,
                    self.portable_kill_may_have_reaped,
                ) {
                    Ok(ExitObservation::Running) => return Ok(None),
                    Ok(ExitObservation::Exited) => self.observe_exit_and_cleanup_tree(),
                    Ok(ExitObservation::StatusAlreadyConsumed) => {
                        self.observe_exit_and_cleanup_tree();
                        return self.recover_consumed_status();
                    }
                    Err(error) => {
                        return Err(error).context("failed to observe PTY child exit");
                    }
                }
            }
        }
        self.try_wait_and_cache()
    }

    fn try_wait_and_cache(&mut self) -> Result<Option<ExitStatus>> {
        let status = self
            .child
            .try_wait()
            .context("failed to query PTY child status")?;
        if let Some(status) = status {
            #[cfg(windows)]
            self.cleanup_descendants();
            self.cache_reaped_status(status.clone());
            return Ok(Some(status));
        }
        Ok(None)
    }

    fn cache_reaped_status(&mut self, status: ExitStatus) {
        if self.exit_status.is_none() {
            self.release_process_tree();
            cache_exit_status(
                &mut self.exit_status,
                &mut self.exit_observed,
                &mut self.spawn_pid,
                status,
            );
            #[cfg(test)]
            {
                self.status_cache_count += 1;
            }
        }
    }

    #[cfg(unix)]
    fn observe_exit_and_cleanup_tree(&mut self) {
        if !self.exit_observed {
            self.exit_observed = true;
            self.cleanup_descendants();
        }
    }

    #[cfg(unix)]
    fn recover_consumed_status(&mut self) -> Result<Option<ExitStatus>> {
        let status = recover_consumed_status(self.child.try_wait())
            .context("failed to recover PTY child status after it was consumed")?;
        self.cache_reaped_status(status.clone());
        Ok(Some(status))
    }

    fn kill_portable_child(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.portable_kill_may_have_reaped = true;
        }
        self.child.kill()
    }

    /// Process-group/job enrollment state.
    pub fn process_tree_diagnostics(&self) -> String {
        self.process_tree
            .as_ref()
            .map(TestProcessTree::diagnostic_summary)
            .unwrap_or_else(|| "tree_unavailable=true".to_owned())
    }

    fn kill_tree_best_effort(&self) {
        if let Some(tree) = &self.process_tree {
            let _ = tree.kill();
        }
    }

    /// One group SIGTERM so a foreground `sleep` that already exists can die,
    /// and the shell can run its TERM trap during the grace. READY is written
    /// before `sleep` is forked, so this signal can also hit only the shell.
    /// The shell then starts `sleep` and defers the trap. That later `sleep`
    /// is stopped in the grace wait, not here. Shell SIGKILL stays in Drop,
    /// after that grace. Returns whether a graceful signal was delivered
    /// (Drop spends the grace only then). Does not call `tree.terminate()`
    /// on that path, and does not signal the group a second time.
    fn terminate_tree_best_effort(&self) -> bool {
        #[cfg(unix)]
        {
            // Both captures are before SIGTERM. A later child can be the
            // trap's `echo` and must not be killed from this snapshot.
            let blockers = self.term_trap_blocker_pids();
            let foreign_foreground = self.foreign_foreground_pgid();
            let mut signaled = self.signal_leader_group(libc::SIGTERM);
            // Job-control sleep is its own group. SIGTERM ends it. SIGKILL of
            // that group stays off this path so a mis-read cannot kill the shell.
            if signal_pgid(foreign_foreground, libc::SIGTERM) {
                signaled = true;
            }
            if !signaled {
                signaled = self.signal_direct_child(libc::SIGTERM);
            }
            if signaled {
                // A stopped process keeps SIGTERM pending until SIGCONT.
                // The shell pid and the pre-SIGTERM blocker pids only.
                // kill(-pgid, SIGCONT) would also hit a trap child forked since.
                let _ = self.signal_direct_child(libc::SIGCONT);
                sigcont_pids(&blockers);
            }
            // A pre-signal child that is `sleep` may have SIGTERM blocked.
            // SIGKILL that pid only. Echo is not in this snapshot, and a
            // pid that is no longer sleep is not killed.
            let killed_blockers = sigkill_sleeper_pids(&blockers);
            if signaled || killed_blockers {
                // Graceful SIGTERM already went out, or the existing sleeper
                // was SIGKILL'd. Do not call tree.terminate() in this same
                // step. The grace wait keeps looking for a sleep forked
                // after this return. Drop SIGKILLs the shell only after
                // PTY_DROP_TERM_GRACE.
                return true;
            }
        }
        self.process_tree
            .as_ref()
            .is_some_and(|tree| tree.terminate().is_ok())
    }

    /// SIGKILL a direct child only when it is `sleep`. Does not signal the
    /// group. The shell stays alive while its TERM trap runs, and that trap's
    /// `echo` is a direct child that must keep running.
    #[cfg(unix)]
    fn sigkill_sleeper_children(&self) {
        let Some(pid) = self.child_pid() else {
            return;
        };
        let pid = pid as libc::pid_t;
        if pid <= 1 {
            return;
        }
        sigkill_sleeper_pids(&direct_child_pids(pid));
    }

    /// Direct children of the trapping process, snapshotted before SIGTERM.
    #[cfg(unix)]
    fn term_trap_blocker_pids(&self) -> Vec<libc::pid_t> {
        let Some(pid) = self.child_pid() else {
            return Vec::new();
        };
        direct_child_pids(pid as libc::pid_t)
    }

    /// SIGTERM or SIGCONT to the direct child when it is not the group leader.
    /// No-op for pid 0 or 1, so this cannot broadcast.
    #[cfg(unix)]
    fn signal_direct_child(&self, signal: i32) -> bool {
        let Some(pid) = self.child_pid() else {
            return false;
        };
        let pid = pid as libc::pid_t;
        if pid <= 1 {
            return false;
        }
        // SAFETY: pid is the live direct child and is greater than 1.
        unsafe { libc::kill(pid, signal) == 0 }
    }

    /// Foreground pgid from `tcgetpgrp`, or `None` when that group is the
    /// shell, the shell's process group, or the caller's group.
    /// Call only before SIGTERM. A later `tcgetpgrp` can be the trap command.
    #[cfg(unix)]
    fn foreign_foreground_pgid(&self) -> Option<libc::pid_t> {
        let shell = self.child_pid()? as libc::pid_t;
        let foreground = self.master.process_group_leader()?;
        if foreground <= 1 || foreground == shell {
            return None;
        }
        // SAFETY: getpgrp and getpgid do not signal.
        let own = unsafe { libc::getpgrp() };
        if foreground == own {
            return None;
        }
        let shell_pgid = unsafe { libc::getpgid(shell) };
        if shell_pgid > 1 && foreground == shell_pgid {
            return None;
        }
        Some(foreground)
    }

    /// SIGTERM or another signal to the direct child's process group.
    /// No-op for pgid <= 1 or the caller's group. The shell does not have to
    /// be the leader: foreground `sleep` shares that group, and a positive-pid
    /// SIGTERM leaves `sleep` running so the trap never starts.
    #[cfg(unix)]
    fn signal_leader_group(&self, signal: i32) -> bool {
        let Some(pid) = self.child_pid() else {
            return false;
        };
        let pid = pid as libc::pid_t;
        if pid <= 1 {
            return false;
        }
        // SAFETY: getpgid and getpgrp do not signal. pid is the live direct child, not 0 or -1.
        let pgid = unsafe { libc::getpgid(pid) };
        let own = unsafe { libc::getpgrp() };
        if pgid <= 1 || pgid == own {
            return false;
        }
        // SAFETY: pgid is this child's group, greater than 1, and not the caller's group.
        // This is not kill(0) or kill(-1).
        unsafe { libc::kill(-pgid, signal) == 0 }
    }

    fn release_process_tree(&mut self) {
        // Drop ProcessScope strong handle first so Weak cannot kill a reused PID.
        self.process_group = None;
        if let Some(mut tree) = self.process_tree.take() {
            tree.release();
            #[cfg(test)]
            {
                self.tree_release_count += 1;
            }
        }
    }

    fn cleanup_descendants(&mut self) {
        self.kill_tree_best_effort();
        self.release_process_tree();
    }

    /// Drop's one grace wait. Same deadline as `wait_child_bounded(grace)`.
    /// Every poll looks for a direct child named `sleep` and SIGKILLs only
    /// that pid, so a `sleep` forked after the group SIGTERM still dies
    /// inside this grace and the shell can run its trap. `echo` is not
    /// sleep, so it is not killed, and the group is not signaled again.
    /// This is not a second grace: Drop does not wait again afterward.
    /// One `PTY_REAP_POLL` past the deadline matches `wait_child_bounded`.
    fn wait_grace_stopping_sleeper(&mut self, grace: Duration) -> Result<Option<ExitStatus>> {
        let deadline = std::time::Instant::now() + grace;
        loop {
            #[cfg(unix)]
            self.sigkill_sleeper_children();
            if let Some(status) = self.poll_exit_status()? {
                return Ok(Some(status));
            }
            if std::time::Instant::now() >= deadline {
                if self.exit_observed {
                    anyhow::bail!(PENDING_STATUS_ERROR);
                }
                return Ok(None);
            }
            std::thread::sleep(PTY_REAP_POLL);
        }
    }

    fn wait_child_bounded(&mut self, timeout: Duration) -> Result<Option<ExitStatus>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(status) = self.poll_exit_status()? {
                return Ok(Some(status));
            }
            if std::time::Instant::now() >= deadline {
                if self.exit_observed {
                    anyhow::bail!(PENDING_STATUS_ERROR);
                }
                return Ok(None);
            }
            std::thread::sleep(PTY_REAP_POLL);
        }
    }
}

impl Drop for PtyController {
    fn drop(&mut self) {
        // Graceful first: one group SIGTERM, then during this same
        // PTY_DROP_TERM_GRACE keep stopping a direct child named sleep so
        // the shell can run its TERM trap. Echo is not signaled again.
        // Shell SIGKILL is only after this grace returns.
        if self.exit_status.is_none() && !self.exit_observed && self.terminate_tree_best_effort() {
            let _ = self.wait_grace_stopping_sleeper(PTY_DROP_TERM_GRACE);
        }
        // Hard stop: SIGKILL the group, kill the direct child, reap bounded.
        if self.exit_status.is_none() {
            self.cleanup_descendants();
            let _ = self.kill_portable_child();
            let _ = self.wait_child_bounded(PTY_DROP_REAP_TIMEOUT);
        }
        self.release_process_tree();
    }
}

#[cfg(unix)]
#[derive(Debug, Eq, PartialEq)]
enum ExitObservation {
    Running,
    Exited,
    StatusAlreadyConsumed,
}

#[cfg(unix)]
fn observe_exit_before_reap(
    observation: io::Result<bool>,
    exit_observed: bool,
    portable_kill_may_have_reaped: bool,
) -> io::Result<ExitObservation> {
    match observation {
        Ok(false) => Ok(ExitObservation::Running),
        Ok(true) => Ok(ExitObservation::Exited),
        Err(error)
            if error.raw_os_error() == Some(libc::ECHILD)
                && (exit_observed || portable_kill_may_have_reaped) =>
        {
            Ok(ExitObservation::StatusAlreadyConsumed)
        }
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn recover_consumed_status(status: io::Result<Option<ExitStatus>>) -> io::Result<ExitStatus> {
    status?.ok_or_else(|| io::Error::other("PTY child status was consumed without being cached"))
}

/// SIGCONT specific pids captured before SIGTERM. Skips 0 and 1.
#[cfg(unix)]
fn sigcont_pids(pids: &[libc::pid_t]) {
    for pid in pids {
        if *pid <= 1 {
            continue;
        }
        // SAFETY: pid is a positive non-init pid, not kill(0) or kill(-1).
        unsafe { libc::kill(*pid, libc::SIGCONT) };
    }
}

/// Signal one process group captured before SIGTERM. `None` and pgid <= 1 are no-ops.
#[cfg(unix)]
fn signal_pgid(pgid: Option<libc::pid_t>, signal: i32) -> bool {
    let Some(pgid) = pgid else {
        return false;
    };
    if pgid <= 1 {
        return false;
    }
    // SAFETY: pgid was refused when it was the shell, the shell's group, or the caller.
    unsafe { libc::kill(-pgid, signal) == 0 }
}

/// True only when this pid is the foreground `sleep`, not the trap's `echo`.
/// The shell stays alive while the trap runs, so a live parent is not proof.
/// `echo` does not catch TERM and does not survive SIGKILL.
#[cfg(all(unix, target_os = "linux"))]
fn is_foreground_sleeper(pid: libc::pid_t) -> bool {
    if pid <= 1 {
        return false;
    }
    let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) else {
        return false;
    };
    let comm = comm.trim();
    if comm == "echo" || comm == "sh" || comm == "dash" || comm == "bash" || comm == "ash" {
        return false;
    }
    if comm == "sleep" {
        return true;
    }
    // busybox applets keep comm as "busybox" and put the applet in argv0.
    let Ok(bytes) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    let Some(first) = bytes.split(|byte| *byte == 0).next() else {
        return false;
    };
    if first.is_empty() {
        return false;
    }
    let arg = String::from_utf8_lossy(first);
    arg.rsplit('/').next() == Some("sleep")
}

#[cfg(all(unix, not(target_os = "linux")))]
fn is_foreground_sleeper(_pid: libc::pid_t) -> bool {
    false
}

/// SIGKILL only pids that are `sleep`. Other children, including `echo`, stay.
#[cfg(unix)]
fn sigkill_sleeper_pids(pids: &[libc::pid_t]) -> bool {
    let mut killed = false;
    for pid in pids {
        if is_foreground_sleeper(*pid) && sigkill_pids(&[*pid]) {
            killed = true;
        }
    }
    killed
}

/// SIGKILL specific pids. Skips 0, 1, and negatives so this cannot broadcast.
#[cfg(unix)]
fn sigkill_pids(pids: &[libc::pid_t]) -> bool {
    let mut killed = false;
    for pid in pids {
        if *pid <= 1 {
            continue;
        }
        // SAFETY: pid is a positive non-init pid, not kill(0) or kill(-1).
        if unsafe { libc::kill(*pid, libc::SIGKILL) == 0 } {
            killed = true;
        }
    }
    killed
}

/// Direct children listed in `/proc/<pid>/task/<pid>/children`.
/// Empty when that file is missing or the child has not forked yet.
#[cfg(all(unix, target_os = "linux"))]
fn direct_child_pids(parent: libc::pid_t) -> Vec<libc::pid_t> {
    if parent <= 1 {
        return Vec::new();
    }
    let path = format!("/proc/{parent}/task/{parent}/children");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.split_whitespace()
        .filter_map(|token| token.parse().ok())
        .filter(|pid| *pid > 1 && *pid != parent)
        .collect()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn direct_child_pids(_parent: libc::pid_t) -> Vec<libc::pid_t> {
    Vec::new()
}

/// Only [`Self::Running`] is live. [`Self::PendingStatus`] has already exited and cleaned descendants; portable-pty has not yielded status yet.
#[must_use = "PTY exit state and poll errors must be handled explicitly"]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtyExitPoll<T> {
    /// The child exited and its cached terminal status is available.
    Exited(T),
    /// The child is non-running, but its portable-pty status is not yet available.
    PendingStatus,
    /// The child is still live.
    Running,
}

fn classify_exit_poll<T, E>(
    poll: std::result::Result<Option<T>, E>,
    exit_observed: bool,
) -> std::result::Result<PtyExitPoll<T>, E> {
    match poll {
        Ok(Some(status)) => Ok(PtyExitPoll::Exited(status)),
        Ok(None) if exit_observed => Ok(PtyExitPoll::PendingStatus),
        Ok(None) => Ok(PtyExitPoll::Running),
        Err(error) => Err(error),
    }
}

fn resolve_wait_poll<T, E>(
    poll: std::result::Result<PtyExitPoll<T>, E>,
    deadline_reached: bool,
) -> std::result::Result<Option<PtyExitPoll<T>>, E> {
    match poll? {
        PtyExitPoll::Running if !deadline_reached => Ok(None),
        state => Ok(Some(state)),
    }
}

fn is_quit_complete<T, E>(
    poll: std::result::Result<PtyExitPoll<T>, E>,
) -> std::result::Result<bool, E> {
    match poll? {
        PtyExitPoll::Exited(_) | PtyExitPoll::PendingStatus => Ok(true),
        PtyExitPoll::Running => Ok(false),
    }
}

fn cache_exit_status(
    exit_status: &mut Option<ExitStatus>,
    exit_observed: &mut bool,
    spawn_pid: &mut Option<u32>,
    status: ExitStatus,
) {
    *exit_status = Some(status);
    *exit_observed = true;
    *spawn_pid = None;
}

fn set_operations<'a>(env: &'a [(&'a str, &'a str)]) -> Vec<EnvOp<'a>> {
    env.iter()
        .map(|(key, value)| EnvOp::set(key, value))
        .collect()
}

/// Spawn a background thread that reads from the PTY master and sends chunks over an `mpsc` channel.
/// The reader is blocking (WezTerm pattern), so it must live on its own thread.
fn spawn_reader(mut reader: Box<dyn Read + Send>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("pty-reader".into())
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        })
        .expect("failed to spawn pty-reader thread");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_poll_distinguishes_pending_running_and_errors() {
        assert_eq!(
            classify_exit_poll::<u32, &'static str>(Ok(None), true),
            Ok(PtyExitPoll::PendingStatus)
        );
        assert_eq!(
            classify_exit_poll::<u32, &'static str>(Ok(None), false),
            Ok(PtyExitPoll::Running)
        );
        assert_eq!(
            classify_exit_poll::<u32, &'static str>(Err("poll failed"), false),
            Err("poll failed")
        );
    }

    #[test]
    fn wait_deadline_preserves_pending_running_and_errors() {
        assert_eq!(
            resolve_wait_poll::<u32, &'static str>(Ok(PtyExitPoll::PendingStatus), false),
            Ok(Some(PtyExitPoll::PendingStatus))
        );
        assert_eq!(
            resolve_wait_poll::<u32, &'static str>(Ok(PtyExitPoll::PendingStatus), true),
            Ok(Some(PtyExitPoll::PendingStatus))
        );
        assert_eq!(
            resolve_wait_poll::<u32, &'static str>(Ok(PtyExitPoll::Running), false),
            Ok(None)
        );
        assert_eq!(
            resolve_wait_poll::<u32, &'static str>(Ok(PtyExitPoll::Running), true),
            Ok(Some(PtyExitPoll::Running))
        );
        assert_eq!(
            resolve_wait_poll::<u32, &'static str>(Err("poll failed"), true),
            Err("poll failed")
        );
    }

    #[test]
    fn quit_completion_accepts_non_running_states_and_propagates_errors() {
        assert_eq!(
            is_quit_complete::<u32, &'static str>(Ok(PtyExitPoll::Exited(0))),
            Ok(true)
        );
        assert_eq!(
            is_quit_complete::<u32, &'static str>(Ok(PtyExitPoll::PendingStatus)),
            Ok(true)
        );
        assert_eq!(
            is_quit_complete::<u32, &'static str>(Ok(PtyExitPoll::Running)),
            Ok(false)
        );
        assert_eq!(
            is_quit_complete::<u32, &'static str>(Err("poll failed")),
            Err("poll failed")
        );
    }

    #[cfg(unix)]
    #[test]
    fn observed_exit_echild_is_typed_only_after_portable_reap_capability() {
        let echild = || io::Error::from_raw_os_error(libc::ECHILD);
        let unrelated = io::Error::other("unrelated poll failure");

        assert_eq!(
            observe_exit_before_reap(Err(echild()), false, true).unwrap(),
            ExitObservation::StatusAlreadyConsumed
        );
        assert_eq!(
            observe_exit_before_reap(Err(echild()), true, false).unwrap(),
            ExitObservation::StatusAlreadyConsumed
        );
        assert_eq!(
            observe_exit_before_reap(Err(echild()), false, false)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ECHILD)
        );
        assert_eq!(
            observe_exit_before_reap(Err(unrelated), true, true)
                .unwrap_err()
                .to_string(),
            "unrelated poll failure"
        );
    }

    #[cfg(unix)]
    #[test]
    fn consumed_status_recovery_requires_a_cached_status() {
        let status = ExitStatus::with_exit_code(0);
        assert_eq!(
            recover_consumed_status(Ok(Some(status.clone())))
                .unwrap()
                .exit_code(),
            0
        );
        assert!(recover_consumed_status(Ok(None)).is_err());
        assert_eq!(
            recover_consumed_status(Err(io::Error::other("real status failure")))
                .unwrap_err()
                .to_string(),
            "real status failure"
        );
    }

    #[cfg(unix)]
    #[test]
    fn observed_exit_then_echild_recovers_cached_status_once() {
        let sandbox = TestSandbox::new();
        let mut controller = PtyController::spawn_in_sandbox(
            Path::new("/bin/sh"),
            PtySize {
                rows: 8,
                cols: 40,
                pixel_width: 0,
                pixel_height: 0,
            },
            &["-c", "exit 7"],
            &sandbox,
            &[],
            None,
        )
        .expect("spawn PTY exit fixture");
        let pid = controller.child_pid().expect("live child pid");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !process_has_exited_without_reap(pid, "PTY exit fixture").expect("observe child exit")
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }

        controller.observe_exit_and_cleanup_tree();
        controller
            .kill_portable_child()
            .expect("portable kill consumes the exited child status");
        assert!(controller.portable_kill_may_have_reaped);
        assert_eq!(controller.tree_release_count, 1);
        assert_eq!(
            process_has_exited_without_reap(pid, "PTY exit fixture")
                .expect_err("consumed status must produce ECHILD")
                .raw_os_error(),
            Some(libc::ECHILD)
        );

        assert_eq!(
            controller
                .poll_exit_status()
                .expect("recover cached portable status")
                .expect("cached status")
                .exit_code(),
            7
        );
        assert_eq!(controller.status_cache_count, 1);
        assert_eq!(controller.tree_release_count, 1);
        assert_eq!(
            controller.poll_exit_status().unwrap().unwrap().exit_code(),
            7
        );
        assert_eq!(controller.status_cache_count, 1);
        assert_eq!(controller.tree_release_count, 1);
        assert_eq!(controller.child_pid(), None);
    }

    #[cfg(unix)]
    #[test]
    fn pty_waits_are_idempotent_and_pid_is_hidden_after_reap() {
        let sandbox = TestSandbox::new();
        let mut controller = PtyController::spawn_in_sandbox(
            Path::new("/bin/sh"),
            PtySize {
                rows: 8,
                cols: 40,
                pixel_width: 0,
                pixel_height: 0,
            },
            &["-c", "exit 7"],
            &sandbox,
            &[],
            None,
        )
        .expect("spawn PTY exit fixture");
        assert!(controller.child_pid().is_some());
        assert_eq!(
            controller.wait_exit_code(Duration::from_secs(2)).unwrap(),
            PtyExitPoll::Exited(7)
        );
        assert_eq!(
            controller.wait_exit_code(Duration::ZERO).unwrap(),
            PtyExitPoll::Exited(7)
        );
        assert_eq!(controller.poll_exit_code().unwrap(), PtyExitPoll::Exited(7));
        assert!(!controller.is_running().unwrap());
        assert_eq!(controller.status_cache_count, 1);
        assert_eq!(controller.tree_release_count, 1);
        assert_eq!(controller.child_pid(), None);
        assert!(controller.send_signal(libc::SIGTERM).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn pty_drop_tree_cleanup_is_bounded_and_reaps_grandchild() {
        let sandbox = TestSandbox::new();
        let pid_file = sandbox.temp_dir().join("pty-grandchild.pid");
        let pid_path = pid_file.to_string_lossy().into_owned();
        let controller = PtyController::spawn_in_sandbox(
            Path::new("/bin/sh"),
            PtySize {
                rows: 8,
                cols: 40,
                pixel_width: 0,
                pixel_height: 0,
            },
            &["-c", "sleep 1000 & echo $! > \"$PID_FILE\"; wait"],
            &sandbox,
            &[EnvOp::set("PID_FILE", &pid_path)],
            None,
        )
        .expect("spawn PTY tree fixture");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let grandchild_pid = loop {
            if let Ok(raw) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = raw.trim().parse::<u32>()
            {
                break pid;
            }
            assert!(std::time::Instant::now() < deadline, "pid file timeout");
            std::thread::sleep(Duration::from_millis(10));
        };

        let started = std::time::Instant::now();
        drop(controller);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "PTY Drop exceeded its bounded wait"
        );
        // Grandchild re-parents to pid 1, which may not reap promptly. The contract is that it stops running, not that it is reaped.
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !xai_tty_utils::process_not_running(grandchild_pid)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            xai_tty_utils::process_not_running(grandchild_pid),
            "PTY grandchild leaked after controller Drop"
        );
        let mut cmd = portable_pty::CommandBuilder::new("true");
        crate::pty_spawn::apply_child_env(&mut cmd, &[EnvOp::set("GROK_OSC52_SINK", "1")]);
        assert_eq!(
            cmd.get_env("GROK_OSC52_SINK").and_then(|v| v.to_str()),
            Some("1"),
            "explicit sink scenarios must be able to re-inject the marker"
        );
    }

    /// A TERM-trapping child must run cleanup before SIGKILL. Without the grace the trap never runs and the marker never appears.
    #[cfg(unix)]
    #[test]
    fn pty_drop_grace_lets_a_trapping_child_run_its_term_cleanup() {
        let sandbox = TestSandbox::new();
        let marker = sandbox.temp_dir().join("graceful-term.marker");
        let marker_path = marker.to_string_lossy().into_owned();
        let ready = sandbox.temp_dir().join("trap-installed.ready");
        let ready_path = ready.to_string_lossy().into_owned();
        // The group SIGTERM kills the foreground sleep; sh's wait for it then
        // returns and the TERM trap writes the marker. READY is written after
        // the trap is installed, closing the drop-before-trap race.
        let controller = PtyController::spawn_in_sandbox(
            Path::new("/bin/sh"),
            PtySize {
                rows: 8,
                cols: 40,
                pixel_width: 0,
                pixel_height: 0,
            },
            &[
                "-c",
                "trap 'echo graceful > \"$MARKER\"; exit 0' TERM; : > \"$READY\"; sleep 600",
            ],
            &sandbox,
            &[
                EnvOp::set("MARKER", &marker_path),
                EnvOp::set("READY", &ready_path),
            ],
            None,
        )
        .expect("spawn PTY trap fixture");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !ready.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "trap-installed ready file timeout"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        let started = std::time::Instant::now();
        drop(controller);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "PTY Drop exceeded its bounded wait"
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            marker.exists(),
            "TERM trap never ran: Drop's SIGTERM grace did not let the child clean up"
        );
    }

    #[cfg(unix)]
    #[test]
    fn pty_tree_diagnostics_surface_enrollment_state() {
        let tree = TestProcessTree::attach(u32::MAX, "invalid PTY fixture");
        let diagnostics = tree.diagnostic_summary();
        assert!(diagnostics.contains("tree_label=\"invalid PTY fixture\""));
        assert!(diagnostics.contains("tree_attached=false"));
        assert!(diagnostics.contains("tree_attach_error=Some"));
    }

    #[test]
    fn set_operations_projection_is_set_only() {
        let operations = set_operations(&[("EXPLICIT_MARKER", "set")]);
        assert_eq!(operations, [EnvOp::set("EXPLICIT_MARKER", "set")]);
    }
}
