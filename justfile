# Grok OSS local recipes.
# GHA quality uses the same recipe chain as `just ci` (flake-meta → ci-prep → test),
# not the `just ci` entrypoint itself -- keep this file the source of truth for those recipes.
# Requires: just, nix (with flakes). No bash scripts -- just recipes + nix.
#
# Refresh locks: `just update` (one workspace Cargo.lock, then flake.lock).
# Bare `just` lists recipes (same idea as `just -l` / `just --list`).
# Full Nix local gate (same recipe chain as GHA quality): `just ci`
#   (or `just check`). Host cargo only: `just check-local`.
#   `just test` is fmt/clippy/tests via cargo-ci.
# Closest GHA repro on Linux: CI_LOW_MEM=1 CI_SYSTEM=x86_64-linux just ci
# Under CI_LOW_MEM, cargo-ci scrubs PATH to nix-store bins only (no host
# pw-record/parec/arecord). Interactive `just dev` keeps impure host PATH.
# Optional on this Linux host: `just check-remote` realizes flake metadata
# and the workspace cargo quality derivation (the same gate as `just check` /
# `just test`: fmt, workspace clippy --all-targets, workspace
# nextest, doctests; members include cargo-mem-guard and grok-nix-helper) on
# the existing trusted-user remote builder (default $HOME/.config/nix/machines).
# Named filters: `just test-remote` / `just cargo-remote` realize
# .#workspace-cargo-named-test the same way (force-remote nix, not host
# rustc). rustc must not run on the caller. Those rustc jobs require
# surmount-remote (plus big-parallel). This laptop never auto-detects
# surmount-remote; the ssh-ng machines line must advertise it. --option
# system-features that omit big-parallel does not stop local nixbld: the
# daemon still advertises big-parallel. Force-remote nix passes
# --option max-jobs 0 so this laptop does not build: crates.io FODs,
# static.rust-lang.org toolchain tarballs (the builder ISA, not extra
# cores), and crane vendor unpacks schedule on the remote. Nix 2.4+
# sends preferLocalBuild derivations to remotes when the caller has
# no local job slots (see https://github.com/NixOS/nix/issues/5646
# accessed: 2026-08-23; https://nix.dev/manual/nix/2.18/advanced-topics/distributed-builds
# accessed: 2026-08-23). builders-use-substitutes stays true so the
# VPS fetches those itself. Force-remote also passes --store with the
# machines-file ssh-ng URI and --eval-store auto (Nix 2.34+; this host
# is 2.35) so cargo-package / cargo-src / toolchain paths stay in the
# VPS store. Default nix build realizes into the local store, then
# copies each remote output back over SSH. That is local store close,
# not a builder miss. nix build --no-link skips a local result symlink.
# -L logs still stream as text. This laptop must not substitute those
# NARs from cache.nixos.org either. Force-remote nix passes --cores 64 so
# one workspace rustc can use the builder's cores. The host machines file
# max-jobs should match that width. Force-remote exports NIX_SSHOPTS (this
# account's known_hosts; host-key checks stay on) and copies that host key
# onto the builders line for nix-daemon SSH. Default `just check` /
# `just ci` stay on this machine's Nix path. They do not require the
# remote builder. `just check-local` runs cargo on this host. GitHub
# Actions must not use check-remote, test-remote, or cargo-remote.

set shell := ["bash", "-euo", "pipefail", "-c"]

# `just` with no recipe → list (just 1.x+; same as CLI `just --default-list` / `JUST_DEFAULT_LIST=1`).
set default-list

# Host system for flake check attributes (e.g. x86_64-linux).
# Prefer CI_SYSTEM (GHA sets it). Local default: uname map — do not call `nix`
# at just parse time (a broken host nix would fail every recipe). Recipe-time
# current_system / require_system use the same uname map in this justfile.
# They must not require a prebuilt grok-nix-helper. Top-level backticks
# cannot expand {{ justfile_directory() }}, so keep uname inline here.
# Attribute sinks use {{ system }} only after require_system.
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
# client scheduled the drv) exit on attempt 1 with no sleep. Hard quality
# misses (rustfmt Diff in, clippy/rustc could not compile, cargo --locked
# lockfile mismatch, cargo nextest test run failed) also exit on attempt 1.
# Linker SIGKILL (collect2 `ld returned 137`, 128+9) is builder memory,
# not a rustc type error: rustc wraps it as could-not-compile, but that
# class retries like a flake 502. Real rustc/clippy fails still fail-fast.
# Cargo payloads stay outside nix_retry so a permanent compile fails once.
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

# Resolve grok-nix-helper: GROK_NIX_HELPER, PATH, result/bin, crate target.
# Never cargo/rustc the helper on this laptop. Never nix-build .#grok-nix-helper
# (check-remote preflight and nix_retry / flake-meta are justfile; later
# recipes that still need the binary locate only).
# Does not print NIX_SSHOPTS, tokens, or GROK_HOME.
[private]
grok_nix_helper_bin:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    if [[ -n "${GROK_NIX_HELPER:-}" ]]; then
      if [[ ! -x "${GROK_NIX_HELPER}" ]]; then
        echo "grok-nix-helper: GROK_NIX_HELPER is not executable" >&2
        exit 2
      fi
      printf '%s\n' "${GROK_NIX_HELPER}"
      exit 0
    fi
    if command -v grok-nix-helper >/dev/null 2>&1; then
      command -v grok-nix-helper
      exit 0
    fi
    for p in \
      "${root}/result/bin/grok-nix-helper" \
      "${root}/crates/codegen/grok-nix-helper/target/release/grok-nix-helper" \
      "${root}/crates/codegen/grok-nix-helper/target/debug/grok-nix-helper"
    do
      if [[ -x "${p}" ]]; then
        printf '%s\n' "${p}"
        exit 0
      fi
    done
    echo "grok-nix-helper is not on PATH and GROK_NIX_HELPER is unset." >&2
    exit 2

# Argv trampoline into grok-nix-helper. One remaining shebang so "$@" is the
# subcommand words (a # in a word must not become a bash comment). Assign the
# path first: bash set -e does not stop `exec "$(failing-cmd)"`, which becomes
# `exec: : not found`.
[private]
[positional-arguments]
grok_helper +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    helper="$(just grok_nix_helper_bin)" || exit $?
    if [[ -z "${helper}" ]]; then
      echo "grok-nix-helper: empty path from grok_nix_helper_bin (will not exec an empty string)." >&2
      exit 2
    fi
    exec "${helper}" "$@"

# Print the Nix system triple (CI_SYSTEM or uname map). No host nix call.
# Same map as parse-time `system :=`. Does not require grok-nix-helper.
[private]
current_system:
    #!/usr/bin/env bash
    set -euo pipefail
    sys="${CI_SYSTEM:-}"
    if [[ -z "${sys}" ]]; then
      case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) sys=x86_64-linux ;;
        Linux-aarch64|Linux-arm64) sys=aarch64-linux ;;
        Darwin-x86_64) sys=x86_64-darwin ;;
        Darwin-arm64) sys=aarch64-darwin ;;
        *)
          echo "unsupported $(uname -s)-$(uname -m); set CI_SYSTEM=..." >&2
          exit 1
          ;;
      esac
    fi
    printf '%s\n' "${sys}"

# Fail fast if the host system string is not safe for shell/attr interpolation.
# Same source as `system` (CI_SYSTEM or uname). Never interpolate `{{ system }}`
# into this recipe (single-quote in CI_SYSTEM must not break assignment).
# Recipes that expand `{{ system }}` into nix attr paths depend on this first.
# Does not require a prebuilt grok-nix-helper.
[private]
require_system:
    #!/usr/bin/env bash
    set -euo pipefail
    sys="$(just current_system)"
    # Same interpolation rule as system_safe_for_interpolation:
    # known triples, or two [A-Za-z0-9_]+ tokens with a digit or underscore
    # (so a just recipe name like just-one is not a Nix system).
    if [[ "${sys}" =~ ^(x86_64-linux|aarch64-linux|x86_64-darwin|aarch64-darwin)$ ]]; then
      exit 0
    fi
    if [[ "${sys}" =~ ^[a-zA-Z0-9_]+-[a-zA-Z0-9_]+$ ]] && [[ "${sys}" =~ [0-9_] ]]; then
      exit 0
    fi
    echo "==> invalid CI_SYSTEM / system (refuse shell interpolation): ${sys}" >&2
    echo "    expected a Nix system like x86_64-linux (cpu-os), not a just recipe name" >&2
    exit 2

