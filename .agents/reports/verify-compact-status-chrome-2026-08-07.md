# Verify: Design A compact free SuperGrok period meter (status chrome)

**Date:** 2026-08-07
**Scope:** Read-only verification of compact status meter text (not `limits --json`).
**Result:** **PASS** (unit + pure helpers). Interactive TUI not required for step-2 proof.

---

## 1. How compact meter text is produced

| Layer | Path | Role |
|-------|------|------|
| Pure string | `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` | Builds the ASCII meter string |
| SuperGrok line paint | same file | Theme color + optional pacing chip |
| Status chrome wire-up | `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` (~1461–1524) | Pushes `"credits"` into the status bar |

### Exact function names

1. **`compact_meter_text_for_live_identity`**
   Thin wrapper → `compact_meter_text_for_live_identity_with_active_poll(..., active_supergrok_poll_auth_failed: false)`.

2. **`compact_meter_text_for_live_identity_with_active_poll`**
   Design A pure policy (active meter only):

   | Live identity | Condition | String |
   |---------------|-----------|--------|
   | Console | prepaid known | `console · $N` or `console · $N.NN` |
   | Console | no prepaid | `console · {gap.as_display_str()}` |
   | SuperGrok | **active poll AuthFailed** | `...%` |
   | SuperGrok | included unknown (`!included_usage_known`) | `...%` |
   | SuperGrok | included ≥ 100% + extras cents > 0 | `SuperGrok extras · $X.XX` |
   | SuperGrok | included ≥ 100% + no extras | `{pct:.0}%` (e.g. `100%`) |
   | SuperGrok | included &lt; 100% (room) | `{pct:.0}%` (e.g. `6%`, `42%`) |

3. **`credit_bar_line_for_session`**
   SuperGrok-primary path for a warm `CreditBalance`:
   - If `!included_usage_known` **or** process active poll AuthFailed → **`credit_bar_loading_line`** → fixed `...%` (dim gray).
   - Else calls pure helper with `SamplingIdentityKind::SuperGrokSession`, balance usage/prepaid, and process AuthFailed flag.
   - Free-period `%` path may append ` · {pacing_chip}` when chip exists and length ≤ 28; extras `$` path does not.

4. **`credit_bar_loading_line`** → always `"...%"` (ASCII dots only; no unicode ellipsis).

5. **`active_supergrok_poll_auth_failed_from_process`** → process-local AuthFailed for the **active** SuperGrok principal (sibling fill must not paint healthy free-period success).

6. **Render status bar** (`render.rs`): if not `chat_kind`:
   - Console live → pure helper with console prepaid/gap.
   - SuperGrok + `credit_balance` Some → `credit_bar_line_for_session`.
   - SuperGrok + cold balance None → `credit_bar_loading_line` (`...%`).

---

## 2. What healthy free-period vs cold looks like

| Scenario | Compact chrome string |
|----------|------------------------|
| **Healthy free SuperGrok period** (live SuperGrok, poll OK, included known, &lt; 100%) | **`6%`**, **`42%`**, etc. (`{usage_pct:.0}%`) |
| Operator dogfood: business SuperGrok live, `pollSucceeded`, free period **6%**, `live_poll` | **`6%`** (pure path; optional pacing suffix only via `credit_bar_line_for_session` when a short chip is present) |
| Free period full + SuperGrok $ extras | `SuperGrok extras · $4.53` (example) |
| Free period full + no extras | `100%` |
| **Cold / AuthFailed** (unknown included, or active poll AuthFailed, or no balance yet) | **`...%`** |
| Console live | `console · $…` / honest gap — never bare SuperGrok `N%` / `...%` |

Named unit contracts in `credit_bar.rs`:
- `compact_status_active_auth_failed_not_sibling_free_period_pct` — 6% sibling-looking reading + AuthFailed → **`...%`**, not `6%`
- `compact_status_supergrok_free_period_room_shows_pct_not_extras` — 42% room + extras on account → **`42%`**
- `compact_status_supergrok_on_extras_shows_dollars_not_free_period_pct`
- `compact_status_supergrok_full_without_extras_shows_100_pct`
- `credit_bar_loading_line_is_honest_placeholder` → `...%`

Render-level Design A (buffer paint, still no interactive TUI):
- `status_bar_supergrok_on_extras_paints_dollars_not_free_period_pct`
- `status_bar_console_live_ignores_cached_supergrok_free_period_pct`

---

## 3. Commands + exit codes

```bash
cargo test -p xai-grok-pager --lib -- compact_status_active_auth_failed free_period credit_bar compact_meter 2>&1 | tail -80
```

| | |
|--|--|
| **Exit** | **0** |
| **Result** | **78 passed**; 0 failed; 8296 filtered out |
| **Key tests** | `compact_status_active_auth_failed_not_sibling_free_period_pct` ok; free-period room / extras / full / loading / console-live all ok; two render Design A status-bar tests ok |

Targeted re-check:

```bash
cargo test -p xai-grok-pager --lib -- \
  compact_status_supergrok_free_period_room_shows_pct_not_extras \
  compact_status_active_auth_failed -- --nocapture
```

**Exit 0** — 2 passed.

No new product code or temporary print tests added. Pure helper already encodes the dogfood case:

```text
compact_meter_text_for_live_identity(
  SuperGrokSession, included_known=true, 6.0, …, extras=any, auth_failed=false
) → "6%"
```

AuthFailed cold:

```text
compact_meter_text_for_live_identity_with_active_poll(
  SuperGrokSession, true, 6.0, …, active_supergrok_poll_auth_failed=true
) → "...%"
```

---

## 4. Is interactive TUI required?

**No** for verifying Design A step 2 (compact free-period chrome text and AuthFailed honesty).

| Proven without interactive TUI | Not proven by these tests |
|--------------------------------|---------------------------|
| Exact meter strings (pure functions) | Live dual-OAuth poll wiring into `CreditBalance` / process AuthFailed flags on a running dogfood host |
| SuperGrok line path + loading placeholder | Layout pixels, hover, full status row composition on a real session |
| Render buffer paint for extras / console-live (unit draw helpers) | That the operator’s **installed** binary matches this tree |

Interactive TUI is only needed if the operator wants a visual eyeball of the live status chrome on their dual SuperGrok session. Code contracts for “healthy 6% vs cold `...%`” are fully unit-proven.

---

## 5. Uncommitted / not-yet-installed product changes

`git status` shows **large uncommitted product diffs** that include this chrome work, among others:

- `crates/codegen/xai-grok-pager/src/views/credit_bar.rs` (major; +~500 lines in diffstat)
- `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs`
- `limits_cmd.rs`, `limits_honesty.rs`, `limits_snapshot.rs`
- Shell auth / billing / sampler paths (`xai-grok-shell`, `xai-grok-sampler`, sampling-types)
- User-guide auth / slash docs; `AGENTS.md` / `FORK.md` / `RESIDUAL.md`

**Implication:** tests above run against the **current working tree**. If the operator is dogfooding an older installed `grok` / pager binary, they may not see Design A compact chrome until they rebuild and reinstall from this tree. Do not assume the live TUI already has this without rebuild.

---

## Bottom line

| Item | Status |
|------|--------|
| Design A pure strings | Implemented; healthy free-period = `N%`; AuthFailed/cold = `...%` |
| Unit filter command | **PASS** exit 0, 78 tests |
| Dogfood 6% business SuperGrok | Pure path → **`6%`** when poll OK and included known |
| Need open interactive TUI for proof? | **No** |
| Install risk | **Yes** — uncommitted product chrome may not be in the binary dogfood is running |
