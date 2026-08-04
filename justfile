# Grok OSS local recipes.
# GHA quality uses the same recipe chain as `just ci` (flake-meta → ci-prep → test),
# not the `just ci` entrypoint itself -- keep this file the source of truth for those recipes.
# Requires: just, nix (with flakes). No bash scripts -- just recipes + nix.
#
# Bare `just` lists recipes (same idea as `just -l` / `just --list`).
# Full local gate (same recipe chain as GHA quality): `just ci`
#   (or `just test` for fmt/clippy/tests only).
# Closest GHA repro on Linux: CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci
# Under CI_LOW_MEM, cargo-ci scrubs PATH to nix-store bins only (no host
# pw-record/parec/arecord). Interactive `just dev` keeps impure host PATH.

set shell := ["bash", "-euo", "pipefail", "-c"]

# `just` with no recipe → list (just 1.x+; same as CLI `just --default-list` / `JUST_DEFAULT_LIST=1`).
set default-list

# Host system for flake check attributes (e.g. x86_64-linux).
# Prefer CI_SYSTEM (GHA sets it). Local default: uname map — do not call `nix`
# at just parse time (a broken host nix would fail every recipe). Recipe-time
# helper: scripts/nix-current-system.sh. Top-level backticks cannot expand
# {{ justfile_directory() }}, so keep uname inline here. Attribute sinks use
# {{ system }} only after require_system.
system := env_var_or_default("CI_SYSTEM", `case "$(uname -s)-$(uname -m)" in Linux-x86_64) echo x86_64-linux;; Linux-aarch64|Linux-arm64) echo aarch64-linux;; Darwin-x86_64) echo x86_64-darwin;; Darwin-arm64) echo aarch64-darwin;; *) echo "unsupported $(uname -s)-$(uname -m); set CI_SYSTEM=..." >&2; exit 1;; esac`)

# Nix flags when CI_LOW_MEM=1: cap cores/jobs for pure nix steps.
nix_low_mem_opts := if env_var_or_default("CI_LOW_MEM", "") == "1" { "--option cores 2 --option max-jobs 1" } else { "" }

# Free GHA (~4 vCPU / 16GB) sets CI_LOW_MEM=1.
low_mem := env_var_or_default("CI_LOW_MEM", "")

# ---------------------------------------------------------------------------
# Transient network resilience (free GHA flake-input / binary-cache flakes)
#
# NIX_CONFIG knobs -- keep in sync with .github/workflows/ci.yml job env:
#   download-attempts          -- retry individual downloads (default 5)
#   connect-timeout            -- TCP connect timeout seconds
#   stalled-download-timeout   -- abort hung transfers
#   http-connections           -- parallel HTTP fetches (lower = less flaky)
#
# nix_retry wraps whole `nix ...` invocations when per-download knobs are not
# enough (e.g. flake metadata 503 HTML). Backoff: 5s, 15s, 45s.
#
# IMPORTANT: retries EVERY non-zero exit (not network-classified). Permanent
# eval failures pay the full attempt budget + backoff. Cargo payloads are
# intentionally OUTSIDE nix_retry so permanent compile fails once.
# Local fail-fast: NIX_RETRY_ATTEMPTS=1 just mem-guard
# Override attempts: NIX_RETRY_ATTEMPTS=5 just mem-guard
#
# Security: +cmd is expanded as shell (trusted recipes only). Never pass
# untrusted user input as the nix_retry command string.
# ---------------------------------------------------------------------------
export NIX_CONFIG := '''
download-attempts = 5
connect-timeout = 30
stalled-download-timeout = 100
http-connections = 4
'''

