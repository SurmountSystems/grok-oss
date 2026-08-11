# Join: durable multi-process SuperGrok included poll history

**Date:** 2026-08-03
**Mode:** implementer red→green TDD
**Scope:** Item 4 flat-poll series multi-process aware under `$GROK_HOME` (recon §3 / §6 smallest slice).
**Not this work:** Management / billing SharedRateLimitStore cooldowns (parallel track).

## What shipped

| Piece | Detail |
|-------|--------|
| Module | `crates/codegen/xai-grok-shell/src/auth/included_poll_history.rs` |
| Durable path | `$GROK_HOME/included_poll_history/{sanitized_identity_id}.json` (`DURABLE_SUBDIR`) |
| Mechanism | exclusive flock + JSON ring (same spirit as `rate_limits/` / `exhausted_credits/`) |
| Cap | 32 samples per SuperGrok `identity_id` (unchanged) |
| Process map | Still kept; mirrored from disk after record; load-from-disk on read / evidence |
| Store handle | `IncludedPollHistoryStore::open(grok_home)` for multi-handle tests and clear product root |
| Secrets | File body: `identity_id` + meter samples only (ts ms, included %, optional Build %, optional extras cents). No tokens. |
| Pure detectors | `included_debit_unproven` / `flat_poll_evidence_for_samples` unchanged |
| Product wire | Existing `record_included_poll_*` + `flat_poll_evidence_from_history` paths now durable; billing / pager call sites unchanged |
| Exports | `DURABLE_SUBDIR`, `IncludedPollHistoryStore`, `clear_process_included_poll_history_only` re-exported from `auth/mod.rs` |
| Test helper | `with_history_lock` isolates temp `$GROK_HOME` so unit tests never write operator home |

## Named contracts (green)

| Test | Meaning |
|------|---------|
| Existing pure detector suite (8) | Flat / step / min polls / min window / evidence flags |
| `process_ring_feeds_flat_from_history` | Same-process series still surfaces unproven |
| `two_store_handles_share_poll_samples` | Two store handles on one temp home share samples; flat fires; no secret substrings in file |
| `cold_process_surfaces_flat_from_prior_process_disk` | Process-1 records one sample → clear process only → process-2 loads disk, records second spaced poll → flat after process clear again |
| `durable_ring_caps_at_thirty_two` | Disk ring drops oldest past 32 |

## Commands + evidence

### RED (named multi-process contracts; written then product made them pass)

- Two handles must share samples via flock files (not process map).
- Cold process (empty process map) must load prior samples and surface flat when the series only exists on disk.

### GREEN

```bash
cargo fmt -p xai-grok-shell
cargo test -p xai-grok-shell --lib included_poll_history
# 13 passed

cargo test -p xai-grok-pager --lib flat_poll
# 7 passed (includes limits_snapshot_sets_flat_poll_from_history_not_only_tests)
```

## Acceptance mapping

| Acceptance | Status |
|------------|--------|
| Sequential cold `limits` processes with same GROK_HOME can get multi-sample flat evidence | **Yes** — durable ring + disk scan in `flat_poll_evidence_from_history`; tested via cold process clear + two store handles |
| No secrets/tokens in files | **Yes** — durable schema is meters + identity_id only; test asserts no token/bearer/sk- |
| Existing flat_poll unit tests still green | **Yes** — shell 13 + pager 7 |

## Explicit non-touch

- Did **not** wire Management / SuperGrok billing HTTP into `SharedRateLimitStore` (parallel implementer).
- Did **not** edit `billing.rs` / `xai_management.rs` (record path already called existing free functions).
- Did **not** invent socket IPC.

## Residual / follow-up (not this join)

- Operator live multipoll still needs healthy SuperGrok billing auth; durability only makes the **series available** across cold CLI processes.
- Optional later: durable Management meter cache TTL (recon §6 polish).
