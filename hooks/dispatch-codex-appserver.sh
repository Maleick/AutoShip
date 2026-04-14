#!/usr/bin/env bash
# dispatch-codex-appserver.sh — Dispatch Codex CLI via `codex exec` (headless)
# Usage: bash hooks/dispatch-codex-appserver.sh <issue-key> <prompt-file>
# Emits COMPLETE or STUCK to .autoship/workspaces/<issue-key>/pane.log
# Emits verify or agent_stuck event to .autoship/event-queue.json
#
# Uses `codex exec` (non-interactive mode) with stdin pipe.
# Sandbox: --dangerously-bypass-approvals-and-sandbox for full file write access.
# Previous JSON-RPC `codex app-server` approach removed (non-functional in codex-cli 0.120+).

set -euo pipefail

source "${BASH_SOURCE%/*}/lib/dispatch-helpers.sh"

ISSUE_KEY="${1:?usage: dispatch-codex-appserver.sh <issue-key> <prompt-file>}"
PROMPT_FILE="${2:?usage: dispatch-codex-appserver.sh <issue-key> <prompt-file>}"

# Validate ISSUE_KEY to prevent path traversal
if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+[a-z0-9-]*$ ]]; then
  echo "Error: invalid ISSUE_KEY: $ISSUE_KEY" >&2
  exit 1
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORKSPACE="${REPO_ROOT}/.autoship/workspaces/${ISSUE_KEY}"
PANE_LOG="${WORKSPACE}/pane.log"
HOOKS_DIR="$(cat "${REPO_ROOT}/.autoship/hooks_dir" 2>/dev/null || echo "${REPO_ROOT}/hooks")"

STALL_SECS=$(( ${STALL_TIMEOUT_MS:-300000} / 1000 ))

# Resolve which tool this dispatch is for — used in all stuck/exhausted paths
TOOL=$(jq -r --arg id "$ISSUE_KEY" '.issues[$id].agent // "codex-spark"' "${REPO_ROOT}/.autoship/state.json" 2>/dev/null || echo "codex-spark")

# Fast-fail health check (cross-platform timeout)
HEALTH_CHECK_FAILED=0
if command -v gtimeout >/dev/null 2>&1; then
  gtimeout 10s codex --version >/dev/null 2>&1 || HEALTH_CHECK_FAILED=1
elif command -v timeout >/dev/null 2>&1; then
  timeout 10s codex --version >/dev/null 2>&1 || HEALTH_CHECK_FAILED=1
else
  codex --version >/dev/null 2>&1 || HEALTH_CHECK_FAILED=1
fi

