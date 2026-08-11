# Implement Work C — Limits vs credits chrome (paying intent + consistency)

**Date:** 2026-08-09
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Scope:** Work C only (not A/B/E). Plan:
`~/.grok/sessions/.../019faf9d-ef93-7d93-b34b-9f19b6345613/plan.md` § Work C.

## Problem addressed

Upper-right Design A free SuperGrok period `%` and footer team prepaid / Grok
Build class dollars looked like competing "who is paying" numbers. Session A vs
B differed when auth principal or management cache warmth differed, not because
of project cwd magic.

## Product changes

### 1. Compact status = intent chrome (labeled)

SuperGrok free SuperGrok period path now paints:

| State | Compact string |
|-------|----------------|
| Free SuperGrok period known, has room | `intent · N%` |
| Free SuperGrok period full, no SuperGrok dollar credits | `intent · 100%` |
| Free SuperGrok period cold / active poll auth-failed | `intent · ...%` |
| Free SuperGrok period full + SuperGrok dollar credits | `SuperGrok extras · $N` (unchanged; already named) |
| Console live | `console · $N` / gap (unchanged) |

Team prepaid and Grok Build class never paint on compact while free SuperGrok
period has room (Design A preserved).

### 2. SuperGrok-live footer = Team settlement secondary

Under SuperGrok session, team Management meters use a settlement prefix:

- Warm prepaid: `Team settlement: prepaid $340`
- Warm prepaid + class: `Team settlement: prepaid $340 · Grok Build class $1162.92`
- Class only: `Team settlement: Grok Build class $201.76`
- Cold management (key present, cents unknown): `Team settlement: loading team prepaid...`
- Missing management key: still omits prepaid gap on SuperGrok-only footers
  (honesty stays on `/limits` Balance)

Console live footers keep `Console key · team prepaid: $N` (console pool is the
live principal, not SuperGrok settlement side-channel).

### 3. Consistency / A≠B honesty

- Same personal SuperGrok principal + warm management data → same settlement
  footer rule (no project cwd gate).
- Cold management cache → honest loading under Team settlement, not silent
  omit when the other session already has numbers.
- `usage_visible = false` (team AuthMeta / API-key consumer surface off) still
  hides footer settlement chips while compact intent `%` still paints. Test
  documents that gate so A≠B is principal/cache, not random.

### 4. `activeDriver` stays intent not settlement

No change to `ActiveSpendDriver` / `activeDriver` wire. Free SuperGrok period
headroom still yields `supergrok_free_period` even when team prepaid is known.
Product does not invent free SuperGrok period used %.

## Files touched

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Intent compact strings; `TEAM_SETTLEMENT_LABEL` + `format_team_settlement_footer`; SuperGrok merge; Work C tests |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Status bar hit tests updated for `intent ·` |
| `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md` | `/limits` status + footer operator copy |
| `crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md` | Burn order + team prepaid surfaces |
| `FORK.md` | Free SuperGrok period chrome note includes Work C labels |
| `RESIDUAL.md` | Settlement gap: chrome labels shipped; `payingMeter` still soft |

## TDD

**Red→green contracts (new):**

- `work_c_free_period_headroom_intent_compact_and_settlement_footer`
- `work_c_cold_management_cache_shows_settlement_loading_not_silent_omit`
- `work_c_usage_visible_false_hides_settlement_footer_not_compact_intent`
- `work_c_settlement_footer_does_not_replace_free_period_intent`

Existing Design A / footer / loading exact-string asserts updated to match
intent and Team settlement labels.

**Commands (green):**

```bash
cargo test -p xai-grok-pager --lib views::credit_bar::tests
# 86 passed (includes 4 work_c_*)

cargo test -p xai-grok-pager --lib status_bar_credits
cargo test -p xai-grok-pager --lib status_bar_free_period
cargo test -p xai-grok-pager --lib status_bar_supergrok
cargo test -p xai-grok-pager --lib status_bar_console
# all green

cargo test -p xai-grok-pager --lib enter_prompt_mode
# green (Work A not regressed)

cargo fmt -p xai-grok-pager
cargo clippy -p xai-grok-pager --lib -- -D warnings
# clean on product lib
```

`--all-targets` clippy still reports pre-existing test-only issues in other
files (not introduced by Work C).

## Acceptance check

| Acceptance | Status |
|------------|--------|
| Operator can tell intent driver (upper-right `intent · N%` / extras / console) | Shipped |
| Team $ clearly settlement secondary under SuperGrok | Shipped (`Team settlement:`) |
| Same dual-auth + warm management → same footer rule | Shipped (no cwd gate; labels consistent) |
| Cold cache → loading, not silent omit | Shipped under Team settlement |
| `activeDriver` intent only; no invented free SuperGrok period % | Unchanged / preserved |
| Work A composer Enter cue not regressed | Verified |

## Not in this slice

- Machine `payingMeter` from settlement deltas (still soft residual)
- Decoupling settlement footer from `usage_visible` for team AuthMeta (gate
  documented; compact still paints for team dual-auth dogfood)
- Settlement-primary compact (would fight free SuperGrok period-first Design A)

## Operator dogfood after rebuild

1. Rebuild/install current tree so chrome strings ship.
2. Personal SuperGrok live with free SuperGrok period room + warm management:
   upper-right `intent · N%`; footer `Team settlement: prepaid $… · Grok Build class $…`.
3. Cold management process: footer shows Team settlement loading, not blank team line.
4. Two projects, same auth process/cache: same footer settlement rule.
