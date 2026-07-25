# Flake hermeticity inventory — 2026-07-24

Read-only map of how Nix shells/packages, `just cargo-ci`, and GHA compose
`PATH` and env for this repo. Trigger: local-green / CI-red on
`doctor_cmd::tests::fake_standalone_facts_compose_through_shared_view` because
host `pw-record`/`parec`/`arecord` were on developer `PATH` and absent on
headless GHA (see
[`ci-doctor-cmd-fail-2026-07-24.md`](./ci-doctor-cmd-fail-2026-07-24.md)).
Test assertion was fixed; this note covers **flake / shell hermeticity** so
local-with-nix ≈ CI without papering over the next host-tool leak.

Related:
- [`local-ci-equivalence-docs-2026-07-24.md`](./local-ci-equivalence-docs-2026-07-24.md) — recipe chain vs env
- `flake.nix`, `justfile`, `.github/workflows/ci.yml`

---

## 1. Inputs and layout

| Item | State |
|------|--------|
| `flake.nix` | Single file; no `nix/` overlays dir |
| Inputs | `nixpkgs` (nixos-unstable), `fenix` (follows nixpkgs), `crane` |
| `flake.lock` | Present; fenix pins toolchain via `rust-toolchain.toml` sha |
| Pure monorepo builds | Crane `buildPackage` / checks use sandbox + `strictDeps = true` |
| Host CI path | **Not** pure nix monorepo build; host cargo under `devShells.ci` |

There is **no** flake-level use of `--pure`, `nix develop --ignore-environment`,
or `NIX_ENFORCE_PURITY`. Comments call crane builds and mem-guard wrapping
“pure Nix / no host PATH” for **those packages’ runtime closures only**.

---

## 2. Packages / devShells and what they put on PATH

### Packages (`packages.*`)

| Attr | What it is | PATH / runtime tooling |
|------|------------|-------------------------|
| `default` / `grok-oss` | Crane-built pager binary | Wrap: `LD_LIBRARY_PATH` for openssl (+ dbus on Linux). No host PATH. Sandboxed build. |
| `just` | Locked `pkgs.just` only | Single binary for GHA bootstrap (`nix shell .#just`) |
| `ci-tools` | `buildEnv` of CI toolchain | **bin:** fenix rustc/cargo/…, `cargo-mem-guard`, `cargo-nextest`, `pkg-config`, `protoc`, `cmake`, `openssl`, `perl`, `rg`, `just`; Linux also `mold`, `dbus` |
| `cargo-mem-guard` | Crane package; Linux `symlinkJoin` + `makeWrapper` | **prefix PATH** with nix `mold`; default `CARGO_MEM_USE_MOLD=1`. Intentional: mold without ambient host PATH |
| `cargo-mem-guard-unwrapped` | Same without mold wrap | No PATH mutation |

**Not on `ci-tools` / any shell:** PipeWire / Pulse / ALSA recorders
(`pw-record`, `parec`, `arecord`), system `git` (except default devShell has
`pkgs.git`), browsers, etc. GHA and a clean nix shell therefore **lack** mic
recorders; a full desktop PATH often has them.

### env blocks (not packages)

| Name | Set by | Notable vars |
|------|--------|--------------|
| `ciLowMemEnv` | `.#ci` shell (`env =`); partially inherited by default shell | `CARGO_MEM_*`, `PROTOC`, `OPENSSL_NO_VENDOR`, `GROK_*_BUNDLE_RG_PATH` → store `rg`, `PKG_CONFIG_PATH`, `LD_LIBRARY_PATH`, `NIX_HARDENING_ENABLE` without fortify (jemalloc probe) |
| Crane `commonArgs` | pure builds | Same protoc/rg/openssl flags; `CARGO_BUILD_JOBS=2`; mold `RUSTFLAGS` on Linux |

### devShells

| Shell | packages | Env | Host PATH behavior |
|-------|----------|-----|--------------------|
| `default` (`nix develop` / `just dev`) | fenix toolchain, rust-analyzer, pkg-config, protobuf, cmake, openssl, **git**, ripgrep, cargo-mem-guard, cargo-nextest; Linux: dbus, mold | Subset of `ciLowMemEnv` (PROTOC, OPENSSL_NO_VENDOR, RG paths, NIX_HARDENING_ENABLE) | **Impure:** `mkShell` **prepends** nix bins; **keeps** ambient host `PATH` |
| `ci` (`nix develop .#ci` / `just dev-ci`) | single package: `ci-tools` | full `ciLowMemEnv` | **Same impurity:** nix bins first, then host `PATH` |

### Flake checks (sandbox / pure build)

| Check | Notes |
|-------|--------|
| `grok-oss`, `cargoCheck`, `openrouter-credentials`, `cargo-mem-guard`, `cargo-mem-guard-tests` | Crane derivations; sandbox; no host mic tools. **GHA quality job does not run these** as the main suite (optional local / mem-guard path only). |

---

## 3. How cargo/tests get PATH today

