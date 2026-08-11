# Join: Bare resolve / console-edge audit with TDD

**Date:** 2026-08-02
**Priority:** 2 (limits-first residual)
**Also:** `/tmp/grok-1000/grok-impl-summary-bare-resolve-audit.md`
**Review:** `/tmp/grok-1000/grok-review-limits-residual-edges.md` (Issues 2, 3, 6 docs, 8 fixed)

## Outcome

Closed real bare-`resolve_credentials` landmines that could put console
`ApiKey` / `api.x.ai` in primary/failover while SuperGrok included still had
headroom, even when `[auth] auto_use_included_limits = true`.

Design A + after-burner order logic unchanged (Slice 4). This pass wires
**call sites** to `resolve_credentials_preferring_with_rank` with live
`preferred_method` + `auto_use_included_limits`, and fail-closes subagent
override when config is missing.

**Did not** flip default `auto_use_included_limits` (operator-gated).

## Fixes (product)

| Site | Before | After |
|------|--------|--------|
| `ModelsManager::sampling_config` | bare `resolve_credentials` | with_rank + cfg preferred/auto_use |
| Subagent model override | rank if config ok; bare or auto_use false on miss | Prefer agent_config → disk → **fail closed** (session live ⇒ auto rank); inherit parent SuperGrok-session-only |
| `resolve_model_to_sampling_config` | bare resolve landmine | with_rank; preferred + auto_use args |

Helpers: `subagent_override_auth_rank_flags`, `parent_sampling_is_supergrok_session_only`.

## RED → GREEN (honest)

### Wave 1 (initial landmines)

| Test | RED evidence | GREEN |
|------|--------------|-------|
| `sampling_config_auto_use_omits_console_while_supergrok_included_headroom` | Observed fail: bare path primary/failover had console (`console-bare-mm-key`) | with_rank omits console |
| `resolve_model_override_agent_config_auto_use_omits_console` | Would fail without agent_config-first rank | omits console under auto_use |
| `resolve_model_to_sampling_config_auto_use_omits_console_while_included_headroom` | Bare helper queued console | with_rank omits console |

### Wave 2 (review Issues 2, 3)

| Test | RED evidence | GREEN |
|------|--------------|-------|
| `subagent_override_auth_rank_flags_fail_closed_when_config_missing_and_session_live` | Pure: old `unwrap_or((None, false))` ⇒ `(None, false)` with session live | `(None, true)` fail closed; parent SuperGrok-only forces auto on disk false |
| `resolve_model_override_config_missing_parent_supergrok_only_omits_console` | Integration would re-queue console with auto_use off | omits console when parent SuperGrok-session-only |
| `sampling_config_api_key_pin_keeps_console_primary` | Call-site gap (resolve-level pin already existed) | ModelsManager pin |
| `resolve_model_override_api_key_pin_keeps_console_primary` | Call-site gap | subagent pin despite auto_use |

**Note:** Host `GROK_HOME` OnceLock can rank a real SuperGrok JWT over fixture tokens. Omit-console asserts check **console key not primary/failover**, not exact fixture JWT equality.

```bash
cargo test -p xai-grok-shell --lib -- \
  subagent_override_auth_rank_flags_fail_closed \
  resolve_model_override_config_missing_parent_supergrok_only \
  resolve_model_override_api_key_pin \
  resolve_model_override_agent_config_auto_use \
  sampling_config_auto_use_omits_console \
  sampling_config_api_key_pin \
  resolve_model_to_sampling_config_auto_use
# 7 passed
```

`cargo fmt -p xai-grok-shell` applied.

## Intentional bypasses (not fixed; documented)

| Path | File:line (approx) | Why intentional / out of scope |
|------|--------------------|--------------------------------|
| Image gen / image edit | `agent_ops.rs` `prepare_image_gen_config` ~L1400–1439 | Always public Imagine host; bearer = live sampling primary |
| Video gen | `agent_ops.rs` `prepare_video_gen_config` ~L1448+ | Same |
| Voice STT | `pager/src/voice/auth.rs` L1–45 | Public STT; AuthManager bearer |
| BYOK / env_key / auth_provider | resolve own-credentials short-circuit | Outside dual-auth rank |
| OpenRouter / non-first-party | same | No xAI console merge |
| `preferred_method = api_key` | pin path | Console primary by design (call-site tests added) |
| Flag default off | `auth/config.rs` | Operator-gated; separate residual |
| Env `XAI_API_KEY` | collect console keys | Enables dual-auth identity only |
| Embeddings | sampling base_url/key | Console only if sampling primary is console |

## Prepaid / after-burner

No path found that reorders credentials **ignoring** `prepaid_balance_cents` after included full once auto rank is used. Bare sites go through the same rank that honors extras.

## Files

- `crates/codegen/xai-grok-shell/src/agent/models.rs`
- `crates/codegen/xai-grok-shell/src/agent/subagent/mod.rs`
- `crates/codegen/xai-grok-shell/src/agent/subagent/tests/mod.rs`
- `crates/codegen/xai-grok-shell/src/agent/config.rs`

## Not done

- Default `auto_use_included_limits=true` for new installs (operator-gated residual)
- Imagine / voice host migration off public API
- Sticky exhaust memo vs live included % guard
- Team-wide other clients on same console key
- Honesty Issues 1, 4, 5, 7 (other implementer)
