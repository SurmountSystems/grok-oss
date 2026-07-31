# Agent rules — Surmount Grok OSS

Concise rules for work in this repository. Global GPG and subagent rules also
apply (`~/.grok/AGENTS.md`).

## Document hierarchy (D0–D3)

Not the same as session-board L0–L2 (todos / child joins). Doc layers:

| Layer | Role | Always load? | Paths |
|-------|------|--------------|-------|
| **D0** | Open residual only | Yes (open section) | [`RESIDUAL.md`](RESIDUAL.md) |
| **D1** | Standing law | Yes (compressed) | This file; `~/.grok/AGENTS.md`; [`FORK.md`](FORK.md) |
| **D2** | Append logs | No — append / read tail | [`docs/upstream-*`](docs/), `doc/dev/campaigns/` |
| **D3** | Deep strategy / research | When needed | `doc/dev/research/`, host skill `references/` |

**Never** append recon diaries or campaign closed writeups into D1 AGENTS —
use D2 logs. Closed residual history:
[`doc/dev/campaigns/interject-todos-closed-2026-07.md`](doc/dev/campaigns/interject-todos-closed-2026-07.md).

## Product priority (value order)

**Code + tests > docs > git.** Docs matter more than git ceremony; docs matter
less than product code and tests. Do not invent long essays or git nags.

## Hard constraints

1. **Never run `git commit`.** Commits are human-only on a real TTY (signed).
   Agents may do complex git labor when asked (conflict resolve, merge setup,
   history diagnosis), then **stage and hand** exact `git commit -S …`
   commands — including after “fix conflicts” / “make the PR mergeable” /
   even “commit this.” Do **not** push unless the operator explicitly asked to push.
   Full policy: `~/.grok/AGENTS.md` § *Commits — agents never commit*.
2. **Never bypass GPG** (`commit.gpgsign=false`, `--no-gpg-sign`, fake
   `gpg.program`, hook disables, etc.).
3. **Never bulk find-and-replace.** Bulk **find** (`rg`) is fine. Edits must
   be surgical and reviewed in context.
4. **Talk to humans in plain language.** No pack of opaque acronyms, false
   either/or menus, or planning jargon (phases, tracks, workstreams) in user
   replies, product docs, tests, or **filenames**. **No bare plan-step codes**
   (`S1`, `S3`, `D3`, `B2`, …) in residual, chat, join notes, **or identifiers**
   without the plain name next to them — agents and humans misread them (e.g.
   **S3** ≠ Amazon, **B2** ≠ Backblaze).
   **Names are product (pinned 2026-07-27):** **file and directory names first**
   (they stand out in git, PRs, and search), then variables, functions, modules,
   test names, and user-visible enums. Thoughtful, readable, plain meaning
   first. Not clever jargon or step codes in paths/symbols (`b2_order.rs`,
   `proactive_hop`). Prefer names a tired reader can parse. Wire/protocol
   fields we do not own may keep upstream spelling; our files, wrappers, and
   tests still get clear names.
   **Sapient Experience (stance pointer):** speak to humans as humans do; do
   not try to *be* human. Full pin: host `~/.grok/AGENTS.md` § Sapient
   Experience; open residual [`RESIDUAL.md`](RESIDUAL.md) §2f. Do not dump
   novels here.
   **Billing meters stay distinct:** personal SuperGrok dollar credits ≠
   included weekly allowance ≠ console API spend. Name which meter.
   **Dual-auth language (pinned 2026-07-27):** ban bare jargon **proactive hop**,
   **sticky exhaust** / **sticky hop**, and **dual-host** without plain
   explanation. Prefer: *mark SuperGrok used up from billing % / leave SuperGrok
   when weekly included is full*; *stay on the console key after switch /
   remember this SuperGrok identity is out of allowance*; *also switch the API
   host (SuperGrok proxy ↔ `api.x.ai`)*. Residual, joins, comments, tests, and
   **identifiers** use the plain names. (UI sticky headers / permission sticky
   cursor are unrelated product terms — leave those alone.)
