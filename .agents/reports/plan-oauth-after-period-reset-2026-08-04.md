# Dogfood: free SuperGrok period after reset (oauth + included limits)

**Date:** 2026-08-04
**Goal:** Prefer free SuperGrok period allowance after a period reset, not SuperGrok $ extras or console first. Does **not** fill Grok Business license charts.

## Config checklist

```toml
# ~/.grok/config.toml
[auth]
preferred_method = "oauth"              # SuperGrok session primary
auto_use_included_limits = true         # prefer free period allowance before extras / console
```

| Setting | What it does | What it does **not** do |
|---------|--------------|-------------------------|
| `preferred_method = "oauth"` | SuperGrok OAuth session is primary resolve | Does not invent Management prepaid; does not fill license page charts |
| `auto_use_included_limits = true` | Prefer free SuperGrok allowance for the current billing period (personal and/or Business SuperGrok) before SuperGrok prepaid top-up dollars and the console API key | Does not merge SuperGrok and team prepaid pools; does not move Platforms → Grok Business licenses meters |

Default for **new/empty** Grok homes is already `auto_use_included_limits = true`. Explicit `false` is preserved. Hard pin `preferred_method = "api_key"` stays console-primary even when free SuperGrok period allowance remains.

## After period reset

1. Confirm config above (or leave default free-period-first on).
2. `grok login` / session present; rebuild product if binary lags.
3. `grok-oss limits --json` → expect `liveSampling` SuperGrok session while included has headroom; SuperGrok included % / reset visible.
4. Team Management dollars still need management key + team id (separate from this ranking).

## Related

- User-guide: `02-authentication.md` (dual-auth + free-period-first + Management surfaces)
- SuperGrok-live team visibility ship: `impl-supergrok-live-team-usage-2026-08-04.md`