# Fail loud before `just check-remote` starts Nix or cargo. Justfile / uname
# / SSH preflight: no grok-nix-helper, no nix-build of the helper.
# GROK_NIX_REMOTE_SYSTEM_FEATURES injects the daemon list so tests skip live
# SSH. ssh-keygen on GROK_NIX_KNOWN_HOSTS. Live path: BatchMode to Host
# surmount-1, then nix config show. Require surmount-remote. The helper
# subcommand require-remote-builder stays for remote-named-cargo.
[private]
require_remote_builder:
    #!/usr/bin/env bash
    set -euo pipefail
    file="${GROK_NIX_BUILDERS_FILE:-${HOME:-}/.config/nix/machines}"
    known_hosts="${GROK_NIX_KNOWN_HOSTS:-${HOME:-}/.ssh/known_hosts}"

    ssh_ng_host() {
      local u="$1"
      u="${u#ssh-ng://}"
      u="${u%%\?*}"
      if [[ "${u}" == *@* ]]; then
        u="${u#*@}"
      fi
      u="${u%%/*}"
      if [[ "${u}" == \[* ]]; then
        u="${u#\[}"
        u="${u%%]*}"
      else
        u="${u%%:*}"
      fi
      printf '%s\n' "${u}"
    }

    host_key_present() {
      local host="$1"
      local kh="$2"
      local line t
      [[ -f "${kh}" ]] || return 1
      local text
      text="$(ssh-keygen -F "${host}" -f "${kh}" 2>/dev/null || true)"
      while IFS= read -r line; do
        [[ "${line}" == \#* ]] && continue
        t="$(awk '{print $2}' <<<"${line}")"
        case "${t}" in
          ssh-*) return 0 ;;
        esac
      done <<<"${text}"
      return 1
    }

    lists_surmount_remote() {
      local feats="$1"
      [[ "${feats}" =~ (^|[[:space:],{])surmount-remote($|[[:space:],}]) ]]
    }

    if [[ ! -f "${file}" ]]; then
      echo "The Nix builders file is missing or empty: ${file}." >&2
      echo "just check-remote reuses the trusted-user machines file already named in the user Nix config (override with GROK_NIX_BUILDERS_FILE)." >&2
      echo "Default just check stays local and does not need this file." >&2
      exit 2
    fi
    body="$(<"${file}")"
    if [[ -z "${body//[$'\t\n\r ']/}" ]]; then
      echo "The Nix builders file is missing or empty: ${file}." >&2
      echo "just check-remote reuses the trusted-user machines file already named in the user Nix config (override with GROK_NIX_BUILDERS_FILE)." >&2
      echo "Default just check stays local and does not need this file." >&2
      exit 2
    fi

    if ! grep -q '^ssh-ng://' <<<"${body}"; then
      echo "The Nix builders file ${file} has no ssh-ng:// builder line." >&2
      echo "just check-remote will not fall back to local Nix store builds." >&2
      exit 2
    fi

    while IFS= read -r line || [[ -n "${line}" ]]; do
      case "${line}" in
        ssh-ng://*)
          read -r uri _ <<<"${line}"
          host="$(ssh_ng_host "${uri}")"
          if [[ -z "${host}" ]]; then
            continue
          fi
          if ! host_key_present "${host}" "${known_hosts}"; then
            echo "This account's known_hosts has no host key for the machines-file builder." >&2
            echo "User ssh to Host surmount-1 is not the nix build SSH path (nix-daemon opens ssh-ng)." >&2
            echo "just check-remote sets NIX_SSHOPTS to this account's known_hosts and will not fall back to a local rustc." >&2
            exit 2
          fi
          ;;
      esac
    done <<<"${body}"

    if [[ -n "${GROK_NIX_REMOTE_SYSTEM_FEATURES:-}" ]]; then
      feats="${GROK_NIX_REMOTE_SYSTEM_FEATURES}"
    else
      if ! ssh -o BatchMode=yes -o ConnectTimeout=8 -o StrictHostKeyChecking=yes surmount-1 true >/dev/null 2>&1; then
        echo "SSH BatchMode to Host surmount-1 failed." >&2
        echo "just check-remote requires that existing remote builder and will not fall back to local Nix store builds." >&2
        exit 2
      fi
      if ! nix_cfg="$(ssh -o BatchMode=yes -o ConnectTimeout=8 -o StrictHostKeyChecking=yes surmount-1 'nix config show' 2>/dev/null)"; then
        echo "Could not read the remote builder nix-daemon system-features over SSH BatchMode." >&2
        echo "just check-remote will not start the long quality build until that query works." >&2
        exit 2
      fi
      feats="$(awk '
        /^system-features/ {
          sub(/^system-features[[:space:]]*/, "")
          sub(/^=[[:space:]]*/, "")
          print
          exit
        }' <<<"${nix_cfg}")"
      if [[ -z "${feats}" ]]; then
        echo "The remote builder SSH reply had no system-features line." >&2
        echo "just check-remote will not start the long quality build until the remote nix-daemon reports its feature list." >&2
        exit 2
      fi
    fi

    if ! lists_surmount_remote "${feats}"; then
      echo "The remote nix-daemon does not list surmount-remote in its system-features." >&2
      echo "The client machines file advertises that feature, so Nix will schedule rustc on the remote, then the daemon will refuse: missing system features." >&2
      echo "Add surmount-remote to the builder daemon (NixOS extra-system-features / nix.conf) and restart or switch. just check-remote will not start the long quality build until that feature is present." >&2
      exit 2
    fi

    echo "==> just check-remote: using builders file ${file}"
    echo "==> just check-remote: NIX_SSHOPTS uses this account's known_hosts (host-key checks stay on)"
    echo "==> just check-remote: rustc, clippy, and nextest require the remote builder surmount-remote feature (fallback=false). This laptop does not advertise that feature, so local nixbld cannot take the rustc job."
    echo "==> just check-remote: force-remote nix sets max-jobs 0. This laptop must not build. Fixed-output derivations and toolchain downloads (crates.io, static.rust-lang.org) go to the remote builder. This laptop must not curl those hosts for this gate. The VPS fetches them from the web (builders-use-substitutes)."
    echo "==> just check-remote: force-remote nix uses --store ssh-ng (same machines-file builder) and --eval-store auto. Cargo-package NARs stay on the VPS. This laptop does not download those NARs from the builder, and does not substitute them from cache.nixos.org. -L logs still stream. nix build --no-link skips a local result symlink."
    echo "==> just check-remote: force-remote nix uses --cores 64. Host machines max-jobs should advertise that many jobs on the builder."

# Retry a nix (or other) command. Live body is this recipe: argv exec of
# "$@", fail-fast on quality/SSH, force-remote flags when
# GROK_NIX_FORCE_REMOTE=1. Does not go through grok_helper /
# grok_nix_helper_bin. Missing helper must not fail check-remote /
# flake-meta. Integer-validates NIX_RETRY_ATTEMPTS (default 4). Prints a
# clear banner per attempt. Unclassified failures retry. Hard SSH /
# no-remote-machine / missing-system-features / rustfmt Diff-in / clippy
# could-not-compile / cargo --locked lockfile mismatch / nix fixed-output
# hash mismatch misses exit on attempt 1 (no 5s/15s/45s sleep). Use only
# around store realization / flake eval, never around host cargo compile
# payloads.
#
# Honor NIX_BIN by putting that binary's directory first on PATH. Exec is
# argv ("$@"), never eval. Never set -- the machines-file line over "$@".
[private]
[positional-arguments]
nix_retry +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "$#" -lt 1 ]]; then
      echo "nix_retry: missing command" >&2
      exit 2
    fi
    if [[ "$1" == ssh-ng://* ]]; then
      echo "==> nix_retry: the first argument is a machines-file line, not the nix command. Pass --option builders @file after the command; do not put the machines line in \"$@\"." >&2
      exit 2
    fi

    attempts="${NIX_RETRY_ATTEMPTS:-4}"
    if [[ ! "${attempts}" =~ ^[1-9][0-9]*$ ]]; then
      echo "==> nix_retry: NIX_RETRY_ATTEMPTS must be a positive integer, got: ${attempts}" >&2
      exit 2
    fi

    if [[ -n "${NIX_BIN:-}" ]]; then
      if [[ ! -x "${NIX_BIN}" ]]; then
        echo "nix_retry: NIX_BIN is not executable: ${NIX_BIN}" >&2
        exit 2
      fi
      nix_dir="$(dirname -- "${NIX_BIN}")"
      case ":${PATH}:" in
        *":${nix_dir}:"*) ;;
        *) export PATH="${nix_dir}:${PATH}" ;;
      esac
    fi

    ssh_ng_host() {
      local u="$1"
      u="${u#ssh-ng://}"
      u="${u%%\?*}"
      if [[ "${u}" == *@* ]]; then
        u="${u#*@}"
      fi
      u="${u%%/*}"
      if [[ "${u}" == \[* ]]; then
        u="${u#\[}"
        u="${u%%]*}"
      else
        u="${u%%:*}"
      fi
      printf '%s\n' "${u}"
    }

    host_key_b64() {
      local host="$1"
      local kh="$2"
      local text line typ key ed_typ="" ed_key="" any_typ="" any_key=""
      [[ -f "${kh}" ]] || return 1
      text="$(ssh-keygen -F "${host}" -f "${kh}" 2>/dev/null || true)"
      while IFS= read -r line; do
        [[ "${line}" == \#* ]] && continue
        typ="$(awk '{print $2}' <<<"${line}")"
        key="$(awk '{print $3}' <<<"${line}")"
        case "${typ}" in
          ssh-ed25519)
            if [[ -z "${ed_typ}" ]]; then
              ed_typ="${typ}"
              ed_key="${key}"
            fi
            ;;
          ssh-*)
            if [[ -z "${any_typ}" ]]; then
              any_typ="${typ}"
              any_key="${key}"
            fi
            ;;
        esac
      done <<<"${text}"
      if [[ -n "${ed_typ}" ]]; then
        printf '%s %s' "${ed_typ}" "${ed_key}" | base64 | tr -d '\n'
        return 0
      fi
      if [[ -n "${any_typ}" ]]; then
        printf '%s %s' "${any_typ}" "${any_key}" | base64 | tr -d '\n'
        return 0
      fi
      return 1
    }

    hard_miss() {
      local status="$1"
      local logf="$2"
      if grep -q 'failed to start SSH connection' "${logf}" \
        || grep -q 'Failed to find a machine for remote build' "${logf}"; then
        echo "==> nix_retry: the builder is listed, but SSH did not start. rustc was not run locally. Not retrying this hard remote miss." >&2
        return 0
      fi
      if grep -q 'missing system features' "${logf}"; then
        echo "==> nix_retry: the remote builder refused this derivation: missing system features. The client scheduled it because the machines file advertises surmount-remote. The remote nix-daemon does not list that feature in its system-features. Add surmount-remote to the builder daemon (NixOS extra-system-features / nix.conf) and restart or switch, then retry. Not retrying this hard remote miss." >&2
        return 0
      fi
      if grep -q 'Diff in ' "${logf}"; then
        echo "==> nix_retry: cargo fmt / rustfmt check failed (Diff in). That is a quality fail, not a flake 502/503. Format the listed files and retry. Not retrying this hard quality miss." >&2
        return 0
      fi
      # rustc wraps a SIGKILL'd linker as could-not-compile. 137 is
      # 128+9 SIGKILL (OOM killer). Retry as infra, not a type error.
      if grep -q 'ld returned 137' "${logf}"; then
        return 1
      fi
      if grep -q 'error: could not compile' "${logf}" || grep -q 'clippy::' "${logf}"; then
        echo "==> nix_retry: cargo clippy / rustc quality failed (could not compile). That is a quality fail, not a flake 502/503. Fix the listed errors and retry. Not retrying this hard quality miss." >&2
        return 0
      fi
      if grep -q 'cannot update the lock file' "${logf}" || grep -qF -- '--locked was passed' "${logf}"; then
        echo "==> nix_retry: cargo lockfile / --locked mismatch (cannot update the lock file). That is a quality fail, not a flake 502/503. Format/lock the listed files and retry. Not retrying this hard quality miss." >&2
        return 0
      fi
      if grep -q 'hash mismatch in fixed-output derivation' "${logf}"; then
        echo "==> nix_retry: nix fixed-output hash mismatch. That is a pin miss, not a flake 502/503. Update the listed sha256 and retry. Not retrying this hard quality miss." >&2
        return 0
      fi
      if grep -q 'error: test run failed' "${logf}" || grep -q 'test run failed' "${logf}"; then
        echo "==> nix_retry: cargo nextest / test run failed. That is a quality fail, not a flake 502/503. Fix the listed tests and retry. Not retrying this hard quality miss." >&2
        return 0
      fi
      if [[ "${status}" -eq 127 ]] && grep -q 'ssh-ng://' "${logf}" && grep -q 'No such file or directory' "${logf}"; then
        echo "==> nix_retry: the command was a machines-file line (exit 127). Force-remote builders belong in --option builders @file after nix. Not retrying this hard recipe miss." >&2
        return 0
      fi
      return 1
    }

    extra=()
    builders_temp=""
    log="$(mktemp)"
    cleanup() {
      rm -f "${log}"
      if [[ -n "${builders_temp}" ]]; then
        rm -f "${builders_temp}"
      fi
    }
    trap cleanup EXIT

    if [[ "${GROK_NIX_FORCE_REMOTE:-}" == "1" ]]; then
      file="${GROK_NIX_BUILDERS_FILE:-${HOME:-}/.config/nix/machines}"
      known_hosts="${GROK_NIX_KNOWN_HOSTS:-${HOME:-}/.ssh/known_hosts}"
      max_conn="${GROK_NIX_SSH_NG_MAX_CONNECTIONS:-8}"
      if [[ ! "${max_conn}" =~ ^[1-9][0-9]*$ ]]; then
        echo "==> nix_retry: GROK_NIX_SSH_NG_MAX_CONNECTIONS must be a positive integer, got: ${max_conn}" >&2
        exit 2
      fi
      builders_temp="$(mktemp)"
      chmod 600 "${builders_temp}"
      store_uri=""
      : > "${builders_temp}"
      if [[ -f "${file}" ]]; then
        while IFS= read -r line || [[ -n "${line}" ]]; do
          case "${line}" in
            ssh-ng://*)
              uri="" systems="-" ssh_key="-" max_jobs="-" speed="-" supported="-" mandatory="-" host_key=""
              read -r uri systems ssh_key max_jobs speed supported mandatory host_key _ <<<"${line}" || true
              systems="${systems:--}"
              ssh_key="${ssh_key:--}"
              max_jobs="${max_jobs:--}"
              speed="${speed:--}"
              supported="${supported:--}"
              mandatory="${mandatory:--}"
              if [[ "${uri}" == *max-connections=* ]]; then
                :
              elif [[ "${uri}" == *'?'* ]]; then
                uri="${uri}&max-connections=${max_conn}"
              else
                uri="${uri}?max-connections=${max_conn}"
              fi
              if [[ -z "${host_key}" || "${host_key}" == "-" ]]; then
                host="$(ssh_ng_host "${uri}")"
                if ! host_key="$(host_key_b64 "${host}" "${known_hosts}")"; then
                  echo "==> nix_retry: this account's known_hosts has no host key for the machines-file builder. User ssh to Host surmount-1 is not the nix build SSH path." >&2
                  exit 2
                fi
              fi
              if [[ -z "${store_uri}" ]]; then
                store_uri="${uri}"
              fi
              printf '%s %s %s %s %s %s %s %s\n' \
                "${uri}" "${systems}" "${ssh_key}" "${max_jobs}" "${speed}" "${supported}" "${mandatory}" "${host_key}" \
                >> "${builders_temp}"
              ;;
            *)
              printf '%s\n' "${line}" >> "${builders_temp}"
              ;;
          esac
        done < "${file}"
      fi
      if [[ -z "${store_uri}" || "${store_uri}" != ssh-ng://* ]]; then
        echo "==> nix_retry: GROK_NIX_FORCE_REMOTE needs an ssh-ng:// builder URI in the machines file so nix can use --store on that builder. This laptop must not realize the graph into the local store." >&2
        exit 2
      fi
      extra+=(
        --option builders "@${builders_temp}"
        --option builders-use-substitutes true
        --option fallback false
        --option system-features "kvm nixos-test uid-range"
        --option max-jobs 0
        --cores 64
        --store "${store_uri}"
      )
      # `nix store cat` rejects --eval-store. `nix build` / flake metadata
      # still use auto so cargo-package NARs stay on the VPS.
      if [[ "${2:-}" != "store" ]]; then
        extra+=(--eval-store auto)
      fi
      if [[ "${2:-}" == "build" ]]; then
        extra+=(--no-link)
      fi
      extra_ssh="-o UserKnownHostsFile=${known_hosts} -o StrictHostKeyChecking=yes"
      if [[ -n "${NIX_SSHOPTS:-}" ]]; then
        export NIX_SSHOPTS="${NIX_SSHOPTS} ${extra_ssh}"
      else
        export NIX_SSHOPTS="${extra_ssh}"
      fi
    fi

    banner_extra=()
    if [[ "${#extra[@]}" -gt 0 ]]; then
      skip_store=0
      for opt in "${extra[@]}"; do
        if [[ "${skip_store}" -eq 1 ]]; then
          banner_extra+=("<builder>")
          skip_store=0
          continue
        fi
        if [[ "${opt}" == "--store" ]]; then
          banner_extra+=("${opt}")
          skip_store=1
          continue
        fi
        banner_extra+=("${opt}")
      done
    fi
    cmd_disp=""
    for w in "$@"; do
      if [[ -n "${cmd_disp}" ]]; then
        cmd_disp="${cmd_disp} ${w}"
      else
        cmd_disp="${w}"
      fi
    done
    if [[ "${#banner_extra[@]}" -gt 0 ]]; then
      for w in "${banner_extra[@]}"; do
        cmd_disp="${cmd_disp} ${w}"
      done
    fi

    n=1
    backoff=5
    while true; do
      echo "==> nix attempt ${n}/${attempts}: ${cmd_disp}"
      : > "${log}"
      set +e
      set +o pipefail
      if [[ "${#extra[@]}" -gt 0 ]]; then
        "$@" "${extra[@]}" 2>&1 | tee "${log}"
      else
        "$@" 2>&1 | tee "${log}"
      fi
      status=${PIPESTATUS[0]}
      set -o pipefail
      set -e
      if [[ "${status}" -eq 0 ]]; then
        exit 0
      fi
      if hard_miss "${status}" "${log}"; then
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
# `just check` / `just ci` = full Nix local gate (same recipe chain as GHA
#   quality). Run before you push when Nix on this machine is usable. No
#   pre-commit hook required for this.
# `just check-local` = host cargo only: fmt --all -- --check, then workspace
#   clippy --all-targets --locked with -D warnings, then workspace nextest
#   --locked, then cargo test --doc --workspace --locked. Use this when the
#   VPS is down. It is not an alias of `just check`.
# `just test` = fmt, clippy (-D warnings, workspace --all-targets),
#   workspace nextest, doctests (members include cargo-mem-guard and
#   grok-nix-helper; nextest covers those tests).
# `just test-extra` = local-only extras CI does not run (cross-target clippy,
#   nix_retry smoke).
#
# There is no `ci-quick` or `ci-host` recipe — use `check`/`check-local`/`ci` or `test`.
# Optional `check-remote` sends that same full cargo gate (fmt, workspace
# clippy --all-targets, nextest, doctests) to the remote builder
# (surmount-remote). Named `just test-remote` / `just cargo-remote` send
# a filter (cargo test / nextest / clippy / build / check) the same way.
# Caller max-jobs is 0: FODs and toolchain downloads must not run on this
# laptop. Default `just check` / `just ci` stay on this machine's Nix path.
#
# Free GHA: CI_LOW_MEM=1 so cargo runs under cargo-mem-guard + mold (no pure
# nix monorepo release build — that OOMs on ~16GB runners). Same flag also
# enables store-only PATH scrub in cargo-ci (see recipe comment).
# ---------------------------------------------------------------------------

# Alias: same full gate as `ci` (preferred short name before push).
check: ci

# Host cargo quality: fmt, then workspace clippy with -D warnings, then
# nextest, then doctests. Does not alias just check.
check-local:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> cargo fmt --all -- --check"
    cargo fmt --all -- --check
    echo "==> cargo clippy --workspace --all-targets --locked (-D warnings)"
    cargo clippy --workspace --all-targets --locked -- -D warnings
    link_jobs="${CARGO_LINK_JOBS:-4}"
    if [ "${link_jobs}" -gt 4 ]; then
      link_jobs=4
    fi
    echo "==> cargo nextest run --workspace --locked --build-jobs ${link_jobs}"
    cargo nextest run --workspace --locked --build-jobs "${link_jobs}"
    echo "==> cargo test --doc --workspace --locked"
    cargo test --doc --workspace --locked --jobs "${link_jobs}"
    echo "just check-local passed"

check-remote:
    #!/usr/bin/env bash
    # Optional remote gate: flake metadata plus the same workspace cargo gate as
    # just check (fmt, workspace clippy --all-targets, nextest run,
    # doctests) as a Nix derivation.
    # rustc requires the remote builder's surmount-remote feature.
    # Optional remote gate. Default local check and ci recipes stay on this machine's Nix path.
    set -euo pipefail
    export GROK_NIX_FORCE_REMOTE=1
    just require_system
    just require_remote_builder
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
    # nix_retry banners and `nix build -L` logs go to stdout. Capturing that
    # whole stream as the store path makes `nix store cat` hit ARG_MAX
    # (exit 126, "Argument list too long"). Keep -L on the terminal and
    # take only the quality /nix/store path.
    out="$(just nix_retry nix build -L ".#workspace-cargo-quality" --print-out-paths | tee /dev/stderr | grep -E '^/nix/store/[0-9a-z]+-workspace-cargo-quality-' | tail -n 1 || true)"
    if [ -z "${out}" ]; then
      echo "just check-remote: missing quality store path in nix_retry output" >&2
      exit 2
    fi
    echo "==> just check-remote: quality output ${out}"
    echo "==> just check-remote: quality receipt (printed even when Nix reuses a previous result)"
    just nix_retry nix store cat "${out}/quality-summary.txt"

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
    export GROK_NIX_FORCE_REMOTE=1
    just grok_helper remote-named-cargo -- "$@"

# Named cargo test on the remote builder. Same path as cargo-remote test.
# Example: just test-remote -p xai-grok-pager --lib -- actions::defaults
# That is cargo test --locked (tests execute; not compile-only). Full gate:
# just check-remote.
[positional-arguments]
test-remote *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    export GROK_NIX_FORCE_REMOTE=1
    just grok_helper remote-named-cargo -- test "$@"

# Shared body for test-remote / cargo-remote. Logic is grok-nix-helper
# remote-named-cargo: validate kind/filter (reject --no-run), require_remote_builder,
# GROK_NIX_FORCE_REMOTE=1, GROK_REMOTE_CARGO_KIND, GROK_REMOTE_TEST_ARGS,
# nix build --impure ".#workspace-cargo-named-test".
[private]
[positional-arguments]
remote_named_cargo *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    export GROK_NIX_FORCE_REMOTE=1
    just grok_helper remote-named-cargo -- "$@"

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

# Refresh the one workspace Cargo.lock, then flake.lock. Does not compile.
# Does not run just check-remote.
update:
    @echo "==> cargo update (workspace)"
    cargo update --manifest-path Cargo.toml
    @echo "==> nix flake update"
    nix flake update

# Prove the flake evaluates (cheap; fails fast on lock/input breakage).
# Uses just nix_retry. Does not locate a helper binary.
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
    @echo "==> build .#grok-nix-helper{{ if low_mem == "1" { " (low-mem nix opts)" } else { "" } }}"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#grok-nix-helper
    @echo "==> check .#grok-nix-helper-tests"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.grok-nix-helper-tests"

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
#   after. grok-nix-helper hermetic-path rebuilds PATH as a /nix/store
#   allowlist so optional desktop tools (pw-record, parec, arecord, ...) cannot
#   flip unit tests. Not a recorder denylist. git is in ci-tools for the same
#   reason. Interactive `just dev` / bare cargo keep impure host PATH.
#   Escape hatch: GROK_CI_ALLOW_HOST_PATH=1.
#
# RULES_RUST_RUNFILES_WORKSPACE_NAME: --all-features enables xai-test-utils'
# optional `bazel`/`runfiles` dep (Bazel-only). That crate needs this env at
# compile time; set a dummy so cargo/host gates are not blocked.
[private]
[positional-arguments]
cargo-ci +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    just grok_helper cargo-ci -- "$@"

# Enter the fenix/crane-aligned dev shell (interactive: no retry wrapper).
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    eval "$(just grok_helper ensure-nix-path --export)"
    exec nix develop

# Enter the free-GHA / low-mem host shell (interactive: no retry wrapper).
dev-ci:
    #!/usr/bin/env bash
    set -euo pipefail
    eval "$(just grok_helper ensure-nix-path --export)"
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
# Covers: fmt check, clippy -D warnings (--workspace --all-targets; members
# include cargo-mem-guard and grok-nix-helper), workspace nextest, doctests.
# Workspace nextest runs those member crate tests; no late cargo test
# --manifest-path.
test: test-fmt test-clippy test-unit test-doc
    @echo "just test passed"

# Local-only extras CI does not run. Grep-only justfile/flake contracts live
# in grok-nix-helper `justfile_contracts` (proved by workspace nextest).
# Remaining recipes here actually run nix, cargo, or the helper.
test-extra: test-clippy-targets test-clippy-all-targets test-ensure-working-nix-path test-nix-retry-smoke test-nix-retry-does-not-require-helper-binary test-nix-retry-hard-remote-miss-fail-fast test-nix-retry-missing-system-features-fail-fast test-nix-retry-rustfmt-diff-fail-fast test-nix-retry-clippy-compile-fail-fast test-nix-retry-linker-sigkill-retries test-nix-retry-locked-lockfile-fail-fast test-nix-retry-fixed-output-hash-mismatch-fail-fast test-nix-retry-nextest-fail-fast test-nix-retry-force-remote-argv-is-nix test-nix-retry-force-remote-ssh-ng-max-connections test-check-remote-builders-file-smoke test-check-remote-cargo-is-remote-nix-derivation test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero test-check-remote-uses-builder-cores test-check-remote-clippy-uses-many-workers test-check-remote-deps-omit-git-sha test-check-remote-omits-local-big-parallel test-check-remote-workspace-rustc-not-local-eligible test-check-remote-preflight-same-path-as-nix-ssh test-check-remote-preflight-remote-daemon-features test-test-remote-workspace-rustc-not-local-eligible test-test-remote-requires-filter
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

# Optional named filters. Default `just test` already runs these via
# workspace nextest.
test-mem-guard:
    @echo "==> cargo nextest run -p cargo-mem-guard"
    just cargo-ci cargo nextest run -p cargo-mem-guard --locked

test-grok-nix-helper:
    @echo "==> cargo nextest run -p grok-nix-helper"
    just cargo-ci cargo nextest run -p grok-nix-helper --locked

# Instantiated quality buildPhase still needs host nix eval. Source greps
# live in grok-nix-helper justfile_contracts. Does not realize rustc.
test-clippy-all-targets:
    #!/usr/bin/env bash
    set -euo pipefail
    sys="$(just current_system)"
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
    echo "test-clippy-all-targets: ok (instantiated quality buildPhase cargo check --all-targets)"

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
    if grep -qE 'realizing \.#grok-nix-helper|\.#grok-nix-helper' <<<"${out}"; then
      echo "test-check-remote-builders-file-smoke: check-remote must not nix-build grok-nix-helper:" >&2
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
# `just ci` (`just test`: fmt, clippy, workspace nextest, doctests)
# through a remote Nix derivation (builders file via
# nix_retry, rustc requires big-parallel). Tests must actually run, not
# compile-only `--no-run`. Does not realize that derivation. Default
# `just ci` must still be local host cargo. GHA must not call check-remote.
# Instantiated quality attr. Source greps live in grok-nix-helper
# justfile_contracts. Does not run check-remote or realize rustc.
test-check-remote-cargo-is-remote-nix-derivation:
    #!/usr/bin/env bash
    set -euo pipefail
    sys="$(just current_system)"
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

# Force-remote must set caller max-jobs 0 so crates.io FODs, toolchain
# tarballs, and crane vendor unpacks cannot run on this laptop. Nix 2.4+
# overrides preferLocalBuild when the caller has no local job slots
# (https://github.com/NixOS/nix/issues/5646 accessed: 2026-08-23).
# rustc still requires surmount-remote + big-parallel. Instantiated attr
# only; source greps live in grok-nix-helper justfile_contracts and
# force_remote.rs. Does not run check-remote or realize rustc.
test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero:
    #!/usr/bin/env bash
    set -euo pipefail
    sys="$(just current_system)"
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
    echo "test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero: ok (caller max-jobs 0; rustc requires big-parallel)"

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
    sys="$(just current_system)"
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
    sys="$(just current_system)"
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
    sys="$(just current_system)"
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
# big-parallel (and benchmark) on the caller. Caller max-jobs 0 is required
# so FODs cannot take a local slot either. Does not realize the workspace
# rustc derivation.
test-check-remote-omits-local-big-parallel:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    helper_src="${root}/crates/codegen/grok-nix-helper/src/force_remote.rs"
    if ! grep -q '"max-jobs"' "${helper_src}" || ! grep -A2 '"max-jobs"' "${helper_src}" | grep -q '"0"'; then
      echo "test-check-remote-omits-local-big-parallel: grok-nix-helper force_remote_nix_args must pass --option max-jobs 0." >&2
      exit 1
    fi
    if ! grep -q '"system-features"' "${helper_src}"; then
      echo "test-check-remote-omits-local-big-parallel: grok-nix-helper force_remote_nix_args must pass --option system-features that omit big-parallel so this host cannot claim workspace rustc." >&2
      echo "This host's nix show-config advertises big-parallel. requiredSystemFeatures alone is not enough." >&2
      exit 1
    fi
    feats="$(sed -n 's/.*"\(kvm nixos-test uid-range\)".*/\1/p' "${helper_src}" | head -1)"
    if [[ -z "${feats}" ]]; then
      echo "test-check-remote-omits-local-big-parallel: could not parse the force-remote system-features list from force_remote.rs." >&2
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
    sys="$(just current_system)"
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
    sys="$(just current_system)"
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

# require_remote_builder is on check-remote. User SSH to Host surmount-1
# is not the nix build path. An empty known_hosts (or missing NIX_SSHOPTS
# host-key file) must fail even when a dummy ssh-ng machines file exists.
# Does not run check-remote or realize quality.
test-check-remote-preflight-same-path-as-nix-ssh:
    #!/usr/bin/env bash
    set -euo pipefail
    # Source contracts live in grok-nix-helper require_remote_builder.rs
    # and justfile_contracts. This recipe is the runtime probe only.
    # Do not grep the require_remote_builder trampoline body.
    if ! grep -qE 'just require_remote_builder' "{{ justfile_directory() }}/justfile"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: check-remote must invoke require_remote_builder:" >&2
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
    if grep -qE 'realizing \.#grok-nix-helper|\.#grok-nix-helper' <<<"${out}"; then
      echo "test-check-remote-preflight-same-path-as-nix-ssh: require_remote_builder must not nix-build grok-nix-helper:" >&2
      echo "${out}" >&2
      exit 1
    fi
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
    # Source contracts live in grok-nix-helper require_remote_builder.rs
    # and justfile_contracts. This recipe is the runtime probe only.
    # Do not grep the require_remote_builder trampoline body.
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
    if grep -qE 'realizing \.#grok-nix-helper|\.#grok-nix-helper' <<<"${miss_out}"; then
      echo "test-check-remote-preflight-remote-daemon-features: require_remote_builder must not nix-build grok-nix-helper:" >&2
      echo "${miss_out}" >&2
      exit 1
    fi
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
    if grep -qE 'realizing \.#grok-nix-helper|\.#grok-nix-helper' <<<"${ok_out}"; then
      echo "test-check-remote-preflight-remote-daemon-features: require_remote_builder must not nix-build grok-nix-helper:" >&2
      echo "${ok_out}" >&2
      exit 1
    fi
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

# Instantiates the named-test drv only. rustc requires surmount-remote so
# this laptop is not eligible. Does not run test-remote or realize rustc.
test-test-remote-workspace-rustc-not-local-eligible:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    flake="${root}/flake.nix"
    sys="$(just current_system)"
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
# Probe must find a real nix (PATH store copy, or distro package after GC).
# A /bin/true named nix is not working. NIX_BIN still honors a stub (smokes).
test-ensure-working-nix-path:
    #!/usr/bin/env bash
    set -euo pipefail
    helper="$(just grok_nix_helper_bin)"
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT
    fake="${tmpdir}/nix"
    cp /bin/true "${fake}"
    chmod +x "${fake}"
    set +e
    out="$(env -u NIX_BIN PATH="${tmpdir}:/usr/bin:/bin" "${helper}" ensure-nix-path 2>&1)"
    status=$?
    set -e
    if [[ "${status}" -ne 0 ]]; then
      echo "test-ensure-working-nix-path: expected a working host nix after a /bin/true PATH stub, got exit ${status}:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'Nix' <<<"${out}"; then
      echo "test-ensure-working-nix-path: expected nix --version to print Nix:" >&2
      echo "${out}" >&2
      exit 1
    fi
    picked="$(printf '%s\n' "${out}" | awk '/^\// { print; exit }')"
    if [[ -z "${picked}" ]]; then
      echo "test-ensure-working-nix-path: expected an absolute nix path in output:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${picked}" == "${fake}" ]]; then
      echo "test-ensure-working-nix-path: must not accept a /bin/true named nix:" >&2
      echo "${out}" >&2
      exit 1
    fi
    stub="${tmpdir}/stub-bin/nix"
    mkdir -p "${tmpdir}/stub-bin"
    printf '%s\n' '#!/bin/sh' "echo 'nix (Nix) stub'" >"${stub}"
    chmod +x "${stub}"
    set +e
    honor_out="$(NIX_BIN="${stub}" "${helper}" ensure-nix-path 2>&1)"
    honor_status=$?
    set -e
    if [[ "${honor_status}" -ne 0 ]]; then
      echo "test-ensure-working-nix-path: NIX_BIN stub must be honored, got exit ${honor_status}:" >&2
      echo "${honor_out}" >&2
      exit 1
    fi
    echo "test-ensure-working-nix-path: ok (host nix after GC skip; /bin/true rejected; NIX_BIN stub honored)"

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

