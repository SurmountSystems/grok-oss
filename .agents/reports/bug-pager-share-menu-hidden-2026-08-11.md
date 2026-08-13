# Pager share kill-switch / menu_hidden restore

**Date:** 2026-08-11
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Cluster:** Residual 3 from `.agents/reports/bug-pager-mass-fail-root-2026-08-11.md`
**Agent:** L2 implementer

---

## Contract

`/share` is **menu_hidden** (still dispatchable via `get_for_dispatch`) until sharing is enabled. It must **not** use hard `hidden`, which blocks typed dispatch and breaks the temporary client kill-switch path (typed `/share` must reach the pager disable message instead of PassThrough).

Named red: `settings_update_sharing_enabled_true_stays_forced_off`
(after kill-switch: `get("share")` is `None`, `get_for_dispatch("share")` is `Some`).

---

## Root

Onto half-merge left `set_share_visible` wired through hard `set_command_visible("share", …)` and `CommandRegistry::new` no longer seeded `menu_hidden` with `"share"`.

Monorepo (`dd04f397` / pre-regression):

- `new()`: `menu_hidden.insert("share")`
- `set_share_visible`: always `hidden.remove("share")`; toggle `menu_hidden` only

---

## Fix

**File:** `crates/codegen/xai-grok-pager/src/slash/registry.rs`

| Change | Detail |
|--------|--------|
| `CommandRegistry::new` | Seed `menu_hidden` with `"share"` (menu-only default) |
| `set_share_visible` | Always clear hard `hidden` for `"share"`; toggle `menu_hidden`; rebuild triggers |
| Unit tests | Restore monorepo expects: default menu-hidden but dispatchable; hide/reveal stays menu-only; `get_for_dispatch_respects_hard_gates` uses `/dashboard` (not `/share`) as hard-gate example |

No FORK/docs dual-pin: no product contract text claimed hard-hide for share.

No git commit/add/push.

---

## Verification

| Filter / suite | Result |
|----------------|--------|
| `settings_update_sharing_enabled_true_stays_forced_off` | **ok** (was red: typed `/share` did not resolve) |
| `set_share_visible_hides_and_restores_share_command` | **ok** |
| `get_for_dispatch_respects_hard_gates` | **ok** |
| `menu_hidden_is_menu_only_and_still_dispatches` | **ok** |
| `restricted_wins_over_visible_setters` | **ok** |
| `slash::registry::tests` (full module) | **28 passed / 0 failed** |

Commands (max-nice via `scripts/run-nice.sh`):

```bash
scripts/run-nice.sh cargo fmt -p xai-grok-pager
scripts/run-nice.sh cargo check -p xai-grok-pager --lib
scripts/run-nice.sh cargo test -p xai-grok-pager --lib -- \
  settings_update_sharing_enabled_true_stays_forced_off \
  set_share_visible_hides_and_restores_share_command \
  get_for_dispatch_respects_hard_gates \
  menu_hidden_is_menu_only restricted_wins_over_visible_setters
```

---

## 5-line summary

1. Red: kill-switch test expected dispatchable menu-hidden `/share`; hard `hidden` returned `None` from `get_for_dispatch`.
2. Restored monorepo `set_share_visible` (menu_hidden only) + default `menu_hidden.insert("share")`.
3. Registry unit tests realigned to monorepo menu-only contract (not hard hide).
4. Named kill-switch + registry share tests green.
5. Residual cluster 3 closed; no FORK pin needed.
