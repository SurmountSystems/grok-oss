# Progress

Updated when a feature changes state. This file is in the grok-build project.

Finished means the product behavior is in the source and a named test passed, or a specialist wrote a note that names the test. Not installed means the grok-oss process you are looking at does not have it yet. The last install was `grok-oss 1.0.3 (8c46e6b95136)`.

| Feature document | What it is | Finished? | Who is on it |
|---|---|---|---|
| `docs/features/l2-shown-token-is-its-own-context-plus-its-l3s.md` | Row must not say `not_fetched`. Count updates live. Number is that L2 plus each L3 it spawned, once. Not added to the L1 footer. | No. Writer `01a0d296` stopped on a repeating sentence after 6 minutes 5 seconds (2.7m tokens). Red run had painted `(6k)` instead of `57k`. | Specialist `01a0d2ab` on grok-4.7. Coordinator `01a0d2aa-9437` exited after 1 minute 10 seconds. No report. |
| `docs/features/soft-plan-one-file-per-feature.md` | Do not use one `secondary-plan.md`. Each feature is its own file under `docs/features/` with a ULID. | No. Specialist `01a0d293-bed9` stopped after 10 minutes 30 seconds while still saying it would find the write sites. The pane still uses `secondary-plan.md`. | Specialist `01a0d2ab-7119` on grok-4.7. Coordinator `01a0d2aa-9438` exited after 1 minute 15 seconds. No report. |
| `docs/features/soft-plan-implements-after-approve.md` | Approve starts the work. Writing the file is not the end. | No. | Coordinator `01a0d2a8-df85`. |
| `docs/features/soft-plan-waits-for-the-button.md` | Text in the box is not a submit. Approve, Comment, Revise, and Exit are the only submits. | No. | Coordinator `01a0d2a8-df85`. |
| `docs/features/l2-window-shows-when-each-row-happened.md` | Each thought, tool run, and specialist row shows the local time it happened, not only a duration. | In the source. Two tests passed: the row paint and the nested L2 overlay. Clock is `11:12 AM`, no seconds. Not installed. | Nobody. Report `~/.agents/reports/impl-l2-window-row-clock-times-2026-09-24.md`. |
| Uptime status line. Note: `/home/hunter/.agents/reports/l3-short-uptime-status-line-notes-2026-09-24.md` | Status line does not start with `last 15 minutes` when observations exist. `/uptime` still prints the full reading. | Specialist said yes. Named test `operator_uptime_shows_a_real_token_sum_and_omits_the_clause_when_a_count_is_missing`. Not installed. | Done in source. Not running. |
| grok-oss sqlite instead of `TECH.md` | Spawn, usage, and exit call `insert_local_usage_event`. `persist_tech_md` is gone from the source. | In the source. The installed binary `8c46e6b95136` still contains the unused leftovers from before that deletion. | Not running. |
| Remote prompt to surmount-1 | `grok-oss gui --ssh nixbuilder@surmount-1 --session SESSION_ID` reads the prompt from stdin. | The flag is in `cli.rs`. No land report. Not confirmed in the installed binary. | Not running. |
| `docs/features/header-token-count-once.md` | Header shows `↓384.7k 384K / 500K`. Drop `↓384.7k`. Keep `384K / 500K`. | In the source. Test `context_chip_keeps_unlabeled_used_over_total_when_windows_match` failed on `↓207k 207K / 500K`, then passed on `207K / 500K`. Not installed. | Nobody. |
