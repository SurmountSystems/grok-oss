# Closed campaign: soft interject + plan/ask honesty (2026-07)

**Role:** D2 append/campaign diary — not always-loaded residual.  
**Open residual:** [`RESIDUAL.md`](../../../RESIDUAL.md) (open section only).  
**Lasting product claims:** [`FORK.md`](../../../FORK.md).

This file holds closed writeups and a resolve index that used to bloat
`RESIDUAL.md`. Do not re-list these under Open residual.

---

## Closed this campaign

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
  `resources_state.json` / `plan.json` and re-emits Plan. User-guide
  `17-sessions` documents SoT. Helpers + unit tests in todo module.
- **Default agent todos usage** — base prompt Planning/`todo_write`; first
  Plan auto-opens pane once; fork copies `resources_state.json`.
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

---

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

---

## Validate honesty (full agent-runnable block)

Short one-liners live in [`RESIDUAL.md`](../../../RESIDUAL.md) § Validate honesty.
Full block for this campaign:

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
