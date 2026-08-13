# Console bypass paths — code audit (2026-08-02)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Mode:** read-only product code audit. No secrets. No product edits.
**Trigger:** Operator saw console team API Usage **$547.87 → $547.88** in one dogfood turn while SuperGrok still had included weekly headroom (~65% used / live SuperGrok path in prior joins).

**Related joins:**
`live-auth-path-now-2026-08-02.md`, `console-api-usage-547-evidence-2026-08-02.md`, plan `limits-first-ideal-2026-08-02.md`.

---

## Executive answer

| Question | Answer |
|----------|--------|
| Does Design A fully block console inference while SuperGrok included has headroom? | **Only when** `[auth] auto_use_included_limits = true` **and** `preferred_method` is not `api_key`, **and** resolve goes through `resolve_credentials_preferring_with_rank` / `order_credentials_for_preferred_auto`. **Default is flag off** (`auto_use_included_limits: false`). |
| Default dual-auth without the flag? | Session primary, **console keys always in `failover_api_keys`** → plain **429** and credit hop can burn console `$` on `api.x.ai` even with SuperGrok included remaining. |
| Can this product move console Usage while live UI says SuperGrok? | **Yes** (mid-turn hop, sticky exhaust, pin, tools forced to public API with console bearer, other clients on same team key). Live `console.isLive: false` does **not** prove zero console spend for that turn or team-wide. |
| $0.01 dogfood bump attribution? | **Not proven** from code alone. Most plausible **in-product** sources for cents-scale: rate-limit hop of a small request, title/aux side-query after hop, image/voice on public API with console key, memory embed if primary is console. **Team-wide** key shared with other machines/clients remains open (F5). |

---

## Design A: `order_credentials_for_preferred_auto` — coverage

### What it does

File: `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs`

| Condition | Chain |
|-----------|--------|
| Any SuperGrok candidate with `included_remaining > 0` | Primary = ranked SuperGrok JWT; **failover = other SuperGrok only**; **console keys omitted** (`primary_is_supergrok_included = true`) |
| No live SuperGrok included headroom | Console keys lead; SuperGrok not primary |
| `preferred_method = api_key` | Rank path **not used** (`preferred_uses_supergrok_auto_rank` false) — console-first dual-auth |
| `auto_use_included_limits = false` (default) | Rank path **not used** — falls through to `resolve_credentials_preferring_inner` |

Named test: `auto_order_omits_console_while_any_supergrok_included_headroom` (~L834–877).

Wire-up: `resolve_credentials_preferring_with_supergrok_sessions` in `agent/config.rs` ~L5097–5179; called only when `preferred_uses_supergrok_auto_rank(auto_use_included_limits, preferred)`.

### What it does **not** cover

1. **Flag default off** — `GrokComConfig.auto_use_included_limits` default `false` (`auth/config.rs` ~L132–139). Without operator/config enabling it, Design A never runs.
2. **`preferred_method = api_key`** — intentional console pin; SuperGrok last or exclusive key.
3. **No SuperGrok session candidates** (empty auth.json OIDC, expired-only after hard-expire filter) — rank empty → console or session-only inner path.
4. **BYOK / model `api_key` / `env_key` / auth_provider** — own credentials short-circuit before dual-auth merge (`resolve_credentials_preferring_inner` ~L5205+).
5. **OpenRouter / non-first-party hosts** — no xAI console dual-auth merge.
6. **`ModelsManager::sampling_config()`** — still calls bare `resolve_credentials` (no preferred, no rank) at `agent/models.rs` ~L949–951. Used at agent construct (`agent_ops.rs` ~L1595). Turn path usually rebuilds via `prepare_sampling_config_for_model` (rank-aware) + `reconstruct_full_config`, but **startup / any consumer of that helper** can reintroduce console failover.
7. **Subagent override when `load_effective_config()` fails** — falls back to `resolve_credentials` without rank (`subagent/mod.rs` ~L852–862).
8. **`resolve_model_to_sampling_config`** — bare `resolve_credentials` (`config.rs` ~L5945); currently appears unused outside sampler docs/tests, still a landmine if re-wired.
9. **Image / video / voice** — force **public** `xai_api_base_url` / `api.x.ai` (or STT WS), independent of Design A chain shape; bearer follows live sampling or AuthManager (see inventory).
10. **Sticky credit exhaust** — `prefer_live_identity_after_credit_exhaust` can promote console **before HTTP** if SuperGrok fingerprint is memoized exhausted, even when billing UI still shows included headroom (false memo / wrong identity). Called from `prepare_sampling_config_for_model` and `reconstruct_full_config`.
11. **Other processes on the same team console key** — not a code path; cannot be killed by Design A (F5).

