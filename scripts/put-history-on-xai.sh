#!/usr/bin/env bash
# Put Surmount commits ON TOP OF the current xAI export tip — for real.
#
# Real `git cherry-pick` onto xai-org/main. Conflicts stop for you to resolve:
#   git add -u && git cherry-pick --continue
#   CONTINUE=1 ./scripts/put-history-on-xai.sh
#
# SAFETY (until the stack is merged / ready):
#   - If onto-xai/<tip> already exists and is a descendant of the xAI tip,
#     the script EXITS 0 and does nothing. It will NOT delete your work.
#   - To rebuild from scratch: FORCE=1 ./scripts/put-history-on-xai.sh
#   - Never run this while mid cherry-pick without finishing or aborting first.
#
# Does NOT push. Does NOT rewrite Surmount main/merge-2. Does NOT touch xai-org.
#
# Usage:
#   ./scripts/put-history-on-xai.sh                 # create stack if missing
#   SURMOUNT_REF=merge-2 ./scripts/put-history-on-xai.sh
#   FORCE=1 SURMOUNT_REF=merge-2 ./scripts/put-history-on-xai.sh
#   CONTINUE=1 ./scripts/put-history-on-xai.sh
#
# Env:
#   SURMOUNT_REF     tip to take commits from (default: merge-2, else origin/main)
#   SEED_REF         exclusive lower bound (default: import-log seed / b189869)
#   FORCE=1          delete and rebuild onto-xai/* even if it already looks good
#   CONTINUE=1       resume after conflict resolution
#   ALLOW_DIRTY=1    allow dirty worktree
#   FIRST_PARENT=1   only first-parent commits (default 0 = no-merges)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if git remote get-url xai-org >/dev/null 2>&1; then
  UPSTREAM_REMOTE="${UPSTREAM_REMOTE:-xai-org}"
elif git remote get-url upstream >/dev/null 2>&1; then
  UPSTREAM_REMOTE="${UPSTREAM_REMOTE:-upstream}"
else
  echo "error: add remote xai-org or upstream first" >&2
  exit 1
fi

UPSTREAM_BRANCH="${UPSTREAM_BRANCH:-main}"
SURMOUNT_REF="${SURMOUNT_REF:-}"
SEED_REF="${SEED_REF:-}"
IMPORT_LOG="${IMPORT_LOG:-docs/upstream-import-log.md}"
FIRST_PARENT="${FIRST_PARENT:-0}"
CONTINUE="${CONTINUE:-0}"
FORCE="${FORCE:-0}"

ORIGINAL_BRANCH="$(git branch --show-current || true)"
ORIGINAL_HEAD="$(git rev-parse HEAD)"

# Mid cherry-pick: never start a fresh rebuild.
if [[ -f .git/CHERRY_PICK_HEAD ]] || [[ -d .git/sequencer ]]; then
  if [[ "$CONTINUE" == "1" ]]; then
    echo "error: cherry-pick still in progress. Finish it first:" >&2
    echo "  git add -u && git cherry-pick --continue" >&2
    echo "  then: CONTINUE=1 $0" >&2
    exit 1
  fi
  echo "error: cherry-pick in progress. Do one of:" >&2
  echo "  # finish current pick" >&2
  echo "  git add -u && git cherry-pick --continue && CONTINUE=1 $0" >&2
  echo "  # or abort and restore a known tip" >&2
  echo "  git cherry-pick --abort" >&2
  echo "  git checkout -B onto-xai/\$(git rev-parse --short=12 xai-org/main) backup/onto-xai-resolved-a335358" >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]] && [[ "$CONTINUE" != "1" ]]; then
  if [[ "${ALLOW_DIRTY:-}" == "1" ]]; then
    echo "WARN: dirty worktree allowed via ALLOW_DIRTY=1" >&2
  else
    echo "error: working tree is dirty. Commit/stash first (or ALLOW_DIRTY=1)." >&2
    git status --porcelain | head -40 >&2
    exit 1
  fi
fi

git fetch "$UPSTREAM_REMOTE" "$UPSTREAM_BRANCH" --force
git fetch origin main 2>/dev/null || true

