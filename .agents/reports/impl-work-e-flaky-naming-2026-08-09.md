# Work E — flaky adoption test + continue-interrupted-turn naming

**Date:** 2026-08-09
**Scope:** Work E only (approved plan).

## E1 — Flaky test fix

### Test
`session_loaded_with_synthetic_running_prompt_id_stays_idle`
(`/home/hunter/Projects/surmount/grok-build/crates/codegen/xai-grok-pager/src/app/acp_handler/tests/queue_and_adoption.rs`)

### Red (observed)
```
cargo test -p xai-grok-pager --lib session_loaded_with_synthetic_running_prompt_id_stays_idle
→ FAILED: synthetic non-scheduler running prompt must not be adopted on load
  (current_prompt_id was Some after SessionLoaded)
```

### Root cause (not a product adoption bug)
Host pollution: leftover
`$GROK_HOME/sessions/%2Ftmp/sess-1/canceled_turn_resume.json`
from dogfood / other tests sharing cwd `/tmp` + session id `sess-1`.

Flow:

1. Synthetic `running_prompt_id` (`task-completed-…`) correctly **not** adopted
   (`should_adopt_running_prompt` pure unit stays green).
2. Because `!adopting`, session load enters **auto-continue interrupted turn**.
3. Host marker loads → enqueue + force drain → `current_prompt_id` set.
4. Adoption contract asserts `current_prompt_id.is_none()` → false red.

Removing the host marker alone made the old fixture green; replanting it made it red again. That is fixture hermeticity, not a race inside the pure adoption predicate.

### Green fix (fixture only; contract not weakened)
- Unique session id: `sess-synthetic-non-adopt`
- Unique `tempfile` cwd on the agent
- Pin `resume_canceled_turn_on_restart = false` so this adoption-only unit cannot start a turn from disk
- Stronger asserts: not adopted **and** still no bound prompt id (no auto-continue either)

### Verify
```
# host marker deliberately present under /tmp/sess-1
cargo test -p xai-grok-pager --lib session_loaded_with_synthetic_running_prompt_id_stays_idle
→ ok
```

## E2 — Naming (product copy)

**Chosen plain English name:** **continue interrupted turn**

Distinct from **`/resume`** (session pick / `-c` / `--resume`).

### User-facing strings
| Surface | Before | After |
|---------|--------|-------|
| Marker auto-continue toast | `Resuming canceled turn...` | `Continuing interrupted turn...` |
| History recovery toast | `Resuming interrupted turn...` | `Continuing interrupted turn...` |
| Failed continue toast | `Interrupted work found but resume failed: …` | `Interrupted work found but could not continue: …` |
| Settings label | Resume canceled turn on restart | Continue interrupted turn on restart |
| Settings save toast | same old label | Continue interrupted turn on restart |

Wire/config key **`resume_canceled_turn_on_restart`** unchanged (no migration).

### Docs / FORK
- `docs/user-guide/17-sessions.md` — section renamed; explicit contrast with `/resume`
- `docs/user-guide/05-configuration.md` — comments + table
- `FORK.md` — short note toast wording
- Settings `defs.rs`, `ui_config.rs` doc comments, setters toast label

### Tests updated for toast copy
`dispatch/tests/turn.rs` toast contains-checks; marker-vs-history toast wording distinction dropped (same operator string by design; prompt text still proves marker wins).

## Commands run

| Step | Result |
|------|--------|
| Red: synthetic stays_idle | FAILED (host marker) |
| Green: same + related session_loaded cancel/continue filters | 7/7 ok |
| `cargo test -p xai-grok-shell --lib canceled_turn_resume` | 9/9 ok |
| `set_resume_canceled_turn_on_restart_persists_and_updates_ui` | ok |
| `cargo fmt -p xai-grok-pager -p xai-grok-shell -p xai-grok-shared` | done |
| `cargo clippy -p xai-grok-{pager,shell,shared} --lib -- -D warnings` | clean |

(`--all-targets` clippy on shell hits pre-existing test-only issues outside this work; not introduced here.)

## Behavior
No intentional product behavior change beyond copy. Auto-continue still re-queues once when the setting is on; only operator-facing language and the flaky fixture isolation changed.

## Out of scope
Work A/B/C chrome, git commit/add/push.
