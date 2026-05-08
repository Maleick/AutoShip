#!/bin/bash
# Code-Archaeology auto-update
set -euo pipefail
cd /home/kara/projects/Code-Archaeology || exit 1
git fetch origin
AHEAD=$(git rev-list HEAD..origin/main --count)
if [ "$AHEAD" -gt 0 ]; then
  git pull origin main
  echo "Code-Archaeology updated: $AHEAD new commits"
else
  echo "Code-Archaeology: already up to date"
fi