### Default dual-auth (flag off) — the big gap

`resolve_credentials_preferring_inner` session+console first-party branch (`config.rs` ~L5325–5344):

- Primary: SuperGrok session + session host (cli-chat-proxy).
- Failover: **all console keys** + `failover_base_url` → `https://api.x.ai/v1` when hosts split (`XAI_API_BASE_URL_DEFAULT`).
- Sampler hop on **402 / credit** and **plain 429** (`prefer_live_primary.rs`) switches host + key → **console Usage**.

This matches user-guide text: default order is session first then console; Design A strip is **optional** via `auto_use_included_limits`.

---

## Inventory table

**Legend (can hit console while SuperGrok included headroom):**
**Y** = yes in common dual-auth setups · **Y\*** = only if flag off / pin / hop / wrong memo · **N** = not under correct SuperGrok primary · **?** = billing attribution unclear (OAuth on public API vs team API key)

**Likelihood for $0.01 one-turn bump:** High / Med / Low / Negligible / Open (team-wide)

| Path | Can hit console `$` while SuperGrok headroom | Evidence | Likelihood $0.01 dogfood turn |
|------|-----------------------------------------------|----------|-------------------------------|
| **Main chat sampling (flag OFF, dual-auth)** | **Y\*** — console in failover; 429/credit hop → `api.x.ai` | `config.rs` L5325–5344; `prefer_live_primary.rs` L122–209 | **Med–High** if any 429/hop; Low if no hop that turn |
| **Main chat (flag ON + headroom)** | **N** for chain strip; **Y\*** if sticky exhaust memo false-positive | `supergrok_identity_rank.rs` L361–388; Design A test L834+; `prefer_live_primary.rs` L263+ | **Low** if flag on and memo clean (matches live `console.isLive: false`) |
| **Main chat `preferred_method=api_key`** | **Y** — console primary by design | `config.rs` L5288–5323; `auth/config.rs` preferred pin | **High** if pinned (operator/config) |
| **Subagent inherit parent** | Same as parent chain | `subagent/mod.rs` L629–636 inherit | Same as main |
| **Subagent model override** | Rank-aware when config loads; **Y\*** bare resolve if config load fails | `subagent/mod.rs` L849–862 | **Low** (inherit common); Med if override + flag off |
| **Title / session summary / aux models** | Same dual-auth as main when `resolve_aux_*_preferring` used | `agent_ops.rs` L64–116; `config.rs` L5651+; `sampler_turn.rs` L704–726 | **Med** small side-query after hop; Low if SuperGrok-only chain |
| **Web search tool sampling** | Same rank-aware resolve | `agent_ops.rs` L1492–1531; `config.rs` L6045+ | **Med** if tool fires + hop/flag-off |
| **Image gen / image edit** | **?**/**Y** — always `endpoints.xai_api_base_url` (default `https://api.x.ai/v1`); bearer = **current** `sampling_config.api_key` | `agent_ops.rs` L1400–1439; CHANGELOG “use api.x.ai directly”; UA `xai-grok-build/{version}` L1423 | **Med** if console primary/hop; **?** if SuperGrok JWT on public Imagine (server may bill SuperGrok not team Usage) |
| **Video gen** | Same as image | `agent_ops.rs` L1448+ | Same as image |
| **Voice STT** | **?**/**Y** — `api.x.ai` / `wss://api.x.ai/v1/stt`; bearer from AuthManager or `XAI_API_KEY` | `pager/src/voice/auth.rs` L1–45; `xai-grok-voice` defaults | **Low–Med** (Usage chart Voice was &lt;$0.01 historically) |
| **Memory embeddings** | **N** if SuperGrok primary (uses sampling `base_url` + key → proxy) · **Y** if console primary | `spawn.rs` L390–391, L770–787; `embedding_session_credentials` scopes session to first-party HTTPS | **Low** on SuperGrok live path |
| **Compaction / auto-summary** | Uses `reconstruct_full_config` (chat-state creds + sticky exhaust) | `compaction.rs` L176, L1017; `sampler_turn.rs` L398–560 | **Med** if failover still has console |
| **Auto-mode permission classifier** | Aux resolve + stamp from session | `sampler_turn.rs` L562+ | **Low–Med** |
| **Models catalog `/v1/models`** | Can use session or API-key fetch auth; list URL often proxy, not inference spend | `agent/models.rs` catalog fetch | **Negligible** for $ Usage (metadata) |
| **OpenRouter / custom non-xAI** | **N** for xAI console keys (explicitly blocked) | `config.rs` L5217–5232; openrouter tests | None for console team Usage |
| **Env `XAI_API_KEY` injection** | **Y** — first source in `collect_xai_console_api_keys` | `config.rs` L4972–4998; `auth_method.rs` L26–38 | Enables all dual-auth console paths; not a request by itself |
| **Keyring / `auth.json` console keys** | Same collect order after env | `xai_console.rs` store URL `https://api.x.ai/v1`; `config.rs` L4984–4997 | Same |
| **Hardcoded `XAI_API_BASE_URL_DEFAULT`** | Console hop host when session on cli-chat-proxy | `config.rs` L51, L5279–5280, L5132–5133 | Used on hop only |
| **`GROK_XAI_API_BASE_URL` env** | Overrides public API base for console/Imagine | `EndpointsConfig::default` L595–596 | Redirects console host if set |
| **Product “Grok Build” Usage line** | **Not a separate spend path** — server-side product bucket for traffic tagged as Build client | Imagine/video: `user-agent: xai-grok-build/...` + `x-grok-client-identifier` (`agent_ops.rs` L1423; tests L2653–6685); sampler `x-grok-*` headers (`sampler/client.rs`); storage docs list `grok-pager` / `grok-shell` | **Does not alone force console `$`**. SuperGrok proxy traffic should **not** appear on console.x.ai team API Usage. Console chart “Grok Build $214” means **console-metered** traffic attributed to Build product (API key / public API with team metering), not “any SuperGrok chat.” |
| **Other clients / machines same team key** | **Y** (external) | Evidence join F5; Usage is team-wide | **Open** for both $547 and $0.01 |