# check-remote / flake-meta / nix_retry must not fail because the helper
# binary is missing. Does not run nix flake metadata.
test-nix-retry-does-not-require-helper-binary:
    #!/usr/bin/env bash
    set -euo pipefail
    unset GROK_NIX_HELPER || true
    unset GROK_NIX_FORCE_REMOTE || true
    export NIX_RETRY_ATTEMPTS=1
    set +e
    out="$(env -u GROK_NIX_HELPER -u GROK_NIX_FORCE_REMOTE NIX_RETRY_ATTEMPTS=1 just nix_retry true 2>&1)"
    status=$?
    set -e
    if grep -q 'grok-nix-helper is not on PATH' <<<"${out}"; then
      echo "test-nix-retry-does-not-require-helper-binary: nix_retry must not locate a helper binary:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 0 ]]; then
      echo "test-nix-retry-does-not-require-helper-binary: expected just nix_retry true to exit 0, got ${status}:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-does-not-require-helper-binary: ok (true succeeded without locating a helper binary)"

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

# rustc wraps a SIGKILL'd linker as "error: could not compile". That is
# builder memory (exit 137 = 128+9 SIGKILL), not a rustc type error.
# nix_retry must retry like a flake 502, not fail-fast as clippy/rustc.
# A log that has only "could not compile" (no ld returned 137) still
# fail-fasts (test-nix-retry-clippy-compile-fail-fast). Does not run
# check-remote or realize a derivation.
test-nix-retry-linker-sigkill-retries:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=2
    set +e
    out="$(timeout 20 just nix_retry sh -c "printf '%s\\n' 'collect2: error: ld returned 137 exit status' 'error: could not compile \`xai-grok-shell\` (test test_leader_death_repro) due to 1 previous error'; exit 19" 2>&1)"
    status=$?
    set -e
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-linker-sigkill-retries: still running after 20s; two attempts plus 5s backoff should finish sooner." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 19 ]]; then
      echo "test-nix-retry-linker-sigkill-retries: expected exit 19 after retries, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'retrying in' <<<"${out}"; then
      echo "test-nix-retry-linker-sigkill-retries: linker SIGKILL must retry, not fail-fast as could-not-compile:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -qE 'attempt 2/2|2 attempt' <<<"${out}"; then
      echo "test-nix-retry-linker-sigkill-retries: expected attempt 2/2:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -q 'clippy / rustc quality failed' <<<"${out}"; then
      echo "test-nix-retry-linker-sigkill-retries: must not classify linker SIGKILL as a rustc type error:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'SSH did not start|missing system features|Diff in' <<<"${out}"; then
      echo "test-nix-retry-linker-sigkill-retries: this class is retryable builder memory, not an SSH, daemon-feature, or rustfmt miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-linker-sigkill-retries: ok (ld returned 137 retried, not fail-fast compile)"

