# Recon: shared rate limits + multi-process awareness for API calls

**Date:** 2026-08-03
**Mode:** read-only explore
**Scope:** where “pooled / shared rate limiting” lives, what is already cross-process, what is process-local (especially Item 4 flat-poll history), gaps vs “any API calls multi-process aware through IPC.”
**Not this feature:** multi SuperGrok OIDC / dual principal identity store (already shipped; separate).

---

## 1. Shared rate limiting (what people mean by “pooled”)

There is **no** symbol named `pooled` / `RateLimitPool`. Product name is **shared rate limits** / crate **`grok-rate-limit`**.

| Item | Location / fact |
|------|-----------------|
| Crate | `/home/hunter/Projects/surmount/grok-build/crates/codegen/grok-rate-limit/` (`src/lib.rs`, `src/store.rs`) |
| Workspace | root `Cargo.toml` members + `grok-rate-limit = { path = ... }` |
| On-disk pool | `$GROK_HOME/rate_limits/{provider_key}.json` (default home `~/.grok`) |
| Mechanism | **`fs2` exclusive flock** + JSON read/write — **not** a Unix socket and **not** a memory-only map shared by magic. Cross-process via files. |
| In-process | `SharedRateLimitStore` has a process-local `Mutex<HashMap>` of `not_before` for fast `remaining()`; full metadata still re-reads the flock file |
| Kill switch | `GROK_DISABLE_SHARED_RATE_LIMIT` (any value) → observe / wait / snapshot become no-ops |
| Semantics | On observe: `not_before = max(existing, now + wait)` (strictest wins). Callers `wait_if_limited` before issuing more HTTP. Attempt budgets stay in the sampler. |
| Provider keys | Host from base URL; optional `host+fingerprint` via `fingerprint_secret` (FNV-1a, not crypto). Well-known strings in `grok_rate_limit::keys`: `xai`, `openrouter`, `github`. |

**Docs:** `FORK.md` § Multi-session rate limits; README “shared rate limits across processes”; residual dual-auth bullet “rate-limit shared cooldown.”

### Who uses it (API gates)

| Client / path | Crate / module | Behavior |
|---------------|----------------|----------|
| **Chat / inference sampler** | `xai-grok-sampler` `src/actor/request_task.rs` | `wait_before_attempt` → `SharedRateLimitStore::process_default().wait_if_limited`; on 429 `observe` + wait; cancel-aware via `CancellationToken`. Key = base URL (+ API key fingerprint when present). Rate-limit **identity hop** also observes cooldown for the **left** fingerprint (temporary; not the 1h credit memo). |
| **GitHub compare (OSS update)** | `xai-grok-update` `src/oss_update.rs` | `ProviderKey::new(keys::GITHUB)`; wait before fetch; observe on 403/429 with header-derived wait |
| **SuperGrok billing `GET …/billing?format=credits`** | `xai-grok-shell` `extensions/billing.rs` | **Does not** call `grok-rate-limit` |
| **Management API** (prepaid / postpaid / usage series / key validation) | `xai-grok-shell` `auth/xai_management.rs` | **Does not** call `grok-rate-limit` |
| **Generic HTTP clients** | `xai-grok-http` shared `OnceLock` clients | Connection reuse only; **no** shared cooldown |

### Crate tests (`cargo test -p grok-rate-limit`)

In `store.rs`: `provider_key_sanitizes`, `provider_key_from_base_url_and_fingerprint_differs_by_key`, `max_merge_keeps_strictest`, `two_store_handles_share_file_state`, `remaining_zero_when_open`, `wait_if_limited_sleeps`, `fingerprint_stable`, `disable_env_makes_ops_noop`, `longer_second_observe_extends_not_before`.

Sampler integration (examples): `wait_before_attempt_aborts_on_cancel`, rate-limit rotate tests that assert **no** credit exhausted memo on plain 429 (`rate_limit_rotate_does_not_memoize_credit_exhausted`).

---

## 2. Other IPC / cross-process mechanisms (related but not the same)

