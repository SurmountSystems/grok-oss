# usage.jsonl identity writes — restore hub append

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch:** `onto-xai/b13fa526f511`
**Prior:** `.agents/reports/impl-upstream-catalog-filters-2026-08-11.md` § C
**Scope:** shell usage logging only (`record_response_token_usage` → `usage.jsonl`)

---

## Goal

Make green (identity contract unchanged):

- `main_usage_jsonl_keeps_main_identity`
- `subagent_usage_jsonl_uses_agent_turn_identity`

## Red (observed)

```
cargo test -p xai-grok-shell --lib -- main_usage_jsonl_keeps_main_identity subagent_usage_jsonl_uses_agent_turn_identity
```

| Test | Fail |
|------|------|
| `main_usage_jsonl_keeps_main_identity` | `usage.jsonl written for main: Os { code: 2, NotFound }` |
| `subagent_usage_jsonl_uses_agent_turn_identity` | `usage.jsonl written for subagent: Os { code: 2, NotFound }` |

Other `record_response_token_usage_*` chat-state tests already passed (ledger fold only).

## Root cause

On this onto hybrid tip, `SessionActor::record_response_token_usage` in
`crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`
updated chat-state / signals / model-call ledger but **never** called
`usage_log::record_model_call` / `record_incomplete`.

`session/usage_log.rs` (schema, `append_usage_record` fail-open +
`create_dir_all`, helpers) was intact. The hub wiring was dropped during
recon; Surmount `main` still had it (`usage_jsonl_identity`,
`append_usage_jsonl`, `append_usage_jsonl_incomplete`).

Catalog suspicion confirmed: not a wrong `session_dir` path — **no write
was attempted**.

## Product fix (minimal)

**File:** `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`

Restored Surmount hub wiring (same shape as pre-onto main):

1. After ledger fold when `response.usage` is `Some`:
   - `append_usage_jsonl(model_id, usage, api_duration_ms, cost_usd_ticks)`
2. On incomplete paths (task output budget closed / sampler retry only before
   output): also `append_usage_jsonl_incomplete(None)`
3. Helpers:
   - `usage_jsonl_identity()` — main → `turn_type`/`agent_kind` = `main`
     (+ optional `work_ulid`); subagent → `agent_turn` +
     `subagent_type_label()` (fallback `subagent`) + `work_ulid`
   - `append_usage_jsonl` / `append_usage_jsonl_incomplete` →
     `persistence::session_dir` + `usage_log::record_model_call` /
     `record_incomplete` (fail-open)

Identity contract not weakened. No test edits. No broader refactor.

## Green

```
cargo test -p xai-grok-shell --lib -- main_usage_jsonl_keeps_main_identity subagent_usage_jsonl_uses_agent_turn_identity
# 2 passed

cargo test -p xai-grok-shell --lib -- usage_log record_response_token_usage
# 17 passed (usage_log unit + record_response_token_usage hub)
```

## Post-impl verify

| Step | Result |
|------|--------|
| `cargo fmt -p xai-grok-shell` | clean |
| `cargo clippy -p xai-grok-shell --lib -- -D warnings` | blocked by **pre-existing** dep lints in `xai-grok-tools` (`ensure_persistent_shell_initialized` dead_code; `Command::spawn` disallowed) — not introduced by this edit |
| Targeted tests | **17/17 green** under `usage_log` + `record_response_token_usage` |

## Out of scope / residual

- Plan five-CTA panel footer, soft interject queue, pager interject/wait
  (catalog A/B/D) — unchanged
- No commit/push; stashes kept

## TDD log

| Phase | Evidence |
|-------|----------|
| **Red** | Both identity tests NotFound before product edit |
| **Green** | Same two tests + full 17-filter after hub restore |
| **Contract** | main: `turn_type`/`agent_kind`=`main`, no `work_ulid`; subagent: `agent_turn` + type + `work_ulid` |
