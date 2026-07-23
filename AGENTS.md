# Agent rules — Surmount Grok OSS

Concise rules for work in this repository. Global GPG and subagent rules also
apply (`~/.grok/AGENTS.md`).

## Hard constraints

1. **Never run `git commit`.** Commits are human-only on a real TTY (signed).
   Agents may do complex git labor when asked (conflict resolve, merge setup,
   history diagnosis), then **stage and hand** exact `git commit -S …`
   commands — including after “fix conflicts” / “make the PR mergeable” /
   even “commit this.” Do **not** push unless he explicitly asked to push.
   Full policy: `~/.grok/AGENTS.md` § *Commits — agents never commit*.
2. **Never bypass GPG** (`commit.gpgsign=false`, `--no-gpg-sign`, fake
   `gpg.program`, hook disables, etc.).
3. **Never bulk find-and-replace.** Bulk **find** (`rg`) is fine. Edits must
   be surgical and reviewed in context.
4. **Talk to humans in plain language.** No pack of opaque acronyms, false
   either/or menus, or planning jargon (phases, tracks, workstreams) in user
   replies, product docs, tests, or **filenames**.
5. **Never ask permission to continue clear work.** If the goal is known
   (finish the onto stack, resolve conflicts, keep going), **do the next step**
   — do not end with “say the word,” “want me to continue?,” or similar. Ask
   only when intent is genuinely ambiguous or an irreversible external action
   needs confirmation (push/PR when not already requested). A dirty mid-pick
   tree is unfinished work, not a pause for ceremony.

## Regressions and deep diagnosis

- Do **not** investigate regressions or multi-file diagnosis in the parent
  thread (no parent-marathon of greps, logs, or long code walks). Spawn tightly
  scoped subagents; join on short on-disk summaries only.
- Full rule: `~/.grok/AGENTS.md` § *Regressions and deep diagnosis — never in
  the parent thread*.

## Regressions and deep diagnosis

- Do **not** investigate regressions or multi-file diagnosis in the parent
  thread (no parent-marathon of greps, logs, or long code walks). Spawn tightly
  scoped subagents; join on short on-disk summaries only.
- Full rule: `~/.grok/AGENTS.md` § *Regressions and deep diagnosis — never in
  the parent thread*.

## When you ship product work

- Update **[`FORK.md`](FORK.md)** with a short hierarchical note (what changed
  for Grok OSS). Link out for detail; do not write novels in FORK.
- Prefer existing living docs over new ephemeral notes.

## CI and quality

- **CI is for checks only** — never a release package build in GHA (supply
  chain). Humans package with `just build` / install recipes when needed.
- Full local gate (same idea as GHA quality): **`just check`** or **`just ci`**.
  Run before push. No pre-commit hook is required for that.
- There is no `ci-quick` or `ci-host` recipe.

## Git flow

- Feature branches → pull request → **`main`**. Tool branches (`import/*`,
  `onto-xai/*`) are not a second product main; they land through PRs.

## Upstream (xAI)

- Prefer **product commits on their current tip** when histories break
  (`scripts/put-history-on-xai.sh` — real cherry-pick). Then **join Surmount
  `main`** into that tip (`scripts/join-main-into-onto.sh`, `merge -s ours`)
  so the branch is PR-able. See [`docs/upstream-history.md`](docs/upstream-history.md).
- **Import** absorbs their tree into Surmount history (different job).
- This fork exists because upstream does not accept external PRs. If that
  changes, open a PR to contribute.

### Onto / put-history — recovery after compaction

Living truth: **`docs/upstream-history.md`** § *HITL runbook* + § *Live stack*,
and **`docs/upstream-onto-log.md`**.

**Frozen mid-work (2026-07-24 — re-read Live stack first):**

| Item | Value |
|------|--------|
| Branch | `onto-xai/6e386420825b` |
| HEAD | `8f2f7f2` impl (#7) |
| Mid-pick | `8b933eb` #13 — conflict resolved+staged; human `git cherry-pick --continue` |
| HEAD | `f8e9eab` merge xai 2 (#12) |
| After | #13 `8b933eb`, join, PR; if continue says “no pick” → already landed, use CONTINUE=1 |
| Issues | close #11 + #14 when PR lands (tips superseded by `6e38642`) |

**Do not invent** `MODE=overlay` / commit-tree modes — cherry-pick only.
**Do not** `cherry-pick --abort` or `FORCE=1` rebuild while this stack is
healthy mid-pick.

**Conflict discipline:** tip APIs → keep HEAD; Grok OSS seams → re-apply
product; union import/feature lists; never blind `--ours`/`--theirs` on the
whole unmerged set; never strip markers without reading both sides; never
fix tests to the wrong intent when ambiguous. Mega picks (#4 done, #12 next)
are the same rule at larger scale.

**Subagents (mandatory for multi-file conflict work — do not forget):**

Conflict resolve and mega-pick diagnosis are **child work**, not a parent
marathon of greps/reads across shell/pager/sampler. Parent coordinates only.

| Do | Do not |
|----|--------|
| Spawn **tightly scoped** agents on **disjoint** path sets (e.g. shell session vs pager UI vs sampler) | Parent solo all 18 UU files |
| Prefer `general-purpose` for actual resolve+stage; `explore` only to map | Fan out one agent per file “just because” (waste) |
| Cap concurrency ~2–3 when scopes are clean and independent | Spin a large parallel swarm with overlapping files |
| Join on short on-disk notes or a staged `git status` check | Re-run the child’s full reads in the parent “to be sure” |
| Pass conflict rules + product seams in the prompt (self-contained) | Dump whole parent chat / invent nested subagents |

Global token strategy still applies (`~/.grok/AGENTS.md` § subagents). Plain
language: use subagents strategically; never wasteful mass spawn.

**Human-only:** every `git cherry-pick --continue` and join merge is
`git commit -S` on a real TTY. Agents stage and hand commands only.

**Scripts:** `put-history-on-xai.sh` is on the branch after early product
picks. `join-main-into-onto.sh` may still be missing until later — take from
`origin/main` if needed. Early bare-tip recovery (temp `/tmp` script + `ROOT`
patch) is in the HITL runbook if ever required again.

## Residual

- [`RESIDUAL.md`](RESIDUAL.md) holds **open** human-intent or unfinished honesty
  items only. When something is finished, move the lasting truth into FORK or
  the right process doc — do not leave it only in residual.

## Naming

- `xai-*` crates and paths stay for mergeability with upstream.
- Surmount-only crates and product names use **`grok-*`** / **`grok-oss`**
  (no `xai-` prefix on novel fork crates).
