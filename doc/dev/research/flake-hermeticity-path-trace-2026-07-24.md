# Flake / CI PATH hermeticity — how PATH reaches cargo test / nextest

**Date:** 2026-07-24  
**Scope:** read-only trace (justfile, flake, cargo-mem-guard, GHA). No code changes.  
**Related:** `doc/dev/research/local-ci-equivalence-docs-2026-07-24.md`,  
`doc/dev/research/ci-doctor-cmd-fail-2026-07-24.md` (voice probe / `pw-record` PATH skew).

## Short PATH story

| Path | How cargo/nextest is invoked | PATH shape | Host `/usr` first? |
|------|------------------------------|------------|--------------------|
| **A) GHA quality** | `nix shell .#just -c just test` → `cargo-ci` → `nix develop .#ci -c cargo-mem-guard -- cargo nextest …` | Nix `mkShell` **prepends** `ci-tools` store bins (fenix cargo, nextest, mold, …); **host PATH remains after** | **No** (nix store first). Host `/usr` still present afterward. |
| **B1) `just ci` / `just check` with `CI_LOW_MEM=1`** | Same funnel as GHA cargo steps | Same as A | Same as A |
| **B2) `just ci` / `just check` without `CI_LOW_MEM`** | `cargo-ci` does `exec cargo nextest …` (no develop, no mem-guard) | **Full ambient host PATH** (developer shell) | **Yes if host puts `/usr` first** (typical) |
| **C) bare `cargo test` / `cargo nextest`** | Outside just | Ambient host PATH; no `cargo-ci` env hygiene | Host-controlled |

**Does `nix develop .#ci` scrub PATH?** **No.** `pkgs.mkShell` / impure `nix develop` adds package bins and env (`ciLowMemEnv`); it does not clear host PATH.

**Does `cargo-mem-guard` scrub PATH?** **No.** It inherits the full parent env, sets `CARGO_BUILD_JOBS` / mold-related `RUSTFLAGS`, and spawns the child. Install-time `wrapProgram` only `--prefix PATH : mold` for the **guard binary itself** (Linux package), not a hermetic test PATH.

**Bottom line:** quality cargo under GHA / `CI_LOW_MEM=1` gets **nix tools first + full host PATH after**. Local default `just ci` skips nix entirely and uses the developer PATH. Neither layer is test-PATH-hermetic. That is why a desktop with `pw-record`/`parec` can green while headless GHA reds on ambient voice probes (see doctor_cmd note).

---

## Call graph (authoritative sources)

### A) GHA quality job

File: `.github/workflows/ci.yml`

- Job env always: `CI_LOW_MEM=1`, `CI_SYSTEM=x86_64-linux`, shared `NIX_CONFIG`, `NIX_BUILD_CORES=2`.
- Runner: `ubuntu-latest` (host PATH = standard Ubuntu; **no** PipeWire/`pw-record` by default).
- Steps (not the `just ci` entrypoint; same recipe chain):
  1. `nix shell .#just -c just flake-meta`
  2. `nix shell .#just -c just ci-prep` → mem-guard package + `nix build .#ci-tools` + `.ci-started`
  3. `nix shell .#just -c just test` → fmt / clippy / nextest / doc / mem-guard crate tests

`nix shell .#just` only realizes locked `pkgs.just` on PATH for that process; it does **not** enter `devShells.ci`. Cargo wrapping happens later inside `cargo-ci`.

### B) Local `just ci` / `just check`

File: `justfile`

```
check → ci → flake-meta → ci-prep → test
test  → test-fmt → test-clippy → test-unit → test-doc → test-mem-guard
each  → cargo-ci <cmd>
```

**`ci-prep`:** builds `.#cargo-mem-guard` (+ flake unit check) and realizes `.#ci-tools`. Does not run tests; does not rewrite PATH for later steps.

**`cargo-ci` (single funnel for quality cargo):**

1. Env hygiene (always): unset `NO_COLOR`, `CARGO_TERM_COLOR`, `OPENROUTER_API_KEY`; set
   `RULES_RUST_RUNFILES_WORKSPACE_NAME`, `GROK_DISABLE_SHARED_HARNESS_SECRETS`,
   `GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY`.
2. **If `CI_LOW_MEM=1`:**
   ```bash
   exec nix develop $nix_low_mem_opts .#ci -c cargo-mem-guard -- <cmd>
   ```
3. **Else:**
   ```bash
   exec <cmd>
   ```

So:

- **With `CI_LOW_MEM=1`:** PATH = develop(ci-tools prepend) + host remainder → mem-guard → cargo/nextest → test bins.
- **Without:** PATH = whatever the developer shell had when they ran `just`.

Closest GHA repro (already documented in justfile header):  
`CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci` on Linux.

### C) Bare `cargo test` outside just

No `cargo-ci`, no mem-guard, no develop. Host rustup/cargo/nextest and full PATH. Also no secret/color hygiene — local bare runs can diverge from `just test` even beyond PATH.

---

## Flake pieces (PATH-related)

File: `flake.nix`

| Attr | Role | PATH behavior |
|------|------|----------------|
| `packages.just` | GHA bootstrap only | `nix shell .#just` adds just; host PATH kept |
| `packages.ci-tools` | `buildEnv` of fenix toolchain, cargo-nextest, cargo-mem-guard, protoc, cmake, openssl, perl, rg, mold/dbus on Linux | Realized by `ci-prep`; **not** auto on PATH unless develop/shell |
| `devShells.ci` (`ciShell`) | `packages = [ ci-tools ]` + `env = ciLowMemEnv` | `nix develop .#ci` **prepends** those bins; **impure** host PATH retained |
| `devShells.default` | Interactive fenix + nextest + mem-guard | Same prepend pattern; shellHook is echo-only (no PATH scrub) |
| `packages.cargo-mem-guard` | Linux: `wrapProgram --prefix PATH : mold --set-default CARGO_MEM_USE_MOLD 1` | Hermetic **only for finding mold** when the guard runs; child still sees full PATH |
| Crane pure builds (`grok-oss`, checks) | Nix sandbox | **Different story** (sandboxed PATH). Not the host-cargo quality job |

`ciLowMemEnv` sets mem-guard knobs, `PROTOC`, `OPENSSL_NO_VENDOR`, bundled `rg` paths, `PKG_CONFIG_PATH`, `LD_LIBRARY_PATH`, `NIX_HARDENING_ENABLE` (fortify off for jemalloc probes). **No PATH key.**

Comment at cargo-mem-guard package (“pure Nix, no host PATH / bash scripts”) means the **mold wrap is pure Nix**, not that CI test processes run with a scrubbed PATH.

---

## cargo-mem-guard (wrapper semantics)

File: `crates/codegen/cargo-mem-guard/src/main.rs`

- Reads `PATH` only to detect `mold` (`mold_on_path`).
- Child: `Command::new(program)` with inherited env + selective `env` / `env_remove` for rustflags / jobs.
- **No** `env_clear`, **no** PATH rewrite for cargo/nextest/tests.
- `nextest` gets build-jobs handling via argv/env, not PATH isolation.

---

## nix develop vs nix shell (this repo)

| Invocation | Used for | Effect on cargo PATH |
|------------|----------|----------------------|
| `nix shell .#just -c just …` | GHA outer steps; local can do the same | Only `just` from flake; cargo later may re-enter develop |
| `nix develop .#ci -c cargo-mem-guard -- …` | `cargo-ci` when `CI_LOW_MEM=1` | Full ci-tools on PATH **first**, host PATH **still after** |
| `nix develop` / `just dev` | Interactive default shell | Same impure prepend model |
| `nix shell .#ci-tools -c …` | Documented ad-hoc (flake comment) | buildEnv bins prepended; host PATH kept (shell, not pure sandbox) |

There is **no** `nix develop --ignore-environment` / pure flag in justfile or GHA today.

---

## Voice / recorder PATH usage (quick)

Product Linux capture probes ambient PATH:

- `xai-grok-voice` `capture_linux.rs`: `binary_on_path` walks `PATH` for `pw-record` → `parec` → `arecord`.
- `xai-grok-pager` `diagnostics::apply_voice_probe` → `input_device_info()`; standalone doctor always probes with `emit_missing_issue=true`.
- Unit tests can inject availability (`detect_recorder_with`); **process-global** doctor/view tests that call real `apply_voice_probe` still see ambient PATH.
- Subprocess product tests that use `TestSandbox` get `env_clear` + minimal allowlist (hermetic for **those** children only — not for in-process unit tests under nextest).

Ambient skew symptom: local green / GHA red on issue counts when mic tools exist only on the developer PATH (fixed for one doctor_cmd test by excluding `VOICE_NO_INPUT_DEVICE_ID` from the assertion; product probe behavior unchanged).

---

## Does anything scrub PATH today?

