# usage.jsonl (workstream D0 + D1)

Date: 2026-07-25 (D0); D1 extended 2026-07-26

## What

Append-only per-session **`usage.jsonl`** written at the end of every model
turn that folds through `record_response_token_usage`. Rows are SQL-ready
(snake_case, schema_version stamped). Fail-open I/O — write failures never
break the turn.

| Item | Location |
|------|----------|
| Schema + writer | `xai-grok-shell` `session/usage_log.rs` |
| Hub | `record_response_token_usage` in `session/acp_session_impl/sampler_turn.rs` |
| Path | `{session_dir}/usage.jsonl` (alongside `updates.jsonl`, `events.jsonl`) |
| ULID | `xai_grok_tools::util::ulid::mint()` for `event_ulid` (+ optional `work_ulid`) |

## Schema v1 (one JSON object per line)

| Field | Type | Notes |
|-------|------|--------|
| `schema_version` | u32 | `1` |
| `event_ulid` | string | 26-char Crockford ULID (row id) |
| `work_ulid` | string? | optional join key; minted at subagent spawn when known |
| `timestamp` | string | RFC3339 UTC millis |
| `turn_type` | string | `"main"` \| `"agent_turn"` |
| `agent_kind` | string | `"main"` or subagent type (`explore`, `general-purpose`, …) |
| `session_id` | string | session id |
| `prompt_id` | string? | current prompt when known |
| `model_id` | string? | assistant model id |
| `input_tokens` | u64? | full prompt (includes cache reads) |
| `output_tokens` | u64? | completion |
| `cached_tokens` | u64? | cache **read** hits only |
| `reasoning_tokens` | u64? | |
| `total_tokens` | u64? | provider total (not always input+output) |
| `cost_usd_ticks` | i64? | 1e10 per USD; omitted when missing/zero |
| `cost_missing` | bool | true when cost absent/zero on this call |
| `incomplete` | bool | true when usage omitted / fail-closed |
| `api_duration_ms` | u64? | |

Token semantics match `TokenUsage` / `UsageLedger` (see explore note
`/tmp/grok-1000/plan-explore-usage-logs.md` or in-tree ledgers).

## Writers

| Session | `turn_type` | `agent_kind` | `work_ulid` |
|---------|-------------|--------------|-------------|
| Main agent | `main` | `main` | omitted unless `StartupHints.work_ulid` set |
| Subagent / task | `agent_turn` | task `subagent_type` (fallback `subagent`) | minted at spawn |

- **With usage:** after ledger fold in `record_response_token_usage`.
- **Without usage + incomplete path** (task budget / sampler-retry-before-output):
  incomplete row with tokens omitted (same identity rules).

Identity is resolved via `SessionActor::usage_jsonl_identity()` from
`startup_hints.is_subagent` + `subagent_type_label()` + `work_ulid`.

## Residual (not D0/D1)

| Item | When |
|------|------|
| Compaction / classifier / side-call typed rows | later |
| Reload ledger from disk on resume | product choice; still process-scoped today |
| SQL views / rollup | consumer |
| Parent-session mirror of child usage rows | optional; child has own `usage.jsonl` |

## Tests

```bash
cargo test -p xai-grok-shell --lib -- usage_log record_response_token_usage
```

Covers serialize format, agent_turn identity, work_ulid, zero-cost
normalization, incomplete omit fields, append JSONL, fail-open unwritable
path, hub main vs subagent rows.
