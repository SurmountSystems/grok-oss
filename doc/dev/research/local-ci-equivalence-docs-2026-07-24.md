# Local quality gate vs GHA CI — docs equivalence (2026-07-24)

Read-only inventory. No product code or living-doc edits.

## 1. Do we claim local `just check` / `just ci` ≡ GHA quality?

**Yes, with soft wording in most places; one hard false entrypoint claim.**

| Path | Claim (exact or near-exact) |
|------|-----------------------------|
| `justfile` L2 | `GitHub Actions runs the same \`just ci\` entrypoint -- keep this file the source of truth.` |
| `justfile` L6 | `Full quality gate matching GHA: \`just ci\`  (or \`just test\` for fmt/clippy/tests only).` |
| `justfile` L121–124 | GHA runs flake-meta, ci-prep, and `just test`; then `` `just check` / `just ci` = full local gate (same idea as GHA quality). `` |
| `justfile` L140 | `Full local gate matching GHA quality (flake + prep + all tests/lints).` |
| `justfile` L137–144 | `check: ci`; `ci` = `flake-meta` → `ci-prep` → `test` |
| `FORK.md` § *CI and local quality* (L98–110) | Table: **`just check`** or **`just ci`** = full local gate; **GHA quality job: flake-meta → ci-prep → `just test`** (see `ci.yml`). |
| `AGENTS.md` § *CI and quality* (L52–56) | `Full local gate (same idea as GHA quality): **just check** or **just ci**.` |
| `RESIDUAL.md` L38, L43–47 | `just check` ≡ `just ci` only (not ≡ GHA); `just check # or just ci` before push |
| `README.md` | Prefers `just check` as full gate before push (no GHA entrypoint claim audited line-by-line here) |

**RESIDUAL** does **not** claim local ≡ GHA; it claims `check` ≡ `ci` and lists “CI checks-only” as resolved.

## 2. Is the claim accurate (workflow entrypoint matches justfile)?

**Partially.**

- **Recipe chain matches `just ci`:** GHA quality does `flake-meta` → `ci-prep` → `just test`, which is what `ci` runs.
- **Entrypoint does *not* match:** `.github/workflows/ci.yml` never runs `just ci` or `just check`. The job is named `just test`; the step runs `nix shell .#just -c just flake-meta`, then `… just ci-prep`, then `… just test`, with an **outer** bootstrap retry (4 attempts, backoff) only around flake-meta/ci-prep until `.ci-started`.
- **False/over-strong:** justfile L2 “runs the same `just ci` entrypoint.”
- **Accurate soft wording:** FORK + AGENTS “same idea as GHA quality”; FORK’s GHA step list matches `ci.yml`.
- **Env divergence (always on GHA, not default local):** `CI_SYSTEM=x86_64-linux`, `CI_LOW_MEM=1`, `NIX_BUILD_CORES=2`, workflow `NIX_CONFIG` (mirrors justfile export), 8G swap step, `ubuntu-latest`, 180m timeout, concurrency cancel-in-progress.
- **Local `just ci` without `CI_LOW_MEM`:** cargo via host PATH (`cargo-ci` does not enter `devShells.ci` / cargo-mem-guard). GHA always wraps cargo under mem-guard + mold via `.#ci`.

## 3. Known gaps (local-green / CI-red without a “real” product bug)

1. **Memory / job caps** — GHA `CI_LOW_MEM=1` → cargo-mem-guard, capped nix/cargo jobs; OOM, kill, or different parallelism vs a fat local machine.
2. **Toolchain path** — GHA: `nix shell .#just` + low-mem `nix develop .#ci`. Local default: system/dev cargo unless `CI_LOW_MEM=1`.
3. **OS / arch** — GHA only `ubuntu-latest` / `x86_64-linux`. Local darwin or aarch64 can pass or fail differently (also `test-extra` cross-clippy is local-only, not GHA).
4. **Cold store / network** — Free GHA nix cache and flake eval flakiness; outer + `nix_retry` retries can still exhaust; warm local nix can hide this (and the reverse: local offline fails where CI caches hit).
5. **Env / secrets hygiene** — `cargo-ci` unsets `NO_COLOR`, `CARGO_TERM_COLOR`, `OPENROUTER_API_KEY`, sets harness secret disables. Running raw `cargo nextest` outside `just` can red locally; rarely green-local/red-CI if something re-injects secrets only in GHA (not configured today).
6. **No nextest retry profile found** — flaky tests fail once on both; not a local-vs-CI retry gap, but GHA outer retry does **not** re-run cargo after `.ci-started`.
7. **Scope confusion** — Local `just test` alone skips flake-meta/ci-prep; green `just test` ≠ full `just ci` / GHA prep. Conversely `test-extra` is local-only and can red without affecting GHA.
8. **Swap / runner resources** — GHA adds ~8G swap; local disk/memory pressure differs.

## 4. Doc edit recommended (do not apply here)

Tighten justfile header L2 (and L6/L140 “matching GHA”) to match FORK/AGENTS: **same recipe chain as GHA quality** (`flake-meta` → `ci-prep` → `test`), **not** “GHA invokes `just ci`.” One sentence: GHA always sets `CI_LOW_MEM=1` / `CI_SYSTEM=x86_64-linux`, so closest local repro is `CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci` on Linux. No FORK/AGENTS change required beyond optional cross-link to that env note; FORK’s GHA step list is already correct.

## Verdict

| Question | Answer |
|----------|--------|
| Claim local full gate ≈ GHA quality? | **Yes** (FORK, AGENTS, justfile “same idea”) |
| Claim GHA runs `just ci`? | **Yes in justfile L2 — inaccurate** |
| Chain equivalent? | **Yes** (same three steps) |
| Env/runtime equivalent by default? | **No** (`CI_LOW_MEM` / OS / nix shell) |
