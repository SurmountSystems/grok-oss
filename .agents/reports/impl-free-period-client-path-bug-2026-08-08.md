# Free SuperGrok period stuck ~6%: client path bug (SessionToken vs rank)

**Date:** 2026-08-08
**Branch:** `fixes-2`
**Binary:** `grok-oss 0.2.111` installed via `just install` → `~/.cargo/bin/grok-oss`
**Meters kept distinct:** free SuperGrok period used % ≠ SuperGrok dollar credits ≠ console team prepaid ≠ team postpaid OAuth / Grok Build class ≠ console API credits.

---

## Operator correction honored

Prior reports treated flat free SuperGrok period % as **server C4 only** and pushed "file xAI ticket." Operator correction: free SuperGrok period **used to work**; treat this as **our client regression** until the product path is proven correct.

This pass found a **real client path bug** and fixed it with red→green TDD. Server debit is **not** closed as "proven fine." Dogfood after install is the next proof of whether free SuperGrok period % steps under the corrected SessionToken bearer.

---

## Root cause (client)

### Named contract

When `[auth] auto_use_included_limits = true` and dual SuperGrok principals both still have free SuperGrok period headroom, **SessionToken sampling must use the free SuperGrok period ranked primary JWT**, not the sticky AuthManager base principal (often Team after a business login).

### What was broken

1. **Rank** (`order_live_supergrok_for_auto` / `ranked_free_period_primary_token`) correctly prefers free SuperGrok period headroom, sooner reset, then **lex `identity_id`**. On dogfood, personal `58c5f686-…` ranks before business `61fab250-…` when headroom and reset are equal.
2. **Credential resolve with rank** (`resolve_credentials_preferring_with_rank`) can order SuperGrok JWTs correctly from `auth.json`.
3. **SessionToken reconstruct** (`reconstruct_full_config` in `sampler_turn.rs`) ignored rank for the live bearer. It always took:
   - `AuthManager::current_wire_valid()` as `api_key`
   - `BearerResolver` that re-reads the same AuthManager base on every request
4. Sticky **AuthManager base** on this host is **Team / business** (`61fab250-…`) after dual login / last Team-shaped load. Personal SuperGrok stays only under the multi-slot (`…::personal`).
5. Net effect: chrome and dual-auth status can say free SuperGrok period first / SuperGrok session, while **live cli-chat-proxy SessionToken traffic rides the Team JWT**. Team postpaid OAuth / Grok Build settlement can climb while free SuperGrok period used % sits flat at **6.0**. That matches "used to work" before sticky Team base became the SessionToken identity.

This is **not** "console became primary" and **not** invented 6% chrome. Poll history is honest: server returns `credit_usage_percent: 6.0` for both identity ring files. The client still sent the **wrong SuperGrok principal** for ranked free-period-first intent.

### Live evidence (this host, 2026-08-08)

| Fact | Evidence |
|------|----------|
| Config | `preferred_method = "oidc"`, `auto_use_included_limits = true`, `allow_spend_when_free_period_debit_unproven = true` |
| Auth base | Team principal `team_id=61fab250-b2c1-40cf-b5b8-628e673a2eeb` on base OIDC scope |
| Personal multi-slot | User principal present under `…::personal` (identity ring file `58c5f686-…`) |
| Poll rings | `~/.grok/included_poll_history/{58c5…,61fab…}.json` — **n=32 each, only 6.0%**, prepaid **10029** cents |
| Prior multipolls | SuperGrok path, `console.isLive=false`, team OAuth class climbing while free SuperGrok period flat |

### What this is not (yet)

- **Not** proven that personal JWT alone makes free SuperGrok period % step. That is **dogfood after install**.
- **Not** a claim that xAI billing is perfect. If free SuperGrok period stays flat **after** SessionToken bearer follows ranked personal primary, then re-evaluate server debit with **path traces that include which identity JWT settled**, not vibes.
- **Not** wrong-binary analysis (`grok` vs `grok-oss`). Focus is product path in Surmount grok-oss.

---

## Fix (minimal product)

### Behavior

When SessionToken sampling is active and `auto_use_included_limits` is true:

1. Load SuperGrok session candidates from the AuthManager home.
2. If ranked free SuperGrok period primary JWT differs from current wire-valid bearer, **upsert that principal to base scope** and **`hot_swap`** so refresh, sampling, and active chrome agree.
3. Then proceed with existing `auth().await` + bearer resolve.

Same align runs from:

- `reconstruct_full_config` (SessionToken turn reconstruct)
- `prepare_sampling_config_for_model` (model switch / initial config)

### Files

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-shell/src/auth/supergrok_identity_rank.rs` | `ranked_free_period_primary_token`, `session_bearer_should_align_to_ranked_free_period_primary`, unit tests |
| `crates/codegen/xai-grok-shell/src/auth/manager.rs` | `AuthManager::align_to_ranked_free_period_primary` |
| `crates/codegen/xai-grok-shell/src/auth/manager_tests.rs` | Integration: sticky Team base → personal ranked primary |
| `crates/codegen/xai-grok-shell/src/auth/mod.rs` | Re-exports |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` | Call align before SessionToken bearer read |
| `crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs` | Call align in prepare_sampling when auto_use on |

### Tests (red→green contract)

```text
cargo test -p xai-grok-shell --lib free_period
```

Includes:

- `ranked_free_period_primary_personal_when_equal_headroom_not_sticky_business`
- `session_bearer_align_false_when_ranked_missing_or_empty`
- `align_to_ranked_free_period_primary_switches_sticky_team_base_to_personal`
- existing free_period_debit_unproven_guard suite

**Result:** 20 passed (filter `free_period`).

Also:

```text
cargo fmt -p xai-grok-shell
cargo clippy -p xai-grok-shell --lib -- -D warnings   # clean
just install   # grok-oss 0.2.111 → ~/.cargo/bin/grok-oss
```

(`cargo clippy --all-targets -D warnings` still has pre-existing failures outside this slice; lib target is clean.)

---

## Dogfood (operator)

1. Use **`grok-oss`** from `~/.cargo/bin` (not official `grok` if that is a different tree).
2. Confirm dual SuperGrok still present; config still `auto_use_included_limits = true`.
3. Run several SuperGrok session turns (not console-primary pin).
4. Watch free SuperGrok period used % on `/limits` and ring under `~/.grok/included_poll_history/`.
5. Expect log line: `auth: aligned SessionToken bearer to free SuperGrok period ranked primary` when base was Team and rank prefers personal.
6. **Pass:** free SuperGrok period used % rises (or SuperGrok dollar credits after free period is full).
   **Fail:** still flat at 6% with log proving personal (or ranked) JWT is wire-active. Then server debit is back on the table with **client path disproven**, not assumed.

---

## Residual honesty update

- Demote "client levers exhausted / only file C4 ticket" for this stuck-6% window.
- Keep C4 server evidence packages as **secondary** if dogfood after this fix still shows flat free SuperGrok period with correct ranked SessionToken.
- Open dogfood: does free SuperGrok period % step under ranked personal SessionToken after install?

---

## Summary

| Question | Answer |
|----------|--------|
| Client or server? | **Client path bug found and fixed:** SessionToken bearer stuck on Team base, ignored free SuperGrok period rank. |
| Proven not client? | **No.** Opposite: client principal selection was wrong for dual SuperGrok. |
| Debit restored? | **Code path restored.** Live meter step needs dogfood on installed `grok-oss`. |
| Ticket-only answer? | **Rejected** until dogfood after this fix fails with path traces. |