# Fail fast if the host system string is not safe for shell/attr interpolation.
# Same source as `system` (CI_SYSTEM or scripts/nix-current-system.sh) — no
# host `nix` call here. Never interpolate `{{ system }}` into this recipe
# (single-quote in CI_SYSTEM must not break assignment).
# Recipes that expand `{{ system }}` into nix attr paths depend on this first.
[private]
require_system:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    case "${sys}" in
      x86_64-linux|aarch64-linux|x86_64-darwin|aarch64-darwin) exit 0 ;;
    esac
    if [[ "${sys}" =~ ^[a-zA-Z0-9_]+-[a-zA-Z0-9_]+$ ]]; then
      exit 0
    fi
    echo "==> invalid CI_SYSTEM / system (refuse shell interpolation): ${sys}" >&2
    echo "    expected e.g. x86_64-linux or ^[a-zA-Z0-9_]+-[a-zA-Z0-9_]+$" >&2
    exit 2

# Retry a nix (or other) command. Integer-validates NIX_RETRY_ATTEMPTS (default 4).
# Prints a clear banner per attempt. Permanent failures fail after all attempts.
# Retries every non-zero exit (not network-classified); use only around store
# realization / flake eval, never around host cargo compile payloads.
#
# Before the first attempt: ensure a working `nix` is first on PATH so a
# broken host binary does not burn the full retry budget. Override: NIX_BIN.
[private]
nix_retry +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    # shellcheck source=scripts/ensure-working-nix-path.sh
    source "{{ justfile_directory() }}/scripts/ensure-working-nix-path.sh"
    raw_attempts="${NIX_RETRY_ATTEMPTS:-4}"
    if [[ ! "${raw_attempts}" =~ ^[1-9][0-9]*$ ]]; then
      echo "==> nix_retry: NIX_RETRY_ATTEMPTS must be a positive integer, got: ${raw_attempts}" >&2
      exit 2
    fi
    attempts="${raw_attempts}"
    backoff=5
    n=1
    while true; do
      echo "==> nix attempt ${n}/${attempts}: {{ cmd }}"
      set +e
      {{ cmd }}
      status=$?
      set -e
      if [[ "${status}" -eq 0 ]]; then
        exit 0
      fi
      if [[ "${n}" -ge "${attempts}" ]]; then
        echo "==> nix FAILED after ${n} attempt(s) (exit ${status}): {{ cmd }}" >&2
        exit "${status}"
      fi
      echo "==> nix attempt ${n} failed (exit ${status}); retrying in ${backoff}s..." >&2
      sleep "${backoff}"
      backoff=$((backoff * 3))
      n=$((n + 1))
    done

# ---------------------------------------------------------------------------
# CI vs release vs local quality
#
# CI is for checks only — never a release package (`nix build .#grok-oss`).
# Shipping from CI would blur the trust boundary (supply chain). Optional
# packaging is for humans: `just build` / `just smoke` / `just install-nix`.
#
# GHA (see .github/workflows/ci.yml): quality job runs flake-meta, ci-prep,
# and `just test` (same chain as `just ci`, not the `just ci` entrypoint) —
# not a release build. GHA always sets CI_LOW_MEM=1 and CI_SYSTEM=x86_64-linux.
#
# `just check` / `just ci` = full local gate (same recipe chain as GHA quality).
#   Run before you push. No pre-commit hook required for this.
# `just test` = fmt, clippy (-D warnings), workspace nextest, doctests,
#   mem-guard (includes offline OpenRouter credential tests via nextest).
# `just test-extra` = local-only extras CI does not run (cross-target clippy,
#   nix_retry smoke).
#
# There is no `ci-quick` or `ci-host` recipe — use `check`/`ci` or `test`.
#
# Free GHA: CI_LOW_MEM=1 so cargo runs under cargo-mem-guard + mold (no pure
# nix monorepo release build — that OOMs on ~16GB runners). Same flag also
# enables store-only PATH scrub in cargo-ci (see recipe comment).
# ---------------------------------------------------------------------------

# Alias: same full gate as `ci` (preferred short name before push).
check: ci

# Full local gate — same recipe chain as GHA quality (flake + prep + all tests/lints).
ci: require_system
    just flake-meta
    just ci-prep
    just test
    @rm -f .ci-started
    @echo "CI passed"

