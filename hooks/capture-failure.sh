#!/usr/bin/env bash
# capture-failure.sh — Capture structured failure artifacts for self-improvement
#
# Usage:
#   bash hooks/capture-failure.sh <category> <issue-id> [key=value ...]
#
# Categories:
#   stuck              — Worker exited without terminal status
#   failed_verification — Verification hook(s) failed
#   reviewer_rejection  — PR reviewer rejected the work
#   model_failure       — Model/API returned an error
#   e2e_failure         — End-to-end test failed
#
# Artifacts captured:
#   - issue: issue number/ID
#   - model: model used
#   - workspace: workspace directory path
#   - hook: relevant hook script
#   - logs: failure logs
#   - failure_category: category above
#   - timestamp: ISO timestamp
#   - attempt: retry attempt number
#   - error_summary: brief error message

set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

AUTOSHIP_DIR=".autoship"
FAILURES_DIR="$AUTOSHIP_DIR/failures"
STATE_FILE="$AUTOSHIP_DIR/state.json"

CATEGORY="${1:-usage}"
shift 2>/dev/null || true

case "$CATEGORY" in
  stuck|failed_verification|reviewer_rejection|model_failure|e2e_failure) ;;
  *)
    echo "Usage: $0 <category> <issue-id> [key=value ...]" >&2
    echo "Categories: stuck, failed_verification, reviewer_rejection, model_failure, e2e_failure" >&2
    exit 1
    ;;
esac

ISSUE_ID="${1:-}"
if [[ -z "$ISSUE_ID" ]]; then
  echo "Error: issue-id required" >&2
  exit 1
fi

if [[ ! "$ISSUE_ID" =~ ^(issue-)?[0-9]+$ ]]; then
  echo "Error: invalid ISSUE_ID: $ISSUE_ID" >&2
  exit 1
fi

ISSUE_NUMBER="${ISSUE_ID#issue-}"
ISSUE_ID="issue-${ISSUE_NUMBER}"

mkdir -p "$FAILURES_DIR"

FAILURE_ID="$(date -u +"%Y%m%dT%H%M%SZ")-${ISSUE_ID}"
FAILURE_FILE="$FAILURES_DIR/${FAILURE_ID}.json"

WORKSPACE_DIR="$AUTOSHIP_DIR/workspaces/${ISSUE_ID}"
WORKSPACE_PATH="$REPO_ROOT/$WORKSPACE_DIR"
MODEL="unknown"
ROLE="unknown"
ATTEMPT=1

if [[ -f "$STATE_FILE" ]]; then
  MODEL=$(jq -r --arg k "$ISSUE_ID" '.issues[$k].model // "unknown"' "$STATE_FILE" 2>/dev/null || echo "unknown")
  ROLE=$(jq -r --arg k "$ISSUE_ID" '.issues[$k].role // .issues[$k].agent // "unknown"' "$STATE_FILE" 2>/dev/null || echo "unknown")
  ATTEMPT=$(jq -r --arg k "$ISSUE_ID" '.issues[$k].attempt // 1' "$STATE_FILE" 2>/dev/null || echo 1)
fi

if [[ -f "$WORKSPACE_DIR/model" ]]; then
  MODEL=$(cat "$WORKSPACE_DIR/model" 2>/dev/null || echo "$MODEL")
fi
if [[ -f "$WORKSPACE_DIR/role" ]]; then
  ROLE=$(cat "$WORKSPACE_DIR/role" 2>/dev/null || echo "$ROLE")
fi

ERROR_SUMMARY=""
for pair in "$@"; do
  if [[ "$pair" == error_summary=* ]]; then
    ERROR_SUMMARY="${pair#*=}"
    break
  fi
done

if [[ ! "$ATTEMPT" =~ ^[0-9]+$ ]]; then
  ATTEMPT=1
fi

LOG_FILE="${AUTOSHIP_FAILURE_LOG:-$WORKSPACE_DIR/AUTOSHIP_RUNNER.log}"
LOGS=""
if [[ -f "$LOG_FILE" ]]; then
  LOGS=$(tail -100 "$LOG_FILE" 2>/dev/null || true)
fi

HOOK="${AUTOSHIP_FAILURE_HOOK:-}"
case "$CATEGORY" in
  stuck)
    HOOK="${HOOK:-hooks/opencode/runner.sh}"
    ;;
  failed_verification)
    HOOK="${HOOK:-hooks/opencode/test-policy.sh}"
    ;;
  reviewer_rejection)
    HOOK="${HOOK:-hooks/opencode/reviewer.sh}"
    ;;
  model_failure)
    HOOK="${HOOK:-hooks/opencode/runner.sh}"
    ;;
  e2e_failure)
    HOOK="${HOOK:-hooks/opencode/smoke-test.sh}"
    ;;
esac

jq -n \
  --arg id "$FAILURE_ID" \
  --arg issue "$ISSUE_ID" \
  --arg category "$CATEGORY" \
  --arg model "$MODEL" \
  --arg role "$ROLE" \
  --arg workspace "$WORKSPACE_PATH" \
  --arg hook "$HOOK" \
  --arg logs "$LOGS" \
  --arg error "$ERROR_SUMMARY" \
  --argjson attempt "$ATTEMPT" \
  --arg now "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  '{
    failure_id: $id,
    issue: $issue,
    failure_category: $category,
    model: $model,
    role: $role,
    workspace: $workspace,
    hook: $hook,
    logs: $logs,
    error_summary: $error,
    attempt: $attempt,
    timestamp: $now
  }' > "$FAILURE_FILE"

if [[ -x "$REPO_ROOT/hooks/opencode/item-record.sh" ]]; then
  bash "$REPO_ROOT/hooks/opencode/item-record.sh" append "$ISSUE_NUMBER" failed "$ATTEMPT" "$MODEL" "$CATEGORY: $ERROR_SUMMARY" >/dev/null 2>&1 || true
fi

echo "Failure artifact captured: $FAILURE_FILE"
