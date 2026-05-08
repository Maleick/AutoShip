#!/bin/bash
# AutoShip auto-update
set -euo pipefail
cd /home/kara/projects/AutoShip || exit 1
git fetch origin
AHEAD=$(git rev-list HEAD..origin/main --count)
if [ "$AHEAD" -gt 0 ]; then
  git pull origin main
  echo "AutoShip updated: $AHEAD new commits"
else
  echo "AutoShip: already up to date"
fi
