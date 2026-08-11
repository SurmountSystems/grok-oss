# Join: Token Economy full product (2026-08-03)

**Plan:** session plan “Token Economy options (Good Grok)” — four pillars, §7 checklist.
**Status:** shipped complete product (not pillar-1-only).

## What shipped

### Pillar 1 — Economic implement effort
- Config table `[token_economy]`: `cap_implement_effort_when_economic` (default true), `max_implement_effort` (default **3**), `desired_implement_effort` (default **2**); validation `desired ≤ max`, both 1–5.
- Shared policy: `xai_grok_shell::token_economy::apply_implement_effort_policy`.
- Entry paths (inventory test):
  1. Auto-run: `xai-grok-pager` `app/auto_implement.rs` → `apply_implement_effort_for_product`
  2. Human `/implement`: `app/dispatch/prompt.rs` → `apply_implement_effort_on_submit` (PassThrough + InjectSkill)
- Over-ceiling → clamp + toast; missing effort → inject desired; economic off / cap master false → no rewrite.
- Stale “identity” / “does not rewrite effort” docs fixed (FORK, user-guide, economic_mode module comments).

### Pillar 2 — Period pacing
- Pure math: `token_economy/period_pacing.rs` (linear burn expected % vs free SuperGrok period used %).
- Copy: “N% ahead of linear burn” / “N% behind linear burn” (not ambiguous “ahead”).
- Surfaces: credit/status compact chip (`credit_bar.rs`); `/limits` principal lines + double-entry context; `/usage` summary (`format_usage_summary_with_live` + console-live path).
- Omit when bounds missing; period start derived from end + weekly/monthly type when wire start absent.
- Console-live honesty labels SuperGrok period as not live principal; never dollar-izes period %.

### Pillar 3 — Double-entry
- Local book: ingest `usage.jsonl` → `local_usage_event` (idempotent on `event_ulid`; cost_missing preserved).
- Remote book: `remote_meter_sample` + Management prepaid/postpaid/series when available.
- `/spend` slash (`slash/commands/spend.rs`, aliases `/double-entry`, `/ledger`) refreshes local book then formats both books + gap honesty.
- `/limits` section via `format_limits_spend_section` (summarize-only; no full session-tree walk on every format).
- Meters named distinctly in reconcile strings.

### Pillar 4 — `$GROK_HOME/grok_oss.db`
- Module `xai_grok_shell::grok_oss`: open path (override honored), NFS-safe journal pattern, busy timeout, fail-open helpers.
- Schema versioned meta + Token Economy tables (local_usage_event, remote_meter_sample, reconciliation_run); additive migrations.
- No secrets in schema (unit test). Session directory layout unchanged.

## Paths (primary)

| Area | Path |
|------|------|
| Config | `crates/codegen/xai-grok-shell/src/token_economy/config.rs` |
| Effort policy | `…/token_economy/implement_effort.rs` |
| Pacing | `…/token_economy/period_pacing.rs` |
| Ledger | `…/token_economy/ledger.rs` |
| Reconcile | `…/token_economy/reconcile.rs` |
| Facade | `…/token_economy/mod.rs` |
| Store | `crates/codegen/xai-grok-shell/src/grok_oss/mod.rs` |
| Auto-run + clamp | `crates/codegen/xai-grok-pager/src/app/auto_implement.rs` |
| Human submit | `…/app/dispatch/prompt.rs` |
| `/spend` dispatch | `…/app/dispatch/status.rs` (`dispatch_show_spend`) |
| Slash | `…/slash/commands/spend.rs` |
| Credit/usage chrome | `…/views/credit_bar.rs` |
| Limits chrome | `…/views/limits_snapshot.rs` |
| Docs | FORK.md; user-guide `04-slash-commands.md`, `05-configuration.md` |
| Host dual-pin | `~/.agents/skills/implement/SKILL.md` (product may rewrite `--effort`) |

## Tests (red → green evidence)

Policy was landed with observed red then product fix in the earlier wave of this implement run. Final green re-run after docs + “behind linear burn” copy polish:

### Shell (`cargo test -p xai-grok-shell --lib token_economy` / `grok_oss`)
**GREEN (30 + 4 exclusive grok_oss):**

