#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION_NAME="textquest-dev"

if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux is required for ./scripts/dev.sh. Install tmux and retry." >&2
  exit 1
fi

if tmux has-session -t "${SESSION_NAME}" 2>/dev/null; then
  echo "Attaching to existing '${SESSION_NAME}' session."
  exec tmux attach -t "${SESSION_NAME}"
fi

cd "${ROOT_DIR}"

tmux new-session -d -s "${SESSION_NAME}" "cargo run -p textquest-web"
tmux split-window -h -t "${SESSION_NAME}" "cargo run -p textquest"
tmux split-window -v -t "${SESSION_NAME}:0.1" "npm run dev --prefix web"
tmux select-layout -t "${SESSION_NAME}" tiled
tmux attach -t "${SESSION_NAME}"
