#!/bin/bash
# AutoResearch auto-update
set -euo pipefail
cd /home/kara/projects/AutoResearch || exit 1
git fetch origin
AHEAD=$(git rev-list HEAD..origin/main --count)
if [ "$AHEAD" -gt 0 ]; then
  git pull origin main
  echo "AutoResearch updated: $AHEAD new commits"
else
  echo "AutoResearch: already up to date"
fi
