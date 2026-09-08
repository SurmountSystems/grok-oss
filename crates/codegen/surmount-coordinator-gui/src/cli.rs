//! CLI for the laptop L0 coordinator binary.
//!
//! Default: read `/running --json` and print safe JSON.
//! Subcommand `set-remote-host-console-api-key`: paste a machine console
//! API key on stdin, write laptop staging files, never print the key.

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use crate::remote_console_key::{
    DEFAULT_GUEST_GROK_HOME, RemoteHostConsoleKeyError, SshDeployInstall, SshInstallSpec,
    SshRequest, set_remote_host_console_api_key,
};
use crate::{SessionHost, safe_json_from_running};

const SET_KEY_CMD: &str = "set-remote-host-console-api-key";

const USAGE: &str = "surmount-coordinator-gui [--host NAME] [FILE]\n\
     Read /running --json from FILE or stdin. Print safe JSON (no prompt).\n\
     \n\
     surmount-coordinator-gui set-remote-host-console-api-key --host NAME [--grok-home PATH]\n\
         [--ssh USER@HOST] [--ssh-install USER@HOST] [--remote-grok-home PATH]\n\
     Laptop-side action: paste a machine console API key on stdin. Write a staging file.\n\
     Optionally print or run scp as the existing deploy user. Never print the key.\n\
     Do not pass the key on the command line. Do not open git on the guest.\n\
     Do not generate a GitHub SSH key. This is not pager /dashboard and not /running.";

pub fn run_cli(args: impl IntoIterator<Item = String>, stdin: impl Read) -> Result<String, String> {
    let args: Vec<String> = args.into_iter().collect();
    if args.first().map(String::as_str) == Some(SET_KEY_CMD) {
        return run_set_remote_host_console_api_key(&args[1..], stdin);
    }
    run_safe_json(args, stdin)
}

fn run_safe_json(args: Vec<String>, mut stdin: impl Read) -> Result<String, String> {
    let parsed = parse_running_args(args)?;
    let input = match parsed.file {
        None => {
            let mut buf = String::new();
            stdin
                .read_to_string(&mut buf)
                .map_err(|err| format!("could not read stdin: {err}"))?;
            buf
        }
        Some(path) => fs::read_to_string(&path)
            .map_err(|err| format!("could not read {}: {err}", path.display()))?,
    };
    safe_json_from_running(&input, parsed.host).map_err(|err| err.to_string())
}

struct RunningArgs {
    file: Option<PathBuf>,
    host: SessionHost,
}

fn parse_running_args(args: Vec<String>) -> Result<RunningArgs, String> {
    let mut file = None;
    let mut host = SessionHost::Local;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--help" || arg == "-h" {
            return Err(USAGE.to_string());
        }
        if arg == "--host" {
            let name = iter
                .next()
                .ok_or_else(|| "--host needs a host name".to_string())?;
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err("--host needs a host name".to_string());
            }
            host = if trimmed.eq_ignore_ascii_case("local") {
                SessionHost::Local
            } else {
                SessionHost::Remote(trimmed.to_string())
            };
            continue;
        }
        if arg.starts_with('-') {
            return Err(format!("unknown argument: {arg}"));
        }
        if file.is_some() {
            return Err("only one FILE is allowed".to_string());
        }
        file = Some(PathBuf::from(arg));
    }
    Ok(RunningArgs { file, host })
}

struct SetKeyArgs {
    host: String,
    grok_home: PathBuf,
    ssh: Option<String>,
    ssh_install: Option<String>,
    remote_grok_home: PathBuf,
}

fn parse_set_key_args(args: &[String]) -> Result<SetKeyArgs, String> {
    let mut host = None;
    let mut grok_home = None;
    let mut ssh = None;
    let mut ssh_install = None;
    let mut remote_grok_home = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--help" || arg == "-h" {
            return Err(USAGE.to_string());
        }
        if arg == "--host" {
            host = Some(next_value(&mut iter, "--host")?);
            continue;
        }
        if arg == "--grok-home" {
            grok_home = Some(PathBuf::from(next_value(&mut iter, "--grok-home")?));
            continue;
        }
        if arg == "--ssh" {
            ssh = Some(next_value(&mut iter, "--ssh")?);
            continue;
        }
        if arg == "--ssh-install" {
            ssh_install = Some(next_value(&mut iter, "--ssh-install")?);
            continue;
        }
        if arg == "--remote-grok-home" {
            remote_grok_home = Some(PathBuf::from(next_value(&mut iter, "--remote-grok-home")?));
            continue;
        }
        if arg.starts_with('-') {
            return Err(format!("unknown argument: {arg}"));
        }
        return Err(RemoteHostConsoleKeyError::KeyOnCommandLine.to_string());
    }
    let host =
        host.ok_or_else(|| "--host needs a host name (for example surmount-1)".to_string())?;
    let grok_home = grok_home.unwrap_or_else(default_laptop_grok_home);
    let remote_grok_home =
        remote_grok_home.unwrap_or_else(|| PathBuf::from(DEFAULT_GUEST_GROK_HOME));
    Ok(SetKeyArgs {
        host,
        grok_home,
        ssh,
        ssh_install,
        remote_grok_home,
    })
}