| Contract | Test name |
|----------|-----------|
| Defaults max 3 / desired 2 | `token_economy::config::tests::defaults_match_plan` |
| desired > max rejected | `…::rejects_desired_above_max` |
| Effort out of range | `…::rejects_effort_out_of_range` |
| Economic on clamps 5→3 + toast | `…implement_effort::…economic_on_clamps_effort_5_to_max_3_with_toast` |
| Missing injects desired 2 | `…missing_effort_injects_desired_2` |
| Effort 2 stays 2 | `…effort_2_stays_2_under_default_ceiling` |
| Economic off no rewrite | `…economic_off_leaves_effort_5` |
| Cap master false | `…cap_master_false_leaves_effort_5` |
| Ahead / behind linear burn | `…period_pacing::…ahead_of_linear_burn`, `…behind_linear_burn` |
| On pace | `…half_period_half_usage_is_on_pace` |
| Omit missing bounds | `…missing_bounds_omit` |
| Console-live label | `…console_live_labels_mark_not_principal` |
| Never `$` on pacing | `…never_dollarizes` |
| Ingest idempotent | `…ledger::…ingest_idempotent_on_event_ulid` |
| cost_missing preserved | `…cost_missing_preserved` |
| Gap honesty | `…reconcile::…honesty_when_all_cost_missing` |
| Meters distinct | `…report_names_meters_distinctly` |
| Limits points to /spend | `…limits_section_points_to_spend` |
| DB open / schema version | `grok_oss::tests::open_creates_schema_and_version` |
| Path override | `…path_override_honored` |
| Reopen idempotent | `…reopen_is_idempotent` |
| No secret columns | `…no_secret_columns_in_schema` |

### Pager
**GREEN:**

| Contract | Test name |
|----------|-----------|
| Ceiling / desired / off | `app::auto_implement::tests::economic_implement_effort_policy_ceiling_desired_and_off` |
| Entry path inventory | `…implement_effort_entry_paths_use_shared_helper` |
| Toast copy | `…toast_copy_matches_product` |
| `/spend` action | `slash::commands::spend::tests::spend_command_emits_show_spend` |
| `/spend` registered | `…spend_registered_in_builtins` |
| Credit bar + usage honesty suite | `views::credit_bar::tests::*` (64) |
| Limits snapshot suite | `views::limits_snapshot::tests::*` (34) |

Commands used:

```bash
cargo fmt -p xai-grok-shell -p xai-grok-pager
cargo test -p xai-grok-shell --lib token_economy
cargo test -p xai-grok-shell --lib grok_oss
cargo test -p xai-grok-pager --lib auto_implement
cargo test -p xai-grok-pager --lib spend
cargo test -p xai-grok-pager --lib credit_bar
cargo test -p xai-grok-pager --lib limits_snapshot
```

## §7 acceptance (all green)

### Effort
- [x] Economic on, max 3: entry paths never leave 4/5 live
- [x] Desired inject when missing
- [x] Explicit over-ceiling → clamp + toast
- [x] Economic off → no rewrite
- [x] Config validation desired ≤ max
- [x] Stale docs fixed; user-guide + FORK updated

### Pacing
- [x] Linear-burn math unit-tested
- [x] Credit/status + `/limits` + `/usage` show pacing when bounds exist
- [x] Omit when bounds missing
- [x] Console-live honesty
- [x] Copy says linear burn

### Ledger + grok_oss.db + reconcile
- [x] `$GROK_HOME/grok_oss.db` (path override)
- [x] Token Economy tables; schema versioned; additive
- [x] Local ingest idempotent from usage.jsonl
- [x] Remote samples for Management pulls
- [x] `/spend` + `/limits` section both books + gap honesty
- [x] Fail-open on DB errors
- [x] No secrets in DB
- [x] Session directory layout unchanged

### Product language
- [x] Meters distinct
- [x] Complete thoughts
- [x] Store named **grok_oss.db**

## Dogfood notes (operator)
1. Rebuild `grok-oss` binary so the live TUI picks up Token Economy.
2. With economic mode on: `/implement --effort 5 …` should toast clamp to 3; auto-run residual without effort should inject `--effort 2`.
3. `/limits` / `/usage` / status: free SuperGrok period pacing when period end + type known.
4. `/spend` after some turns: local book rows; remote book needs management key + team id (honest gap without them).
5. Confirm `$GROK_HOME/grok_oss.db` appears; no secrets in the file.

## Residual
- **No half-shipped Token Economy residual.** Full product is in tree + FORK.
- **Operator-gated dogfood only:** live `/spend` remote book with a real management key; rebuild binary for TUI toast/pacing chrome. Not open code work unless dogfood finds a bug.
- **Settings modal:** economic mode description cross-links Token Economy. Full knobs are **`config.toml` `[token_economy]`** (validated load). Dedicated Settings rows per knob are optional polish, not blocking §7 (acceptance is config + product paths + docs).

## Non-goals (unchanged)
C4 free SuperGrok period debit invent; model reasoning-effort cap; global non-implement specialist fan-out; migrating worktrees.db into grok_oss.db.