if [[ "$HEALTH_CHECK_FAILED" -eq 1 ]]; then
  echo "STUCK" >> "$PANE_LOG"
  bash "$HOOKS_DIR/quota-update.sh" stuck "$TOOL" || true

  ISSUE_NUMBER="${ISSUE_KEY#issue-}"
  EVENT=$(jq -n \
    --arg type    "agent_stuck" \
    --arg issue   "$ISSUE_KEY" \
    --arg issueN  "$ISSUE_NUMBER" \
    --argjson tok 0 \
    --arg ts      "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    '{type: $type, issue: $issue, issue_number: ($issueN | tonumber), tokens_used: $tok, timestamp: $ts}')
  bash "$HOOKS_DIR/emit-event.sh" "$EVENT" 2>/dev/null || true
  exit 0
fi

if [[ ! -f "$PROMPT_FILE" ]]; then
  echo "ERROR: prompt file not found: $PROMPT_FILE" >&2
  echo "STUCK" >> "$PANE_LOG"
  exit 1
fi

mkdir -p "$WORKSPACE"

# Clean up stale watcher and increment attempt counter
kill_old_watcher "$WORKSPACE"
ATTEMPT_COUNT_FILE="${WORKSPACE}/.attempt_count"
ATTEMPT_NUM=$(( $(cat "$ATTEMPT_COUNT_FILE" 2>/dev/null || echo 0) + 1 ))
echo "$ATTEMPT_NUM" > "$ATTEMPT_COUNT_FILE"
echo "=== Dispatch attempt #$ATTEMPT_NUM ===" >> "$PANE_LOG"

# ---------------------------------------------------------------------------
# Worktree → standalone repo conversion (fixes Codex sandbox issue #133)
# Codex exec -C scopes its sandbox to WORKSPACE. Git worktrees store
# metadata in <parent>/.git/worktrees/<name>/ which is outside the sandbox,
# so git commit fails with "Operation not permitted" on index.lock.
# Fix: absorb the worktree .git pointer into a real .git/ directory.
# ---------------------------------------------------------------------------
if [[ -f "${WORKSPACE}/.git" ]]; then
  WORKTREE_GITDIR=$(sed -n 's/^gitdir: //p' "${WORKSPACE}/.git")
  # Resolve relative gitdir paths (relative to WORKSPACE)
  if [[ "$WORKTREE_GITDIR" != /* ]]; then
    WORKTREE_GITDIR="$(cd "${WORKSPACE}" && cd "${WORKTREE_GITDIR}" && pwd)"
  fi
  if [[ -n "$WORKTREE_GITDIR" && -d "$WORKTREE_GITDIR" ]]; then
    # Resolve parent .git dir (absolute) — commondir is relative from worktree gitdir
    PARENT_GITDIR=$(cd "$WORKTREE_GITDIR" && cd "$(git rev-parse --git-common-dir 2>/dev/null)" && pwd)
    if [[ -n "$PARENT_GITDIR" && -d "$PARENT_GITDIR" ]]; then
      rm "${WORKSPACE}/.git"
      mkdir -p "${WORKSPACE}/.git"
      # Copy worktree-specific metadata (these are per-worktree, not shared)
      for f in HEAD ORIG_HEAD MERGE_HEAD FETCH_HEAD index COMMIT_EDITMSG; do
        [[ -f "${WORKTREE_GITDIR}/${f}" ]] && cp "${WORKTREE_GITDIR}/${f}" "${WORKSPACE}/.git/"
      done
      # Symlink shared resources from the parent repo (absolute paths for Codex sandbox)
      ln -sf "${PARENT_GITDIR}/objects"     "${WORKSPACE}/.git/objects"
      ln -sf "${PARENT_GITDIR}/refs"        "${WORKSPACE}/.git/refs"
      ln -sf "${PARENT_GITDIR}/packed-refs" "${WORKSPACE}/.git/packed-refs" 2>/dev/null || true
      ln -sf "${PARENT_GITDIR}/config"      "${WORKSPACE}/.git/config"
      ln -sf "${PARENT_GITDIR}/hooks"       "${WORKSPACE}/.git/hooks"  2>/dev/null || true
      ln -sf "${PARENT_GITDIR}/info"        "${WORKSPACE}/.git/info"   2>/dev/null || true
      echo "Converted worktree to standalone .git (sandbox fix)" >> "$PANE_LOG"
    fi
  fi
fi

# ---------------------------------------------------------------------------
# Run codex exec with stdin pipe and stall watchdog
# ---------------------------------------------------------------------------
EXIT_CODE=0
CODEX_PID=""

cleanup() {
  [[ -n "${CODEX_PID:-}" ]] && kill "$CODEX_PID" 2>/dev/null || true
  [[ -n "${WATCHDOG_PID:-}" ]] && kill "$WATCHDOG_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Determine sandbox mode from environment or default to full bypass
SANDBOX_FLAG="${CODEX_SANDBOX_FLAG:---dangerously-bypass-approvals-and-sandbox}"

# Pipe prompt via stdin to codex exec (avoids shell argument length limits)
cat "$PROMPT_FILE" | codex exec $SANDBOX_FLAG -C "$WORKSPACE" >> "$PANE_LOG" 2>&1 &
CODEX_PID=$!

# Stall watchdog — kills codex if deadline exceeded
(
  sleep "$STALL_SECS"
  kill "$CODEX_PID" 2>/dev/null || true
  echo "STUCK" >> "$PANE_LOG"
) &
WATCHDOG_PID=$!

# Wait for codex to finish
wait "$CODEX_PID" 2>/dev/null || EXIT_CODE=$?
CODEX_PID=""

# Cancel watchdog
kill "$WATCHDOG_PID" 2>/dev/null || true
WATCHDOG_PID=""

# ---------------------------------------------------------------------------
# Determine status
# ---------------------------------------------------------------------------
STATUS=""
if [[ -f "${WORKSPACE}/AUTOSHIP_RESULT.md" ]]; then
  STATUS="COMPLETE"
elif [[ $EXIT_CODE -eq 0 ]]; then
  # Codex exited cleanly but didn't produce a result file.
  # Check if it made any git changes (uncommitted or committed).
  CHANGES=$(git -C "$WORKSPACE" diff --name-only HEAD -- ':!pane.log' ':!.watcher.pid' ':!.codex*' ':!run-agent.sh' 2>/dev/null | wc -l | tr -d ' ')
  COMMITS=$(git -C "$WORKSPACE" log "$(git -C "$WORKSPACE" merge-base HEAD master 2>/dev/null || echo HEAD)"..HEAD --oneline 2>/dev/null | wc -l | tr -d ' ')
  if [[ "$CHANGES" -gt 0 ]] || [[ "$COMMITS" -gt 0 ]]; then
    STATUS="COMPLETE"
  else
    STATUS="STUCK"
  fi
else
  STATUS="STUCK"
fi

echo "$STATUS" >> "$PANE_LOG"

if [[ "$STATUS" == "STUCK" ]]; then
  bash "$HOOKS_DIR/quota-update.sh" stuck "$TOOL" || true
fi

# ---------------------------------------------------------------------------
# Parse token usage from codex output if present
# ---------------------------------------------------------------------------
TOKENS_USED=0
if [[ -f "$PANE_LOG" ]]; then
  PARSED=$(grep -oiE '(total tokens|tokens used|token_count)[[:space:]]*[:=][[:space:]]*[0-9]+' "$PANE_LOG" \
    | grep -oE '[0-9]+$' | tail -1 || true)
  [[ -n "$PARSED" ]] && TOKENS_USED="$PARSED"
fi

# ---------------------------------------------------------------------------
# Emit event to queue
# ---------------------------------------------------------------------------
ISSUE_NUMBER="${ISSUE_KEY#issue-}"
EVENT_TYPE="$( [[ "$STATUS" == "COMPLETE" ]] && echo "verify" || echo "agent_stuck" )"
EVENT=$(jq -n \
  --arg type    "$EVENT_TYPE" \
  --arg issue   "$ISSUE_KEY" \
  --arg issueN  "$ISSUE_NUMBER" \
  --argjson tok "$TOKENS_USED" \
  --arg ts      "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  '{type: $type, issue: $issue, issue_number: ($issueN | sub("[^0-9].*"; "") | tonumber), tokens_used: $tok, timestamp: $ts}')

bash "$HOOKS_DIR/emit-event.sh" "$EVENT" 2>/dev/null || true

# Update token count in state
bash "$HOOKS_DIR/update-state.sh" set-running "$ISSUE_KEY" "tokens_used=${TOKENS_USED}" 2>/dev/null || true

exit 0
