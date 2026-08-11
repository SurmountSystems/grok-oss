# Plan: verify session work (TDD) + limits before credits honesty (still 6%)

**Date:** 2026-08-07 (evening)
**Goal:** Prove what we shipped still builds and passes named contracts with red/green discipline where required. Keep the product goal **use free SuperGrok period (limits) before SuperGrok $ extras and before console credits**. Face the fact that free period used is **still ~6%** after heavy dogfood.

**Inventory:** `.agents/reports/plan-verify-tdd-limits-inventory-2026-08-07.md`
**C4 / 6% writeups:** `still-6pct-chrome`, `doubt-free-period-stuck-6pct`, `how-to-fix-c4-free-period-debit`

## Live snapshot (just now)

| Field | Value |
|-------|--------|
| Binary | `grok-oss 0.2.111 (c87f66a61d94)` |
| Live sampling | SuperGrok session (business) |
| activeDriver | **supergrok_free_period** |
| Free SuperGrok period used | **6.0%** (both principals, live_poll OK, shared pool) |
| SuperGrok $ extras | ~$100.29 |
| console.isLive | false |
| Team prepaid (Management) | $340 |
| Team postpaid OAuth / Grok Build class | ~$1011+ (still climbing vs free period flat) |

**Reading:** Client is doing Design A (limits first). Compact **6%** is the free-period poll, not a failed install. Free period not moving past 6% under load is **C4 server residual**, not "limits-before-credits code failed."

## Context

### What "limits before credits" means (product)

1. While free SuperGrok period has room: stay on SuperGrok session; compact shows free-period **%**; do **not** primary-hop to console to "save" free period.
2. After free period full: SuperGrok $ extras before console (after-burner).
3. Then console team prepaid / API path.

Unit tests encode that. Live `activeDriver=supergrok_free_period` + `console.isLive=false` means the path is right. **Absorption of burn into free-period % is a billing server question (C4).**

### What this plan is

A **verification and honesty** vertical:

1. Re-run the session's named unit filters (keep green; any red is a regression to fix with TDD).
2. `just install` (or `/rebuild`) so dogfood binary matches the tree under test.
3. One live `limits --json` check after install: free-period-first still holds; record free period %.
4. Do **not** invent free-period debit. Do **not** "fix" 6% by painting a fake higher %.
5. Optional soft: open `/rebuild` paint glitch only if time after verify (separate bug).

### What this plan is not

- Closing C4 by client invent.
- Full `just ci` / all 27k tests unless operator wants that burn.
- Claiming free period absorbed dogfood because chrome says free period first.

## Approach

### Wave A: re-run contract tests (no product invent)

Run these packages/filters. Expect green. If any fail: stop, treat as regression, red evidence + fix + same filter green.

**Limits before credits (primary):**

```bash
cargo test -p xai-grok-pager --lib -- \
  check_limits_first compact_status_ c6_team_usage flat_poll limits_honesty \
  limits_json_ status_bar_supergrok status_bar_console meter_identity branch_2b \
  format_supergrok_session active_driver status_bar_free_period sticky_memo

cargo test -p xai-grok-shell --lib -- \
  auto_order_omits_console auto_with_included_headroom auto_after_included \
  allowance_exhaust_from_billing out_of_allowance_helper

cargo test -p xai-grok-sampler --lib -- prefer_live exhausted
```

**Session ship wave (rebuild / Ctrl+C / killall resume / also-guard):**

```bash
cargo test -p xai-grok-pager --lib -- \
  soft_park_empty_ctrl_c_abandons plan_panel_empty_ctrl_c_abandons \
  plan_approval_ctrl_c_clears_draft \
  quit_mid_turn_writes_canceled quit_idle_does_not_write \
  slash::commands::rebuild dispatch::rebuild

cargo test -p xai-grok-shell --lib -- \
  leader_is_older_than parse_binary decide_relaunch \
  canceled_turn_resume process_shutdown_class \
  auth_failed_poll order_live_prefers_poll_ok sibling_poll_skips \
  session_needs_oidc_refresh non_active_poll_targets

cargo test -p xai-grok-update --lib -- rebuild::
cargo test -p xai-grok-tools --lib -- live_demote_guard todo_bound_task_id
cargo test -p xai-grok-agent --lib -- test_base_template_plan_present_includes_planning
```

