#!/usr/bin/env bash
set -euo pipefail

# detect-tools.sh — Detect available AI CLI tools and output JSON report.

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || REPO_ROOT="$(pwd)"
# Outputs quota_pct for each tool: 100 if known-full, -1 if unknown, 0-100 if parseable.
# Always exits 0; missing tools are reported as unavailable, not errors.

# ---------------------------------------------------------------------------
# Quota helpers — return integer 0-100 or -1 (unknown)
# ---------------------------------------------------------------------------

QUOTA_FILE="${REPO_ROOT}/.autoship/quota.json"

# Claude uses a Max subscription — treat as always full.
quota_claude() {
  echo "100"
}

# Read quota from quota.json if it exists; fall back to 100 (optimistic default).
# Returns -1 only if the tool is unavailable (not installed).
_read_quota() {
  local tool="$1"
  if [[ -f "$QUOTA_FILE" ]]; then
    local q
    q=$(jq -r --arg t "$tool" '.[$t].quota_pct // -1' "$QUOTA_FILE" 2>/dev/null)
    echo "${q:--1}"
  else
    echo "100"  # No quota file yet → assume fresh/full
  fi
}

# Codex: try OpenAI usage API if OPENAI_API_KEY is set; fall back to decay estimate.
# Returns integer 0-100. Sets CODEX_QUOTA_SOURCE="api" or "est".
CODEX_QUOTA_SOURCE="est"
quota_codex_openai_api() {
  [[ -z "${OPENAI_API_KEY:-}" ]] && return 1
  # Fetch today's usage in dollars; compare to typical monthly budget
  local today
  today=$(date -u +"%Y-%m-%d")
  local response
  response=$(curl -sf --max-time 5 \
    "https://api.openai.com/v1/usage?date=${today}" \
    -H "Authorization: Bearer $OPENAI_API_KEY" 2>/dev/null) || return 1
  # Parse total_usage (in cents); assume ~$20 daily budget → 2000 cents
  local used_cents
  used_cents=$(echo "$response" | jq -r '[.data[].total_usage // 0] | add // 0' 2>/dev/null) || return 1
  local budget_cents=2000  # ~$20/day budget assumption
  local used_pct=$(( used_cents * 100 / budget_cents ))
  [[ $used_pct -gt 100 ]] && used_pct=100
  echo $(( 100 - used_pct ))
  CODEX_QUOTA_SOURCE="api"
  return 0
}

quota_codex_spark() {
  local v
  if v=$(quota_codex_openai_api 2>/dev/null); then
    echo "$v"
  else
    CODEX_QUOTA_SOURCE="est"
    _read_quota "codex-spark"
  fi
}
quota_codex_gpt() {
  local v
  if v=$(quota_codex_openai_api 2>/dev/null); then
    echo "$v"
  else
    CODEX_QUOTA_SOURCE="est"
    _read_quota "codex-gpt"
  fi
}

# Gemini: try Google AI usage API if GEMINI_API_KEY or GOOGLE_API_KEY is set; fall back to decay.
GEMINI_QUOTA_SOURCE="est"
quota_gemini_api() {
  local api_key="${GEMINI_API_KEY:-${GOOGLE_API_KEY:-}}"
  [[ -z "$api_key" ]] && return 1
  # Check quota via generativelanguage API — returns 429 when rate-limited
  local status
  status=$(curl -sf --max-time 5 -o /dev/null -w "%{http_code}" \
    "https://generativelanguage.googleapis.com/v1beta/models?key=${api_key}" 2>/dev/null) || return 1
  case "$status" in
    200) echo "100"; GEMINI_QUOTA_SOURCE="api" ;;  # reachable → assume full
    429) echo "0";   GEMINI_QUOTA_SOURCE="api" ;;  # rate limited → exhausted
    *)   return 1 ;;
  esac
}

quota_gemini() {
  local v
  if v=$(quota_gemini_api 2>/dev/null); then
    echo "$v"
  else
    GEMINI_QUOTA_SOURCE="est"
    _read_quota "gemini"
  fi
}

# ---------------------------------------------------------------------------
# Per-tool detection
# ---------------------------------------------------------------------------

detect_claude() {
  if command -v claude >/dev/null 2>&1; then
    local ver qpct
    ver=$(claude --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    qpct=$(quota_claude)
    printf '"claude": {"available": true, "version": "%s", "quota_pct": %s}' "$ver" "$qpct"
  else
    printf '"claude": {"available": false, "quota_pct": -1}'
  fi
}

detect_codex() {
  if command -v codex >/dev/null 2>&1; then
    local ver spark_q gpt_q
    ver=$(codex --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')

    # Probe app-server availability — both codex variants use the same binary
    if ! codex app-server help >/dev/null 2>&1; then
      bash "${REPO_ROOT}/hooks/quota-update.sh" stuck "codex-spark" || true
      bash "${REPO_ROOT}/hooks/quota-update.sh" stuck "codex-gpt"   || true
    fi

    spark_q=$(quota_codex_spark)
    gpt_q=$(quota_codex_gpt)
    printf '"codex-spark": {"available": true, "version": "%s", "quota_pct": %s, "quota_source": "%s"}, ' \
      "$ver" "$spark_q" "$CODEX_QUOTA_SOURCE"
    printf '"codex-gpt": {"available": true, "version": "%s", "quota_pct": %s, "quota_source": "%s"}' \
      "$ver" "$gpt_q" "$CODEX_QUOTA_SOURCE"
  else
    printf '"codex-spark": {"available": false, "quota_pct": -1, "quota_source": "n/a"}, '
    printf '"codex-gpt": {"available": false, "quota_pct": -1, "quota_source": "n/a"}'
  fi
}

detect_gemini() {
  if command -v gemini >/dev/null 2>&1 && gemini --version >/dev/null 2>&1; then
    local ver qpct
    ver=$(gemini --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    qpct=$(quota_gemini)
    printf '"gemini": {"available": true, "version": "%s", "quota_pct": %s, "quota_source": "%s"}' \
      "$ver" "$qpct" "$GEMINI_QUOTA_SOURCE"
  else
    printf '"gemini": {"available": false, "quota_pct": -1, "quota_source": "n/a"}'
  fi
}

detect_copilot() {
  if command -v gh >/dev/null 2>&1 && gh copilot --version >/dev/null 2>&1; then
    local ver
    ver=$(gh copilot --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    printf '"copilot": {"available": true, "version": "%s", "quota_pct": -1, "exhausted": false}' "$ver"
  elif command -v copilot >/dev/null 2>&1; then
    local ver
    ver=$(copilot --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    printf '"copilot": {"available": true, "version": "%s", "quota_pct": -1, "exhausted": false}' "$ver"
  else
    printf '"copilot": {"available": false, "quota_pct": -1}'
  fi
}

# ---------------------------------------------------------------------------
# Build JSON output
# ---------------------------------------------------------------------------

echo -n "{"
detect_claude
echo -n ", "
detect_codex
echo -n ", "
detect_gemini
echo -n ", "
detect_copilot
echo "}"

exit 0
