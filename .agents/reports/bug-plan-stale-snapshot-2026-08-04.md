# Bug: plan mode shows stale plan copies (2026-08-04)

## Summary

File-backed plan approval could freeze reverse-request `plan_content` after
session `plan.md` was rewritten while approval stayed parked. Users saw the old
body in the open side panel; approve Interject line quotes could still cite the
park-time snapshot; agent post-approve tool text did not stress that the body
was re-read at approval.

## Root cause

1. **Open panel freeze (main dogfood path).** Soft-park auto-opens the plan
   side panel and embeds body in `LineViewerState.markdown_content`. Soft-park
   card refresh only ran when the panel was **closed**. With the panel left open
   (default), disk rewrites did not repaint until a full reopen of the preview.
2. **CTA path used frozen `plan_content`.** `approve_plan` / revise / clarify
   formatted feedback from `plan_approval_view.plan_content` without re-reading
   session `plan.md` first, so line quotes could still show park-time text.
3. **Agent handoff messaging.** `exit_plan_mode` already re-reads disk when the
   tool runs after approve, but the model-facing message did not say the body
   was post-approve disk content (easy to follow earlier draft titles in chat).

Already good before this fix (kept and re-verified):

- `plan_body_for_preview` FileBacked path re-reads disk
- `show_plan_preview` / `reopen_plan_approval` / `/view-plan` refresh path
- Soft-park transcript card refresh when panel is closed
- Shell intercept + tool re-read of `plan.md` at tool run

## Named contracts

1. After session `plan.md` is rewritten while FileBacked approval is parked,
   paint sync refreshes an **already open** plan panel to the new body.
2. After that rewrite, **approve** Interject quotes use the **new** disk body.
3. `exit_plan_mode` tool result embeds **current** disk body and tells the model
   it was re-read at approval.
4. Inline CreatePlan stays request-body first (not file-backed SoT). Missing
   disk FileBacked still falls back to reverse-request snapshot.

## Tests (red → green)

### New / strengthened

| Test | Package | Contract |
|------|---------|----------|
| `file_backed_open_panel_live_refreshes_on_paint_after_disk_rewrite` | `xai-grok-pager` | Open panel + paint sync → disk B |
| `file_backed_approve_interject_quotes_disk_body_after_rewrite` | `xai-grok-pager` | Approve Interject quotes disk B |
| `exit_plan_mode_reads_current_disk_not_earlier_draft` | `xai-grok-tools` | Tool result is disk B + re-read wording |

### Re-verified (pre-existing)

| Test | Package |
|------|---------|
| `file_backed_plan_preview_rereads_disk_after_park_rewrite` | pager |
| `file_backed_reopen_panel_body_is_single_disk_plan_not_dual_merge` | pager |
| `soft_park_card_refreshes_from_disk_after_plan_md_rewrite` | pager |
| `file_backed_plan_preview_falls_back_to_snapshot_when_disk_missing` | pager |
| other `exit_plan_mode` unit suite | tools |

### Commands and results

```text
cargo test -p xai-grok-pager --lib file_backed_
# 8 passed

cargo test -p xai-grok-pager --lib soft_park_card_refreshes
# 1 passed

cargo test -p xai-grok-tools --lib exit_plan_mode
# 19 passed (includes new disk re-read test)
```

Fail mode without the open-panel / approve refresh: open panel and approve
Interject would still contain park-time markers (e.g.
`old_token_economy_marker`) after disk was rewritten to the live title.

## Fix

### Pager (`xai-grok-pager`)

- `refresh_open_file_backed_plan_panel_if_stale`: if FileBacked approval is
  parked and the plan viewer body differs from session `plan.md`, rebuild the
  panel (and refresh `plan_content`).
- Call that from `sync_plan_viewer_approval_chrome` (every paint while panel is
  open).
- Call `refresh_file_backed_plan_from_disk` at the start of `approve_plan`,
  `send_plan_feedback`, and `send_plan_questions`.

### Tools (`xai-grok-tools`)

- `ExitPlanMode` PlanReady message: point at the plan file path, state the body
  was re-read from disk at approval, and prefer it over earlier draft titles.

## Files changed

- `crates/codegen/xai-grok-pager/src/app/agent_view/plan.rs`
- `crates/codegen/xai-grok-tools/src/implementations/grok_build/exit_plan_mode/mod.rs`
- `.agents/reports/bug-plan-stale-snapshot-2026-08-04.md` (this report)

`cargo fmt -p xai-grok-pager -p xai-grok-tools` run.

## Residual

- **Inline CreatePlan** still freezes request body by design (not session
  `plan.md` SoT). Casual `/view-plan` without FileBacked approval still prefers
  inline then disk.
- **Chat history** may still contain intermediate draft plan text; the tool
  result now instructs the model to use the post-approve disk body. No product
  rewrite of prior turns.
- **Wire reverse-request** still carries park-time `planContent` for clients;
  FileBacked UI re-reads disk for display and CTAs. Changing the wire to stream
  updates while parked was out of scope.
- Rebuild the live `grok-oss` binary so the running TUI picks this up.

## Not touched

Team-usage / credit_bar / limits work. Plan-mode paths only. No `git add` /
`git commit`.
