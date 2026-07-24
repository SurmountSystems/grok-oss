# Onto-xAI stack log

Append-only record of Surmount **product stacks parented at an xAI export tip**.

Each row is a local (or Surmount-remote) `onto-xai/*` branch whose first parent
chain includes the xAI tip — so `git log xai-org/main..<onto tip>` shows our
work. Surmount `main` remains the product archive. These branches are disposable
and rebuilt after force-exports.

**Current mechanics:** real `git cherry-pick -x` via `scripts/put-history-on-xai.sh`,
then optional `scripts/join-main-into-onto.sh` (`merge -s ours`) so `main` is an
ancestor and GitHub PR compare works. Full HITL runbook:
[`docs/upstream-history.md`](upstream-history.md) § *HITL runbook*.

There is **no** `MODE=overlay` / commit-tree mode in the current scripts.

| Date (UTC) | xAI tip | xAI tree | Surmount tip stacked | Onto tip | Notes |
|------------|---------|----------|----------------------|----------|-------|
| 2026-07-18 | `98c3b2438aa922fbbe6178a5c0a4c48f85edc8ce` | `b40a1962cb8061b85c2354850ab4d5707f48414b` | (older) | (local) | Historical only (pre cherry-pick script) |
| 2026-07-22 | `3af4d5d39897855bdcc74f23e690024a5dc05573` | `e595174931be9bfb490aacf149e2c9cc0ca0ebba` | product via cherry-pick | landed as PR #12 (`f8126f9` tip family) | First full HITL land: put-history → join → PR #12 |
| 2026-07-24 *(join landed; CI fixes dirty)* | `6e386420825bd44ae648c63e7c8cba12fcec9401` | `3db5a3bd92232bb54581fb8701c6ec79ba48293d` | `origin/main` @ `8b933eb` | branch `onto-xai/6e386420825b` **pre-join tip `56d1fc2`**; tree `2cbad23add47…` | Product stack complete: OpenRouter→#2→#3→#4→#7→#12→#13. `join-main-into-onto.sh` ran (`-s ours`); Join committed (`b1bd97d`). Post-join CI test fixes in worktree (branding asserts, shell workflow restore/cap, persist_ack abort, merge-dup tests).. Then `just check`, push, PR base=main, close #11+#14. |

## How to append (after stack lands)

```bash
echo "| $(date -u +%Y-%m-%d) | \`<xai-sha>\` | \`<xai-tree>\` | \`<surmount-sha>\` | \`<onto-sha>\` | <notes> |" \
  >> docs/upstream-onto-log.md
```

## Rebuild after the next force-export

```bash
git fetch xai-org main --force
FORCE=1 SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh
# resolve conflicts carefully; signed cherry-pick --continue on TTY
./scripts/join-main-into-onto.sh
git commit -S …   # human
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
