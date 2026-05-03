#!/usr/bin/env bash
# Hermes agent cronjob dispatcher — create Hermes cron jobs for queued issues
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUTOSHIP_DIR="$REPO_ROOT/.autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"

# This script is called by the Hermes cron to dispatch work
# It reads the HERMES_PROMPT.md from a workspace and creates a Hermes cronjob

issue_key="${1:?Issue key required}"
workspace_dir="$WORKSPACES_DIR/$issue_key"

if [[ ! -d "$workspace_dir" ]]; then
  echo "Error: workspace not found: $workspace_dir" >&2
  exit 1
fi

if [[ ! -f "$workspace_dir/HERMES_PROMPT.md" ]]; then
  echo "Error: HERMES_PROMPT.md not found in workspace" >&2
  exit 1
fi

prompt_file="$workspace_dir/HERMES_PROMPT.md"
worktree_path=$(cat "$workspace_dir/worktree-path.txt" 2>/dev/null || echo "$workspace_dir")

# Extract issue number from issue_key
issue_num=$(echo "$issue_key" | sed 's/issue-//')

# Create a Hermes cronjob for this issue
# The cronjob will run in the worktree and execute the prompt
cat <<EOF
# Hermes Cronjob for $issue_key
# Run this to create the cronjob:
# hermes cronjob create --name "autoship-$issue_key" --schedule "every 10m" --workdir "$worktree_path" --prompt-file "$prompt_file"
EOF

echo "Cronjob spec generated for $issue_key"
echo "Worktree: $worktree_path"
echo "Prompt: $prompt_file"
