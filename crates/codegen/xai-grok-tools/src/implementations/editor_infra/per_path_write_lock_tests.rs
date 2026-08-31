//! ACP edit-tool contracts for the per-path write lock.
//!
//! These tests call `search_replace`, `apply_patch`, `write`, and
//! `hashline_edit`. They are the product red/green proof. The lock table
//! unit tests live next to the helper module.

use std::sync::Arc;

use crate::computer::local::LocalFs;
use crate::implementations::codex::apply_patch::{ApplyPatchInput, ApplyPatchTool};
use crate::implementations::editor_infra::per_path_write_lock::{
    release_holder, try_acquire_write, try_reserve_writes,
};
use crate::implementations::grok_build::search_replace::{SearchReplaceInput, SearchReplaceTool};
use crate::implementations::grok_build_hashline::edit::{
    HashlineEditInput, HashlineEditTool, HashlineOp,
};
use crate::implementations::opencode::write::{WriteInput, WriteTool};
use crate::notification::types::ToolNotificationHandle;
use crate::types::output::SearchReplaceOutput;
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

#[tokio::test]
async fn search_replace_refuses_a_path_reserved_by_another_agent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("reserved.txt");
    std::fs::write(&path, "keep\n").unwrap();
    let holder = format!("spawn-claim-{}", path.display());
    try_reserve_writes([&path], &holder).unwrap();

    let err = xai_tool_runtime::Tool::run(
        &SearchReplaceTool,
        test_ctx(search_replace_resources(tmp.path(), "other-writer")),
        search_replace_input("reserved.txt", "keep\n", "overwrite\n"),
    )
    .await
    .expect_err("a spawn-time claim must block another agent's edit");

    assert!(
        err.detail.contains(&holder),
        "error must name the holder: {}",
        err.detail
    );
    assert!(
        err.detail.contains("reserved.txt"),
        "error must name the file: {}",
        err.detail
    );
    assert_no_human_lock_menu(&err.detail);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep\n");
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
