#!/usr/bin/env bash
set -euo pipefail

# detect-tools.sh — Detect OpenCode first-party model availability.

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
QUOTA_FILE="${REPO_ROOT}/.autoship/quota.json"

available=false
version=""
if command -v opencode >/dev/null 2>&1; then
  available=true
  version=$(opencode --version 2>/dev/null | head -1 || echo "unknown")
fi

json=$(jq -n \
  --argjson available "$available" \
  --arg version "$version" \
  '{opencode: {available: $available, version: $version, quota_pct: -1, quota_source: "provider"}}')

echo "$json"

mkdir -p "$REPO_ROOT/.autoship"
echo "$json" | jq '.' > "${QUOTA_FILE}.tmp" 2>/dev/null && mv "${QUOTA_FILE}.tmp" "$QUOTA_FILE" || true
