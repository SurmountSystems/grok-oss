//! Force-remote nix argv for GROK_NIX_FORCE_REMOTE=1.
//!
//! Caller max-jobs 0, --cores 64, --store ssh-ng, --eval-store auto.
//! Banner redacts the --store URI as `<builder>`. Never logs NIX_SSHOPTS.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

/// Parse the SSH host from an ssh-ng URI (query and user stripped).
pub fn ssh_ng_host(uri: &str) -> String {
    let mut u = uri.strip_prefix("ssh-ng://").unwrap_or(uri);
    if let Some(q) = u.find('?') {
        u = &u[..q];
    }
    if let Some(at) = u.find('@') {
        u = &u[at + 1..];
    }
    if let Some(slash) = u.find('/') {
        u = &u[..slash];
    }
    if let Some(rest) = u.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest).to_string()
    } else {
        u.split(':').next().unwrap_or(u).to_string()
    }
}

/// Append ssh-ng max-connections query if missing.
pub fn with_max_connections(uri: &str, max_conn: &str) -> String {
    if uri.contains("max-connections=") {
        uri.to_string()
    } else if uri.contains('?') {
        format!("{uri}&max-connections={max_conn}")
    } else {
        format!("{uri}?max-connections={max_conn}")
    }
}

/// RFC 4648 standard base64 (same as `base64 -w0`).
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };
        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if i + 1 < data.len() {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < data.len() {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

fn host_key_b64(host: &str, known_hosts: &Path) -> Option<String> {
    if !known_hosts.is_file() {
        return None;
    }
    let out = Command::new("ssh-keygen")
        .args(["-F", host, "-f"])
        .arg(known_hosts)
        .output()
        .ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ed25519: Option<(String, String)> = None;
    let mut any: Option<(String, String)> = None;
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut cols = line.split_whitespace();
        let Some(_name) = cols.next() else {
            continue;
        };
        let Some(typ) = cols.next() else {
            continue;
        };
        let Some(key) = cols.next() else {
            continue;
        };
        if typ == "ssh-ed25519" && ed25519.is_none() {
            ed25519 = Some((typ.to_string(), key.to_string()));
        } else if typ.starts_with("ssh-") && any.is_none() {
            any = Some((typ.to_string(), key.to_string()));
        }
    }
    let (typ, key) = ed25519.or(any)?;
    Some(base64_encode(format!("{typ} {key}").as_bytes()))
}

/// One machines-file line, fields 1-8 (URI through host-key).
#[derive(Debug, Clone)]
struct BuildersLine {
    uri: String,
    systems: String,
    ssh_key: String,
    max_jobs: String,
    speed: String,
    supported: String,
    mandatory: String,
    host_key: String,
}

fn parse_builders_line(line: &str) -> Option<BuildersLine> {
    let mut it = line.split_whitespace();
    let uri = it.next()?.to_string();
    Some(BuildersLine {
        uri,
        systems: it.next().unwrap_or("-").to_string(),
        ssh_key: it.next().unwrap_or("-").to_string(),
        max_jobs: it.next().unwrap_or("-").to_string(),
        speed: it.next().unwrap_or("-").to_string(),
        supported: it.next().unwrap_or("-").to_string(),
        mandatory: it.next().unwrap_or("-").to_string(),
        host_key: it.next().unwrap_or("").to_string(),
    })
}

fn format_builders_line(l: &BuildersLine) -> String {
    format!(
        "{} {} {} {} {} {} {} {}",
        l.uri, l.systems, l.ssh_key, l.max_jobs, l.speed, l.supported, l.mandatory, l.host_key
    )
}

pub struct ForceRemote {
    pub extra: Vec<String>,
    pub nix_sshopts: String,
    _temp: Option<PathBuf>,
}

impl Drop for ForceRemote {
    fn drop(&mut self) {
        if let Some(p) = &self._temp {
            let _ = fs::remove_file(p);
        }
    }
}

#[derive(Debug)]
pub enum ForceRemoteError {
    MaxConnNotPositive(String),
    NoHostKey,
    NoStoreUri,
}

impl ForceRemoteError {
    pub fn message(&self) -> String {
        match self {
            ForceRemoteError::MaxConnNotPositive(raw) => format!(
                "==> nix_retry: GROK_NIX_SSH_NG_MAX_CONNECTIONS must be a positive integer, got: {raw}"
            ),
            ForceRemoteError::NoHostKey => {
                "==> nix_retry: this account's known_hosts has no host key for the machines-file builder. User ssh to Host surmount-1 is not the nix build SSH path.".into()
            }
            ForceRemoteError::NoStoreUri => {
                "==> nix_retry: GROK_NIX_FORCE_REMOTE needs an ssh-ng:// builder URI in the machines file so nix can use --store on that builder. This laptop must not realize the graph into the local store.".into()
            }
        }
    }

    pub fn exit_code(&self) -> u8 {
        2
    }
}

/// Build force-remote nix flags. `second_word` is argv[1] (`build` gets --no-link).
pub fn force_remote_nix_args(
    store_uri: &str,
    builders_at: &str,
    second_word: Option<&str>,
) -> Vec<String> {
    let mut opts = vec![
        "--option".into(),
        "builders".into(),
        format!("@{builders_at}"),
        "--option".into(),
        "builders-use-substitutes".into(),
        "true".into(),
        "--option".into(),
        "fallback".into(),
        "false".into(),
        "--option".into(),
        "system-features".into(),
        "kvm nixos-test uid-range".into(),
        "--option".into(),
        "max-jobs".into(),
        "0".into(),
        "--cores".into(),
        "64".into(),
        "--store".into(),
        store_uri.to_string(),
        "--eval-store".into(),
        "auto".into(),
    ];
    if second_word == Some("build") {
        opts.push("--no-link".into());
    }
    opts
}

/// Redact `--store` value as `<builder>` for the attempt banner.
pub fn redact_store_banner(opts: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(opts.len());
    let mut skip_store = false;
    for opt in opts {
        if skip_store {
            out.push("<builder>".into());
            skip_store = false;
            continue;
        }
        if opt == "--store" {
            out.push(opt.clone());
            skip_store = true;
            continue;
        }
        out.push(opt.clone());
    }
    out
}

fn extra_sshopts(existing: Option<&str>, known_hosts: &Path) -> String {
    let extra = format!(
        "-o UserKnownHostsFile={} -o StrictHostKeyChecking=yes",
        known_hosts.display()
    );
    match existing.filter(|s| !s.is_empty()) {
        Some(cur) => format!("{cur} {extra}"),
        None => extra,
    }
}

fn write_temp_builders(body: &str) -> io::Result<PathBuf> {
    let path = env_temp("grok-nix-helper-builders");
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }
    let mut f = opts.open(&path)?;
    f.write_all(body.as_bytes())?;
    Ok(path)
}

