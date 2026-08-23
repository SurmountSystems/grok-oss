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
# Optional on this Linux host: `just check-remote` realizes flake metadata
# and the workspace cargo quality derivation (the same gate as `just check` /
# `just test`: fmt, clippy, workspace nextest, doctests, cargo-mem-guard) on
# the existing trusted-user remote builder (default $HOME/.config/nix/machines).
# Named filters: `just test-remote` / `just cargo-remote` realize
# .#workspace-cargo-named-test the same way (force-remote nix, not host
# rustc). rustc must not run on the caller. Those rustc jobs require
# surmount-remote (plus big-parallel). This laptop never auto-detects
# surmount-remote; the ssh-ng machines line must advertise it. --option
# system-features that omit big-parallel does not stop local nixbld: the
# daemon still advertises big-parallel. Tiny crane vendor unpacks that
# prefer a local build may run here. Force-remote nix passes --cores 64 so
# one workspace rustc can use the builder's cores. The host machines file
# max-jobs should match that width. Force-remote exports NIX_SSHOPTS (this
# account's known_hosts; host-key checks stay on) and copies that host key
# onto the builders line for nix-daemon SSH. Default `just check` /
# `just ci` stay local. GitHub Actions must not use check-remote,
# test-remote, or cargo-remote.

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
# Unclassified non-zero exits retry (GHA flake-input 502/503, downloads).
# Hard remote-assign misses (failed to start SSH connection, Failed to
# find a machine for remote build, or missing system features after the
# client scheduled the drv) exit on attempt 1 with no sleep. Cargo
# payloads stay outside nix_retry so a permanent compile fails once.
# Local fail-fast: NIX_RETRY_ATTEMPTS=1 just mem-guard
# Override attempts: NIX_RETRY_ATTEMPTS=5 just mem-guard
#
# Security: nix_retry execs the +cmd words as argv ("$@"), then appends
# force-remote flags. Never pass untrusted user input as those words.
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