# Store prep before cargo under CI_LOW_MEM (mem-guard + ci-tools + .ci-started).
# Permanent cargo failures must not re-enter the outer GHA bootstrap loop.
ci-prep: require_system mem-guard
    @echo "==> ci-prep: realize .#ci-tools (nix_retry)"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#ci-tools
    @touch .ci-started

# Prove the flake evaluates (cheap; fails fast on lock/input breakage).
flake-meta:
    @echo "==> flake-meta"
    just nix_retry nix flake metadata

# Optional pure-nix release package (NOT CI). For local packaging / install-nix.
build:
    @echo "==> build .#grok-oss{{ if low_mem == "1" { " (low-mem nix opts)" } else { "" } }} [not CI]"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#grok-oss

# Optional: binary exists and runs a version probe (depends on release build).
smoke: build
    @echo "==> smoke [not CI]"
    test -x ./result/bin/grok-oss
    ./result/bin/grok-oss --version

# Optional crane cargo-check of pager-bin (NOT CI quality; prefer just test).
cargo-check: require_system
    @echo "==> cargo-check [not CI quality]"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.cargoCheck"

# Optional flake-only re-run of openrouter credential tests (offline; not a
# separate GHA job). Prefer `just test` / `cargo test -p xai-grok-shell --test
# openrouter_credentials` for the normal path.
openrouter-tests: require_system
    @echo "==> openrouter-tests (optional flake check; covered by just test)"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.openrouter-credentials"

# Build cargo-mem-guard package + unit tests as flake check.
mem-guard: require_system
    @echo "==> build .#cargo-mem-guard{{ if low_mem == "1" { " (low-mem nix opts)" } else { "" } }}"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#cargo-mem-guard
    @echo "==> check .#cargo-mem-guard-tests"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.cargo-mem-guard-tests"

# Run a cargo (or other) command; under CI_LOW_MEM=1 wrap with cargo-mem-guard
# via devShells.ci (mold + pressure defaults). Cargo payloads are never
# nix_retry'd (permanent compile fails once).
#
# Env hygiene (always):
#   - unset NO_COLOR, CARGO_TERM_COLOR, OPENROUTER_API_KEY
#   - set harness secret disables + loopback proxy trust + runfiles dummy
#
# PATH hygiene (CI_LOW_MEM=1 only — GHA + local closest-CI repro):
#   nix develop .#ci prepends ci-tools/stdenv store bins but keeps host PATH
#   after. scripts/with-ci-hermetic-path.sh rebuilds PATH as a /nix/store
#   allowlist so optional desktop tools (pw-record, parec, arecord, …) cannot
#   flip unit tests. Not a recorder denylist. git is in ci-tools for the same
#   reason. Interactive `just dev` / bare cargo keep impure host PATH.
#   Escape hatch: GROK_CI_ALLOW_HOST_PATH=1.
#
# RULES_RUST_RUNFILES_WORKSPACE_NAME: --all-features enables xai-test-utils'
# optional `bazel`/`runfiles` dep (Bazel-only). That crate needs this env at
# compile time; set a dummy so cargo/host gates are not blocked.
[private]
cargo-ci +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    # shellcheck source=scripts/ensure-working-nix-path.sh
    source "{{ justfile_directory() }}/scripts/ensure-working-nix-path.sh"
    export RULES_RUST_RUNFILES_WORKSPACE_NAME="${RULES_RUST_RUNFILES_WORKSPACE_NAME:-grok-oss}"
    # Theme/color unit tests need distinct SGR slots. Host shells (and the
    # agent runtime) often export NO_COLOR=1, which quantizes every theme
    # color to Reset and collapses accent_skill vs text_primary checks.
    # Match CI (no NO_COLOR) so local `just test` is deterministic.
    #
    # Same for CARGO_TERM_COLOR=never: nextest binds --color to that env and
    # drops its progress UI / live status when color is forced off. Unset so
    # nextest/cargo use auto (TTY -> rich UI; pipe/CI -> plain).
    unset NO_COLOR
    unset CARGO_TERM_COLOR
    # Developer machines often export OPENROUTER_API_KEY and/or have Zed
    # OpenRouter keys in the OS keychain; unit tests assert NotAuthenticated
    # and that default catalog entries lack live credentials. Match CI.
    unset OPENROUTER_API_KEY
    export GROK_DISABLE_SHARED_HARNESS_SECRETS="${GROK_DISABLE_SHARED_HARNESS_SECRETS:-1}"
    # Skip OS Secret Service / keyring in tests (D-Bus can hang forever on
    # desktop sessions). File-backend only — same as headless CI intent for
    # GROK_CREDENTIALS_FORCE_FILE in credentials_store.
    export GROK_CREDENTIALS_FORCE_FILE="${GROK_CREDENTIALS_FORCE_FILE:-1}"
    # Idle-resume e2e tests bind a loopback axum mock as cli-chat-proxy.
    export GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY="${GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY:-1}"
    if [[ "${CI_LOW_MEM:-}" == "1" ]]; then
      # ci-tools + stdenv first (develop), then store-only PATH scrub, then mem-guard.
      exec nix develop {{ nix_low_mem_opts }} .#ci -c \
        ./scripts/with-ci-hermetic-path.sh \
        cargo-mem-guard -- {{ cmd }}
    fi
    exec {{ cmd }}

