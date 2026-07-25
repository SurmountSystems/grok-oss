#!/usr/bin/env bash
# Rebuild PATH as a nix-store-only allowlist, then exec the remaining args.
#
# Used by `just cargo-ci` under CI_LOW_MEM=1 after `nix develop .#ci` so
# quality cargo/nextest children do not resolve optional host tools
# (pw-record / parec / arecord, clipboard helpers, etc.) from ambient
# desktop PATH. Interactive `nix develop` / default shell stay impure.
#
# Expectation: already inside `nix develop .#ci` (or equivalent) so PATH
# begins with ci-tools + stdenv store bins. This script drops everything
# that is not under /nix/store.
#
# Escape hatch (debug only): GROK_CI_ALLOW_HOST_PATH=1 keeps the full PATH.
#
# Usage:
#   nix develop .#ci -c ./scripts/with-ci-hermetic-path.sh cargo-mem-guard -- cargo test ...
set -euo pipefail

if [[ "${GROK_CI_ALLOW_HOST_PATH:-}" == "1" ]]; then
  exec "$@"
fi

old_path="${PATH:-}"
hermetic_path=""
IFS=':'
# shellcheck disable=SC2086
for d in ${old_path}; do
  case "${d}" in
    /nix/store/*)
      if [[ -n "${hermetic_path}" ]]; then
        hermetic_path="${hermetic_path}:${d}"
      else
        hermetic_path="${d}"
      fi
      ;;
  esac
done
unset IFS

if [[ -z "${hermetic_path}" ]]; then
  echo "with-ci-hermetic-path: PATH has no /nix/store entries after scrub" >&2
  echo "  (run under: nix develop .#ci -c …)" >&2
  exit 2
fi

export PATH="${hermetic_path}"
exec "$@"
