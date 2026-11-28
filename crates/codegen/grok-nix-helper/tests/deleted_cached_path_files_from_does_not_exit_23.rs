//! A path still listed by `git ls-files --cached` but absent from the worktree
//! must not make the files-from sender exit 23 when `--delete-missing-args` is
//! set. Without that flag the same sender exits 23.
//!
//! The fixture is a temp git repo. This test does not stage, restore, or
//! commit anything in the product tree.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const MISSING: &[&str] = &[
    "crates/codegen/xai-grok-agent/scripts/encrypt_templates.py",
    "crates/codegen/xai-grok-shell/tests/memory_integration/run_tests.py",
];

const ASSERT: &str = "a path still listed by git ls-files --cached but absent from the worktree must not make the files-from sender exit 23, and the destination must not still contain that path after the sync. The snapshot must not contain that path, so it will not re-send a path that is gone.";

struct TempTree {
    path: PathBuf,
}

impl TempTree {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "deleted-cached-rsync.{}.{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap_or_else(|e| panic!("mkdir {}: {e}", path.display()));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn repo_root() -> Option<PathBuf> {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..8 {
        if p.join("justfile").is_file() && p.join("flake.nix").is_file() {
            return Some(p);
        }
        if !p.pop() {
            break;
        }
    }
    None
}

fn recipe_starts(line: &str, name: &str) -> bool {
    let Some(rest) = line.trim_end().strip_prefix(name) else {
        return false;
    };
    rest.is_empty()
        || rest.starts_with(':')
        || rest.starts_with(' ')
        || rest.starts_with('\t')
        || rest.starts_with('*')
        || rest.starts_with('+')
}

/// Recipe body lines after the header, until the next unindented line.
/// Matches the files-from sender scan: first `--files-from=-` line only.
fn files_from_line<'a>(justfile: &'a str, name: &str) -> &'a str {
    let mut taking = false;
    for line in justfile.lines() {
        if !taking {
            if recipe_starts(line, name) {
                taking = true;
            }
            continue;
        }
        if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        if line.contains("--files-from=-") {
            return line;
        }
    }
    panic!("FAIL: {name} has no --files-from sender");
}

fn parse_files_from_opts(line: &str, name: &str) -> Vec<String> {
    let fail = || -> ! { panic!("FAIL: {name} files-from rsync line could not be parsed: {line}") };
    let Some(rsync_at) = line.rfind("rsync") else {
        fail();
    };
    let after = &line[rsync_at + "rsync".len()..];
    if !after.starts_with([' ', '\t']) {
        fail();
    }
    let after = after.trim_start();
    let Some(marker_at) = after.rfind(" ./ ") else {
        fail();
    };
    let opts = after[..marker_at].trim();
    if opts.is_empty() {
        fail();
    }
    if !opts.contains("--files-from=-") || !opts.contains("--from0") {
        panic!("FAIL: {name} files-from opts lost --from0 or --files-from=-: {opts}");
    }
    if !opts.contains("--delete-missing-args") {
        panic!("FAIL: {name} files-from opts lost --delete-missing-args: {opts}");
    }
    opts.split_whitespace().map(str::to_owned).collect()
}

