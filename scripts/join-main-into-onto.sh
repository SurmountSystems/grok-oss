#!/usr/bin/env bash
# Join Surmount `main` into an onto-xai/* tip so the tip becomes a descendant
# of Surmount history (GitHub compare / PR onto → main works).
#
# Uses merge strategy **ours**: records `main` as a second parent and **keeps
# the current tip tree 100%** (xAI export + product stack). This is not a
# content fold of older main over a newer export.
#
# Prerequisites:
#   - Clean worktree (or ALLOW_DIRTY=1)
#   - On onto-xai/<short> (or ONTO_REF / current branch that already has
#     product stacked on an xAI tip via put-history)
#   - main is NOT already an ancestor (else no-op)
#
# Does NOT push. Does NOT rewrite main. Does NOT touch xai-org.
# Creates a merge commit only when DO_COMMIT=1 and signing works; otherwise
# leaves the merge staged (--no-commit) for a human TTY:
#   git commit -S -m "Merge Surmount main into onto-xai (keep tip tree)" ...
#
# Usage:
#   ./scripts/join-main-into-onto.sh
#   MAIN_REF=origin/main ./scripts/join-main-into-onto.sh
#   DO_COMMIT=1 ./scripts/join-main-into-onto.sh   # try signed commit here
#   FORCE=1 ./scripts/join-main-into-onto.sh       # re-join even if main is ancestor
#
# Env:
#   MAIN_REF       Surmount archive tip (default: origin/main, else main)
#   ONTO_REF       tip branch to join into (default: current branch)
#   DO_COMMIT=1    attempt git commit after merge (needs GPG/TTY if gpgsign)
#   FORCE=1        join even when MAIN_REF is already an ancestor
#   ALLOW_DIRTY=1  allow dirty worktree
#   DRY_RUN=1      print plan only
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MAIN_REF="${MAIN_REF:-}"
ONTO_REF="${ONTO_REF:-}"
DO_COMMIT="${DO_COMMIT:-0}"
FORCE="${FORCE:-0}"
DRY_RUN="${DRY_RUN:-0}"

if [[ -n "$(git status --porcelain)" ]]; then
  if [[ "${ALLOW_DIRTY:-}" == "1" ]]; then
    echo "WARN: dirty worktree allowed via ALLOW_DIRTY=1" >&2
  else
    echo "error: working tree is dirty. Commit/stash first (or ALLOW_DIRTY=1)." >&2
    git status --porcelain | head -40 >&2
    exit 1
  fi
fi

if [[ -f .git/MERGE_HEAD ]]; then
  echo "error: merge already in progress. Finish or abort first:" >&2
  echo "  git commit -S   # or: git merge --abort" >&2
  exit 1
fi

if [[ -z "$MAIN_REF" ]]; then
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    MAIN_REF=origin/main
  else
    MAIN_REF=main
  fi
fi
MAIN_REF=$(git rev-parse --verify "$MAIN_REF")
MAIN_SHORT=$(git rev-parse --short=12 "$MAIN_REF")

if [[ -z "$ONTO_REF" ]]; then
  ONTO_REF=$(git branch --show-current || true)
  if [[ -z "$ONTO_REF" ]]; then
    echo "error: detached HEAD; set ONTO_REF=onto-xai/<short> or checkout a branch" >&2
    exit 1
  fi
fi

if [[ "$ONTO_REF" != onto-xai/* ]] && [[ "${ALLOW_NON_ONTO:-}" != "1" ]]; then
  echo "error: expected onto-xai/* branch (got: $ONTO_REF)." >&2
  echo "  Checkout onto-xai/<tip> or set ALLOW_NON_ONTO=1 if intentional." >&2
  exit 1
fi

git checkout "$ONTO_REF"
ONTO_TIP=$(git rev-parse HEAD)
ONTO_TREE=$(git rev-parse 'HEAD^{tree}')
ONTO_SHORT=$(git rev-parse --short=12 HEAD)

echo "=== Join Surmount main into onto (strategy ours) ==="
echo "Onto branch: $ONTO_REF @ $ONTO_SHORT"
echo "Onto tree:   $ONTO_TREE"
echo "Main ref:    $MAIN_REF ($MAIN_SHORT)"
echo

if git merge-base --is-ancestor "$MAIN_REF" HEAD 2>/dev/null; then
  if [[ "$FORCE" != "1" ]]; then
    echo "=== Already joined — main is an ancestor of HEAD (safe default) ==="
    echo "Tip:  $(git rev-parse HEAD)"
    echo "To force another ours-merge: FORCE=1 $0"
    exit 0
  fi
  echo "WARN: main already ancestor; FORCE=1 continues" >&2
fi

if [[ "$DRY_RUN" == "1" ]]; then
  echo "DRY_RUN=1 — would run:"
  echo "  git merge -s ours $MAIN_REF --allow-unrelated-histories --no-commit"
  echo "  verify tree == $ONTO_TREE"
  echo "  git commit -S  # human TTY when commit.gpgsign=true"
  exit 0
fi

# Fetch latest main if remote ref
if [[ "$MAIN_REF" == origin/* ]] || git rev-parse --verify "refs/remotes/$MAIN_REF" >/dev/null 2>&1; then
  git fetch origin main 2>/dev/null || true
  MAIN_REF=$(git rev-parse --verify "${MAIN_REF}")
fi

git merge -s ours "$MAIN_REF" --allow-unrelated-histories --no-commit \
  -m "Merge Surmount main into onto-xai (keep tip tree)"

NEW_TREE=$(git write-tree)
if [[ "$NEW_TREE" != "$ONTO_TREE" ]]; then
  echo "error: post-merge tree $NEW_TREE != pre-merge onto tree $ONTO_TREE" >&2
  echo "  aborting merge" >&2
  git merge --abort
  exit 1
fi

echo "Tree identity OK: $NEW_TREE"
echo

if [[ "$DO_COMMIT" == "1" ]]; then
  if git commit -S \
    -m "Merge Surmount main into onto-xai (keep tip tree)" \
    -m "Join Surmount archive history so main is an ancestor of this tip." \
    -m "Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main."; then
    echo "=== Merge committed ==="
    git log --oneline --graph -8
  else
    echo "error: commit failed (GPG/TTY?). Merge is still staged." >&2
    echo "On a real TTY run:" >&2
    echo "  git commit -S -m \"Merge Surmount main into onto-xai (keep tip tree)\" \\" >&2
    echo "    -m \"Join Surmount archive history so main is an ancestor of this tip.\" \\" >&2
    echo "    -m \"Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main.\"" >&2
    exit 1
  fi
else
  echo "=== Merge staged (--no-commit); tree kept ==="
  echo "Pre-merge tip: $ONTO_TIP"
  echo "On a real TTY (signed):"
  echo "  git commit -S -m \"Merge Surmount main into onto-xai (keep tip tree)\" \\"
  echo "    -m \"Join Surmount archive history so main is an ancestor of this tip.\" \\"
  echo "    -m \"Strategy ours: retain onto tree (xAI tip + product). Enables normal PR onto → main.\""
  echo
  echo "Then verify:"
  echo "  git merge-base --is-ancestor $MAIN_SHORT HEAD"
  echo "  test \"\$(git rev-parse HEAD^{tree})\" = \"$ONTO_TREE\""
fi
