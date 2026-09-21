//! ACP edit-tool contracts for the per-path write lock and CoW snapshot read.
//!
//! These tests call `search_replace`, `apply_patch`, `write`,
//! `hashline_edit`, and `read_file`. They are the product red/green proof.
//! The lock table unit tests live next to the helper module.

use std::sync::Arc;

use crate::computer::local::LocalFs;
use crate::implementations::codex::apply_patch::{ApplyPatchInput, ApplyPatchTool};
use crate::implementations::editor_infra::per_path_write_lock::{
    format_soft_assignment_reminder, held, normalize_lock_path, release_holder, try_acquire_read,
    try_acquire_write, try_reserve_writes,
};
use crate::implementations::grok_build::read_file::{ReadFileInput, ReadFileTool};
use crate::implementations::grok_build::search_replace::{SearchReplaceInput, SearchReplaceTool};
use crate::implementations::grok_build_hashline::edit::{
    HashlineEditInput, HashlineEditTool, HashlineOp,
};
use crate::implementations::opencode::write::{WriteInput, WriteTool};
use crate::notification::types::ToolNotificationHandle;
use crate::types::output::{ReadFileOutput, SearchReplaceOutput};
use crate::types::resources::{Cwd, FileSystem, NotificationHandle, OwnerSessionId, Resources};
use crate::types::template_renderer::TemplateRenderer;
use crate::types::tool::ToolKind;
use crate::types::tool_metadata::test_ctx;
use tempfile::TempDir;

fn search_replace_resources(
    cwd: &std::path::Path,
    holder: &str,
) -> crate::types::resources::SharedResources {
    let mut resources = Resources::new();
    resources.insert(Cwd(cwd.to_path_buf()));
    resources.insert(FileSystem(Arc::new(LocalFs)));
    resources.insert(NotificationHandle(ToolNotificationHandle::noop()));
    resources.insert(OwnerSessionId(holder.to_string()));
    let edit_params = std::collections::HashMap::from([
        ("old_string".to_string(), "old_string".to_string()),
        ("new_string".to_string(), "new_string".to_string()),
        ("replace_all".to_string(), "replace_all".to_string()),
    ]);
    resources.insert(TemplateRenderer::new(
        std::collections::HashMap::from([(ToolKind::Read, "read_file".to_string())]),
        std::collections::HashMap::from([(ToolKind::Edit, edit_params)]),
    ));
    resources.into_shared()
}

fn write_resources(
    cwd: &std::path::Path,
    holder: &str,
) -> crate::types::resources::SharedResources {
    let mut resources = Resources::new();
    resources.insert(Cwd(cwd.to_path_buf()));
    resources.insert(FileSystem(Arc::new(LocalFs)));
    resources.insert(NotificationHandle(ToolNotificationHandle::noop()));
    resources.insert(OwnerSessionId(holder.to_string()));
    resources.into_shared()
}

fn search_replace_input(file_path: &str, old: &str, new: &str) -> SearchReplaceInput {
    SearchReplaceInput {
        file_path: file_path.to_string(),
        old_string: old.to_string(),
        new_string: new.to_string(),
        replace_all: false,
    }
}

fn wrap_patch(body: &str) -> String {
    format!("*** Begin Patch\n{body}\n*** End Patch")
}