# Enter the fenix/crane-aligned dev shell (interactive: no retry wrapper).
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    # shellcheck source=scripts/ensure-working-nix-path.sh
    source "{{ justfile_directory() }}/scripts/ensure-working-nix-path.sh"
    exec nix develop

# Enter the free-GHA / low-mem host shell (interactive: no retry wrapper).
dev-ci:
    #!/usr/bin/env bash
    set -euo pipefail
    # shellcheck source=scripts/ensure-working-nix-path.sh
    source "{{ justfile_directory() }}/scripts/ensure-working-nix-path.sh"
    exec nix develop .#ci

# Quality gate (GHA `quality` job + local pre-push). No release build.
#
# Cargo host scope (not --all-features): Bazel-only features (default-bazel /
# runfiles) break plain cargo. Not --all-targets on clippy: unit/integration
# tests pull cross-crate `cfg(test)` seams that Bazel injects via default-bazel;
# those need per-crate test-support (partially wired). Clippy therefore lints
# production surfaces (--lib --bins). Unit/integration tests run via
# cargo-nextest (process-per-test isolation for globals like theme cache).
# Doctests stay on `cargo test --doc` (nextest does not run rustdoc tests).
#
# Covers: fmt check, clippy -D warnings (lib+bins), workspace nextest, doctests,
# cargo-mem-guard (workspace-excluded).
test: test-fmt test-clippy test-unit test-doc test-mem-guard
    @echo "just test passed"

# Local-only extras CI does not run.
test-extra: test-clippy-targets test-nix-retry-smoke
    @echo "just test-extra passed"

test-fmt:
    @echo "==> cargo fmt --all -- --check"
    just cargo-ci cargo fmt --all -- --check

test-clippy:
    @echo "==> cargo clippy --workspace --lib --bins (-D warnings)"
    just cargo-ci cargo clippy --workspace --lib --bins --locked -- -D warnings

# Process-per-test runner. Requires cargo-nextest on PATH (devShell / ci-tools /
# or `cargo install cargo-nextest`). Under CI_LOW_MEM, cargo-mem-guard wraps the
# whole `cargo nextest` invocation and caps compile jobs via CARGO_BUILD_JOBS.
test-unit:
    @echo "==> cargo nextest run --workspace"
    just cargo-ci cargo nextest run --workspace --locked

test-doc:
    @echo "==> cargo test --workspace --doc"
    just cargo-ci cargo test --workspace --doc --locked

# Standalone (Cargo.toml workspace exclude).
test-mem-guard:
    @echo "==> cargo test cargo-mem-guard (workspace-excluded)"
    just cargo-ci cargo test --manifest-path crates/codegen/cargo-mem-guard/Cargo.toml --locked

