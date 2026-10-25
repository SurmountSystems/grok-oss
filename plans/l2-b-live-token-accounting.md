# Plan 0 - Live token accounting: lock-free slots, one count, reused ledger math

> Keep a live-updating token counter as a Vec of `AtomicU64` slots (input, output, cached, reasoning) so every completed model call at L1, L2, and L3, including retries and tool-loop turns, is counted once, and cost stays the existing ledger addition.

**North star:** L2-C can read live per-slot totals and a rollup that equals the sum of leaves, persist per-call and per-job rows, and read estimate versus actual by job kind, without guessing slot indices, without a second cost formula, and without putting JWTs in SQLite.

**Baseline (what exists today):**
- Durable cost: `model_call_cost_for_sessions` in `crates/codegen/xai-grok-shell/src/token_economy/ledger.rs` (export 656). Cost is `input_tokens + output_tokens` on complete `main` / `agent_turn` rows. Reasoning is already inside output. Cached is already inside input. `total_tokens` is not the cost. Named test `l3_model_call_tokens_contribute_to_l2_cost` at ledger.rs:1021.
- Per-call append log: `{session_dir}/usage.jsonl` (`session/usage_log.rs`, `USAGE_FILE` at line 22). Ingest into `grok_oss.db` table `local_usage_event` (schema v1, `grok_oss/mod.rs`:209).
- Session snapshot (not the append log): `{session_dir}/usage.json` (`session/storage/mod.rs`:34, `session/usage_file.rs`).
- Live nested overlay: `l2_token_tracking.rs` `AtomicU64` high-water (`fetch_max` at 290) plus `sum_shown_counts_once` (421) and `l2_row_count_present_past_and_each_specialist_once` (443). That map is nested **context** chrome, not the four-slot cost vec.
- Nested output atoms: `session/mod.rs` `nested_output` (`record_own_model_response` 652, `live_output_total` 722). L3 under L2 is linked only when the parent is already nested (`note_descendant_if_parent_nested` 687).
- Gap: no published lock-free four-slot vec, no schema-v8 per-job estimate-versus-actual table, and L2-C has no small stable call surface.

---

## Goals

### G1 - Lock-free four-slot vec
Each agent (session id) owns a `Vec` / `[AtomicU64; 4]` updated with `fetch_add` as each completed model call lands. Slot order is `TokenSlot` so L2-C does not guess indices: Input=0, Output=1, Cached=2, Reasoning=3. Cost is slot 0 plus slot 1. Cached and reasoning stay visible slots; they are not added into cost.

### G2 - Count once across L1 / L2 / L3, retries, tool-loop turns
Each completed call has one `event_ulid`. The live vec and SQLite both treat that ULID as once. A retry that completes is a new ULID, not a rewrite of the failed call. Tool-loop turns count as the model calls that produced them (`record_response_token_usage` at `sampler_turn.rs`:2663), not as a second pass over the same tokens. Reasoning already inside output is not re-added. Parent rollup is the sum of leaf session ids passed in. Completion-notice rows (`turn_type` `nested`) stay out of cost, same as the ledger.

### G3 - Reuse ledger math
Durable job cost **calls** `model_call_cost_for_sessions` (ledger.rs:656). Live cost uses the same addition (`input.saturating_add(output)` at ledger.rs:679). Do not fork. Do not add `cached_tokens`. Do not add `reasoning_tokens`. Do not use `total_tokens` as cost. Overlay context functions (`sum_shown_counts_once`, `fetch_max` high-water) stay context; they are not a second cost book.

### G4 - SQLite without secrets
Additive schema v8 on `$GROK_HOME/grok_oss.db`. Per-call rows stay `local_usage_event` (already the ingest table). New table `job_cost` stores per-job estimate versus actual so job kind can be read back. No JWT, api_key, password, bearer, management_key, or secret columns (extend `no_secret_columns_in_schema` at `grok_oss/mod.rs`:642).

---

## Workstreams
| # | Work | Done when |
|---|---|---|
| W0 | This plan + published API note | both files on disk; L2-C can call the named surface without guessing slots |
| W1 | `token_economy/live.rs`: `TokenSlot`, `LiveTokenLedger`, record/read/rollup | `record_completed_model_call` `fetch_add`s the four slots; `live_cost` equals input plus output |
| W2 | Schema v8 `job_cost` + persist/finish that **calls** `model_call_cost_for_sessions` | `estimate_vs_actual_by_job_kind` returns stored estimate and ledger actual |
| W3 | Wire record at each completed model call (L1/L2/L3, retries, tool-loop turns) | one `event_ulid` cannot `fetch_add` twice; rollup equals sum of leaves |
| W4 | Tests in Rust: once-count, no double under concurrency, cost formula unchanged | named tests cite `l3_model_call_tokens_contribute_to_l2_cost` and do not rewrite its asserts |

Tests in Rust (`xai-grok-shell` / pager unit tests next to the named ledger and overlay tests). No Python drivers. This Plan 0 turn is docs only.

## Success criteria
1. L2-C records a completed call, reads four slots by `TokenSlot`, and reads rollup without guessing vec indices.
2. Live cost and durable cost are the same addition: input plus output. Cached and reasoning are not extra addends.
3. Passing L2 plus L3 session ids to `model_call_cost_for_sessions` includes L3 tokens in the L2 job cost (existing test at ledger.rs:1021).
4. `job_cost` can be grouped by `job_kind` for estimate versus actual. SQLite schema has no JWT column.

## Non-goals
Painting the Subagents dock (L2-A). Token report tool copy or agent prompts (L2-C). `GROK_TOOL_RESULT_FORMAT` defaults (L2-D). Editing `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` (stop and report if paint would need that file). A second cost formula. Putting JWTs or other secrets in SQLite. Changing `slakeOwnsPackageTypecheck`, FullHost, occupancy, or mill range.

## Later implement write set (not this turn)
- `crates/codegen/xai-grok-shell/src/token_economy/` (new `live.rs`, re-export from `mod.rs`; do not rewrite `model_call_cost_for_sessions`)
- `crates/codegen/xai-grok-pager/src/app/agent_view/l2_token_tracking.rs` (reuse `sum_shown_counts_once` / `fetch_max`; do not keep a second cost counter)
- `crates/codegen/xai-grok-shell/src/grok_oss/mod.rs` plus a v8 apply helper (additive migration, `SCHEMA_VERSION` 7 to 8)

## Stop / report if a later edit would share
- `crates/codegen/xai-grok-pager/src/app/agent_view/viewer.rs` (forbidden this wave)
- Subagent dock view owned by L2-A
- Token report tool / agent prompts owned by L2-C
- `GROK_TOOL_RESULT_FORMAT` owned by L2-D
- `crates/codegen/xai-grok-pager/src/views/tasks_pane.rs` imports overlay helpers; if a signature change would force that file, stop and report

## Published API
Same directory as this plan: `/home/hunter/plans/l2-b-published-api.md`. Copy: `/home/hunter/.agents/reports/l2-b-published-api.md`.
