# Agent rules for Surmount Grok OSS

This file is standing law for this repository. Keep it short. Detail lives in [`FORK.md`](FORK.md). Open residual lives in [`RESIDUAL.md`](RESIDUAL.md). GitHub tracking lives in [`docs/github-tracking.md`](docs/github-tracking.md). Git-flow lives in [`docs/git-workflow.md`](docs/git-workflow.md). Host process law still lives in `~/.grok/AGENTS.md`. Do not copy that host file into this file.

## Git

Never run `git commit`. Commits are human-only on a real TTY and must be GPG-signed. Never bypass GPG (`commit.gpgsign=false`, `--no-gpg-sign`, a fake `gpg.program`, or hook disables). Never run `git add` or stage unless the Operator asked. Leave the index alone. Do not push unless the Operator asked to push.

## Tests

Named tests are contracts. Do not fit tests to code. Observe red, then change the product so the same test passes. Do not rewrite a failing assert to match the code.

## Agent depth

L0 is the dashboard, one level above L1. L1 is where the agent talks to the Operator. L2 is a coordinator (the Operator can prompt an L2 they are watching). L3 implements and reviews. An L3 does not spawn. There is no L4. Default: L1 spawns an L2; that L2 spawns and manages L3s. Do not keep implement work on L1. Deep home: [`FORK.md`](FORK.md) § *Hierarchical subagents*.

The Hierarchical fast path on L1 is one already-named command, one already-named file read, the asked-for short report, or one already-named one-line edit. "Just a quick look" is not that path. Nested sampling death is not a stop: document remaining on disk, resume or spawn, keep the named product work moving. Remaining pointer: `/home/hunter/.agents/reports/remaining-2026-10-21-all-open-bugs.md`. A finished nested agent is a same-turn next action. Silence after a finish looks like a stall. Detail: host `~/.grok/AGENTS.md` § *Report finished nested work the same turn* and [`FORK.md`](FORK.md) § *Hierarchical subagents*.

Speaker labels are Operator and Agent. Nested agents are L1, L2, and L3. Do not call L1 a parent of L2, or L2 a child of L1, or L3 a child of L2.

## Builder

Do not run cargo, rustc, clippy, or `cargo test` on this laptop. Agents may run `just check-remote` one at a time. `just test-remote` is Operator-owned. After a product change, one runner owns `just check-remote` then `just install`. Do not restart a live remote compile at five minutes.

## Product

The product CLI is `grok-oss`. Start `grok-oss`, not bare `grok`.

SuperGrok is a paid product. Never call SuperGrok free. Name which meter you mean: included SuperGrok period limits, SuperGrok dollar credits, or console team prepaid / console API credits.

### Already-open L1 sibling IPC re-auth

Login success in one grok-oss process live-publishes IPC `{authorized, permission_mode}` on `$GROK_HOME/authorize_ask.sock`. That document never carries tokens. Already-open L1 contexts reload credentials from the shared store, resend the waiting prompt, and must not paint `Run /login to re-authenticate, then resend your message.` Inherit-on-startup of a new session is not that path. One expiry must not paint Authentication required / Run /login twice. Never tell the Operator to /login. Dual-pin: [`FORK.md`](FORK.md) same heading.

## Reports and homes

Reports live under `~/.agents/reports/` on this machine. They are not part of the git tree. Do not invent `docs/issues/` or `docs/dev/bugs/` as a reports home. Do not create project `.agents/reports/` or project-root `.grok/` for reports. Live plans live under `~/.agents/plans/`.

## Priority

Code and tests matter more than docs. Docs matter more than git ceremony. Do not invent long essays or git nags in this file.
