# Plan: preferred_method aliases + role-aware failover

**Status:** deferred durable outline (2026-07-27). **Do not implement** until
operator explicitly approves a full plan pass. Token-cheap: residual §4 + this
file are SoT after compaction.

**Board:** `feat:auth-preferred-aliases-roles`, `feat:failover-any-live-limits`,
`bug:credits-meter-wrong-pool`. Slice: `feat:second-supergrok-business`.

## Problem (dogfood)

- `preferred_method = "api_key"` correctly forces **console API $** first.
  Operator wants **SuperGrok included limits** first (personal limits, not
  dollar extras), with console as backup when limits are gone.
- Config name `oidc` is opaque; should read as **oauth** / SuperGrok login.
- Role (personal SuperGrok vs Business SuperGrok vs console key) should drive
  **failover order and chrome**, not a `preferred_method = personal|business`
  enum (operator rejected that).

## Non-goals (this plan)

- Drive-by product code before approve.
- Treating console API key as SuperGrok Heavy Business limits.
- `preferred_method` values `personal` / `business`.

## Slice A — naming only (small, can ship alone)

**Contract:** `PreferredAuthMethod` remains two variants (console key vs SuperGrok
login session).

| Canonical (serde / docs) | Aliases (accept on deserialize) |
|--------------------------|----------------------------------|
| `api_key` | `console_api_key`, `api`, `key` |
| `oauth` (prefer over `oidc` in docs/UI) | `oauth_token`, `oidc` (keep working) |

Serialize preference: write `api_key` and `oauth` (not `oidc`) when emitting
config examples. Doctor / `--list-api-keys`: plain "SuperGrok login" /
"console API key".

**TDD (when implementing A):**

- Deserialize each alias → correct enum variant.
- Unknown string fails closed.
- Round-trip serialize uses canonical names.
- Existing `preferred_method_deserializes_from_toml` extended, not loosened.

**Files:** `xai-grok-shell` `auth/config.rs`; dual_auth_status labels; user-guide
`02-authentication.md`.

## Slice B — role-aware failover (larger; plan before code)

**Intent:** identities carry a **role** used only for ordering, labels, meter
pool, and hop policy. Method pin still only chooses session-vs-key primary when
both method classes exist.

**Draft default order (TBD at implement plan time; do not ship from this line):**

1. SuperGrok session(s) with **included** headroom (personal, then Business if
   multi-session exists)
2. SuperGrok **dollar extras** only if policy allows (today often hop to console
   instead when included is 100%)
3. Console API keys (env → store multi-add order)

**TDD matrix themes (expand when implementing B):**

| Case | Expected |
|------|----------|
| Default unset preferred + session + console | Session first; console failover |
| `oauth` / aliases + both | Same as session primary |
| `api_key` / aliases + both | Console first; session last |
| Included weekly ≥100% + dual-auth | Leave SuperGrok included path; prefer next live (console today) |
| Credit/Heavy limit / plain 429 | Hop within failover list; host switch |
| Console primary + successful console | Do not silently sample SuperGrok first |
| Meter on console live | Not SuperGrok extras $; not permanent "no $ meter yet" without roadmap |
| Multi SuperGrok (if built) | Separate store slots; role labels; order per defaults |

**Open design (park until implement plan):** multi SuperGrok secret store shape;
whether Business is second OAuth slot or team switch only; exact extras-vs-
console priority.

## Immediate operator workaround (no code)

1. Remove `preferred_method = "api_key"` or set session primary (`oauth` once
   aliases ship; today `oidc` or omit).
2. Restart CLI.
3. Personal included limits require personal SuperGrok OAuth session live.
4. Console key remains failover after limits exhausted.

## Verification when unparked

Red→green in-tree tests per slice; `grok-oss login --list-api-keys` wording;
no secret leakage in status.

## Related

- Residual §4 dual-auth still-open
- Joins: `join-dual-auth-audit.md`, `join-hop-wiring.md`,
  `join-failover-meter-intent.md`
