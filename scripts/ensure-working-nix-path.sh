#!/usr/bin/env bash
# Ensure PATH prefers a working `nix` binary.
#
# Host package-manager installs can fail hard while a store or profile copy
# still works. Profile dirs may sit on PATH empty, so a broken system `nix`
# wins. This helper prefers NIX_BIN, then a probing PATH hit, then a store bin.
#
# Behavior:
#   - Honor NIX_BIN if set and executable (its directory is prepended to PATH).
#   - If `nix --version` already works (2s timeout), leave PATH alone.
#   - Else pick a working /nix/store/*-nix-*/bin/nix and prepend its dir.
#   - Skip probing common system package paths (avoids crash noise).
#
# Usage (source from recipes):
#   # shellcheck source=scripts/ensure-working-nix-path.sh
#   source "${ROOT}/scripts/ensure-working-nix-path.sh"
#
# Or: eval "$(.../ensure-working-nix-path.sh --export)"
set -euo pipefail

_grok_nix_probe() {
  local bin="$1"
  # System package paths: skip probe; recover via store/profile instead.
  case "${bin}" in
    /usr/bin/nix | /bin/nix | /usr/local/bin/nix)
      return 1
      ;;
  esac
  (
    ulimit -c 0 2>/dev/null || true
    timeout -k 1s 2s "${bin}" --version >/dev/null 2>&1
  )
}

_grok_nix_pick_store() {
  # Newest-looking store path first (version-ish sort).
  local p
  # shellcheck disable=SC2012
  for p in $(ls -1d /nix/store/*-nix-[0-9]*/bin/nix 2>/dev/null | sort -V -r); do
    [[ -x "${p}" ]] || continue
    if _grok_nix_probe "${p}"; then
      printf '%s\n' "${p}"
      return 0
    fi
  done
  return 1
}

_grok_ensure_working_nix_path() {
  local bin dir picked

  if [[ -n "${NIX_BIN:-}" ]]; then
    if [[ ! -x "${NIX_BIN}" ]]; then
      echo "ensure-working-nix-path: NIX_BIN is not executable: ${NIX_BIN}" >&2
      return 2
    fi
    dir="$(cd "$(dirname "${NIX_BIN}")" && pwd)"
    case ":${PATH}:" in
      *":${dir}:"*) ;;
      *) export PATH="${dir}:${PATH}" ;;
    esac
    return 0
  fi

  if command -v nix >/dev/null 2>&1; then
    bin="$(command -v nix)"
    if _grok_nix_probe "${bin}"; then
      return 0
    fi
  fi

  if ! picked="$(_grok_nix_pick_store)"; then
    echo "ensure-working-nix-path: no working nix found" >&2
    echo "  Set NIX_BIN to a working binary, or repair the host nix install." >&2
    return 2
  fi

  dir="$(cd "$(dirname "${picked}")" && pwd)"
  export PATH="${dir}:${PATH}"
  echo "==> ensure-working-nix-path: using ${picked}" >&2
  return 0
}

if [[ "${1:-}" == "--export" ]]; then
  _grok_ensure_working_nix_path
  printf 'export PATH=%q\n' "${PATH}"
  exit 0
fi

# When sourced, only define/run the ensure. When executed without --export,
# run ensure and print which nix is active (debug).
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  _grok_ensure_working_nix_path
  command -v nix
  nix --version
else
  _grok_ensure_working_nix_path
fi
