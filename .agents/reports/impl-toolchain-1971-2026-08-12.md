# Report: project Rust toolchain → 1.97.1

**Date:** 2026-08-12
**Branch:** onto-xai/b13fa526f511
**Goal:** Lock project toolchain to operator host stable `rustc 1.97.1` (Surmount wins over upstream 1.94.0).

## Changes

| File | Old | New |
|------|-----|-----|
| `rust-toolchain.toml` `channel` | `1.94.0` | `1.97.1` |
| `flake.nix` fenix comment | channel 1.94.0 / channel-rust-1.94.0.toml | channel 1.97.1 / channel-rust-1.97.1.toml |
| `flake.nix` `fromToolchainFile` `sha256` | `sha256-qqF33vNuAdU5vua96VKVIwuc43j4EFeEXbjQ6+l4mO4=` | `sha256-A1abGIbOtcBSdrUMhDGrER3pRM1hQP4fp9gh3Y4PKc8=` |

Components kept: `rustfmt`, `clippy`. Profile `default`. Targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`.

## Commands + exit codes

1. `just nix_retry nix build -L .#cargo-mem-guard`
   - Exit **1** (expected FOD mismatch on old sha256).
   - `got:` `sha256-A1abGIbOtcBSdrUMhDGrER3pRM1hQP4fp9gh3Y4PKc8=` for `channel-rust-1.97.1.toml`.

2. After setting that SRI: `just nix_retry nix build -L .#cargo-mem-guard`
   - Exit **0**.
   - Realized fenix 1.97.1 (rustc/cargo/clippy/rustfmt + aarch64 std), crane-built `cargo-mem-guard` with `cargo 1.97.1 (c980f4866 2026-06-30)`.

## Host alignment

- Host: `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- Active toolchain after edit: `1.97.1-x86_64-unknown-linux-gnu` (overridden by project `rust-toolchain.toml`).

## Grep residual (not Surmount pins)

- `docs/upstream-history.md` still says tip lockfiles may need **1.94+** (historical MSRV note, not a project pin). Left alone.
- FORK.md / AGENTS.md: no "we pin 1.94" standing law. No edits.
- justfile / CI workflows: no hardcoded `1.94` / `channel-rust-1.94` (they follow fenix + `rust-toolchain.toml`).
- SVG path noise in `login.rs` matched `1.94` substring only; not a version pin.

## Residual / follow-ups

- Full workspace `cargo check` / clippy / `just check` not required for this pin acceptance; only mem-guard nix path was green.
- If rust-lang rewrites the 1.97.1 channel manifest later, re-pin FOD from the next hash-mismatch `got:` value (same comment pattern in `flake.nix`).
- No git commit/stage performed.
