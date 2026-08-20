#!/usr/bin/env bash
# Assert Surmount process-pin paths are present in the worktree (or a git tree).
#
# Use after import restore, after onto stack lands, or anytime before calling
# residual "done" on recon. Fails with a missing list — does not modify git.
#
# Usage:
#   ./scripts/assert-process-pins.sh
#   ./scripts/assert-process-pins.sh HEAD
#   ./scripts/assert-process-pins.sh origin/main
#   TREE_ISH=onto-xai/… ./scripts/assert-process-pins.sh
#
# Env:
#   TREE_ISH   if set (or first arg), check that git tree instead of worktree
#   STRICT=1   also require doc/dev and docs/dev research roots non-empty
#
# See docs/upstream-history.md (import review) and
# doc/dev/research/fork-paths-hardening-2026-07-24.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TREE_ISH="${TREE_ISH:-${1:-}}"
STRICT="${STRICT:-0}"

# Required files: silent absence after recon is a process bug.
REQUIRED_FILES=(
  AGENTS.md
  FORK.md
  RESIDUAL.md
  README.md
  CONTRIBUTING.md
  SECURITY.md
  justfile
  flake.nix
  flake.lock
  docs/upstream-history.md
  docs/upstream-import-log.md
  docs/upstream-onto-log.md
  docs/git-workflow.md
  doc/dev/upstream-regression-filters.md
  scripts/detect-upstream-export.sh
  scripts/import-upstream-export.sh
  scripts/sync-upstream.sh
  scripts/put-history-on-xai.sh
  scripts/join-main-into-onto.sh
  scripts/with-ci-hermetic-path.sh
  scripts/assert-process-pins.sh
  scripts/recon-status.sh
  scripts/replay-onto-upstream.sh
  .github/workflows/upstream-export.yml
  .github/workflows/ci.yml
)

# Required directories (at least one tracked blob under path, or dir in worktree).
REQUIRED_DIRS=(
  packaging
  crates/codegen/grok-rate-limit
  doc/dev
  docs/dev
  .grok/workflows   # Rhai process workflows; in FORK_PATHS — must not silent-drop
)

missing=()
warn=()

path_in_tree() {
  local p="$1"
  git cat-file -e "${TREE_ISH}:${p}" 2>/dev/null
}

dir_in_tree() {
  local p="$1"
  # non-empty tree entry
  git ls-tree -r --name-only "$TREE_ISH" -- "$p" 2>/dev/null | grep -q .
}

if [[ -n "$TREE_ISH" ]]; then
  if ! git rev-parse --verify "$TREE_ISH^{tree}" >/dev/null 2>&1; then
    echo "error: not a valid tree-ish: $TREE_ISH" >&2
    exit 2
  fi
  echo "assert-process-pins: checking tree $TREE_ISH"
  for f in "${REQUIRED_FILES[@]}"; do
    if ! path_in_tree "$f"; then
      missing+=("$f")
    fi
  done
  for d in "${REQUIRED_DIRS[@]}"; do
    if ! dir_in_tree "$d"; then
      missing+=("$d/ (empty or absent)")
    fi
  done
else
  echo "assert-process-pins: checking worktree at $ROOT"
  for f in "${REQUIRED_FILES[@]}"; do
    if [[ ! -f $f ]]; then
      missing+=("$f")
    fi
  done
  for d in "${REQUIRED_DIRS[@]}"; do
    if [[ ! -d $d ]]; then
      missing+=("$d/ (absent)")
    elif [[ "$STRICT" == "1" ]] && [[ -z "$(find "$d" -type f 2>/dev/null | head -1)" ]]; then
      missing+=("$d/ (empty, STRICT=1)")
    fi
  done
fi

# Catalog is the land inventory (not a second board). The file must exist
# (REQUIRED_FILES) and still name the seven product class titles. This does
# not prove crate `fn` names. Land agents still `rg` the catalog identifiers.
LAND_CLASS_MARKERS=(
  "### 1. CLI identity"
  "### 2. Config is a surface"
  "### 3. grok-oss SQL extras"
  "### 4. DOGE / Surmount chrome"
  "### 5. Dual-auth hop after included SuperGrok period limits are full"
  "### 6. Last-session on start"
  "### 7. Product skills are not a Python runtime"
)
catalog_body=""
if [[ -n "$TREE_ISH" ]]; then
  if path_in_tree "doc/dev/upstream-regression-filters.md"; then
    catalog_body=$(git show "${TREE_ISH}:doc/dev/upstream-regression-filters.md" 2>/dev/null || true)
  fi