fn next_value<'a>(
    iter: &mut impl Iterator<Item = &'a String>,
    flag: &str,
) -> Result<String, String> {
    let value = iter.next().ok_or_else(|| format!("{flag} needs a value"))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{flag} needs a value"));
    }
    Ok(trimmed.to_string())
}

fn default_laptop_grok_home() -> PathBuf {
    if let Ok(home) = std::env::var("GROK_HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".grok"))
        .unwrap_or_else(|_| PathBuf::from(".grok"))
}

fn run_set_remote_host_console_api_key(
    args: &[String],
    mut stdin: impl Read,
) -> Result<String, String> {
    let parsed = parse_set_key_args(args)?;
    let mut key = String::new();
    stdin
        .read_to_string(&mut key)
        .map_err(|err| format!("could not read the key from stdin: {err}"))?;
    let installer = SshDeployInstall;
    let ssh_request = if let Some(user_at_host) = parsed.ssh_install.as_ref() {
        Some(SshRequest::Install {
            spec: SshInstallSpec {
                user_at_host: user_at_host.clone(),
                remote_grok_home: parsed.remote_grok_home.clone(),
            },
            installer: &installer,
        })
    } else {
        parsed.ssh.as_ref().map(|user_at_host| {
            SshRequest::PlanOnly(SshInstallSpec {
                user_at_host: user_at_host.clone(),
                remote_grok_home: parsed.remote_grok_home.clone(),
            })
        })
    };
    let report =
        set_remote_host_console_api_key(&parsed.grok_home, &parsed.host, &key, ssh_request)
            .map_err(|err| err.to_string())?;
    debug_assert!(
        !report.operator_copy.contains(key.trim()) || key.trim().is_empty(),
        "CLI output must never include the key"
    );
    Ok(report.operator_copy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SET_REMOTE_HOST_CONSOLE_API_KEY_ACTION;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};

    const FAKE_KEY: &str = "xai-FAKE-MACHINE-CONSOLE-KEY-DO-NOT-USE-9f3c2a1b";

    fn test_home() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "surmount-l0-cli-key-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).expect("temp grok home");
        p
    }

    /// Named contract: the CLI never prints the machine console API key.
    #[test]
    fn set_remote_host_console_api_key_cli_never_prints_the_key() {
        let home = test_home();
        let args = vec![
            SET_KEY_CMD.to_string(),
            "--host".to_string(),
            "surmount-1".to_string(),
            "--grok-home".to_string(),
            home.display().to_string(),
        ];
        let out = run_cli(args, Cursor::new(FAKE_KEY)).expect("cli");
        assert!(
            !out.contains(FAKE_KEY),
            "CLI stdout must never include the key; got {out}"
        );
        assert!(out.contains(SET_REMOTE_HOST_CONSOLE_API_KEY_ACTION));
        assert!(out.contains("not pager /dashboard"));
        let env_path = home.join("l0-remote-console-key/surmount-1/console.env");
        let env_body = fs::read_to_string(&env_path).expect("staged env");
        assert!(env_body.contains(FAKE_KEY));
        let _ = fs::remove_dir_all(&home);
    }

    /// Named contract: passing the key as argv is refused and not echoed.
    #[test]
    fn set_remote_host_console_api_key_cli_refuses_key_on_argv() {
        let home = test_home();
        let args = vec![
            SET_KEY_CMD.to_string(),
            "--host".to_string(),
            "surmount-1".to_string(),
            "--grok-home".to_string(),
            home.display().to_string(),
            FAKE_KEY.to_string(),
        ];
        let err = run_cli(args, Cursor::new("")).expect_err("argv key");
        assert!(
            !err.contains(FAKE_KEY),
            "error must not echo the key; got {err}"
        );
        assert!(err.contains("do not pass the machine console API key on the command line"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn set_remote_host_console_api_key_cli_ssh_plan_omits_the_key() {
        let home = test_home();
        let args = vec![
            SET_KEY_CMD.to_string(),
            "--host".to_string(),
            "surmount-1".to_string(),
            "--grok-home".to_string(),
            home.display().to_string(),
            "--ssh".to_string(),
            "deploy@surmount-1".to_string(),
        ];
        let out = run_cli(args, Cursor::new(FAKE_KEY)).expect("cli ssh plan");
        assert!(!out.contains(FAKE_KEY));
        assert!(out.contains("scp"));
        assert!(!out.to_ascii_lowercase().contains("git remote"));
        assert!(!out.to_ascii_lowercase().contains("ssh-keygen"));
        let _ = fs::remove_dir_all(&home);
    }
}
