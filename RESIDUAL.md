# Open residual (human intent and unfinished honesty)

Only **open** items. Finished work lives in [`FORK.md`](FORK.md), process docs,
or code — not only here.

## Closed this campaign (see FORK, not novels)

- **Soft interject never cancels** — mid-turn Interject / queue `[Interject]` /
  empty-Enter interject inject into the running turn only. Cancel = Esc/stop.
  Held + background subagents: status `… Interject to force`, row Interject
  force-drains. User tip/status copy: **Enter to interject**. Esc on cancel-turn
  panel dismisses only (no parent cancel). FORK Product bullets +
  `interject_contract_*` / cancel-turn Esc tests.
- **Todo UI no longer wiped on auto-compact** — leave board as-is on
  `AutoCompactCompleted`. Test: `auto_compact_completed_preserves_todo_board`.
- **plan.json honesty + resume board** — compact writes live Resources
  `TodoState` (not empty); resume `RestoreTodoBoard` seeds from
  `tool_state.json` / `plan.json` and re-emits Plan. User-guide `17-sessions`
  documents SoT. Helpers + unit tests in todo module.
- **`ask:*` auto-seed** — real user turns seed protected `ask:<prompt_id>`
  (cap 20); protected on `merge: false`. FORK + todo unit tests.
- **Host bulk-replace MVP + C3 edit storm** — shell
  `~/.grok/hooks/block-bulk-replace.{sh,json}` + Edit storm
  `block-bulk-replace-edit.py` (N=5, T=120s, state under
  `~/.grok/bulk-edit-state/`). GPG hooks unchanged (suite green). Product
  `deny_replace_all` / apply_patch file cap still open if needed.
- **Live-apply auto-compact threshold** — Settings
  `auto_compact_threshold_percent` is `restart_required: false`. Commit path:
  AppView + PersistSetting (disk) → ACP `x.ai/auto_compact_threshold_changed`
  → `SessionCommand::SetAutoCompactThreshold` →
  `SessionActor::apply_auto_compact_threshold` (Cells; shared with model
  switch). Toast no longer says “restart to apply”. Tests:
  `set_auto_compact_threshold_command_updates_gate`,
  `set_auto_compact_threshold_toast_no_restart`, settings_e2e
  `auto_compact_threshold_renders_under_session…` asserts
  `!restart_required`.
- **Parked sendable-wait bare Enter (document-only)** — product law remains
  soft Interject for mid-turn steer. Cancel-and-send on **blocked wait +
  empty queue + plain Enter with text** is intentional “unblock
  immediately,” not a soft-law bug. User-guide `03-keyboard-shortcuts` +
  FORK Soft interject bullet. No code change this pass.

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

5. **Human: land `fix/interject-no-cancel`**  
   Soft-interject + plan/ask honesty + live-apply auto-compact + parked-wait
   document decision are on this branch and **staged** (agent does not commit).
   **Full `just check` green** (2026-07-25): exit 0, “CI passed”, nextest
   `26762 passed` (403 skipped); log
   `/tmp/grok-1000/just-check-3d0a3903.log`. Focused contracts green (shell
   interject 31; pager interject/force_interject/cancel_turn/auto_compact 129+;
   `set_auto_compact_threshold_command_updates_gate`;
   `auto_compact_completed_preserves_todo_board`). Remaining: human
   `git commit -S` on a real TTY, then rebuild/install, push/PR when ready.
   No invent recon/onto on this branch.

6. **Host / product bulk-replace follow-ups**  
   Shell + C3 edit-storm installed. **Product `deny_replace_all` / apply_patch
   file cap skipped this pass** (host hooks already cover shell bulk-replace +
   edit storm; no clear product schema gap). Still optional: default
   `GROK_DENY_REPLACE_ALL=1`, Python/node rewrite one-liners. Design:
   `/tmp/grok-1000/bulk-replace-acp-prevention.md`. Residual only if multi-file
   surgical storms persist after host C3.

7. **Internal send_now names**  
   Behavior is soft Interject; symbols still say `send_now_*` /
   `try_send_now_queued_from_prompt` / `force_interject`. Cosmetic rename only.

## Highest-value next (to unblock parallelization)

