//! Laptop-side action: set a machine console API key for a remote host.
//!
//! The operator creates the key at console.x.ai. This module writes a
//! staging file on the laptop (and can copy that file over SSH as an
//! existing deploy user). It never prints the key. It does not open git
//! on the guest. It does not generate a GitHub SSH key.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Operator-facing name of this L0 action.
pub const SET_REMOTE_HOST_CONSOLE_API_KEY_ACTION: &str = "set remote host console API key";

const STAGING_DIR: &str = "l0-remote-console-key";
const ENV_FILE: &str = "console.env";
const CONFIG_SNIPPET_FILE: &str = "machine-console-auth.toml";
const NOTES_FILE: &str = "INSTALL.txt";

/// Default grok home on host surmount-1 for user grok when `GROK_HOME` is unset.
pub const DEFAULT_GUEST_GROK_HOME: &str = "/home/grok/.grok";

/// SSH copy target: existing deploy user at a host, plus that host's grok home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshInstallSpec {
    pub user_at_host: String,
    pub remote_grok_home: PathBuf,
}

/// Plan the scp commands, or also run an installer that copies files.
pub enum SshRequest<'a> {
    PlanOnly(SshInstallSpec),
    Install {
        spec: SshInstallSpec,
        installer: &'a dyn HostFileInstall,
    },
}

/// Copy owner-only files onto the guest. Implementations must not print
/// file contents (the env file holds the key).
pub trait HostFileInstall {
    fn install_owner_only_file(&self, local: &Path, dest: &str) -> Result<(), String>;
}

/// Why the machine-key action could not finish.
#[derive(Debug)]
pub enum RemoteHostConsoleKeyError {
    EmptyKey,
    EmptyHost,
    UnsafeHost,
    KeyOnCommandLine,
    Io(io::Error),
    RemoteCopy(String),
}

impl fmt::Display for RemoteHostConsoleKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => write!(f, "the machine console API key is empty"),
            Self::EmptyHost => write!(f, "the remote host name is empty"),
            Self::UnsafeHost => write!(
                f,
                "the remote host name must be a single path component (for example surmount-1)"
            ),
            Self::KeyOnCommandLine => write!(
                f,
                "do not pass the machine console API key on the command line; paste it on stdin"
            ),
            Self::Io(err) => write!(f, "could not write the staging files: {err}"),
            Self::RemoteCopy(err) => write!(f, "could not copy the staging files over SSH: {err}"),
        }
    }
}

impl std::error::Error for RemoteHostConsoleKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for RemoteHostConsoleKeyError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Result of the laptop-side action. Fields and Display never include the key.
#[derive(Clone, PartialEq, Eq)]
pub struct RemoteHostConsoleKeyReport {
    pub host: String,
    pub staging_dir: PathBuf,
    pub env_file: PathBuf,
    pub config_snippet_file: PathBuf,
    pub notes_file: PathBuf,
    pub operator_copy: String,
    pub ssh_copy_commands: Vec<String>,
    pub opens_guest_git: bool,
    pub generates_guest_github_ssh: bool,
    pub copies_laptop_supergrok_oauth: bool,
}

impl fmt::Debug for RemoteHostConsoleKeyReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteHostConsoleKeyReport")
            .field("host", &self.host)
            .field("staging_dir", &self.staging_dir)
            .field("env_file", &self.env_file)
            .field("config_snippet_file", &self.config_snippet_file)
            .field("notes_file", &self.notes_file)
            .field("opens_guest_git", &self.opens_guest_git)
            .field(
                "generates_guest_github_ssh",
                &self.generates_guest_github_ssh,
            )
            .field(
                "copies_laptop_supergrok_oauth",
                &self.copies_laptop_supergrok_oauth,
            )
            .field("ssh_copy_commands", &self.ssh_copy_commands)
            .finish()
    }
}

impl fmt::Display for RemoteHostConsoleKeyReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.operator_copy)
    }
}

