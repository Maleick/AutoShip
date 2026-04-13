#!/usr/bin/env bash
# measure-baselines.sh — time TextQuest build and test targets, output JSON, append to .autoship/baselines.json
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINES_FILE="$REPO_ROOT/.autoship/baselines.json"

time_cmd() {
    local start end elapsed
    start=$(date +%s)
    eval "$@" > /dev/null 2>&1
    end=$(date +%s)
    elapsed=$((end - start))
    echo "$elapsed"
}

echo "Measuring TextQuest performance baselines..." >&2
echo "Repo: $REPO_ROOT" >&2
echo "" >&2

cd "$REPO_ROOT"

echo "  [1/3] cargo build (debug)..." >&2
DEBUG_BUILD_S=$(time_cmd "cargo build")
echo "        ${DEBUG_BUILD_S}s" >&2

echo "  [2/3] cargo test -p textquest (unit tests)..." >&2
UNIT_TESTS_S=$(time_cmd "cargo test -p textquest")
echo "        ${UNIT_TESTS_S}s" >&2

echo "  [3/3] cargo test (full suite)..." >&2
FULL_SUITE_S=$(time_cmd "cargo test")
echo "        ${FULL_SUITE_S}s" >&2

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

JSON=$(printf '{"debug_build_s":%d,"unit_tests_s":%d,"full_suite_s":%d,"timestamp":"%s"}' \
    "$DEBUG_BUILD_S" "$UNIT_TESTS_S" "$FULL_SUITE_S" "$TIMESTAMP")

echo "$JSON"

# Append to .autoship/baselines.json (creates file if missing)
if [ ! -f "$BASELINES_FILE" ]; then
    echo "[$JSON]" > "$BASELINES_FILE"
else
    # Read existing content, strip trailing ']', append new entry
    existing=$(cat "$BASELINES_FILE")
    # Handle empty array
    if [ "$existing" = "[]" ]; then
        echo "[$JSON]" > "$BASELINES_FILE"
    else
        # Insert before closing bracket
        trimmed="${existing%]}"
        echo "${trimmed},$JSON]" > "$BASELINES_FILE"
    fi
fi

echo "" >&2
echo "Appended to $BASELINES_FILE" >&2