| Rank | Work | Why |
|------|------|-----|
| 1 | Human `git commit -S` on staged `fix/interject-no-cancel` | Gate already green; unblocks push/PR / rebuild |
| 2 | Onto join + PR / import decision | Upstream land path (separate; not this branch) |
| 3 | Optional bulk-replace product gates if host C3 insufficient | Only if multi-file surgical storms persist |

## Validate honesty (agent-runnable)

```bash
# Soft interject contracts
cargo test -p xai-grok-shell --lib -- interject handle_interject
cargo test -p xai-grok-pager --lib -- interject force_interject cancel_turn queue_edit_routing

# Live-apply auto-compact threshold
cargo test -p xai-grok-shell --lib -- set_auto_compact_threshold_command_updates_gate
cargo test -p xai-grok-pager --lib -- set_auto_compact_threshold_toast_no_restart
cargo test -p xai-grok-pager --test settings_e2e -- auto_compact_threshold_renders_under_session

# Compact does not wipe todos (UI)
cargo test -p xai-grok-pager --lib -- auto_compact_completed_preserves_todo_board

# plan.json / resume / ask helpers + protect
cargo test -p xai-grok-tools --lib -- todo

# Held-queue Interject force + tip copy
cargo test -p xai-grok-pager --lib -- idle_with_subagents_and_held force_drain_dispatch send_now_tip

# Full local quality gate (before push)
just check

# Host bulk-replace self-check (canned stdin)
printf '%s' '{"toolInput":{"command":"sed -i s/a/b/ f"}}' | ~/.grok/hooks/block-bulk-replace.sh; echo exit=$?   # expect 2
printf '%s' '{"toolInput":{"command":"sed s/a/b/ f | head"}}' | ~/.grok/hooks/block-bulk-replace.sh; echo exit=$? # expect 0
GROK_BULK_EDIT_SELFTEST=1 ~/.grok/hooks/block-bulk-replace-edit.py   # expect 0
~/.git-hooks/test-unsigned-guard.sh   # GPG must stay green

# Process pins still on branch
./scripts/assert-process-pins.sh
```

## Not residual (resolved elsewhere)

- CI checks-only (no release package in GHA) — FORK + justfile + AGENTS  
- `just check` ≡ `just ci` — justfile  
- put-history is cherry-pick — upstream-history + onto log  
- Auto-implement **appends** after existing local queue — `auto_implement.rs` + FORK  
- GPG / no bulk replace / no agent commit defaults — AGENTS.md  
  (host shell + C3 edit-storm now also enforced — see Closed)  
- Import recon process pins (`FORK_PATHS` expanded + post-restore assert) —
  `scripts/import-upstream-export.sh`, `scripts/assert-process-pins.sh`,
  `just upstream-assert-process-pins`, FORK § recon, upstream-history checklist,
  `doc/dev/research/fork-paths-hardening-2026-07-24.md`  
- Todo levels product surface (`priority`/`meta` writable; `merge: false`
  keep-unless-mentioned for protected prefixes including `ask:`; light
  `[kind]` badge) — FORK + `doc/dev/research/todo-levels-product-2026-07-24.md`  
- Session notes channel (`/note` not a pending prompt; list + `/tasks`
  count) — FORK + `doc/dev/research/notes-channel-2026-07-24.md` +
  user-guide `04-slash-commands`
- Git recon depth (host `git-recon` skill + `scripts/recon-status.sh` +
  `just recon-status` + FORK_PATHS/assert pin + optional Rhai status
  workflow) — FORK Process + `doc/dev/research/recon-status-script-2026-07-24.md`
  + `doc/dev/research/git-recon-skill-created-2026-07-24.md`
- `allow_worktree` OSS default `false` — `SubagentsConfig` Default + serde
  (`default_allow_worktree` → false); empty config force-none; opt in with
  `allow_worktree = true`. Force-none path + tests green. User-guide
  migration notes in `05-configuration` + `16-subagents`. FORK Process +
  `doc/dev/research/task-worktree-pins-2026-07-24.md`
- plan.json empty-on-compact + discarded plan_state on resume — **fixed**
  this pass (see Closed + FORK)
- Live-apply auto-compact threshold — **fixed** this pass (see Closed + FORK)
- Parked sendable-wait bare Enter vs pure soft law — **decided document-only**
  (see Closed + FORK + user-guide)


## Local quality before push

```bash
just check    # or just ci
```
