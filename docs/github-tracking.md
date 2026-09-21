# GitHub tracking and git-flow (pinned 2026-09-08)

Standing process for Surmount Grok OSS
([SurmountSystems/grok-oss](https://github.com/SurmountSystems/grok-oss))
and for other Surmount repos we share with collaborators. Chat is not
enough. Dual-pin: [`AGENTS.md`](../AGENTS.md), [`FORK.md`](../FORK.md),
[`CONTRIBUTING.md`](../CONTRIBUTING.md), this file,
[`git-workflow.md`](git-workflow.md), host `~/.grok/AGENTS.md`, and the
plan skill. Compaction must not drop this.

Collaborative issues distinguish **feature**, **bug**, and
**upstream-merge** labels so peers can filter work without guessing
from the title.

## 1. Plan Approve → GitHub issue

When the operator **Approves** a plan (product plan-panel Approve, not
empty Enter, not always-approve tool permissions):

1. Same turn, open a **new GitHub issue** on this project's origin
   (`SurmountSystems/grok-oss` here).
2. Title is the plan title.
3. Body is the **full plan text** from `plan.md` (session plan file).
4. Label `enhancement` for a feature plan, `bug` for a bug-fix plan, or
   `upstream-merge` for an upstream merge.
5. Do not wait for a later “should I file this?”

## 2. Bug report → GitHub issue (with screenshots)

When the operator **reports a bug** (screenshot, “this is broken”, a
reproducing window):

1. Same turn, open a **new GitHub issue** on the **appropriate** origin
   for that workspace (this repo for grok-oss; the other project's
   GitHub remote when the bug is not this tree).
2. Attach screenshots. Prefer `gh issue create` with image files
   uploaded (gist or GitHub attachment) so the issue body can show them.
   Do not leave “see the chat” as the only copy.
3. Label `bug`.
4. Quote the reproducing path, session, and what the operator saw.

## 3. Never sign. Never unsigned.

Agents **never** run `git commit`. Agents **never** GPG-sign. Agents
**never** disable signing (`commit.gpgsign=false`, `--no-gpg-sign`, fake
`gpg.program`). After docs and product are ready, **hand** exact:

```bash
git add <paths>
git commit -S -m "<message>"
git push -u origin <branch>
```

The operator runs those on a real TTY.

## 4. After the operator signs and pushes → pull request

Once the operator has a **signed** commit **on origin** for that branch,
the agent **must** open (or update) a GitHub pull request that describes
**all** the work: plans, bugs, files, tests, and linked issues. Do not
stop at “noted.” Do not wait to be asked again for that PR. If a PR
already exists for the branch (example: #51 on `remote-1`), update its
body and do not open a duplicate.

## 5. Git-flow feature branches

Collaborative git work uses **git-flow-style feature branches** so peers
can parallelize:

- Branch from current `main` (this repo has no `develop` default).
- Names: `feat/<slug>` for features, `fix/<slug>` for bugs, `docs/<slug>`
  for process-only docs.
- One concern per branch when the work can split.
- Published branches catch up with **`git merge origin/main`**, not rebase.
  See [`git-workflow.md`](git-workflow.md).
- Agents may `git checkout -b feat/…` when starting collaborative work
  the operator asked to land. They still do not `git add` unless asked
  and never `git commit`.

## What this is not

- Not a license to push.
- Not a license to click Approve in the plan panel.
- Not GitHub Actions secret writes.
- Not public issues for security reports ([`SECURITY.md`](../SECURITY.md)).
