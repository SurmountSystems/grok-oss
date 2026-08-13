# Residual after the onto-xai / Grok Build 1.0 mop (2026-08-12)

Read-only recon. Docs can lie. Evidence is the tree plus GitHub compare.

## What this tree is

- Branch `onto-xai/b13fa526f511` at `09c407e2` (message `merge upstream`). Same SHA on `origin/onto-xai/b13fa526f511`.
- Single parent `241f6f12`. This is the mop commit, not a `join-main-into-onto` merge (`-s ours`).
- This stack last used xAI export `b13fa526f511` (tree `0f26f408…` in Live stack). Product crate versions here are `1.0.0`. Local `xai-org/main` is still that same SHA (stale fetch).
- Operator: CI green; this mop committed and pushed. Do not start onto, join, or land.

## Still real merge residual

1. **Rejoin Surmount `main` is still owed.** GitHub `main...onto-xai/b13fa526f511` is **diverged**: onto is 51 ahead and **1 behind**. Merge-base is still `a1515fe1`. Current `origin/main` is `f17e84d8` (`fixes 2 (#31)`). That commit is **not** an ancestor of the onto tip. First join (`ea7a9ad5`) only covered old `main`. `09c407e2` did not rejoin. No open PR for this head. `impl:upstream-rejoin-main` remains real **graph** work so a landable PR can compare. Operator-gated. Do not run the join from this report.

2. **A newer xAI export exists (second onto, parked).** Live `xai-org/grok-build` `main` is `e5fd4816` (2026-08-12, crate versions `1.0.3`). This tree last stacked `b13fa526` (Grok Build 1.0 / `1.0.0` here). Residual already says do not start a second onto from the mop. It is the next process job after the operator chooses, not unfinished mop code.

3. **Formal Surmount-first import ledger is still open and stale.** `docs/upstream-import-log.md` still lists `3af4d5d…` as pending. Onto + join is not the same as that import. Decide later. Not this mop.

4. **PTY grandchild-kill flake.** `close_pty_kills_a_background_grandchild` is still in `pty_session.rs`, not ignored. Historical TRY1 timeout / TRY2 pass. Reliability leftover from this onto wave, not a hard CI unit fail.

5. **Nucleo leftovers after the shipped reuse fix** (not another storm). Per-root matchers (2 workers each), path identity is the string as passed (two spellings can still make two pools), `cleanup_stale` only on the next `open` (no timer), TUI `@` is still one nucleo per `PromptWidget` after first `@`. Storm fix is in the tree (`reuse_existing`; poll does not refresh `last_activity`). Live PIDs need a new binary.

## Already shipped / docs stale

- CI-239 unit mass, 1.97.1 pin (`rust-toolchain.toml` + fenix), package clippy mop, nucleo reuse-per-root: **in the tree**. Residual Open bullets that call these “shipped, not open” match the code.
- `09c407e2` is the mop, not an unfinished commit. Live stack still says onto tip `9060f502`, join `ea7a9ad5`, `origin/main` `a1515fe1`. Those SHAs are old.
- Residual Open item 8 (“merge staged or about to be signed”) is stale. The mop landed. The missing piece is rejoin of **current** `main`.
- Wave-status “rejoin held for mop commit” is stale. Mop is on the remote tip.

## Dogfood / operator-gated

- Install or `/rebuild`, quit old TUIs, reopen `grok-oss` (checklist `.agents/reports/d0-dogfood-checklist-2026-08-09.md`). Required for nucleo workers and chrome. Not a code hole.
- Rejoin + PR to `main`, and any second onto, stay human TTY process. Not started here.

## Not merge residual

Older product / process Open items: dogfood chrome checks, agentic fmt/clippy ACP, thoughtful todos, agent-written `plan.md` freeform menus, Sapient Experience / billing language, C4 server ticket, pause-without-cancel, import-ledger policy, send_now rename.

## Verdict

This mop’s product and CI work is in the pushed tip. What is still real after that is: **rejoin current `main` (1 commit behind, not ancestor)**, **PTY flake reliability**, **nucleo leftover edges**, **operator dogfood install**, and a **newer xAI `e5fd4816` / 1.0.3 export** if the operator wants a second onto later.
