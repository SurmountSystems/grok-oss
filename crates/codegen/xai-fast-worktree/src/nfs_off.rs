//! Grove worktree operations. Create, remove, and status decline in this build.
//! Redirect telemetry still uses the control socket when an endpoint is set.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use anyhow::Result;

pub(crate) use crate::grove_api::is_safe_worktree_id;
pub use crate::grove_api::{
    CAP_CANCEL_WORKTREE_CREATE, CAP_FORK_FROM_BACKING, CleanArtifactsReply, DetachReply,
    GroveHardFail, NfsAdopted, NfsCreateDecision, NfsStatusView, NfsWorktreeOpts, SalvageReply,
    daemon_capability_class, grove_hard_fail,
};
#[allow(unused_imports)] // re-exported for discovery / execute when those modules are on
pub(crate) use crate::grove_api::{default_grove_creation_mode, nfs_error_blocks_fallback};

pub const WORKTREE_BACKING_DIR: &str = "worktree-backing";

pub fn dest_is_nfs_mount(_path: &Path) -> bool {
    false
}

#[cfg(windows)]
fn folded_components(path: &Path) -> Vec<String> {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

#[cfg(windows)]
pub(crate) fn dest_paths_equivalent(a: &Path, b: &Path) -> bool {
    folded_components(a) == folded_components(b)
}

#[cfg(windows)]
pub(crate) fn dest_path_contains(parent: &Path, child: &Path) -> bool {
    let parent = folded_components(parent);
    let child = folded_components(child);
    child.len() >= parent.len() && child[..parent.len()] == parent[..]
}

#[cfg(not(windows))]
pub(crate) fn dest_paths_equivalent(a: &Path, b: &Path) -> bool {
    a == b
}

#[cfg(not(windows))]
pub(crate) fn dest_path_contains(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent)
}

#[derive(Debug, Clone)]
pub struct NfsWorktreeClient {
    control_sock: Option<PathBuf>,
}

impl NfsWorktreeClient {
    #[must_use]
    pub fn from_opts(opts: &NfsWorktreeOpts) -> Self {
        Self {
            control_sock: opts.control_sock.clone(),
        }
    }

    pub fn detach_worktree(&self, _dest: &Path, _allow_copy: bool) -> Result<DetachReply> {
        anyhow::bail!("not available in this build")
    }

    pub fn salvage_worktree(&self, _dest: &Path, _out: &Path) -> Result<SalvageReply> {
        anyhow::bail!("not available in this build")
    }

    pub fn clean_artifacts(&self, _dest: &Path) -> Result<CleanArtifactsReply> {
        anyhow::bail!("not available in this build")
    }

    pub fn redirect_events(
        &self,
        ack_through: u64,
        timeout: std::time::Duration,
    ) -> Result<(Vec<serde_json::Value>, u64)> {
        let reply = self.call(
            &serde_json::json!({
                "op": "redirect_events",
                "v": 1,
                "ack_through": ack_through,
            }),
            timeout,
        )?;
        redirect_reply(reply)
    }

    /// One newline-delimited exchange on the control socket.
    ///
    /// Connect fails at once when no endpoint is configured or the socket is
    /// absent. The same `timeout` then bounds the write and the read
    /// separately, so a daemon that accepts and then stalls cannot hold either
    /// step longer than `timeout`.
    fn call(
        &self,
        request: &serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value> {
        control_call(self.control_sock.as_deref(), request, timeout)
    }

    pub fn status_for_dir(&self, _dest: &Path) -> Option<NfsStatusView> {
        None
    }

    pub fn source_is_linked_local_view(&self, _source: &Path) -> bool {
        false
    }
}

#[cfg(unix)]
fn control_call(
    path: Option<&Path>,
    request: &serde_json::Value,
    timeout: std::time::Duration,
) -> Result<serde_json::Value> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let Some(path) = path else {
        anyhow::bail!("grove control socket is unreachable");
    };
    let mut stream = connect_control(path, timeout)?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(socket_err)?;
    stream.set_read_timeout(Some(timeout)).map_err(socket_err)?;
    let mut line = request.to_string();
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(socket_err)?;
    let mut reply = String::new();
    BufReader::new(&stream)
        .read_line(&mut reply)
        .map_err(socket_err)?;
    if reply.is_empty() {
        anyhow::bail!("grove control socket is unreachable");
    }
    serde_json::from_str(reply.trim())
        .map_err(|_| anyhow::anyhow!("grove control reply was not json"))
}

