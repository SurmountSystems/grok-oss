#!/usr/bin/env bash
# Extract DWARF/debug symbols to a sidecar next to the binary, strip the
# binary, and embed a GNU debuglink so debuggers find the sidecar.
#
# Profile intent: [profile.release-dist] keeps strip=false + debug=1 so this
# post-build step can extract symbols. Plain `just install` uses --release and
# strip without a sidecar.
#
# Usage:
#   ./scripts/extract-debug-sidecar.sh <path-to-binary>
#
# Writes:
#   <binary>.debug   — debug-only object (same directory)
#   <binary>         — stripped in place, with .gnu_debuglink → basename
#
# Linux: requires objcopy (binutils) or llvm-objcopy.
# macOS: uses dsymutil + strip (no GNU debuglink; .dSYM bundle instead).
#
# See: just build-dist / just install-dist
set -euo pipefail

usage() {
  echo "Usage: $0 <path-to-binary>" >&2
  exit 2
}

[[ $# -eq 1 ]] || usage
BIN="$1"

if [[ ! -f "$BIN" ]]; then
  echo "extract-debug-sidecar: binary not found: $BIN" >&2
  exit 1
fi

# Resolve to an absolute path so cwd does not matter for later steps.
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
DIR="$(dirname "$BIN")"
BASE="$(basename "$BIN")"
OS="$(uname -s)"

find_objcopy() {
  if command -v objcopy >/dev/null 2>&1; then
    command -v objcopy
  elif command -v llvm-objcopy >/dev/null 2>&1; then
    command -v llvm-objcopy
  else
    return 1
  fi
}

case "$OS" in
  Linux)
    OBJCOPY="$(find_objcopy)" || {
      echo "extract-debug-sidecar: need objcopy or llvm-objcopy (binutils / llvm)" >&2
      exit 1
    }
    DEBUG="${BIN}.debug"
    echo "==> extract debug → ${DEBUG}"
    "$OBJCOPY" --only-keep-debug "$BIN" "$DEBUG"
    echo "==> strip debug + unneeded from ${BIN}"
    "$OBJCOPY" --strip-debug --strip-unneeded "$BIN"
    # Basename only: tools look for the sidecar next to the installed binary.
    echo "==> add GNU debuglink → ${BASE}.debug"
    (
      cd "$DIR"
      "$OBJCOPY" --add-gnu-debuglink="${BASE}.debug" "$BASE"
    )
    chmod -x "$DEBUG" 2>/dev/null || true
    echo "==> done: ${BIN} (stripped) + ${DEBUG}"
    ;;
  Darwin)
    # macOS: dSYM bundle; no GNU debuglink equivalent used by default.
    if ! command -v dsymutil >/dev/null 2>&1; then
      echo "extract-debug-sidecar: dsymutil not found (Xcode CLT required on macOS)" >&2
      exit 1
    fi
    if ! command -v strip >/dev/null 2>&1; then
      echo "extract-debug-sidecar: strip not found" >&2
      exit 1
    fi
    DSYM="${BIN}.dSYM"
    echo "==> dsymutil → ${DSYM}"
    dsymutil "$BIN" -o "$DSYM"
    echo "==> strip ${BIN}"
    strip -S "$BIN"
    echo "==> done: ${BIN} (stripped) + ${DSYM}"
    ;;
  *)
    echo "extract-debug-sidecar: unsupported OS '${OS}' (Linux + macOS only)" >&2
    exit 1
    ;;
esac