| Mechanism | Path / role | Cross-process? |
|-----------|-------------|----------------|
| **Shared rate-limit store** | `$GROK_HOME/rate_limits/*.json` + flock | Yes — cooldown coordination |
| **Exhausted-credit memo** | `$GROK_HOME/exhausted_credits/{fp}.json` + process `LazyLock` map; `xai-grok-sampler` `exhausted_identity.rs` | Yes for **credit / allowance** skip after hop (1h TTL). **Not** used for plain 429 (by design; rate-limit uses `grok-rate-limit` instead) |
| **Managed config apply** | `xai-grok-shell` `managed_config.rs` flock | Yes — serialize managed-config apply/remove |
| **Leader mode** | `xai-grok-shell` `leader/` Unix domain socket (`use_leader`) | Yes — multi-client **session / agent** hosting over IPC, not billing meters or rate cooldowns |
| **Plugin marketplace git cache** | flock + TTL under cache root | Yes for that cache only |
| **Credentials / keyring / auth.json** | disk + Secret Service | Shared secrets, not poll series or HTTP cooldowns |

There is **no** general “all HTTP goes through one IPC broker” design. Multi-process awareness for rate limits is **file + flock**, which is the product’s chosen IPC-ish coordination for cooldowns.

---

## 3. Billing / limits poll history (Item 4 flat-poll honesty)

| Item | Fact |
|------|------|
| Module | `xai-grok-shell` `src/auth/included_poll_history.rs` (re-exported from `auth/mod.rs`) |
| Storage | **Process-local only:** `static POLL_HISTORY_BY_IDENTITY: Mutex<BTreeMap<…, VecDeque<IncludedPollSample>>>` |
| Cap | 32 samples per SuperGrok `identity_id` |
| Durability | Explicit: “Not durable across process restarts.” Never stores tokens. |
| Record path | Successful SuperGrok credits polls → `record_included_poll_history_from_config` in `extensions/billing.rs` → `record_included_poll_now` |
| Surface | `attach_flat_poll_from_history` in `xai-grok-pager` `limits_cmd.rs` sets `LimitsSnapshot.flat_poll_unproven_debit` + observed Build/extras flags; honesty copy in `views/limits_honesty.rs` |
| Multi-process | **Two separate `grok limits` (or TUI) processes do not share history.** Cold process ⇒ no series ⇒ `flat_poll` note stays off even if another process just polled flat meters. Matches residual “cold process; `flat_poll` absent.” |

### Related process-local SuperGrok billing memory (not flat-poll series)

`allowance_exhaust_from_billing.rs`: `INCLUDED_BILLING_BY_IDENTITY` process map (`remember_supergrok_included_billing` / dollar extras / Build %). Feeds ranking headroom and preemptive exhaust; also **not** durable / not multi-process.

### Tests (Slice 1)

Shell: `poll_history_marks_flat_when_included_and_extras_unchanged`, `poll_history_clears_flat_when_included_pct_steps`, `poll_history_clears_flat_when_build_product_usage_steps`, `poll_history_clears_flat_when_extras_cents_drop`, `process_ring_feeds_flat_from_history`, `flat_evidence_*`, plus residual filters
`cargo test -p xai-grok-shell --lib included_poll_history` and
`cargo test -p xai-grok-pager --lib flat_poll` / `limits_honesty` / `attach` paths (e.g. `limits_snapshot_sets_flat_poll_from_history_not_only_tests`).

---

## 4. Other API paths still process-only (caches / limits)

| Surface | Module | Process-only behavior |
|---------|--------|------------------------|
| **Console team prepaid** | `xai_management.rs` `PREPAID_CACHE` | Mutex + **60s** TTL (`CONSOLE_TEAM_BILLING_METER_CACHE_TTL_SECS`). Honesty note names process-cache lag. Force `grok limits` clears; background `FetchBilling` honors TTL. |
| **Console team postpaid preview** | same file `POSTPAID_CACHE` | Same 60s process TTL |
| **Management team id discovery** | `TEAM_ID_CACHE` | Process-only after validation GET |
| **Usage series POST** | `fetch_console_team_usage_series_*` | No shared store found; each process/path that needs series hits HTTP (unless caller holds its own result) |
| **SuperGrok included remember** | `allowance_exhaust_from_billing.rs` | Process map only |
| **Poll history / flat_poll** | `included_poll_history.rs` | Process ring only |
| **Inference 429 coordination** | sampler + `grok-rate-limit` | **Already multi-process** |
| **GitHub update 429** | `oss_update.rs` + store | **Already multi-process** |

