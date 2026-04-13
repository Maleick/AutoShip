#!/usr/bin/env bash
# DEPRECATED: Grok CLI removed — does not support OAuth authentication.
# This shim is no longer used. Kept for git history only.
echo "ERROR: grok-appserver.sh is deprecated. Use copilot or gemini instead." >&2
exit 1

# grok-appserver.sh — Thin Symphony shim for the Grok CLI.
#
# DEPRECATED: Grok CLI has no OAuth support and is being phased out.
# tokens_used is always reported as -1 (sentinel: unparseable/unavailable).
#
# Usage:
#   echo '{"type":"turn/start","prompt":"..."}' | grok-appserver.sh
#   grok-appserver.sh /path/to/prompt-file.json
#
# Reads a turn/start JSON from stdin or a prompt file path passed as $1.
# Extracts the prompt text, runs `grok --prompt-file`, streams output,
# and emits synthetic Symphony JSON-RPC events on completion.
#
# Final line is always COMPLETE or STUCK.

set -euo pipefail

# ---------------------------------------------------------------------------
# Guard: grok must be installed
# ---------------------------------------------------------------------------
if ! command -v grok >/dev/null 2>&1; then
  printf '{"type":"turn/error","threadId":"grok-shim","error":"grok: command not found — install the Grok CLI"}\n' >&2
  echo "STUCK"
  exit 1
fi

# ---------------------------------------------------------------------------
# Read input: $1 (file path) or stdin
# ---------------------------------------------------------------------------
INPUT_JSON=""
if [[ -n "${1:-}" && -f "$1" ]]; then
  INPUT_JSON=$(cat "$1")
else
  INPUT_JSON=$(cat)
fi

# ---------------------------------------------------------------------------
# Extract prompt text from turn/start JSON
# ---------------------------------------------------------------------------
PROMPT_TEXT=""
if command -v jq >/dev/null 2>&1; then
  PROMPT_TEXT=$(printf '%s' "$INPUT_JSON" | jq -r '.prompt // .content // .text // empty' 2>/dev/null || true)
fi

# Fallback: strip JSON, use raw input as prompt
if [[ -z "$PROMPT_TEXT" ]]; then
  PROMPT_TEXT="$INPUT_JSON"
fi

if [[ -z "$PROMPT_TEXT" ]]; then
  printf '{"type":"turn/error","threadId":"grok-shim","error":"No prompt text found in input"}\n' >&2
  echo "STUCK"
  exit 1
fi

# ---------------------------------------------------------------------------
# Extract thread ID (optional; default to timestamp-based ID)
# ---------------------------------------------------------------------------
THREAD_ID=""
if command -v jq >/dev/null 2>&1; then
  THREAD_ID=$(printf '%s' "$INPUT_JSON" | jq -r '.threadId // empty' 2>/dev/null || true)
fi
if [[ -z "$THREAD_ID" ]]; then
  THREAD_ID="grok-$(date +%s)"
fi

# ---------------------------------------------------------------------------
# Write prompt to temp file
# ---------------------------------------------------------------------------
TMPFILE=$(mktemp /tmp/grok-prompt-XXXXXX.txt)
trap 'rm -f "$TMPFILE"' EXIT

printf '%s' "$PROMPT_TEXT" > "$TMPFILE"

# ---------------------------------------------------------------------------
# Run grok CLI and stream output
# ---------------------------------------------------------------------------
EXIT_CODE=0
grok --prompt-file "$TMPFILE" || EXIT_CODE=$?

# ---------------------------------------------------------------------------
# Emit synthetic Symphony JSON-RPC completion events.
# Token count is always -1 (sentinel) — Grok CLI does not expose token usage.
# ---------------------------------------------------------------------------
if [[ $EXIT_CODE -eq 0 ]]; then
  printf '{"type":"turn/completed","threadId":"%s","status":"success"}\n' "$THREAD_ID"
  printf '{"type":"thread/tokenUsage/updated","threadId":"%s","totalTokens":-1}\n' "$THREAD_ID"
  echo "COMPLETE"
else
  printf '{"type":"turn/error","threadId":"%s","status":"error","exitCode":%d}\n' \
    "$THREAD_ID" "$EXIT_CODE" >&2
  echo "STUCK"
  exit 1
fi
