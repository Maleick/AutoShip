#!/usr/bin/env bash
set -euo pipefail

PR_NUMBER="${1:?PR number required}"
if [[ ! "$PR_NUMBER" =~ ^[0-9]+$ ]]; then
  echo "Error: PR number must be numeric" >&2
  exit 1
fi
labels=$(gh pr view "$PR_NUMBER" --json labels --jq '[.labels[].name] | join(",")')
if ! grep -Eq '(^|,)autoship:auto-merge(,|$)' <<<"$labels"; then
  echo "PR #$PR_NUMBER is not labeled autoship:auto-merge"
  exit 0
fi
checks=$(gh pr view "$PR_NUMBER" --json statusCheckRollup --jq '[.statusCheckRollup[]? | .conclusion // .status // empty] | join(",")')
if [[ -z "$checks" ]]; then
  echo "PR #$PR_NUMBER has no reported checks; refusing auto-merge"
  exit 1
fi
if grep -Eq 'FAILURE|ERROR|CANCELLED|PENDING|QUEUED|IN_PROGRESS' <<<"$checks"; then
  echo "PR #$PR_NUMBER checks are not mergeable: $checks"
  exit 1
fi
gh pr merge "$PR_NUMBER" --squash --delete-branch