| Layer | Scrubs PATH? |
|-------|----------------|
| GHA workflow | No |
| `just` `ci` / `test` / `ci-prep` | No |
| `cargo-ci` | No (hygiene is other env vars only) |
| `nix develop .#ci` / `mkShell` | No (prepend only) |
| `nix shell .#just` / `.#ci-tools` | No |
| `cargo-mem-guard` process | No |
| cargo-mem-guard **package** wrap | Prefix mold only |
| nextest config in-repo | None found (no workspace nextest.toml env policy) |
| Crane pure checks | Sandbox (not host quality path) |
| `TestSandbox` children | Yes for **spawned** product binaries only |

---

## Recommended hook for hermetic test PATH under `just ci`

**Primary hook: `justfile` `cargo-ci`.**  
It is the only shared gate for fmt, clippy, nextest, doctests, and the excluded mem-guard crate tests — both GHA (via `just test` + `CI_LOW_MEM=1`) and local `just ci` / `just check` / `just test`.

### Why not other layers

| Hook | Why weaker |
|------|------------|
| `cargo-mem-guard` only | Runs **only** when `CI_LOW_MEM=1`; default local `just ci` never enters it |
| `nix develop .#ci` shellHook alone | Skipped when `CI_LOW_MEM` unset; impure develop still inherits host unless pure/`-i` |
| nextest config only | Misses fmt/clippy/doc and non-nextest `cargo test` |
| Per-test `env_clear` | Correct for product children; does not fix in-process unit tests that walk process PATH |

### Minimal change shape (proposal only — not applied)

1. **In `cargo-ci`, after existing unset/export hygiene**, construct a deliberate PATH for the cargo payload:
   - **`CI_LOW_MEM=1` path:** keep using `nix develop .#ci` so fenix/nextest/mold stay first; then either:
     - **A (minimal / low risk):** drop known audio recorders from effective visibility without full scrub, e.g. prepend a tiny empty/guard dir and **remove** host dirs only if the goal is “match headless GHA for mic probes” (narrow), or
     - **B (stronger hermetic):** after develop is active, set  
       `PATH="$NIX_CI_BIN:…essential…"` where `$NIX_CI_BIN` is the mkShell-prepended store path(s), and **omit** host `/usr` `/usr/local` for the cargo child — still allow `/bin` `/usr/bin` only if a tool is truly required (git, bash for build scripts). Prefer listing **allow** entries over deny-list for recorders.
   - **Non-`CI_LOW_MEM` local `just ci`:** either document “not PATH-equivalent to GHA” (status quo) or **always** enter `nix develop .#ci` for cargo (heavier, closer parity) and apply the same PATH policy so desktop `pw-record` cannot leak into nextest.

2. **Do not rely on cargo-mem-guard for PATH policy** unless you also always wrap through it; keep mem-guard focused on jobs/memory/mold.

3. **Optional pure develop:** `nix develop --ignore-environment .#ci -c …` is a bigger hammer (must re-pass `HOME`, `USER`, `TERM`, cache dirs, `SSL_CERT_FILE`, etc.). Prefer explicit PATH allowlist inside `cargo-ci` first.

4. **Tests that intentionally need host tools** should opt in via env (e.g. restore full PATH for a single integration suite) rather than making ambient desktop packages the default for workspace nextest.

### Acceptance for a future change

- `CI_LOW_MEM=1 just cargo-ci cargo nextest run -p xai-grok-pager --lib doctor_cmd::` on a machine **with** `pw-record` on host PATH behaves like GHA for voice findings (missing recorder) unless a test injects a fake.
- Compile still finds mold/protoc/pkg-config via nix (LOW_MEM) or documented host tools (non-LOW_MEM).
- Bare `cargo test` remains non-hermetic by design (escape hatch / IDE).

---

## Verdict (one screen)

```
GHA / CI_LOW_MEM just:
  host PATH
    → nix shell .#just (just only)
    → just test → cargo-ci
    → nix develop .#ci  (ci-tools PREPEND, host PATH STILL THERE)
    → cargo-mem-guard   (no PATH scrub; mold via wrap/prefix)
    → cargo nextest / cargo test
    → test process PATH = nix-bins : host-/usr-…

Local just without CI_LOW_MEM:
  cargo-ci → exec host cargo/nextest on full desktop PATH

Bare cargo:
  full desktop PATH, no cargo-ci hygiene

Hermetic? No for host-cargo quality.
Best hook: cargo-ci PATH policy (optionally always develop .#ci for parity).
```