elif [[ -f doc/dev/upstream-regression-filters.md ]]; then
  catalog_body=$(<doc/dev/upstream-regression-filters.md)
fi
if [[ -n "$catalog_body" ]]; then
  for marker in "${LAND_CLASS_MARKERS[@]}"; do
    if ! grep -F -q -- "$marker" <<<"$catalog_body"; then
      missing+=("doc/dev/upstream-regression-filters.md (missing land class title: $marker)")
    fi
  done
fi

# Light content sniffs (worktree only) — catch xAI placeholder / empty shells.
if [[ -z "$TREE_ISH" ]]; then
  if [[ -f AGENTS.md ]] && ! grep -q 'parent is coordinator' AGENTS.md 2>/dev/null; then
    warn+=("AGENTS.md present but missing expected 'parent is coordinator' pin")
  fi
  if [[ -f FORK.md ]] && ! grep -qi 'upstream\|import\|onto' FORK.md 2>/dev/null; then
    warn+=("FORK.md present but no upstream/import/onto mention (odd for this fork)")
  fi
  if [[ -f README.md ]] && ! grep -qi 'Grok OSS\|grok-oss' README.md 2>/dev/null; then
    warn+=("README.md present but missing Grok OSS branding (possible xAI clobber)")
  fi
  if [[ -f FORK.md ]] && ! grep -q 'non-excepted Python' FORK.md 2>/dev/null; then
    missing+=("FORK.md land class 7 (must say a restack that installs non-excepted Python is a failed land)")
  fi
  if [[ -f FORK.md ]] && ! grep -q 'stay-supergrok' FORK.md 2>/dev/null; then
    missing+=("FORK.md (must name stay-supergrok)")
  fi
  if [[ -f FORK.md ]] && ! grep -q 'limits_pins.json' FORK.md 2>/dev/null; then
    missing+=("FORK.md (must name limits_pins.json sidecar)")
  fi
  if [[ -f AGENTS.md ]] && ! grep -q 'stay-supergrok' AGENTS.md 2>/dev/null; then
    missing+=("AGENTS.md (must name stay-supergrok fail-open / named /limits commands)")
  fi
  guide="crates/codegen/xai-grok-pager/docs/user-guide/08-skills.md"
  if [[ -f $guide ]] && ! grep -q 'not a Python runtime' "$guide" 2>/dev/null; then
    missing+=("$guide (must say product skills are not a Python runtime)")
  fi
  # Product skill roots in this tree. Host ~/.agents/skills is operator-owned.
  allowed_product_skill_py() {
    local rest="$1"
    case "$rest" in
      implement/scripts/memory.py|execute-plan/scripts/validate-plan.py|shared/resume-session/session_reader.py)
        return 0
        ;;
      docx/*|pptx/*|xlsx/*|pdf/*)
        [[ "$rest" == *.py ]] && return 0
        ;;
    esac
    return 1
  }
  while IFS= read -r -d '' py; do
    rel="${py#./}"
    rest="${rel#.agents/skills/}"
    rest="${rest#.grok/skills/}"
    if ! allowed_product_skill_py "$rest"; then
      missing+=("$rel (non-excepted Python under a product skill root)")
    fi
  done < <(find .agents/skills .grok/skills -name '*.py' -print0 2>/dev/null || true)
fi

if ((${#warn[@]})); then
  echo "WARN:" >&2
  for w in "${warn[@]}"; do
    echo "  - $w" >&2
  done
fi

if ((${#missing[@]})); then
  echo "FAIL: process-pin paths missing (${#missing[@]}):" >&2
  for m in "${missing[@]}"; do
    echo "  - $m" >&2
  done
  echo >&2
  echo "After import: ensure paths are in FORK_PATHS (scripts/import-upstream-export.sh)." >&2
  echo "After onto: re-apply from origin/main or cherry-pick the product commits that added them." >&2
  echo "Research: doc/dev/research/fork-paths-hardening-2026-07-24.md" >&2
  exit 1
fi

echo "OK: all required process-pin paths present (${#REQUIRED_FILES[@]} files + ${#REQUIRED_DIRS[@]} dirs)."
if ((${#warn[@]})); then
  exit 0  # warnings only
fi
exit 0