# Fail loud before `just check-remote` starts Nix or cargo. Reuses the
# trusted-user builders file already named in the user Nix config. Does not
# bake a host address. Does not fall back to local Nix store builds.
# User SSH to Host surmount-1 is not the nix build path: require this
# account's known_hosts entry for the machines-file builder, and export
# NIX_SSHOPTS (UserKnownHostsFile). nix_retry also copies that host key
# into the builders line so nix-daemon SSH can verify it.
# After SSH BatchMode works, query the remote daemon system-features
# (Host alias, stderr discarded). If that list omits surmount-remote,
# exit 2 before the long quality build. Tests inject
# GROK_NIX_REMOTE_SYSTEM_FEATURES and skip live SSH.
[private]
require_remote_builder:
    #!/usr/bin/env bash
    set -euo pipefail
    file="${GROK_NIX_BUILDERS_FILE:-$HOME/.config/nix/machines}"
    known_hosts="${GROK_NIX_KNOWN_HOSTS:-$HOME/.ssh/known_hosts}"
    extra_ssh="-o UserKnownHostsFile=${known_hosts} -o StrictHostKeyChecking=yes"
    if [[ -n "${NIX_SSHOPTS:-}" ]]; then
      export NIX_SSHOPTS="${NIX_SSHOPTS} ${extra_ssh}"
    else
      export NIX_SSHOPTS="${extra_ssh}"
    fi
    if [[ ! -s "${file}" ]]; then
      echo "The Nix builders file is missing or empty: ${file}." >&2
      echo "just check-remote reuses the trusted-user machines file already named in the user Nix config (override with GROK_NIX_BUILDERS_FILE)." >&2
      echo "Default just check stays local and does not need this file." >&2
      exit 2
    fi
    if ! grep -q 'ssh-ng://' "${file}"; then
      echo "The Nix builders file ${file} has no ssh-ng:// builder line." >&2
      echo "just check-remote will not fall back to local Nix store builds." >&2
      exit 2
    fi
    ssh_ng_host() {
      local u="${1#ssh-ng://}"
      u="${u%%\?*}"
      u="${u#*@}"
      u="${u%%/*}"
      if [[ "${u}" == \[* ]]; then
        u="${u#\[}"
        u="${u%%]*}"
      else
        u="${u%%:*}"
      fi
      printf '%s' "${u}"
    }
    host_key_present() {
      local host="$1"
      [[ -s "${known_hosts}" ]] || return 1
      ssh-keygen -F "${host}" -f "${known_hosts}" 2>/dev/null | awk '!/^#/ && $2 ~ /^ssh-/ { found=1; exit } END { exit !found }'
    }
    while IFS= read -r line || [[ -n "${line}" ]]; do
      [[ "${line}" == ssh-ng://* ]] || continue
      set -- ${line}
      host="$(ssh_ng_host "${1}")"
      if [[ -z "${host}" ]] || ! host_key_present "${host}"; then
        echo "This account's known_hosts has no host key for the machines-file builder." >&2
        echo "User ssh to Host surmount-1 is not the nix build SSH path (nix-daemon opens ssh-ng)." >&2
        echo "just check-remote sets NIX_SSHOPTS to this account's known_hosts and will not fall back to a local rustc." >&2
        exit 2
      fi
    done < "${file}"
    inject_feats="${GROK_NIX_REMOTE_SYSTEM_FEATURES-}"
    if [[ -z "${inject_feats}" ]]; then
      if ! ssh -o BatchMode=yes -o ConnectTimeout=8 -o StrictHostKeyChecking=yes surmount-1 true; then
        echo "SSH BatchMode to Host surmount-1 failed." >&2
        echo "just check-remote requires that existing remote builder and will not fall back to local Nix store builds." >&2
        exit 2
      fi
    fi
    remote_feats=""
    if [[ -n "${inject_feats}" ]]; then
      remote_feats="${inject_feats}"
    else
      set +e
      feats_out="$(ssh -o BatchMode=yes -o ConnectTimeout=8 -o StrictHostKeyChecking=yes surmount-1 'nix config show' 2>/dev/null)"
      feats_status=$?
      set -e
      if [[ "${feats_status}" -ne 0 ]]; then
        echo "Could not read the remote builder nix-daemon system-features over SSH BatchMode." >&2
        echo "just check-remote will not start the long quality build until that query works." >&2
        exit 2
      fi
      remote_feats="$(awk -F' = ' '/^system-features / { print $2; exit }' <<<"${feats_out}")"
      if [[ -z "${remote_feats}" ]]; then
        echo "The remote builder SSH reply had no system-features line." >&2
        echo "just check-remote will not start the long quality build until the remote nix-daemon reports its feature list." >&2
        exit 2
      fi
    fi
    if ! grep -Eq '(^|[[:space:],{])surmount-remote($|[[:space:],}])' <<<"${remote_feats}"; then
      echo "The remote nix-daemon does not list surmount-remote in its system-features." >&2
      echo "The client machines file advertises that feature, so Nix will schedule rustc on the remote, then the daemon will refuse: missing system features." >&2
      echo "Add surmount-remote to the builder daemon (NixOS extra-system-features / nix.conf) and restart or switch. just check-remote will not start the long quality build until that feature is present." >&2
      exit 2
    fi
    echo "==> just check-remote: using builders file ${file}"
    echo "==> just check-remote: NIX_SSHOPTS uses this account's known_hosts (host-key checks stay on)"
    echo "==> just check-remote: rustc, clippy, and nextest require the remote builder surmount-remote feature (fallback=false). This laptop does not advertise that feature, so local nixbld cannot take the rustc job. Tiny vendor unpacks that prefer a local build may run here."
    echo "==> just check-remote: force-remote nix uses --cores 64. Host machines max-jobs should advertise that many jobs on the builder."

# Retry a nix (or other) command. Integer-validates NIX_RETRY_ATTEMPTS (default 4).
# Prints a clear banner per attempt. Unclassified failures retry. Hard SSH /
# no-remote-machine / missing-system-features / rustfmt Diff-in misses exit on
# attempt 1 (no 5s/15s/45s sleep). Use only
# around store realization / flake eval, never around host cargo compile
# payloads.
#
# Before the first attempt: ensure a working `nix` is first on PATH so a
# broken host binary does not burn the full retry budget. Override: NIX_BIN.
[private]
[positional-arguments]
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
    attempt_log="$(mktemp)"
    enriched_builders=""
    cleanup_nix_retry_log() { rm -f "${attempt_log}" "${enriched_builders}"; }
    trap cleanup_nix_retry_log EXIT
    force_remote_opts=()
    if [[ "${GROK_NIX_FORCE_REMOTE:-}" == "1" ]]; then
      builders_file="${GROK_NIX_BUILDERS_FILE:-$HOME/.config/nix/machines}"
      known_hosts="${GROK_NIX_KNOWN_HOSTS:-$HOME/.ssh/known_hosts}"
      extra_ssh="-o UserKnownHostsFile=${known_hosts} -o StrictHostKeyChecking=yes"
      if [[ -n "${NIX_SSHOPTS:-}" ]]; then
        export NIX_SSHOPTS="${NIX_SSHOPTS} ${extra_ssh}"
      else
        export NIX_SSHOPTS="${extra_ssh}"
      fi
      ssh_ng_host() {
        local u="${1#ssh-ng://}"
        u="${u%%\?*}"
        u="${u#*@}"
        u="${u%%/*}"
        if [[ "${u}" == \[* ]]; then
          u="${u#\[}"
          u="${u%%]*}"
        else
          u="${u%%:*}"
        fi
        printf '%s' "${u}"
      }
      host_key_b64() {
        local host="$1"
        local line typ key
        [[ -s "${known_hosts}" ]] || return 1
        line="$(ssh-keygen -F "${host}" -f "${known_hosts}" 2>/dev/null | awk '!/^#/ && $2=="ssh-ed25519" {print; exit}')"
        if [[ -z "${line}" ]]; then
          line="$(ssh-keygen -F "${host}" -f "${known_hosts}" 2>/dev/null | awk '!/^#/ && $2 ~ /^ssh-/ {print; exit}')"
        fi
        [[ -n "${line}" ]] || return 1
        typ="$(awk '{print $2}' <<<"${line}")"
        key="$(awk '{print $3}' <<<"${line}")"
        printf '%s' "${typ} ${key}" | base64 -w0
      }
      # max-connections is ssh-ng concurrent daemon connections (copy slots).
      # Default Nix 1 is serial NAR copy. Open hang report:
      # https://github.com/NixOS/nix/issues/14615 (accessed: 2026-08-18).
      max_conn="${GROK_NIX_SSH_NG_MAX_CONNECTIONS:-8}"
      if [[ ! "${max_conn}" =~ ^[1-9][0-9]*$ ]]; then
        echo "==> nix_retry: GROK_NIX_SSH_NG_MAX_CONNECTIONS must be a positive integer, got: ${max_conn}" >&2
        exit 2
      fi
      enriched_builders="$(mktemp)"
      chmod 600 "${enriched_builders}"
      while IFS= read -r line || [[ -n "${line}" ]]; do
        if [[ "${line}" != ssh-ng://* ]]; then
          printf '%s\n' "${line}" >>"${enriched_builders}"
          continue
        fi
        # Parse fields with read. Do not `set --` the machines line: that
        # replaces the nix command in "$@" and makes argv0 ssh-ng:// (exit 127).
        uri="" systems="" ssh_key="" max_jobs="" speed="" supported="" mandatory="" host_key=""
        read -r uri systems ssh_key max_jobs speed supported mandatory host_key _rest <<<"${line}" || true
        if [[ "${uri}" != *"max-connections="* ]]; then
          if [[ "${uri}" == *\?* ]]; then
            uri="${uri}&max-connections=${max_conn}"
          else
            uri="${uri}?max-connections=${max_conn}"
          fi
        fi
        if [[ -n "${host_key:-}" && "${host_key}" != "-" ]]; then
          printf '%s %s %s %s %s %s %s %s\n' \
            "${uri}" "${systems:--}" "${ssh_key:--}" "${max_jobs:--}" "${speed:--}" "${supported:--}" "${mandatory:--}" "${host_key}" >>"${enriched_builders}"
          continue
        fi
        host="$(ssh_ng_host "${uri}")"
        if ! b64="$(host_key_b64 "${host}")"; then
          echo "==> nix_retry: this account's known_hosts has no host key for the machines-file builder. User ssh to Host surmount-1 is not the nix build SSH path." >&2
          exit 2
        fi
        printf '%s %s %s %s %s %s %s %s\n' \
          "${uri}" "${systems:--}" "${ssh_key:--}" "${max_jobs:--}" "${speed:--}" "${supported:--}" "${mandatory:--}" "${b64}" >>"${enriched_builders}"
      done < "${builders_file}"
      builders_file="${enriched_builders}"
      force_remote_opts=(
        --option builders "@${builders_file}"
        --option builders-use-substitutes true
        --option fallback false
        --option system-features "kvm nixos-test uid-range"
        --cores 64
      )
    fi
    if [[ "${1:-}" == ssh-ng://* ]]; then
      echo "==> nix_retry: the first argument is a machines-file line, not the nix command. Pass --option builders @file after the command; do not put the machines line in \"\$@\"." >&2
      exit 2
    fi
    while true; do
      if ((${#force_remote_opts[@]})); then
        echo "==> nix attempt ${n}/${attempts}: $* ${force_remote_opts[*]}"
      else
        echo "==> nix attempt ${n}/${attempts}: $*"
      fi
      set +e
      set +o pipefail
      "$@" "${force_remote_opts[@]}" 2>&1 | tee "${attempt_log}"
      status="${PIPESTATUS[0]}"
      set -o pipefail
      set -e
      if [[ "${status}" -eq 0 ]]; then
        exit 0
      fi
      if grep -qE 'failed to start SSH connection|Failed to find a machine for remote build' "${attempt_log}"; then
        echo "==> nix_retry: the builder is listed, but SSH did not start. rustc was not run locally. Not retrying this hard remote miss." >&2
        exit "${status}"
      fi
      if grep -qE 'missing system features' "${attempt_log}"; then
        echo "==> nix_retry: the remote builder refused this derivation: missing system features. The client scheduled it because the machines file advertises surmount-remote. The remote nix-daemon does not list that feature in its system-features. Add surmount-remote to the builder daemon (NixOS extra-system-features / nix.conf) and restart or switch, then retry. Not retrying this hard remote miss." >&2
        exit "${status}"
      fi
      if grep -qE 'Diff in ' "${attempt_log}"; then
        echo "==> nix_retry: cargo fmt / rustfmt check failed (Diff in). That is a quality fail, not a flake 502/503. Format the listed files and retry. Not retrying this hard quality miss." >&2
        exit "${status}"
      fi
      if grep -qE 'error: could not compile|clippy::' "${attempt_log}"; then
        echo "==> nix_retry: cargo clippy / rustc quality failed (could not compile). That is a quality fail, not a flake 502/503. Fix the listed errors and retry. Not retrying this hard quality miss." >&2
        exit "${status}"
      fi
      if [[ "${status}" -eq 127 ]] && grep -qE 'ssh-ng://.*No such file or directory' "${attempt_log}"; then
        echo "==> nix_retry: the command was a machines-file line (exit 127). Force-remote builders belong in --option builders @file after nix. Not retrying this hard recipe miss." >&2
        exit "${status}"
      fi
      if [[ "${n}" -ge "${attempts}" ]]; then
        echo "==> nix FAILED after ${n} attempt(s) (exit ${status}): $*" >&2
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
# Optional `check-remote` sends that same full cargo gate (fmt, clippy,
# nextest, doctests, cargo-mem-guard) to the remote builder
# (surmount-remote). Named `just test-remote` / `just cargo-remote` send
# a filter (cargo test / nextest / clippy / build / check) the same way.
# Vendor unpacks may stay on this machine. Default `just check` /
# `just ci` stay local.
#
# Free GHA: CI_LOW_MEM=1 so cargo runs under cargo-mem-guard + mold (no pure
# nix monorepo release build — that OOMs on ~16GB runners). Same flag also
# enables store-only PATH scrub in cargo-ci (see recipe comment).
# ---------------------------------------------------------------------------

# Alias: same full gate as `ci` (preferred short name before push).
check: ci

# Optional remote gate: flake metadata plus the same workspace cargo gate as
# `just check` (fmt, clippy, nextest run, doctests, cargo-mem-guard) as a
# Nix derivation. rustc requires the remote builder's surmount-remote
# feature. Default `just check` stays local.
check-remote: require_system require_remote_builder
    #!/usr/bin/env bash
    set -euo pipefail
    export GROK_NIX_FORCE_REMOTE=1
    export GROK_NIX_BUILDERS_FILE="${GROK_NIX_BUILDERS_FILE:-$HOME/.config/nix/machines}"
    known_hosts="${GROK_NIX_KNOWN_HOSTS:-$HOME/.ssh/known_hosts}"
    extra_ssh="-o UserKnownHostsFile=${known_hosts} -o StrictHostKeyChecking=yes"
    if [[ -n "${NIX_SSHOPTS:-}" ]]; then
      export NIX_SSHOPTS="${NIX_SSHOPTS} ${extra_ssh}"
    else
      export NIX_SSHOPTS="${extra_ssh}"
    fi
    echo "==> just check-remote: flake metadata"
    just flake-meta
    echo "==> just check-remote: workspace cargo quality as a remote Nix derivation"
    just nix_retry nix build -L ".#workspace-cargo-quality"

# Named cargo on the same remote builder as check-remote (surmount-remote).
# Kind is test, nextest, clippy, build, or check. Remaining words are the
# cargo/nextest argv after that subcommand. rustc does not run on this
# laptop. GitHub Actions must not call this recipe.
#
#   just cargo-remote test -p xai-grok-pager --lib -- actions::defaults
#   just cargo-remote clippy -p xai-grok-pager --all-targets -- -D warnings
#   just cargo-remote build -p xai-grok-pager
[positional-arguments]
cargo-remote *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    just remote_named_cargo "$@"

# Named cargo test on the remote builder. Same path as cargo-remote test.
# Example: just test-remote -p xai-grok-pager --lib -- actions::defaults
# That is cargo test --locked (tests execute; not compile-only). Full gate:
# just check-remote.
[positional-arguments]
test-remote *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    just remote_named_cargo test "$@"

# Shared body for test-remote / cargo-remote. Check argv before
# require_remote_builder so a missing filter does not SSH. Encode filters
# as base64 NUL-separated env for flake builtins.getEnv (nix build --impure).
# Do not put filter words on the nix_retry argv.
[private]
[positional-arguments]
remote_named_cargo *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ $# -lt 1 ]]; then
      echo "just cargo-remote needs a kind (test, nextest, clippy, build, or check) and a filter." >&2
      echo "Example: just test-remote -p xai-grok-pager --lib -- actions::defaults" >&2
      echo "That runs cargo test on the remote builder. It does not run rustc on this laptop." >&2
      echo "Full gate: just check-remote." >&2
      exit 2
    fi
    kind="$1"
    shift
    case "${kind}" in
      test|nextest|clippy|build|check) ;;
      *)
        echo "just cargo-remote kind must be test, nextest, clippy, build, or check, got: ${kind}" >&2
        exit 2
        ;;
    esac
    if [[ $# -lt 1 ]]; then
      echo "just cargo-remote ${kind} needs a filter (for example -p xai-grok-pager --lib -- actions::defaults)." >&2
      echo "Refusing to run the whole workspace on the builder from an empty filter. Full gate: just check-remote." >&2
      exit 2
    fi
    if [[ "${kind}" == "test" || "${kind}" == "nextest" ]]; then
      for a in "$@"; do
        if [[ "${a}" == "--no-run" ]]; then
          echo "just test-remote / just cargo-remote ${kind} runs the tests on the remote builder. Do not pass --no-run (that is compile-only)." >&2
          exit 2
        fi
      done
    fi
    just require_remote_builder
    export GROK_NIX_FORCE_REMOTE=1
    export GROK_NIX_BUILDERS_FILE="${GROK_NIX_BUILDERS_FILE:-$HOME/.config/nix/machines}"
    known_hosts="${GROK_NIX_KNOWN_HOSTS:-$HOME/.ssh/known_hosts}"
    extra_ssh="-o UserKnownHostsFile=${known_hosts} -o StrictHostKeyChecking=yes"
    if [[ -n "${NIX_SSHOPTS:-}" ]]; then
      export NIX_SSHOPTS="${NIX_SSHOPTS} ${extra_ssh}"
    else
      export NIX_SSHOPTS="${extra_ssh}"
    fi
    export GROK_REMOTE_CARGO_KIND="${kind}"
    export GROK_REMOTE_TEST_ARGS="$(printf '%s\0' "$@" | base64 -w0)"
    echo "==> just cargo-remote ${kind}: named cargo ${kind} as a remote Nix derivation (nix build --impure \".#workspace-cargo-named-test\")"
    echo "==> rustc requires surmount-remote. This laptop does not run that rustc."
    just nix_retry nix build --impure -L ".#workspace-cargo-named-test"

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
      force_remote_opts=()
      if [[ "${GROK_NIX_FORCE_REMOTE:-}" == "1" ]]; then
        builders_file="${GROK_NIX_BUILDERS_FILE:-$HOME/.config/nix/machines}"
        force_remote_opts=(
          --option builders "@${builders_file}"
          --option builders-use-substitutes true
          --option fallback false
          --option system-features "kvm nixos-test uid-range"
        )
      fi
      exec nix develop {{ nix_low_mem_opts }} "${force_remote_opts[@]}" .#ci -c \
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
# runfiles) break plain cargo. Clippy uses --all-targets so it lints lib, bins,
# tests, examples, benches, and `#[cfg(test)]` (cargo compile units, not extra
# rustc targets such as aarch64). Unit/integration tests still run via
# cargo-nextest (process-per-test isolation for globals like theme cache).
# Doctests stay on `cargo test --doc` (nextest does not run rustdoc tests).
#
# Covers: fmt check, clippy -D warnings (--all-targets), workspace nextest, doctests,
# cargo-mem-guard (workspace-excluded).
test: test-fmt test-clippy test-unit test-doc test-mem-guard
    @echo "just test passed"

# Local-only extras CI does not run.
test-extra: test-clippy-targets test-clippy-all-targets test-nix-retry-smoke test-nix-retry-hard-remote-miss-fail-fast test-nix-retry-missing-system-features-fail-fast test-nix-retry-rustfmt-diff-fail-fast test-nix-retry-clippy-compile-fail-fast test-nix-retry-force-remote-argv-is-nix test-nix-retry-force-remote-ssh-ng-max-connections test-check-remote-builders-file-smoke test-check-remote-cargo-is-remote-nix-derivation test-check-remote-quotes-quality-attr test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero test-check-remote-uses-builder-cores test-check-remote-clippy-uses-many-workers test-check-remote-deps-omit-git-sha test-check-remote-omits-local-big-parallel test-check-remote-workspace-rustc-not-local-eligible test-check-remote-exports-nix-sshopts test-check-remote-preflight-same-path-as-nix-ssh test-check-remote-preflight-remote-daemon-features test-test-remote-is-force-remote-nix test-test-remote-runs-tests-not-no-run test-test-remote-workspace-rustc-not-local-eligible test-test-remote-requires-filter
    @echo "just test-extra passed"

test-fmt:
    @echo "==> cargo fmt --all -- --check"
    just cargo-ci cargo fmt --all -- --check

test-clippy:
    @echo "==> cargo clippy --workspace --all-targets (-D warnings)"
    just cargo-ci cargo clippy --workspace --all-targets --locked -- -D warnings

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

# Clippy must lint every cargo compile unit (`--all-targets`: lib, bins,
# tests, examples, benches, `#[cfg(test)]`), not only `--lib --bins`.
# That flag is cargo units, not extra rustc targets (aarch64 stays
# `test-clippy-targets`). Instantiates just/flake text only. Does not
# realize rustc. GHA must not call check-remote; check-remote stays the
# backup gate.
test-clippy-all-targets:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    flake="${root}/flake.nix"
    gha="${root}/.github/workflows/ci.yml"
    recipe_body() {
      local name="$1"
      awk -v name="${name}" '
        $0 ~ ("^" name ":") { p=1; next }
        p && /^[a-zA-Z0-9_.-]+[ \t]*:/ { exit }
        p { print }
      ' "${justfile}"
    }
    clippy_body="$(recipe_body test-clippy)"
    targets_body="$(recipe_body test-clippy-targets)"
    if [[ -z "${clippy_body}" ]]; then
      echo "test-clippy-all-targets: expected just test-clippy" >&2
      exit 1
    fi
    if ! grep -qE -- 'cargo clippy --workspace --all-targets --locked -- -D warnings' <<<"${clippy_body}"; then
      echo "test-clippy-all-targets: just test-clippy must be cargo clippy --workspace --all-targets --locked -- -D warnings (lint tests, not only lib+bins):" >&2
      echo "${clippy_body}" >&2
      exit 1
    fi
    if grep -qE -- '--lib --bins' <<<"${clippy_body}" && ! grep -q -- '--all-targets' <<<"${clippy_body}"; then
      echo "test-clippy-all-targets: just test-clippy still has exclusive --lib --bins without --all-targets:" >&2
      echo "${clippy_body}" >&2
      exit 1
    fi
    if ! grep -q -- '--all-targets' <<<"${targets_body}"; then
      echo "test-clippy-all-targets: just test-clippy-targets must pass --all-targets (cargo compile units, not extra CPUs):" >&2
      echo "${targets_body}" >&2
      exit 1
    fi
    if grep -qE -- '--lib --bins' <<<"${targets_body}" && ! grep -q -- '--all-targets' <<<"${targets_body}"; then
      echo "test-clippy-all-targets: just test-clippy-targets still has exclusive --lib --bins without --all-targets:" >&2
      echo "${targets_body}" >&2
      exit 1
    fi
    quality="$(awk '
      $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /openrouter-credentials/ { exit }
      p { print }
    ' "${flake}")"
    named="$(awk '
      $0 ~ /workspace-cargo-named-test = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /ciLowMemEnv/ { exit }
      p { print }
    ' "${flake}")"
    if ! grep -q 'clippy-driver' <<<"${quality}"; then
      echo "test-clippy-all-targets: workspace-cargo-quality must still lint with clippy-driver:" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -qE -- 'check --profile "\$CARGO_PROFILE" --jobs "\$CARGO_BUILD_JOBS" --workspace --all-targets --locked' <<<"${quality}"; then
      echo "test-clippy-all-targets: quality clippy cargo check must be --workspace --all-targets (tests wrapped by clippy-driver):" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if grep -qE -- '--workspace --lib --bins' <<<"${quality}" && ! grep -q -- '--all-targets' <<<"${quality}"; then
      echo "test-clippy-all-targets: quality still has exclusive --lib --bins without --all-targets:" >&2
      echo "${quality}" >&2
      exit 1
    fi
    clippy_kind="$(awk '
      $0 ~ /clippy\)/ { p=1 }
      p && $0 ~ /build\)/ { exit }
      p { print }
    ' <<<"${named}")"
    if ! grep -q -- '--all-targets' <<<"${clippy_kind}"; then
      echo "test-clippy-all-targets: named-test clippy kind must pass --all-targets on cargo check:" >&2
      echo "${clippy_kind}" >&2
      exit 1
    fi
    if grep -qE -- '--lib --bins' <<<"${clippy_kind}" && ! grep -q -- '--all-targets' <<<"${clippy_kind}"; then
      echo "test-clippy-all-targets: named-test clippy kind still has exclusive --lib --bins without --all-targets:" >&2
      echo "${clippy_kind}" >&2
      exit 1
    fi
    if grep -q 'check-remote' "${gha}"; then
      echo "test-clippy-all-targets: GitHub Actions must not call check-remote" >&2
      exit 1
    fi
    if ! grep -qE '^check-remote:' "${justfile}"; then
      echo "test-clippy-all-targets: just check-remote must stay as the backup gate" >&2
      exit 1
    fi
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    phase="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.buildPhase")"
    if ! grep -q -- '--all-targets' <<<"${phase}"; then
      echo "test-clippy-all-targets: instantiated quality buildPhase must cargo check --all-targets:" >&2
      echo "${phase}" >&2
      exit 1
    fi
    if grep -qE -- '--workspace --lib --bins' <<<"${phase}" && ! grep -q -- '--all-targets' <<<"${phase}"; then
      echo "test-clippy-all-targets: instantiated quality still has exclusive --lib --bins without --all-targets:" >&2
      echo "${phase}" >&2
      exit 1
    fi
    echo "test-clippy-all-targets: ok (just/flake clippy --all-targets; GHA does not call check-remote; check-remote stays)"

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
      echo "==> cargo clippy --target ${t} --workspace --all-targets (-D warnings)"
      if [[ "${CI_LOW_MEM:-}" == "1" ]]; then
        nix develop {{ nix_low_mem_opts }} .#ci -c cargo-mem-guard -- \
          cargo clippy --workspace --all-targets --locked --target "${t}" -- -D warnings
      else
        cargo clippy --workspace --all-targets --locked --target "${t}" -- -D warnings
      fi
    done

# Smoke-test check-remote fail-loud: a missing builders file must exit 2
# before any Nix or cargo work. Not on default `just test`.
test-check-remote-builders-file-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    missing="/tmp/does-not-exist-builders-$$"
    rm -f "${missing}"
    set +e
    out="$(GROK_NIX_BUILDERS_FILE="${missing}" just check-remote 2>&1)"
    status=$?
    set -e
    if [[ "${status}" -ne 2 ]]; then
      echo "test-check-remote-builders-file-smoke: expected exit 2, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'missing or empty' <<<"${out}"; then
      echo "test-check-remote-builders-file-smoke: expected a missing-or-empty builders file sentence:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-check-remote-builders-file-smoke: ok (missing builders file exited ${status})"

# Prove `just check-remote` sends the same cargo gate as `just check` /
# `just ci` (`just test`: fmt, clippy, workspace nextest, doctests,
# cargo-mem-guard) through a remote Nix derivation (builders file via
# nix_retry, rustc requires big-parallel). Tests must actually run, not
# compile-only `--no-run`. Does not realize that derivation. Default
# `just ci` must still be local host cargo. GHA must not call check-remote.
test-check-remote-cargo-is-remote-nix-derivation:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    flake="${root}/flake.nix"
    gha="${root}/.github/workflows/ci.yml"
    recipe_body() {
      local name="$1"
      awk -v name="${name}" '
        $0 ~ ("^" name ":") { p=1; next }
        p && /^[a-zA-Z0-9_.-]+[ \t]*:/ { exit }
        p { print }
      ' "${justfile}"
    }
    remote_body="$(recipe_body check-remote)"
    if grep -qE '^[[:space:]]*just[[:space:]]+(ci|test|cargo-ci)([[:space:]]|$)' <<<"${remote_body}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: check-remote must not run host just ci/test/cargo-ci:" >&2
      echo "${remote_body}" >&2
      exit 1
    fi
    if ! grep -q 'GROK_NIX_FORCE_REMOTE=1' <<<"${remote_body}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: check-remote must set GROK_NIX_FORCE_REMOTE=1 so nix_retry uses the builders file:" >&2
      echo "${remote_body}" >&2
      exit 1
    fi
    if ! grep -qE 'nix build .*workspace-cargo-quality' <<<"${remote_body}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: check-remote must nix build .#workspace-cargo-quality:" >&2
      echo "${remote_body}" >&2
      exit 1
    fi
    ci_body="$(recipe_body ci)"
    if ! grep -qE '^[[:space:]]*just[[:space:]]+test([[:space:]]|$)' <<<"${ci_body}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: just ci must still run local just test:" >&2
      echo "${ci_body}" >&2
      exit 1
    fi
    if grep -q 'require_remote_builder' <<<"${ci_body}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: just ci must stay local (no require_remote_builder):" >&2
      echo "${ci_body}" >&2
      exit 1
    fi
    if grep -q 'check-remote' "${gha}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: GitHub Actions must not call check-remote" >&2
      exit 1
    fi
    if ! grep -q 'preferLocalBuild = false' "${flake}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: flake.nix must set preferLocalBuild = false on the cargo quality derivation" >&2
      exit 1
    fi
    quality="$(awk '
      $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /openrouter-credentials/ { exit }
      p { print }
    ' "${flake}")"
    if ! grep -q 'clippy-driver' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: flake.nix workspace-cargo-quality must lint with clippy-driver (not skip clippy)" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -qE -- '--workspace --all-targets --locked' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: flake.nix workspace-cargo-quality must still lint the workspace (--all-targets)" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if grep -qE -- '--workspace --lib --bins' <<<"${quality}" && ! grep -q -- '--all-targets' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: quality must not lint only --lib --bins without --all-targets" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -qE 'nextest run --workspace --locked' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: flake.nix workspace-cargo-quality must run cargo nextest run --workspace --locked (tests must actually execute, same as just test)" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if grep -q -- '--no-run' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: workspace-cargo-quality must not be compile-only (no cargo --no-run)" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -qE 'test --workspace --doc' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: workspace-cargo-quality must run cargo test --workspace --doc like just test" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'crates/codegen/cargo-mem-guard/Cargo.toml' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: workspace-cargo-quality must run workspace-excluded cargo-mem-guard tests like just test" >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'cargo-nextest' <<<"${quality}"; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: workspace-cargo-quality must put cargo-nextest on the derivation PATH" >&2
      echo "${quality}" >&2
      exit 1
    fi
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    prefer="$(nix eval ".#packages.${sys}.workspace-cargo-quality.preferLocalBuild")"
    if [[ "${prefer}" != "false" ]]; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: expected preferLocalBuild=false, got ${prefer}" >&2
      exit 1
    fi
    drv="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.drvPath")"
    if [[ ! "${drv}" == /nix/store/*.drv ]]; then
      echo "test-check-remote-cargo-is-remote-nix-derivation: expected a store .drv, got ${drv}" >&2
      exit 1
    fi
    echo "test-check-remote-cargo-is-remote-nix-derivation: ok (check-remote builds ${drv})"

# A bash shebang recipe must not leave .#attr unquoted: bash treats # as a
# comment, so `nix build -L .#workspace-cargo-quality` becomes `nix build -L .`
# (packages.default / grok-oss) with no requiredSystemFeatures. nix_retry must
# not splice {{ cmd }} into the script either, or a # in the command comments
# out the force-remote flags. Does not run check-remote or realize quality.
test-check-remote-quotes-quality-attr:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    recipe_body() {
      local name="$1"
      awk -v name="${name}" '
        $0 ~ ("^" name "([ \t]|:)") { p=1; next }
        p && /^[a-zA-Z0-9_.-]+[ \t]*:/ { exit }
        p { print }
      ' "${justfile}"
    }
    remote_body="$(recipe_body check-remote)"
    retry_body="$(recipe_body nix_retry)"
    if grep -qE '(^|[^"'\''])\.#workspace-cargo-quality' <<<"${remote_body}"; then
      echo "test-check-remote-quotes-quality-attr: check-remote must quote .#workspace-cargo-quality (unquoted # is a bash comment; nix then builds . / grok-oss locally):" >&2
      echo "${remote_body}" >&2
      exit 1
    fi
    if ! grep -qE '["'\'']\.#workspace-cargo-quality["'\'']' <<<"${remote_body}"; then
      echo "test-check-remote-quotes-quality-attr: check-remote must nix build the quoted .#workspace-cargo-quality attr:" >&2
      echo "${remote_body}" >&2
      exit 1
    fi
    if grep -vE '^[[:space:]]*#' <<<"${retry_body}" | grep -qE '\{\{[[:space:]]*cmd[[:space:]]*\}\}'; then
      echo "test-check-remote-quotes-quality-attr: nix_retry must not interpolate {{ "{{" }} cmd }} into the bash script (a # in cmd comments out force-remote flags). Use \"\$@\" \"\${force_remote_opts[@]}\":" >&2
      echo "${retry_body}" >&2
      exit 1
    fi
    if ! grep -qE '"\$@"[[:space:]]+"\$\{force_remote_opts\[@\]\}"' <<<"${retry_body}"; then
      echo "test-check-remote-quotes-quality-attr: nix_retry must invoke \"\$@\" \"\${force_remote_opts[@]}\" so # cannot comment force-remote flags:" >&2
      echo "${retry_body}" >&2
      exit 1
    fi
    if ! awk '
      $0 ~ /^nix_retry([ \t]|:)/ { found=1; exit }
      $0 ~ /^\[positional-arguments\]/ { pos=1 }
      $0 ~ /^\[/ { next }
      { pos=0 }
      END { exit found && pos ? 0 : 1 }
    ' "${justfile}"; then
      echo "test-check-remote-quotes-quality-attr: nix_retry must set [positional-arguments] so \"\$@\" is the command words, not empty:" >&2
      exit 1
    fi
    echo "test-check-remote-quotes-quality-attr: ok (quality attr quoted; nix_retry uses argv)"

# max-jobs=0 plus crane vendor unpacks that prefer a local build cannot
# realize: Nix will not send preferLocalBuild derivations to remotes, and
# max-jobs=0 then forbids them here. vendor-registry is runCommandLocal.
# Force-remote must keep a local job slot for those unpacks and pin rustc
# to the remote with requiredSystemFeatures = [ "big-parallel" ].
test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    flake="${root}/flake.nix"
    if grep -nE -- '--option[[:space:]]+max-jobs[[:space:]]+0' "${justfile}" | grep -q .; then
      echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: GROK_NIX_FORCE_REMOTE must not set max-jobs=0." >&2
      echo "crane vendor-registry is runCommandLocal (preferLocalBuild). cargo-package unpacks are the same class." >&2
      echo "Nix will not send those to remotes, so max-jobs=0 means they cannot build anywhere." >&2
      grep -nE -- '--option[[:space:]]+max-jobs[[:space:]]+0' "${justfile}" >&2 || true
      exit 1
    fi
    if ! grep -q 'surmount-remote' "${flake}"; then
      echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: flake.nix must require surmount-remote on workspace cargo rustc" >&2
      exit 1
    fi
    if ! grep -A25 'workspaceCargoArtifacts = craneLib.buildDepsOnly' "${flake}" | grep -q 'surmount-remote'; then
      echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: workspaceCargoArtifacts (dep rustc) must require surmount-remote" >&2
      exit 1
    fi
    if ! grep -A35 'workspace-cargo-quality = craneLib.mkCargoDerivation' "${flake}" | grep -q 'surmount-remote'; then
      echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: workspace-cargo-quality must require surmount-remote" >&2
      exit 1
    fi
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    feats="$(nix eval ".#packages.${sys}.workspace-cargo-quality.requiredSystemFeatures")"
    if ! grep -q 'big-parallel' <<<"${feats}" || ! grep -q 'surmount-remote' <<<"${feats}"; then
      echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: expected requiredSystemFeatures to include big-parallel and surmount-remote, got ${feats}" >&2
      exit 1
    fi
    prefer="$(nix eval ".#packages.${sys}.workspace-cargo-quality.preferLocalBuild")"
    if [[ "${prefer}" != "false" ]]; then
      echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: rustc derivation must keep preferLocalBuild=false, got ${prefer}" >&2
      exit 1
    fi
    echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: ok (no max-jobs=0; rustc requires big-parallel)"

# Remote rustc must not stay 8-wide or inherit CARGO_BUILD_JOBS=2 from the
# low-memory package sandbox. Force-remote nix must pass --cores 64 so
# NIX_BUILD_CORES follows the builder, and workspace-cargo-quality must set
# CARGO_BUILD_JOBS to 32 (OOM hedge vs 64 rustc processes). Cargo clippy
# and cargo test --doc must pass --jobs on argv from those cores (capped),
# after the subcommand (`cargo clippy --jobs N`; cargo 1.97 has no global
# `cargo --jobs`), and must drop MAKEFLAGS/CARGO_MAKEFLAGS so a 1-token
# jobserver cannot ignore --jobs. cargo nextest run uses CARGO_BUILD_JOBS
# for rustc. Must use the dev profile like local `just test-clippy`,
# not crane's default --release check (one rustc thread at opt-level 3).
# Host machines max-jobs lives outside this tree. Does not realize the
# derivation.
test-check-remote-uses-builder-cores:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    flake="${root}/flake.nix"
    retry_body="$(awk '
      $0 ~ /^nix_retry / { p=1 }
      p && $0 ~ /^[a-zA-Z0-9_.-]+[ \t]*:/ { exit }
      p { print }
    ' "${justfile}")"
    if ! grep -qE -- '--cores[[:space:]]+64' <<<"${retry_body}"; then
      echo "test-check-remote-uses-builder-cores: GROK_NIX_FORCE_REMOTE / nix_retry must pass --cores 64 so one workspace rustc is not 8-wide." >&2
      echo "${retry_body}" >&2
      exit 1
    fi
    jobs_helper="$(awk '
      $0 ~ /workspaceCargoJobsFromCores =/ { p=1 }
      p && $0 ~ /workspaceCargoArtifacts = craneLib.buildDepsOnly/ { exit }
      p { print }
    ' "${flake}")"
    quality="$(awk '
      $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /openrouter-credentials/ { exit }
      p { print }
    ' "${flake}")"
    artifacts="$(awk '
      $0 ~ /workspaceCargoArtifacts = craneLib.buildDepsOnly/ { p=1 }
      p && $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { exit }
      p { print }
    ' "${flake}")"
    if ! grep -q 'CARGO_BUILD_JOBS = "32"' <<<"${quality}"; then
      echo "test-check-remote-uses-builder-cores: workspace-cargo-quality must set CARGO_BUILD_JOBS = \"32\" (not inherit 2 from commonArgs)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'CARGO_BUILD_JOBS = "32"' <<<"${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: workspaceCargoArtifacts must set CARGO_BUILD_JOBS = \"32\"." >&2
      echo "${artifacts}" >&2
      exit 1
    fi
    if ! grep -q 'enableParallelBuilding = true' <<<"${quality}"; then
      echo "test-check-remote-uses-builder-cores: workspace-cargo-quality must set enableParallelBuilding = true." >&2
      exit 1
    fi
    if ! grep -q 'enableParallelBuilding = true' <<<"${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: workspaceCargoArtifacts must set enableParallelBuilding = true." >&2
      exit 1
    fi
    if ! grep -q 'CARGO_BUILD_JOBS = "2"' "${flake}"; then
      echo "test-check-remote-uses-builder-cores: commonArgs must keep CARGO_BUILD_JOBS = \"2\" for the local/GHA package sandbox." >&2
      exit 1
    fi
    if ! grep -q 'CARGO_PROFILE = "dev"' <<<"${quality}"; then
      echo "test-check-remote-uses-builder-cores: workspace-cargo-quality must set CARGO_PROFILE = \"dev\" so clippy/check is not crane --release (one rustc thread, opt-level 3)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'CARGO_PROFILE = "dev"' <<<"${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: workspaceCargoArtifacts must set CARGO_PROFILE = \"dev\" (same profile as quality, or clippy rebuilds deps)." >&2
      echo "${artifacts}" >&2
      exit 1
    fi
    if ! grep -q -- '--jobs' <<<"${quality}"; then
      echo "test-check-remote-uses-builder-cores: workspace-cargo-quality must pass cargo --jobs on argv (CARGO_BUILD_JOBS env alone is not enough)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if grep -qE -- 'cargo --jobs' <<<"${quality}${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: cargo 1.97 has no global --jobs; put --jobs after the subcommand (cargo check --jobs, cargo test --jobs)." >&2
      echo "${quality}" >&2
      echo "${artifacts}" >&2
      exit 1
    fi
    if ! grep -qE -- 'check --profile "\$CARGO_PROFILE" --jobs "\$CARGO_BUILD_JOBS"' <<<"${quality}"; then
      echo "test-check-remote-uses-builder-cores: workspace clippy must pass --jobs after cargo check (builtin; cargo clippy is an external dispatcher)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -qE -- 'check --profile "\$CARGO_PROFILE" --jobs "\$CARGO_BUILD_JOBS"' <<<"${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: artifacts check must pass --jobs after the subcommand (cargo check --jobs)." >&2
      echo "${artifacts}" >&2
      exit 1
    fi
    if ! grep -q -- '--jobs' <<<"${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: workspaceCargoArtifacts must pass cargo --jobs on argv." >&2
      echo "${artifacts}" >&2
      exit 1
    fi
    if ! grep -q 'NIX_BUILD_CORES' <<<"${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: cargo --jobs must be taken from NIX_BUILD_CORES (then capped at 32)." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'unset MAKEFLAGS' <<<"${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: must unset MAKEFLAGS/CARGO_MAKEFLAGS so a 1-token GNU jobserver cannot ignore cargo --jobs." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'workspaceCargoJobsFromCores' <<<"${quality}" || ! grep -q 'workspaceCargoJobsFromCores' <<<"${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: quality and artifacts build phases must use workspaceCargoJobsFromCores before cargo --jobs." >&2
      echo "${quality}" >&2
      echo "${artifacts}" >&2
      exit 1
    fi
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    jobs="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.CARGO_BUILD_JOBS")"
    if [[ "${jobs}" != "32" ]]; then
      echo "test-check-remote-uses-builder-cores: expected CARGO_BUILD_JOBS=32 on workspace-cargo-quality, got ${jobs}" >&2
      exit 1
    fi
    profile="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.CARGO_PROFILE")"
    if [[ "${profile}" != "dev" ]]; then
      echo "test-check-remote-uses-builder-cores: expected CARGO_PROFILE=dev on workspace-cargo-quality, got ${profile}" >&2
      exit 1
    fi
    # Instantiated buildPhase is what the remote builder runs. Source greps
    # can pass while crane preBuild copies NIX_BUILD_CORES=1 into
    # CARGO_BUILD_JOBS and a 1-token jobserver ignores later --jobs.
    phase="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.buildPhase")"
    if ! grep -qE -- 'check --profile "\$CARGO_PROFILE" --jobs "\$CARGO_BUILD_JOBS"' <<<"${phase}"; then
      echo "test-check-remote-uses-builder-cores: instantiated buildPhase must pass cargo check --jobs from CARGO_BUILD_JOBS (cap 32 from NIX_BUILD_CORES) for workspace clippy." >&2
      echo "${phase}" >&2
      exit 1
    fi
    clippy_cmd="$(grep 'clippy-driver' <<<"${phase}" | head -1 || true)"
    nextest_cmd="$(grep 'cargo nextest' <<<"${phase}" | head -1 || true)"
    doctest_cmd="$(grep 'cargo test --workspace --doc' <<<"${phase}" | head -1 || true)"
    if grep -qE -- '-j[[:space:]]*1|--jobs[[:space:]]+1' <<<"${clippy_cmd}"; then
      echo "test-check-remote-uses-builder-cores: clippy must not be given --jobs 1." >&2
      echo "${clippy_cmd}" >&2
      exit 1
    fi
    unset_line="$(grep -n 'unset MAKEFLAGS' <<<"${phase}" | head -1 | cut -d: -f1 || true)"
    clippy_line="$(grep -n 'clippy-driver' <<<"${phase}" | head -1 | cut -d: -f1 || true)"
    nextest_line="$(grep -n 'cargo nextest' <<<"${phase}" | head -1 | cut -d: -f1 || true)"
    doctest_line="$(grep -n 'cargo test --workspace --doc' <<<"${phase}" | head -1 | cut -d: -f1 || true)"
    if [[ -z "${unset_line}" ]] || ! grep -q 'CARGO_MAKEFLAGS' <<<"${phase}"; then
      echo "test-check-remote-uses-builder-cores: instantiated buildPhase must unset MAKEFLAGS and CARGO_MAKEFLAGS so a 1-token jobserver cannot ignore --jobs." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if [[ -z "${clippy_line}" || "${unset_line}" -ge "${clippy_line}" ]]; then
      echo "test-check-remote-uses-builder-cores: unset MAKEFLAGS must run before clippy-driver workspace lint." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if [[ -z "${nextest_line}" || "${unset_line}" -ge "${nextest_line}" ]]; then
      echo "test-check-remote-uses-builder-cores: unset MAKEFLAGS must run before cargo nextest (not -j1)." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if [[ -z "${doctest_line}" || "${unset_line}" -ge "${doctest_line}" ]]; then
      echo "test-check-remote-uses-builder-cores: unset MAKEFLAGS must run before cargo test --doc." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -q -- '--jobs "$CARGO_BUILD_JOBS"' <<<"${doctest_cmd}"; then
      echo "test-check-remote-uses-builder-cores: instantiated cargo test --doc must pass --jobs from CARGO_BUILD_JOBS." >&2
      echo "${doctest_cmd}" >&2
      exit 1
    fi
    if grep -qE -- '-j[[:space:]]*1|--jobs[[:space:]]+1' <<<"${nextest_cmd}"; then
      echo "test-check-remote-uses-builder-cores: nextest must not be given -j1." >&2
      echo "${nextest_cmd}" >&2
      exit 1
    fi
    if grep -q 'floor="${CARGO_BUILD_JOBS' <<<"${phase}" || grep -q "floor=\"\${CARGO_BUILD_JOBS" <<<"${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: do not floor cargo jobs from CARGO_BUILD_JOBS (crane preBuild can set it to NIX_BUILD_CORES=1)." >&2
      echo "${jobs_helper}" >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -qE 'cargoJobs" -lt 2|"\$cargoJobs" -lt 2' <<<"${phase}"; then
      echo "test-check-remote-uses-builder-cores: when NIX_BUILD_CORES is 1, cargo jobs must still become 32 (cap), not one rustc." >&2
      echo "${phase}" >&2
      exit 1
    fi
    echo "test-check-remote-uses-builder-cores: ok (--cores 64; workspace cargo jobs 32 from cores on argv; CARGO_PROFILE=dev; package sandbox still 2)"

# `cargo clippy --workspace --jobs N` is an external cargo-clippy binary.
# The outer cargo may start a 1-token GNU jobserver from
# available_parallelism() (often 1 in a Nix sandbox). Inner `--jobs N` is
# then ignored: one clippy-driver, sequential Checking, idle cores.
# Quality must lint via builtin `cargo check` + clippy-driver under a GNU
# make jobserver with $CARGO_BUILD_JOBS tokens (cap 32). Must not loop
# `cargo clippy -p` with -j1. Does not realize rustc.
test-check-remote-clippy-uses-many-workers:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    flake="${root}/flake.nix"
    jobs_helper="$(awk '
      $0 ~ /workspaceCargoJobsFromCores =/ { p=1 }
      p && $0 ~ /workspaceCargoArtifacts = craneLib.buildDepsOnly/ { exit }
      p { print }
    ' "${flake}")"
    quality="$(awk '
      $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /openrouter-credentials/ { exit }
      p { print }
    ' "${flake}")"
    named="$(awk '
      $0 ~ /workspace-cargo-named-test = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /ciLowMemEnv/ { exit }
      p { print }
    ' "${flake}")"
    if grep -qE -- 'cargo clippy --' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: workspace-cargo-quality must not invoke cargo clippy (external dispatcher; 1-token jobserver). Use cargo check + clippy-driver." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'workspace_run_make_jobserver' <<<"${jobs_helper}"; then
      echo "test-check-remote-clippy-uses-many-workers: workspaceCargoJobsFromCores must define workspace_run_make_jobserver (GNU make -j\$CARGO_BUILD_JOBS tokens)." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -qE -- 'make -j"\$CARGO_BUILD_JOBS"' <<<"${jobs_helper}"; then
      echo "test-check-remote-clippy-uses-many-workers: jobserver helper must run make -j\"\$CARGO_BUILD_JOBS\" (cap 32 from cores)." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'RUSTC_WORKSPACE_WRAPPER' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: quality must set RUSTC_WORKSPACE_WRAPPER to clippy-driver." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'clippy-driver' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: quality must run clippy-driver (workspace lint)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'CLIPPY_ARGS' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: quality must set CLIPPY_ARGS so clippy-driver still denies warnings." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -qE -- '-D__CLIPPY_HACKERY__warnings|CLIPPY_ARGS=.*-D' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: CLIPPY_ARGS must deny warnings (-D warnings via clippy-driver)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'workspace_run_make_jobserver' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: quality clippy must run under workspace_run_make_jobserver so independent crates share a \$CARGO_BUILD_JOBS-token jobserver." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -qE -- 'check --profile "\$CARGO_PROFILE" --jobs "\$CARGO_BUILD_JOBS" --workspace --all-targets --locked' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: quality must cargo check --workspace --all-targets --jobs from cores (one cargo; not a per-crate -j1 loop)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if grep -qE -- 'for[[:space:]].+in[[:space:]].+; do' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: quality must not loop crates (cargo lock serializes cargo clippy -p; -j1 is forbidden)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if grep -qE -- '-j[[:space:]]*1|--jobs[[:space:]]+1' <<<"${quality}${jobs_helper}"; then
      echo "test-check-remote-clippy-uses-many-workers: quality clippy/jobserver must not pass -j1 / --jobs 1." >&2
      echo "${quality}" >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    phase="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.buildPhase")"
    if grep -qE -- 'cargo clippy --' <<<"${phase}"; then
      echo "test-check-remote-clippy-uses-many-workers: instantiated buildPhase must not invoke cargo clippy." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -q 'clippy-driver' <<<"${phase}" || ! grep -q 'RUSTC_WORKSPACE_WRAPPER' <<<"${phase}"; then
      echo "test-check-remote-clippy-uses-many-workers: instantiated buildPhase must set RUSTC_WORKSPACE_WRAPPER=clippy-driver." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -q 'workspace_run_make_jobserver' <<<"${phase}" || ! grep -qE -- 'make -j"\$CARGO_BUILD_JOBS"' <<<"${phase}"; then
      echo "test-check-remote-clippy-uses-many-workers: instantiated buildPhase must run make -j\"\$CARGO_BUILD_JOBS\" for clippy." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -qE -- 'check --profile "\$CARGO_PROFILE" --jobs "\$CARGO_BUILD_JOBS" --workspace --all-targets --locked' <<<"${phase}"; then
      echo "test-check-remote-clippy-uses-many-workers: instantiated clippy must be cargo check --workspace --all-targets --jobs from CARGO_BUILD_JOBS." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if grep -qE -- '-j[[:space:]]*1|--jobs[[:space:]]+1' <<<"${phase}"; then
      echo "test-check-remote-clippy-uses-many-workers: instantiated buildPhase must not pass -j1 / --jobs 1." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if grep -qE -- 'for[[:space:]].+in[[:space:]].+; do' <<<"${phase}"; then
      echo "test-check-remote-clippy-uses-many-workers: instantiated buildPhase must not loop crates with cargo clippy -p." >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -q 'clippy-driver' <<<"${named}" || ! grep -q 'workspace_run_make_jobserver' <<<"${named}"; then
      echo "test-check-remote-clippy-uses-many-workers: named-test clippy kind must use clippy-driver + make jobserver too." >&2
      echo "${named}" >&2
      exit 1
    fi
    named_clippy="$(awk '
      $0 ~ /clippy\)/ { p=1 }
      p && $0 ~ /build\)/ { exit }
      p { print }
    ' <<<"${named}")"
    if ! grep -q -- '--all-targets' <<<"${named_clippy}"; then
      echo "test-check-remote-clippy-uses-many-workers: named-test clippy kind must cargo check --all-targets." >&2
      echo "${named_clippy}" >&2
      exit 1
    fi
    echo "test-check-remote-clippy-uses-many-workers: ok (cargo check + clippy-driver; make -j\$CARGO_BUILD_JOBS jobserver; no cargo clippy dispatcher; no -j1 crate loop)"

# Dummy workspace deps stubs do not need the pager build-id. GROK_GIT_SHA
# from dirtyShortRev must not be on workspace-cargo-quality-deps or a dirty
# tree (even files cargo filter drops) busts the remote crates.io cache.
# grok-oss and quality still inject GROK_GIT_SHA (pager-bin build.rs).
# Instantiates .drv files only. Does not realize rustc or run check-remote.
test-check-remote-deps-omit-git-sha:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    flake="${root}/flake.nix"
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    artifacts="$(awk '
      $0 ~ /workspaceCargoArtifacts = craneLib.buildDepsOnly/ { p=1 }
      p && $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { exit }
      p { print }
    ' "${flake}")"
    quality="$(awk '
      $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /openrouter-credentials/ { exit }
      p { print }
    ' "${flake}")"
    grok_oss="$(awk '
      $0 ~ /grok-oss = craneLib.buildPackage/ { p=1 }
      p && $0 ~ /cargoCheck = craneLib.mkCargoDerivation/ { exit }
      p { print }
    ' "${flake}")"
    if ! grep -q 'GROK_GIT_SHA = self.shortRev or self.dirtyShortRev or "unknown"' "${flake}"; then
      echo "test-check-remote-deps-omit-git-sha: grok-oss still needs GROK_GIT_SHA from shortRev/dirtyShortRev (pager-bin build.rs)." >&2
      exit 1
    fi
    if ! grep -q 'commonArgs' <<<"${grok_oss}"; then
      echo "test-check-remote-deps-omit-git-sha: grok-oss must keep commonArgs (GROK_GIT_SHA for pager-bin)." >&2
      echo "${grok_oss}" >&2
      exit 1
    fi
    if ! grep -q 'removeAttrs commonArgs' <<<"${artifacts}" || ! grep -q 'GROK_GIT_SHA' <<<"${artifacts}"; then
      echo "test-check-remote-deps-omit-git-sha: workspaceCargoArtifacts must drop GROK_GIT_SHA via removeAttrs commonArgs so dirtyShortRev cannot bust the deps drv." >&2
      echo "${artifacts}" >&2
      exit 1
    fi
    if grep -q 'removeAttrs commonArgs' <<<"${quality}"; then
      echo "test-check-remote-deps-omit-git-sha: workspace-cargo-quality compiles pager-bin and must keep GROK_GIT_SHA (do not removeAttrs it)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if ! grep -q 'commonArgs' <<<"${quality}"; then
      echo "test-check-remote-deps-omit-git-sha: workspace-cargo-quality must keep commonArgs (GROK_GIT_SHA for pager-bin build.rs)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    grok_sha="$(nix eval --raw ".#packages.${sys}.grok-oss.GROK_GIT_SHA")"
    if [[ -z "${grok_sha}" ]]; then
      echo "test-check-remote-deps-omit-git-sha: expected grok-oss.GROK_GIT_SHA to be set, got empty." >&2
      exit 1
    fi
    quality_sha="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.GROK_GIT_SHA")"
    if [[ -z "${quality_sha}" ]]; then
      echo "test-check-remote-deps-omit-git-sha: expected workspace-cargo-quality.GROK_GIT_SHA to be set (clippy compiles pager-bin), got empty." >&2
      exit 1
    fi
    quality_drv="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.drvPath")"
    deps_drv="$(rg -o '/nix/store/[0-9a-z]+-workspace-cargo-quality-deps-[^"]+\.drv' "${quality_drv}" | head -n1)"
    if [[ -z "${deps_drv}" || ! -e "${deps_drv}" ]]; then
      echo "test-check-remote-deps-omit-git-sha: expected workspace-cargo-quality-deps.drv among quality inputs" >&2
      exit 1
    fi
    if rg -q 'GROK_GIT_SHA' "${deps_drv}"; then
      echo "test-check-remote-deps-omit-git-sha: workspace-cargo-quality-deps.drv must not set GROK_GIT_SHA (dirtyShortRev must not bust crates.io deps)." >&2
      exit 1
    fi
    if ! rg -q 'GROK_GIT_SHA' "${quality_drv}"; then
      echo "test-check-remote-deps-omit-git-sha: workspace-cargo-quality.drv must still set GROK_GIT_SHA." >&2
      exit 1
    fi
    echo "test-check-remote-deps-omit-git-sha: ok (deps omit GROK_GIT_SHA; grok-oss and quality keep it)"

# requiredSystemFeatures=big-parallel only keeps rustc off this machine when
# local Nix does not advertise that feature. This host's user config does
# advertise it, so force-remote must pass --option system-features that omit
# big-parallel (and benchmark) on the caller. Do not set max-jobs=0.
# Does not realize the workspace rustc derivation.
test-check-remote-omits-local-big-parallel:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    retry_body="$(awk '
      $0 ~ /^nix_retry / { p=1 }
      p && $0 ~ /^[a-zA-Z0-9_.-]+[ \t]*:/ { exit }
      p { print }
    ' "${justfile}")"
    if grep -nE -- '--option[[:space:]]+max-jobs[[:space:]]+0' "${justfile}" | grep -q .; then
      echo "test-check-remote-omits-local-big-parallel: GROK_NIX_FORCE_REMOTE must not set max-jobs=0." >&2
      exit 1
    fi
    if ! grep -qE -- '--option[[:space:]]+system-features' <<<"${retry_body}"; then
      echo "test-check-remote-omits-local-big-parallel: GROK_NIX_FORCE_REMOTE / nix_retry must pass --option system-features that omit big-parallel so this host cannot claim workspace rustc." >&2
      echo "This host's nix show-config advertises big-parallel. requiredSystemFeatures alone is not enough." >&2
      echo "${retry_body}" >&2
      exit 1
    fi
    feats="$(awk '
      /--option[[:space:]]+system-features/ {
        sub(/.*system-features[[:space:]]+/, "")
        gsub(/["'\'']/, "")
        print
        exit
      }
    ' <<<"${retry_body}")"
    if [[ -z "${feats}" ]]; then
      echo "test-check-remote-omits-local-big-parallel: could not parse the force-remote system-features list." >&2
      echo "${retry_body}" >&2
      exit 1
    fi
    if grep -Eq '(^|[[:space:]])big-parallel($|[[:space:]])' <<<"${feats}"; then
      echo "test-check-remote-omits-local-big-parallel: force-remote system-features must omit big-parallel, got: ${feats}" >&2
      exit 1
    fi
    if grep -Eq '(^|[[:space:]])benchmark($|[[:space:]])' <<<"${feats}"; then
      echo "test-check-remote-omits-local-big-parallel: force-remote system-features must omit benchmark, got: ${feats}" >&2
      exit 1
    fi
    shown="$(nix --option system-features "${feats}" show-config | awk -F' = ' '/^system-features / { print $2; exit }')"
    if grep -Eq '(^|[[:space:]])big-parallel($|[[:space:]])' <<<"${shown}"; then
      echo "test-check-remote-omits-local-big-parallel: nix --option system-features ${feats} still advertises big-parallel: ${shown}" >&2
      exit 1
    fi
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    required="$(nix eval ".#packages.${sys}.workspace-cargo-quality.requiredSystemFeatures")"
    if ! grep -q 'big-parallel' <<<"${required}"; then
      echo "test-check-remote-omits-local-big-parallel: workspace-cargo-quality must still require big-parallel, got ${required}" >&2
      exit 1
    fi
    echo "test-check-remote-omits-local-big-parallel: ok (local system-features omit big-parallel; rustc still requires it)"

# --option system-features is not what the local nix-daemon builder uses.
# This host still advertises big-parallel by default (see
# https://nix.dev/manual/nix/2.28/command-ref/conf-file.html#conf-system-features
# accessed: 2026-08-18). Workspace rustc .drv files must require a feature
# this laptop never has. The host machines file must advertise that same
# feature on the ssh-ng builder. nix build --dry-run only lists missing
# outputs; it is not a machine-assignment proof. Does not run check-remote.
# Does not realize the quality derivation.
test-check-remote-workspace-rustc-not-local-eligible:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    flake="${root}/flake.nix"
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    local_feats="$(nix config show | awk -F' = ' '/^system-features / { print $2; exit }')"
    drv_feats() {
      local drv="$1"
      local raw
      raw="$(rg -o '\("requiredSystemFeatures","[^"]*"' "${drv}" | sed 's/.*","//;s/"$//' || true)"
      echo "${raw}"
    }
    missing_from_local() {
      local required="$1"
      local miss=""
      local f
      for f in ${required}; do
        if ! grep -Eq "(^|[[:space:]])${f}($|[[:space:]])" <<<"${local_feats}"; then
          miss="${miss} ${f}"
        fi
      done
      echo "${miss}"
    }
    quality_drv="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.drvPath")"
    deps_drv="$(rg -o '/nix/store/[0-9a-z]+-workspace-cargo-quality-deps-[^"]+\.drv' "${quality_drv}" | head -n1)"
    if [[ -z "${deps_drv}" || ! -e "${deps_drv}" ]]; then
      echo "test-check-remote-workspace-rustc-not-local-eligible: expected workspace-cargo-quality-deps.drv among quality inputs" >&2
      exit 1
    fi
    q_req="$(drv_feats "${quality_drv}")"
    d_req="$(drv_feats "${deps_drv}")"
    q_miss="$(missing_from_local "${q_req}")"
    d_miss="$(missing_from_local "${d_req}")"
    if [[ -z "${q_miss}" || -z "${d_miss}" ]]; then
      echo "test-check-remote-workspace-rustc-not-local-eligible: workspace rustc .drv requiredSystemFeatures must include a feature this laptop's default Nix does not advertise." >&2
      echo "--option system-features is ignored for local nixbld scheduling (dry-run with builders empty still takes big-parallel jobs)." >&2
      echo "default local system-features: ${local_feats}" >&2
      echo "quality ${quality_drv} requiredSystemFeatures: ${q_req:-<missing>}" >&2
      echo "deps ${deps_drv} requiredSystemFeatures: ${d_req:-<missing>}" >&2
      exit 1
    fi
    if ! grep -q 'surmount-remote' <<<"${q_req}${d_req}"; then
      echo "test-check-remote-workspace-rustc-not-local-eligible: flake rustc drvs must require surmount-remote (a machines-file feature this laptop never auto-detects)." >&2
      echo "quality requiredSystemFeatures: ${q_req}" >&2
      echo "deps requiredSystemFeatures: ${d_req}" >&2
      exit 1
    fi
    if ! grep -A20 'workspaceCargoArtifacts = craneLib.buildDepsOnly' "${flake}" | grep -q 'surmount-remote'; then
      echo "test-check-remote-workspace-rustc-not-local-eligible: workspaceCargoArtifacts must require surmount-remote" >&2
      exit 1
    fi
    if ! grep -A30 'workspace-cargo-quality = craneLib.mkCargoDerivation' "${flake}" | grep -q 'surmount-remote'; then
      echo "test-check-remote-workspace-rustc-not-local-eligible: workspace-cargo-quality must require surmount-remote" >&2
      exit 1
    fi
    file="${GROK_NIX_BUILDERS_FILE:-$HOME/.config/nix/machines}"
    if [[ ! -s "${file}" ]]; then
      echo "test-check-remote-workspace-rustc-not-local-eligible: builders file is missing or empty (override with GROK_NIX_BUILDERS_FILE)." >&2
      exit 1
    fi
    builder_feats="$(awk '$1 ~ /^ssh-ng:/ { print $6; exit }' "${file}")"
    if ! grep -Eq '(^|,)surmount-remote(,|$)' <<<"${builder_feats}"; then
      echo "test-check-remote-workspace-rustc-not-local-eligible: the ssh-ng builder supported-features column must include surmount-remote (do not print the builders URI)." >&2
      echo "parsed supported-features: ${builder_feats}" >&2
      exit 1
    fi
    echo "test-check-remote-workspace-rustc-not-local-eligible: ok (rustc requires surmount-remote; local Nix does not advertise it; machines file does)"

# Force-remote must export NIX_SSHOPTS with this account's known_hosts so
# client SSH (and any Nix process that honors the env) can verify the
# builder. Must not disable host-key checks. Does not run check-remote.
test-check-remote-exports-nix-sshopts:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    recipe_body() {
      local name="$1"
      awk -v name="${name}" '
        $0 ~ ("^" name "([ \t]|:)") { p=1; next }
        p && /^[a-zA-Z0-9_.-]+[ \t]*:/ { exit }
        p { print }
      ' "${justfile}"
    }
    remote_body="$(recipe_body check-remote)"
    retry_body="$(recipe_body nix_retry)"
    require_body="$(recipe_body require_remote_builder)"
    combined="${remote_body}"$'\n'"${retry_body}"$'\n'"${require_body}"
    if ! grep -q 'NIX_SSHOPTS' <<<"${combined}"; then
      echo "test-check-remote-exports-nix-sshopts: check-remote / nix_retry / require_remote_builder must export NIX_SSHOPTS:" >&2
      echo "${combined}" >&2
      exit 1
    fi
    if ! grep -q 'UserKnownHostsFile' <<<"${combined}"; then
      echo "test-check-remote-exports-nix-sshopts: NIX_SSHOPTS must point SSH at UserKnownHostsFile (this account's known_hosts):" >&2
      echo "${combined}" >&2
      exit 1
    fi
    if ! grep -qE 'HOME.*\.ssh/known_hosts|GROK_NIX_KNOWN_HOSTS' <<<"${combined}"; then
      echo "test-check-remote-exports-nix-sshopts: UserKnownHostsFile must be this account's known_hosts (HOME/.ssh/known_hosts or GROK_NIX_KNOWN_HOSTS):" >&2
      echo "${combined}" >&2
      exit 1
    fi
    if grep -q 'StrictHostKeyChecking=no' <<<"${combined}"; then
      echo "test-check-remote-exports-nix-sshopts: must not set StrictHostKeyChecking=no:" >&2
      echo "${combined}" >&2
      exit 1
    fi
    if ! grep -q 'GROK_NIX_FORCE_REMOTE' <<<"${retry_body}"; then
      echo "test-check-remote-exports-nix-sshopts: nix_retry must still key force-remote off GROK_NIX_FORCE_REMOTE:" >&2
      echo "${retry_body}" >&2
      exit 1
    fi
    echo "test-check-remote-exports-nix-sshopts: ok (NIX_SSHOPTS uses this account's known_hosts; host-key checks stay on)"

# require_remote_builder is on check-remote. User SSH to Host surmount-1
# is not the nix build path. An empty known_hosts (or missing NIX_SSHOPTS
# host-key file) must fail even when a dummy ssh-ng machines file exists.
# Does not run check-remote or realize quality.
test-check-remote-preflight-same-path-as-nix-ssh:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    recipe_body() {
      local name="$1"
      awk -v name="${name}" '
        $0 ~ ("^" name "([ \t]|:)") { p=1; next }
        p && /^[a-zA-Z0-9_.-]+[ \t]*:/ { exit }
        p { print }
      ' "${justfile}"
    }
    remote_body="$(recipe_body check-remote)"
    require_body="$(recipe_body require_remote_builder)"
    retry_body="$(recipe_body nix_retry)"
    if ! grep -qE '^check-remote:.*require_remote_builder' "${justfile}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: check-remote must invoke require_remote_builder:" >&2
      echo "${remote_body}" >&2
      exit 1
    fi
    if ! grep -q 'NIX_SSHOPTS' <<<"${require_body}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: require_remote_builder must set NIX_SSHOPTS (user ssh to Host surmount-1 is not enough):" >&2
      echo "${require_body}" >&2
      exit 1
    fi
    if ! grep -qE 'ssh-keygen|known_hosts' <<<"${require_body}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: require_remote_builder must check this account's known_hosts for the machines-file host:" >&2
      echo "${require_body}" >&2
      exit 1
    fi
    if ! grep -qE 'base64-ssh-public-host-key|sshPublicHostKey|host.key|host_key|UserKnownHostsFile' <<<"${retry_body}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: nix_retry force-remote must pass the user known_hosts key to Nix (machines host-key field or NIX_SSHOPTS UserKnownHostsFile):" >&2
      echo "${retry_body}" >&2
      exit 1
    fi
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT
    machines="${tmpdir}/machines"
    empty_hosts="${tmpdir}/known_hosts"
    : >"${empty_hosts}"
    printf '%s\n' 'ssh-ng://probe@example.invalid x86_64-linux - 1 1 surmount-remote' >"${machines}"
    set +e
    out="$(GROK_NIX_BUILDERS_FILE="${machines}" GROK_NIX_KNOWN_HOSTS="${empty_hosts}" just require_remote_builder 2>&1)"
    status=$?
    set -e
    if [[ "${status}" -eq 0 ]]; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: empty known_hosts must fail preflight (user ssh to Host surmount-1 is not enough):" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 2 ]]; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: expected exit 2, got ${status}:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -qE 'known_hosts|NIX_SSHOPTS|host key' <<<"${out}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: fail message must name the missing host-key / NIX_SSHOPTS path:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -qE 'surmount-1|user SSH|user ssh' <<<"${out}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: fail message must say user ssh to Host surmount-1 is not enough:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|example\.invalid' <<<"${out}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: preflight must not print the machines-file URI:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-check-remote-preflight-same-path-as-nix-ssh: ok (same-path preflight; empty known_hosts fails; surmount-1 is not enough)"

# require_remote_builder must notice when the scheduled remote's nix-daemon
# will refuse surmount-remote (client machines file already advertises it).
# Tests inject GROK_NIX_REMOTE_SYSTEM_FEATURES so this never SSHs to a live
# builder. Does not run check-remote or realize quality.
test-check-remote-preflight-remote-daemon-features:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    recipe_body() {
      local name="$1"
      awk -v name="${name}" '
        $0 ~ ("^" name "([ \t+:]|:)") { p=1; next }
        p && (/^\[/ || /^[a-zA-Z0-9_.-]+([ \t]|:)/) { exit }
        p { print }
      ' "${justfile}"
    }
    require_body="$(recipe_body require_remote_builder)"
    if ! grep -q 'GROK_NIX_REMOTE_SYSTEM_FEATURES' <<<"${require_body}"; then
      echo "test-check-remote-preflight-remote-daemon-features: require_remote_builder must accept GROK_NIX_REMOTE_SYSTEM_FEATURES so tests can inject the daemon feature list:" >&2
      echo "${require_body}" >&2
      exit 1
    fi
    if ! grep -q 'system-features' <<<"${require_body}"; then
      echo "test-check-remote-preflight-remote-daemon-features: require_remote_builder must query the remote daemon system-features:" >&2
      echo "${require_body}" >&2
      exit 1
    fi
    if ! grep -q 'surmount-remote' <<<"${require_body}"; then
      echo "test-check-remote-preflight-remote-daemon-features: require_remote_builder must require surmount-remote on the remote daemon list:" >&2
      echo "${require_body}" >&2
      exit 1
    fi
    if grep -q 'StrictHostKeyChecking=no' <<<"${require_body}"; then
      echo "test-check-remote-preflight-remote-daemon-features: must not set StrictHostKeyChecking=no:" >&2
      echo "${require_body}" >&2
      exit 1
    fi
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT
    machines="${tmpdir}/machines"
    hosts="${tmpdir}/known_hosts"
    printf '%s\n' 'ssh-ng://probe@example.invalid x86_64-linux - 1 1 surmount-remote' >"${machines}"
    printf '%s\n' 'example.invalid ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA' >"${hosts}"
    set +e
    miss_out="$(GROK_NIX_BUILDERS_FILE="${machines}" GROK_NIX_KNOWN_HOSTS="${hosts}" GROK_NIX_REMOTE_SYSTEM_FEATURES='benchmark big-parallel kvm nixos-test' just require_remote_builder 2>&1)"
    miss_status=$?
    set -e
    if [[ "${miss_status}" -eq 0 ]]; then
      echo "test-check-remote-preflight-remote-daemon-features: remote daemon missing surmount-remote must fail preflight:" >&2
      echo "${miss_out}" >&2
      exit 1
    fi
    if [[ "${miss_status}" -ne 2 ]]; then
      echo "test-check-remote-preflight-remote-daemon-features: expected exit 2 when the daemon list omits surmount-remote, got ${miss_status}:" >&2
      echo "${miss_out}" >&2
      exit 1
    fi
    if ! grep -q 'surmount-remote' <<<"${miss_out}"; then
      echo "test-check-remote-preflight-remote-daemon-features: fail message must name surmount-remote:" >&2
      echo "${miss_out}" >&2
      exit 1
    fi
    if ! grep -qE 'nix-daemon|system-features' <<<"${miss_out}"; then
      echo "test-check-remote-preflight-remote-daemon-features: fail message must name the remote daemon system-features miss:" >&2
      echo "${miss_out}" >&2
      exit 1
    fi
    if ! grep -qE 'extra-system-features|nix.conf' <<<"${miss_out}"; then
      echo "test-check-remote-preflight-remote-daemon-features: fail message must say how to add the feature on the builder daemon:" >&2
      echo "${miss_out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|example\.invalid|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|/id_' <<<"${miss_out}"; then
      echo "test-check-remote-preflight-remote-daemon-features: must not print IP, machines URI, or key paths:" >&2
      echo "${miss_out}" >&2
      exit 1
    fi
    set +e
    ok_out="$(GROK_NIX_BUILDERS_FILE="${machines}" GROK_NIX_KNOWN_HOSTS="${hosts}" GROK_NIX_REMOTE_SYSTEM_FEATURES='benchmark big-parallel kvm nixos-test surmount-remote' just require_remote_builder 2>&1)"
    ok_status=$?
    set -e
    if [[ "${ok_status}" -ne 0 ]]; then
      echo "test-check-remote-preflight-remote-daemon-features: injected list with surmount-remote must pass preflight:" >&2
      echo "${ok_out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|example\.invalid|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|/id_' <<<"${ok_out}"; then
      echo "test-check-remote-preflight-remote-daemon-features: pass path must not print IP, machines URI, or key paths:" >&2
      echo "${ok_out}" >&2
      exit 1
    fi
    echo "test-check-remote-preflight-remote-daemon-features: ok (inject miss fails; inject with surmount-remote passes; no live SSH)"

# Prove `just test-remote` / `just cargo-remote` send named cargo through the
# same force-remote nix path as check-remote (GROK_NIX_FORCE_REMOTE,
# require_remote_builder, nix build --impure .#workspace-cargo-named-test).
# Does not realize rustc. Default just ci stays local. GHA must not call
# test-remote or cargo-remote.
test-test-remote-is-force-remote-nix:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    flake="${root}/flake.nix"
    gha="${root}/.github/workflows/ci.yml"
    recipe_body() {
      local name="$1"
      awk -v name="${name}" '
        $0 ~ ("^" name "([ \t*:]|:)") { p=1; next }
        p && (/^\[/ || /^[a-zA-Z0-9_.-]+([ \t]|:)/) { exit }
        p { print }
      ' "${justfile}"
    }
    named_body="$(recipe_body remote_named_cargo)"
    test_body="$(recipe_body test-remote)"
    cargo_body="$(recipe_body cargo-remote)"
    if ! grep -qE '^test-remote:' "${justfile}" && ! grep -qE '^test-remote \*args:' "${justfile}"; then
      echo "test-test-remote-is-force-remote-nix: justfile must define test-remote:" >&2
      exit 1
    fi
    if ! grep -q 'remote_named_cargo test' <<<"${test_body}"; then
      echo "test-test-remote-is-force-remote-nix: test-remote must invoke remote_named_cargo test:" >&2
      echo "${test_body}" >&2
      exit 1
    fi
    if ! grep -q 'remote_named_cargo' <<<"${cargo_body}"; then
      echo "test-test-remote-is-force-remote-nix: cargo-remote must invoke remote_named_cargo:" >&2
      echo "${cargo_body}" >&2
      exit 1
    fi
    if grep -qE '^[[:space:]]*just[[:space:]]+(ci|test|cargo-ci)([[:space:]]|$)' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: named remote cargo must not run host just ci/test/cargo-ci:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if ! grep -q 'GROK_NIX_FORCE_REMOTE=1' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: remote_named_cargo must set GROK_NIX_FORCE_REMOTE=1:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if ! grep -q 'require_remote_builder' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: remote_named_cargo must invoke require_remote_builder:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if ! grep -q -- '--impure' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: nix build must pass --impure so builtins.getEnv sees the filter:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if grep -qE '(^|[^"'\''])\.#workspace-cargo-named-test' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: must quote .#workspace-cargo-named-test (unquoted # is a bash comment):" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if ! grep -qE '["'\'']\.#workspace-cargo-named-test["'\'']' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: must nix build the quoted .#workspace-cargo-named-test attr:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if ! grep -q 'GROK_REMOTE_TEST_ARGS' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: must export GROK_REMOTE_TEST_ARGS for flake getEnv:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if ! grep -q 'GROK_REMOTE_CARGO_KIND' <<<"${named_body}"; then
      echo "test-test-remote-is-force-remote-nix: must export GROK_REMOTE_CARGO_KIND:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    if grep -qE 'test-remote|cargo-remote|workspace-cargo-named-test' "${gha}"; then
      echo "test-test-remote-is-force-remote-nix: GitHub Actions must not call test-remote, cargo-remote, or workspace-cargo-named-test" >&2
      exit 1
    fi
    if ! grep -q 'workspace-cargo-named-test' "${flake}"; then
      echo "test-test-remote-is-force-remote-nix: flake.nix must define workspace-cargo-named-test" >&2
      exit 1
    fi
    echo "test-test-remote-is-force-remote-nix: ok (force-remote nix build --impure .#workspace-cargo-named-test; GHA does not call it)"

# Named remote tests must actually execute (cargo test / nextest run), not
# compile-only. Does not realize rustc. Does not weaken check-remote smokes.
test-test-remote-runs-tests-not-no-run:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    justfile="${root}/justfile"
    flake="${root}/flake.nix"
    named="$(awk '
      $0 ~ /workspace-cargo-named-test = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /ciLowMemEnv/ { exit }
      p { print }
    ' "${flake}")"
    if [[ -z "${named}" ]]; then
      echo "test-test-remote-runs-tests-not-no-run: expected workspace-cargo-named-test in flake.nix" >&2
      exit 1
    fi
    if ! grep -qE 'cargo test --locked' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: named-test must run cargo test --locked (tests execute):" >&2
      echo "${named}" >&2
      exit 1
    fi
    if ! grep -qE 'nextest run --locked' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: named-test must be able to run cargo nextest run --locked:" >&2
      echo "${named}" >&2
      exit 1
    fi
    if ! grep -q 'clippy-driver' <<<"${named}" || ! grep -qE 'cargo build' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: named-test must support clippy-driver lint and cargo build kinds:" >&2
      echo "${named}" >&2
      exit 1
    fi
    if grep -q -- '--no-run' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: workspace-cargo-named-test must not be compile-only (no cargo --no-run):" >&2
      echo "${named}" >&2
      exit 1
    fi
    if ! grep -q 'preferLocalBuild = false' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: named-test must set preferLocalBuild = false:" >&2
      echo "${named}" >&2
      exit 1
    fi
    if ! grep -q 'surmount-remote' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: named-test must require surmount-remote:" >&2
      echo "${named}" >&2
      exit 1
    fi
    if ! grep -q 'builtins.getEnv "GROK_REMOTE_TEST_ARGS"' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: named-test must read GROK_REMOTE_TEST_ARGS via builtins.getEnv:" >&2
      echo "${named}" >&2
      exit 1
    fi
    if ! grep -q 'workspaceCargoArtifacts' <<<"${named}"; then
      echo "test-test-remote-runs-tests-not-no-run: named-test must reuse workspaceCargoArtifacts (same remote rustc cache as quality):" >&2
      echo "${named}" >&2
      exit 1
    fi
    named_body="$(awk '
      $0 ~ /^remote_named_cargo / { p=1; next }
      p && (/^\[/ || /^[a-zA-Z0-9_.-]+([ \t]|:)/) { exit }
      p { print }
    ' "${justfile}")"
    if ! grep -q -- '--no-run' <<<"${named_body}"; then
      echo "test-test-remote-runs-tests-not-no-run: just recipe must reject --no-run for test/nextest:" >&2
      echo "${named_body}" >&2
      exit 1
    fi
    echo "test-test-remote-runs-tests-not-no-run: ok (cargo test / nextest run; no --no-run in the derivation)"

# Instantiates the named-test drv only. rustc requires surmount-remote so
# this laptop is not eligible. Does not run test-remote or realize rustc.
test-test-remote-workspace-rustc-not-local-eligible:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    flake="${root}/flake.nix"
    sys="$(bash "${root}/scripts/nix-current-system.sh")"
    if ! grep -A30 'workspace-cargo-named-test = craneLib.mkCargoDerivation' "${flake}" | grep -q 'surmount-remote'; then
      echo "test-test-remote-workspace-rustc-not-local-eligible: workspace-cargo-named-test must require surmount-remote" >&2
      exit 1
    fi
    feats="$(nix eval ".#packages.${sys}.workspace-cargo-named-test.requiredSystemFeatures")"
    if ! grep -q 'surmount-remote' <<<"${feats}" || ! grep -q 'big-parallel' <<<"${feats}"; then
      echo "test-test-remote-workspace-rustc-not-local-eligible: expected requiredSystemFeatures to include big-parallel and surmount-remote, got ${feats}" >&2
      exit 1
    fi
    prefer="$(nix eval ".#packages.${sys}.workspace-cargo-named-test.preferLocalBuild")"
    if [[ "${prefer}" != "false" ]]; then
      echo "test-test-remote-workspace-rustc-not-local-eligible: expected preferLocalBuild=false, got ${prefer}" >&2
      exit 1
    fi
    echo "test-test-remote-workspace-rustc-not-local-eligible: ok (named-test rustc requires surmount-remote; preferLocalBuild=false)"

# A missing filter or --no-run must exit 2 before require_remote_builder /
# Nix. Does not SSH and does not realize rustc.
test-test-remote-requires-filter:
    #!/usr/bin/env bash
    set -euo pipefail
    set +e
    empty_out="$(just test-remote 2>&1)"
    empty_status=$?
    set -e
    if [[ "${empty_status}" -ne 2 ]]; then
      echo "test-test-remote-requires-filter: just test-remote with no filter expected exit 2, got ${empty_status}" >&2
      echo "${empty_out}" >&2
      exit 1
    fi
    if ! grep -qE 'filter|test-remote' <<<"${empty_out}"; then
      echo "test-test-remote-requires-filter: empty-filter message must name the missing filter:" >&2
      echo "${empty_out}" >&2
      exit 1
    fi
    if grep -qE 'nix attempt|nix build' <<<"${empty_out}"; then
      echo "test-test-remote-requires-filter: empty filter must not start nix build:" >&2
      echo "${empty_out}" >&2
      exit 1
    fi
    set +e
    norun_out="$(just test-remote --no-run 2>&1)"
    norun_status=$?
    set -e
    if [[ "${norun_status}" -ne 2 ]]; then
      echo "test-test-remote-requires-filter: just test-remote --no-run expected exit 2, got ${norun_status}" >&2
      echo "${norun_out}" >&2
      exit 1
    fi
    if ! grep -q -- '--no-run' <<<"${norun_out}"; then
      echo "test-test-remote-requires-filter: --no-run reject message must name --no-run:" >&2
      echo "${norun_out}" >&2
      exit 1
    fi
    if grep -qE 'nix attempt|nix build' <<<"${norun_out}"; then
      echo "test-test-remote-requires-filter: --no-run must not start nix build:" >&2
      echo "${norun_out}" >&2
      exit 1
    fi
    set +e
    kind_out="$(just cargo-remote not-a-kind -p xai-grok-pager 2>&1)"
    kind_status=$?
    set -e
    if [[ "${kind_status}" -ne 2 ]]; then
      echo "test-test-remote-requires-filter: bad cargo-remote kind expected exit 2, got ${kind_status}" >&2
      echo "${kind_out}" >&2
      exit 1
    fi
    echo "test-test-remote-requires-filter: ok (empty filter and --no-run exit 2 before nix)"
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

# Hard SSH / no-remote-machine text is not a flake 502/503. nix_retry must
# exit on attempt 1 with that status and must not sleep 5s/15s/45s.
# Unclassified failures still retry (see test-nix-retry-smoke).
test-nix-retry-hard-remote-miss-fail-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=4
    run_case() {
      local label="$1"
      local needle="$2"
      local start elapsed status out
      start="$(date +%s)"
      set +e
      out="$(timeout 8 just nix_retry sh -c "printf '%s\\n' '${needle}'; exit 19" 2>&1)"
      status=$?
      set -e
      elapsed="$(($(date +%s) - start))"
      if [[ "${status}" -eq 124 ]]; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: still running after 8s; fail-fast should exit on attempt 1 (retries sleep 65s+)." >&2
        echo "${out}" >&2
        exit 1
      fi
      if [[ "${status}" -ne 19 ]]; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: expected exit 19, got ${status}" >&2
        echo "${out}" >&2
        exit 1
      fi
      if grep -q 'retrying in' <<<"${out}"; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: must not sleep or retry a hard remote miss:" >&2
        echo "${out}" >&2
        exit 1
      fi
      if grep -qE 'attempt 2/|FAILED after [2-9]' <<<"${out}"; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: must stop on attempt 1:" >&2
        echo "${out}" >&2
        exit 1
      fi
      if ! grep -q 'attempt 1/4' <<<"${out}"; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: expected attempt 1/4:" >&2
        echo "${out}" >&2
        exit 1
      fi
      if [[ "${elapsed}" -ge 8 ]]; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: took ${elapsed}s; fail-fast should finish in a few seconds." >&2
        exit 1
      fi
      if ! grep -q 'SSH did not start' <<<"${out}"; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: expected an operator sentence that SSH did not start:" >&2
        echo "${out}" >&2
        exit 1
      fi
      if ! grep -q 'rustc was not run locally' <<<"${out}"; then
        echo "test-nix-retry-hard-remote-miss-fail-fast: ${label}: expected an operator sentence that rustc was not run locally:" >&2
        echo "${out}" >&2
        exit 1
      fi
    }
    run_case ssh 'failed to start SSH connection'
    run_case no-machine 'Failed to find a machine for remote build'
    echo "test-nix-retry-hard-remote-miss-fail-fast: ok (both hard-remote strings exited on attempt 1)"

# The remote nix-daemon scheduled the drv (client machines file advertises
# surmount-remote) then refused it: missing system features. That is not a
# flake 502/503. nix_retry must exit on attempt 1 and must not sleep
# 5s/15s/45s. Does not run check-remote or realize a derivation.
test-nix-retry-missing-system-features-fail-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=4
    needle='error: Cannot build '"'"'workspace-cargo-quality-deps'"'"'. Reason: missing system features Required features: {big-parallel, surmount-remote} Available features: {benchmark, big-parallel, kvm, nixos-test}'
    start="$(date +%s)"
    set +e
    out="$(timeout 8 just nix_retry sh -c "printf '%s\\n' '${needle}'; exit 19" 2>&1)"
    status=$?
    set -e
    elapsed="$(($(date +%s) - start))"
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-missing-system-features-fail-fast: still running after 8s; fail-fast should exit on attempt 1 (retries sleep 65s+)." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 19 ]]; then
      echo "test-nix-retry-missing-system-features-fail-fast: expected exit 19, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -q 'retrying in' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: must not sleep or retry a missing-system-features refuse:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'attempt 2/|FAILED after [2-9]' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: must stop on attempt 1:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'attempt 1/4' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: expected attempt 1/4:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${elapsed}" -ge 8 ]]; then
      echo "test-nix-retry-missing-system-features-fail-fast: took ${elapsed}s; fail-fast should finish in a few seconds." >&2
      exit 1
    fi
    if ! grep -q 'missing system features' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: expected an operator sentence that names missing system features:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'machines file advertises surmount-remote' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: expected the client/machines-file vs remote-daemon mismatch:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -qE 'extra-system-features|nix.conf' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: expected how to add surmount-remote on the builder daemon:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'SSH did not start' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: this class is a daemon feature refuse, not an SSH miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|known_hosts|/id_|machines URI' <<<"${out}"; then
      echo "test-nix-retry-missing-system-features-fail-fast: must not print IP, machines URI, or key paths:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-missing-system-features-fail-fast: ok (missing system features exited on attempt 1)"

# cargo fmt --check / rustfmt prints "Diff in <path>". That is a quality
# fail, not a flake 502/503. nix_retry must exit on attempt 1 and must not
# sleep 5s/15s/45s. Does not run check-remote or realize a derivation.
test-nix-retry-rustfmt-diff-fail-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=4
    needle='Diff in /build/source/crates/xai-grok-tui/src/session.rs:65:'
    start="$(date +%s)"
    set +e
    out="$(timeout 8 just nix_retry sh -c "printf '%s\\n' '${needle}'; exit 19" 2>&1)"
    status=$?
    set -e
    elapsed="$(($(date +%s) - start))"
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: still running after 8s; fail-fast should exit on attempt 1 (retries sleep 65s+)." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 19 ]]; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: expected exit 19, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -q 'retrying in' <<<"${out}"; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: must not sleep or retry a rustfmt / cargo-fmt Diff in fail:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'attempt 2/|FAILED after [2-9]' <<<"${out}"; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: must stop on attempt 1:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'attempt 1/4' <<<"${out}"; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: expected attempt 1/4:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${elapsed}" -ge 8 ]]; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: took ${elapsed}s; fail-fast should finish in a few seconds." >&2
      exit 1
    fi
    if ! grep -qE 'cargo fmt|rustfmt' <<<"${out}"; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: expected an operator sentence that names cargo fmt / rustfmt:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'Diff in' <<<"${out}"; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: expected an operator sentence that names Diff in:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'SSH did not start|missing system features' <<<"${out}"; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: this class is a rustfmt quality fail, not an SSH or daemon-feature miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|known_hosts|/id_|machines URI' <<<"${out}"; then
      echo "test-nix-retry-rustfmt-diff-fail-fast: must not print IP, machines URI, or key paths:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-rustfmt-diff-fail-fast: ok (Diff in rustfmt check exited on attempt 1)"

# cargo clippy -D warnings / rustc prints "error: could not compile". That is
# a quality fail, not a flake 502/503. nix_retry must exit on attempt 1 and
# must not sleep 5s/15s/45s. Does not run check-remote or realize a derivation.
test-nix-retry-clippy-compile-fail-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=4
    needle="error: could not compile \`xai-grok-pager\` (lib) due to 5 previous errors"
    start="$(date +%s)"
    set +e
    out="$(timeout 8 just nix_retry sh -c "printf '%s\\n' '${needle}'; exit 19" 2>&1)"
    status=$?
    set -e
    elapsed="$(($(date +%s) - start))"
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-clippy-compile-fail-fast: still running after 8s; fail-fast should exit on attempt 1 (retries sleep 65s+)." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 19 ]]; then
      echo "test-nix-retry-clippy-compile-fail-fast: expected exit 19, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -q 'retrying in' <<<"${out}"; then
      echo "test-nix-retry-clippy-compile-fail-fast: must not sleep or retry a clippy / rustc could-not-compile fail:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'attempt 2/|FAILED after [2-9]' <<<"${out}"; then
      echo "test-nix-retry-clippy-compile-fail-fast: must stop on attempt 1:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'attempt 1/4' <<<"${out}"; then
      echo "test-nix-retry-clippy-compile-fail-fast: expected attempt 1/4:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${elapsed}" -ge 8 ]]; then
      echo "test-nix-retry-clippy-compile-fail-fast: took ${elapsed}s; fail-fast should finish in a few seconds." >&2
      exit 1
    fi
    if ! grep -qE 'clippy|rustc' <<<"${out}"; then
      echo "test-nix-retry-clippy-compile-fail-fast: expected an operator sentence that names cargo clippy / rustc:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'could not compile' <<<"${out}"; then
      echo "test-nix-retry-clippy-compile-fail-fast: expected an operator sentence that names could not compile:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'SSH did not start|missing system features|Diff in' <<<"${out}"; then
      echo "test-nix-retry-clippy-compile-fail-fast: this class is a clippy/rustc quality fail, not an SSH, daemon-feature, or rustfmt miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|known_hosts|/id_|machines URI' <<<"${out}"; then
      echo "test-nix-retry-clippy-compile-fail-fast: must not print IP, machines URI, or key paths:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-clippy-compile-fail-fast: ok (could not compile exited on attempt 1)"

# GROK_NIX_FORCE_REMOTE must keep the caller command as argv0 (nix, or the
# fake first word this smoke supplies). Copying known_hosts into builders
# field 8 must not `set --` the machines line over "$@". Force-remote
# flags stay after that command as --option builders @temp. Does not run
# check-remote or realize a derivation.
test-nix-retry-force-remote-argv-is-nix:
    #!/usr/bin/env bash
    set -euo pipefail
    justfile="{{ justfile_directory() }}/justfile"
    if awk '
      $0 ~ /^nix_retry([ \t]|:)/ { p=1; next }
      p && /^[A-Za-z0-9_[]/ { exit 1 }
      p && /^[[:space:]]*set --/ { found=1 }
      END { exit !found }
    ' "${justfile}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: nix_retry must not set -- the machines line (that replaces \"\$@\")." >&2
      exit 1
    fi
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT
    machines="${tmpdir}/machines"
    hosts="${tmpdir}/known_hosts"
    argv_dump="${tmpdir}/argv"
    field8_dump="${tmpdir}/field8"
    builders_copy="${tmpdir}/builders-copy"
    fake_nix="${tmpdir}/nix"
    ssh-keygen -q -t ed25519 -N "" -f "${tmpdir}/hostkey" -C smoke
    # known_hosts: hostname type key. Host matches ssh-ng://probe@example.invalid.
    awk '{print "example.invalid", $1, $2}' "${tmpdir}/hostkey.pub" >"${hosts}"
    printf '%s\n' 'ssh-ng://probe@example.invalid x86_64-linux - 1 1 big-parallel,surmount-remote' >"${machines}"
    # Fake nix dumps argv and copies field 8 before nix_retry deletes the temp.
    cat >"${fake_nix}" <<EOS
    #! /usr/bin/env bash
    set -euo pipefail
    printf '%s\n' "\$0" "\$@" >"${argv_dump}"
    prev2=""
    prev1=""
    for a in "\$@"; do
      if [[ "\${prev2}" == "--option" && "\${prev1}" == "builders" && "\${a}" == @* ]]; then
        f="\${a#@}"
        cp "\${f}" "${builders_copy}"
        awk 'NF >= 8 { print \$8; exit }' "\${f}" >"${field8_dump}"
      fi
      prev2="\${prev1}"
      prev1="\${a}"
    done
    EOS
    chmod +x "${fake_nix}"
    export GROK_NIX_FORCE_REMOTE=1
    export GROK_NIX_BUILDERS_FILE="${machines}"
    export GROK_NIX_KNOWN_HOSTS="${hosts}"
    export NIX_RETRY_ATTEMPTS=1
    set +e
    out="$(timeout 8 just nix_retry "${fake_nix}" flake metadata 2>&1)"
    status=$?
    set -e
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: still running after 8s (retries sleep on a 127 recipe miss)." >&2
      echo "${out}" >&2
      exit 1
    fi
    banner="$(awk '/nix attempt / { sub(/^.*nix attempt [0-9]+\/[0-9]+: /, ""); print; exit }' <<<"${out}")"
    first="$(awk '{ print $1; exit }' <<<"${banner}")"
    if [[ -z "${first}" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: missing nix attempt banner:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${first}" == ssh-ng://* ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: first token is the machines-file line, not the nix command (or the fake argv this smoke supplied)." >&2
      echo "force-remote opts must follow the command; do not set -- the builders line over \"\$@\"." >&2
      exit 1
    fi
    if [[ "${first}" != "${fake_nix}" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: expected first token to be the fake nix this smoke supplied." >&2
      echo "got first token: ${first}" >&2
      exit 1
    fi
    if grep -q 'ssh-ng://' <<<"${banner}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: machines-file line must not appear on the nix argv banner (not extra \"\$@\")." >&2
      exit 1
    fi
    if ! grep -qE -- '--option[[:space:]]+builders[[:space:]]+@' <<<"${banner}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: expected --option builders @temp on the force-remote argv:" >&2
      echo "${banner}" >&2
      exit 1
    fi
    if [[ ! -s "${field8_dump}" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: dummy command must see --option builders @temp and copy field 8 (ssh public host key)." >&2
      exit 1
    fi
    field8="$(tr -d '[:space:]' <"${field8_dump}")"
    if [[ -z "${field8}" || "${field8}" == "-" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: builders field 8 (ssh public host key) must be present on the temp machines line." >&2
      exit 1
    fi
    if [[ ! -s "${builders_copy}" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: expected a copy of the temp builders file passed as --option builders @file." >&2
      exit 1
    fi
    if ! awk 'NF >= 8 && $8 != "-" { found=1 } END { exit !found }' "${builders_copy}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: copied builders line must still have field 8." >&2
      exit 1
    fi
    if [[ ! -s "${argv_dump}" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: dummy nix did not run (argv0 was not the command)." >&2
      echo "${out}" >&2
      exit 1
    fi
    exec_first="$(head -n1 "${argv_dump}")"
    if [[ "${exec_first}" != "${fake_nix}" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: executed argv0 must be the supplied nix, not the machines-file line." >&2
      exit 1
    fi
    if grep -q 'ssh-ng://' "${argv_dump}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: machines-file line must not be prepended onto executed \"\$@\"." >&2
      exit 1
    fi
    if ! grep -qx -- '--option' "${argv_dump}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: executed argv must still append --option builders @file after the command." >&2
      exit 1
    fi
    if [[ "${status}" -ne 0 ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: expected the dummy command to exit 0, got ${status}:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-force-remote-argv-is-nix: ok (argv0 is the command; builders @temp; field 8 present)"

# Force-remote must append ssh-ng URI query max-connections (copy slots).
# Default Nix is 1 (serial NAR copy). Default here is 8; GROK_NIX_SSH_NG_MAX_CONNECTIONS
# overrides. Does not print the builders URI or host. Does not run check-remote.
test-nix-retry-force-remote-ssh-ng-max-connections:
    #!/usr/bin/env bash
    set -euo pipefail
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT
    machines="${tmpdir}/machines"
    hosts="${tmpdir}/known_hosts"
    builders_copy="${tmpdir}/builders-copy"
    fake_nix="${tmpdir}/nix"
    ssh-keygen -q -t ed25519 -N "" -f "${tmpdir}/hostkey" -C smoke
    awk '{print "example.invalid", $1, $2}' "${tmpdir}/hostkey.pub" >"${hosts}"
    printf '%s\n' 'ssh-ng://probe@example.invalid x86_64-linux - 1 1 big-parallel,surmount-remote' >"${machines}"
    cat >"${fake_nix}" <<EOS
    #! /usr/bin/env bash
    set -euo pipefail
    prev2=""
    prev1=""
    for a in "\$@"; do
      if [[ "\${prev2}" == "--option" && "\${prev1}" == "builders" && "\${a}" == @* ]]; then
        cp "\${a#@}" "${builders_copy}"
      fi
      prev2="\${prev1}"
      prev1="\${a}"
    done
    EOS
    chmod +x "${fake_nix}"
    export GROK_NIX_FORCE_REMOTE=1
    export GROK_NIX_BUILDERS_FILE="${machines}"
    export GROK_NIX_KNOWN_HOSTS="${hosts}"
    export NIX_RETRY_ATTEMPTS=1
    unset GROK_NIX_SSH_NG_MAX_CONNECTIONS || true
    set +e
    out="$(timeout 8 just nix_retry "${fake_nix}" flake metadata 2>&1)"
    status=$?
    set -e
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: still running after 8s." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 0 ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: expected the dummy command to exit 0, got ${status}." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ ! -s "${builders_copy}" ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: dummy command must see --option builders @temp." >&2
      exit 1
    fi
    uri="$(awk '$1 ~ /^ssh-ng:/ { print $1; exit }' "${builders_copy}")"
    if [[ -z "${uri}" ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: expected an ssh-ng URI in field 1 of the temp builders line (do not print the URI)." >&2
      exit 1
    fi
    query="${uri#*\?}"
    if [[ "${uri}" == "${query}" ]] || [[ "${query}" != *max-connections=8* ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: expected max-connections=8 on the temp builders URI when GROK_NIX_SSH_NG_MAX_CONNECTIONS is unset (do not print the URI)." >&2
      exit 1
    fi
    if [[ "${query}" != *max-connections=8* ]] || [[ "${query}" == *max-connections=8[0-9]* ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: expected exactly max-connections=8 (do not print the URI)." >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|example\.invalid' <<<"${out}"; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: force-remote output must not print the machines-file URI." >&2
      exit 1
    fi
    echo "test-nix-retry-force-remote-ssh-ng-max-connections: ok (default max-connections=8; URI not printed)"
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
      check_limits_first multipoll
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

# Token economy multipoll evidence harness (P1 path + P2 free SuperGrok period series).
#
# Runs `grok-oss limits multipoll` (N samples, default sleep 30s between ends to
# meet flat-detector min wall). Writes JSONL + summary under
# `.agents/reports/limits-multipoll-<utc>/` (or --out-dir). Exit 0 when path is
# OK; exit non-zero only on path failure. Free SuperGrok period flat is
# measurement only and does not fail the process.
#
# Env:
#   GROK_OSS_BIN — binary (default: ./target/release/grok-oss, else grok-oss on PATH)
#   LIMITS_MULTIPOLL_SAMPLES — default 2
#   LIMITS_MULTIPOLL_SLEEP_SECS — default 30
#   LIMITS_MULTIPOLL_OUT_DIR — optional override for --out-dir
limits-multipoll *ARGS:
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
        echo "limits-multipoll: no grok-oss binary (build release, just install, or set GROK_OSS_BIN)" >&2
        exit 2
      fi
    fi
    samples="${LIMITS_MULTIPOLL_SAMPLES:-2}"
    sleep_secs="${LIMITS_MULTIPOLL_SLEEP_SECS:-30}"
    extra=()
    if [[ -n "${LIMITS_MULTIPOLL_OUT_DIR:-}" ]]; then
      extra+=(--out-dir "${LIMITS_MULTIPOLL_OUT_DIR}")
    fi
    # shellcheck disable=SC2086
    cd "${root}"
    echo "==> limits-multipoll: ${bin} limits multipoll --samples ${samples} --sleep-secs ${sleep_secs} ${extra[*]:-} {{ ARGS }}"
    exec "${bin}" limits multipoll --samples "${samples}" --sleep-secs "${sleep_secs}" ${extra[@]+"${extra[@]}"} {{ ARGS }}

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

# Path assert, then remind land agents to walk the existing catalog.
# Does not replace just check. Does not run cargo (deleted tests stay silent).
upstream-land-filters *ARGS:
    ./scripts/assert-process-pins.sh {{ ARGS }}
    @echo ""
    @echo "Path assert OK. Next: walk FORK.md Land checklist and"
    @echo "doc/dev/upstream-regression-filters.md Required land inventory."
    @echo "Seven product classes: CLI identity; config is a surface; /spend ingest;"
    @echo "DOGE/chrome paint; dual-auth hop after included SuperGrok period limits are full;"
    @echo "last-session on start; product skills are not a Python runtime."
    @echo "rg each required identifier for a matching fn. Missing fn = land failed."
    @echo "Walk extra neighbors the catalog lists (bubble click, plan present is not Approve,"
    @echo "SHA-aware rebuild, nucleo, from_config cold catalog, pause / Clear finished,"
    @echo "always-three-layer product prompt, user-guide hop / spend-order)."
    @echo "Not a second numbered board."
    @echo "Then run the operator cheat-sheet cargo blocks in that catalog."
    @echo "just check is quality only. Chrome-only is a failed land."

# Read-only recon probe: branch, CHERRY_PICK/MERGE, UU count, onto-ish, next human action
recon-status:
    ./scripts/recon-status.sh

upstream-sync *ARGS:
    ./scripts/sync-upstream.sh {{ ARGS }}