if [[ $# -ge 1 ]]; then
  XAI_TIP=$(git rev-parse "$1")
else
  XAI_TIP=$(git rev-parse "$UPSTREAM_REMOTE/$UPSTREAM_BRANCH")
fi
XAI_SHORT=$(git rev-parse --short=12 "$XAI_TIP")
BRANCH="onto-xai/$XAI_SHORT"

# Surmount tip: never default to "current onto-xai branch" (that would re-stack
# the onto branch onto itself / fall back to origin/main incorrectly).
if [[ -z "$SURMOUNT_REF" ]]; then
  if [[ -n "$ORIGINAL_BRANCH" \
    && "$ORIGINAL_BRANCH" != onto-xai/* \
    && "$ORIGINAL_BRANCH" != import/* \
    && "$ORIGINAL_BRANCH" != backup/* ]]; then
    SURMOUNT_REF="$ORIGINAL_HEAD"
    SURMOUNT_LABEL="$ORIGINAL_BRANCH"
  elif git show-ref --verify --quiet refs/heads/merge-2; then
    SURMOUNT_REF=merge-2
    SURMOUNT_LABEL=merge-2
  elif git show-ref --verify --quiet refs/remotes/origin/main; then
    SURMOUNT_REF=origin/main
    SURMOUNT_LABEL=origin/main
  else
    SURMOUNT_REF=main
    SURMOUNT_LABEL=main
  fi
else
  SURMOUNT_LABEL="$SURMOUNT_REF"
fi
SURMOUNT_REF=$(git rev-parse --verify "$SURMOUNT_REF")
SURMOUNT_SHORT=$(git rev-parse --short=12 "$SURMOUNT_REF")

if [[ -z "$SEED_REF" ]]; then
  if [[ -f "$IMPORT_LOG" ]]; then
    SEED_REF=$(
      grep -E '^\| 20' "$IMPORT_LOG" | grep -i seed | head -1 \
        | grep -oE '`[0-9a-f]{40}`' | head -1 | tr -d '`' || true
    )
  fi
  if [[ -z "$SEED_REF" ]]; then
    SEED_REF=b189869b7755d2b482969acf6c92da3ecfeffd36
  fi
fi
SEED_REF=$(git rev-parse "$SEED_REF")

if ! git merge-base --is-ancestor "$SEED_REF" "$SURMOUNT_REF" 2>/dev/null; then
  echo "error: SEED_REF not ancestor of SURMOUNT_REF" >&2
  exit 1
fi

# --- if stack already exists and looks good, leave it alone ---
if [[ "$CONTINUE" != "1" && "$FORCE" != "1" ]] \
  && git show-ref --verify --quiet "refs/heads/$BRANCH"; then
  existing=$(git rev-parse "$BRANCH")
  if git merge-base --is-ancestor "$XAI_TIP" "$existing" 2>/dev/null; then
    ahead=$(git rev-list --count "$XAI_TIP..$existing")
    if [[ "$ahead" -gt 0 ]]; then
      echo "=== Stack already present — not rebuilding (safe default) ==="
      echo "Branch:  $BRANCH"
      echo "Tip:     $existing ($(git rev-parse --short "$existing"))"
      echo "xAI tip: $XAI_TIP (ancestor: yes)"
      echo "Ahead:   $ahead commit(s)"
      echo
      git log --oneline "$XAI_TIP..$BRANCH" | head -20
      echo
      echo "This is intentional until the stack is merged/ready."
      echo "To rebuild from scratch (DESTRUCTIVE): FORCE=1 SURMOUNT_REF=$SURMOUNT_LABEL $0"
      echo "Backup of last good tip (if present): backup/onto-xai-resolved-a335358"
      # Ensure we're on the good branch, not left detached.
      if [[ "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)" != "$BRANCH" ]]; then
        git checkout "$BRANCH"
      fi
      exit 0
    fi
  fi
fi

# Commit list
if [[ "$FIRST_PARENT" == "1" ]]; then
  mapfile -t COMMITS < <(git rev-list --reverse --first-parent "$SEED_REF..$SURMOUNT_REF")
else
  mapfile -t COMMITS < <(git rev-list --reverse --no-merges "$SEED_REF..$SURMOUNT_REF")
fi

if [[ ${#COMMITS[@]} -eq 0 ]]; then
  echo "error: no commits to cherry-pick between $SEED_REF and $SURMOUNT_REF" >&2
  exit 1
fi

echo "=== REAL cherry-pick: Surmount → on top of xAI ==="
echo "Checkout was: ${ORIGINAL_BRANCH:-detached} ($ORIGINAL_HEAD)"
echo "xAI tip:      $XAI_TIP ($XAI_SHORT)"
echo "Stacking:     $SURMOUNT_LABEL @ $SURMOUNT_SHORT"
echo "Seed:         $SEED_REF"
echo "Commits:      ${#COMMITS[@]}"
echo "Branch:       $BRANCH"
echo "FORCE:        $FORCE"
echo

backup_existing_branch() {
  if git show-ref --verify --quiet "refs/heads/$BRANCH"; then
    local bak="backup/${BRANCH//\//-}-$(git rev-parse --short "$BRANCH")-$(date -u +%Y%m%dT%H%M%SZ)"
    git branch "$bak" "$BRANCH"
    echo "Backed up previous $BRANCH → $bak"
  fi
}

if [[ "$CONTINUE" == "1" ]]; then
  if ! git show-ref --verify --quiet "refs/heads/$BRANCH"; then
    echo "error: $BRANCH missing; cannot CONTINUE" >&2
    exit 1
  fi
  git checkout "$BRANCH"
  # Skip commits already present via cherry-pick -x trailer or identical subject+tree is hard;
  # use cherry-pick source trailer "cherry picked from commit <sha>"
  done_list=$(
    git log --format=%B "$XAI_TIP..HEAD" \
      | grep -E 'cherry picked from commit [0-9a-f]{40}' \
      | sed -E 's/.*cherry picked from commit ([0-9a-f]{40}).*/\1/' || true
  )
  remaining=()
  for c in "${COMMITS[@]}"; do
    if echo "$done_list" | grep -qx "$c"; then
      echo "  skip already applied: $(git rev-parse --short "$c") $(git log -1 --format=%s "$c")"
      continue
    fi
    remaining+=("$c")
  done
  COMMITS=("${remaining[@]}")
  if [[ ${#COMMITS[@]} -eq 0 ]]; then
    echo "Nothing left to cherry-pick. Done."
    git log --oneline "$XAI_TIP..HEAD"
    exit 0
  fi
  echo "Continuing with ${#COMMITS[@]} remaining commit(s)"
else
  if git show-ref --verify --quiet "refs/heads/$BRANCH"; then
    if [[ "$FORCE" != "1" ]]; then
      echo "error: $BRANCH exists. Refusing to delete (set FORCE=1 to rebuild)." >&2
      exit 1
    fi
    backup_existing_branch
    echo "FORCE=1: replacing $BRANCH ($(git rev-parse --short "$BRANCH"))"
    if [[ "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)" == "$BRANCH" ]]; then
      git checkout --detach HEAD
    fi
    git branch -D "$BRANCH"
  fi
  git checkout -B "$BRANCH" "$XAI_TIP"
fi

for c in "${COMMITS[@]}"; do
  subj=$(git log -1 --format=%s "$c")
  short=$(git rev-parse --short "$c")
  echo ">>> cherry-pick $short $subj"
  if git cherry-pick -x "$c"; then
    echo "    ok → $(git rev-parse --short HEAD)"
  else
    echo
    echo "CONFLICT while cherry-picking $short ($subj)"
    echo "Resolve every conflict, then:"
    echo "  git add -u"
    echo "  git cherry-pick --continue"
    echo "  CONTINUE=1 $0"
    echo "Or abort and restore backup:"
    echo "  git cherry-pick --abort"
    echo "  git checkout -B $BRANCH backup/onto-xai-resolved-a335358  # if that backup exists"
    echo
    echo "Unmerged:"
    git diff --name-only --diff-filter=U || true
    exit 2
  fi
done

echo
echo "=== Done (real stack) ==="
echo "Branch: $BRANCH"
echo "Tip:    $(git rev-parse HEAD)"
echo "xAI is ancestor: $(git merge-base --is-ancestor "$XAI_TIP" HEAD && echo yes || echo NO)"
echo "Commits on top of xAI:"
git log --oneline "$XAI_TIP..HEAD"
echo
echo "Diff vs xAI tip (summary):"
git diff --stat "$XAI_TIP" HEAD | tail -20
echo
echo "Surmount product branches were NOT modified."
echo "XAI_TIP=$XAI_TIP"
echo "ONTO_BRANCH=$BRANCH"
echo "ONTO_TIP=$(git rev-parse HEAD)"
echo "SURMOUNT_REF=$SURMOUNT_REF"
