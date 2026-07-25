#!/usr/bin/env bash
# Print Surmount recon status (onto / cherry-pick / merge / next human action).
# Read-only: never commits, aborts, FORCE rebuilds, or invents overlay modes.
#
# Usage:
#   ./scripts/recon-status.sh
#   just recon-status
#
# Prefer this over ad-hoc git probes for recon:status.
# Living law: docs/upstream-history.md § HITL runbook + § Live stack
# Skill:     ~/.agents/skills/git-recon/SKILL.md (recon:status)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! git rev-parse --git-dir >/dev/null 2>&1; then
  echo "error: not a git repository (cwd=$ROOT)" >&2
  exit 2
fi

# Worktree-safe paths (not bare .git/CHERRY_PICK_HEAD assumptions).
cherry_path=$(git rev-parse --git-path CHERRY_PICK_HEAD)
merge_path=$(git rev-parse --git-path MERGE_HEAD)
sequencer_path=$(git rev-parse --git-path sequencer)

branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)
if [[ -z "$branch" || "$branch" == "HEAD" ]]; then
  branch="DETACHED@$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
fi

cherry_pick=no
if [[ -f "$cherry_path" ]]; then
  cherry_pick=yes
fi

merge_head=no
if [[ -f "$merge_path" ]]; then
  merge_head=yes
fi

sequencer=no
if [[ -d "$sequencer_path" ]]; then
  sequencer=yes
fi

mapfile -t unmerged < <(git diff --name-only --diff-filter=U 2>/dev/null || true)
unmerged_count=${#unmerged[@]}
# Drop empty single element if diff printed nothing
if [[ "$unmerged_count" -eq 1 && -z "${unmerged[0]:-}" ]]; then
  unmerged=()
  unmerged_count=0
fi

onto_ish=no
onto_name=""
if [[ "$branch" == onto-xai/* ]]; then
  onto_ish=yes
  onto_name="$branch"
fi

main_ancestor=unknown
if git rev-parse --verify origin/main >/dev/null 2>&1; then
  if git merge-base --is-ancestor origin/main HEAD 2>/dev/null; then
    main_ancestor=yes
  else
    main_ancestor=no
  fi
elif git rev-parse --verify main >/dev/null 2>&1; then
  if git merge-base --is-ancestor main HEAD 2>/dev/null; then
    main_ancestor=yes
  else
    main_ancestor=no
  fi
fi

dirty=no
if [[ -n "$(git status --porcelain 2>/dev/null || true)" ]]; then
  dirty=yes
fi

# Recommended next human action only (plain English; no invented modes).
next=""
if ((unmerged_count > 0)); then
  if [[ "$cherry_pick" == "yes" || "$sequencer" == "yes" ]]; then
    next="resolve UU paths (spawn if multi-file), stage, then human: git cherry-pick --continue (signed TTY)"
  elif [[ "$merge_head" == "yes" ]]; then
    next="resolve UU paths, stage, then human: git commit -S (finish merge)"
  else
    next="resolve UU paths and stage; re-run recon-status for next step"
  fi
elif [[ "$cherry_pick" == "yes" || "$sequencer" == "yes" ]]; then
  next="human: git cherry-pick --continue (signed TTY); then CONTINUE=1 SURMOUNT_REF=origin/main ./scripts/put-history-on-xai.sh if stack continues"
elif [[ "$merge_head" == "yes" ]]; then
  next="human: git commit -S (join/merge already staged — do not invent new merge)"
elif [[ "$onto_ish" == "yes" && "$main_ancestor" == "no" ]]; then
  next="run ./scripts/join-main-into-onto.sh (stages -s ours), then human: git commit -S join message"
elif [[ "$onto_ish" == "yes" && "$main_ancestor" == "yes" ]]; then
  next="clean recon state (onto tip; main is ancestor). Land: ./scripts/assert-process-pins.sh HEAD && just check; push/PR only if asked"
else
  next="clean (not mid cherry-pick/merge). Route if needed: ./scripts/detect-upstream-export.sh or put-history / import (see git-recon recon:route)"
fi

echo "branch:           $branch"
echo "CHERRY_PICK_HEAD: $cherry_pick"
echo "MERGE_HEAD:       $merge_head"
echo "sequencer:        $sequencer"
echo "unmerged:         $unmerged_count"
if ((unmerged_count > 0)); then
  # Cap list noise; full list is always available via git diff --name-only --diff-filter=U
  max_show=40
  i=0
  for p in "${unmerged[@]}"; do
    ((i++)) || true
    if ((i > max_show)); then
      echo "  … and $((unmerged_count - max_show)) more"
      break
    fi
    echo "  - $p"
  done
fi
if [[ -n "$onto_name" ]]; then
  echo "onto-ish:         $onto_ish ($onto_name)"
else
  echo "onto-ish:         $onto_ish"
fi
echo "main_ancestor:    $main_ancestor"
echo "dirty_worktree:   $dirty"
echo "next:             $next"
