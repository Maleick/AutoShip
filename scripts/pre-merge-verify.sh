#!/usr/bin/env bash
# pre-merge-verify.sh — Pre-merge verification for TextQuest
# Runs fmt, clippy, tests, and Python checks before merging a branch.
# Usage: bash scripts/pre-merge-verify.sh [--strict] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

STRICT=0
for arg in "$@"; do
    case "$arg" in
        --strict) STRICT=1 ;;
        --help|-h)
            echo "Usage: $(basename "$0") [--strict]"
            echo ""
            echo "Runs pre-merge verification checks:"
            echo "  cargo fmt --check            Format check"
            echo "  cargo clippy --all           Lint (warnings as errors)"
            echo "  cargo test -p textquest      Orchestrator tests"
            echo "  python3 -m unittest discover Python tests"
            echo ""
            echo "Options:"
            echo "  --strict    Also run cargo test --all (full workspace suite)"
            echo "  --help      Show this help"
            exit 0
            ;;
    esac
done

cd "${REPO_ROOT}"

# ─── Result tracking ──────────────────────────────────────────────────────────
declare -a CHECK_NAMES=()
declare -a CHECK_STATUSES=()

record() {
    local status="$1"
    local name="$2"
    CHECK_NAMES+=("$name")
    CHECK_STATUSES+=("$status")
}

print_status() {
    local status="$1"
    local name="$2"
    if [ "$status" = "PASS" ]; then
        echo "  [PASS] $name"
    else
        echo "  [FAIL] $name"
    fi
}

# ─── Check helpers ────────────────────────────────────────────────────────────
run_check() {
    local name="$1"
    shift
    echo "Running: $name ..."
    if "$@" 2>&1; then
        record "PASS" "$name"
    else
        record "FAIL" "$name"
    fi
    echo ""
}

# ─── Checks ───────────────────────────────────────────────────────────────────
echo "=== Pre-merge verification ==="
echo "Repo: ${REPO_ROOT}"
echo "Strict: $([ "$STRICT" -eq 1 ] && echo yes || echo no)"
echo ""

run_check "cargo fmt --check" \
    cargo fmt --check

run_check "cargo clippy --all -D warnings" \
    cargo clippy --all -- -D warnings

run_check "cargo test -p textquest" \
    cargo test -p textquest

run_check "python3 -m unittest discover" \
    python3 -m unittest discover -s tests -p 'test_*.py' -v

if [ "$STRICT" -eq 1 ]; then
    run_check "cargo test --all (strict)" \
        cargo test --all
fi

# ─── Summary ──────────────────────────────────────────────────────────────────
echo "=== Summary ==="
PASS_COUNT=0
FAIL_COUNT=0

for i in "${!CHECK_NAMES[@]}"; do
    name="${CHECK_NAMES[$i]}"
    status="${CHECK_STATUSES[$i]}"
    print_status "$status" "$name"
    if [ "$status" = "PASS" ]; then
        (( PASS_COUNT++ )) || true
    else
        (( FAIL_COUNT++ )) || true
    fi
done

echo ""
echo "Result: ${PASS_COUNT} passed, ${FAIL_COUNT} failed"

if [ "$FAIL_COUNT" -eq 0 ]; then
    echo "All checks passed. Ready to merge."
    exit 0
else
    echo "Pre-merge checks failed. Fix issues above before merging."
    exit 1
fi