# cargo --locked prints "cannot update the lock file" / "--locked was passed"
# when Cargo.toml is ahead of Cargo.lock. That is a quality fail, not a
# flake 502/503. nix_retry must exit on attempt 1 and must not sleep
# 5s/15s/45s. Does not run check-remote or realize a derivation.
test-nix-retry-locked-lockfile-fail-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=4
    needle='error: cannot update the lock file /build/source/Cargo.lock because --locked was passed to prevent this'
    start="$(date +%s)"
    set +e
    out="$(timeout 8 just nix_retry sh -c "printf '%s\\n' '${needle}'; exit 19" 2>&1)"
    status=$?
    set -e
    elapsed="$(($(date +%s) - start))"
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-locked-lockfile-fail-fast: still running after 8s; fail-fast should exit on attempt 1 (retries sleep 65s+)." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 19 ]]; then
      echo "test-nix-retry-locked-lockfile-fail-fast: expected exit 19, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -q 'retrying in' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: must not sleep or retry a cargo --locked lockfile mismatch:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'attempt 2/|FAILED after [2-9]' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: must stop on attempt 1:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'attempt 1/4' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: expected attempt 1/4:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${elapsed}" -ge 8 ]]; then
      echo "test-nix-retry-locked-lockfile-fail-fast: took ${elapsed}s; fail-fast should finish in a few seconds." >&2
      exit 1
    fi
    if ! grep -qE 'lockfile|--locked' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: expected an operator sentence that names lockfile / --locked:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'cannot update the lock file' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: expected an operator sentence that names cannot update the lock file:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -qE 'Format/lock|quality fail' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: expected an operator sentence to format/lock the listed files:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'SSH did not start|missing system features|Diff in|could not compile' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: this class is a lockfile quality fail, not an SSH, daemon-feature, rustfmt, or clippy miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|known_hosts|/id_|machines URI' <<<"${out}"; then
      echo "test-nix-retry-locked-lockfile-fail-fast: must not print IP, machines URI, or key paths:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-locked-lockfile-fail-fast: ok (cannot update the lock file exited on attempt 1)"