/// A unix connect finishes when the kernel queues it, which is before
/// `accept`, or it fails at once when the socket is absent. `timeout` rejects
/// a caller that already has no time left. The write and the read apply the
/// same budget after this returns.
#[cfg(unix)]
fn connect_control(
    path: &Path,
    timeout: std::time::Duration,
) -> Result<std::os::unix::net::UnixStream> {
    use std::os::unix::net::UnixStream;

    if timeout.is_zero() {
        anyhow::bail!("grove control socket timed out");
    }
    UnixStream::connect(path).map_err(socket_err)
}

#[cfg(unix)]
fn socket_err(err: std::io::Error) -> anyhow::Error {
    anyhow::anyhow!("grove control socket is unreachable: {err}")
}

#[cfg(not(unix))]
fn control_call(
    path: Option<&Path>,
    request: &serde_json::Value,
    timeout: std::time::Duration,
) -> Result<serde_json::Value> {
    let _ = (path, request, timeout);
    anyhow::bail!("not available in this build")
}

fn redirect_reply(reply: serde_json::Value) -> Result<(Vec<serde_json::Value>, u64)> {
    if reply.get("status").and_then(serde_json::Value::as_str) != Some("ok") {
        let detail = reply
            .pointer("/data/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("grove redirect_events was declined");
        anyhow::bail!("{detail}");
    }
    let data = reply.get("data");
    let events = match data.and_then(|value| value.get("redirect_events")) {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Array(events)) => events.clone(),
        Some(_) => anyhow::bail!("grove redirect_events reply was not an array"),
    };
    let Some(next_seq) = data
        .and_then(|value| value.get("redirect_next_seq"))
        .and_then(serde_json::Value::as_u64)
    else {
        anyhow::bail!("grove redirect_events reply omitted redirect_next_seq");
    };
    Ok((events, next_seq))
}

#[must_use]
pub fn source_is_linked_local_view(_opts: &NfsWorktreeOpts, _source: &Path) -> bool {
    false
}

pub fn source_keeps_grove_create(_opts: &NfsWorktreeOpts, _source: &Path) -> bool {
    false
}

pub fn try_nfs_remove(_worktree_path: &Path) -> Result<Option<crate::RemoveReport>> {
    Ok(None)
}

#[must_use]
pub(crate) fn probe_daemon_capability_class(
    _opts: Option<&NfsWorktreeOpts>,
) -> Option<&'static str> {
    None
}

pub fn dest_is_known_unmounted(_path: &Path) -> bool {
    true
}

pub fn dest_is_mountpoint(_path: &Path) -> bool {
    false
}

pub fn dest_is_projected_mount(_path: &Path) -> bool {
    false
}

pub fn dest_is_grove_projection(_path: &Path) -> bool {
    false
}

#[must_use]
pub fn source_is_grove_parent(_path: &Path) -> bool {
    false
}

#[cfg(feature = "metadata")]
mod metadata {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use crate::db::WorktreeRecord;

    pub const RANK_DB: u8 = 0;
    /// `mounts.toml` rows. A backing marker outranks these.
    const RANK_MOUNTS: u8 = 1;
    /// On-disk `grok-nfs-worktree.json`. Highest disk source.
    const RANK_MARKER: u8 = 3;
    const BACKING_MARKER_FILE: &str = "grok-nfs-worktree.json";
    const MOUNTS_FILE: &str = "mounts.toml";
    const PIN_GC_LEDGER: &str = "pin_gc_orphans.json";
    const DAEMON_DB: &str = "daemon.db";
    const PIN_PREFIX: &str = "refs/grok/worktrees/";
    /// A ledger row is pruned only after a prior sighting is at least this old.
    /// `first_seen` of 1 is already past it; a row written this pass is not.
    const PIN_ORPHAN_GRACE_SECS: i64 = 60 * 60;

    #[derive(Debug, Clone)]
    pub struct NfsIdentity {
        pub worktree_id: String,
        pub dest: Option<PathBuf>,
        pub source_repo: Option<PathBuf>,
        pub pin_ref: Option<String>,
        pub backing: Option<PathBuf>,
        pub mount_id: Option<i64>,
        pub rank: u8,
        pub phase: Option<String>,
    }

    #[derive(Debug, Default)]
    pub struct PinGcReport {
        pub examined: u64,
        pub pruned: u64,
        pub deferred_grace: u64,
        pub kept_live: u64,
        pub pruned_ids: Vec<String>,
    }

    pub fn candidate_data_dirs() -> Vec<PathBuf> {
        crate::data_dirs::candidate_data_dirs()
    }

    pub fn nfs_record_is_dead(dest: &Path, _backing: Option<&Path>) -> bool {
        if crate::nfs::dest_is_mountpoint(dest) || !crate::nfs::dest_is_known_unmounted(dest) {
            return false;
        }
        std::fs::symlink_metadata(dest).is_err()
    }

