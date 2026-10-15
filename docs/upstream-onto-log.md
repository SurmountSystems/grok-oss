# Onto-xAI stack log

Append-only record of Surmount **product stacks parented at an xAI export tip**.

Each row is a local (or Surmount-remote) `onto-xai/*` branch whose first parent
chain includes the xAI tip — so `git log xai-org/main..<onto tip>` shows our
work. Surmount `main` remains the product archive. These branches are disposable
and rebuilt after force-exports.

**Current mechanics:** real `git cherry-pick -x` via `grok-nix-helper put-history-on-xai`,
then optional `grok-nix-helper join-main-into-onto` (`merge -s ours`) so `main` is an
ancestor and GitHub PR compare works. Full HITL runbook:
[`docs/upstream-history.md`](upstream-history.md) § *HITL runbook*.

There is **no** `MODE=overlay` / commit-tree mode in the current helper.

**Live stack (SHAs / mid-work):** canonical home is
[`docs/upstream-history.md`](upstream-history.md) § *Live stack*. Project
`AGENTS.md` only **links** there (no frozen tip table in D1 law).

| Date (UTC) | xAI tip | xAI tree | Surmount tip stacked | Onto tip | Notes |
|------------|---------|----------|----------------------|----------|-------|
| 2026-07-18 | `98c3b2438aa922fbbe6178a5c0a4c48f85edc8ce` | `b40a1962cb8061b85c2354850ab4d5707f48414b` | (older) | (local) | Historical only (pre cherry-pick script) |
| 2026-07-22 | `3af4d5d39897855bdcc74f23e690024a5dc05573` | `e595174931be9bfb490aacf149e2c9cc0ca0ebba` | product via cherry-pick | landed as PR #12 (`f8126f9` tip family) | First full HITL land: put-history → join → PR #12 |
| 2026-07-24 *(join landed; CI fixes dirty)* | `6e386420825bd44ae648c63e7c8cba12fcec9401` | `3db5a3bd92232bb54581fb8701c6ec79ba48293d` | `origin/main` @ `8b933eb` | branch `onto-xai/6e386420825b` **pre-join tip `56d1fc2`**; tree `2cbad23add47…` | Product stack complete: OpenRouter→#2→#3→#4→#7→#12→#13. `join-main-into-onto.sh` ran (`-s ours`); Join committed (`b1bd97d`). Post-join CI test fixes in worktree (branding asserts, shell workflow restore/cap, persist_ack abort, merge-dup tests).. Then `just check`, push, PR base=main, close #11+#14. |
| 2026-08-10 | `b13fa526f5112c0b20dad5f1f2300d3d3b127895` | `0f26f4082a3b9602ec712b218e177626b2bf72e5` | `origin/main` @ `a1515fe1` + `main..fixes-2` (6) | branch `onto-xai/b13fa526f511` tip `9060f502`; join `ea7a9ad5`; post-docs tree `16808643...` | FORCE rebuild on unsigned xAI tip. Main product 9/9 + fixes-2 unique 6 + join ours. Assert pins green. `grok-rate-limit` 15/15. Cargo feature mop (dup test-support, shell local-workspace/test-support). No agent push/PR. Stash `recon-temp-work-b-wip-2026-08-10` kept. |
| 2026-08-12 | `e5fd4816d43260c15ba785f103990c1ed6cea230` | `25eefa9bdb3a4748cc065be3fa8200d04bc54493` | `origin/main` @ `f17e84d8` | branch `onto-xai/b13fa526f511` first-parent `ee8a80d2`; join `e08e5961`; compile mop `4651593a`; tree `42dfccb6...` | Restack first-parent `b13fa526f511..09c407e2` (24) onto public Grok Build 1.0.3. Same PR branch name. Old joined tip on `backup/onto-xai-b13fa526f511-0f61ff44-joined-20260812`. Unsigned intermediates via commit-tree. Nucleo reuse-per-root + rustc 1.97.1 kept. Compile mop restored 1.0.3 shell/pager cores and re-applied Surmount seams. |
| 2026-10-15 | `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8` | `d7f2fa79787169d549eee97c8c2dbf9fbe3a43d3` | `origin/main` @ `b7ab91532c9e8008081b4f281794be43ecae73a6` (HB-1, tree `0088decb0b81780e899771f46c447f8c62154ba6`) | branch `onto-xai/2bdd1d6a6369` tip `81e1530e1801db580f3cab3c0d1286ef44c9bb5b`; first parent `b8ccd3ffa99e8c795f7082a1c55df39e7ba6a885`; second parent `b7ab9153`; tree `e9bb02c65f4b29ac98454f2ba9838e89789bd8d1` | Real merge of HB-1 into the onto branch. Merge-base `d1d3b737`. 301 unmerged paths resolved with Hunter's razor. `origin/main` is an ancestor. xAI tip is an ancestor. Join `-s ours` was not required. `assert-process-pins` still fails the same two paths as `origin/main` (`crates/codegen/grok-nix-helper/Cargo.lock` and `.agents/skills/`). Seven-class land `fn` names that exist on `origin/main` remain on this tip. No agent push. No GitHub PR until origin has this tip. |

