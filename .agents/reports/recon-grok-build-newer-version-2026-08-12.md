# Recon: last onto vs public Grok Build (2026-08-12)

Read-only check. No onto or import was started.

## What this tree last absorbed

Last stacked export is `b13fa526f5112c0b20dad5f1f2300d3d3b127895` (xAI tree `0f26f4082a3b9602ec712b218e177626b2bf72e5`), recorded 2026-08-10 in [`docs/upstream-onto-log.md`](../../docs/upstream-onto-log.md) and the live-stack table in [`docs/upstream-history.md`](../../docs/upstream-history.md). Branch `onto-xai/b13fa526f511`, onto tip `9060f502`, join `ea7a9ad5`.

Package lockstep in this tree is still `1.0.0` (`crates/codegen/xai-grok-version`, pager, pager-bin). Root `SOURCE_REV` is monorepo pin `a51a1dc62fe20029ac39a665985bba78edbb870f`. Surmount keeps rustc `1.97.1` (FORK / residual mop). The import log’s last completed row is still the seed `b189869`; later tips were stacked via onto, not a finished `import/*` PR.

## What is public now

GitHub `xai-org/grok-build` `main` tip is `e5fd4816d43260c15ba785f103990c1ed6cea230` (“Synced from monorepo”, 2026-08-12 22:28:38 UTC, tree `25eefa9bdb3a4748cc065be3fa8200d04bc54493`). Parent is `be713136d2a69080743a3f6b3c72077057e5948f` (2026-08-11). No tags and no GitHub Releases. Accessed: 2026-08-12. Sources: [commits](https://github.com/xai-org/grok-build/commits/main), [tip commit](https://github.com/xai-org/grok-build/commit/e5fd4816d43260c15ba785f103990c1ed6cea230), [changelog](https://x.ai/build/changelog).

Public lockstep is `1.0.3`. Public `SOURCE_REV` is `ea094a8c369475f97c85540d01730baec0dce5d6`. Public `rust-toolchain.toml` is still `1.94.0`.

Two published exports sit after our last onto: `be713136` (Aug 11) then `e5fd481` (Aug 12).

## Delta if we onto again

A second onto from `e5fd481` would absorb product `1.0.1` (Aug 10), `1.0.2` (Aug 11), and `1.0.3` (Aug 12): history-only `/rewind` with confirm, `grok du`, tabbed `/usage` `/session-info` `/context`, bounded subagent spawn, tool-call argument spinner, image-compaction recovery, faster subagent spawn when `~/.grok` is large, 120 Hz TUI cadence, click-to-copy `/session-info`, plus new crates (`xai-grok-active-sessions`, `xai-grok-foreign-sessions`, `xai-grok-workspace-daemon`, `xai-grok-session-search`, and others). Keep Surmount rustc `1.97.1`. Residual already says not to start that onto from the mop note.

## Comparison

| Item | This repo (last onto) | Public now |
|------|----------------------|------------|
| xAI export SHA | `b13fa526f5112c0b20dad5f1f2300d3d3b127895` | `e5fd4816d43260c15ba785f103990c1ed6cea230` |
| Date | 2026-08-10 | 2026-08-12 |
| Branch name | `onto-xai/b13fa526f511` | `main` (would be `onto-xai/e5fd4816d432`) |
| Package version | `1.0.0` | `1.0.3` |
| `SOURCE_REV` | `a51a1dc62fe20029ac39a665985bba78edbb870f` | `ea094a8c369475f97c85540d01730baec0dce5d6` |
| rustc pin | Surmount `1.97.1` | Upstream still `1.94.0` |
| Status | **Behind.** Need a second onto. | Current public feed. |
