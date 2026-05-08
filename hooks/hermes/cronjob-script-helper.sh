#!/usr/bin/env bash
# AutoShip cronjob script helper — write no-agent scripts to disk and return relative path
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

AUTOSHIP_SCRIPTS_DIR="${AUTOSHIP_SCRIPTS_DIR:-$HOME/.hermes/scripts/autoship}"
AUTOSHIP_TEMPLATE_DIR="${AUTOSHIP_TEMPLATE_DIR:-$REPO_ROOT/scripts/hermes/autoship}"

usage() {
  echo "Usage: $0 <name> <script_content_file>"
  echo "  name: base name for the script (e.g. 'continuous-runner')"
  echo "  script_content_file: path to file containing the bash script content"
  echo ""
  echo "Writes script to ~/.hermes/scripts/autoship/<name>.sh, chmod +x, returns relative path."
  exit 1
}

if [[ $# -lt 2 ]]; then
  usage
fi

name="$1"
content_file="$2"

if [[ ! -f "$content_file" ]]; then
  echo "Error: content file not found: $content_file" >&2
  exit 1
fi

# Ensure scripts directory exists
mkdir -p "$AUTOSHIP_SCRIPTS_DIR"

script_path="$AUTOSHIP_SCRIPTS_DIR/${name}.sh"

# If a template exists in the repo, use it as base; otherwise use provided content
if [[ -f "$AUTOSHIP_TEMPLATE_DIR/${name}.sh" ]]; then
  cat "$AUTOSHIP_TEMPLATE_DIR/${name}.sh" | sed 's/\r$//' >"$script_path"
else
  cat "$content_file" | sed 's/\r$//' >"$script_path"
fi
chmod +x "$script_path"

# Verify syntax
if ! bash -n "$script_path" 2>/dev/null; then
  echo "Error: script has syntax errors: $script_path" >&2
  rm -f "$script_path"
  exit 1
fi

# Return relative path from ~/.hermes/scripts/
# The cron scheduler resolves relative to ~/.hermes/scripts/
rel_path="autoship/${name}.sh"
echo "$rel_path"
