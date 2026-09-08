# Contributing to Grok OSS

**Pull requests are welcome on this repository**
([SurmountSystems/grok-oss](https://github.com/SurmountSystems/grok-oss) or the
current Surmount fork URL if the rename is pending).

This project is a **faithful open-source fork** of
[xai-org/grok-build](https://github.com/xai-org/grok-build). Upstream does **not**
accept external PRs; improvements intended for Grok OSS should target **this**
repo. See [`FORK.md`](FORK.md) for remotes, sync policy, and branding rules.

## How to contribute

This fork uses **git-flow-style feature branches** so collaborators can
work in parallel. Canonical process:
[`docs/github-tracking.md`](docs/github-tracking.md) and
[`docs/git-workflow.md`](docs/git-workflow.md).

1. Branch from current `main` as `feat/<slug>`, `fix/<slug>`, or
   `docs/<slug>`. One concern per branch when the work can split.
2. Prefer small, reviewable **signed** commits (`git commit -S`) on a
   real TTY. Agents never commit and never GPG-sign.
3. **Keep open PRs mergeable without rewriting history.** If `main` moved,
   merge `origin/main` into your feature branch and push normally. Do **not**
   rebase a published PR branch or force-push while CI is running.
4. Keep **upstream mergeability**: avoid renaming `xai-grok-*` crates or
   rewriting large unrelated areas.
5. Run targeted checks when possible. Full remote gate is
   `just check-remote` (operator VPS).
6. After a signed push, open a PR against Surmount `main` that describes
   all the work and links the GitHub issues (plan Approve files a plan
   issue; bug reports file bug issues with screenshots).
7. Agents: Plan **Approve** → `gh issue create` with the full plan text.
   Bug report → issue with screenshots. Operator signed+pushed → PR.

## Security reports

Do **not** open public issues for vulnerabilities. Prefer the process in
[`SECURITY.md`](SECURITY.md). For Surmount-specific packaging or fork-only
code, contact maintainers privately (see SECURITY.md).

## Licensing

By submitting a contribution, you agree it is provided under the
**Apache License, Version 2.0** (see [`LICENSE`](LICENSE)), consistent with
this tree.