fn env_temp(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("{prefix}-{}", std::process::id()));
    p
}

/// Prepare force-remote extra argv and NIX_SSHOPTS. Caller keeps the value
/// until the retry loop finishes so the temp builders file stays on disk.
pub fn prepare(
    builders_file: &Path,
    known_hosts: &Path,
    max_conn_raw: &str,
    existing_sshopts: Option<&str>,
    second_word: Option<&str>,
) -> Result<ForceRemote, ForceRemoteError> {
    if !max_conn_raw.chars().all(|c| c.is_ascii_digit())
        || max_conn_raw.is_empty()
        || max_conn_raw.starts_with('0')
    {
        return Err(ForceRemoteError::MaxConnNotPositive(max_conn_raw.into()));
    }

    let src = fs::read_to_string(builders_file).unwrap_or_default();
    let mut out_lines = String::new();
    let mut store_uri = String::new();
    for line in src.lines() {
        if !line.starts_with("ssh-ng://") {
            out_lines.push_str(line);
            out_lines.push('\n');
            continue;
        }
        let Some(mut parsed) = parse_builders_line(line) else {
            out_lines.push_str(line);
            out_lines.push('\n');
            continue;
        };
        parsed.uri = with_max_connections(&parsed.uri, max_conn_raw);
        if parsed.host_key.is_empty() || parsed.host_key == "-" {
            let host = ssh_ng_host(&parsed.uri);
            let Some(b64) = host_key_b64(&host, known_hosts) else {
                return Err(ForceRemoteError::NoHostKey);
            };
            parsed.host_key = b64;
        }
        if store_uri.is_empty() {
            store_uri = parsed.uri.clone();
        }
        out_lines.push_str(&format_builders_line(&parsed));
        out_lines.push('\n');
    }

    if store_uri.is_empty() || !store_uri.starts_with("ssh-ng://") {
        return Err(ForceRemoteError::NoStoreUri);
    }

    let temp = write_temp_builders(&out_lines).map_err(|_| ForceRemoteError::NoStoreUri)?;
    let extra = force_remote_nix_args(&store_uri, &temp.to_string_lossy(), second_word);
    let nix_sshopts = extra_sshopts(existing_sshopts, known_hosts);
    Ok(ForceRemote {
        extra,
        nix_sshopts,
        _temp: Some(temp),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_from_user_at_hostname() {
        assert_eq!(
            ssh_ng_host("ssh-ng://probe@example.invalid"),
            "example.invalid"
        );
    }

    #[test]
    fn host_strips_query() {
        assert_eq!(
            ssh_ng_host("ssh-ng://probe@example.invalid?max-connections=8"),
            "example.invalid"
        );
    }

    #[test]
    fn host_ipv6_brackets() {
        assert_eq!(ssh_ng_host("ssh-ng://user@[::1]:2222"), "::1");
    }

    #[test]
    fn max_connections_appended() {
        assert_eq!(
            with_max_connections("ssh-ng://probe@example.invalid", "8"),
            "ssh-ng://probe@example.invalid?max-connections=8"
        );
        let already = "ssh-ng://probe@example.invalid?max-connections=8";
        assert_eq!(with_max_connections(already, "4"), already);
    }

    #[test]
    fn force_remote_flags_include_operator_sentences() {
        let opts = force_remote_nix_args(
            "ssh-ng://probe@example.invalid?max-connections=8",
            "/tmp/builders",
            Some("build"),
        );
        assert!(
            opts.windows(3)
                .any(|w| w[0] == "--option" && w[1] == "max-jobs" && w[2] == "0")
        );
        assert!(opts.windows(2).any(|w| w[0] == "--cores" && w[1] == "64"));
        assert!(
            opts.windows(2)
                .any(|w| w[0] == "--eval-store" && w[1] == "auto")
        );
        assert!(
            opts.windows(2)
                .any(|w| w[0] == "--store" && w[1].starts_with("ssh-ng://"))
        );
        assert!(opts.iter().any(|a| a == "--no-link"));
        assert!(opts.windows(3).any(|w| w[0] == "--option"
            && w[1] == "system-features"
            && w[2] == "kvm nixos-test uid-range"
            && !w[2].contains("big-parallel")
            && !w[2].contains("benchmark")));
    }

    #[test]
    fn flake_metadata_omits_no_link() {
        let opts = force_remote_nix_args(
            "ssh-ng://probe@example.invalid",
            "/tmp/builders",
            Some("flake"),
        );
        assert!(!opts.iter().any(|a| a == "--no-link"));
    }

    #[test]
    fn banner_redacts_store_uri() {
        let opts = force_remote_nix_args("ssh-ng://probe@secret.invalid", "/tmp/b", None);
        let banner = redact_store_banner(&opts);
        assert!(
            banner
                .windows(2)
                .any(|w| w[0] == "--store" && w[1] == "<builder>")
        );
        assert!(!banner.iter().any(|a| a.contains("secret.invalid")));
    }

    #[test]
    fn base64_matches_round_trip_alphabet() {
        let s = base64_encode(b"ssh-ed25519 AAAAC3");
        assert!(
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        );
        assert!(!s.contains('\n'));
    }
}
