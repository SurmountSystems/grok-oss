# Fix: stale plan body after same-turn rewrite + exit_plan_mode (2026-08-09)

## Summary

When the agent rewrote session `plan.md` and called `exit_plan_mode` in the
**same multi-tool batch**, the plan approval surface and the post-approve tool
result still showed the **pre-write** plan (e.g. "Deploy automation") instead of
the rewritten secrets plan.

## Root cause

Dogfood evidence from surmount-server session chat history (2026-08-09):

1. One assistant turn issued `write`/`search_replace` of `plan.md` (secrets
   plan) **and** `exit_plan_mode` together.
2. `execute_tool_calls` phase 1 **prepares** every tool before phase 2 dispatch.
3. `prepare_tool_call` for `exit_plan_mode` **awaits operator approval** (or, when
   headless, still only reads disk later in the tool body after prepare).
4. Co-batched plan writes sit in the prepared queue and do **not** run until
   after prepare finishes for the whole batch.
5. So park-time reverse-request content and the post-approve tool re-read can
   still see plan A while the write of plan B has not run yet. In the live
   session, `plan.md` mtime matched exit completion time: write landed only
   when exit finished.

Pager-side FileBacked re-read (panel paint, soft-park card, `/view-plan`) was
already green for rewrites **while parked after the write had completed**. That
path could not fix a write that had not executed yet.

## Named contract

Same multi-tool batch that contains both non-exit tools and `exit_plan_mode`
must run the non-exit tools **to completion first**, then prepare/run
`exit_plan_mode`, so park and tool body see the post-write plan body.

## Fix

In `xai-grok-shell` `execute_tool_calls`:

1. `split_tool_batch_before_exit_plan_mode` partitions a mixed batch into
   non-exit tools and exit-plan tools.
2. Non-exit tools run via a full nested `execute_tool_calls` pass first.
3. Then exit-plan tools run (park/re-read after writes have landed).
4. If the first pass fails closed (permission reject / cancel / followup),
   remaining exit tools get cancelled tool results (same messaging as the
   existing in-batch skip path).

## Tests (red → green)

| Test | Package | Contract |
|------|---------|----------|
| `split_tool_batch_runs_non_exit_before_exit_plan_mode` | `xai-grok-shell` | Mixed batch orders write/todo before exit |
| `split_tool_batch_skips_when_exit_only_or_no_exit` | `xai-grok-shell` | No split when not needed |
| `same_batch_plan_write_before_exit_plan_mode_returns_new_body` | `xai-grok-shell` | Headless same-batch `search_replace` plan A→B + `exit_plan_mode` returns B, not A |

Without the two-pass split, the integration test fails when the exit tool body
re-reads before the co-batched write runs (frozen A markers in the tool
result).

### Commands

```text
cargo test -p xai-grok-shell --lib -- split_tool_batch same_batch_plan_write_before_exit
# 3 passed (plus related exit_plan unit tests)

cargo test -p xai-grok-shell --lib -- exit_plan
# 16 passed

cargo test -p xai-grok-tools --lib -- exit_plan_mode
# 19 passed

cargo test -p xai-grok-pager --lib -- file_backed_plan soft_park_card_refreshes
# 4 passed

cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --lib -- -D warnings
# clean
```

`--all-targets` clippy still reports pre-existing await-holding-lock noise in
unrelated test helpers; not introduced by this change.

## Files changed

- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`
  - helpers: `is_exit_plan_mode_tool_name`, `split_tool_batch_before_exit_plan_mode`
  - two-pass `execute_tool_calls` for mixed batches
  - unit tests under `exit_plan_intercept_tests`
- `crates/codegen/xai-grok-shell/src/session/acp_session_tests/plan_approval_resume_tests.rs`
  - integration test `same_batch_plan_write_before_exit_plan_mode_returns_new_body`

No `git add` / `git commit` / push.

## Residual

1. **False-approve wording** on the `ExitPlanMode` PlanReady message
   ("Your plan has been approved. You can now start coding") still fires when
   the tool body runs **after** a real approve (shell intercept). That is not
   the soft-park present path. AGENTS pin about soft-park ≠ approve remains;
   no one-line change required for this bug. Soft residual:
   `bug:exit-plan-mode-false-approve` if product still wants softer present-time
   copy elsewhere.
2. **Agent habit**: still better to write `plan.md` and only then call
   `exit_plan_mode` on a later turn, but the product no longer depends on that.
3. **Pager FileBacked live refresh** (while parked after disk already changed)
   remains as previously shipped; unchanged here.
4. Rebuild the dogfood `grok-oss` binary so the running TUI picks up shell
   ordering.

## Scope left alone

- Full plan UX redesign
- Freeform chat approval menus
- Wire reverse-request streaming of plan body while parked
