#!/usr/bin/env bash
# AutoShip worktree cleanup — safely remove completed/abandoned worktrees
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available
if [[ -f "$SCRIPT_DIR/../lib/common.sh" ]]; then
  source "$SCRIPT_DIR/../lib/common.sh"
else
  autoship_repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || {
      echo "Error: not inside a git repository" >&2
      return 1
    }
  }
fi

REPO_ROOT=$(autoship_repo_root) || exit 1
cd "$REPO_ROOT"

# ── Auto-sync: pull latest plugin code before cleanup ──
if [[ -z "${AUTOSHIP_NO_SYNC:-}" ]]; then
  # Only sync if we have a valid git remote (skip temp/policy-test repos)
  if git rev-parse --verify HEAD >/dev/null 2>&1 && git remote get-url origin >/dev/null 2>&1; then
    sync_gap=$(git log --oneline HEAD..origin/main 2>/dev/null | wc -l | tr -d ' ')
    if [[ "$sync_gap" =~ ^[0-9]+$ && "$sync_gap" -gt 0 ]]; then
      echo "[autoship-sync] $sync_gap commit(s) behind origin/main — pulling..."
      git pull origin main >/dev/null 2>&1 || echo "[autoship-sync] WARN: git pull failed, continuing with local code"
    fi
  fi
fi

AUTOSHIP_DIR=".autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"

# Resolve target repo path: config.json → env → auto-detect → legacy fallback
TARGET_REPO=""
if [[ -f "$AUTOSHIP_DIR/config.json" ]]; then
  REPO="$(jq -r '.repo // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)"
  if [[ -n "$REPO" && "$REPO" == */* ]]; then
    REPO_NAME="${REPO#*/}"
    TARGET_REPO="$HOME/Projects/${REPO_NAME%.git}"
  fi
fi
if [[ -z "$TARGET_REPO" && -n "${HERMES_TARGET_REPO_PATH:-}" ]]; then
  TARGET_REPO="$HERMES_TARGET_REPO_PATH"
fi
if [[ -z "$TARGET_REPO" && -n "${HERMES_TARGET_REPO:-}" ]]; then
  REPO_NAME="${HERMES_TARGET_REPO#*/}"
  TARGET_REPO="$HOME/Projects/${REPO_NAME%.git}"
fi
if [[ -z "$TARGET_REPO" ]]; then
  CURRENT_REMOTE="$(git remote get-url origin 2>/dev/null || true)"
  if [[ "$CURRENT_REMOTE" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
    REPO_NAME="${BASH_REMATCH[2]}"
    REPO_NAME="${REPO_NAME%.git}"
    TARGET_REPO="$HOME/Projects/$REPO_NAME"
  fi
fi
TARGET_REPO="${TARGET_REPO:-$HOME/Projects/TextQuest}"
DRY_RUN=false
VERBOSE=false

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=true ;;
    --verbose) VERBOSE=true ;;
  esac
done

log() {
  [[ "$VERBOSE" == true ]] && echo "$@"
}

cleanup_count=0
skipped_count=0
error_count=0

# Phase 1: Clean AutoShip workspace directories
log "=== Phase 1: AutoShip workspace cleanup ==="
for ws_dir in "$WORKSPACES_DIR"/issue-*; do
  if [[ ! -d "$ws_dir" ]]; then
    continue
  fi

  issue_key=$(basename "$ws_dir")
  issue_num=$(echo "$issue_key" | sed 's/issue-//')
  status_file="$ws_dir/status"
  # Strip \r from CRLF line endings before reading status
  status=$(cat "$status_file" 2>/dev/null | tr -d '\r' || echo "unknown")

  # Remove terminal states only. STUCK remains retryable and must be preserved.
  case "$status" in
    COMPLETE | BLOCKED | unknown)
      if [[ "$DRY_RUN" == true ]]; then
        echo "[DRY-RUN] Would remove workspace: $issue_key (status=$status)"
      else
        rm -rf "$ws_dir"
        log "  Removed workspace: $issue_key (status=$status)"
      fi
      cleanup_count=$((cleanup_count + 1))
      ;;
    *)
      log "  Skipping workspace: $issue_key (status=$status, active)"
      skipped_count=$((skipped_count + 1))
      ;;
  esac
done

# Phase 2: Clean git worktrees in target repo
log ""
log "=== Phase 2: Git worktree cleanup ==="
if [[ -d "$TARGET_REPO" ]]; then
  cd "$TARGET_REPO"

  for wt_info in $(git worktree list --porcelain 2>/dev/null | grep -E "^worktree " | awk '{print $2}'); do
    if [[ ! "$wt_info" =~ \.worktrees/issue-[0-9]+$ ]]; then
      continue
    fi

    wt_path="$wt_info"
    issue_num=$(basename "$wt_path" | sed 's/issue-//')

    # Check if issue is still active (autoship:ready or autoship:running)
    is_active=false
    if command -v gh &>/dev/null; then
      labels=$(gh issue view "$issue_num" --json labels --jq '[.labels[].name]' 2>/dev/null) || {
        log "  Skipping worktree: issue-$issue_num (could not verify GitHub labels)"
        skipped_count=$((skipped_count + 1))
        continue
      }
      if echo "$labels" | grep -qE "autoship:ready|autoship:running"; then
        is_active=true
      fi
    fi

    # Check if worktree is dirty (has uncommitted changes)
    is_dirty=false
    if [[ -d "$wt_path" ]]; then
      if git -C "$wt_path" status --short 2>/dev/null | grep -q .; then
        is_dirty=true
      fi
    fi

    if [[ "$is_active" == "true" ]]; then
      log "  Skipping worktree: issue-$issue_num (still active in queue)"
      skipped_count=$((skipped_count + 1))
      continue
    fi

    if [[ "$is_dirty" == "true" ]]; then
      log "  Skipping worktree: issue-$issue_num (dirty — uncommitted changes)"
      skipped_count=$((skipped_count + 1))
      continue
    fi

    if [[ "$DRY_RUN" == true ]]; then
      echo "[DRY-RUN] Would remove worktree: $wt_path (issue-$issue_num)"
    else
      git worktree remove "$wt_path" --force 2>/dev/null || rm -rf "$wt_path" 2>/dev/null || {
        log "  ⚠️ Could not remove worktree: $wt_path"
        error_count=$((error_count + 1))
        continue
      }
      log "  Removed worktree: $wt_path (issue-$issue_num)"
    fi
    cleanup_count=$((cleanup_count + 1))
  done
else
  log "  Target repo not found: $TARGET_REPO"
fi

# Phase 3: Prune git worktree metadata
log ""
log "=== Phase 3: Prune stale worktree metadata ==="
cd "$TARGET_REPO" 2>/dev/null && git worktree prune 2>/dev/null || true

echo ""
echo "=== Cleanup Summary ==="
echo "Removed: $cleanup_count"
echo "Skipped (active/dirty): $skipped_count"
[[ $error_count -gt 0 ]] && echo "Errors: $error_count"
echo ""

if [[ "$DRY_RUN" == true ]]; then
  echo "This was a dry run. No files were actually removed."
  echo "Run without --dry-run to execute cleanup."
fi
