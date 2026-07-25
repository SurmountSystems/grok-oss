# Flake / CI hermeticity — what landed (2026-07-24)

**Prior research (do not re-litigate):**
- [`flake-hermeticity-inventory-2026-07-24.md`](./flake-hermeticity-inventory-2026-07-24.md)
- [`flake-hermeticity-path-trace-2026-07-24.md`](./flake-hermeticity-path-trace-2026-07-24.md)
- Trigger: doctor_cmd local-green / GHA-red when host had mic recorders
  ([`ci-doctor-cmd-fail-2026-07-24.md`](./ci-doctor-cmd-fail-2026-07-24.md))

## What changed

| Path | Change |
|------|--------|
| `scripts/with-ci-hermetic-path.sh` | **New.** After impure `nix develop .#ci`, rebuild `PATH` as allowlist of `/nix/store/*` entries only; `exec` the rest of argv. Escape: `GROK_CI_ALLOW_HOST_PATH=1`. |
| `justfile` `cargo-ci` | Under `CI_LOW_MEM=1`: `nix develop .#ci -c ./scripts/with-ci-hermetic-path.sh cargo-mem-guard -- <cmd>`. Document env + PATH hygiene in recipe comments and file header. |
| `flake.nix` `packages.ci-tools` | Add `pkgs.git` so scrubbed PATH still has VCS for cargo git deps and git-using unit tests. Comment: **do not** add audio recorders. |
| `FORK.md` | Short CI PATH hermeticity note. |

**Not changed (by design):**
- Interactive `devShells.default` / `just dev` — still impure host PATH for daily coding.
- Crane pure packages / checks — already sandboxed.
- `cargo-mem-guard` — still jobs/memory/mold only; PATH policy lives in `cargo-ci`.
- doctor_cmd test that excludes `VOICE_NO_INPUT_DEVICE_ID` from shared-view assertion — kept (complementary seam; not a substitute for PATH scrub).
- Default local `just cargo-ci` without `CI_LOW_MEM` — still host PATH (fast host-dev; not GHA-equivalent).

## How it works

```text
CI_LOW_MEM=1 just cargo-ci <cmd>
  → env hygiene (NO_COLOR, CARGO_TERM_COLOR, OPENROUTER_API_KEY, harness flags)
  → nix develop .#ci          # ci-tools + stdenv store bins PREPEND; host PATH still after
  → with-ci-hermetic-path.sh  # PATH := only /nix/store/* components (allowlist)
  → cargo-mem-guard -- <cmd>  # cargo/nextest/tests see store-only PATH
```

Allowlist (not denylist of three recorder names): anything not under `/nix/store`
is dropped, so future optional desktop tools cannot leak without a deliberate
change.

## Verification (this machine, 2026-07-24)

Host had recorders on ambient PATH (`/usr/bin/parec`, `/usr/bin/pw-record`).
Probe script printed `command -v` results under each wrapper.

| Probe | Result |
|-------|--------|
| Host ambient | `parec` + `pw-record` under `/usr/bin` |
| `nix develop .#ci -c ./scripts/with-ci-hermetic-path.sh <probe>` | recorders **absent**; cargo/git/mold from store; `host-leak-dirs=0`, `nix-store-dirs=22` |
| `CI_LOW_MEM=1 just cargo-ci <probe>` | same scrub (via mem-guard); recorders **absent**; git from ci-tools |
| `GROK_CI_ALLOW_HOST_PATH=1 CI_LOW_MEM=1 just cargo-ci <probe>` | host recorders **visible** again (escape hatch) |
| `nix develop .#ci -c <probe>` alone (no scrub) | host recorders still visible (impure develop unchanged) |

## Residual risks

1. **Non-LOW_MEM `just ci` / `just test`** still uses full host PATH — not GHA-equivalent; documented. Prefer `CI_LOW_MEM=1` when claiming local≡CI.
2. **Bare `cargo test` / IDE** — no scrub; escape hatch for host-dev.
3. **`TestSandbox` children** still inherit the *test process* PATH (now hermetic under LOW_MEM quality) but allowlist is parent PATH on Unix — fine when parent was scrubbed.
4. **New host-only tool needed by a test** — must go into `ci-tools` (or an explicit opt-in), not rely on desktop PATH under LOW_MEM.
5. **ci-tools rebuild cost** — adding `git` invalidates the ci-tools buildEnv once; cold `ci-prep` slightly larger, still no audio packages.
6. **macOS / other store layouts** — allowlist is `/nix/store/*` only; matches standard Nix multi-user installs used here and on GHA Linux.

## Acceptance checklist

- [x] CI cargo funnel does not resolve host `pw-record` / `parec` / `arecord`
- [x] Allowlist PATH (store bins), not recorder denylist
- [x] No audio recorders in `ci-tools` / `devShells.ci`
- [x] Interactive default shell still impure
- [x] Documented in justfile + FORK + this note
- [x] doctor_cmd voice-id exclusion left intact
- [x] No agent git commit/push