# Nix prints "hash mismatch in fixed-output derivation" when a FOD sha256 pin
# is stale (for example channel-rust-stable.toml after a rust-stable
# publish). That is a pin miss, not a flake 502/503. nix_retry must exit on
# attempt 1 and must not sleep 5s/15s/45s. Does not run check-remote or
# realize a derivation.
test-nix-retry-fixed-output-hash-mismatch-fail-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=4
    needle="error: hash mismatch in fixed-output derivation '/nix/store/sz7d1n6cbqwc77lvmlqy6fzgpikphz5x-channel-rust-stable.toml.drv':"
    start="$(date +%s)"
    set +e
    out="$(timeout 8 just nix_retry sh -c "printf '%s\\n' '${needle}'; exit 19" 2>&1)"
    status=$?
    set -e
    elapsed="$(($(date +%s) - start))"
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: still running after 8s; fail-fast should exit on attempt 1 (retries sleep 65s+)." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 19 ]]; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: expected exit 19, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -q 'retrying in' <<<"${out}"; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: must not sleep or retry a nix fixed-output hash mismatch:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'attempt 2/|FAILED after [2-9]' <<<"${out}"; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: must stop on attempt 1:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'attempt 1/4' <<<"${out}"; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: expected attempt 1/4:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${elapsed}" -ge 8 ]]; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: took ${elapsed}s; fail-fast should finish in a few seconds." >&2
      exit 1
    fi
    if ! grep -qE 'fixed-output hash mismatch|pin miss' <<<"${out}"; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: expected an operator sentence that names fixed-output hash mismatch / pin miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'sha256' <<<"${out}"; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: expected an operator sentence to update the listed sha256:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'SSH did not start|missing system features|Diff in|could not compile|cannot update the lock file' <<<"${out}"; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: this class is a nix FOD pin miss, not an SSH, daemon-feature, rustfmt, clippy, or lockfile miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|known_hosts|/id_|machines URI' <<<"${out}"; then
      echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: must not print IP, machines URI, or key paths:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-fixed-output-hash-mismatch-fail-fast: ok (hash mismatch in fixed-output derivation exited on attempt 1)"