```text
GHA (.github/workflows/ci.yml)
  env: CI_LOW_MEM=1, CI_SYSTEM=x86_64-linux, NIX_CONFIG, NIX_BUILD_CORES=2
  nix shell .#just -c just flake-meta
  nix shell .#just -c just ci-prep   # builds .#cargo-mem-guard + .#ci-tools
  nix shell .#just -c just test
    → just cargo-ci <cargo|nextest …>
         CI_LOW_MEM=1 → nix develop .#ci -c cargo-mem-guard -- <cmd>
         # PATH = ci-tools bins + GHA ubuntu user PATH (usually no pw-record)

Local default (no CI_LOW_MEM)
  just cargo-ci <cmd>
    → exec <cmd> on ambient host PATH (rustup/system cargo, full desktop PATH)
    → only env scrub: unset NO_COLOR, CARGO_TERM_COLOR, OPENROUTER_API_KEY;
      set harness secret disables + loopback proxy trust

Local closest to GHA
  CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci
    → same nix develop .#ci wrap as GHA
    → still inherits host PATH after nix bins (pw-record still visible on NixOS/desktop)
```

`cargo-ci` does **not** scrub `PATH`. Entering `.#ci` does **not** isolate from
host mic tools if they remain later on `PATH`.

`TestSandbox` (integration children) `env_clear()` then allowlists parent
`PATH` on Unix — children still see host tools. In-process unit tests use the
test process `PATH` directly (voice probe → `binary_on_path`).

---

## 4. Impurity vectors (ranked by local ≠ CI impact)

| Rank | Vector | Why it hurts | Example |
|------|--------|--------------|---------|
| **1** | Host `PATH` leakage into unit tests | Product probes optional tools via `PATH`; assertions on issue counts / findings flip with desktop vs GHA | `pw-record`/`parec`/`arecord` → `voice.no-input-device` (doctor_cmd, any live `apply_voice_probe`) |
| **2** | Default local `cargo-ci` skips `.#ci` | Without `CI_LOW_MEM=1`, different rustc, no mem-guard, no mold wrap, full host PATH | Local `just check` green; GHA OOM or toolchain drift |
| **3** | `nix develop` / `mkShell` is not pure | Even with (2) fixed, host PATH still after store bins | Local `CI_LOW_MEM=1 just test` can still see mic tools |
| **4** | Env secrets / color already partially scrubbed | Asymmetric: some vars cleaned, PATH not | `OPENROUTER_API_KEY` unset is good; optional tools remain |
| **5** | `TestSandbox` preserves parent `PATH` | Headless e2e children inherit host bins | Less often flips pure unit asserts; can flip spawn/discovery tests |
| **6** | Impure `nix eval` for `system` | Local just parse uses `--impure` currentSystem; GHA sets `CI_SYSTEM` | Attr path selection only; low product impact |
| **7** | Hardcoded `/usr/bin` candidates in product | Shell resolution falls back to fixed dirs after `which` | Config shell lookup; usually host-dev intentional |
| **8** | Pure crane builds unused in GHA quality | Sandbox hermeticity exists but quality is host-cargo | Packaging path is hermetic; CI quality path is not |

**Not a flake impurity (by design):** product runtime discovery of mic
recorders on the user’s machine for real voice dictation. Tests and CI shells
must not treat “recorder present” as a stable fixture unless they inject it.

---

## 5. What “sufficiently hermetic” should mean here

Goal: **local-with-nix quality gate ≈ GHA quality**, not full NixOS pure eval of
every developer keystroke.

1. **Tests under `just cargo-ci` / GHA must not change pass/fail solely because
   optional host utilities are on `PATH`** (mic recorders, clipboard helpers,
   desktop session tools). Prefer inject seams, fake facts, or assert only on
   fixtures under test (pattern already used for doctor shared-view + voice id
   exclusion).
2. **CI / low-mem shell supplies the compile toolchain only** (rustc, nextest,
   protoc, mold, openssl, dbus, rg paths). It should **not** silently add
   desktop audio tools just to green a probe test.
3. **`CI_LOW_MEM=1` + `.#ci` is the reference runtime** for “like CI.” Docs
   already say closest repro is
   `CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci`.
4. **Pure crane `nix build` remains the packaging sandbox**; quality stays
   host-cargo under mem-guard (memory / free-GHA constraint).
5. **Host-dev without Nix remains valid** for interactive coding (`cargo`
   / rustup / `just cargo-ci` without low-mem). Hermeticity knobs apply when
   claiming “same as CI,” not when forcing every local edit through pure shell.

---

## 6. Concrete fix options (least invasive first)

