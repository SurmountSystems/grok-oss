# Join: Phase R — Rate limits by API type

**Date:** 2026-08-03
**Status:** done
**Prior:** billing + Management already wired (`.agents/joins/impl-shared-rate-limit-billing-management-2026-08-03.md`)

## Public docs (re-verified)

| Doc | Link | Accessed |
|-----|------|----------|
| xAI rate limits | https://docs.x.ai/developers/rate-limits | 2026-08-03 |
| xAI models | https://docs.x.ai/developers/models | (cited; RPS table lives on rate-limits page) |
| OpenRouter limits | https://openrouter.ai/docs/api_reference/limits | 2026-08-03 |
| GitHub REST rate limits | https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api | 2026-08-03 |

Policy: **header-driven** wait (`Retry-After`, then `x-ratelimit-reset`); fallback 60s. No hardcoded tier tables.

## Inventory matrix

| Class | Product path | Doc | Wired | ProviderKey approach |
|-------|--------------|-----|-------|----------------------|
| Chat / inference (xAI, SuperGrok proxy, OpenRouter, BYOK base URL) | `xai-grok-sampler` `request_task.rs` | xAI / OpenRouter | **yes** (kept) | host + key fingerprint (no class; stable filenames) |
| SuperGrok billing | shell `extensions/billing.rs` | (proxy) | **yes** (kept) | host + session fingerprint |
| Management API | shell `auth/xai_management.rs` | Management host | **yes** (kept) | host + management-key fingerprint |
| GitHub OSS update | `xai-grok-update` `oss_update.rs` | GitHub REST | **yes** (kept) | logical `keys::GITHUB` |
| Imagine image gen | tools `image_gen` | xAI Imagine RPS 5 | **yes (new)** | host + fingerprint + `imagine` |
| Imagine image edit | tools `image_edit` | same bucket | **yes (new)** | same Imagine key |
| Imagine video | tools `video_gen` start + poll | xAI video RPS 10 | **yes (new)** | host + fingerprint + `video` |
| Voice STT | `xai-grok-voice` WS connect | xAI Voice (separate; sales for increases) | **yes (new)** | host + fingerprint + `voice` |
| Responses / web_search | tools `web_search` client | xAI / OpenRouter | **yes (new)** | host + fingerprint + `responses` |
| Web fetch (arbitrary URLs) | tools `web_fetch` | N/A (user content) | no | not a product API rate-limit surface |
| Durable poll history | shell | N/A | out of scope | not rate limit |

Operator correction applied: Imagine / voice / BYOK / public API hosts are **not** intentional edges that skip shared limits.

## What shipped

### `grok-rate-limit`

- `api_class::{IMAGINE, VIDEO, VOICE, RESPONSES}`
- `ProviderKey::from_base_url_fingerprint_and_class`
- `http` module: `DEFAULT_RATE_LIMIT_WAIT`, `wait_from_header_values`, `should_observe_status`, `observe_status` (429 always; 403 only with retry hint)
- Crate docs cite public rate-limit pages with accessed dates

### `xai-grok-tools`

- New `shared_http_rate_limit` module (mirrors shell helper; test store override)
- Wired: `image_gen`, `image_edit`, `video_gen` (start + poll), `web_search` (`search` + `search_with_titles`)
- Effective bearer for keys: dynamic provider, else static config key (`rate_limit_bearer`)

### `xai-grok-voice`

- Wait before STT WebSocket connect; observe on HTTP 429/403-with-hint from handshake failure

### Docs

- `FORK.md` § Multi-session rate limits expanded with class table + citations
- User-guide `11-custom-models.md` § Multi-process shared rate limits

## Tests (green)

```bash
cargo test -p grok-rate-limit
# 15 passed

cargo test -p xai-grok-tools --lib shared_http_rate_limit
# 5 passed

cargo test -p xai-grok-tools --lib imagine_429_observes
# imagine_429_observes_shared_rate_limit_store ok

cargo test -p xai-grok-voice --lib voice_provider
# voice_provider_key_uses_class_and_fingerprint ok

cargo test -p xai-grok-shell --lib shared_http_rate_limit
# 9 passed (pre-existing billing/Management helpers still green)

cargo test -p xai-grok-tools --lib static_api_key_is_fallback
# ok (web_search path still works with wait)
```

Named contracts:

1. API class separates Imagine / video / voice / responses from chat host+fp keys.
2. Hermetic Imagine 429 + `Retry-After` publishes remaining a peer store handle sees.
3. Bare 403 without retry hint does not observe.
4. Secrets never appear in provider key strings / filenames.
5. Kill switch `GROK_DISABLE_SHARED_RATE_LIMIT` still honored by the store.

## Out of scope (as requested)

- Free SuperGrok period debit invention
- Plan approval Phase P / residual AGENTS novels
- Durable poll history redesign
- Rewriting already-wired sampler / billing / Management / GitHub paths

## Acceptance

- [x] Inventory matrix in join
- [x] Missing product 429 paths wait + observe with type-appropriate keys
- [x] Reuse `SharedRateLimitStore` / helpers; extend keys/api_class
- [x] Red→green hermetic tests per new class (Imagine full HTTP; key/unit for others)
- [x] FORK + user-guide with markdown links + accessed dates
- [x] Code comments cite docs with accessed dates
- [x] `cargo fmt -p grok-rate-limit -p xai-grok-tools -p xai-grok-voice`
