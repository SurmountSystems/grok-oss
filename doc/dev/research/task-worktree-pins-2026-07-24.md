# Task levels + worktree pins (2026-07-24)

**Scope:** process/skill namespaces, plan terminology, product
`allow_worktree`, dual-pin on branch.

## A) Host skills (operator overlay)

| File | Change |
|------|--------|
| `~/.agents/skills/_SKILL_RULES-read-first-pls.md` | Todo namespace table (`plan:*` `impl:*` `pr-N:*` `recon:*` `residual:*`); merge policy — never wipe foreign prefixes; implement must not `merge:false` wipe plan/recon |
| `~/.agents/skills/plan/SKILL.md` | Present footer: no phases/tracks language; handoff writes high-level residual bullets to project `RESIDUAL.md` when present; seeds `impl:*` from Steps |
| `~/.agents/skills/implement/SKILL.md` | Open with merge upsert respecting foreign prefixes; prefer isolation none; note product `allow_worktree` force-none |
| `~/.agents/skills/execute-plan/SKILL.md` | Full shared-cwd protocol: default `isolation_mode=shared-cwd`, Steps 4a/4b/5a/5c dual-path, no fragile TOML parse; fall back when `allow_worktree=false` / create fails. See `execute-plan-no-worktree-2026-07-24.md` |
| `~/.agents/skills/shared/references/subagent-token-strategy.md` | L0/L1/L2 task levels table; pending prompts = intentional next turns only |

## B) Product config (this repo)

**Minimum shipped:**

| Piece | Location |
|-------|----------|
| Config key | `[subagents] allow_worktree` on `SubagentsConfig` (default **true**) |
| Runtime field | `Config.subagent_allow_worktree` via `resolve_subagents` |
| Spawn context | `SubagentSpawnContext.subagent_allow_worktree` |
| Force-none | `handle_request.rs` after runtime isolation resolve: if `!allow_worktree` and isolation ≠ none → force `None` + info log |
| Docs | user-guide `05-configuration` (Subagents table), `16-subagents` (Isolation + disable globally) |
| Tests | `subagents_config_allow_worktree_defaults_true`, `…_false_via_resolve` |

**Not in this pass (residual):**

- Default `allow_worktree = false` for OSS installs
- Product namespaced-todo API / wipe guard
- Dedicated L2 “notes channel” UI

**Done later (skill half, 2026-07-24):** `/execute-plan` full shared-cwd
auto-adapt — `doc/dev/research/execute-plan-no-worktree-2026-07-24.md`.

## C) Branch dual-pin

| File | Note |
|------|------|
| `AGENTS.md` | Pointer to campaign + L0/L1/L2 + prefer no worktrees |
| `FORK.md` | Hierarchical: subagent worktree policy + skill dual-pin |
| `RESIDUAL.md` | Open: todo levels product surface, notes channel, skill override completeness |
| `doc/dev/campaigns/operator-orchestration-2026-07.md` | Campaign summary |

## Operator recommendation

```toml
# ~/.grok/config.toml — prefer shared workspace for children
[subagents]
allow_worktree = false
```

Skills already prefer isolation none; config makes it hard when a prompt
still requests worktree.

## Verify

```bash
# Config parse + resolve tests (shell crate)
cargo test -p xai-grok-shell subagents_config_allow_worktree -- --nocapture
```

No git commit in this pass (human-only signed commits).

---

## Close-out — survival edges (2026-07-24)

| Edge | Action |
|------|--------|
| Import `FORK_PATHS` | `.grok/workflows` restored from base (Rhai team workflows; not GHA YAML) |
| `assert-process-pins.sh` | `REQUIRED_DIRS` includes `.grok/workflows` |
| Host `~/.grok/config.toml` | `[subagents] allow_worktree = false` (backup: `config.toml.bak.*`); `[hints] fork_worktree_mode = "never"` already set |
| Workflow on disk | `.grok/workflows/git-recon-status.rhai` — track in git for restore to work on next import base |

**Note:** FORK_PATHS restore only keeps paths present on `BASE_REF`. Commit the
workflow dir on `main` (human-signed) so the next import base has something to
check out. Host skill `git-recon` remains the durable recon SOP regardless.

No git commit in this close-out (human-only signed commits).