    pub fn identities_from_worktree_records(recs: &[WorktreeRecord]) -> Vec<NfsIdentity> {
        recs.iter()
            .filter(|rec| rec.status != crate::db::WorktreeStatus::Dead)
            .map(|rec| {
                let pin_ref = meta_str(rec, "source_pin")
                    .unwrap_or_else(|| format!("{PIN_PREFIX}{}", rec.id));
                NfsIdentity {
                    worktree_id: rec.id.clone(),
                    dest: Some(rec.path.clone()),
                    source_repo: Some(rec.source_repo.clone()),
                    pin_ref: Some(pin_ref),
                    backing: meta_str(rec, "backing").map(PathBuf::from),
                    mount_id: meta_i64(rec, "mount_id"),
                    rank: RANK_DB,
                    phase: None,
                }
            })
            .collect()
    }

    pub fn collect_identities(
        data_dir: &Path,
        _worktrees: &[NfsIdentity],
    ) -> HashMap<String, NfsIdentity> {
        let mut out = HashMap::new();
        merge_nfs_identities(&mut out, marker_identities(data_dir));
        merge_nfs_identities(&mut out, mount_identities(data_dir));
        out
    }

    pub fn merge_nfs_identities(
        into: &mut HashMap<String, NfsIdentity>,
        src: impl IntoIterator<Item = NfsIdentity>,
    ) {
        for idn in src {
            if idn.worktree_id.is_empty() {
                continue;
            }
            match into.get(&idn.worktree_id) {
                Some(prev) if prev.rank >= idn.rank => {}
                _ => {
                    into.insert(idn.worktree_id.clone(), idn);
                }
            }
        }
    }

    pub fn gc_orphan_pins(
        data_dir: &Path,
        worktrees: &[NfsIdentity],
        now: i64,
        dry_run: bool,
    ) -> Result<PinGcReport> {
        let mut report = PinGcReport::default();
        let mut ledger = load_pin_ledger(data_dir);
        let aborted = aborted_creates(data_dir);
        let mut ids: Vec<String> = ledger.keys().cloned().collect();
        for id in aborted.keys() {
            if !ids.iter().any(|seen| seen == id) {
                ids.push(id.clone());
            }
        }
        ids.sort();
        let mut changed = false;
        for id in ids {
            if !super::is_safe_worktree_id(&id) {
                continue;
            }
            let prior = ledger.get(&id).cloned();
            let journal = aborted.get(&id);
            if prior.is_none() && journal.is_none() {
                continue;
            }
            report.examined = report.examined.saturating_add(1);
            let pin_ref = prior
                .as_ref()
                .and_then(|row| row.pin_ref.clone())
                .filter(|pin| pin.starts_with(PIN_PREFIX))
                .unwrap_or_else(|| format!("{PIN_PREFIX}{id}"));
            if pin_is_live(&id, &pin_ref, worktrees) {
                report.kept_live = report.kept_live.saturating_add(1);
                if ledger.remove(&id).is_some() {
                    changed = true;
                }
                continue;
            }
            let first_seen = prior.as_ref().map(|row| row.first_seen).unwrap_or(now);
            let cycles = prior.as_ref().map(|row| row.cycles).unwrap_or(0);
            let aged = prior.is_some()
                && cycles >= 1
                && first_seen > 0
                && now.saturating_sub(first_seen) >= PIN_ORPHAN_GRACE_SECS;
            // Journal proves the create aborted. A ledger row alone must not
            // delete a pin that a later successful create still owns.
            if aged && journal.is_some() {
                let source = prior
                    .as_ref()
                    .and_then(|row| row.source.clone())
                    .or_else(|| journal.and_then(|row| row.source.clone()));
                if dry_run {
                    push_pruned(&mut report, id);
                    continue;
                }
                if source
                    .as_deref()
                    .is_some_and(|repo| delete_pin(repo, &pin_ref))
                {
                    push_pruned(&mut report, id.clone());
                    ledger.remove(&id);
                    changed = true;
                    continue;
                }
            }
            report.deferred_grace = report.deferred_grace.saturating_add(1);
            if dry_run {
                continue;
            }
            let source = prior
                .as_ref()
                .and_then(|row| row.source.clone())
                .or_else(|| journal.and_then(|row| row.source.clone()));
            ledger.insert(
                id.clone(),
                PinGcOrphan {
                    first_seen: if prior.is_some() { first_seen } else { now },
                    cycles: cycles.saturating_add(1).max(1),
                    source,
                    pin_ref: Some(pin_ref),
                },
            );
            changed = true;
        }
        if changed && !dry_run {
            let _ = store_pin_ledger(data_dir, &ledger);
        }
        Ok(report)
    }

