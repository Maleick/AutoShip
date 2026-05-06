#!/usr/bin/env bash
set -euo pipefail

GH_RETRY_MAX_ATTEMPTS="${GH_RETRY_MAX_ATTEMPTS:-3}"
GH_RETRY_BASE_DELAY="${GH_RETRY_BASE_DELAY:-2}"
GH_RETRY_MAX_DELAY="${GH_RETRY_MAX_DELAY:-30}"
GH_RETRY_HEARTBEAT="${GH_RETRY_HEARTBEAT:-false}"

readonly GH_RETRY_MAX_ATTEMPTS
readonly GH_RETRY_BASE_DELAY
readonly GH_RETRY_MAX_DELAY

log_heartbeat() {
  if [[ "$GH_RETRY_HEARTBEAT" == "true" ]]; then
    echo "[gh-retry] attempt $1: $2" >&2
  fi
}

exponential_backoff() {
  local attempt="$1"
  local delay=$((GH_RETRY_BASE_DELAY * (2 ** (attempt - 1))))
  if ((delay > GH_RETRY_MAX_DELAY)); then
    delay=$GH_RETRY_MAX_DELAY
  fi
  echo "$delay"
}

gh_retry() {
  local attempt=1
  local exit_code=0
  local output
  local error_output

  while ((attempt <= GH_RETRY_MAX_ATTEMPTS)); do
    log_heartbeat "$attempt" "executing: gh $*"

    set +e
    output=$(gh "$@" 2>&1)
    exit_code=$?
    set -e

    if ((exit_code == 0)); then
      echo "$output"
      return 0
    fi

    error_output=$(echo "$output" | head -3)

    case "$exit_code" in
      1)
        if echo "$output" | grep -qiE "(not found|already exists|permission denied|unauthorized)"; then
          log_heartbeat "$attempt" "non-retryable error: $error_output"
          echo "$output" >&2
          return "$exit_code"
        fi
        ;;
      2)
        log_heartbeat "$attempt" "positional argument error"
        echo "$output" >&2
        return "$exit_code"
        ;;
      4)
        log_heartbeat "$attempt" "invalid flag/option"
        echo "$output" >&2
        return "$exit_code"
        ;;
    esac

    if ((attempt < GH_RETRY_MAX_ATTEMPTS)); then
      local delay
      delay=$(exponential_backoff "$attempt")
      log_heartbeat "$attempt" "failed with exit $exit_code, retrying in ${delay}s..."
      sleep "$delay"
    fi

    ((attempt++))
  done

  echo "$output" >&2
  return "$exit_code"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  if [[ $# -lt 1 ]]; then
    echo "Usage: $0 [--] <gh-command> [args...]" >&2
    echo "Or: source $0 && gh_retry <gh-command> [args...]" >&2
    echo "" >&2
    echo "Environment:" >&2
    echo "  GH_RETRY_MAX_ATTEMPTS  - max retry attempts (default: 3)" >&2
    echo "  GH_RETRY_BASE_DELAY   - base delay in seconds (default: 2)" >&2
    echo "  GH_RETRY_MAX_DELAY   - max delay in seconds (default: 30)" >&2
    echo "  GH_RETRY_HEARTBEAT   - print retry progress (default: false)" >&2
    exit 1
  fi

  if [[ "$1" == "--" ]]; then
    shift
  fi

  gh_retry "$@"
fi
