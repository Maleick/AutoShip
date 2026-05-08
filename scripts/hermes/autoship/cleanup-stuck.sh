#!/bin/bash
# AutoShip stuck worker cleanup
set -euo pipefail
cd /home/kara/projects/AutoShip || exit 1
bash hooks/hermes/cleanup-worktrees.sh 2>&1 || true
