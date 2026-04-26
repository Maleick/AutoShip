#!/usr/bin/env bash
set -euo pipefail

if ! command -v shfmt >/dev/null 2>&1; then
  echo "shfmt not installed" >&2
  exit 1
fi
shfmt -w -i 2 -ci -bn hooks/
