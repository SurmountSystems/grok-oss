# bug: channel-rust-1.94.0 FOD hash regressed again

**Date:** 2026-08-12
**Symptom:** `just check` → `cargo-mem-guard` fails fixed-output hash on `channel-rust-1.94.0.toml`

## Cause

Not broken Nix daemon or admin install. `fenix.fromToolchainFile` in `flake.nix`
pins an SRI for the Rust channel manifest. Working tree still had the **old**
hash after yesterday's fix report (likely lost in dirty onto mop / uncommitted
revert). Upstream still serves the manifest whose hash is the "got:" value.

| | SRI |
|--|-----|
| **was in tree** | `sha256-sqSWJDUxc+zaz1nBWMAJKTAGBuGWP25GCftIOlCEAtA=` |
| **live got:** | `sha256-qqF33vNuAdU5vua96VKVIwuc43j4EFeEXbjQ6+l4mO4=` |

Same pin as `bug-nix-rust-194-channel-hash-2026-08-11.md` (that report claimed green;
tree no longer had the edit).

## Fix

`flake.nix`: set `sha256` to the got SRI; comment notes 1.94.0 + re-pin recipe.

## Verify

```bash
nice -n 19 ionice -c3 just nix_retry nix build -L .#cargo-mem-guard
```

**Exit 0.** No sudo / nix-daemon restart needed.

## Admin commands?

**None** for this failure. Optional later: full `just check` after commit.

No git commit by agent.
