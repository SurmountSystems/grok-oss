# Open residual (human intent and unfinished honesty)

Only **open** items. Finished work lives in [`FORK.md`](FORK.md), process docs,
or code — not only here.

## Open

1. **Formal content import of current xAI tip into Surmount `main`**  
   Tip `3af4d5d…` / tree `e595174…` is logged as *pending* in the import ledger.
   The `onto-xai/3af4d5d39897` stack + **join-main** (`-s ours`) is the landable
   product path (PR onto → `main`). That is **not** the same as a reviewed
   import-ledger absorption under Surmount-first parents. Decide when import
   still needs its own PR/log row.

2. **xAI history stability**  
   Unknown whether force-exports continue. Prefer stacking product on their tip
   when they rewrite; do not promise they will stop.

3. **Finish join + PR for current onto tip**  
   Merge of `main` into onto is staged or about to be signed; docs/script for
   the workflow land in a follow-up commit; then push and open PR to `main`.

4. **Confidence notes**  
   If a process detail is still fuzzy after reading FORK + upstream-history,
   ask a human rather than inventing policy. Write the answer here only while
   it stays open; then migrate the lasting rule into FORK or AGENTS.

5. **Live-apply auto-compact threshold (settings → open session)**  
   Settings still mark `auto_compact_threshold_percent` as restart-required:
   open sessions keep the threshold resolved at spawn / last model switch.
   Slice 1 fixed catalog undercut + banner honesty; live Cell update on
   settings commit (mirror model-switch / economic-mode patterns,
   `restart_required: false`) is still open.

6. **Todo levels product surface (session board)**  
   Host skills pin namespaces (`plan:*` `impl:*` `pr-N:*` `recon:*`
   `residual:*`) and merge policy. Product still lacks a first-class
   namespaced todo API / gate that rejects foreign-prefix wipe. Optional
   later: runtime guard or docs-only.

7. **Notes / join channel for child artifacts**  
   Join-on-disk is process law; no dedicated product “notes channel” UI for
   L2 child summaries yet. Skills write scratch paths; residual if a pane
   is desired.

8. **`allow_worktree` remaining**  
   Skill half done: `/execute-plan` shared-cwd auto-adapt (host skill + dual-pin).
   Still open: optional product default `allow_worktree = false` for OSS installs
   (config key + force-none already ship; default flip not done).

## Not residual (resolved elsewhere)

- CI checks-only (no release package in GHA) — FORK + justfile + AGENTS  
- `just check` ≡ `just ci` — justfile  
- put-history is cherry-pick — upstream-history + onto log  
- Auto-implement **appends** after existing local queue — `auto_implement.rs` + FORK  
- GPG / no bulk replace / no agent commit defaults — AGENTS.md  
- Import recon process pins (`FORK_PATHS` expanded + post-restore assert) —
  `scripts/import-upstream-export.sh`, `scripts/assert-process-pins.sh`,
  `just upstream-assert-process-pins`, FORK § recon, upstream-history checklist,
  `doc/dev/research/fork-paths-hardening-2026-07-24.md`  


## Local quality before push

```bash
just check    # or just ci
```
