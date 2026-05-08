#!/bin/bash
# AutoShip continuous runner — keep the pipeline flowing every 15 minutes
set -euo pipefail

AUTOSHIP_DIR="/home/kara/projects/AutoShip/.autoship"
REPO_ROOT="/home/kara/projects/AutoShip"

# Step 1: Run the Hermes runner to dispatch any queued workers
cd "$REPO_ROOT"
bash hooks/hermes/runner.sh 2>&1 || true

# Step 2: Check for stuck workers and retry them
cd "$REPO_ROOT"
bash hooks/hermes/reconcile-state.sh 2>&1 || true

# Step 3: Run stuck worker cleanup if needed
if [[ -f "$AUTOSHIP_DIR/event-queue.json" ]]; then
    stuck_count=$(python3 -c "
import json
with open('$AUTOSHIP_DIR/event-queue.json') as f:
    q = json.load(f)
print(len([x for x in q if x.get('type') == 'stuck']))
" 2>/dev/null || echo "0")
    if [[ "$stuck_count" -gt 0 ]]; then
        echo "$stuck_count stuck issues found, running cleanup..."
        cd "$REPO_ROOT"
        bash hooks/hermes/cleanup-worktrees.sh 2>&1 || true
    fi
fi

# Step 4: Check if we need to dispatch more issues
running=$(cd "$REPO_ROOT" && bash hooks/hermes/runner.sh 2>&1 | grep -oP '\d+ running' | grep -oP '\d+' || echo "0")
max=5
available=$((max - running))

if [[ "$available" -gt 0 ]]; then
    echo "$available slot(s) available, checking for queued issues..."
    cd "$REPO_ROOT"
    bash hooks/hermes/dispatch.sh 2>&1 || true
fi

echo "AutoShip continuous runner completed"