| # | Option | Pros | Cons | Prefer? |
|---|--------|------|------|---------|
| A | **Test/product seams** — inject `detect_recorder` / voice probe; assert without host-dependent findings; keep product PATH discovery for real users | Surgical; already fixed doctor_cmd; no shell UX break | Must find every host-probe test; ongoing discipline | **Yes — primary** |
| B | **`cargo-ci` PATH scrub** when running tests — e.g. build `PATH` from `command -v cargo` dir + known needful bins, or strip known optional names; optional `HERMETIC_PATH=1` | Stops class of leaks without pure shell | Easy to over-strip (`git`, linkers); must list keep/deny carefully | **Yes — secondary for `just test`** |
| C | **Always enter `.#ci` for `cargo-ci`** (not only `CI_LOW_MEM`) — mem-guard optional via flag | Aligns toolchain/PATH prefix with GHA | Slower cold start; still **does not** remove host PATH tail | Partial; combine with B or D |
| D | **`nix develop --ignore-environment .#ci -c …`** (or pure-ish wrapper) for `cargo-ci` under low-mem / always-CI mode | Strong isolation: only shell env + packages | Breaks if tests need host `git`, locales, `HOME`, credentials; need explicit re-exports | For CI-like only; careful allowlist |
| E | **Add intentional tools to `ci-tools`** (e.g. alsa-utils) so probe always “finds” a recorder | Local+CI both green the same way | **Wrong direction** for headless GHA (fake success; no real device); masks product “missing mic” path | **No** for mic tools |
| F | Rely on `strictDeps` / pure crane only | Already on packages | Does not run full nextest suite in GHA | Keep for packaging; not quality fix |
| G | `NIX_ENFORCE_PURITY` / pure eval globally | Strong | Hostile to host-dev; wrong layer for cargo tests | **No** as default |

### Ranked recommendations (top 5)

1. **Keep fixing host-probe tests with inject/exclude seams** (A) — do not assert
   live `PATH` inventory in composition tests. Audit other `apply_voice_probe` /
   `binary_on_path` callers under tests.
2. **Optional `cargo-ci` hermetic PATH mode** (B) for `just test` / when
   `CI_LOW_MEM=1`: start from nix shell bins only, or deny-list mic tools + other
   known optional desktop bins; document keep-list (`git`, `cc` if needed).
3. **Document + optionally default local gate to GHA env** (C + docs):
   `CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci` as “true local CI”; leave
   bare `just cargo-ci` for fast host-dev.
4. **If PATH scrub is insufficient, use ignore-environment for low-mem cargo-ci
   only** (D) with an explicit env allowlist (`HOME`, `USER`, `TERM`, locale,
   `SSH_AUTH_SOCK` only if needed — prefer not for unit tests).
5. **Do not put audio recorders into `ci-tools`** (anti-E); product missing-mic
   behavior stays intentional on headless hosts.

---

## 7. Files that would need edits (list only)

| Path | Why |
|------|-----|
| `justfile` | `cargo-ci` PATH scrub / always-`nix develop` / `--ignore-environment` |
| `flake.nix` | Shell packages, pure shell helper, comments; **not** for adding mic tools |
| `.github/workflows/ci.yml` | Only if new env flags or develop invocation change |
| `crates/codegen/xai-grok-pager/src/doctor_cmd/tests.rs` | Already adjusted; further inject if needed |
| `crates/codegen/xai-grok-pager/src/diagnostics/**` | Inject seam for voice probe in tests |
| `crates/codegen/xai-grok-voice/src/audio/capture_linux.rs` | Already has `detect_recorder_with` for unit tests; export/inject for pager tests if required |
| `crates/codegen/xai-grok-test-support/src/sandbox.rs` | Optional: default child PATH policy for stricter e2e |
| `FORK.md` / `README.md` / research notes | Document “sufficiently hermetic” and closest GHA repro |
| `doc/dev/research/local-ci-equivalence-docs-2026-07-24.md` | Cross-link PATH impurity |

No new `nix/` tree required unless overlays grow beyond one file.

---

## 8. Non-goals

- Break **host-dev without Nix** (rustup + ambient PATH for day-to-day edit).
- Force every `cargo test` through pure sandbox crane builds on free GHA.
- Make product stop discovering user mic tools at runtime on real machines.
- Add PipeWire/Pulse/ALSA packages to `ci-tools` so probes always succeed.
- Global pure flake eval / `NIX_ENFORCE_PURITY` for interactive `nix develop`.
- Claim local default `just check` ≡ GHA without `CI_LOW_MEM` (already false;
  see local-ci-equivalence note).

---

## 9. Summary diagram

```text
                    ┌─────────────────────────────────────┐
  pure crane build  │ sandbox PATH = buildInputs only     │  packaging only
                    └─────────────────────────────────────┘

  GHA / CI_LOW_MEM  │ nix develop .#ci                     │
                    │ PATH = [ci-tools…] + host PATH  ◄── still impure tail
                    │ cargo-mem-guard → cargo/nextest      │

  local default     │ host PATH only (full desktop)        │  mic tools common
                    │ cargo-ci env scrub (keys/color only) │
```

**Bottom line:** the flake is already careful for **package closures**
(`strictDeps`, mold wrap, store `PROTOC`/`rg`). The quality path is **host-cargo
in an impure `mkShell`**, so optional host binaries remain the main
local≠CI risk. Prefer test seams + optional `cargo-ci` PATH policy over
pretending `devShells.ci` is pure.
