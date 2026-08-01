#!/usr/bin/env bash
# Print a Nix system triple for this host without requiring a working `nix`.
#
# Prefer CI_SYSTEM when set; otherwise map uname → triple. Avoids parse-time
# `nix eval` so a broken host nix cannot fail every just recipe.
#
# Usage: ./scripts/nix-current-system.sh
# Override: CI_SYSTEM=x86_64-linux
set -euo pipefail

if [[ -n "${CI_SYSTEM:-}" ]]; then
  printf '%s\n' "${CI_SYSTEM}"
  exit 0
fi

kernel="$(uname -s)"
arch="$(uname -m)"

case "${kernel}" in
  Linux)
    case "${arch}" in
      x86_64) printf '%s\n' "x86_64-linux" ;;
      aarch64 | arm64) printf '%s\n' "aarch64-linux" ;;
      *)
        echo "nix-current-system: unsupported Linux arch: ${arch}" >&2
        echo "  set CI_SYSTEM=... (e.g. x86_64-linux)" >&2
        exit 1
        ;;
    esac
    ;;
  Darwin)
    case "${arch}" in
      x86_64) printf '%s\n' "x86_64-darwin" ;;
      arm64) printf '%s\n' "aarch64-darwin" ;;
      *)
        echo "nix-current-system: unsupported Darwin arch: ${arch}" >&2
        echo "  set CI_SYSTEM=... (e.g. aarch64-darwin)" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "nix-current-system: unsupported kernel: ${kernel}" >&2
    echo "  set CI_SYSTEM=... (e.g. x86_64-linux)" >&2
    exit 1
    ;;
esac