**1.0.3 seam-loss note (2026-08-13).** The compile mop kept helpers. It dropped operator-visible crate seams that `FORK_PATHS` / assert cannot restore. Diagnosis: [`.agents/reports/fork-loss-postmortem-2026-08-13.md`](../.agents/reports/fork-loss-postmortem-2026-08-13.md). Still missing in source: status compact **included SuperGrok period limits** meter (helpers live, `status.push("credits")` never runs), hop list `failover_api_keys` always empty, AutoCompact still wipes the todo board, `hide_header` unread. Restored later in source: rails, SIGUSR1 fleet, `--version` without TTY, keyring leak, plan five-CTA paint. Catalog reds were deleted so `just check` could not fail them. Next land: assert, then catalog **test names still exist**, then dogfood (rails, five-CTA, included SuperGrok period limits meter, SIGUSR1 after a failed install). SuperGrok is paid; do not call that meter free SuperGrok.

## Process-pin survival (import FORK_PATHS, 2026-07-24)

Import used to restore a minimal fork list and **silently drop** project
`AGENTS.md`, `RESIDUAL.md`, `README.md` branding, `scripts/join-main-into-onto.sh`,
`scripts/with-ci-hermetic-path.sh`, research under `doc/dev/` + `docs/dev/`, and
Surmount `ci.yml`. Those are now in `FORK_PATHS` in
`grok-nix-helper` (`fork_paths.rs`). After restore (and anytime post-onto):

```bash
grok-nix-helper assert-process-pins          # worktree
grok-nix-helper assert-process-pins HEAD     # or a tip tree-ish
just upstream-assert-process-pins
```

Detail: [`doc/dev/research/fork-paths-hardening-2026-07-24.md`](../doc/dev/research/fork-paths-hardening-2026-07-24.md).
Canonical recon law remains [`docs/upstream-history.md`](upstream-history.md).

## How to append (after stack lands)

```bash
echo "| $(date -u +%Y-%m-%d) | \`<xai-sha>\` | \`<xai-tree>\` | \`<surmount-sha>\` | \`<onto-sha>\` | <notes> |" \
  >> docs/upstream-onto-log.md
```

## Rebuild after the next force-export

```bash
git fetch xai-org main --force
FORCE=1 SURMOUNT_REF=origin/main grok-nix-helper put-history-on-xai
# resolve conflicts carefully; signed cherry-pick --continue on TTY
grok-nix-helper join-main-into-onto
git commit -S ...   # human
just check && git push -u origin HEAD
```

## HITL checklist (short)

1. Fetch tip; do **not** stack an obsolete issue SHA if tip moved.
2. Cherry-pick product; **sign every continue** on a real TTY.
3. Conflict rules: tip APIs = HEAD; Grok OSS seams = re-apply product; union imports/features; no bulk marker strip.
4. Join with `-s ours` before PR to `main`.
5. Append this log; close related `upstream-export` issues.
6. Never agent-run `git commit` / never disable GPG.

Detail: [`upstream-history.md`](upstream-history.md).
