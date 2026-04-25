#!/usr/bin/env bash
# AutoShip worker initialization script
# Ensures worktrees are based on fresh origin/master to prevent stale code inheritance

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: init.sh <KEY> [--workspace-path PATH]

Initialize an AutoShip worker worktree on origin/master.

Arguments:
  KEY                 Unique identifier for this worker (e.g., "codex-appserver-001")
  --workspace-path    Optional path to workspace root (default: .autoship/workspaces)

Environment:
  AUTOSHIP_BRANCH     Override branch name (default: autoship/$KEY)

Example:
  init.sh codex-appserver-001
  init.sh my-worker --workspace-path /tmp/workers
EOF
  exit 1
}

[ "$#" -ge 1 ] || usage

KEY="$1"
WORKSPACE_PATH=".autoship/workspaces"

shift || true
while [ "$#" -gt 0 ]; do
  case "$1" in
    --workspace-path)
      shift
      [ "$#" -gt 0 ] || { echo "Error: --workspace-path requires a value" >&2; exit 1; }
      WORKSPACE_PATH="$1"
      ;;
    *)
      echo "Error: unknown argument: $1" >&2
      usage
      ;;
  esac
  shift || true
done

BRANCH_NAME="${AUTOSHIP_BRANCH:-autoship/$KEY}"
WORKTREE_PATH="$WORKSPACE_PATH/$KEY"

# Step 1: Fetch origin/master to ensure fresh state
echo "[init.sh] Fetching origin/master..."
git fetch origin master || {
  echo "Error: Failed to fetch origin/master" >&2
  exit 1
}

# Step 2: Check if local master is behind origin/master
LOCAL_MASTER=$(git rev-parse master 2>/dev/null || echo "")
REMOTE_MASTER=$(git rev-parse origin/master 2>/dev/null || echo "")

if [ -n "$LOCAL_MASTER" ] && [ -n "$REMOTE_MASTER" ] && [ "$LOCAL_MASTER" != "$REMOTE_MASTER" ]; then
  echo "[init.sh] WARNING: local master is behind origin/master" >&2
  echo "[init.sh]   Local:  $LOCAL_MASTER" >&2
  echo "[init.sh]   Remote: $REMOTE_MASTER" >&2
  echo "[init.sh]   Consider running: git checkout master && git pull origin master" >&2
fi

# Step 3: Create worktree based on origin/master (not local master)
echo "[init.sh] Creating worktree at $WORKTREE_PATH..."
mkdir -p "$(dirname "$WORKTREE_PATH")" || {
  echo "Error: Failed to create workspace parent directory" >&2
  exit 1
}

git worktree add "$WORKTREE_PATH" -b "$BRANCH_NAME" origin/master || {
  echo "Error: Failed to create worktree" >&2
  exit 1
}

echo "[init.sh] Worker initialized successfully at $WORKTREE_PATH"
echo "[init.sh] Branch: $BRANCH_NAME"
echo "[init.sh] Base: origin/master ($REMOTE_MASTER)"