    fn push_pruned(report: &mut PinGcReport, id: String) {
        report.pruned = report.pruned.saturating_add(1);
        report.pruned_ids.push(id);
    }

    fn pin_is_live(id: &str, pin_ref: &str, worktrees: &[NfsIdentity]) -> bool {
        worktrees
            .iter()
            .any(|rec| rec.worktree_id == id || rec.pin_ref.as_deref() == Some(pin_ref))
    }

    fn marker_identities(data_dir: &Path) -> Vec<NfsIdentity> {
        let root = data_dir.join(super::WORKTREE_BACKING_DIR);
        let Ok(entries) = std::fs::read_dir(&root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let slot = entry.path();
            if !std::fs::symlink_metadata(&slot).is_ok_and(|meta| meta.is_dir()) {
                continue;
            }
            let marker_path = slot.join(BACKING_MARKER_FILE);
            let Ok(bytes) = std::fs::read(&marker_path) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            if let Some(idn) = identity_from_marker(&slot, &value) {
                out.push(idn);
            }
        }
        out
    }

    fn identity_from_marker(slot: &Path, value: &serde_json::Value) -> Option<NfsIdentity> {
        let obj = value.as_object()?;
        if obj
            .get("schema")
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|schema| schema != 1)
        {
            return None;
        }
        let worktree_id = obj.get("worktree_id").and_then(serde_json::Value::as_str)?;
        if !super::is_safe_worktree_id(worktree_id) {
            return None;
        }
        let dirent = slot.file_name()?.to_str()?;
        if dirent != worktree_id {
            return None;
        }
        Some(NfsIdentity {
            worktree_id: worktree_id.to_owned(),
            dest: json_path(obj.get("dest")?)
                .filter(|path| !path.as_os_str().is_empty() && path.as_os_str() != "unknown"),
            source_repo: obj.get("source_repo").and_then(json_path),
            pin_ref: obj
                .get("pin_ref")
                .and_then(serde_json::Value::as_str)
                .filter(|pin| !pin.is_empty())
                .map(str::to_owned),
            backing: Some(slot.to_path_buf()),
            mount_id: obj.get("mount_id").and_then(serde_json::Value::as_i64),
            rank: RANK_MARKER,
            phase: None,
        })
    }

    fn json_path(value: &serde_json::Value) -> Option<PathBuf> {
        value
            .as_str()
            .filter(|text| !text.is_empty())
            .map(PathBuf::from)
    }

    fn mount_identities(data_dir: &Path) -> Vec<NfsIdentity> {
        let Ok(text) = std::fs::read_to_string(data_dir.join(MOUNTS_FILE)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for row in parse_mounts_toml(&text) {
            if row.kind.as_deref().is_some_and(|kind| kind != "worktree") {
                continue;
            }
            let Some(worktree_id) = mount_worktree_id(&row) else {
                continue;
            };
            if !super::is_safe_worktree_id(&worktree_id) {
                continue;
            }
            out.push(NfsIdentity {
                worktree_id,
                dest: row
                    .dest
                    .filter(|path| !path.as_os_str().is_empty() && path.as_os_str() != "unknown"),
                source_repo: row.source_repo,
                pin_ref: row.pin_ref,
                backing: row.backing,
                mount_id: row.mount_id,
                rank: RANK_MOUNTS,
                phase: row.phase,
            });
        }
        out
    }

    #[derive(Default)]
    struct MountRow {
        kind: Option<String>,
        pin_ref: Option<String>,
        backing: Option<PathBuf>,
        dest: Option<PathBuf>,
        source_repo: Option<PathBuf>,
        worktree_id: Option<String>,
        mount_id: Option<i64>,
        phase: Option<String>,
    }

    fn parse_mounts_toml(text: &str) -> Vec<MountRow> {
        let mut rows = Vec::new();
        let mut cur: Option<MountRow> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "[[mounts]]" {
                if let Some(row) = cur.take() {
                    rows.push(row);
                }
                cur = Some(MountRow::default());
                continue;
            }
            let Some(row) = cur.as_mut() else {
                continue;
            };
            let Some((key, raw)) = line.split_once('=') else {
                continue;
            };
            let val = unquote(raw.trim());
            match key.trim() {
                "kind" => row.kind = Some(val),
                "pin_ref" => row.pin_ref = Some(val),
                "backing" => row.backing = Some(PathBuf::from(val)),
                "dest" => row.dest = Some(PathBuf::from(val)),
                "source" | "source_repo" => row.source_repo = Some(PathBuf::from(val)),
                "worktree_id" => row.worktree_id = Some(val),
                "mount_id" => row.mount_id = val.parse().ok(),
                "phase" => row.phase = Some(val),
                _ => {}
            }
        }
        if let Some(row) = cur {
            rows.push(row);
        }
        rows
    }

    fn unquote(raw: &str) -> String {
        if let Some(inner) = raw
            .strip_prefix('"')
            .and_then(|text| text.strip_suffix('"'))
        {
            return inner.to_owned();
        }
        if let Some(inner) = raw
            .strip_prefix('\'')
            .and_then(|text| text.strip_suffix('\''))
        {
            return inner.to_owned();
        }
        raw.to_owned()
    }

    fn mount_worktree_id(row: &MountRow) -> Option<String> {
        if let Some(id) = row.worktree_id.clone() {
            return Some(id);
        }
        if let Some(pin) = row.pin_ref.as_deref() {
            if let Some(id) = pin.strip_prefix(PIN_PREFIX) {
                if super::is_safe_worktree_id(id) {
                    return Some(id.to_owned());
                }
            }
        }
        row.backing
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
    }

    fn meta_str(rec: &WorktreeRecord, key: &str) -> Option<String> {
        let meta = rec.metadata.as_ref()?;
        for group in ["grove", "nfs"] {
            if let Some(text) = meta
                .get(group)
                .and_then(|value| value.get(key))
                .and_then(serde_json::Value::as_str)
                .filter(|text| !text.is_empty())
            {
                return Some(text.to_owned());
            }
        }
        None
    }

    fn meta_i64(rec: &WorktreeRecord, key: &str) -> Option<i64> {
        let meta = rec.metadata.as_ref()?;
        for group in ["grove", "nfs"] {
            if let Some(number) = meta
                .get(group)
                .and_then(|value| value.get(key))
                .and_then(serde_json::Value::as_i64)
            {
                return Some(number);
            }
        }
        None
    }

    #[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
    struct PinGcOrphan {
        first_seen: i64,
        #[serde(default)]
        cycles: u64,
        #[serde(default)]
        source: Option<PathBuf>,
        #[serde(default)]
        pin_ref: Option<String>,
    }

    #[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
    struct PinGcLedger {
        #[serde(default)]
        orphans: HashMap<String, PinGcOrphan>,
    }

    fn load_pin_ledger(data_dir: &Path) -> HashMap<String, PinGcOrphan> {
        let Ok(bytes) = std::fs::read(data_dir.join(PIN_GC_LEDGER)) else {
            return HashMap::new();
        };
        serde_json::from_slice::<PinGcLedger>(&bytes)
            .map(|ledger| ledger.orphans)
            .unwrap_or_default()
    }

    fn store_pin_ledger(data_dir: &Path, orphans: &HashMap<String, PinGcOrphan>) -> Result<()> {
        let ledger = PinGcLedger {
            orphans: orphans.clone(),
        };
        let bytes = serde_json::to_vec(&ledger)?;
        std::fs::write(data_dir.join(PIN_GC_LEDGER), bytes)?;
        Ok(())
    }

    struct AbortedCreate {
        source: Option<PathBuf>,
    }

    fn aborted_creates(data_dir: &Path) -> HashMap<String, AbortedCreate> {
        let path = data_dir.join(DAEMON_DB);
        if !std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()) {
            return HashMap::new();
        }
        let Ok(conn) = rusqlite::Connection::open(path) else {
            return HashMap::new();
        };
        if conn
            .busy_timeout(std::time::Duration::from_secs(2))
            .is_err()
        {
            return HashMap::new();
        };
        let Ok(mut stmt) =
            conn.prepare("SELECT worktree_id, source FROM wt_create_state WHERE phase = 'aborted'")
        else {
            return HashMap::new();
        };
        let Ok(rows) = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let source: Option<String> = row.get(1)?;
            Ok((
                id,
                AbortedCreate {
                    source: source.map(PathBuf::from),
                },
            ))
        }) else {
            return HashMap::new();
        };
        rows.filter_map(|row| row.ok())
            .filter(|(id, _)| super::is_safe_worktree_id(id))
            .collect()
    }

    fn delete_pin(repo: &Path, pin: &str) -> bool {
        if !pin.starts_with(PIN_PREFIX) || pin.contains("..") {
            return false;
        }
        let mut cmd = std::process::Command::new("git");
        xai_tty_utils::detach_std_command(&mut cmd);
        cmd.current_dir(repo)
            .args(["update-ref", "-d", pin])
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[cfg(feature = "metadata")]
pub use metadata::*;