fn assert_no_human_lock_menu(message: &str) {
    let lower = message.to_ascii_lowercase();
    assert!(
        !lower.contains("steal"),
        "error must not offer steal: {message}"
    );
    assert!(
        !lower.contains("skip"),
        "error must not offer skip: {message}"
    );
    assert!(
        !lower.contains("wait"),
        "error must not offer wait: {message}"
    );
    assert!(
        !lower.contains("menu"),
        "error must not be a human menu: {message}"
    );
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins exclusive writes so two agents cannot edit the same path at once.
#[tokio::test]
async fn two_agents_cannot_write_the_same_path_at_once() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("shared.txt");
    std::fs::write(&path, "original\n").unwrap();
    let _held = try_acquire_write(&path, "explore-agent-a").unwrap();

    let err = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "explore-agent-b")),
        search_replace_input("shared.txt", "original\n", "changed by b\n"),
    )
    .await
    .expect_err("second writer must be a tool error");

    assert!(
        err.detail.contains("explore-agent-a"),
        "error must name the holder: {}",
        err.detail
    );
    assert!(
        err.detail.contains("shared.txt"),
        "error must name the file: {}",
        err.detail
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "original\n",
        "disk must be unchanged when the lock is held"
    );
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins a silent happy path when the first writer holds the path.
#[tokio::test]
async fn happy_path_first_writer_succeeds_silently() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("solo.txt");
    std::fs::write(&path, "hello\n").unwrap();

    let result = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "first-writer")),
        search_replace_input("solo.txt", "hello\n", "goodbye\n"),
    )
    .await
    .unwrap();

    match result {
        SearchReplaceOutput::EditsApplied(applied) => {
            let text = applied.tool_output_for_prompt.to_ascii_lowercase();
            assert!(
                !text.contains("lock") && !text.contains("already writing"),
                "happy path must stay silent about the lock: {}",
                applied.tool_output_for_prompt
            );
        }
        other => panic!("expected EditsApplied, got {other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "goodbye\n");
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins the lock to the tool call, then a later call may write.
#[tokio::test]
async fn lock_releases_after_the_tool_call_so_a_later_call_can_write() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("later.txt");
    std::fs::write(&path, "one\n").unwrap();

    let first = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "first-writer")),
        search_replace_input("later.txt", "one\n", "two\n"),
    )
    .await
    .unwrap();
    assert!(matches!(first, SearchReplaceOutput::EditsApplied(_)));

    let second = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "second-writer")),
        search_replace_input("later.txt", "two\n", "three\n"),
    )
    .await
    .unwrap();
    assert!(matches!(second, SearchReplaceOutput::EditsApplied(_)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "three\n");
}