**Named Design A smoking gun (must pass):**

- `compact_status_sticky_memo_with_free_period_headroom_shows_pct_not_console_dollars`
  (free period 6% + sticky memo + team prepaid → string **`6%`**, not `console · $N`)

### Wave B: build install

```bash
just install
# or grok-oss rebuild / /rebuild once TUI is healthy
~/.cargo/bin/grok-oss --version
```

### Wave C: live honesty check (operator + agent)

```bash
grok-oss limits --json
```

Expect:

| Check | Pass means |
|-------|------------|
| activeDriver | free SuperGrok period while used &lt; 100% |
| liveSampling | SuperGrok session |
| console.isLive | false under free-period headroom |
| includedUsedPct | whatever server says (today ~6%); do not invent higher |
| Compact chrome dogfood | `N%` not false console dollars |

Record multi-sample if free period steps; if flat while team OAuth $ rises, that is C4 evidence, not client fail.

### Wave D: TDD hygiene for any fix

If Wave A finds a red:

1. Name the contract in plain English.
2. Keep the failing test; do not loosen asserts.
3. Minimal product fix; same filter green.
4. fmt + clippy on touched packages.

If Wave A is all green: **do not invent fake red** for already-shipped code. Document "re-verified green" in a short report. (Host TDD law: green without prior red is not new TDD, but re-verify is required for this plan.)

### Wave E: C4 (human, parallel, not blocked by A–C)

- Keep ticket package ready; add 2026-08-07 addendum: free period stuck ~6%, team OAuth class ~$1000+, SuperGrok primary.
- Client cannot close C4.

### Wave F (optional residual): `/rebuild` TUI glitch

Only after A–C green if operator still cares. Separate from limits-before-credits. Board: `bug:rebuild-tui-glitch`.

## Critical files

| Path | Why |
|------|-----|
| `credit_bar.rs` / status render | Compact free-period % |
| `limits_cmd` / `active_driver` | activeDriver wire |
| Rank / auto_order | Omit console while free period headroom |
| Test modules listed above | Contracts |
| `justfile` install | Binary for dogfood |
| C4 evidence join | Server escalate only |

## Steps after approve

1. Run Wave A filters; log pass/fail in `.agents/reports/verify-session-tdd-limits-2026-08-07.md`.
2. Fix any red with proper TDD.
3. Wave B install; Wave C limits snapshot into same report.
4. Plain English residual note if free period still 6%: client path OK; C4 open.
5. Optional Wave F glitch only if requested after verify.

## Done when

1. All Wave A filters green (or fixed red→green with evidence).
2. Installed binary matches tree SHA/version under test.
3. Live limits: free-period-first path confirmed; free period % recorded honestly (still 6% is OK for client pass).
4. Report states clearly: **limits-before-credits product OK** vs **C4 free-period absorption not proven**.
5. No invented debit.

## Risks

| Risk | Mitigation |
|------|------------|
| Operator reads "still 6%" as client fail | Explicit Design A vs C4 split in report |
| Full CI burn | Targeted filters only unless asked |
| Fake TDD theater | No loosening tests; no invent red for green code |

## Verification

The plan **is** the verification. Success = report with command exit codes + live limits table + honesty split.

## Critical Files for Implementation

- Test entry points under `xai-grok-pager`, `xai-grok-shell`, `xai-grok-sampler`, `xai-grok-update`, `xai-grok-tools`, `xai-grok-agent`
- `just install` / dogfood binary
- `.agents/reports/verify-session-tdd-limits-2026-08-07.md` (to write)

## References

- `.agents/reports/plan-verify-tdd-limits-inventory-2026-08-07.md`
- `.agents/reports/impl-limits-before-credits-2026-08-07.md`
- `.agents/reports/how-to-fix-c4-free-period-debit-2026-08-07.md`
- `doc/dev/upstream-regression-filters.md` §2c when relevant