---

## Deep notes by focus area

### 1. Subagent sampling

- Inherit parent: no re-resolve; copies parent `SamplerConfig` (already dual-auth shaped).
- Override / definition model: `resolve_credentials_preferring_with_rank` with live `preferred_method` + `auto_use_included_limits` (Design A when flag on).
- Gap: config load failure → bare `resolve_credentials` → console in failover always.

### 2. Title / recaps / aux

- `build_summary_client` and `resolve_aux_sampler_config` pass preferred + auto_use into `resolve_aux_model_sampling_config_preferring`.
- Aux fallback path can build a synthetic model with `api_key: Some(bearer)` where bearer is session **or** `XAI_API_KEY` env **or** deployment key (`config.rs` L5690–5736). Env key on that path is a **console key** if session missing.
- Title generation uses session summary model (small token spend); on SuperGrok-only chain it should hit proxy, not console Usage.

### 3. Embeddings

- Bound to **sampling** base_url/key at session spawn (`spawn.rs` L390–391), not forced to public API.
- Session credentials only attach when `is_xai_api_bearer_url` (HTTPS first-party).

### 4. Image / voice / batch

- Image/video: **always** public API base; not Design A’s host. Credentials follow whoever is live primary (or sticky console).
- Voice: public STT; AuthManager bearer (OAuth or key). Comment: OAuth bills per-user; API key bills key path.
- No separate xAI **batch inference** product path found in shell/tools for dogfood chat (memory “batch” = embed chunking only).

### 5. Hardcoded / env hosts

| Constant / env | Role |
|----------------|------|
| `XAI_API_BASE_URL_DEFAULT` = `https://api.x.ai/v1` | Console hop + default public API |
| `GROK_XAI_API_BASE_URL` | Override public API base |
| `XAI_CONSOLE_API_URL` | Keyring URL for console keys |
| `GROK_CLI_CHAT_PROXY_BASE_URL` / default proxy | SuperGrok session inference |
| `management-api.x.ai` | Prepaid/management only — **not** inference Usage |

### 6. preferred_method pin

| Value | Effect while included headroom |
|-------|--------------------------------|
| unset / oidc / oauth | Session-first; console failover unless Design A strip |
| `api_key` | Console primary (burns console even with SuperGrok headroom) |

### 7. OpenRouter / custom models

- OpenRouter: never xAI session or `XAI_API_KEY` dual-auth.
- Custom first-party model with own `api_key` on `api.x.ai`: **always** that key (BYOK), outside Design A ranking.

### 8. “Grok Build” product header vs console Usage

- Product **does** set Build-oriented identity on **direct public API** tool traffic (`user-agent: xai-grok-build/...`, `x-grok-client-identifier`).
- Sampler uses `x-grok-client-identifier`, conv/session headers on chat.
- Console Usage categories (Text / Grok Build / Image & Video / Voice) are **server-side** buckets for **console-metered** traffic. Headers can move spend **into** the Grok Build **line** when the request is already on the console bill; they do **not** mean SuperGrok proxy traffic is re-labeled onto console Usage.
- Therefore: **$214 “Grok Build” on the $547 chart is console-side Build-attributed spend**, consistent with this product (or another Build client) using a **team API key** / public API path — not proof that SuperGrok primary chat was mis-billed as console.

