#!/usr/bin/env bash
# gemini-appserver.sh — Thin Symphony shim for the Gemini CLI.
#
# Usage:
#   echo '{"type":"turn/start","input":[{"text":"..."}]}' | gemini-appserver.sh
#   gemini-appserver.sh /path/to/prompt-file.json
#
# Reads a turn/start JSON from stdin or a prompt file path passed as $1.
# Extracts the prompt text, runs `gemini -p` (headless mode), streams output,
# and emits synthetic Symphony JSON-RPC events on completion.
#
# Stall timeout: 300 seconds of no output kills the subprocess.
# Final line is always COMPLETE or STUCK.

set -uo pipefail

THREAD_ID="autoship-gemini"
STALL_TIMEOUT=300

# ---------------------------------------------------------------------------
# Guard: gemini must be installed
# ---------------------------------------------------------------------------
if ! command -v gemini >/dev/null 2>&1; then
  printf '{"type":"turn/failed","threadId":"%s","error":"gemini: command not found — install the Gemini CLI"}\n' "$THREAD_ID"
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
# Supports:
#   .input[0].text  (Symphony turn/start format)
#   .prompt         (simple format)
#   .content / .text (aliases)
# Falls back to raw input as plain text prompt.
# ---------------------------------------------------------------------------
PROMPT_TEXT=""
if command -v jq >/dev/null 2>&1; then
  PROMPT_TEXT=$(printf '%s' "$INPUT_JSON" \
    | jq -r '.input[0].text // .prompt // .content // .text // empty' 2>/dev/null || true)
fi

# Fallback: use raw input as prompt
if [[ -z "$PROMPT_TEXT" ]]; then
  PROMPT_TEXT="$INPUT_JSON"
fi

if [[ -z "$PROMPT_TEXT" ]]; then
  printf '{"type":"turn/failed","threadId":"%s","error":"No prompt text found in input"}\n' "$THREAD_ID"
  echo "STUCK"
  exit 1
fi

# ---------------------------------------------------------------------------
# Write prompt to temp file
# ---------------------------------------------------------------------------
TMPFILE=$(mktemp /tmp/gemini-prompt-XXXXXX.txt)
trap 'rm -f "$TMPFILE"' EXIT

printf '%s' "$PROMPT_TEXT" > "$TMPFILE"

# ---------------------------------------------------------------------------
# Run gemini CLI with stall-timeout watchdog
# Stream output line-by-line, tracking the last output timestamp.
# If no output for STALL_TIMEOUT seconds, kill the process.
# ---------------------------------------------------------------------------
EXIT_CODE=0
TOTAL_TOKENS=0
GEMINI_OUTPUT=""

# Run gemini in background, capture output via a temp pipe file
OUTFILE=$(mktemp /tmp/gemini-output-XXXXXX.txt)
trap 'rm -f "$TMPFILE" "$OUTFILE"' EXIT

# Use -p (headless prompt mode) + --yolo (auto-approve all tool calls).
# --prompt-file does not exist in Gemini CLI; -p reads the prompt as a string.
gemini -p "$(cat "$TMPFILE")" --yolo >"$OUTFILE" 2>&1 &
GEMINI_PID=$!

# Stream output with stall watchdog
LAST_SIZE=0
LAST_ACTIVITY=$(date +%s)

while kill -0 "$GEMINI_PID" 2>/dev/null; do
  CURRENT_SIZE=$(wc -c <"$OUTFILE" 2>/dev/null || echo 0)

  if [[ "$CURRENT_SIZE" -gt "$LAST_SIZE" ]]; then
    # New output arrived — print new bytes and reset stall timer
    tail -c +"$((LAST_SIZE + 1))" "$OUTFILE" 2>/dev/null || true
    LAST_SIZE="$CURRENT_SIZE"
    LAST_ACTIVITY=$(date +%s)
  fi

  NOW=$(date +%s)
  ELAPSED=$(( NOW - LAST_ACTIVITY ))
  if [[ "$ELAPSED" -ge "$STALL_TIMEOUT" ]]; then
    kill "$GEMINI_PID" 2>/dev/null || true
    wait "$GEMINI_PID" 2>/dev/null || true
    printf '{"type":"turn/failed","threadId":"%s","error":"stall timeout after %ds with no output"}\n' \
      "$THREAD_ID" "$STALL_TIMEOUT"
    echo "STUCK"
    exit 1
  fi

  sleep 1
done

# Wait for process and capture exit code
wait "$GEMINI_PID" 2>/dev/null || EXIT_CODE=$?

# Flush any remaining output
CURRENT_SIZE=$(wc -c <"$OUTFILE" 2>/dev/null || echo 0)
if [[ "$CURRENT_SIZE" -gt "$LAST_SIZE" ]]; then
  tail -c +"$((LAST_SIZE + 1))" "$OUTFILE" 2>/dev/null || true
fi

# ---------------------------------------------------------------------------
# Parse token usage from gemini output if present.
# Gemini CLI may print lines like:
#   "Total tokens: 1234"  or  "tokens: 1234"  or  "tokenCount: 1234"
# ---------------------------------------------------------------------------
GEMINI_OUTPUT=$(cat "$OUTFILE" 2>/dev/null || true)
PARSED_TOKENS=$(printf '%s' "$GEMINI_OUTPUT" \
  | grep -oiE '(total tokens|tokens|tokencount)[[:space:]]*:[[:space:]]*[0-9]+' \
  | grep -oE '[0-9]+$' \
  | tail -1 || true)

if [[ -n "$PARSED_TOKENS" ]]; then
  TOTAL_TOKENS="$PARSED_TOKENS"
fi

# ---------------------------------------------------------------------------
# Emit synthetic Symphony JSON-RPC completion events
# ---------------------------------------------------------------------------
if [[ $EXIT_CODE -eq 0 ]]; then
  printf '{"type":"turn/completed","threadId":"%s","status":"success"}\n' "$THREAD_ID"
  printf '{"type":"thread/tokenUsage/updated","threadId":"%s","totalTokens":%d}\n' \
    "$THREAD_ID" "$TOTAL_TOKENS"
  echo "COMPLETE"
else
  printf '{"type":"turn/failed","threadId":"%s","error":"exit %d"}\n' "$THREAD_ID" "$EXIT_CODE"
  echo "STUCK"
  exit 1
fi