# Cross-target clippy (local / test-extra). Not on free GHA quality job.
# Override: EXTRA_CLIPPY_TARGETS="aarch64-unknown-linux-gnu ..."
test-clippy-targets:
    #!/usr/bin/env bash
    set -euo pipefail
    targets="${EXTRA_CLIPPY_TARGETS:-aarch64-unknown-linux-gnu}"
    host="$(rustc -vV | awk '/^host:/{print $2}')"
    for t in ${targets}; do
      if [[ "${t}" == "${host}" ]]; then
        echo "==> clippy target ${t}: skip (host, already in test-clippy)"
        continue
      fi
      echo "==> cargo clippy --target ${t} --workspace --lib --bins (-D warnings)"
      if [[ "${CI_LOW_MEM:-}" == "1" ]]; then
        nix develop {{ nix_low_mem_opts }} .#ci -c cargo-mem-guard -- \
          cargo clippy --workspace --lib --bins --locked --target "${t}" -- -D warnings
      else
        cargo clippy --workspace --lib --bins --locked --target "${t}" -- -D warnings
      fi
    done

# Smoke-test nix_retry: NIX_RETRY_ATTEMPTS=2 must fail after 2 attempts of
# `false` (proves banner + integer path). Also checks invalid attempts reject.
test-nix-retry-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=2
    set +e
    out="$(just nix_retry false 2>&1)"
    status=$?
    set -e
    if [[ "${status}" -eq 0 ]]; then
      echo "test-nix-retry-smoke: expected false to fail" >&2
      exit 1
    fi
    if ! grep -qE 'attempt 2/2|2 attempt' <<<"${out}"; then
      echo "test-nix-retry-smoke: expected attempt 2/2 (or '2 attempt') in output:" >&2
      echo "${out}" >&2
      exit 1
    fi
    # Integer validation path (fail-fast, no retries of false).
    set +e
    bad_out="$(NIX_RETRY_ATTEMPTS=nope just nix_retry true 2>&1)"
    bad_status=$?
    set -e
    if [[ "${bad_status}" -eq 0 ]]; then
      echo "test-nix-retry-smoke: expected invalid NIX_RETRY_ATTEMPTS to fail" >&2
      exit 1
    fi
    if ! grep -q 'NIX_RETRY_ATTEMPTS must be a positive integer' <<<"${bad_out}"; then
      echo "test-nix-retry-smoke: expected integer validation message:" >&2
      echo "${bad_out}" >&2
      exit 1
    fi
    echo "test-nix-retry-smoke: ok (false failed after 2 attempts, exit ${status}; invalid attempts rejected)"

# Install grok-oss -> ~/.cargo/bin (Cargo.toml [[bin]] name = "grok-oss").
# Overrides host -fuse-ld=wild (breaks this link). See comments in recipe body.
# Strips the installed artifact only: [profile.release] stays unstripped for
# local debugging; release-dist keeps strip=false for sidecar extract.
install:
    # Host ~/.cargo/config often sets -fuse-ld=wild; wild fails this workspace
    # (undefined drop_in_place<serde_json::Value>). CLI --config rustflags wins.
    mkdir -p "${CARGO_HOME:-$HOME/.cargo}/bin"
    @echo "==> cargo build --release -p xai-grok-pager-bin (no wild linker)"
    cargo build --release -p xai-grok-pager-bin --locked \
      --config 'target.x86_64-unknown-linux-gnu.rustflags=["-C","force-unwind-tables=yes"]' \
      --config 'target.aarch64-unknown-linux-gnu.rustflags=["-C","force-unwind-tables=yes"]'
    @echo "==> strip unneeded symbols (install artifact only)"
    strip --strip-unneeded target/release/grok-oss
    @echo "==> install -> ${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    install -Dm755 target/release/grok-oss "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    @echo "==> verify"
    "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" --version
    @file "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" | grep -q 'stripped' \
      || (echo "install: expected stripped binary" >&2; file "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" >&2; exit 1)

