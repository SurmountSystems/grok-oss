# Join: RESIDUAL limits-first honesty update

**Date:** 2026-08-02
**Scope:** D0 residual docs only (`RESIDUAL.md` §4 + Highest-value next + Validate)
**No product code.**

## Inputs

| Source | Role |
|--------|------|
| `.agents/joins/impl-slice1-poll-history-2026-08-02.md` | Slice 1 shipped |
| `.agents/joins/impl-slice3-m3-postpaid-2026-08-02.md` | Slice 3 M3 shipped |
| `.agents/joins/impl-slice4-extras-before-console-2026-08-02.md` | Slice 4 C5 shipped |
| `.agents/joins/slice2-dogfood-g4-2026-08-02.md` | Was missing at first residual edit; **exists now** (see Follow-up) |
| `.agents/plans/limits-first-ideal-2026-08-02.md` | Criteria C1–C7 |
| `.agents/plans/limits-first-api-fix-section-2026-08-02.md` | Slice numbering S1/M3/C5 |

## What changed in `RESIDUAL.md`

### §4 dual-auth / limits

1. **Header status:** core meters + limits-first Slices **1 / 3 / 4** shipped; live C4 dogfood + edges still open (not "series/dogfood open" as the only leftover).
2. **Operator pin fronted:** **Limits before credits** always; Design A; both halves still intended.
3. **Auto order text corrected:** no longer "ExhaustedAll → console" as the only post-included path. Documents after-burner: included full + SuperGrok $ extras > 0 → SuperGrok primary / console failover; extras 0/unknown → console primary. Preemptive mark note aligned with Slice 4 (do not mark while known positive extras under auto_use).
4. **Shipped block for limits-first campaign:** Slice 1 poll history / flat honesty; Slice 3 M3 postpaid OAuth vs API + C6 note; Slice 4 extras-before-console (C5). Joins linked under `.agents/joins/`.
5. **Half B heading:** prepaid + M3 postpaid shipped; series charts still optional.
6. **Still open rewrote honestly:**
   - Slice 2 live dogfood for **C4** (no join file; prior flat ~65% / $100.29 → **do not invent debit**)
   - F1b console Usage $ pain / attribution (M3 surface shipped; live re-dogfood open)
   - Default `auto_use_included_limits` for new installs (optional)
   - Bare resolve / console-edge audit
   - Soft OAuth Usage honesty polish
   - M6 series optional; prepaid dogfood done; TUI postpaid cache soft
7. **Meters distinct** line adds console team postpaid OAuth/API class (Usage $).
8. **Highest-value next inside §4** re-ranked: C4 dogfood first, then bare resolve, default auto_use, honesty polish, M6/defaultCredits.
9. **Plans:** points at `limits-first-ideal` + `limits-first-api-fix-section` (older dual-auth plans kept as older).

### Highest-value next (global table)

- Rank 1 = live Slice 2 C4 dogfood (unblocks parallel honesty branches).
- Ranks 2–5 = bare resolve, default auto_use, soft polish, M6/defaultCredits.
- Parallelization note: dogfood can run while bare-resolve implementer works disjoint paths; no dual writers on resolve/rank files.
- Explicit: **do not claim SuperGrok included debit proven** without a moving series.

### Validate honesty

- Added cargo filters **2d / 2e / 2f** for Slices 1, 3, 4 (from joins).
- Added **2g** live `limits --json` dogfood checklist (not cargo).

## Explicit non-claims

- Did **not** invent C4 SuperGrok debit as proven.
- Did **not** invent Slice 2 dogfood join (file absent **at this join’s
  write time**).
- Did **not** move lasting product truth to FORK (docs residual only this turn).

## Follow-up (same day): Slice 2 dogfood join exists

**2026-08-02 later:** dogfood join is on disk at
[`.agents/joins/slice2-dogfood-g4-2026-08-02.md`](slice2-dogfood-g4-2026-08-02.md).
Summary for residual: **C4 SuperGrok included debit still FAIL** (flat 65%
included / Build 54% / SuperGrok $ extras $100.29); **C1/C3 pass** for this
product path; residual **branch 2b** (server lag / no proven debit); Slice 4
C5 code-only not live-proved; `flat_poll` can fire from process history; M3
cache OAuth ~$202 vs API ~$6. `RESIDUAL.md` Open §4 + Highest-value next +
Validate 2g were updated to cross-link that join and re-rank highest-value
next around 2b + optional live recheck. Still **do not invent SuperGrok
debit.**

## Validation agents can run

```bash
cargo test -p xai-grok-shell --lib included_poll_history
cargo test -p xai-grok-pager --lib flat_poll
cargo test -p xai-grok-shell --lib xai_management
cargo test -p xai-grok-pager --lib limits_cmd
cargo test -p xai-grok-pager --lib limits_honesty
cargo test -p xai-grok-shell --lib -- auto_order_keeps_supergrok auto_after_included_and_extras resolve_auto_after_included_exhausted resolve_enforced_auto_use_included_limits
# live (rebuilt binary):
grok-oss limits --json
```
