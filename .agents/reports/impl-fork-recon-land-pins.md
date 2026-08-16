# Report: restack land pins (paint filters)

**Date:** 2026-08-13  
**Slice:** process pins only. No product crate edits. No git add/commit.

Diagnosis source: `.agents/reports/fork-loss-postmortem-2026-08-13.md` §6.

## What was pinned

### 1. `/home/hunter/Projects/surmount/grok-build/FORK.md`

Section **Upstream regression filters**, immediately after the existing
"product seams inside `xai-grok-*` are not path-restored" paragraph.

- Added the standing **Paint filters (restack land)** paragraph from the
  postmortem (complete sentences, **included SuperGrok period limits**, no
  em dash). A restack is not done until those paint filters exist and pass.
  Deleting a red catalog test is not a restore.
- Tightened the sentence above it: after recon, run assert, the catalog
  block, and the paint-filter name check. `just check` cannot fail a
  deleted catalog test.
- Extended the operator cheat sheet with the owed/keep/restore cargo
  identifiers (name must exist; missing `fn` means land failed).

### 2. `/home/hunter/.agents/skills/git-recon/SKILL.md`

Land step 9 is now assert, then catalog cheat sheet, then `rg` that named
`fn` still exist (missing `fn` = land failed), then the dogfood screenshot
list: Human/agent rails, plan five CTAs, included SuperGrok period limits
compact meter (click opens `/limits`), SIGUSR1 fleet still alive after a
**failed** install. Do not accept "compile mop re-applied seams" without
those.

Also updated: ownership-split land line, `recon:land` board meaning,
`recon:land` command block, anti-pattern row.

Same land comments in
`/home/hunter/.agents/skills/git-recon/references/hand-commands.md`
(the copy-paste land template).

### 3. `/home/hunter/Projects/surmount/grok-build/doc/dev/upstream-regression-filters.md`

Cheat sheet now names the owed/keep/restore filters (no Rust tests written):

| Identifier | Land |
|------------|------|
| status bar `"credits"` + compact included SuperGrok period limits meter | **Owed** (no paint `fn` yet; `credit_bar` helpers do not count) |
| `hit_credits` click → `ShowLimits` | **Owed** |
| `plan_approval_footer_paints_five_cta_vocabulary` | **Keep** (old `soft_park_draw_paints_panel_*` gone; do not revive) |
| `sampling_config_auto_use_*` | **Restore** |
| `auto_compact_completed_preserves_todo_board` | **Restore** |
| `hide_header_zeroes_*` | **Restore** (serde default tests are not paint) |
| `failed_install_must_not_replace_or_signal_peers` | **Keep** |
| `version_without_tty` | **Keep** (`xai-grok-pager-bin --test version_without_tty`) |

Also: third **Paint / dogfood** layer in the survive-how table; operator
cheat sheet `rg` + cargo block; user-guide onto check now includes
`/limits` (zero hits = failed land).

### 4. Dual-pin HITL (same law, not a novel)

`/home/hunter/Projects/surmount/grok-build/docs/upstream-history.md`
review checklist: product-filter bullet now requires the name-existence
check; added a paint/dogfood checkbox; user-guide bullet includes
`/limits`. Left the old "or at least `just check`" lie off that path.

Did **not** edit `AGENTS.md`.

## Leftovers

- Status compact meter paint + `ShowLimits` click tests are still **owed**.
  Another implementer is restoring the meter. This slice did not write
  those tests.
- Catalog **restore** names (`sampling_config_auto_use_*`,
  `auto_compact_completed_preserves_todo_board`, `hide_header_zeroes_*`)
  are law. Several of those `fn`s are still missing in this tree. The next
  land is supposed to go red, not silent.
- FORK product inventory still says "free SuperGrok period" in older
  bullets. Language residual, not this pin.
- Host skill lives outside product git. Operator owns any
  `~/.agents/skills` commit. Not synced to `~/.grok/skills`.
- Did not run `just check` or the catalog cargo blocks.
- Did not edit product crates or `line_viewer.rs`.
