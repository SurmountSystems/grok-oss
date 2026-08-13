# Report: Rewind failed after auto-compact (missing compaction checkpoint)

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Board:** `bug:rewind-compaction-checkpoint`

## Operator symptom

After context hit ~98% and auto-compacted, then plan soft-park toast ("Plan written. Click or /view-plan"), a red rewind error:

```
Cannot rewind to prompt #385 – compaction checkpoint data is unavailable
(Compaction checkpoint file missing: …/compaction_checkpoints/….json)
```

Plan soft-park is unrelated as a *cause*: it only co-occurred. The failure path is cross-compaction **conversation rewind** (`handle_rewind` → `replay_to_prompt`).

## Root cause

1. **Any session with `last_compaction_prompt_index` set forces replay on rewind**
   (`needs_compaction_replay` in `acp_session_impl/rewind.rs`). Replay walks `updates.jsonl` and loads every `CompactionCheckpoint` marker’s file under `session_dir/compaction_checkpoints/{id}.json`.

2. **Hard fail on the first missing intermediate file**
   `ReplayState::handle_checkpoint` returned `NotFound` as soon as *any* referenced checkpoint file was missing, even when a **later** checkpoint on disk fully covered the target. That later load would have replaced conversation state and made the intermediate file unnecessary.

3. **Confirmed on the live dragon-npu session** (read-only check of
   `~/.grok/sessions/…/dragon-npu/019f6476-0a84-70a0-a971-be89fa1011fe`):
   - Missing marker file only at **prompt_index 43** (`4de09d6f-…`)
   - Good checkpoints from **71 through 386** (including the one needed near prompt 385)
   - Replay died at the first missing file and never reached the good files

4. **Secondary durability gap (prevented for new compacts):**
   `persist_compaction_checkpoint` used to enqueue the file write and the `updates.jsonl` marker separately. On write failure the marker could still land (warn-only). File is now written **synchronously first**; marker only after success.

Who triggers rewind: user (or UI) `x.ai/rewind/execute` → `SessionCommand::Rewind` → `handle_rewind`. Not automatic after `exit_plan_mode`. Cancel/pristine rewind is a different path.

## Named product contract

1. **Missing intermediate checkpoint, later file covers target:**
   Cross-compaction rewind/replay **must succeed**. Skip missing/corrupt intermediate markers; load the latest covering checkpoint.

2. **Latest needed checkpoint for the target is missing:**
   Fail soft with a clear message (no panic). Explain that auto-compact history for that target is unavailable; suggest a later prompt or continue without rewind.

3. **New compactions:**
   Write the checkpoint **file before** recording the `updates.jsonl` marker. Never record a marker when the file write fails.

## TDD

### Red

| Item | Detail |
|------|--------|
| Test | `session::helpers::replay::tests::replay_skips_missing_intermediate_checkpoint_when_later_covers_target` |
| Command | `cargo test -p xai-grok-shell --lib replay_skips_missing_intermediate_checkpoint` |
| Fail (before product edit) | `replay must succeed when a later checkpoint covers the target: … Compaction checkpoint file missing: …/ckpt_early.json` |

Companion (soft-fail still required):
`replay_fails_when_latest_needed_checkpoint_file_missing` (missing *covering* file → `NotFound` + checkpoint wording).

### Green

Same filters after product edit: both pass. Full module:

```text
cargo test -p xai-grok-shell --lib session::helpers::replay
# 15 passed
cargo test -p xai-grok-shell --lib rewind_cross_compaction
# 5 passed
```

## Fix summary

| Area | Change |
|------|--------|
| `helpers/replay.rs` | Skip missing/corrupt intermediate post-boundary checkpoints; track `max_post_checkpoint_index`; fail only if the covering checkpoint never loaded. Soften pre-boundary missing to warn + continue (no `original_user_info` from that file). |
| `compaction.rs` | Sync write checkpoint JSON before `updates.jsonl` marker; on write failure, skip marker. |
| `acp_session_impl/rewind.rs` | Clearer soft-fail copy when replay cannot reconstruct. |
| Tests | Two named-contract unit tests in `replay.rs`. |

## Files changed

- `crates/codegen/xai-grok-shell/src/session/helpers/replay.rs`
- `crates/codegen/xai-grok-shell/src/session/compaction.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/rewind.rs`

## Post-impl verify

- `cargo fmt -p xai-grok-shell`
- `cargo clippy -p xai-grok-shell --lib -- -D warnings` (ok)
- Targeted tests above (green)

No `git add` / commit.

## Residual

- Existing sessions with a **missing covering** checkpoint for a chosen target still cannot rewind there (by design). Intermediate-missing sessions (like dragon-npu at #385) work after this build.
- Optional later: surface “rewind unavailable for this range” in the rewind picker when covering checkpoints are known missing, instead of only at execute time.
- Optional later: single `PersistenceMsg` that pairs file + marker for backends that only see the channel (sync path already owns durability on disk).
- Fork copy of checkpoint files remains covered by existing tests (`rewind_succeeds_in_forked_session_with_compaction_checkpoint`); incomplete historical forks may still lack covering files.
