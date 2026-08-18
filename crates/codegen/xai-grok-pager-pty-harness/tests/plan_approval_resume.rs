//! Integration test: the shell re-parks `exit_plan_mode` on
//! resume, so approval chrome reappears after quit/`--continue` and approving
//! via the side-panel footer mouse CTA leaves plan mode + starts the implement
//! turn (letter `a` types; empty Enter never Approves; default park auto-opens
//! the right pane).
//!
//! CI stages the pager binary via `PAGER_BINARY`. Also runs under plain cargo
//! (which builds the pager on demand):
//!
//! ```bash
//! cargo test -p xai-grok-pager-pty-harness --test plan_approval_resume -- --nocapture
//! ```

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_approval_restored_after_resume() {
    xai_grok_pager_pty_harness::scenarios::plan_approval_resume::assert_plan_approval_restored_after_resume()
        .await
        .expect("shell must re-park exit_plan_mode on resume so approval chrome returns");
}