---

## Most likely code explanation for $0.01 one-turn move

Ordered by plausibility given prior live capture (SuperGrok primary, console not live, key available, ~65% included):

1. **Team-wide Usage noise / other process** on same console key (not this turn’s primary path). Code cannot disprove.
2. **Silent 429 hop** (flag off **or** pre-Design-A binary **or** failover list still had console) on a **small** request (tool/aux/title) → cents on console.
3. **Sticky exhaust memo** promoted console for one request despite UI headroom (identity fingerprint bug class).
4. **Imagine/voice** with console bearer (if primary was console or key-only). Historical Image+Voice on chart is tiny.
5. **Not** full main-turn SuperGrok proxy traffic (that should not appear on console team API Usage).

$547 multi-day total remains multi-factor (historical hops, flag-off dogfood, other clients, older builds without Design A).

---

## Recommended kill list for implementer (ordered)

1. **Default `auto_use_included_limits = true`** (or product-level limits-first default for dual SuperGrok+console installs) so Design A is not opt-in.
   - Acceptance: dual-auth + SuperGrok headroom ⇒ `failover_api_keys` empty of console keys without requiring TOML flag.
   - Tests: extend existing Design A resolve tests; default-config integration.

2. **Close bare `resolve_credentials` landmines**
   - `ModelsManager::sampling_config` → `resolve_credentials_preferring_with_rank` with live preferred + auto_use.
   - Subagent fallback when config missing: same rank path (or fail closed), never bare dual-auth with console failover.
   - Deprecate/redirect `resolve_model_to_sampling_config` bare resolve.

3. **Rate-limit hop policy under included headroom**
   - Even if flag somehow off: do not hop SuperGrok→console on plain 429 while any SuperGrok included remaining (cooldown/retry same identity, or hop only to other SuperGrok).
   - Today Design A removes console from list when flag on; flag-off still burns console on 429.

4. **Hard pin audit / honesty**
   - If `preferred_method=api_key`, surface loud live status (“console primary by config”) and do not claim limits-first.
   - Optional: refuse dual-auth console pin while included headroom unless explicit second confirm (product decision).

5. **Imagine / video / voice billing path**
   - Decide: SuperGrok JWT on public Imagine should not debit console team Usage (verify with xAI), **or** keep Imagine on SuperGrok-metered host when session primary.
   - If console key is live primary, image traffic **will** hit console Usage under Grok Build / Image lines — document.

6. **Sticky exhaust false positives**
   - Guard `prefer_live_identity_after_credit_exhaust` with live billing included % when available (do not promote console if included &lt; 100% and flag on).
   - Log non-secret hop reason + live `included` reading for dogfood.

7. **Observability for C3**
   - Per-request log line: primary host class (proxy vs `api.x.ai`), auth type, failover count, whether console key present in chain, hop yes/no.
   - Correlate operator Usage bump with same-second hop logs.

8. **Do not invent kill for external F5**
   - Team key shared across machines: rotate/remove `XAI_API_KEY` from shells that should be SuperGrok-only; inventory other clients. Out of product scope except docs/doctor.

9. **ExhaustedAll → SuperGrok $ extras before console** (ideal C5; separate from headroom bypass)
   - After included 100%, prefer session extras before console primary (plan F3). Not the $0.01-with-headroom bug, but same kill wave.

---

## File index (absolute)

| Area | Path |
|------|------|
| Design A order | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` |
| Credential resolve | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/agent/config.rs` |
| Auth flag defaults | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/auth/config.rs` |
| Console key collect | `collect_xai_console_api_keys` in `agent/config.rs`; `auth/xai_console.rs` |
| Hop / sticky | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-sampler/src/prefer_live_primary.rs` |
| Main prepare | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs` |
| Turn reconstruct | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` |
| Subagent | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/agent/subagent/mod.rs` |
| Models bare resolve | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-shell/src/agent/models.rs` |
| Image/video prepare | `agent_ops.rs` `prepare_image_gen_config` / `prepare_video_gen_config` |
| Voice | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/voice/auth.rs` |
| User-guide dual-auth | `/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md` |

---

## Non-claims

- Does not prove the $0.01 was this binary.
- Does not prove SuperGrok included debit (F1a).
- Does not claim Management prepaid SPEND equals API Usage.
- Docs in tree can lag; resolve path + tests above are the evidence for Design A scope.
