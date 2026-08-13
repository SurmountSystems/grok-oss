# Join: Item 1 + process pin (prefer free SuperGrok period allowance by default)

**Date:** 2026-08-03
**Plan:** five open limits/billing gaps (session plan approved with "Good Grok.")
**Scope:** Part A process pin + Item 1 only. Did **not** implement Item 5, Item 2 ticket filing, or live multipoll exhaust.

## Part A — Process pin (dual-write)

| Path | What |
|------|------|
| `/home/hunter/Projects/surmount/grok-build/AGENTS.md` | Hard constraint 4: **Complete thoughts** (complete American English; no half-labels; meters named fully; config names after the plain thought) |
| `/home/hunter/.grok/AGENTS.md` | Prose + tone: same **Complete thoughts** pin; billing meters wording uses full meter names |

Rule essence: plans, residual, joins, board titles, user-facing docs, and product chat use complete American English thoughts. Wrong: "SuperGrok included weekly." Right: "the free SuperGrok allowance for the current billing period (how much of that free quota is already used)."

## Part B — Item 1 product

### Behavior

- New/empty Grok home config: `[auth] auto_use_included_limits` defaults to **true** (prefer free SuperGrok period allowance before SuperGrok top-up dollars and the console API key).
- Existing file with explicit `false` (or `true`) is preserved.
- `preferred_method = api_key` still forces console-primary ranking off free-period-first.
- Serialize: default true is omitted from TOML; explicit false is written so it round-trips.

### Code

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-shell/src/auth/config.rs` | Default true via `default_auto_use_included_limits()`; serde `default = …` + skip serialize when true; tests for empty/false/api_key pin/round-trip |
| `crates/codegen/xai-grok-shell/src/auth/mod.rs` | Export `default_auto_use_included_limits` |
| `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` | Disk probe missing key → default true; doctor human copy names free period + how to set false |
| User-guide `02-authentication.md`, `11-custom-models.md` | Default true; how to set false; complete English meter names |
| `RESIDUAL.md`, `FORK.md` | Item 1 marked shipped |

### TDD (red → green)

**Contract:** empty new config → true; file with false → false; `preferred_method=api_key` still forces console.

Prior state: empty TOML deserialized to **false** (`auto_use_included_limits_deserializes_independently` asserted `!cfg.auto_use_included_limits`). That was the observed red contract before the flip.

**Green tests (same filters after product edit):**

```bash
cargo test -p xai-grok-shell --lib --locked -- \
  auto_use_included_limits_new_install \
  auto_use_included_limits_serializes \
  auto_use_included_limits_deserializes \
  format_human_auto_use \
  resolve_api_key_pin_stays_console
```

All passed. Also:

```bash
just check-limits-first-path
```

Passed (shell 16 + pager 22; hermetic).

## How to verify

```bash
# Item 1 unit contracts
cargo test -p xai-grok-shell --lib --locked -- \
  auto_use_included_limits_new_install \
  auto_use_included_limits_serializes \
  auto_use_included_limits_deserializes \
  format_human_auto_use \
  resolve_api_key_pin_stays_console

# Offline spend-order path
just check-limits-first-path

# Manual: empty home
# - no auto_use_included_limits in config.toml → free-period-first on
# - auto_use_included_limits = false → classic dual-auth
# - preferred_method = "api_key" → console primary regardless
# - grok doctor human report: "Prefer free SuperGrok period allowance: yes/no"
```

## Not done (out of scope)

- Item 2 xAI ticket filing
- Item 3 live after free period full
- Item 4 live multipoll honesty
- Item 5 spend charts / default-credits UI