/// Write laptop staging files for a machine console API key.
///
/// `key` is written only to the owner-only env file. It is not returned,
/// logged, or interpolated into `operator_copy`.
pub fn set_remote_host_console_api_key(
    laptop_grok_home: &Path,
    host: &str,
    key: &str,
    ssh: Option<SshRequest<'_>>,
) -> Result<RemoteHostConsoleKeyReport, RemoteHostConsoleKeyError> {
    if host.trim().is_empty() {
        return Err(RemoteHostConsoleKeyError::EmptyHost);
    }
    let host = sanitize_host(host).ok_or(RemoteHostConsoleKeyError::UnsafeHost)?;
    let key = key.trim();
    if key.is_empty() {
        return Err(RemoteHostConsoleKeyError::EmptyKey);
    }

    let staging_dir = laptop_grok_home.join(STAGING_DIR).join(host);
    fs::create_dir_all(&staging_dir)?;
    let env_file = staging_dir.join(ENV_FILE);
    let config_snippet_file = staging_dir.join(CONFIG_SNIPPET_FILE);
    let notes_file = staging_dir.join(NOTES_FILE);

    write_owner_only_env_file(&env_file, key)?;
    fs::write(&config_snippet_file, machine_console_auth_toml())?;

    let spec = ssh.as_ref().map(|req| match req {
        SshRequest::PlanOnly(spec) => spec,
        SshRequest::Install { spec, .. } => spec,
    });
    let ssh_copy_commands = spec
        .map(|spec| ssh_copy_commands(spec, &env_file, &config_snippet_file))
        .unwrap_or_default();

    if let Some(SshRequest::Install { spec, installer }) = ssh.as_ref() {
        let env_dest = remote_dest(spec, ENV_FILE)?;
        let cfg_dest = remote_dest(spec, CONFIG_SNIPPET_FILE)?;
        installer
            .install_owner_only_file(&env_file, &env_dest)
            .map_err(RemoteHostConsoleKeyError::RemoteCopy)?;
        installer
            .install_owner_only_file(&config_snippet_file, &cfg_dest)
            .map_err(RemoteHostConsoleKeyError::RemoteCopy)?;
        assert_commands_do_not_open_guest_git(&ssh_copy_commands);
        assert_commands_do_not_generate_github_ssh(&ssh_copy_commands);
    }

    let operator_copy = operator_copy(
        host,
        &staging_dir,
        &env_file,
        &config_snippet_file,
        spec,
        &ssh_copy_commands,
    );
    fs::write(&notes_file, &operator_copy)?;

    debug_assert!(
        !operator_copy.contains(key),
        "operator copy must never include the key"
    );

    Ok(RemoteHostConsoleKeyReport {
        host: host.to_string(),
        staging_dir,
        env_file,
        config_snippet_file,
        notes_file,
        operator_copy,
        ssh_copy_commands,
        opens_guest_git: false,
        generates_guest_github_ssh: false,
        copies_laptop_supergrok_oauth: false,
    })
}

/// `scp` argv that copies `local` to the guest path. The key is not an argument.
pub fn scp_copy_argv(local: &Path, dest: &str) -> Vec<String> {
    vec![
        "scp".to_string(),
        "-p".to_string(),
        local.display().to_string(),
        dest.to_string(),
    ]
}

/// `ssh` argv that sets owner-only mode on a remote path. Does not cat the file.
pub fn ssh_chmod_argv(user_at_host: &str, remote_path: &str) -> Vec<String> {
    vec![
        "ssh".to_string(),
        user_at_host.to_string(),
        format!("chmod 0600 {remote_path}"),
    ]
}

