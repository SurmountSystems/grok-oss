# bug: nix channel-rust-1.94.0 FOD hash mismatch

**Date:** 2026-08-11
**Branch:** `onto-xai/b13fa526f511`
**Symptom:** `just check` → `cargo-mem-guard` FOD fail on `channel-rust-1.94.0.toml`

## Cause

`fenix.fromToolchainFile` in `flake.nix` pins an SRI for the Rust channel
manifest. Upstream rewrote `channel-rust-1.94.0.toml`, so the fixed-output
derivation hash no longer matched.

## File(s) changed

| Path | Change |
|------|--------|
| `/home/hunter/Projects/surmount/grok-build/flake.nix` | Update fenix `sha256`; comment channel 1.92.0 → 1.94.0 |

No other pins of the old SRI (grep: only `flake.nix`).
`rust-toolchain.toml` already says `channel = "1.94.0"`; no version bump.

## Hash

| | SRI |
|--|-----|
| **old** | `sha256-sqSWJDUxc+zaz1nBWMAJKTAGBuGWP25GCftIOlCEAtA=` |
| **new** | `sha256-qqF33vNuAdU5vua96VKVIwuc43j4EFeEXbjQ6+l4mO4=` |

Source of new hash: operator paste `got:` from the FOD error (not flaky).

## Verify

```bash
just nix_retry nix build -L .#cargo-mem-guard
```

- **Exit:** 0
- Channel FOD accepted; rustc/cargo/clippy/rustfmt/std 1.94.0 (2026-03-05)
  fetched and installed; crane built `cargo-mem-guard` release.
- `./result/bin/cargo-mem-guard --help` prints usage.

## Residual

- Full `just check` / `just ci` not re-run (long). FOD for channel-rust is fixed;
  remaining CI risk is unrelated compile/test/flake checks.
- No FORK/docs edit: project does not maintain a separate durable pin table for
  this SRI beyond `flake.nix` + `rust-toolchain.toml`.
- Stashes left alone (`recon-temp-work-b-wip-2026-08-10`,
  `recon-resume-local-dirt-2026-08-10`).