// Grok OSS: ACP per-path write lock extra. GitHub #129. After write returns,
// `held` is empty. Lock must be released.
#[tokio::test]
async fn after_write_returns_held_is_empty_lock_must_be_released() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("held-empty.txt");
    std::fs::write(&path, "before\n").unwrap();

    let result = xai_tool_runtime::Tool::run(
        &WriteTool,
        test_ctx(write_resources(tmp.path(), "write-then-release")),
        WriteInput {
            file_path: path.to_string_lossy().into_owned(),
            content: "after write\n".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(result, SearchReplaceOutput::EditsApplied(_)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "after write\n");
    let key = normalize_lock_path(&path);
    assert!(
        !held().contains_key(&key),
        "after write returns, held is empty; lock must be released"
    );
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins search_replace, apply_patch, and write to the same exclusive lock.
#[tokio::test]
async fn search_replace_apply_patch_and_write_all_take_the_lock() {
    let tmp = TempDir::new().unwrap();
    let sr_path = tmp.path().join("sr.txt");
    let patch_path = tmp.path().join("patch.txt");
    let write_path = tmp.path().join("write.txt");
    std::fs::write(&sr_path, "sr-original\n").unwrap();
    std::fs::write(&patch_path, "patch-original\n").unwrap();
    std::fs::write(&write_path, "write-original\n").unwrap();

    let _sr = try_acquire_write(&sr_path, "holder-sr").unwrap();
    let _patch = try_acquire_write(&patch_path, "holder-patch").unwrap();
    let _write = try_acquire_write(&write_path, "holder-write").unwrap();

    let sr_err = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "other-sr")),
        search_replace_input("sr.txt", "sr-original\n", "sr-changed\n"),
    )
    .await
    .expect_err("search_replace must take the lock");
    assert!(sr_err.detail.contains("holder-sr"), "{}", sr_err.detail);
    assert!(sr_err.detail.contains("sr.txt"), "{}", sr_err.detail);
    assert_eq!(std::fs::read_to_string(&sr_path).unwrap(), "sr-original\n");

    let patch = wrap_patch("*** Update File: patch.txt\n@@\n-patch-original\n+patch-changed\n");
    let patch_err = xai_tool_runtime::Tool::run(
        &ApplyPatchTool,
        test_ctx(write_resources(tmp.path(), "other-patch")),
        ApplyPatchInput { patch },
    )
    .await
    .expect_err("apply_patch must take the lock");
    assert!(
        patch_err.detail.contains("holder-patch"),
        "{}",
        patch_err.detail
    );
    assert!(
        patch_err.detail.contains("patch.txt"),
        "{}",
        patch_err.detail
    );
    assert_eq!(
        std::fs::read_to_string(&patch_path).unwrap(),
        "patch-original\n"
    );

    let write_err = xai_tool_runtime::Tool::run(
        &WriteTool,
        test_ctx(write_resources(tmp.path(), "other-write")),
        WriteInput {
            file_path: write_path.to_string_lossy().into_owned(),
            content: "write-changed\n".to_string(),
        },
    )
    .await
    .expect_err("write must take the lock");
    assert!(
        write_err.detail.contains("holder-write"),
        "{}",
        write_err.detail
    );
    assert!(
        write_err.detail.contains("write.txt"),
        "{}",
        write_err.detail
    );
    assert_eq!(
        std::fs::read_to_string(&write_path).unwrap(),
        "write-original\n"
    );
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins a named holder and file, not a steal/skip/wait menu.
#[tokio::test]
async fn held_path_error_names_holder_and_file_without_a_steal_skip_wait_menu() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("conflict.txt");
    std::fs::write(&path, "keep\n").unwrap();
    let _held = try_acquire_write(&path, "explore-agent-a").unwrap();

    let err = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "explore-agent-b")),
        search_replace_input("conflict.txt", "keep\n", "overwrite\n"),
    )
    .await
    .expect_err("held path must be a tool error");

    assert!(err.detail.contains("explore-agent-a"), "{}", err.detail);
    assert!(err.detail.contains("conflict.txt"), "{}", err.detail);
    assert_no_human_lock_menu(&err.detail);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep\n");
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins sequential writes after the first tool call returns, even when both agents share write_paths.
#[tokio::test]
async fn sequential_search_replace_succeeds_after_the_first_tool_call_returns_when_both_agents_were_assigned_the_same_write_paths()
 {
    // Operator: write locks must be hard at the tool-call level, not at
    // the agent/layer level. Two agents sequential writes to the same
    // file after first tool call returns must succeed.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("shared-seq.txt");
    std::fs::write(&path, "one\n").unwrap();
    let first = format!("seq-sr-a-{}", path.display());
    let second = format!("seq-sr-b-{}", path.display());
    try_reserve_writes([&path], &first);
    try_reserve_writes([&path], &second);

    let first_result = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), &first)),
        search_replace_input("shared-seq.txt", "one\n", "two\n"),
    )
    .await
    .expect("first agent's tool call must succeed");
    assert!(matches!(first_result, SearchReplaceOutput::EditsApplied(_)));

    let second_result = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), &second)),
        search_replace_input("shared-seq.txt", "two\n", "three\n"),
    )
    .await
    .expect("after the first tool call returns, the sibling write must succeed");
    assert!(matches!(
        second_result,
        SearchReplaceOutput::EditsApplied(_)
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "three\n");
    release_holder(&first);
    release_holder(&second);
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins spawn-time write_paths as a reminder, not an exclusive block.
#[tokio::test]
async fn search_replace_succeeds_when_a_sibling_only_has_a_soft_write_paths_assignment() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("reserved.txt");
    std::fs::write(&path, "keep\n").unwrap();
    let holder = format!("spawn-claim-{}", path.display());
    try_reserve_writes([&path], &holder);

    let note = format_soft_assignment_reminder(Some("other-writer"))
        .expect("soft-lock reminder must be observable");
    assert!(
        note.contains(&format!("L2 {holder} is assigned these paths")),
        "reminder must name the assigned sibling: {note}"
    );
    assert!(
        note.contains("reserved.txt"),
        "reminder must name the file: {note}"
    );

    let result = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "other-writer")),
        search_replace_input("reserved.txt", "keep\n", "overwrite\n"),
    )
    .await
    .expect("a spawn-time write_paths assignment must not exclusive-block another agent's edit");
    assert!(matches!(result, SearchReplaceOutput::EditsApplied(_)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "overwrite\n");
    release_holder(&holder);
}

