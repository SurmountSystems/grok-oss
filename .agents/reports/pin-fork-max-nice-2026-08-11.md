# Report: FORK.md max-niceness dual-pin (2026-08-11)

## Status

Done. Minimal dual-pin added; FORK was not bulk-rewritten.

## What changed

| Path | Change |
|------|--------|
| `FORK.md` | **+6 lines** only (one short paragraph). No other FORK edits. |
| Structure / other pins | Unchanged (DOGE, upstream, PATH hermeticity text, tables, headings all intact). |

**Before this turn:** `git diff HEAD -- FORK.md` was empty (niceness work lived in AGENTS 3a, justfile, `scripts/run-nice.sh` only). FORK already had PATH hermeticity / `cargo-ci` under **CI and local quality**; it did **not** mention max niceness.

## Exact FORK anchor

- **Section:** `## CI and local quality`
- **Placement:** immediately after the **PATH hermeticity (CI / low-mem)** paragraph; immediately before `## Versioning and “am I up to date?”`
- **Heading text in body:** `**Max niceness (local heavy work):**` (starts ~line 619 after insert)

**Content (substance):**

- `cargo-ci` and `nix_retry` run under `scripts/run-nice.sh` (`nice -n 19` + `ionice -c3` on Linux)
- So `just check` / `just ci` / `just test` / nix build recipes stay out of the way of interactive UI
- Escape: `GROK_NO_NICE=1`
- Agents prefer `just cargo-ci cargo …` (or same prefix outside just)
- Dual-pin: `AGENTS.md` hard constraint **3a**; justfile header

## Confirmation: not bulk-rewritten

```text
git diff --stat HEAD -- FORK.md
 FORK.md | 6 ++++++
 1 file changed, 6 insertions(+)
```

Diff is a pure insert of one paragraph; zero deletions, zero heading moves, no rewrite of surrounding process law.

## Not done (out of scope)

- No git add / commit / push
- No AGENTS / justfile / `run-nice.sh` edits (already present from prior niceness work)
