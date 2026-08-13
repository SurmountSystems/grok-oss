# CI-239 residual wave status (2026-08-12)

**Repo:** `/home/hunter/Projects/surmount/grok-build`
**Sources:** process mop + existing residual reports under `.agents/reports/`
**Companion mop detail:** [`impl-process-mop-ci239-2026-08-12.md`](impl-process-mop-ci239-2026-08-12.md)
**Final reverify:** [`impl-final-reverify-ci239-2026-08-12.md`](impl-final-reverify-ci239-2026-08-12.md)

## Snapshot

| Area | Live status (this mop / latest reports) |
|------|----------------------------------------|
| **Pager full lib** | **Green** — **8813 passed / 0 failed** (`--test-threads=8`; final reverify holds) |
| **Shell named residual clusters** | **Green** — MCP reenable, plan rejects, send-now, recap/side-question, auth_retry sample (**53** filters ok) + tail oneshots report green; final reverify `mcp_reenable` **6/0** |
| **Non-shell oneshots** | **Green** — tools, agent encrypted templates, hooks tty, sampler CF copy, update smoke, pager-render auto dark (see `bug-non-shell-oneshots-2026-08-12.md`) |
| **pager-minimal** | **Green** — **86 / 0** after dim-rail + `insert_block_before` API restore (`bug-pager-minimal-dim-rail-2026-08-12.md`) |
| **Process clippy deps** | **Green** after mop: tools, pty-harness, pager-render |
| **Shell / pager package clippy `-D warnings`** | **Green** — final reverify: `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` **exit 0**; `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` **exit 0** (clippy mop greened shell + update + pager) |
| **PTY flaky** | **Still residual** — `close_pty_kills_a_background_grandchild` historically TRY1 timeout / TRY2 pass (`bug-ci-239-test-cluster-2026-08-11.md`); reliability only, not in original 239 hard fails |
| **Rejoin Surmount main** | **Operator-gated** — mop/join held for signed commit + join script on real TTY (`impl-upstream-rejoin-main-2026-08-11.md`) |
| **Dogfood** | **Operator-gated** — install/`/rebuild`, quit old binaries, re-verify plan panel + cancel-resume (`d0-dogfood-checklist-2026-08-09.md`) |

## Green by wave (reports)

### Pager

| Report | Outcome |
|--------|---------|
| `bug-pager-layout-acp-singletons-2026-08-12.md` | layout / ACP / share-wake / privacy / slash leftovers greened |
| `bug-pager-key-owner-residual-2026-08-12.md` | key_owner bar **30/30** |
| `bug-pager-plan-cta-residual-2026-08-12.md` | approve_plan_flush **118/0** (five-CTA surface) |
| `bug-pager-lib-residual-resample-2026-08-12.md` | full lib **8810→8813**, last 3 theme-cache flakes pinned |
| Process mop reverify | **8813/0** holds |
| Final reverify (`impl-final-reverify-ci239-2026-08-12.md`) | **8813 passed; 0 failed; 11 ignored** |

### Shell

| Report | Outcome |
|--------|---------|
| `bug-shell-mcp-plan-residual-2026-08-11.md` | MCP reenable + plan ask_user/exit_plan green |
| `bug-shell-residual-wave-2026-08-12.md` | send-now, auth, cancel/chat, recap clusters green |
| `bug-shell-residual-tail-2026-08-12.md` | bearer, channel scrub, list force-kind, timeout, acp setup, registry churn green |
| Process mop sample | **53/0** on named filters |
| Final reverify `mcp_reenable` | **6 passed; 0 failed** |

### Clippy (package-scoped `-D warnings`)

| Package | Command | Result |
|---------|---------|--------|
| `xai-grok-shell` | `cargo clippy -p xai-grok-shell --all-targets -- -D warnings` | **exit 0** (final reverify) |
| `xai-grok-pager` | `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **exit 0** (final reverify) |

Workspace `clippy.toml` may still print the known non-fatal `tokio::process::Command::spawn` disallowed-method path warning; package lint under `-D warnings` is clean.

### Oneshots / other packages

| Report | Outcome |
|--------|---------|
| `bug-non-shell-oneshots-2026-08-12.md` | tools ×2, agent encrypted, sampler, update, pager-render dark green; pager-minimal was blocked then fixed |
| `bug-pager-minimal-dim-rail-2026-08-12.md` | pager APIs restored; minimal **86/0** |

## Remaining (not unit-test residual from CI-239 inventory)

1. **PTY flaky grandchild kill** under nextest stress (reliability; not claimed hard-red in mop).
2. **Operator rejoin main:** commit mop (signed TTY), then `./scripts/join-main-into-onto.sh` when main is ahead of onto tip.
3. **Operator dogfood:** `just install` or `/rebuild`, quit deleted-inode TUIs, re-check plan present + cancel-resume on live `grok-oss`.

## What CI-239 “unit mass” looks like now

Original cluster was **239** unit fails (pager ~148 + shell ~59 + small packages). After this residual wave + mop reverify + final reverify:

- **Pager lib mass:** closed (**0** fails; **8813/0**).
- **Shell inventory oneshots / named clusters:** closed in reports + sample green; `mcp_reenable` **6/0**.
- **Shell + pager package clippy `-D warnings`:** closed (**exit 0** both).
- **Non-shell oneshots:** closed.
- Open product-adjacent work is **pty flake reliability** and **operator gates** (rejoin, dogfood) — not another 239-scale unit red wall, and not package clippy red.