fn write_owner_only_env_file(path: &Path, key: &str) -> io::Result<()> {
    let mut body = String::from(
        "# Machine xAI console API key for this host only. Owner-only. Never commit.\n\
         # This spends console API credits / console team prepaid.\n\
         # It is not included SuperGrok period limits. It is not SuperGrok dollar credits.\n\
         XAI_API_KEY=",
    );
    body.push_str(key);
    body.push('\n');
    fs::write(path, body.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn machine_console_auth_toml() -> &'static str {
    "# Machine identity for this host. Pin sampling on the console API key.\n\
     # This spends console API credits / console team prepaid.\n\
     # It is not included SuperGrok period limits. It is not SuperGrok dollar credits.\n\
     # Do not copy a laptop SuperGrok OAuth login onto this host.\n\
     [auth]\n\
     preferred_method = \"api_key\"\n"
}

fn sanitize_host(host: &str) -> Option<&str> {
    let host = host.trim();
    if host.is_empty() || host.eq_ignore_ascii_case("local") {
        return None;
    }
    if host == "." || host == ".." {
        return None;
    }
    if host.contains('/') || host.contains('\\') || host.contains('\0') || host.contains(':') {
        return None;
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return None;
    }
    Some(host)
}

fn remote_dest(spec: &SshInstallSpec, file: &str) -> Result<String, RemoteHostConsoleKeyError> {
    let user_at_host = spec.user_at_host.trim();
    if user_at_host.is_empty() || !user_at_host.contains('@') || user_at_host.contains(' ') {
        return Err(RemoteHostConsoleKeyError::RemoteCopy(
            "SSH target must look like deploy@surmount-1".to_string(),
        ));
    }
    let home = spec.remote_grok_home.to_string_lossy();
    if home.is_empty() || home.contains('\0') {
        return Err(RemoteHostConsoleKeyError::RemoteCopy(
            "remote grok home is empty".to_string(),
        ));
    }
    Ok(format!("{user_at_host}:{home}/{file}"))
}

fn ssh_copy_commands(spec: &SshInstallSpec, env_file: &Path, config_file: &Path) -> Vec<String> {
    let env_dest = match remote_dest(spec, ENV_FILE) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let cfg_dest = match remote_dest(spec, CONFIG_SNIPPET_FILE) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let remote_env = format!(
        "{}/{ENV_FILE}",
        spec.remote_grok_home
            .to_string_lossy()
            .trim_end_matches('/')
    );
    vec![
        scp_copy_argv(env_file, &env_dest).join(" "),
        scp_copy_argv(config_file, &cfg_dest).join(" "),
        ssh_chmod_argv(&spec.user_at_host, &remote_env).join(" "),
    ]
}

fn assert_commands_do_not_open_guest_git(commands: &[String]) {
    for cmd in commands {
        let lower = cmd.to_ascii_lowercase();
        debug_assert!(
            !lower.contains("git init")
                && !lower.contains("git remote")
                && !lower.contains("git push")
                && !lower.contains("git clone"),
            "SSH plan must not open git on the guest: {cmd}"
        );
    }
}

fn assert_commands_do_not_generate_github_ssh(commands: &[String]) {
    for cmd in commands {
        let lower = cmd.to_ascii_lowercase();
        debug_assert!(
            !lower.contains("ssh-keygen") && !lower.contains("github.com"),
            "SSH plan must not generate a GitHub SSH key: {cmd}"
        );
    }
}

fn operator_copy(
    host: &str,
    staging_dir: &Path,
    env_file: &Path,
    config_file: &Path,
    ssh: Option<&SshInstallSpec>,
    ssh_copy_commands: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(SET_REMOTE_HOST_CONSOLE_API_KEY_ACTION);
    out.push_str(" (laptop side).\n\n");
    out.push_str("The key was accepted and written to the staging env file. This message does not include the key.\n\n");
    out.push_str("Host: ");
    out.push_str(host);
    out.push('\n');
    out.push_str(
        "This key spends console API credits / console team prepaid. It is not included SuperGrok period limits. It is not SuperGrok dollar credits.\n",
    );
    out.push_str(
        "Do not copy a laptop SuperGrok OAuth login onto this host. The operator creates the console API key at https://console.x.ai and labels it for this host only. This action does not create the key.\n",
    );
    out.push_str("Never commit the key. Do not put the key in Nix or git.\n\n");
    out.push_str("Staging files on this laptop:\n");
    out.push_str(&format!("- {}\n", env_file.display()));
    out.push_str(&format!("- {}\n", config_file.display()));
    out.push_str(&format!("Directory: {}\n\n", staging_dir.display()));
    out.push_str(
        "On the remote host, grok home is $GROK_HOME when that variable is set, otherwise ~/.grok for user grok (typically /home/grok/.grok). Copy console.env there as console.env with mode 0600 and owner grok. Merge machine-console-auth.toml into that grok home config.toml. Start grok-oss in tmux as user grok after sourcing console.env.\n\n",
    );
    out.push_str("Attach stays SSH + tmux as user grok. There is no boot TUI.\n");
    out.push_str(
        "Do not configure git or GitHub SSH on the guest. Code is edited on the laptop. Commits are GPG-signed on the laptop. The guest must not git push.\n\n",
    );
    out.push_str(
        "This L0 action is the laptop coordinator. It is not a website on the mail host :443. It is not pager /dashboard. It is not /running. Those three products must not merge.\n",
    );
    if let Some(spec) = ssh {
        out.push('\n');
        out.push_str("SSH as the existing deploy user ");
        out.push_str(spec.user_at_host.trim());
        out.push_str(". Remote grok home: ");
        out.push_str(&spec.remote_grok_home.display().to_string());
        out.push_str(".\n");
        if !ssh_copy_commands.is_empty() {
            out.push_str("Copy commands (file paths only; the key is not an argument):\n");
            for cmd in ssh_copy_commands {
                out.push_str("- ");
                out.push_str(cmd);
                out.push('\n');
            }
        }
    } else {
        out.push('\n');
        out.push_str(
            "Copy the staging files yourself, or rerun with --ssh deploy@host to print scp as the existing deploy user.\n",
        );
    }
    out
}

/// Real `scp` / `ssh` installer. Does not print file contents.
pub struct SshDeployInstall;

impl HostFileInstall for SshDeployInstall {
    fn install_owner_only_file(&self, local: &Path, dest: &str) -> Result<(), String> {
        let status = std::process::Command::new("scp")
            .arg("-p")
            .arg(local)
            .arg(dest)
            .status()
            .map_err(|err| format!("scp failed to start: {err}"))?;
        if !status.success() {
            return Err(format!("scp exited {status}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoordinatorApp;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    const FAKE_KEY: &str = "xai-FAKE-MACHINE-CONSOLE-KEY-DO-NOT-USE-9f3c2a1b";

    fn test_home() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "surmount-l0-machine-key-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).expect("temp grok home");
        p
    }

    struct RecordingInstall {
        dests: Mutex<Vec<String>>,
    }

    impl RecordingInstall {
        fn new() -> Self {
            Self {
                dests: Mutex::new(Vec::new()),
            }
        }
    }

    impl HostFileInstall for RecordingInstall {
        fn install_owner_only_file(&self, local: &Path, dest: &str) -> Result<(), String> {
            assert!(local.is_file(), "installer copies a real local file");
            let mut dests = self.dests.lock().expect("dests");
            dests.push(dest.to_string());
            Ok(())
        }
    }

    fn assert_key_absent(haystack: &str, label: &str) {
        assert!(
            !haystack.contains(FAKE_KEY),
            "{label} must never include the machine console API key; got {haystack}"
        );
        assert!(
            !haystack.contains("FAKE-MACHINE-CONSOLE-KEY"),
            "{label} must never include the fake key token; got {haystack}"
        );
    }

    /// Named contract: L0 never prints the machine console API key.
    #[test]
    fn set_remote_host_console_api_key_never_prints_the_key() {
        let home = test_home();
        let report = set_remote_host_console_api_key(&home, "surmount-1", FAKE_KEY, None)
            .expect("stage key");
        assert_key_absent(&report.operator_copy, "operator_copy");
        assert_key_absent(&format!("{report}"), "Display");
        assert_key_absent(&format!("{report:?}"), "Debug");
        for cmd in &report.ssh_copy_commands {
            assert_key_absent(cmd, "ssh command");
        }
        let notes = fs::read_to_string(&report.notes_file).expect("notes");
        assert_key_absent(&notes, "INSTALL.txt");
        let env_body = fs::read_to_string(&report.env_file).expect("env file holds the key");
        assert!(
            env_body.contains(FAKE_KEY),
            "the owner-only staging env file is where the key lives"
        );
        let _ = fs::remove_dir_all(&home);
    }

    /// Named contract: documented workflow does not require a guest git remote.
    #[test]
    fn set_remote_host_console_api_key_documented_workflow_does_not_require_guest_git_remote() {
        let home = test_home();
        let spec = SshInstallSpec {
            user_at_host: "deploy@surmount-1".to_string(),
            remote_grok_home: PathBuf::from(DEFAULT_GUEST_GROK_HOME),
        };
        let report = set_remote_host_console_api_key(
            &home,
            "surmount-1",
            FAKE_KEY,
            Some(SshRequest::PlanOnly(spec)),
        )
        .expect("stage key");
        assert!(
            !report.opens_guest_git,
            "the action must not open git on the guest"
        );
        let cmds = report.ssh_copy_commands.join("\n").to_ascii_lowercase();
        for needle in ["git remote", "git init", "git push", "git clone"] {
            assert!(
                !cmds.contains(needle),
                "SSH copy commands must not run {needle}; got {cmds}"
            );
        }
        assert!(
            report.operator_copy.contains("guest must not git push"),
            "operator copy must say the guest must not git push"
        );
        assert!(
            !report.operator_copy.contains("git remote add"),
            "operator copy must not instruct adding a guest git remote"
        );
        let _ = fs::remove_dir_all(&home);
    }

    /// Named contract: this action does not generate a GitHub SSH key on the guest.
    #[test]
    fn set_remote_host_console_api_key_does_not_generate_github_ssh() {
        let home = test_home();
        let spec = SshInstallSpec {
            user_at_host: "deploy@surmount-1".to_string(),
            remote_grok_home: PathBuf::from(DEFAULT_GUEST_GROK_HOME),
        };
        let recorder = RecordingInstall::new();
        let report = set_remote_host_console_api_key(
            &home,
            "surmount-1",
            FAKE_KEY,
            Some(SshRequest::Install {
                spec,
                installer: &recorder,
            }),
        )
        .expect("install via fake SSH");
        assert!(
            !report.generates_guest_github_ssh,
            "the action must not generate a GitHub SSH key on the guest"
        );
        let hay = format!(
            "{}\n{}",
            report.operator_copy,
            report.ssh_copy_commands.join("\n")
        );
        assert!(!hay.to_ascii_lowercase().contains("ssh-keygen"));
        assert!(!hay.to_ascii_lowercase().contains("github.com"));
        for dest in recorder.dests.lock().expect("dests").iter() {
            assert_key_absent(dest, "scp dest");
        }
        let _ = fs::remove_dir_all(&home);
    }

    /// Named contract: pager /dashboard is a different product from L0.
    #[test]
    fn set_remote_host_console_api_key_is_not_pager_dashboard() {
        let home = test_home();
        let report = set_remote_host_console_api_key(&home, "surmount-1", FAKE_KEY, None)
            .expect("stage key");
        assert!(
            report.operator_copy.contains("not pager /dashboard"),
            "operator copy must say L0 is not pager /dashboard; got {}",
            report.operator_copy
        );
        assert!(
            report.operator_copy.contains("not /running"),
            "operator copy must say L0 is not /running"
        );
        assert!(
            report
                .operator_copy
                .contains("not a website on the mail host :443"),
            "operator copy must say L0 is not mail-host :443"
        );
        let _ = fs::remove_dir_all(&home);
    }

    /// Named contract: scp argv copies the file path and never takes the key.
    #[test]
    fn scp_copy_argv_does_not_include_the_key() {
        let argv = scp_copy_argv(
            Path::new("/tmp/l0-remote-console-key/surmount-1/console.env"),
            "deploy@surmount-1:/home/grok/.grok/console.env",
        );
        let joined = argv.join(" ");
        assert_key_absent(&joined, "scp argv");
        assert_eq!(argv[0], "scp");
        assert!(joined.contains("console.env"));
        assert!(!joined.contains("git"));
        assert!(!joined.contains("ssh-keygen"));
    }

    #[allow(non_snake_case)]
    #[test]
    fn CoordinatorApp_set_remote_host_console_api_key_never_prints_the_key() {
        let home = test_home();
        let app = CoordinatorApp::load(&home, "[]", None).unwrap();
        let report = app
            .set_remote_host_console_api_key("surmount-1", FAKE_KEY)
            .expect("app action");
        assert_key_absent(&report.operator_copy, "CoordinatorApp operator_copy");
        assert_eq!(report.host, "surmount-1");
        assert!(!report.copies_laptop_supergrok_oauth);
        assert!(!report.opens_guest_git);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn refuses_empty_key_without_echoing_input() {
        let home = test_home();
        let err = set_remote_host_console_api_key(&home, "surmount-1", "   ", None)
            .expect_err("empty key");
        let msg = err.to_string();
        assert_key_absent(&msg, "empty-key error");
        assert!(msg.contains("empty"));
        let _ = fs::remove_dir_all(&home);
    }
}
