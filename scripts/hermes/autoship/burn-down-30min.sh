#!/bin/bash
# AutoShip burn-down dispatcher — 3 issues at a time, every 30 minutes
set -euo pipefail

AUTOSHIP_DIR="/home/kara/projects/AutoShip/.autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
REPO_ROOT="/home/kara/projects/AutoShip"

# Get list of issues that have HERMES_PROMPT.md but no status file (not yet started)
queued=()
for ws in $(ls -1 "$WORKSPACES_DIR" | grep '^issue-' | sort); do
    if [[ -f "$WORKSPACES_DIR/$ws/HERMES_PROMPT.md" ]]; then
        # Check if already has a running/completed status
        status_file="$WORKSPACES_DIR/$ws/status.json"
        if [[ ! -f "$status_file" ]]; then
            issue_num=$(echo "$ws" | sed 's/issue-//')
            queued+=("$issue_num")
        fi
    fi
done

# Also check event-queue.json for stuck issues
if [[ -f "$AUTOSHIP_DIR/event-queue.json" ]]; then
    stuck=$(python3 -c "
import json
with open('$AUTOSHIP_DIR/event-queue.json') as f:
    q = json.load(f)
for item in q:
    if item.get('status') == 'STUCK' or item.get('type') == 'stuck':
        issue = item['issue'].replace('issue-', '')
        print(issue)
" 2>/dev/null || true)
    for issue in $stuck; do
        if [[ ! " ${queued[@]} " =~ " ${issue} " ]]; then
            queued+=("$issue")
        fi
    done
fi

# Limit to 3 at a time
count=0
for issue in "${queued[@]}"; do
    if [[ $count -ge 3 ]]; then
        break
    fi
    
    # Check if already dispatched via cronjob
    existing=$(hermes cronjob list 2>/dev/null | grep "autoship-issue-$issue" || true)
    if [[ -n "$existing" ]]; then
        echo "Issue #$issue already has cronjob, skipping"
        continue
    fi
    
    echo "Dispatching issue #$issue..."
    cd "$REPO_ROOT"
    bash hooks/hermes/dispatch.sh "$issue" 2>&1 || true
    
    # Create cronjob for the worker
    ws_dir="$WORKSPACES_DIR/issue-$issue"
    if [[ -f "$ws_dir/HERMES_PROMPT.md" ]]; then
        prompt_file="$ws_dir/HERMES_PROMPT.md"
        worktree_path=$(cat "$ws_dir/worktree-path.txt" 2>/dev/null || echo "$ws_dir")
        
        # Check if cronjob already exists
        job_list=$(hermes cronjob list 2>/dev/null || echo "")
        if echo "$job_list" | grep -q "autoship-issue-$issue"; then
            echo "Cronjob for issue-$issue already exists"
        else
            echo "Creating cronjob for issue-$issue..."
            # Create via hermes cronjob create command
            hermes cron create \
                --name "autoship-issue-$issue" \
                --schedule "every 10m" \
                --workdir "$worktree_path" \
                --prompt-file "$prompt_file" \
                --no-agent 2>/dev/null || \
            echo "Note: hermes cron create not available, skipping cronjob creation"
        fi
    fi
    
    count=$((count + 1))
done

if [[ $count -eq 0 ]]; then
    echo "No new issues to dispatch. All caught up or max concurrency reached."
else
    echo "Dispatched $count issue(s)."
fi