5. **Never ask permission to continue clear work.** If the goal is known
   (finish the onto stack, resolve conflicts, keep going), **do the next step**
   — do not end with “say the word,” “want me to continue?,” or similar. Ask
   only when intent is genuinely ambiguous or an irreversible external action
   needs confirmation (push/PR when not already requested). A dirty mid-pick
   tree is unfinished work, not a pause for ceremony.
6. **Prefer Rust tools over inventing Python/bash** when a product or host
   tool/bin already covers the job (token + security). Do not paste inventories
   into this file. Migration plan:
   [`doc/dev/research/python-to-rust-tools-2026-07-26.md`](doc/dev/research/python-to-rust-tools-2026-07-26.md).
7. **Friction → suggest plan** — process pushback / “plan first” → stop
   implementing and suggest or enter plan mode; explicit “just fix it” is fine.
   Host law: `~/.grok/AGENTS.md` § *Friction → suggest plan*.
8. **No questionnaire modals for plan clarifications** (re-pinned 2026-07-26).
   Do **not** use host multi-choice / `ask_user_question` widgets while
   planning. Put open questions **in the plan file** (and plain chat). Host
   skill `/plan` § hard rule 6; `--legacy` only if the operator opts in for that run.
9. **Do not invent “out of scope.”** Prior docs/plans saying “credit-only”
   or “soft-429 hop = No” are **defaults that can be wrong** for new intent.
   Verify code; if the operator names a real need (e.g. rate-limit failover),
   plan it — do not dismiss as out of scope from stale residual text.
10. **Ambiguity → park** (pinned 2026-07-26). Do not invent intent. Track
    clarification (`ask:*` / plan open question / residual) and ask in **plain
    freeform** — no plan questionnaire modals. Host: `~/.grok/AGENTS.md` §
    *Ambiguity → park*.
11. **Git silence** (pinned 2026-07-26). No nags about stage/commit/push/PR/
    uncommitted trees in ordinary status. Engage git only for **complex**
    recon/upstream/onto/put-history when asked. Still never `git commit`.
    Host: § *Git silence*.
12. **Autonomy default** (pinned 2026-07-26). Prefer always-approve autonomy;
    keep Zed-style deny/ask filters and explicit approval via existing
    `permission_mode` + `[permission]` rules / hooks — do not invent a second
    system. Host: § *Autonomy default + filter/approval*; user-guide
    `22-permissions-and-safety.md`; `~/.grok/config.toml`.
13. **No throwaway compiler probes in the workspace** (pinned 2026-07-26).
    Prove bugs with project tests (`cargo test` / nextest) or a *suggested*
    unit test in review notes — never bare `rustc` / one-shot ELFs (`rust_out`)
    under the repo. Do **not** gitignore probe junk (visibility intentional).
    Reviewers suggest the test; implementers land red→green. Host:
    `~/.grok/AGENTS.md` § *Workspace hygiene*.
14. **Proper red/green TDD when behavior changes** (pinned 2026-07-26).
    Observed fail first (in-tree test, ran, named contract), then minimal
    product fix so the **same** test passes. Do not claim TDD without a red
    log line. Exceptions: pure docs/typos/format; operator says skip. Host:
    `~/.grok/AGENTS.md` § *Red/green TDD* + § *User-reported bugs & features*.
15. **Do not fit tests to code** (pinned 2026-07-26). Changing a test needs a
    named contract + evidence + stronger/equal assert; park if intent unclear.
    False-green → stricter assert, not weaker. Host: § *Test intent*.

## Subagents — parent is HITL UX only (hard)

The **main/parent thread is HITL UX only**: goals, spawn/wait, join **short
on-disk notes** subagents wrote, brief user status. **Research and
implementation never run in the parent** — not even “just a quick look.” Full
rule: `~/.grok/AGENTS.md` § *Regressions…* + § *Hard stop — parent is
coordinator only*. Git handoff only when the operator asked for complex git
help (see hard constraint **Git silence**).

### Agent depth L1 / L2 / L3 (pinned 2026-07-29) — not session-board layers

**Not** the session-board L0/L1/L2 table below (residual / todos / join notes).
Product agent depth:

| Depth | Name | Role |
|-------|------|------|
| **L1** | **Main thread** | Operator chat. HITL coordinator only. **Modal-free** — typing and chat must stay unobstructed. Must not get stuck in plan soft-park or exclusive key capture that blocks the prompt. |
| **L2** | **Subagents** | Spawned by L1 (implement, explore, plan, review, …). Planning, research, implement, review, test work lives here. |
| **L3** | **Subagent-spawned specialists** | L2 may spawn further specialists/personas. **Do not go deeper than L3.** |

L1 coordinates; deeper layers do the heavy work. Product: soft-park that
**traps** L1 is rejected; plan review is on demand (`/view-plan`, status click,
panel CTAs), not forced keyboard capture of the main thread.

**Default loop (pinned 2026-07-27):** track on board → **spawn** → **wait** →
join on disk. Do **not** kill/respawn mid-flight to re-scope; do **not**
monologue interim workarounds while an implementer runs. Mid-flight operator
clarifications → board upsert only; **resume** after join (or additive spawn
if disjoint). Host: § *Hard stop* default loop.

- **User-facing language** (mirror of host `~/.grok/AGENTS.md` § Language,
  2026-07-26): never bare **child/children** as a nickname for subagents
  (“Child finished green” is wrong). Prefer **subagent**, **implementer**,
  **explore agent**, **worker**, or a role name. Keep ban on “kids” + “cheap.”
  “Child process” = OS process only, in technical docs.
- **CI fail, regression, multi-file diagnosis, non-trivial fix, skills-location
  claims:** first tool turn is `spawn_subagent` — not parent `grep` / `gh` log
  pull / test file reads / “I’ll check the docs.”
- Parent may: goals, spawn/wait, read **short on-disk join notes**, brief user
  status; git handoff only when asked for complex git/recon work.
- Parent must **not**: pull CI logs, open failing tests, re-run nextest, edit
  product code, re-do the subagent’s greps “to be sure,” or research/implement
  in the main thread.
- **Additive asks / “also”:** phrases like **also**, **this too**, **and also**,
  **this work too** mean a second slice, not a pivot. Board-upsert; **spawn**
  another subagent (or queue if same-file race); **never kill**, cancel, or
  re-prompt healthy in-flight subagents on the prior goal unless the operator
  explicitly stops/supersedes. Full pin: `~/.grok/AGENTS.md` § *Additive asks*.

## Never assume without checking

**Docs can be wrong** — treat prose as untrusted until code and load paths
confirm it. Docs in this repo (including this file, FORK, research notes) can
be wrong or stale. Do **not** claim skills location, CI root cause, conflict
intent, or recon survival from prose alone.

- First tool turn for multi-file / CI / regression / “where do skills live?” is
  **spawn_subagent** (explore or general-purpose as fits).
- Verify against **code and load paths** (and live trees) before asserting.
- Join on short on-disk notes; do not re-prove the subagent in the parent.
- **Auth / credentials store / keyring:** diagnose with **red/green TDD**
  (`cargo test` contracts), not host shell D-Bus/keyring probes. Do **not** fan
  out explore + implementer on the same store bug. One implementer owns TDD.
  Pin: `~/.grok/AGENTS.md` § *Product auth / store diagnosis*.
- **Plan approval:** product CTAs only (`exit_plan_mode` → soft-park →
  `a`/`A`/`?`/`s`/`q`). **Never** freeform chat “reply approve/revise/abandon.”
  Pin: `~/.grok/AGENTS.md` § *Plan approval — product CTAs only*.

## Skills (multi-source)

Skills are **not** “off this branch only.”

| Layer | Who owns it |
|-------|-------------|
| Discovery, load order, project skill roots (`.agents/skills`, `.grok/skills`), bundle install/sync, user-guide | **Product on this branch** |
| Operator skill packs (`implement`, `pr-babysit`, …) under `~/.agents/skills` | **Host** overlay (wins at User tier) |
| Platform pack cache under `~/.grok/bundled/skills` | **Network** bundle (product writes the cache) |

Process that must survive recon: pin on **branch** (`AGENTS`, `FORK`,
`docs/upstream-*`) **and** host when both apply. Detail:
`doc/dev/research/where-skills-come-from-2026-07-24.md`, user-guide `08-skills.md`.

## Survive recon (process pins on the branch)

