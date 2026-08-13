# feat: max niceness for cargo / nix / just heavy work (2026-08-11)

## Status

**Done.** Default just quality and nix paths run under maximal process niceness so they do not fight interactive UI.

## Inventory (before)

| Path | Heavy work | Already reniced? |
|------|------------|------------------|
| `just check` → `ci` → flake-meta / ci-prep / test | nix + cargo | No |
| `just cargo-ci` (private) | All `just test-*`, limits hermetic tests | No (env hygiene + CI_LOW_MEM mem-guard only) |
| `just nix_retry` (private) | All nix build / flake-meta / mem-guard | No |
| `just install` / `build-dist` | bare `cargo build` | No |
| `just test-clippy-targets` | bare cargo / nix develop | No (duplicated CI_LOW_MEM branch) |
| `just dev` / `dev-ci` | interactive shell | N/A (leave interactive) |
| Product TUI runtime | not in scope | untouched |
| Prior nice/ionice in tree | none found | — |

## Mechanism

Central wrapper: [`scripts/run-nice.sh`](../../scripts/run-nice.sh)

- Default: `nice -n 19 ionice -c3 <cmd…>` when both exist; else `nice -n 19`; else bare exec.
- Escape: `GROK_NO_NICE=1` skips renice.
- Children inherit niceness (cargo rustc jobs, nix client children, mem-guard).

Wired only at the two private just chokepoints (plus gap closes for install/build-dist/clippy-targets):

1. **`cargo-ci`** — `exec "${run_nice}" …` for both normal and `CI_LOW_MEM=1` (outer nice around `nix develop … cargo-mem-guard`).
2. **`nix_retry`** — each attempt: `"${run_nice}" {{ cmd }}`.
3. **`install` / `build-dist`** — cargo step now `just cargo-ci cargo build …` (inherits nice + env hygiene).
4. **`test-clippy-targets`** — uses `just cargo-ci` instead of duplicated bare cargo / nix develop.

Header comment in `justfile` documents the contract. Minimal agent pin in project `AGENTS.md` hard constraint **3a** (prefer `just cargo-ci` / prefix when calling cargo outside just).

**Not reniced (intentional):** interactive `just dev` / `dev-ci`; product binary recipes (`limits-multipoll`, live limits); pure shell helpers (assert-process-pins, recon-status, upstream git scripts).

**Note:** nix-daemon build workers are separate from the client process; this renices the client invocation and any local non-daemon work. That still covers host cargo (the main UI fight) and the nix CLI / develop wrappers used by just.

## Files changed

| File | Change |
|------|--------|
| `scripts/run-nice.sh` | **New** central nice/ionice wrapper |
| `justfile` | Header note; cargo-ci + nix_retry wrap; install/build-dist/test-clippy-targets use cargo-ci |
| `AGENTS.md` | One sentence on max-nice via just / prefix |

## How to verify

```bash
# Probe script
printf '%s\n' '#!/usr/bin/env bash' 'echo ni=$(nice)' 'echo io=$(ionice -p $$)' > /tmp/print-prio.sh
chmod +x /tmp/print-prio.sh

./scripts/run-nice.sh /tmp/print-prio.sh
# expect: ni=19, io=idle

just cargo-ci /tmp/print-prio.sh
# expect: ni=19, io=idle

GROK_NO_NICE=1 just cargo-ci /tmp/print-prio.sh
# expect: ni=0, io=none (or best-effort)

NIX_RETRY_ATTEMPTS=1 just nix_retry /tmp/print-prio.sh
# expect: ni=19, io=idle

# Real gate entry (inherits via cargo-ci / nix_retry):
# just check   # or just test / just flake-meta
```

Verified on this host 2026-08-11:

- bare: `ni=0`
- `run-nice` / `just cargo-ci` / `just nix_retry`: `ni=19`, `io=idle`
- `GROK_NO_NICE=1 just cargo-ci`: `ni=0`

## Done / not done

| Item | State |
|------|--------|
| Central wrapper | Done |
| `just check` / `ci` / `test` chain reniced | Done (via cargo-ci + nix_retry) |
| nix recipes via nix_retry reniced | Done |
| install / build-dist / test-clippy-targets gaps | Done |
| Agent one-liner (AGENTS + justfile header) | Done |
| Product TUI cargo-as-tool renice | Out of scope (none intentional path required) |
| Renice nix-daemon workers system-wide | Not done (not client-side; would need host/nix config) |
| git commit | Not done (operator-owned) |
