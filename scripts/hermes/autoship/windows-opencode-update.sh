#!/bin/bash
# Windows OpenCode auto-update
set -euo pipefail
cd /home/kara/.hermes/scripts || exit 1
python3 windows_bridge.py update-opencode 2>&1
echo "---"
python3 windows_bridge.py update-plugins 2>&1
