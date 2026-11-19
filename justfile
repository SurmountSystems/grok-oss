# Grok OSS recipes. GitHub Actions must not call check-remote, test-remote, or cargo-remote.

set shell := ["bash", "-euo", "pipefail", "-c"]

set default-list

# CI_SYSTEM, else inline uname. Do not call nix at parse time.
system := env_var_or_default("CI_SYSTEM", `case "$(uname -s)-$(uname -m)" in Linux-x86_64) echo x86_64-linux;; Linux-aarch64|Linux-arm64) echo aarch64-linux;; Darwin-x86_64) echo x86_64-darwin;; Darwin-arm64) echo aarch64-darwin;; *) echo "unsupported $(uname -s)-$(uname -m); set CI_SYSTEM=..." >&2; exit 1;; esac`)

# When CI_LOW_MEM=1, cap cores and jobs on pure nix steps.
nix_low_mem_opts := if env_var_or_default("CI_LOW_MEM", "") == "1" { "--option cores 2 --option max-jobs 1" } else { "" }

low_mem := env_var_or_default("CI_LOW_MEM", "")

# Keep in sync with .github/workflows/ci.yml.
export NIX_CONFIG := '''
download-attempts = 5
connect-timeout = 30
stalled-download-timeout = 100
http-connections = 4
'''

# Locate only. Do not cargo or nix-build the helper on this laptop.
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

# Assign the helper path, then exec it. Do not exec a command substitution.
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

# CI_SYSTEM or the same uname map. No host nix and no helper binary.
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

# Refuse an unsafe system string. Do not interpolate system into this recipe.
[private]
require_system:
    #!/usr/bin/env bash
    set -euo pipefail
    sys="$(just current_system)"
    # Known triples, or two tokens that include a digit or underscore.
    if [[ "${sys}" =~ ^(x86_64-linux|aarch64-linux|x86_64-darwin|aarch64-darwin)$ ]]; then
      exit 0
    fi
    if [[ "${sys}" =~ ^[a-zA-Z0-9_]+-[a-zA-Z0-9_]+$ ]] && [[ "${sys}" =~ [0-9_] ]]; then
      exit 0
    fi
    echo "==> invalid CI_SYSTEM / system (refuse shell interpolation): ${sys}" >&2
    echo "    expected a Nix system like x86_64-linux (cpu-os), not a just recipe name" >&2
    exit 2

# SSH preflight only. No helper binary. Require surmount-remote.
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
    echo "==> just check-remote: rustc, clippy, and nextest run on the builder. This laptop does not run them. The builder must list surmount-remote."
    echo "==> just check-remote: source is rsynced to /home/nixbuilder/grok-oss-src. Incremental cargo output stays at /home/nixbuilder/grok-oss-target on that machine."
    echo "==> just check-remote: this recipe does not upload a cargo target directory and does not realize the quality derivation."

# Argv exec. Do not route through the helper. Do not set -- over the command.
# Retry ld returned 137. Fail fast on SSH and on a real compile error.
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
      # Linker SIGKILL is not a type error. Check it before could-not-compile.
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
      # store cat rejects --eval-store. Other commands still pass --eval-store auto.
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

# Same Nix gate as ci.
check: ci

