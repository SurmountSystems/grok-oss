# Join: lock + min implement-loop effort

**Date:** 2026-08-03
**Status:** green

## Outcome

Extended `[token_economy]` with always-on **min floor** and optional **lock** for implement-loop effort (1–5 reviewer fan-out). Economic ceiling + desired inject stay gated on economic mode + cap master.

## Config

| Key | Default | Behavior |
|-----|---------|----------|
| `min_implement_effort` | **1** | Floor always applied. Set **2** for always-a-reviewer. |
| `lock_implement_effort` | unset / **0** | When 1–5, force that value (ignore prompt + desired). |

Validation: fields in 1–5; `min ≤ max`; `desired ≤ max`; if lock set, `min ≤ lock ≤ max` (lock 5 + max 3 fails load). Runtime still applies economic ceiling after lock if max was lowered mid-session.

## Policy order

1. Lock (if set)
2. Else: present stays; missing → desired inject only when economic caps active
3. Floor `min` (always; missing + min > 1 injects floor)
4. Ceiling `max` when economic caps active

## Files

- `crates/codegen/xai-grok-shell/src/token_economy/config.rs`
- `crates/codegen/xai-grok-shell/src/token_economy/implement_effort.rs`
- `crates/codegen/xai-grok-shell/src/util/config/economic_mode.rs` (doc)
- `crates/codegen/xai-grok-pager/src/app/auto_implement.rs` (doc)
- `crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md`
- `crates/codegen/xai-grok-pager/docs/user-guide/04-slash-commands.md`
- `FORK.md`
- host `~/.agents/skills/implement/SKILL.md` (one-line product floor/lock/clamp note)

**Incidental (not this feature):** parallel `RemoteMeterSample` rename left pager tuple destructure broken. Fixed call sites only:

- `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs`
- `crates/codegen/xai-grok-pager/src/views/limits_snapshot.rs`

Did **not** touch `ledger.rs`.

## Tests (named contracts)

Shell (`token_economy`):

- min 2: effort 1 → 2 + toast
- min 2: effort 3 stays 3
- lock 2: effort 5 → 2; missing → 2 not desired
- economic off + min 2 still floors; economic off + lock still locks
- economic on + max 3 + min 2 + effort 5 → 3; effort 1 → 2
- config rejects min > max, lock > max, lock < min
- existing ceiling / desired / economic-off defaults still pass

## Proof

```text
cargo fmt -p xai-grok-shell -p xai-grok-pager
cargo clippy -p xai-grok-shell --lib --locked -- -D warnings   # ok
cargo test -p xai-grok-shell --lib token_economy               # 45 passed
cargo test -p xai-grok-pager --lib auto_implement --locked     # 11 passed
```

## Operator config example

```toml
[token_economy]
min_implement_effort = 2   # always at least one reviewer
# lock_implement_effort = 2  # optional: force exactly 2 always
```

No git commit/add.
