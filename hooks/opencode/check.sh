#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Error: not inside a git repository" >&2
  exit 1
}

HOOKS_DIR="$REPO_ROOT/hooks"

print_usage() {
  cat <<EOF
Usage: bash hooks/opencode/check.sh [OPTIONS]

Run all AutoShip pre-commit verification checks.

Options:
  --fast       Run only syntax checks (skip full test-policy and smoke-test)
  --policy     Run only test-policy.sh
  --smoke      Run only smoke-test.sh
  --syntax     Run only bash syntax checks
  --lint       Run shellcheck and shfmt checks
  -h, --help  Show this help message

Without options, runs all checks: syntax, test-policy, and smoke-test.
EOF
}

RUN_ALL=true
RUN_POLICY=true
RUN_SMOKE=true
RUN_SYNTAX=true
RUN_LINT=true

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fast)
      RUN_ALL=false
      RUN_POLICY=false
      RUN_SMOKE=false
      RUN_SYNTAX=true
      RUN_LINT=false
      shift
      ;;
    --policy)
      RUN_ALL=false
      RUN_SYNTAX=false
      RUN_SMOKE=false
      RUN_LINT=false
      shift
      ;;
    --smoke)
      RUN_ALL=false
      RUN_SYNTAX=false
      RUN_POLICY=false
      RUN_LINT=false
      shift
      ;;
    --syntax)
      RUN_ALL=false
      RUN_POLICY=false
      RUN_SMOKE=false
      RUN_LINT=false
      shift
      ;;
    --lint)
      RUN_POLICY=false
      RUN_SMOKE=false
      RUN_SYNTAX=false
      RUN_LINT=true
      shift
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      print_usage >&2
      exit 1
      ;;
  esac
done

FAILED=0

run_syntax_check() {
  echo "=== Bash syntax check ==="
  local syntax_failed=0
  for script in "$HOOKS_DIR"/*.sh; do
    [[ -f "$script" ]] || continue
    local output
    output=$(bash -n "$script" 2>&1) || true
    if [[ -n "$output" ]]; then
      echo "FAIL: syntax check failed for $script" >&2
      echo "$output" | head -5 >&2
      syntax_failed=1
    fi
  done
  for script in "$HOOKS_DIR/opencode"/*.sh; do
    [[ -f "$script" ]] || continue
    local output
    output=$(bash -n "$script" 2>&1) || true
    if [[ -n "$output" ]]; then
      echo "FAIL: syntax check failed for $script" >&2
      echo "$output" | head -5 >&2
      syntax_failed=1
    fi
  done
  if [[ $syntax_failed -ne 0 ]]; then
    FAILED=1
  fi
}

run_policy_check() {
  echo "=== Policy test ==="
  (
    set +e
    cd "$REPO_ROOT"
    exec bash "$HOOKS_DIR/opencode/test-policy.sh"
  ) || {
    echo "FAIL: policy test failed" >&2
    FAILED=1
  }
}

run_smoke_check() {
  echo "=== Smoke test ==="
  (
    set +e
    cd "$REPO_ROOT"
    exec bash "$HOOKS_DIR/opencode/smoke-test.sh"
  ) || {
    echo "FAIL: smoke test failed" >&2
    FAILED=1
  }
}

run_lint_check() {
  echo "=== Shell lint/format check ==="
  if command -v shellcheck >/dev/null 2>&1; then
    shellcheck "$HOOKS_DIR"/*.sh "$HOOKS_DIR/opencode"/*.sh || FAILED=1
  else
    echo "WARN: shellcheck not installed; skipping" >&2
  fi
  if command -v shfmt >/dev/null 2>&1; then
    shfmt -d -i 2 -ci -bn "$HOOKS_DIR" || FAILED=1
  else
    echo "WARN: shfmt not installed; skipping" >&2
  fi
}

if [[ "$RUN_SYNTAX" == "true" ]]; then
  run_syntax_check
fi

if [[ "$RUN_POLICY" == "true" ]]; then
  run_policy_check
fi

if [[ "$RUN_SMOKE" == "true" ]]; then
  run_smoke_check
fi

if [[ "$RUN_LINT" == "true" ]]; then
  run_lint_check
fi

if [[ $FAILED -ne 0 ]]; then
  echo "Verification failed" >&2
  exit 1
fi

echo "All checks passed"
exit 0
