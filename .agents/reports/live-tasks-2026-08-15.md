# Live tasks (survive compaction) — 2026-08-15

Operator pin (re-stated same day): always remember to document every
live task so it survives context compaction. Chat is not enough.
Law: project `AGENTS.md` 3e; host `~/.grok/AGENTS.md` § *Document
every live task on disk*.

No product agents running as of this write. Clippy
`question_mark` on the per-path write lock is shipped.

## Shipped this session (keep; do not re-do)

- File-level infer-from-path verify. After `search_replace` /
  `apply_patch`, a `.rs` write is rustfmt that file and clippy-driver
  that file. Not `cargo clippy -p <crate> --lib`. Command tool refuses
  crate-wide cargo. Reports: `impl-edit-verify-89e0807b.md`,
  `fork-file-level-edit-verify.md`.
- ACP per-path write lock on `search_replace` / `apply_patch` /
  `write`. Held path is a tool error naming holder and file. Nine
  named tests green. Report: `impl-acp-file-edit-lock.md`.
- Auto-wake cancel-barrier tests. Isolated each test agent
  `state_path`. 33/33 green. Report:
  `bug-auto-wake-cancel-barrier.md`.
- OpenCode relative path test. Path join was already correct.
  Content assert now expects rustfmt after file-level verify. Report:
  `bug-opencode-edit-relative-path.md`.
- L2 wait can find a live L3. Nested reparent hid the child from the
  spawning session. Immediate-spawner map fixes that. Report:
  `bug-l2-wait-l3-not-found.md`.
- One review job per slice. `--effort 3` must not spawn three visible
  Review agents. Report: `pin-one-reviewer-not-three.md`.
- Tools improve tools. Do not write disposable bash/Python/curl.
  Improve the named product tools. Report:
  `pin-tools-improve-tools.md`. Dual-pin AGENTS constraint 6.
- Clippy `question_mark` on the per-path write lock. The `match` in
  `try_acquire_writes` is now `?`. Nine named lock tests still pass.
  Report: `bug-clippy-per-path-write-lock-question-mark.md`.

## Open product leftovers (honest, not started)

- OpenCode `edit` does not take the per-path write lock. The lock
  slice reserved that file for the relative-path fixer. Wire
  `editor_infra::per_path_write_lock::acquire_for_tool` there.
- Hashline structured edit does not take the lock.
- Short fire-and-forget race if spawn returns an id before the
  coordinator processes Spawn. Not the long wait miss.

## Open process (not a product code slice)

- Document every live task on disk (this file + AGENTS 3e).
- Thoughtful todo / session-board tracking. After dogfood. Board:
  `feat:thoughtful-todo-tracking-process`.

## Operator-gated (do not invent agent work)

- Live TUI still needs a full quit and reopen of `grok-oss` to pick
  up the new wait fix and the lock. Dogfood items on the board stay
  pending.
- `ask:ef34dc39` PTY grandchild-kill flake.
- `ask:te-second-supergrok-login-stored` (second SuperGrok login).
- `plan:structured-token-efficient-convo` residual 2h structured
  conversations. Do not invent a second novel.
- Token economy suspected not working. Later. Tools first.

## Cancelled on purpose

- Three parallel "Review edit-verify …" specialists. Do not relaunch.
- Docs L2 that spawned three FORK writers.
- Separate lock planner wave. Implemented instead.