# Host cargo only. Not an alias of check.
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
	# Snapshot tracked and untracked files. Skip gitignored cargo output.
	snapshot="$(mktemp -d "${TMPDIR:-/tmp}/grok-oss-source.XXXXXX")"
	trap 'rm -rf "$snapshot"' EXIT
	git ls-files -z --cached --others --exclude-standard \
	  | rsync -a --from0 --files-from=- ./ "${snapshot}/"
	if find "$snapshot" -type d -name target -print -quit | grep -q .; then
	  echo "just check-remote: snapshot still contains a target directory" >&2
	  exit 2
	fi
	builder_field="$(awk '$1 ~ /^ssh-ng:/ { print $1; exit }' "${GROK_NIX_BUILDERS_FILE}")"
	builder_field="${builder_field%%\?*}"
	ssh_spec="${builder_field#ssh-ng://}"
	if [[ -z "${ssh_spec}" || "${ssh_spec}" == "${builder_field}" ]]; then
	  echo "just check-remote: builders file has no ssh-ng destination" >&2
	  exit 2
	fi
	remote_src="/home/nixbuilder/grok-oss-src"
	echo "==> just check-remote: rsync filtered source to the builder. The first sync sends the crate tree. Later syncs send changed files."
	# Do not print the builders destination. Do not upload a cargo target.
	ssh -o BatchMode=yes ${NIX_SSHOPTS} "${ssh_spec}" "mkdir -p ${remote_src}"
	rsync -a --delete --exclude target -e "ssh -o BatchMode=yes ${NIX_SSHOPTS}" "${snapshot}/" "${ssh_spec}:${remote_src}/"
	echo "==> just check-remote: flake metadata on the builder"
	# On the builder. Do not send this tree back through nix_retry --store.
	ssh -o BatchMode=yes ${NIX_SSHOPTS} "${ssh_spec}" "nix flake metadata 'path:${remote_src}'"
	echo "==> just check-remote: fmt, clippy, nextest, and doctests on the builder"
	# Do not realize the quality derivation.
	ssh -o BatchMode=yes ${NIX_SSHOPTS} "${ssh_spec}" "bash -s" <<-'GROK_CHECK_REMOTE_SCRIPT'
	set -euo pipefail
	mkdir -p /home/nixbuilder/grok-oss-target
	cat > /home/nixbuilder/grok-oss-target/run-quality.sh <<-'GROK_CHECK_REMOTE_QUALITY'
	set -euo pipefail
	cd /home/nixbuilder/grok-oss-src
	if [[ -z "${GROK_CHECK_REMOTE_SANDBOX:-}" ]]; then
	  if ! command -v bwrap >/dev/null 2>&1; then
	    echo "just check-remote: bubblewrap is not on the dev-shell PATH" >&2
	    exit 2
	  fi
	  etc_copy=/home/nixbuilder/grok-oss-target/etc-overlay
	  rm -rf "${etc_copy}"
	  mkdir -p "${etc_copy}"
	  # This copies readable entries from surmount-1 /etc and skips root-only files.
	  while IFS= read -r -d '' src; do
	    rel="${src#/etc/}"
	    dest="${etc_copy}/${rel}"
	    if [[ ! -r "${src}" ]]; then
	      continue
	    fi
	    if [[ -L "${src}" ]]; then
	      mkdir -p "$(dirname "${dest}")"
	      cp -a --no-preserve=ownership "${src}" "${dest}"
	    elif [[ -d "${src}" ]]; then
	      mkdir -p "${dest}"
	    else
	      mkdir -p "$(dirname "${dest}")"
	      cp -a --no-preserve=ownership "${src}" "${dest}"
	    fi
	  done < <(find /etc -mindepth 1 -print0 2>/dev/null || true)
	  rm -f "${etc_copy}/hosts"
	  cp -L /etc/hosts "${etc_copy}/hosts"
	  if [[ -L "${etc_copy}/hosts" || ! -f "${etc_copy}/hosts" ]]; then
	    echo "just check-remote: could not make a regular /etc/hosts for the quality sandbox" >&2
	    exit 2
	  fi
	  export GROK_CHECK_REMOTE_SANDBOX=1
	  # Bubblewrap binds host /etc/passwd over the overlay copy.
	  exec bwrap --dev-bind / / --tmpfs /tmp --bind "${etc_copy}" /etc --ro-bind /etc/passwd /etc/passwd --die-with-parent bash /home/nixbuilder/grok-oss-target/run-quality.sh
	fi
	export TMPDIR=/tmp
	export TEMP=/tmp
	export TMP=/tmp
	unset no_proxy NO_PROXY http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY all_proxy || true
	# Unset SSH_CONNECTION SSH_TTY SSH_CLIENT ET_VERSION.
	unset SSH_CONNECTION SSH_TTY SSH_CLIENT ET_VERSION || true
	clean_path=""
	IFS=':' read -ra path_parts <<< "${PATH}"
	for part in "${path_parts[@]}"; do
	  case "${part}" in
	    /nix/store/*) clean_path="${clean_path:+${clean_path}:}${part}" ;;
	  esac
	done
	if [[ -z "${clean_path}" ]]; then
	  echo "just check-remote: dev-shell PATH has no /nix/store entries" >&2
	  exit 2
	fi
	export PATH="${clean_path}"
	while IFS= read -r -d '' assignment; do
	  name="${assignment%%=*}"
	  value="${assignment#*=}"
	  case "${value}" in
	    *per-user*)
	      if [[ -n "${name}" && "${name}" != "PATH" ]]; then
	        unset "${name}" || true
	      fi
	      ;;
	  esac
	done < <(env -0)
	# Cargo output stays at /home/nixbuilder/grok-oss-target.
	export CARGO_TARGET_DIR=/home/nixbuilder/grok-oss-target
	export CARGO_INCREMENTAL=1
	# Keep the Nix cc wrapper. NIX_ENFORCE_PURITY=0.
	export NIX_ENFORCE_PURITY=0
	if ! command -v mold >/dev/null 2>&1; then
	  echo "just check-remote: mold is not on the dev-shell PATH" >&2
	  exit 2
	fi
	if ! command -v python3 >/dev/null 2>&1; then
	  echo "just check-remote: python3 is not on the dev-shell PATH" >&2
	  exit 2
	fi
	unset CARGO_ENCODED_RUSTFLAGS
	export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
	lib_path=""
	if command -v pkg-config >/dev/null 2>&1; then
	  for pc in libssl libcrypto dbus-1; do
	    libdir="$(pkg-config --variable=libdir "${pc}" 2>/dev/null || true)"
	    if [[ -n "${libdir}" && "${lib_path}" != *"${libdir}"* ]]; then
	      lib_path="${lib_path:+${lib_path}:}${libdir}"
	    fi
	  done
	fi
	if [[ -n "${DUCKDB_LIB_DIR:-}" ]]; then
	  lib_path="${lib_path:+${lib_path}:}${DUCKDB_LIB_DIR}"
	fi
	if [[ -n "${lib_path}" ]]; then
	  export LD_LIBRARY_PATH="${lib_path}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
	fi
	export RULES_RUST_RUNFILES_WORKSPACE_NAME="${RULES_RUST_RUNFILES_WORKSPACE_NAME:-grok-oss}"
	export GROK_DISABLE_SHARED_HARNESS_SECRETS="${GROK_DISABLE_SHARED_HARNESS_SECRETS:-1}"
	export GROK_CREDENTIALS_FORCE_FILE="${GROK_CREDENTIALS_FORCE_FILE:-1}"
	export GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY="${GROK_TRUST_LOOPBACK_CLI_CHAT_PROXY:-1}"
	unset NO_COLOR
	unset CARGO_TERM_COLOR
	unset OPENROUTER_API_KEY
	mkdir -p frontend/apps/grok-desktop/scripts
	cp -f crates/codegen/xai-grok-pager/scripts/install.sh frontend/apps/grok-desktop/scripts/install.sh
	cargoJobs="${NIX_BUILD_CORES:-64}"
	case "${cargoJobs}" in
	  "" | *[!0-9]*) cargoJobs=64 ;;
	esac
	if [ "${cargoJobs}" -gt 64 ]; then
	  cargoJobs=64
	fi
	if [ "${cargoJobs}" -le 64 ]; then
	  cargoJobs=64
	fi
	export CARGO_BUILD_JOBS="${cargoJobs}"
	linkJobs="${CARGO_BUILD_JOBS}"
	if [ "${linkJobs}" -gt 32 ]; then
	  linkJobs=32
	fi
	export CARGO_LINK_JOBS="${linkJobs}"
	export CARGO_PROFILE=dev
	unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
	echo "workspace cargo jobs=${CARGO_BUILD_JOBS} link jobs=${CARGO_LINK_JOBS} NIX_BUILD_CORES=${NIX_BUILD_CORES:-unset}"
	workspace_run_make_jobserver() {
	  unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
	  if [ "$#" -lt 1 ]; then
	    echo "workspace_run_make_jobserver: missing command" >&2
	    exit 2
	  fi
	  mk="${TMPDIR:-/tmp}/workspace-cargo-jobserver.mk"
	  {
	    printf 'all:\n\t+env -u MAKEFLAGS -u MFLAGS -u CARGO_MAKEFLAGS nice -n 19 '
	    printf '%q ' "$@"
	    printf '\n'
	  } > "${mk}"
	  echo "workspace cargo make -j${CARGO_BUILD_JOBS} jobserver: $*"
	  make -j"${CARGO_BUILD_JOBS}" --no-print-directory -f "${mk}"
	}
	nice -n 19 cargo fmt --all -- --check
	clippy_driver="$(command -v clippy-driver || true)"
	if [[ -z "${clippy_driver}" || ! -x "${clippy_driver}" ]]; then
	  echo "just check-remote: clippy-driver not on PATH" >&2
	  exit 2
	fi
	export RUSTC_WORKSPACE_WRAPPER="${clippy_driver}"
	export CLIPPY_ARGS="-D__CLIPPY_HACKERY__warnings__CLIPPY_HACKERY__"
	workspace_run_make_jobserver cargo check --profile "$CARGO_PROFILE" --jobs "$CARGO_BUILD_JOBS" --workspace --all-targets --locked
	unset RUSTC_WORKSPACE_WRAPPER CLIPPY_ARGS
	unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
	nextest_log="${CARGO_TARGET_DIR}/nextest.log"
	if ! command -v rg >/dev/null 2>&1; then
	  echo "just check-remote: rg not on PATH" >&2
	  exit 2
	fi
	CARGO_BUILD_JOBS="$CARGO_LINK_JOBS" nice -n 19 cargo nextest run --workspace --locked --build-jobs "$CARGO_LINK_JOBS" -j "$CARGO_BUILD_JOBS" 2>&1 | tee "${nextest_log}"
	unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
	workspace_run_make_jobserver cargo test --workspace --doc --locked --profile "$CARGO_PROFILE" --jobs "$CARGO_LINK_JOBS"
	summary="$(rg -a -F -n 'Summary [' "${nextest_log}" | tail -n 1 || true)"
	if [[ -z "${summary}" ]]; then
	  echo "just check-remote: nextest Summary line missing" >&2
	  exit 2
	fi
	{
	  echo "workspace-cargo-quality receipt"
	  echo "fmt: cargo fmt --all -- --check"
	  echo "clippy: cargo check --workspace --all-targets (clippy-driver -D warnings)"
	  echo "${summary}"
	  echo "doctests: cargo test --workspace --doc"
	} > /home/nixbuilder/grok-oss-target/quality-summary.txt
	GROK_CHECK_REMOTE_QUALITY
	cd /home/nixbuilder/grok-oss-src
	nix develop 'path:/home/nixbuilder/grok-oss-src' -c bash /home/nixbuilder/grok-oss-target/run-quality.sh
	GROK_CHECK_REMOTE_SCRIPT
	echo "==> just check-remote: quality receipt"
	ssh -o BatchMode=yes ${NIX_SSHOPTS} "${ssh_spec}" "cat /home/nixbuilder/grok-oss-target/quality-summary.txt"

# rustc does not run on this laptop. GitHub Actions must not call this recipe.
[positional-arguments]
cargo-remote *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    export GROK_NIX_FORCE_REMOTE=1
    just grok_helper remote-named-cargo -- "$@"

[positional-arguments]
test-remote *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    export GROK_NIX_FORCE_REMOTE=1
    # Empty filter and --no-run are local rejects. Do not locate grok-nix-helper first.
    if [[ "$#" -eq 0 ]]; then
      echo "just test-remote: missing test filter. Pass a filter (for example -p xai-grok-pager --lib -- actions::defaults)." >&2
      echo "Refusing to run the whole workspace on the builder from an empty filter. Full gate: just check-remote." >&2
      exit 2
    fi
    for a in "$@"; do
      if [[ "${a}" == "--no-run" ]]; then
        echo "just test-remote / just cargo-remote test runs the tests on the remote builder. Do not pass --no-run (that is compile-only)." >&2
        exit 2
      fi
    done
    just grok_helper remote-named-cargo -- test "$@"

[private]
[positional-arguments]
remote_named_cargo *args: require_system
    #!/usr/bin/env bash
    set -euo pipefail
    export GROK_NIX_FORCE_REMOTE=1
    just grok_helper remote-named-cargo -- "$@"

ci: require_system
    just flake-meta
    just ci-prep
    just test
    @rm -f .ci-started
    @echo "CI passed"

ci-prep: require_system mem-guard
    @echo "==> ci-prep: realize .#ci-tools (nix_retry)"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#ci-tools
    @touch .ci-started

# Refreshes locks only. Does not compile.
update:
    @echo "==> cargo update (workspace)"
    cargo update --manifest-path Cargo.toml
    @echo "==> nix flake update"
    nix flake update

flake-meta:
    @echo "==> flake-meta"
    just nix_retry nix flake metadata

# Not the CI gate.
build:
    @echo "==> build .#grok-oss{{ if low_mem == "1" { " (low-mem nix opts)" } else { "" } }} [not CI]"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#grok-oss

smoke: build
    @echo "==> smoke [not CI]"
    test -x ./result/bin/grok-oss
    ./result/bin/grok-oss --version

cargo-check: require_system
    @echo "==> cargo-check [not CI quality]"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.cargoCheck"

openrouter-tests: require_system
    @echo "==> openrouter-tests (optional flake check; covered by just test)"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.openrouter-credentials"

mem-guard: require_system
    @echo "==> build .#cargo-mem-guard{{ if low_mem == "1" { " (low-mem nix opts)" } else { "" } }}"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#cargo-mem-guard
    @echo "==> check .#cargo-mem-guard-tests"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.cargo-mem-guard-tests"
    @echo "==> build .#grok-nix-helper{{ if low_mem == "1" { " (low-mem nix opts)" } else { "" } }}"
    just nix_retry nix build -L {{ nix_low_mem_opts }} .#grok-nix-helper
    @echo "==> check .#grok-nix-helper-tests"
    just nix_retry nix build -L {{ nix_low_mem_opts }} ".#checks.{{ system }}.grok-nix-helper-tests"

# Do not wrap this cargo payload in nix_retry.
# CI_LOW_MEM scrubs PATH to nix-store bins. GROK_CI_ALLOW_HOST_PATH=1 keeps the host PATH.
[private]
[positional-arguments]
cargo-ci +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    just grok_helper cargo-ci -- "$@"

dev:
    #!/usr/bin/env bash
    set -euo pipefail
    eval "$(just grok_helper ensure-nix-path --export)"
    exec nix develop

dev-ci:
    #!/usr/bin/env bash
    set -euo pipefail
    eval "$(just grok_helper ensure-nix-path --export)"
    exec nix develop .#ci

# Clippy is --all-targets. Do not pass --manifest-path.
test: test-fmt test-clippy test-unit test-doc
    @echo "just test passed"

test-extra: test-clippy-targets test-clippy-all-targets test-ensure-working-nix-path test-nix-retry-smoke test-nix-retry-does-not-require-helper-binary test-nix-retry-hard-remote-miss-fail-fast test-nix-retry-missing-system-features-fail-fast test-nix-retry-rustfmt-diff-fail-fast test-nix-retry-clippy-compile-fail-fast test-nix-retry-linker-sigkill-retries test-nix-retry-locked-lockfile-fail-fast test-nix-retry-fixed-output-hash-mismatch-fail-fast test-nix-retry-nextest-fail-fast test-nix-retry-force-remote-argv-is-nix test-nix-retry-force-remote-ssh-ng-max-connections test-check-remote-builders-file-smoke test-check-remote-cargo-is-remote-nix-derivation test-check-remote-vendor-unpacks-not-blocked-by-max-jobs-zero test-check-remote-uses-builder-cores test-check-remote-clippy-uses-many-workers test-check-remote-deps-omit-git-sha test-check-remote-omits-local-big-parallel test-check-remote-workspace-rustc-not-local-eligible test-check-remote-preflight-same-path-as-nix-ssh test-check-remote-preflight-remote-daemon-features test-test-remote-workspace-rustc-not-local-eligible test-test-remote-requires-filter
    @echo "just test-extra passed"

test-fmt:
    @echo "==> cargo fmt --all -- --check"
    just cargo-ci cargo fmt --all -- --check

test-clippy:
    @echo "==> cargo clippy --workspace --all-targets (-D warnings)"
    just cargo-ci cargo clippy --workspace --all-targets --locked -- -D warnings

test-unit:
    @echo "==> cargo nextest run --workspace"
    just cargo-ci cargo nextest run --workspace --locked

test-doc:
    @echo "==> cargo test --workspace --doc"
    just cargo-ci cargo test --workspace --doc --locked

test-mem-guard:
    @echo "==> cargo nextest run -p cargo-mem-guard"
    just cargo-ci cargo nextest run -p cargo-mem-guard --locked

test-grok-nix-helper:
    @echo "==> cargo nextest run -p grok-nix-helper"
    just cargo-ci cargo nextest run -p grok-nix-helper --locked

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

# EXTRA_CLIPPY_TARGETS overrides the non-host target.
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
    quality_nix="${root}/flake/workspace-quality.nix"
    jobs_helper="$(awk '
      $0 ~ /workspaceCargoJobsFromCores =/ { p=1 }
      p && $0 ~ /workspaceCargoArtifacts = craneLib.buildDepsOnly/ { exit }
      p { print }
    ' "${quality_nix}")"
    quality="$(awk '
      $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /^in$/ { exit }
      p { print }
    ' "${quality_nix}")"
    artifacts="$(awk '
      $0 ~ /workspaceCargoArtifacts = craneLib.buildDepsOnly/ { p=1 }
      p && $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { exit }
      p { print }
    ' "${quality_nix}")"
    if ! grep -q 'CARGO_BUILD_JOBS = "64"' <<<"${quality}"; then
      echo "test-check-remote-uses-builder-cores: workspace-cargo-quality must set CARGO_BUILD_JOBS = \"64\" (not inherit 2 from commonArgs)." >&2
      echo "${quality}" >&2
      exit 1
    fi
    if grep -q 'CARGO_BUILD_JOBS = "32"' <<<"${quality}${artifacts}${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: quality / artifacts / helper must not keep CARGO_BUILD_JOBS = \"32\"." >&2
      echo "${quality}" >&2
      echo "${artifacts}" >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'CARGO_BUILD_JOBS = "64"' <<<"${artifacts}"; then
      echo "test-check-remote-uses-builder-cores: workspaceCargoArtifacts must set CARGO_BUILD_JOBS = \"64\"." >&2
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
      echo "test-check-remote-uses-builder-cores: cargo --jobs must be taken from NIX_BUILD_CORES (then 64 from --cores 64)." >&2
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
    if [[ "${jobs}" != "64" ]]; then
      echo "test-check-remote-uses-builder-cores: expected CARGO_BUILD_JOBS=64 on workspace-cargo-quality, got ${jobs}" >&2
      exit 1
    fi
    profile="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.CARGO_PROFILE")"
    if [[ "${profile}" != "dev" ]]; then
      echo "test-check-remote-uses-builder-cores: expected CARGO_PROFILE=dev on workspace-cargo-quality, got ${profile}" >&2
      exit 1
    fi
    # Check the instantiated buildPhase. A source grep can miss the jobserver.
    phase="$(nix eval --raw ".#packages.${sys}.workspace-cargo-quality.buildPhase")"
    if ! grep -qE -- 'check --profile "\$CARGO_PROFILE" --jobs "\$CARGO_BUILD_JOBS"' <<<"${phase}"; then
      echo "test-check-remote-uses-builder-cores: instantiated buildPhase must pass cargo check --jobs from CARGO_BUILD_JOBS (jobs 64 from NIX_BUILD_CORES) for workspace clippy." >&2
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
    if grep -qE 'cargoJobs" -lt 2|"\$cargoJobs" -lt 2' <<<"${phase}${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: old promotion-only-for-0-or-1 must go; 2 through 63 must not remain cargo jobs." >&2
      echo "${jobs_helper}" >&2
      echo "${phase}" >&2
      exit 1
    fi
    if grep -qE 'cargoJobs" -le 32|"\$cargoJobs" -le 32|cargoJobs" -gt 32|"\$cargoJobs" -gt 32' <<<"${phase}${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: do not leave a 32 cargo-jobs floor or cap in this helper." >&2
      echo "${jobs_helper}" >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -qE 'cargoJobs" -le 64|"\$cargoJobs" -le 64' <<<"${phase}${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: when NIX_BUILD_CORES is 64 or less (including 0, 1, and 2 through 63), cargo jobs must become 64." >&2
      echo "${jobs_helper}" >&2
      echo "${phase}" >&2
      exit 1
    fi
    if ! grep -q 'nice -n 19' <<<"${jobs_helper}" || ! grep -q 'env -u MAKEFLAGS -u MFLAGS -u CARGO_MAKEFLAGS nice -n 19' <<<"${jobs_helper}"; then
      echo "test-check-remote-uses-builder-cores: make recipe must env -u MAKEFLAGS -u MFLAGS -u CARGO_MAKEFLAGS nice -n 19 cargo." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'nice -n 19 cargo fmt' <<<"${phase}" || ! grep -q 'nice -n 19 cargo nextest' <<<"${phase}"; then
      echo "test-check-remote-uses-builder-cores: direct quality cargo (fmt, nextest) must be nice -n 19." >&2
      echo "${phase}" >&2
      exit 1
    fi
    echo "test-check-remote-uses-builder-cores: ok (--cores 64; workspace cargo jobs 64 from cores on argv; nice -n 19; CARGO_PROFILE=dev; package sandbox still 2)"

test-check-remote-clippy-uses-many-workers:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    quality_nix="${root}/flake/workspace-quality.nix"
    named_nix="${root}/flake/workspace-named-test.nix"
    jobs_helper="$(awk '
      $0 ~ /workspaceCargoJobsFromCores =/ { p=1 }
      p && $0 ~ /workspaceCargoArtifacts = craneLib.buildDepsOnly/ { exit }
      p { print }
    ' "${quality_nix}")"
    quality="$(awk '
      $0 ~ /workspace-cargo-quality = craneLib.mkCargoDerivation/ { p=1 }
      p && $0 ~ /^in$/ { exit }
      p { print }
    ' "${quality_nix}")"
    named="$(awk '
      $0 ~ /workspace-cargo-named-test = craneLib.mkCargoDerivation/ { p=1 }
      p { print }
    ' "${named_nix}")"
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
      echo "test-check-remote-clippy-uses-many-workers: jobserver helper must run make -j\"\$CARGO_BUILD_JOBS\" (jobs 64 from cores)." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS' <<<"${jobs_helper}"; then
      echo "test-check-remote-clippy-uses-many-workers: helper must unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'env -u MAKEFLAGS' <<<"${jobs_helper}" || ! grep -q -- '-u CARGO_MAKEFLAGS' <<<"${jobs_helper}" || ! grep -q -- '-u MFLAGS' <<<"${jobs_helper}"; then
      echo "test-check-remote-clippy-uses-many-workers: make recipe must env -u MAKEFLAGS -u MFLAGS -u CARGO_MAKEFLAGS so cargo owns --jobs (does not join make's jobserver)." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'env -u MAKEFLAGS -u MFLAGS -u CARGO_MAKEFLAGS nice -n 19' <<<"${jobs_helper}"; then
      echo "test-check-remote-clippy-uses-many-workers: make recipe must drop the jobserver, then nice -n 19, then cargo." >&2
      echo "${jobs_helper}" >&2
      exit 1
    fi
    if ! grep -q 'workspace_run_make_jobserver cargo check' <<<"${quality}"; then
      echo "test-check-remote-clippy-uses-many-workers: clippy-as-check must be wrapped: workspace_run_make_jobserver cargo check." >&2
      echo "${quality}" >&2
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
    if ! grep -q 'env -u MAKEFLAGS' <<<"${phase}" || ! grep -q -- '-u CARGO_MAKEFLAGS' <<<"${phase}"; then
      echo "test-check-remote-clippy-uses-many-workers: instantiated make recipe must env -u MAKEFLAGS and -u CARGO_MAKEFLAGS so cargo owns --jobs." >&2
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
    echo "test-check-remote-clippy-uses-many-workers: ok (cargo check + clippy-driver; make -j\$CARGO_BUILD_JOBS jobserver; nice -n 19; no cargo clippy dispatcher; no -j1 crate loop)"

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

test-check-remote-preflight-same-path-as-nix-ssh:
    #!/usr/bin/env bash
    set -euo pipefail
    # Runtime probe only. Do not grep the preflight recipe body.
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

test-check-remote-preflight-remote-daemon-features:
    #!/usr/bin/env bash
    set -euo pipefail
    # Runtime probe only. Do not grep the preflight recipe body.
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

test-test-remote-requires-filter:
    #!/usr/bin/env bash
    set -euo pipefail
    set +e
    empty_out="$(env -u GROK_NIX_HELPER just test-remote 2>&1)"
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
    if grep -qF 'grok-nix-helper is not on PATH' <<<"${empty_out}"; then
      echo "test-test-remote-requires-filter: empty just test-remote must not print grok-nix-helper is not on PATH:" >&2
      echo "${empty_out}" >&2
      exit 1
    fi
    if ! grep -qF 'missing test filter' <<<"${empty_out}"; then
      echo "test-test-remote-requires-filter: empty just test-remote must name a missing test filter (recipe failed alone is not enough):" >&2
      echo "${empty_out}" >&2
      exit 1
    fi
    if grep -qE 'nix attempt|nix build' <<<"${empty_out}"; then
      echo "test-test-remote-requires-filter: empty filter must not start nix build:" >&2
      echo "${empty_out}" >&2
      exit 1
    fi
    set +e
    norun_out="$(env -u GROK_NIX_HELPER just test-remote --no-run 2>&1)"
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
    # Honor NIX_BIN so this smoke does not call host nix.
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
install:
    #!/usr/bin/env bash
    set -euo pipefail
    # Pin mold. Host rustflags often set -fuse-ld=wild.
    mkdir -p "${CARGO_HOME:-$HOME/.cargo}/bin"
    echo "==> nice -n 19 cargo build --release -p xai-grok-pager-bin (no wild linker)"
    env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
      RULES_RUST_RUNFILES_WORKSPACE_NAME="${RULES_RUST_RUNFILES_WORKSPACE_NAME:-grok-oss}" \
      nice -n 19 \
      cargo build --release -p xai-grok-pager-bin --locked \
      --config 'build.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]' \
      --config 'target.x86_64-unknown-linux-gnu.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]' \
      --config 'target.aarch64-unknown-linux-gnu.rustflags=["-C","link-arg=-fuse-ld=mold","-C","force-unwind-tables=yes"]'
    echo "==> strip unneeded symbols (install artifact only)"
    strip --strip-unneeded target/release/grok-oss
    echo "==> install -> ${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    install -Dm755 target/release/grok-oss "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    # Fish starts at ~/.local/bin. Symlink the installed binary there.
    mkdir -p "${HOME}/.local/bin"
    ln -sfn "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" "${HOME}/.local/bin/grok-oss"
    echo "==> verify"
    "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" --version
    file "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" | grep -q 'stripped' \
      || (echo "install: expected stripped binary" >&2; file "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" >&2; exit 1)

build-dist:
    # Pin mold. Host rustflags often set -fuse-ld=wild.
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

# Strip after copy. Nix may already have stripped the binary.
install-nix: build
    mkdir -p "${CARGO_HOME:-$HOME/.cargo}/bin"
    install -Dm755 ./result/bin/grok-oss "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    strip --strip-unneeded "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss"
    "${CARGO_HOME:-$HOME/.cargo}/bin/grok-oss" --version

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
    just cargo-ci cargo test -p xai-grok-pager --lib --locked -- \
      flat_poll prepaid_lag \
      management_meter_cache_policy should_clear_management_meter \
      check_limits_first multipoll
    echo "check-limits-first-path passed (hermetic)"

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

upstream-detect:
    just grok_helper detect-upstream-export

upstream-import *ARGS:
    just grok_helper import-upstream-export {{ ARGS }}

upstream-put-history *ARGS:
    just grok_helper put-history-on-xai {{ ARGS }}

upstream-join-main *ARGS:
    just grok_helper join-main-into-onto {{ ARGS }}

[positional-arguments]
upstream-assert-process-pins *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    just grok_helper assert-process-pins --root "{{ justfile_directory() }}" "$@"

# Does not replace just check. Does not run cargo.
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

recon-status:
    just grok_helper recon-status

upstream-sync *ARGS:
    just grok_helper sync-upstream {{ ARGS }}