# Build release-dist binary, extract .debug sidecar, strip (not CI quality gate).
build-dist:
    # Host ~/.cargo/config often sets -fuse-ld=wild; wild fails this workspace.
    @echo "==> cargo build --profile release-dist -p xai-grok-pager-bin (no wild linker)"
    cargo build --profile release-dist -p xai-grok-pager-bin --locked \
      --config 'target.x86_64-unknown-linux-gnu.rustflags=["-C","force-unwind-tables=yes"]' \
      --config 'target.aarch64-unknown-linux-gnu.rustflags=["-C","force-unwind-tables=yes"]'
    @echo "==> extract debug sidecar + strip binary"
    ./scripts/extract-debug-sidecar.sh target/release-dist/grok-oss
    @echo "==> artifacts"
    ls -lh target/release-dist/grok-oss target/release-dist/grok-oss.debug
    file target/release-dist/grok-oss target/release-dist/grok-oss.debug

# Install release-dist binary + grok-oss.debug sidecar to cargo bin (not CI).
install-dist: build-dist
    mkdir -p "${CARGO_HOME:-$HOME/.cargo}/bin"
    @echo "==> install stripped binary + sidecar -> ${CARGO_HOME:-$HOME/.cargo}/bin/"
    install -Dm755 target/release-dist/grok-oss "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    install -Dm644 target/release-dist/grok-oss.debug "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss.debug"
    @echo "==> verify"
    "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" --version
    @file "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" | grep -q 'stripped' \
      || (echo "install-dist: expected stripped binary" >&2; file "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" >&2; exit 1)
    @test -f "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss.debug" \
      || (echo "install-dist: missing sidecar grok-oss.debug" >&2; exit 1)

# Install from Nix result (matches just build / CI; no host cargo linker).
# Strip after copy: nix fixup may already strip; --strip-unneeded is safe/idempotent.
install-nix: build
    mkdir -p "${CARGO_HOME:-$HOME/.cargo}/bin"
    install -Dm755 ./result/bin/grok-oss "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    strip --strip-unneeded "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" --version

# ---------------------------------------------------------------------------
# Limits-first path certainty (Slice E2)
#
# Never claim "limits-first path certain" without runnable proof.
# Hermetic suite = rank / bare resolve / memo + JSON path checker unit tests.
# Live recipe = rebuilt binary `limits --json` + same pure checker (ignored test).
#
# Spend order when auto_use_included_limits=true and preferred ≠ api_key:
#   1. included weekly used < 100% → SuperGrok session only (console omitted)
#   2. included full + SuperGrok dollar extras > 0 → SuperGrok primary, console failover
#   3. included full + extras 0/unknown → console primary
#
# Dogfood field template: .agents/joins/template-limits-dogfood-window.md
# ---------------------------------------------------------------------------

# Hermetic path suite: spend-order rank, bare resolve, memo, flat-poll, prepaid
# lag, Management ForceRefresh policy, and the pure limits --json C1/C3 checker.
# One command. Run before claiming limits-first path certain.
check-limits-first-path:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> check-limits-first-path (hermetic)"
    echo "==> xai-grok-shell: spend order + bare resolve + memo"
    just cargo-ci cargo test -p xai-grok-shell --lib --locked -- \
      auto_order_keeps_supergrok auto_with_included_headroom auto_after_included \
      auto_afterburner resolve_auto_after_included resolve_enforced_auto_use \
      resolve_api_key_pin sampling_config_auto_use resolve_model_override \
      afterburner_skips_allowance_mark apply_billing_100_pct_with_positive_extras
    echo "==> xai-grok-pager: flat_poll + prepaid lag + ForceRefresh policy + path checker"
    # Filters: flat_poll / prepaid_lag match honesty tests; management_meter_cache
    # + should_clear_management cover ForceRefresh; check_limits_first is E2 SoT.
    just cargo-ci cargo test -p xai-grok-pager --lib --locked -- \
      flat_poll prepaid_lag \
      management_meter_cache_policy should_clear_management_meter \
      check_limits_first
    echo "check-limits-first-path passed (hermetic)"

