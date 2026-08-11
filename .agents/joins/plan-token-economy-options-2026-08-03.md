# Join: plan Token Economy options

**Date:** 2026-08-03
**Role:** plan subagent (read-only product tree; plan + join only)
**Plan path:** [`.agents/plans/token-economy-options-2026-08-03.md`](../plans/token-economy-options-2026-08-03.md)

## Inventory findings (short)

1. **Economic mode is context-only today** — 200k soft-cap (`economic_mode.rs`); default on. Stale comment still claims implement effort clamp-to-1; **code is identity** (`clamp_implement_effort_for_economic_mode`); FORK/docs say explicit `--effort` is honored.
2. **Implement-loop effort 1–5** is the host implement skill’s reviewer fan-out, not model reasoning effort. Auto-run lives in `app/auto_implement.rs` + `[ui] auto_run_implement`.
3. **Limits / dual-auth stack is largely shipped** — free SuperGrok period %, SuperGrok top-up $, console prepaid/postpaid, default credits, usage series POST, multiproc rate-limit + poll history. Residual §4 still has C4 debit (server) and dogfood rebuild, not a blank slate.
4. **Credit bar** shows compact free SuperGrok period `XX%` only (`credit_bar.rs`); period end + weekly/monthly labels exist. **No ahead/behind pacing** math or chrome. No in-tree “GRLD” product name.
5. **Local spend** = per-session `usage.jsonl` (tokens + optional cost ticks). **Remote spend** = Management prepaid/postpaid/usage series. No double-entry ledger or cross-book UI.
6. **Sessions are filesystem jsonl** under `$GROK_HOME/sessions/…`, not a product SQLite session DB. Other SQLite: worktrees, memory index. Prefer **new** `$GROK_HOME/token_economy.db` (or similar) for ledger to keep upstream session layout untouched.
7. **Docs vs code gap** on economic × effort must be fixed when implementing a real ceiling (proposed max 3 + configurable desired effort).
8. Period-reset sticky-console and network-economics were fixed separately (joins same day); not re-planned as token economy.

## Open questions for operator

See plan §4 Q1–Q5 (explicit effort honor vs clamp; desired default 1 vs 2; period = billing period vs calendar week; reconcile window; ship order).

## Recommended next

- **Wait for plan CTA or freeform approve.** Do **not** start product implementation.
- Parent: present plan via `exit_plan_mode` / panel when coordinating; do not invent chat approve menus.
