#!/usr/bin/env bash
set -euo pipefail

FLAME_RETRY_COUNT="${FLAME_RETRY_COUNT:-1}"
FLAME_RETRY_DELAY="${FLAME_RETRY_DELAY:-5}"
FLAME_TEST_COMMAND="${FLAME_TEST_COMMAND:-npm test}"

readonly FLAME_RETRY_COUNT
readonly FLAME_RETRY_DELAY

run_with_anti_flake() {
  local test_cmd="${1:-$FLAME_TEST_COMMAND}"
  local attempt=0
  local last_status=0
  local -a cmd_parts=()

  # Execute the command directly (no eval) to avoid shell injection when
  # test_cmd comes from repo-controlled configuration.
  read -r -a cmd_parts <<< "$test_cmd"
  if [[ "${#cmd_parts[@]}" -eq 0 ]]; then
    echo "[anti-flake] FAIL: empty test command" >&2
    return 1
  fi

  while (( attempt <= FLAME_RETRY_COUNT )); do
    ((attempt++))

    set +e
    "${cmd_parts[@]}" 2>&1
    last_status=$?
    set -e

    if [[ $last_status -eq 0 ]]; then
      if [[ $attempt -gt 1 ]]; then
        echo "[anti-flake] PASS on retry $((attempt-1))/$FLAME_RETRY_COUNT" >&2
      fi
      return 0
    fi

    if [[ $attempt -le $FLAME_RETRY_COUNT ]]; then
      echo "[anti-flake] FAIL (attempt $attempt/$FLAME_RETRY_COUNT), retrying in ${FLAME_RETRY_DELAY}s..." >&2
      sleep "$FLAME_RETRY_DELAY"
    fi
  done

  echo "[anti-flake] FAIL after $attempt attempts" >&2
  return $last_status
}

test_flake_config() {
  local test_cmd="${1:-$FLAME_TEST_COMMAND}"

  echo "Testing anti-flake configuration..."
  echo "  retry count: $FLAME_RETRY_COUNT"
  echo "  retry delay: ${FLAME_RETRY_DELAY}s"
  echo "  test command: $test_cmd"

  if [[ "$FLAME_RETRY_COUNT" -lt 1 ]]; then
    echo "ERROR: FLAME_RETRY_COUNT must be >= 1" >&2
    return 1
  fi

  echo "Configuration OK"
  return 0
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  COMMAND="${1:-}"
  shift || true

  case "$COMMAND" in
    run)
      run_with_anti_flake "${1:-$FLAME_TEST_COMMAND}"
      ;;
    test-config)
      test_flake_config "${1:-$FLAME_TEST_COMMAND}"
      ;;
    *)
      echo "Usage: $0 <command> [args...]" >&2
      echo "Commands:" >&2
      echo "  run [test-cmd]     - Run test with retry on failure" >&2
      echo "  test-config        - Verify configuration" >&2
      echo "" >&2
      echo "Environment:" >&2
      echo "  FLAME_RETRY_COUNT   - Number of retries (default: 1)" >&2
      echo "  FLAME_RETRY_DELAY - Delay between retries in seconds (default: 5)" >&2
      echo "  FLAME_TEST_COMMAND - Test command (default: npm test)" >&2
      exit 1
      ;;
  esac
fi