# Live C1/C3 after rebuild: SuperGrok primary while included weekly used < 100%,
# console not live. Requires auto_use_included_limits=true and preferred ≠ api_key
# on the home under test. Uses the same pure checker as unit tests.
#
# Env:
#   GROK_OSS_BIN — binary (default: ./target/release/grok-oss, else grok-oss on PATH)
#   LIMITS_FIRST_AUTO_USE — default 1
#   LIMITS_FIRST_PREFERRED_API_KEY — default 0 (set 1 only when preferred=api_key)
#   LIMITS_JSON_TIMEOUT_SECS — default 90
check-limits-first-live:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    bin="${GROK_OSS_BIN:-}"
    if [[ -z "${bin}" ]]; then
      if [[ -x "${root}/target/release/grok-oss" ]]; then
        bin="${root}/target/release/grok-oss"
      elif command -v grok-oss >/dev/null 2>&1; then
        bin="$(command -v grok-oss)"
      else
        echo "check-limits-first-live: no grok-oss binary (build release or set GROK_OSS_BIN)" >&2
        exit 2
      fi
    fi
    timeout_secs="${LIMITS_JSON_TIMEOUT_SECS:-90}"
    tmp="$(mktemp "${TMPDIR:-/tmp}/limits-first-XXXXXX.json")"
    cleanup() { rm -f "${tmp}"; }
    trap cleanup EXIT
    echo "==> check-limits-first-live: ${bin} limits --json (timeout ${timeout_secs}s)"
    set +e
    timeout "${timeout_secs}" "${bin}" limits --json >"${tmp}" 2>/tmp/limits-first-live.err
    status=$?
    set -e
    if [[ "${status}" -ne 0 ]]; then
      echo "check-limits-first-live: limits --json failed (exit ${status})" >&2
      cat /tmp/limits-first-live.err >&2 || true
      exit "${status}"
    fi
    echo "==> liveSampling / console.isLive / includedUsedPct (preview):"
    if command -v jq >/dev/null 2>&1; then
      jq '{liveSampling, livePrincipalRole, consoleIsLive: .console.isLive, keyAvailable: .console.keyAvailable, principals: [.supergrok.principals[]? | {label, includedUsedPct, dollarExtrasUsd}]}' "${tmp}" || cat "${tmp}"
    else
      head -c 2000 "${tmp}"; echo
    fi
    export LIMITS_FIRST_JSON="${tmp}"
    export LIMITS_FIRST_AUTO_USE="${LIMITS_FIRST_AUTO_USE:-1}"
    export LIMITS_FIRST_PREFERRED_API_KEY="${LIMITS_FIRST_PREFERRED_API_KEY:-0}"
    echo "==> pure checker (live_check_limits_first_from_env_json)"
    just cargo-ci cargo test -p xai-grok-pager --lib --locked \
      live_check_limits_first_from_env_json -- --ignored --nocapture
    echo "check-limits-first-live passed"

# Upstream monorepo export helpers (see docs/upstream-history.md).
upstream-detect:
    ./scripts/detect-upstream-export.sh

upstream-import *ARGS:
    ./scripts/import-upstream-export.sh {{ ARGS }}

# Cherry-pick Surmount product onto current xAI tip → onto-xai/<short>
upstream-put-history *ARGS:
    ./scripts/put-history-on-xai.sh {{ ARGS }}

# Join Surmount main into current onto tip (-s ours; stages merge for signed commit)
upstream-join-main *ARGS:
    ./scripts/join-main-into-onto.sh {{ ARGS }}

# Fail if AGENTS/FORK/RESIDUAL/join script/… missing after recon
upstream-assert-process-pins *ARGS:
    ./scripts/assert-process-pins.sh {{ ARGS }}

# Read-only recon probe: branch, CHERRY_PICK/MERGE, UU count, onto-ish, next human action
recon-status:
    ./scripts/recon-status.sh

upstream-sync *ARGS:
    ./scripts/sync-upstream.sh {{ ARGS }}