fn hashline_write_input(file_path: &str, content: &str) -> HashlineEditInput {
    HashlineEditInput {
        file_path: file_path.to_string(),
        edits: vec![HashlineOp::Write {
            content: content.to_string(),
        }],
    }
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins hashline_edit to the same exclusive lock.
#[tokio::test]
async fn hashline_edit_refuses_when_another_agent_holds_the_path() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("hashline-held.txt");
    std::fs::write(&path, "original\n").unwrap();
    let _held = try_acquire_write(&path, "explore-agent-a").unwrap();

    let err = xai_tool_runtime::Tool::run(
        &HashlineEditTool,
        test_ctx(write_resources(tmp.path(), "explore-agent-b")),
        hashline_write_input("hashline-held.txt", "changed by b\n"),
    )
    .await
    .expect_err("hashline_edit must be a tool error when the path is held");

    assert!(
        err.detail.contains("explore-agent-a"),
        "error must name the holder: {}",
        err.detail
    );
    assert!(
        err.detail.contains("hashline-held.txt"),
        "error must name the file: {}",
        err.detail
    );
    assert_no_human_lock_menu(&err.detail);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "original\n",
        "disk must be unchanged when the lock is held"
    );
}

// Grok OSS: ACP per-path write lock extra. This diverges from upstream xAI because FORK.md pins a silent hashline happy path.
#[tokio::test]
async fn hashline_edit_happy_path_does_not_mention_the_lock() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("hashline-solo.txt");
    std::fs::write(&path, "hello\n").unwrap();

    let result = xai_tool_runtime::Tool::run(
        &HashlineEditTool,
        test_ctx(write_resources(tmp.path(), "first-writer")),
        hashline_write_input("hashline-solo.txt", "goodbye\n"),
    )
    .await
    .unwrap();

    match result {
        SearchReplaceOutput::EditsApplied(applied) => {
            let text = applied.tool_output_for_prompt.to_ascii_lowercase();
            assert!(
                !text.contains("lock") && !text.contains("already writing"),
                "happy path must stay silent about the lock: {}",
                applied.tool_output_for_prompt
            );
        }
        other => panic!("expected EditsApplied, got {other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "goodbye\n");
}

// Grok OSS: ACP per-path lock extra. This diverges from upstream xAI because the Operator contract is a CoW snapshot read lock, ephemeral, many readers, one writer.
#[tokio::test]
async fn read_file_uses_cow_snapshot_and_does_not_take_the_exclusive_write_lock() {
    // Operator: read lock is a snapshot read (CoW), separate from the write
    // lock, ephemeral, does not interfere with writers. Readers must not take
    // the exclusive write lock.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("cow-read.txt");
    std::fs::write(&path, "before\n").unwrap();
    let snapshot = try_acquire_read(&path).expect("CoW snapshot read must succeed");
    let _held = try_acquire_write(&path, "explore-agent-a")
        .expect("a snapshot reader must not exclusive-block a writer");
    std::fs::write(&path, "after\n").unwrap();

    let result = xai_tool_runtime::Tool::run(
        &ReadFileTool,
        test_ctx(search_replace_resources(tmp.path(), "reader-b")),
        ReadFileInput {
            path: "cow-read.txt".to_string(),
            offset: None,
            limit: None,
            pages: None,
            format: None,
        },
    )
    .await
    .expect("read_file must not take the exclusive write lock");

    match result {
        ReadFileOutput::FileContent(content) => {
            assert!(
                content.content.contains("before"),
                "CoW snapshot while a writer holds must be the pre-write point in time: {}",
                content.content
            );
            assert!(
                !content.content.contains("after"),
                "in-flight disk mutation must not leak into the snapshot: {}",
                content.content
            );
        }
        other => panic!("expected FileContent, got {other:?}"),
    }
    assert_eq!(snapshot.as_bytes(), b"before\n");
}
