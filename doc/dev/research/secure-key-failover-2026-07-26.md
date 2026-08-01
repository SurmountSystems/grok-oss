# Dual subscription failover + secure API keys

Date: 2026-07-26 · status: **D1+D2+D3+S1 + live re-bind + multi-add CLI shipped** (optional `$GROK_HOME` durable memo still open; dual OAuth S3 out of scope)  
Class: D2 research + operator honesty.

## Intent (operator-clarified)

1. **Consumer SuperGrok** via regular xAI / X account **OAuth** (`grok login`).
2. **Console / Business API key** as the second billing path.
3. **Graceful bidirectional failover** on credit/quota limits
   (session ↔ key, both directions).
4. **Secure storage** of the console key (not a tracked env file).

Earlier assumption “two console API keys” is **wrong for this operator** — one
side is OAuth-only SuperGrok.

## What already works (today)

| Capability | Status |
|------------|--------|
| Multi-key **BYOK** list (key→key) | Yes — comma `XAI_API_KEY`, multi `env_key`, OpenRouter |
| Failover mid-request | Yes — **credit/quota** only (`402`, credit-worded 403/429/400) |
| Pure **rate-limit 429** | Retries **same** credential; does **not** hop |
| Secure store (OpenRouter) | keyring `grok-build` + `provider_credentials.json` (0600) |
| OAuth session + console key together | **No** — resolve is exclusive; session wins and leaves empty failover |
| Session → key or key → session hop | **No** — rotate is string-only; clears bearer_resolver |
| Dual SuperGrok OAuth accounts | **No** |

Docs already: user-guide `11-custom-models.md` § *Credit failover*,
`02-authentication.md` (single-identity story).

### Why comma keys do not solve this

```bash
export XAI_API_KEY='console-key'   # alone → no SuperGrok session in failover
# with grok login active, session usually wins and env key is never queued
```

Session path always sets `failover_api_keys = []`. Kill-switch
`disable_api_key_auth` **clears** failover on purpose.

Explore (mixed modes): `/tmp/grok-1000/grok-explore-oauth-api-key-failover.md`  
Explore (multi-key BYOK): `/tmp/grok-1000/grok-explore-api-key-failover.md`

## Gaps for this setup

| Wishlist | Gap |
|----------|-----|
| Session primary + console key failover | Resolve merge (D1) |
| Console key primary + session failover | Same + rotate reinstalls bearer (D2) |
| Mid-request hop both ways | Rotate identity mode (D2) |
| Console key not in env file | Keyring for xAI keys (S1) |
| Toast / exhausted memo | D3 polish — **shipped** (process-local) |
| AuthManager live re-bind without prior stash | **Shipped** — `session_bearer_resolver` durable + hop-to-session live re-bind |
| Multi-add `grok login --api-key` + list fingerprints | **Shipped** |
| Second OAuth SuperGrok login | S3 — **not** this operator path |

**S1 keyring alone is insufficient** — storage does not fix exclusive resolve
or string-only rotate.

## Recommended slices

| Slice | What | Effort |
|-------|------|--------|
| **D1** | Resolve merge: session + console key as primary/failover (config prefer) | M |
| **D2** | Rotate toggles `bearer_resolver` / JWT vs static key | M |
| **D3** | Exhausted-identity memo + toast + docs | S — **shipped** (process-local 1h memo + hop status/toast) |
| **S1** | xAI console key in keyring (OpenRouter pattern) | S–M |
| **S3** | Dual OAuth SuperGrok | L–XL + ToS — out of scope here |

**Pragmatic ship order:** D1 → D2 → D3, S1 in parallel once D1 config is fixed.

## Risks

- **Bearer re-inject** undoing hop-to-key if resolver not cleared
- **Refresh races** when hopping to session mid-turn
- **Double-spend** if soft 429 hops (keep credit-only)
- **Enterprise** single-identity kill-switch must keep working
- **ToS / framing:** explicit dual-auth config; legitimate Business + personal
  SuperGrok — not marketed as limit evasion

## Plan

[`.agents/plans/plan-secure-key-failover.md`](../../../.agents/plans/plan-secure-key-failover.md)

## Defaults to confirm at implement

| Question | Recommend |
|----------|-----------|
| Prefer which primary? | **Session first** (consumer SuperGrok) |
| Soft 429 hop? | **No** |
| Hop sticky across turns? | Mid-request + process memo until recover/restart |