Management and SuperGrok billing HTTP also do **not** publish or wait on shared cooldowns, so concurrent `limits` / TUI polls can stampede those endpoints if the server rate-limits them.

---

## 5. Gaps — what “multi-process aware through IPC” likely means here

Operator intent (from ask + Item 4 honesty context) splits into two different products:

### A. Shared **cooldowns** on HTTP 429 (existing pattern)

**Already done** for inference (and GitHub update). **Not done** for:

- SuperGrok proxy `GET /billing?format=credits` (and sibling multi-principal polls)
- Management prepaid / postpaid / usage series / management-key validation

“Any API calls” in this sense = every product HTTP client that can hit 429 should `wait_if_limited` + `observe` with a stable `ProviderKey` (host + management-key or session fingerprint), reusing `grok-rate-limit` rather than inventing sockets.

### B. Shared **observational state** (flat-poll / meter caches)

**Not done.** Two CLI processes:

- cannot combine poll samples for `flat_poll_unproven_debit`
- cannot share Management 60s meter warm cache (extra load only; honesty lag is per process)
- cannot share included-billing remember maps for ranking until each process polls

This is **not** rate-limit pooling; it is durable or flock-backed **history/cache** under `$GROK_HOME` (same spirit as `rate_limits/` and `exhausted_credits/`).

### C. What it does **not** mean (already separate)

- Multi SuperGrok OIDC / dual principals (shipped)
- Leader Unix IPC for agent sessions (different subsystem)
- Credit exhausted durable memo (credit hop only; must stay distinct from 429 cooldown)

---

## 6. Recommended next slices (for parent plan / implement)

### Smallest product slice (matches Item 4 + “two CLI limits processes”)

**Durable SuperGrok included poll history under `$GROK_HOME`**, flock + JSON ring per identity (mirror `rate_limits/` / `exhausted_credits/` patterns; no tokens).

- Keep pure detector (`included_debit_unproven` / `flat_poll_evidence_for_samples`) unchanged.
- Replace or back `POLL_HISTORY_BY_IDENTITY` with load-on-record / load-on-read that merges process ring + disk.
- Tests: two logical “store handles” (or temp `GROK_HOME`) see each other’s samples; cold process can still fire flat_poll after another process’s spaced polls; clear helper for tests.
- Acceptance: two sequential `grok limits` processes with the same `GROK_HOME` can surface `flat_poll` when the series only exists across processes (within min window / min polls).

Optional polish in same family: durable Management meter snapshot with short TTL (reduces double-fetch); lower urgency than poll history for honesty.

### Separate / larger design (true “all API calls” cooldowns)

Wire **billing + Management** HTTP through `SharedRateLimitStore`:

1. Choose keys: e.g. SuperGrok proxy host (+ session fp optional), Management host + management-key fingerprint.
2. Before send: `wait_if_limited`; on 429/403-with-retry: `observe` with Retry-After or fixed fallback.
3. Tests: hermetic observe/wait for management-shaped keys; no secrets on disk filenames.

Do **not** fold this into the credit exhausted memo.

### Explicit non-goals for a first PR

- New Unix-socket rate-limit daemon (flock JSON already is the IPC for cooldowns)
- Leader-hosted billing
- Claiming C4 server debit proven via multi-process history alone (history only improves **evidence availability** for honesty notes)

---

## Status summary (plain)

| Area | Status |
|------|--------|
| Shared inference / GitHub rate cooldowns (`grok-rate-limit` + flock) | **Shipped** |
| SuperGrok billing + Management API shared cooldowns | **Missing** |
| Included poll history / `flat_poll` multi-process | **Process-local only** (shipped for single long-lived process) |
| Management prepaid/postpaid process cache | **Process-local 60s TTL** (documented honesty) |
| Exhausted-credit durable memo | **Shipped** (credit path only) |
| Operator “IPC for any API call” | Likely means **extend flock-backed shared state** (cooldowns and/or poll history), not invent a second IPC stack |

**Suggested first implement:** durable included poll history under `$GROK_HOME` so Item 4 flat-poll honesty works across separate CLI `limits` processes. **Second track:** Management + SuperGrok billing `wait`/`observe` on `SharedRateLimitStore`.