Chat is **not** enough. Import restores only `FORK_PATHS`; put-history
cherry-picks product; join (`-s ours`) keeps the onto tip tree. Pins on branch:
this file, [`FORK.md`](FORK.md), [`RESIDUAL.md`](RESIDUAL.md),
[`docs/upstream-history.md`](docs/upstream-history.md) + sibling logs, upstream
scripts (`put-history`, import, join, hermetic PATH, assert pins, …).

```bash
./scripts/assert-process-pins.sh          # worktree
./scripts/assert-process-pins.sh HEAD     # or a tip tree-ish
just upstream-assert-process-pins
```

Import runs the assert after `FORK_PATHS` restore. See FORK § *What recon keeps*.

**Product seams inside `xai-grok-*` are not path-restored.** OpenRouter,
dual-auth, DOGE default, titles-on, stuck-retry clear, and similar live in
shared crates and survive onto only via cherry-picks plus **cargo tests**.
Assert proves files exist; it does not prove those contracts. Durable filter
catalog: [`doc/dev/upstream-regression-filters.md`](doc/dev/upstream-regression-filters.md);
cheat sheet also in FORK § *Upstream regression filters*. After recon, run
assert **and** those filters (or `just check`).

## Ship / CI / git

- Ship product work: short hierarchical note in [`FORK.md`](FORK.md); link out.
- **CI is checks only** — never a release package build in GHA. Local gate:
  **`just check`** / **`just ci`**. No `ci-quick` / `ci-host`.
- Feature branches → PR → **`main`**. Tool branches (`import/*`, `onto-xai/*`)
  land through PRs, not a second main.

## Upstream (xAI)

Prefer **product commits on their current tip** (`scripts/put-history-on-xai.sh`
— cherry-pick), then **join Surmount `main`** (`scripts/join-main-into-onto.sh`,
`merge -s ours`). **Import** is a different job (absorb tree into Surmount
history). Detail: [`docs/upstream-history.md`](docs/upstream-history.md).

### Onto recovery (no live SHAs in this file)

**Live tip SHAs / mid-work = D2 only.** Recovery pointers:

1. [`docs/upstream-history.md`](docs/upstream-history.md) § *HITL runbook* + § *Live stack*
2. [`docs/upstream-onto-log.md`](docs/upstream-onto-log.md)

No `MODE=overlay` / commit-tree. No `cherry-pick --abort` or `FORCE=1` rebuild
while a healthy stack is mid-pick. Multi-file conflicts → subagents on disjoint
paths; join on disk. Every continue/join merge = human `git commit -S`.

## Residual

- [`RESIDUAL.md`](RESIDUAL.md) holds **open** human-intent or unfinished honesty
  items only (D0). When finished, move lasting truth into FORK or process docs;
  campaign closed writeups → `doc/dev/campaigns/`.

## Operator orchestration (session board L0–L2)

Session-board layers only (todos / joins). **Do not confuse** with agent depth
**main thread (L1) / subagents (L2) / specialists (L3 max)** above.

| Session layer | Where |
|---------------|--------|
| **L0** durable residual | `RESIDUAL.md` (D0 open) / campaign docs |
| **L1** session todos | Namespaced `plan:*` `impl:*` `pr-N:*` `recon:*` `residual:*` `ask:*` `feat:*` `bug:*` — **never casual wipe**; merge upsert only; product keep-unless-mentioned on `merge: false`. **Fib leaves:** size **1 or 2** only; larger work → split children; **progress = Σ leaf sizes** (phases/containers unsized). Prefer `meta.kind` + `parentId`. See [`doc/dev/research/todo-progress-fib-2026-07-26.md`](doc/dev/research/todo-progress-fib-2026-07-26.md). |
| **L2** child joins | Short on-disk notes |

Prefer no worktrees (`allow_worktree = false` default). Campaign + joins:
`doc/dev/campaigns/operator-orchestration-2026-07.md`,
`doc/dev/research/todo-levels-product-2026-07-24.md`,
`doc/dev/research/execute-plan-no-worktree-2026-07-24.md`,
`doc/dev/research/todo-progress-fib-2026-07-26.md`.

## Naming

- `xai-*` crates/paths stay for upstream mergeability.
- Novel Surmount crates/names: **`grok-*`** / **`grok-oss`** (no `xai-` prefix).