# cargo nextest prints "error: test run failed" when the suite is red. That
# is a quality fail, not a flake 502/503. nix_retry must exit on attempt 1
# and must not sleep 5s/15s/45s. Does not run check-remote or realize a
# derivation.
test-nix-retry-nextest-fail-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    export NIX_RETRY_ATTEMPTS=4
    needle='error: test run failed'
    start="$(date +%s)"
    set +e
    out="$(timeout 8 just nix_retry sh -c "printf '%s\\n' '${needle}'; exit 19" 2>&1)"
    status=$?
    set -e
    elapsed="$(($(date +%s) - start))"
    if [[ "${status}" -eq 124 ]]; then
      echo "test-nix-retry-nextest-fail-fast: still running after 8s; fail-fast should exit on attempt 1 (retries sleep 65s+)." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 19 ]]; then
      echo "test-nix-retry-nextest-fail-fast: expected exit 19, got ${status}" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -q 'retrying in' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: must not sleep or retry a cargo nextest / test run failed:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'attempt 2/|FAILED after [2-9]' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: must stop on attempt 1:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'attempt 1/4' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: expected attempt 1/4:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ "${elapsed}" -ge 8 ]]; then
      echo "test-nix-retry-nextest-fail-fast: took ${elapsed}s; fail-fast should finish in a few seconds." >&2
      exit 1
    fi
    if ! grep -qE 'cargo nextest|nextest' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: expected an operator sentence that names cargo nextest:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -q 'test run failed' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: expected an operator sentence that names test run failed:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if ! grep -qE 'Fix the listed tests|quality fail' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: expected an operator sentence to fix the listed tests:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'SSH did not start|missing system features|Diff in|could not compile|cannot update the lock file|fixed-output hash mismatch' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: this class is a nextest quality fail, not an SSH, daemon-feature, rustfmt, clippy, lockfile, or FOD miss:" >&2
      echo "${out}" >&2
      exit 1
    fi
    if grep -qE 'ssh-ng://|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|known_hosts|/id_|machines URI' <<<"${out}"; then
      echo "test-nix-retry-nextest-fail-fast: must not print IP, machines URI, or key paths:" >&2
      echo "${out}" >&2
      exit 1
    fi
    echo "test-nix-retry-nextest-fail-fast: ok (error: test run failed exited on attempt 1)"

