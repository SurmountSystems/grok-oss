# Plan: OAuth SuperGrok ↔ console API key failover + secure key store

## Context

Operator setup (clarified 2026-07-26):

1. **Consumer SuperGrok** via regular xAI / X account OAuth (`grok login`,
   session in `auth.json`).
2. **Business / console API key** for a second billing path.
3. Needs **graceful bidirectional failover** on credit/quota exhaustion
   (OAuth → key **and** key → OAuth), plus **secure storage** of the console
   key (not a tracked env file).

**Already in product:**

- BYOK multi-**key** lists + mid-request rotate on **credit/quota** only
- OpenRouter **keyring** store
- Pure 429 retries same credential (per-fingerprint cooldowns)

**Not in product (blocker for this setup):**

- Resolve never pairs session JWT with a console key as failover
- Session path sets `failover_api_keys = []`
- Rotate is **string-only** (key→key); clears `bearer_resolver`; cannot hop
  to/from OAuth session
- xAI console keys not first-class in keyring (often env / plaintext auth scope)

Explore: `/tmp/grok-1000/grok-explore-oauth-api-key-failover.md`  
Research: `doc/dev/research/secure-key-failover-2026-07-26.md`

Non-goals:

- Dual **OAuth** personal SuperGrok accounts as silent multi-login (separate S3)
- Soft-429 hop by default (double-spend risk)
- age/sops vault as the product path
- Encouraging ToS-dodging “two personal accounts”

## Approach

### D1 — Resolve merge (primary + failover across kinds) — **core**

When both OAuth session and console API key are available:

- Build primary + ordered failover **identities**, not mutually exclusive branches.
- Config (recommended default: **session first**, console key failover — matches
  “daily SuperGrok, Business when caps hit”; allow `prefer = api_key` reverse).
- Stop emptying failover solely because primary is session.
- Keep enterprise kill-switch: `disable_api_key_auth` / preferred OIDC-only
  still blocks or clears key failover.

Primary symbols: `agent/config.rs` `resolve_credentials`,
`collect_own_credentials`, `enforce_disable_api_key_auth`.

### D2 — Rotate identity mode (not string-only) — **core**

On `is_credit_exhausted`:

| Hop | Action |
|-----|--------|
| Session → API key | Set `api_key` to console key; **clear** `bearer_resolver` (existing pattern) |
| API key → session | Reinstall `AuthManagerBearerResolver`; ensure fresh session token; use JWT as active credential |
| Key → key | Keep existing pop-from-list |

Track active kind for logs / rate-limit fingerprint. Rebuild `SamplingClient`.
Credit-only trigger (unchanged).

Primary: `sampler/.../request_task.rs` `try_rotate_to_failover_key` +
`sampler_turn` reconstruct / bearer wiring.

### D3 — Exhausted-identity memo + UX

- Process-local (optional `$GROK_HOME`) “fingerprint F exhausted until T”
- Toast / status: “Switched SuperGrok session → console key” (and reverse)
- Docs in user-guide auth + custom-models credit failover

### S1 — Secure console key store (still needed)

- Mirror OpenRouter: xAI API keys in keyring service `grok-build` + 0600
  `provider_credentials.json` fallback
- `grok login --api-key` multi-add; list fingerprints only
- Env wins when set; no store write that fights env (OpenRouter parity)
- **Does not** replace D1/D2 — storage alone cannot failover OAuth↔key

### S3 — Dual OAuth SuperGrok (out of scope for this operator path)

Only if a second **browser login** identity is required later. Not needed if
one side is console key.

## Critical files

| Path | Why |
|------|-----|
| `xai-chat-state/.../types.rs` | `Credentials`, `AuthType`, failover list shape |
| `xai-grok-shell/src/agent/config.rs` | resolve merge / kill-switch clear |
| `xai-grok-shell/.../acp_session_impl/sampler_turn.rs` | bearer_resolver + reconstruct |
| `xai-grok-sampler/.../request_task.rs` | rotate + credit path |
| `xai-grok-sampling-types/.../error.rs` | `is_credit_exhausted` |
| `xai-grok-shell/src/auth/credentials_store.rs` | S1 keyring |
| `xai-grok-shell/src/auth/manager*.rs` | session refresh on hop-to-session |
| user-guide `02-authentication`, `11-custom-models` | operator honesty |

## Steps (implementation order)

1. **Red tests:** session primary + key failover present; credit hop
   session→key and key→session; bare 429 no hop; kill-switch still clears key.
2. **D1** resolve merge + config pin (prefer session | prefer key).
3. **D2** rotate with bearer install/clear.
4. **D3** toast + exhausted memo + docs.
5. **S1** keyring for console key (can parallel after D1 design frozen).

## Risks

| Risk | Mitigation |
|------|------------|
| Bearer re-inject undoes hop | Hop-to-key must clear resolver (already); hop-to-session only after memo |
| Refresh races mid-turn | Single-flight AuthManager; no dual parallel refresh of same family |
| Double-spend on soft 429 | Credit-only rotate (keep) |
| Enterprise single-identity | Kill-switch keeps clear/block |
| Wrong host | First-party xAI only; never session on OpenRouter |
| ToS / product framing | Explicit dual-auth config; no “evade limits” marketing |

## Verification

```bash
cargo test -p xai-grok-sampler --lib -- failover credit rotate
cargo test -p xai-grok-shell --lib -- resolve_credentials failover session api_key
cargo test -p xai-grok-sampling-types --lib -- credit_exhausted
```

Manual: `grok login` (consumer) + stored console key, no env; exhaust SuperGrok
quota → console key serves request; exhaust key → session again when restored;
soft 429 waits; restart still has keyring key.

## Open questions (remaining)

1. Default prefer: **session-first** vs **key-first**?  
   *Recommend:* session-first (consumer SuperGrok primary).
2. Soft 429 hop?  
   *Recommend:* no by default.
3. Should hop persist as “active identity” for next turns, or only mid-request?  
   *Recommend:* mid-request + process memo until credit recovers / restart.

## Research

`doc/dev/research/secure-key-failover-2026-07-26.md`  
Explore joins:  
`/tmp/grok-1000/grok-explore-api-key-failover.md`  
`/tmp/grok-1000/grok-explore-oauth-api-key-failover.md`
