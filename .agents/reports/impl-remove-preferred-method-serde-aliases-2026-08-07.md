# Report: remove preferred_method serde aliases (2026-08-07)

## Goal

`preferred_method = "oauth"` was accepted via Surmount aliases but is not
stock-compatible for dual debugging with ordinary grok. Accept only the
canonical wire values stock/shared configs understand.

## Canonical wire values

| Enum | Wire (serde) |
|------|----------------|
| `PreferredAuthMethod::ApiKey` | `api_key` |
| `PreferredAuthMethod::Oidc` | `oidc` |

Rationale: matches `AuthMode` snake_case (`oidc`), live dogfood configs, and
manager comments. Former "canonical oauth" was a Surmount-only alias layer.

## Rejected (no longer deserialize)

`oauth`, `oauth_token`, `console_api_key`, `api`, `key`, and still `auto` /
`personal` / `business`.

## Code

| File | Change |
|------|--------|
| `crates/codegen/xai-grok-shell/src/auth/config.rs` | Serialize/deserialize only `api_key` / `oidc`; docs; tests |
| `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` | Label probe no longer maps aliases; doctor shows `oidc` not `oauth` for the pin |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | Example + allowed values → `oidc` |
| `crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md` | Allowed values → `api_key` / `oidc` only |

Display copy elsewhere that says "SuperGrok OAuth session" (product feature name)
is unchanged. That is not the config wire field.

## Tests (red→green)

Renamed/replaced:

- `preferred_method_accepts_aliases_not_auto` → `preferred_method_rejects_aliases_and_auto`
- `preferred_method_oauth_round_trips_as_oauth` → `preferred_method_oidc_round_trips_as_oidc` (serialize `oidc`; reject `"oauth"`)

Commands:

```text
cargo fmt -p xai-grok-shell
cargo test -p xai-grok-shell --lib preferred_method
# → 4 passed
cargo test -p xai-grok-shell --lib dual_auth_status
# → 7 passed
```

## Clippy

`cargo clippy -p xai-grok-shell --all-targets -- -D warnings` fails on many
**pre-existing** issues outside this change (field_reassign_with_default in
models/subagent tests, await_holding_lock in scrub tests, etc.). One hit is
the pre-existing `auto_use_included_limits_does_not_block_automatic_oidc`
field-reassign style in `config.rs`; not introduced by the alias removal.
Not mopped here (out of scope).

## Operator action

If `$GROK_HOME/config.toml` still has:

```toml
preferred_method = "oauth"
```

change to:

```toml
preferred_method = "oidc"
```

or omit the pin for default session-primary dual-auth.

## Not done

- No git add/commit
- Historical `.agents/plans/` / `.agents/joins/` / reports still mention
  `oauth` as preferred_method; left as history
- FORK residual prose already uses `preferred_method=api_key` / product
  "OAuth" names; no FORK edit required for this wire scrub