# GROK_NIX_FORCE_REMOTE must keep the caller command as argv0 (nix, or the
# fake first word this smoke supplies). Copying known_hosts into builders
# field 8 must not `set --` the machines line over "$@". Force-remote
# flags stay after that command as --option builders @temp and must
# include --option max-jobs 0 (this laptop must not build FODs).
# --store is the machines-file ssh-ng URI and --eval-store auto so the
# quality graph is not realized into the default local store. nix build
# also gets --no-link. Does not run check-remote, realize a derivation,
# or fetch the network.
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
    # Hermetic: nix_retry sources ensure-working-nix-path. Honor this stub so
    # the smoke does not need a host nix or a network fetch.
    export NIX_BIN="${fake_nix}"
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
    if grep -qE 'ssh-ng://probe@|example\.invalid' <<<"${banner}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: machines-file URI must not appear on the nix argv banner (redact --store)." >&2
      echo "${banner}" >&2
      exit 1
    fi
    if ! grep -qE -- '--store[[:space:]]+<builder>' <<<"${banner}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: banner must show redacted --store <builder>:" >&2
      echo "${banner}" >&2
      exit 1
    fi
    if ! grep -qE -- '--eval-store[[:space:]]+auto' <<<"${banner}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: banner must include --eval-store auto:" >&2
      echo "${banner}" >&2
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
    store_next=0
    eval_next=0
    found_store=0
    found_eval=0
    while IFS= read -r tok; do
      if [[ "${store_next}" -eq 1 ]]; then
        if [[ "${tok}" != ssh-ng://* ]]; then
          echo "test-nix-retry-force-remote-argv-is-nix: --store value must be the machines-file ssh-ng URI (do not print it)." >&2
          exit 1
        fi
        if [[ "${tok}" != ssh-ng://probe@example.invalid* ]]; then
          echo "test-nix-retry-force-remote-argv-is-nix: --store must reuse the machines-file builder URI (same ssh-ng identity)." >&2
          exit 1
        fi
        found_store=1
        store_next=0
        continue
      fi
      if [[ "${eval_next}" -eq 1 ]]; then
        if [[ "${tok}" != "auto" ]]; then
          echo "test-nix-retry-force-remote-argv-is-nix: --eval-store value must be auto, got: ${tok}" >&2
          exit 1
        fi
        found_eval=1
        eval_next=0
        continue
      fi
      if [[ "${tok}" == "--store" ]]; then
        store_next=1
        continue
      fi
      if [[ "${tok}" == "--eval-store" ]]; then
        eval_next=1
        continue
      fi
      if [[ "${tok}" == ssh-ng://* ]]; then
        echo "test-nix-retry-force-remote-argv-is-nix: ssh-ng URI must only appear as the --store value, not as extra argv." >&2
        exit 1
      fi
    done < "${argv_dump}"
    if [[ "${found_store}" -ne 1 ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: executed argv must include --store ssh-ng (not the default local store)." >&2
      cat "${argv_dump}" >&2
      exit 1
    fi
    if [[ "${found_eval}" -ne 1 ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: executed argv must include --eval-store auto." >&2
      cat "${argv_dump}" >&2
      exit 1
    fi
    if grep -qx -- '--no-link' "${argv_dump}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: flake metadata must not pass --no-link (that flag is nix build only)." >&2
      cat "${argv_dump}" >&2
      exit 1
    fi
    if ! grep -qx -- '--option' "${argv_dump}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: executed argv must still append --option builders @file after the command." >&2
      exit 1
    fi
    if ! awk '
      $0 == "--option" { o=1; next }
      o && $0 == "max-jobs" { m=1; next }
      m && $0 == "0" { found=1; exit }
      { o=0; m=0 }
      END { exit !found }
    ' "${argv_dump}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: executed argv must include --option max-jobs 0 so this laptop does not build FODs." >&2
      cat "${argv_dump}" >&2
      exit 1
    fi
    if ! grep -qE -- '--option[[:space:]]+max-jobs[[:space:]]+0' <<<"${banner}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: nix attempt banner must include --option max-jobs 0:" >&2
      echo "${banner}" >&2
      exit 1
    fi
    if [[ "${status}" -ne 0 ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: expected the dummy command to exit 0, got ${status}:" >&2
      echo "${out}" >&2
      exit 1
    fi
    rm -f "${argv_dump}"
    set +e
    out_build="$(timeout 8 just nix_retry "${fake_nix}" build -L dummy-attr 2>&1)"
    status_build=$?
    set -e
    if [[ "${status_build}" -eq 124 ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: nix build dummy still running after 8s." >&2
      echo "${out_build}" >&2
      exit 1
    fi
    if [[ "${status_build}" -ne 0 ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: expected dummy nix build to exit 0, got ${status_build}:" >&2
      echo "${out_build}" >&2
      exit 1
    fi
    if [[ ! -s "${argv_dump}" ]]; then
      echo "test-nix-retry-force-remote-argv-is-nix: dummy nix build did not dump argv." >&2
      echo "${out_build}" >&2
      exit 1
    fi
    if ! grep -qx -- '--no-link' "${argv_dump}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: nix build must pass --no-link so the result stays on the remote store." >&2
      cat "${argv_dump}" >&2
      exit 1
    fi
    if ! awk '
      $0 == "--store" { s=1; next }
      s && $0 ~ /^ssh-ng:\/\// { found=1; exit }
      { s=0 }
      END { exit !found }
    ' "${argv_dump}"; then
      echo "test-nix-retry-force-remote-argv-is-nix: nix build must still pass --store ssh-ng (not the default local store)." >&2
      cat "${argv_dump}" >&2
      exit 1
    fi
    echo "test-nix-retry-force-remote-argv-is-nix: ok (argv0 is the command; builders @temp; field 8 present; max-jobs 0; --store ssh-ng; --eval-store auto; nix build --no-link)"

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
    store_uri_dump="${tmpdir}/store-uri"
    fake_nix="${tmpdir}/nix"
    ssh-keygen -q -t ed25519 -N "" -f "${tmpdir}/hostkey" -C smoke
    awk '{print "example.invalid", $1, $2}' "${tmpdir}/hostkey.pub" >"${hosts}"
    printf '%s\n' 'ssh-ng://probe@example.invalid x86_64-linux - 1 1 big-parallel,surmount-remote' >"${machines}"
    cat >"${fake_nix}" <<EOS
    #! /usr/bin/env bash
    set -euo pipefail
    prev2=""
    prev1=""
    store_next=0
    for a in "\$@"; do
      if [[ "\${store_next}" -eq 1 ]]; then
        printf '%s\n' "\${a}" >"${store_uri_dump}"
        store_next=0
      fi
      if [[ "\${a}" == "--store" ]]; then
        store_next=1
      fi
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
    export NIX_BIN="${fake_nix}"
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
    if grep -qE 'example\.invalid|probe@' <<<"${out}"; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: force-remote output must not print the machines-file URI." >&2
      echo "${out}" >&2
      exit 1
    fi
    if [[ ! -s "${store_uri_dump}" ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: dummy command must see --store ssh-ng." >&2
      exit 1
    fi
    store_tok="$(tr -d '[:space:]' <"${store_uri_dump}")"
    store_query="${store_tok#*\?}"
    if [[ "${store_tok}" != ssh-ng://* ]] || [[ "${store_tok}" == "${store_query}" ]] || [[ "${store_query}" != *max-connections=8* ]]; then
      echo "test-nix-retry-force-remote-ssh-ng-max-connections: --store URI must reuse the machines-file ssh-ng URI including max-connections=8 (do not print the URI)." >&2
      exit 1
    fi
    echo "test-nix-retry-force-remote-ssh-ng-max-connections: ok (default max-connections=8; --store reuses that URI; host not printed)"
# Overrides host -fuse-ld=wild (breaks this link). See comments in recipe body.
# Strips the installed artifact only: [profile.release] stays unstripped for
# local debugging; release-dist keeps strip=false for sidecar extract.
install:
    # Host ~/.cargo/config and RUSTFLAGS often set -fuse-ld=wild; wild fails
    # this workspace. Unset encoded rustflags and pin mold only.
    mkdir -p "${CARGO_HOME:-$HOME/.cargo}/bin"
    @echo "==> cargo build --release -p xai-grok-pager-bin (no wild linker)"
    env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
      RULES_RUST_RUNFILES_WORKSPACE_NAME="${RULES_RUST_RUNFILES_WORKSPACE_NAME:-grok-oss}" \
      cargo build --release -p xai-grok-pager-bin --locked \
      --config 'build.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]' \
      --config 'target.x86_64-unknown-linux-gnu.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]' \
      --config 'target.aarch64-unknown-linux-gnu.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]'
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
    # Host ~/.cargo/config and RUSTFLAGS often set -fuse-ld=wild; wild fails
    # this workspace. Unset encoded rustflags and pin mold only.
    @echo "==> cargo build --profile release-dist -p xai-grok-pager-bin (no wild linker)"
    env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
      RULES_RUST_RUNFILES_WORKSPACE_NAME="${RULES_RUST_RUNFILES_WORKSPACE_NAME:-grok-oss}" \
      cargo build --profile release-dist -p xai-grok-pager-bin --locked \
      --config 'build.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]' \
      --config 'target.x86_64-unknown-linux-gnu.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]' \
      --config 'target.aarch64-unknown-linux-gnu.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]'
    @echo "==> extract debug sidecar + strip binary"
    just grok_helper extract-debug-sidecar target/release-dist/grok-oss
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
    just grok_helper detect-upstream-export

upstream-import *ARGS:
    just grok_helper import-upstream-export {{ ARGS }}

# Cherry-pick Surmount product onto current xAI tip → onto-xai/<short>
upstream-put-history *ARGS:
    just grok_helper put-history-on-xai {{ ARGS }}

# Join Surmount main into current onto tip (-s ours; stages merge for signed commit)
upstream-join-main *ARGS:
    just grok_helper join-main-into-onto {{ ARGS }}

# Fail if AGENTS/FORK/RESIDUAL/helper recon modules missing after recon
[positional-arguments]
upstream-assert-process-pins *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    just grok_helper assert-process-pins --root "{{ justfile_directory() }}" "$@"

# Path assert, then remind land agents to walk the existing catalog.
# Does not replace just check. Does not run cargo (deleted tests stay silent).
[positional-arguments]
upstream-land-filters *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    just grok_helper assert-process-pins --root "{{ justfile_directory() }}" "$@"
    echo ""
    echo "Path assert OK. Next: walk FORK.md Land checklist and"
    echo "doc/dev/upstream-regression-filters.md Required land inventory."
    echo "Seven product classes: CLI identity; config is a surface; /spend ingest;"
    echo "DOGE/chrome paint; dual-auth hop after included SuperGrok period limits are full;"
    echo "last-session on start; product skills are not a Python runtime."
    echo "rg each required identifier for a matching fn. Missing fn = land failed."
    echo "Walk extra neighbors the catalog lists (bubble click, plan present is not Approve,"
    echo "SHA-aware rebuild, nucleo, from_config cold catalog, pause / Clear finished,"
    echo "always-three-layer product prompt, user-guide hop / spend-order)."
    echo "Not a second numbered board."
    echo "Then run the operator cheat-sheet cargo blocks in that catalog."
    echo "just check is quality only. Chrome-only is a failed land."

# Read-only recon probe: branch, CHERRY_PICK/MERGE, UU count, onto-ish, next human action
recon-status:
    just grok_helper recon-status

upstream-sync *ARGS:
    just grok_helper sync-upstream {{ ARGS }}
