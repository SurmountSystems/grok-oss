# Impl report: free SuperGrok period limits over credits — operator protect (2026-08-08)

**Branch:** `fixes-2`
**Mandate:** stop silent money drain when free SuperGrok period limits do not debit; multipoll evidence first; TDD; no invent free SuperGrok period used %

---

## Multipoll evidence (operator-run 20260808T104042Z)

Dir: `.agents/reports/limits-multipoll-20260808T104042Z/`

| Meter | Sample 0 | Sample 1 | Moved? |
|-------|----------|----------|--------|
| free SuperGrok period used % (both principals) | 6.0 | 6.0 | **flat** |
| SuperGrok dollar credits USD | 100.29 | 100.29 | **flat** |
| team postpaid OAuth / Grok Build class USD | 1068.66 | 1068.82 | **+$0.16** |
| team postpaid API class USD | 5.8 | 5.8 | flat |
| team usage series OAuth class | ~911.20 | ~911.35 | **rose** |
| liveSampling | supergrok_session | same | path |
| activeDriver | supergrok_free_period | same | Design A |
| consoleIsLive | false | false | not console |
| pathOk / P1 | true | — | path OK |
| freePeriodSeries / P2 | flat | — | unproven debit |
| flatPollUnprovenDebit | true | true | |

**Verdict:** P1 path OK (not console primary under free SuperGrok period headroom). P2 free SuperGrok period flat. Team OAuth settlement climbed under SuperGrok session. SuperGrok dollar credits did not move as primary spend.

---

## Root cause

**C4-only (server free SuperGrok period debit), not a client spend-order bug.**

| Audit item | Result |
|------------|--------|
| Rank / resolve / sampling under free SuperGrok period headroom | Session primary; console omitted while free SuperGrok period has room (`auto_use_included_limits`); live multipoll confirms `consoleIsLive=false` |
| Afterburner SuperGrok dollar credits | Only after free SuperGrok period full (≥100%); multipoll at 6% correctly shows free period as active driver; credits flat $100.29 |
| Dual-auth failover / sticky exhaust / preferred_method | No false console primary under headroom; sticky must not paint console · $ while free period has room (prior ship) |
| Host | cli-chat-proxy SuperGrok session (not api.x.ai primary) |
| Subagent / implement-loop / Imagine / voice / BYOK | Separate credential paths remain path facts; rate-limit cooldowns already shared (Phase R). Main chat sampler is the SuperGrok session money path multipoll measured |
| Invent free SuperGrok period used % | **Banned** — not done |

Money waste = team Grok Build / OAuth settlement climbing under SuperGrok session while free SuperGrok period limits stay flat (server absorption gap). Client cannot force server debit.

---

## What shipped this turn

### Operator-protect lever (default block)

| Piece | Detail |
|-------|--------|
| Config | `[auth] allow_spend_when_free_period_debit_unproven` default **false** (block) |
| Env opt-in | `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=1` (truthy) |
| Pure decision | `should_block_spend_when_free_period_debit_unproven` — block when unproven + free SuperGrok period headroom (used &lt; 100%) + not console pin + not opted in |
| Sampler gate | `run_turn_via_sampler` fail-loud with clear message before inference |
| Honesty | `/limits` + `/usage` note when turns blocked; doctor dual-auth status line |
| Docs | user-guide `02-authentication.md` § *Block turns when free SuperGrok period debit is unproven* |

**Default justification:** same safety-first spirit as `auto_use_included_limits=true` for new installs. Cold processes without multipoll history do **not** block (usage unknown or unproven false). After multipoll proves flat free SuperGrok period under headroom, silent continue was burning team settlement; fail-loud stop is the product lever until C4 closes or the operator opts in.

### Code

- `crates/codegen/xai-grok-shell/src/auth/free_period_debit_unproven_guard.rs` (new)
- `crates/codegen/xai-grok-shell/src/auth/config.rs` — config field
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` — gate
- `crates/codegen/xai-grok-shell/src/auth/dual_auth_status.rs` — doctor line
- `crates/codegen/xai-grok-pager/src/views/limits_honesty.rs` — note + tests
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs` / `credit_bar.rs` — wire note
- Residual + C4 multipoll addendum

---

## TDD

| Test | Contract | Result |
|------|----------|--------|
| `multipoll_six_percent_flat_unproven_blocks_by_default` | 6% headroom + unproven + not opted in → block | green |
| pure allows opt-in / console pin / full period / unknown / not unproven | escape paths | green |
| `allow_spend_when_free_period_debit_unproven_default_false_opt_in_true` | config default false; true round-trips | green |
| `turns_blocked_note_when_guard_active` | limits honesty names opt-in | green |
| related flat_poll / honesty suite | no regression | green |

Commands:

```bash
cargo test -p xai-grok-shell --lib -- free_period_debit_unproven allow_spend_when_free_period
cargo test -p xai-grok-pager --lib -- limits_honesty flat_poll turns_blocked limits_snapshot_sets_flat
cargo fmt -p xai-grok-shell -p xai-grok-pager
cargo clippy -p xai-grok-shell --lib -- -D warnings
cargo clippy -p xai-grok-pager --lib -- -D warnings
```

---

## Residual honesty

- **Open (server C4):** free SuperGrok period debit still unproven; operator must file paste-ready ticket.
- **Open (client protect):** shipped; operator can opt in to spend under unproven debit if they accept team settlement climb.
- **Not claimed:** server now debits free SuperGrok period; SuperGrok dollar credits preferred over free SuperGrok period (path remains free SuperGrok period first).

---

## Operator use after install

Default: with multipoll history showing flat free SuperGrok period at 6%, new turns **block** with the fail-loud message.

To resume traffic under unproven free SuperGrok period debit:

```toml
[auth]
allow_spend_when_free_period_debit_unproven = true
```

Or: `GROK_ALLOW_SPEND_WHEN_FREE_PERIOD_DEBIT_UNPROVEN=1`
