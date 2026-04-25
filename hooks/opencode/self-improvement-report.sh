#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Error: not inside a git repository" >&2
  exit 1
}

FAILURES_DIR="$REPO_ROOT/.autoship/failures"

if [[ ! -d "$FAILURES_DIR" ]]; then
  cat <<'EOF'
# AutoShip Self-Improvement Report

No failure artifacts found.
EOF
  exit 0
fi

tmp_json="$(mktemp)"
trap 'rm -f "$tmp_json"' EXIT

if ! jq -s '[.[] | select(type == "object")]' "$FAILURES_DIR"/*.json > "$tmp_json" 2>/dev/null; then
  echo "# AutoShip Self-Improvement Report"
  echo
  echo "No readable failure artifacts found."
  exit 0
fi

total="$(jq 'length' "$tmp_json")"

echo "# AutoShip Self-Improvement Report"
echo
echo "Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "Failures analyzed: $total"
echo
echo "## Failure Summary"
jq -r '
  group_by(.failure_category // "unknown")[] |
  "- " + (.[0].failure_category // "unknown") + ": " + (length | tostring) + " failure(s)"
' "$tmp_json"
echo
echo "## Root Cause Evidence"
jq -r '
  group_by((.failure_category // "unknown") + "|" + (.error_summary // ""))[] |
  .[0] as $first |
  "### " + ($first.failure_category // "unknown") + " — " + (if ($first.error_summary // "") == "" then "no summary" else $first.error_summary end) + "\n" +
  "- Occurrences: " + (length | tostring) + "\n" +
  "- Issues: " + ([.[].issue // "unknown"] | unique | join(", ")) + "\n" +
  "- Models: " + ([.[].model // "unknown"] | unique | join(", ")) + "\n" +
  "- Evidence: " + (($first.logs // $first.error_summary // "") | split("\n") | map(select(length > 0)) | .[0:3] | join(" / ")) + "\n"
' "$tmp_json"
echo "## Affected Files"
jq -r '
  [.[].hook // empty] | unique | .[] | "- " + .
' "$tmp_json"
echo
echo "## Candidate Acceptance Criteria"

if jq -e '[.[] | select(((.logs // "") + " " + (.error_summary // "")) | test("Insufficient balance"; "i"))] | length > 0' "$tmp_json" >/dev/null; then
  echo "- When a paid model returns an insufficient-balance error, paid model balance failures fall back to a configured free model and record the fallback in routing logs."
fi

if jq -e '[.[] | select(.failure_category == "stuck")] | length > 0' "$tmp_json" >/dev/null; then
  echo "- When a worker stops making progress, AutoShip records the stale RUNNING workspace as stuck with process evidence and retry eligibility."
fi

if jq -e '[.[] | select(.failure_category == "failed_verification")] | length > 0' "$tmp_json" >/dev/null; then
  echo "- When verification fails, AutoShip includes the failing hook, log excerpt, and candidate fix criteria in the next retry prompt."
fi

if jq -e '[.[] | select(.failure_category == "reviewer_rejection")] | length > 0' "$tmp_json" >/dev/null; then
  echo "- When reviewer rejection occurs, AutoShip preserves reviewer evidence and requires the retry to address each rejected acceptance criterion."
fi

echo "- Each self-improvement recommendation links a recurring failure category to root cause evidence and affected files."
