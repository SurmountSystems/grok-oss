# Report: auto-resume canceled turn + soft stop + options GUI

**Date:** 2026-08-03 (verify closeout 2026-08-04)
**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Branch work:** soft stop, resume-on-restart, Settings Token Economy + resume

## Product surfaces shipped

### 1. Auto-resume after explicit cancel (on restart)

| Contract | Behavior |
|----------|----------|
| When | Last top-level turn was **explicit user cancel** (Esc/stop), not clean success, not fearless pause |
| Persist | `$GROK_HOME/sessions/<cwd>/<session_id>/canceled_turn_resume.json` (prompt text + optional prompt id + reason; mode 0600; not secrets) |
| On open | If `[ui] resume_canceled_turn_on_restart` is on (**default true**), re-queue prompt **once**, toast **"Resuming canceled turn..."**, clear marker |
| Clear | Successful turn end clears leftover marker; never invent finished/never-canceled work |
| Pause | Global pause uses in-process stash only (`allow_local_rewind: false`); does **not** write restart marker |

**Config / Settings:** `[ui] resume_canceled_turn_on_restart` (Settings → Session).

### 2. Soft stop (not fearless pause)

| Item | Detail |
|------|--------|
| Chord | `Ctrl+Shift+S` (`ActionId::ToggleSoftStop`); does **not** steal `Ctrl+Shift+Space` pause |
| Phases | Off → Armed → Holding (after current top-level turn finishes success or terminal fail) |
| Effect | While Holding, automatic queue drain does **not** start the next item; subagents for the finishing turn may complete with it |
| Toggle | Arm again before turn ends disarms; toggle while Holding releases hold and re-drains queues |
| Toast (take effect) | `"Soft stop: finished current turn; queue held."` |
| Chrome | Status label armed vs queue held (`AppView` status path) |
| Force drain | Explicit force drain releases Holding so operator can still push work |

### 3. Options GUI (Settings modal)

Extended existing Settings (no second modal):

- **Token Economy** (Agent): cap when economic; max/min/desired/lock implement effort; show period pacing; local ledger; reconcile Management usage. Persist `[token_economy]` via `PersistSetting` + shell writers; live policy re-reads disk.
- **Economic mode** (cross-link already present).
- **Resume canceled turn on restart** (Session, default on).

## Key files

| Path | Role |
|------|------|
| `crates/codegen/xai-grok-pager/src/app/soft_stop.rs` | SoftStop phase machine + toasts + chrome labels |
| `crates/codegen/xai-grok-pager/src/app/dispatch/soft_stop.rs` | Toggle dispatch + release drain |
| `crates/codegen/xai-grok-pager/src/app/dispatch/tests/soft_stop.rs` | Queue gate / unarmed / distinct from pause |
| `crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs` | Drain blocked when Holding or global pause |
| `crates/codegen/xai-grok-pager/src/app/dispatch/turn.rs` | Write cancel marker on interactive cancel; soft-stop on turn end; clear marker on clean success |
| `crates/codegen/xai-grok-pager/src/app/dispatch/prompt.rs` | Soft-stop on prompt-path turn finish |
| `crates/codegen/xai-grok-pager/src/app/dispatch/session/load.rs` | Auto-resume on hydrate when marker + setting on |
| `crates/codegen/xai-grok-pager/src/app/dispatch/global_pause.rs` | Cancel with `allow_local_rewind: false` (no restart marker) |
| `crates/codegen/xai-grok-shell/src/session/canceled_turn_resume.rs` | Durable marker write/load/clear + policy |
| `crates/codegen/xai-grok-shared/src/ui_config.rs` | `resume_canceled_turn_on_restart` default on |
| `crates/codegen/xai-grok-pager/src/settings/defs.rs` + `registry.rs` | Settings metadata + current values |
| `crates/codegen/xai-grok-pager/src/views/settings_modal/state.rs` | `action_for_bool` / int arms |
| `crates/codegen/xai-grok-pager/src/app/dispatch/settings/{ui,setters}.rs` | Reset/rollback + setters + PersistSetting |
| `crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs` | Disk writers for resume + token_economy |
| `crates/codegen/xai-grok-pager/docs/user-guide/03-keyboard-shortcuts.md` | Soft stop chord |
| `crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md` | Config keys + Token Economy |
| `crates/codegen/xai-grok-pager/docs/user-guide/17-sessions.md` | Resume marker lifecycle |
| `FORK.md` | Short product bullets (soft stop, resume, Settings GUI) |

## Tests (green)

```bash
cargo test -p xai-grok-pager --lib -- soft_stop
# 13 soft_stop + binding tests

cargo test -p xai-grok-shell --lib -- canceled_turn
# 6 marker / should_auto_resume / round-trip tests

cargo test -p xai-grok-shell --lib -- token_economy
# 45 config / effort / pacing / ledger / reconcile tests

cargo test -p xai-grok-pager --lib -- \
  set_resume_canceled_turn_on_restart_persists \
  set_token_economy_bool_emits \
  every_setting_has_action_for_reset_arm \
  every_persisting_setting_has_rollback_arm \
  every_setting_has_action_for_bool_arm \
  every_setting_has_dispatch_arm
```

Coverage highlights:

- Soft stop with non-empty queue does not drain next; unarmed continues; distinct from global pause mid-turn cancel.
- Cancel marker round-trip; empty prompt rejected; finished work has no marker; resume requires enabled + UserCancel.
- Settings: resume PersistSetting payload + UiConfig flip; Token Economy PersistSetting key; full registry reset/rollback arms for new keys.

## Fix during closeout

`move_setting_away_from_default` lacked arms for `resume_canceled_turn_on_restart` and all `token_economy.*` keys, so:

- `every_setting_has_action_for_reset_arm`
- `every_persisting_setting_has_rollback_arm`

failed. Arms added in `app/dispatch/tests/settings.rs`.

## Not done / operator

- **No git commit/add** (per request).
- Full `just check` left for operator.
- Soft stop is process-local (not persisted across restart); only cancel-resume marker is durable.
- Token Economy GUI toggles re-read disk after `PersistSetting` applies (no separate in-memory cache beyond config file).

## Verify commands (package scoped)

```bash
cargo fmt -p xai-grok-pager -p xai-grok-shell -p xai-grok-shared

cargo test -p xai-grok-pager --lib -- soft_stop
cargo test -p xai-grok-pager --lib -- set_resume_canceled set_token_economy every_setting
cargo test -p xai-grok-shell --lib -- canceled_turn token_economy
```
