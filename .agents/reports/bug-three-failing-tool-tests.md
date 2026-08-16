# Three failing tool tests — synthesis (blocked)

Date: 2026-08-15  
Repo: `/home/hunter/Projects/surmount/grok-build`  
Branch: `onto-xai/b13fa526f511`

## Status

**Blocked.** Both L3 specialists vanished from the wait API (`not_found`). Per parent instruction, those jobs were **not** respawned. No specialist reports were read because they were not produced by these jobs.

## Jobs

| Slice | Spawn description | L3 task id | Wait result | Report |
|-------|-------------------|------------|-------------|--------|
| xai-grok-shell auto-wake cancel-barrier | Fix auto-wake cancel-barrier tests | `01a00873-d32c-7583-8fc9-9ac27dd0f2cf` | `not_found` (long wait, then poll) | `.agents/reports/bug-auto-wake-cancel-barrier.md` — not written by this job |
| xai-grok-tools OpenCode edit relative path | Fix OpenCode edit relative path test | `01a00873-d32c-7583-8fc9-9ad0c4ac56e1` | `not_found` (long wait, then poll) | `.agents/reports/bug-opencode-edit-relative-path.md` — not written by this job |

## Pass/fail per test

| Test | Result |
|------|--------|
| `xai-grok-shell` `session::acp_session::auto_wake_suppression_tests::cancel_barrier_rejects_task_completion_wake_without_reporting_it` | **Unknown.** Specialist vanished before a report. L2 did not run tests. |
| `xai-grok-shell` `session::acp_session::auto_wake_suppression_tests::task_completion_wake_is_admitted_without_cancel_barrier` | **Unknown.** Specialist vanished before a report. L2 did not run tests. |
| `xai-grok-tools` `implementations::opencode::edit::tests::relative_path_resolution` | **Unknown.** Specialist vanished before a report. L2 did not run tests. |

## Files touched

None by this coordinator. L2 did not edit product code. L3s did not return a file list.

## Leftovers

- Named tests are still the operator-pasted failures until a specialist finishes and writes its report.
- Host wait API lost both L3 task ids immediately after spawn. Cause unknown. Not a product diagnosis.
- No reviewer, mop, or replacement L3 was started.

## Coordinator actions

1. Spawned exactly two L3 general-purpose specialists (disjoint crates).
2. Waited (`timeout_ms=600000`), then polled the same ids (`timeout_ms=0`).
3. Both returned `not_found`. Did not respawn.