fn git(work: &Path, args: &[&str]) -> Output {
    let out = Command::new("git")
        .arg("-C")
        .arg(work)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git -C {} {args:?} exit {:?}: {}",
        work.display(),
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| panic!("mkdir {}: {e}", parent.display()));
    }
    fs::write(path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn path_present(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

fn slash_dir(path: &Path) -> String {
    let mut s = path.display().to_string();
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn git_ls_files_z(work: &Path) -> Vec<u8> {
    let out = git(
        work,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    );
    out.stdout
}

/// `stdin_bytes` is the files-from list. `None` means no stdin.
fn rsync_status(args: &[String], stdin_bytes: Option<&[u8]>) -> (i32, String) {
    let mut cmd = Command::new("rsync");
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin_bytes.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    let mut child = cmd.spawn().unwrap_or_else(|e| panic!("spawn rsync: {e}"));
    if let Some(bytes) = stdin_bytes {
        let mut input = child.stdin.take().expect("rsync stdin");
        input
            .write_all(bytes)
            .unwrap_or_else(|e| panic!("write rsync stdin: {e}"));
        drop(input);
    }
    let out = child
        .wait_with_output()
        .unwrap_or_else(|e| panic!("wait rsync: {e}"));
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn assert_cached_and_missing_on_disk(name: &str, work: &Path, rel: &str) {
    let listed = git(work, &["ls-files", "--cached", "--", rel]);
    let text = String::from_utf8_lossy(&listed.stdout);
    assert!(
        text.lines().any(|line| line == rel),
        "FAIL: {name} fixture lost cached {rel}"
    );
    assert!(
        !path_present(&work.join(rel)),
        "FAIL: {name} fixture still has {rel} on disk"
    );
}

fn assert_absent(name: &str, root: &Path, rel: &str, message: &str) {
    assert!(!path_present(&root.join(rel)), "FAIL: {name} {message}");
}

/// Same fixture for both sender modes. The exit-23 run writes to
/// `snapshot-absent` so it cannot delete the pre-seeded green snapshot.
fn run_recipe(base: &Path, name: &str, opts: &[String]) {
    let root = base.join(name);
    let work = root.join("work");
    let snapshot = root.join("snapshot");
    let snapshot_absent = root.join("snapshot-absent");
    let dest = root.join("dest");
    let resend = root.join("resend");
    for dir in [&work, &snapshot, &snapshot_absent, &dest, &resend] {
        fs::create_dir_all(dir).unwrap_or_else(|e| panic!("mkdir {}: {e}", dir.display()));
    }

    git(&work, &["init", "-q"]);
    git(
        &work,
        &["config", "user.email", "deleted-cached-rsync@example.com"],
    );
    git(&work, &["config", "user.name", "deleted-cached-rsync"]);
    write_file(&work.join("keep.txt"), b"keep\n");
    git(&work, &["add", "--", "keep.txt"]);

    for rel in MISSING {
        write_file(&work.join(rel), b"stale\n");
        git(&work, &["add", "--", rel]);
        write_file(&snapshot.join(rel), b"stale-snapshot\n");
        write_file(&dest.join(rel), b"stale-dest\n");
        fs::remove_file(work.join(rel)).unwrap_or_else(|e| panic!("rm {name} {rel}: {e}"));
        assert_cached_and_missing_on_disk(name, &work, rel);
    }

    let absent: Vec<String> = opts
        .iter()
        .filter(|word| word.as_str() != "--delete-missing-args")
        .cloned()
        .collect();
    assert!(
        opts.iter().any(|word| word == "--delete-missing-args"),
        "FAIL: {name} files-from opts lost --delete-missing-args: {}",
        opts.join(" ")
    );
    assert!(
        absent.iter().all(|word| word != "--delete-missing-args"),
        "FAIL: {name} red sender still passes --delete-missing-args"
    );
    assert!(
        absent.iter().any(|word| word == "--files-from=-")
            && absent.iter().any(|word| word == "--from0"),
        "FAIL: {name} files-from opts lost --from0 or --files-from=-: {}",
        absent.join(" ")
    );

    let red_list = git_ls_files_z(&work);
    let mut red_args = absent.clone();
    red_args.push(slash_dir(&work));
    red_args.push(slash_dir(&snapshot_absent));
    let (red_code, red_err) = rsync_status(&red_args, Some(&red_list));
    assert_eq!(
        red_code, 23,
        "FAIL: {name} files-from sender without --delete-missing-args exit {red_code}, want 23. {red_err}"
    );

    let green_list = git_ls_files_z(&work);
    let mut green_args = opts.to_vec();
    green_args.push(slash_dir(&work));
    green_args.push(slash_dir(&snapshot));
    let (green_code, green_err) = rsync_status(&green_args, Some(&green_list));
    assert_eq!(
        green_code, 0,
        "FAIL: {name} files-from sender exit {green_code}. {ASSERT}\n{green_err}"
    );

    for rel in MISSING {
        assert_absent(
            name,
            &snapshot,
            rel,
            &format!(
                "snapshot still contains missing cached path {rel}, so it will re-send a path that is gone. {ASSERT}"
            ),
        );
    }
    assert!(
        snapshot.join("keep.txt").is_file(),
        "FAIL: {name} snapshot dropped existing tracked file keep.txt"
    );

    let delete_args = vec![
        "-a".to_string(),
        "--delete".to_string(),
        "--exclude".to_string(),
        "target".to_string(),
        slash_dir(&snapshot),
        slash_dir(&dest),
    ];
    let (delete_code, delete_err) = rsync_status(&delete_args, None);
    assert_eq!(
        delete_code, 0,
        "FAIL: {name} rsync -a --delete --exclude target snapshot/ dest/ exit {delete_code}: {delete_err}"
    );
    for rel in MISSING {
        assert_absent(
            name,
            &dest,
            rel,
            &format!("destination still contains missing cached path {rel}. {ASSERT}"),
        );
    }
    assert!(
        dest.join("keep.txt").is_file(),
        "FAIL: {name} destination dropped existing tracked file keep.txt"
    );

    let resend_args = vec!["-a".to_string(), slash_dir(&snapshot), slash_dir(&resend)];
    let (resend_code, resend_err) = rsync_status(&resend_args, None);
    assert_eq!(
        resend_code, 0,
        "FAIL: {name} rsync -a snapshot/ resend/ exit {resend_code}: {resend_err}"
    );
    for rel in MISSING {
        assert_absent(
            name,
            &resend,
            rel,
            &format!("snapshot re-sent missing cached path {rel}. {ASSERT}"),
        );
    }
}

#[test]
fn deleted_cached_path_files_from_does_not_exit_23() {
    let Some(root) = repo_root() else {
        eprintln!(
            "skipping deleted_cached_path_files_from_does_not_exit_23: repo root not next to crate (isolated helperSrc)"
        );
        return;
    };
    let just =
        fs::read_to_string(root.join("justfile")).unwrap_or_else(|e| panic!("read justfile: {e}"));
    let base = TempTree::new();
    for recipe in ["check-remote", "test-remote"] {
        let line = files_from_line(&just, recipe);
        assert!(
            line.contains("--delete-missing-args"),
            "FAIL: {recipe} files-from line lost --delete-missing-args: {line}"
        );
        assert!(
            line.contains("--files-from=-") && line.contains("--from0"),
            "FAIL: {recipe} files-from opts lost --from0 or --files-from=-: {line}"
        );
        let opts = parse_files_from_opts(line, recipe);
        run_recipe(base.path(), recipe, &opts);
    }
}
